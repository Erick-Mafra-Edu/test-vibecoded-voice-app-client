//! Minimal SFU WebRTC transport.
//!
//! The WebSocket remains the signaling channel; RTP/Opus never crosses it.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use bytes::{Bytes, BytesMut};
use opus::{Application, Channels, Decoder, Encoder};
use rtc::{
    media::Sample,
    media_stream::MediaStreamTrack,
    peer_connection::{
        configuration::{
            interceptor_registry::register_default_interceptors, media_engine::MediaEngine,
            RTCConfigurationBuilder, RTCIceServer,
        },
        sdp::RTCSessionDescription,
        transport::RTCIceCandidateInit,
    },
    rtp_transceiver::rtp_sender::{
        RTCRtpCodec, RTCRtpCodingParameters, RTCRtpEncodingParameters, RtpCodecKind,
    },
    statistics::{report::RTCStatsReportEntry, StatsSelector},
};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use webrtc::{
    data_channel::{DataChannel, DataChannelEvent},
    media_stream::{
        track_local::{static_sample::TrackLocalStaticSample, TrackLocal},
        track_remote::{TrackRemote, TrackRemoteEvent},
    },
    peer_connection::{
        PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCPeerConnectionState,
    },
};

use super::{audio, e2e::Cipher};

const SAMPLE_RATE: usize = 48_000;
const FRAME_SAMPLES: usize = 960;

#[derive(Clone)]
struct Handler {
    decoded: mpsc::Sender<(String, Route, Vec<f32>)>,
    signaling: mpsc::UnboundedSender<Value>,
    status: mpsc::UnboundedSender<AudioStatus>,
    last_decoded: Arc<Mutex<HashMap<(String, Route), Instant>>>,
    cipher: Arc<Cipher>,
    screen_channels: Arc<Mutex<HashMap<String, Arc<dyn DataChannel>>>>,
    screen_tx: mpsc::Sender<(String, Vec<u8>)>,
    target: CandidateTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Route {
    P2p,
    Sfu,
    None,
}

pub enum AudioStatus {
    PeerConnection { user: String, connected: bool },
    RemoteAudio,
}

#[derive(Clone)]
enum CandidateTarget {
    Sfu(String),
    Peer(String),
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for Handler {
    async fn on_data_channel(&self, channel: Arc<dyn DataChannel>) {
        if channel.label().await.ok().as_deref() != Some("screen") {
            return;
        }
        if let CandidateTarget::Peer(user) = &self.target {
            register_screen_channel(
                user.clone(),
                channel,
                self.screen_channels.clone(),
                self.screen_tx.clone(),
                self.cipher.clone(),
            );
        }
    }
    async fn on_ice_candidate(&self, event: webrtc::peer_connection::RTCPeerConnectionIceEvent) {
        if let Ok(candidate) = event.candidate.to_json() {
            let candidate = json!({
                "candidate": candidate.candidate,
                "sdpMid": candidate.sdp_mid.filter(|mid| !mid.is_empty()).unwrap_or_else(|| "0".to_owned()),
                "sdpMLineIndex": candidate.sdp_mline_index,
                "usernameFragment": candidate.username_fragment,
            });
            let message = match &self.target {
                CandidateTarget::Sfu(transport) => {
                    json!({"type":"media_candidate","transport":transport,"candidate":candidate})
                }
                CandidateTarget::Peer(to) => {
                    json!({"type":"signal","to":to,"candidate":candidate})
                }
            };
            let _ = self.signaling.send(message);
        }
    }

    async fn on_track(&self, track: Arc<dyn TrackRemote>) {
        log::info!("audio track received");
        let decoded = self.decoded.clone();
        let status = self.status.clone();
        let (publisher, route) = match &self.target {
            CandidateTarget::Sfu(user) => (user.clone(), Route::Sfu),
            CandidateTarget::Peer(user) => (user.clone(), Route::P2p),
        };
        let last_decoded = self.last_decoded.clone();
        let cipher = self.cipher.clone();
        tokio::spawn(async move {
            let mut decoder = match Decoder::new(SAMPLE_RATE as u32, Channels::Mono) {
                Ok(decoder) => decoder,
                Err(error) => {
                    log::warn!("could not create Opus decoder: {error}");
                    return;
                }
            };
            let mut received_audio = false;
            while let Some(event) = track.poll().await {
                if let TrackRemoteEvent::OnRtpPacket(packet) = event {
                    let encoded = match cipher.open(&publisher, &packet.payload) {
                        Ok(encoded) => encoded,
                        Err(_) => continue,
                    };
                    let mut pcm = vec![0.0f32; FRAME_SAMPLES];
                    match decoder.decode_float(&encoded, &mut pcm, false) {
                        Ok(samples) => {
                            if !received_audio {
                                log::info!("first remote audio packet decoded");
                                let _ = status.send(AudioStatus::RemoteAudio);
                                received_audio = true;
                            }
                            pcm.truncate(samples);
                            last_decoded
                                .lock()
                                .unwrap()
                                .insert((publisher.clone(), route), Instant::now());
                            let _ = decoded.try_send((publisher.clone(), route, pcm));
                        }
                        Err(error) => log::debug!("Opus decode error: {error}"),
                    }
                }
            }
        });
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        if let CandidateTarget::Peer(user) = &self.target {
            let _ = self.status.send(AudioStatus::PeerConnection {
                user: user.clone(),
                connected: state == RTCPeerConnectionState::Connected,
            });
        }
        if state == RTCPeerConnectionState::Connected {
            log::info!("WebRTC audio connection established");
        }
        if matches!(
            state,
            RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed
        ) {
            log::warn!("SFU WebRTC connection ended: {state:?}");
        }
    }
}

pub struct SfuSession {
    user: String,
    outbound_tracks: Arc<Mutex<HashMap<String, Arc<TrackLocalStaticSample>>>>,
    muted: Arc<AtomicBool>,
    capture: audio::AudioCapture,
    playback: audio::AudioPlayback,
    peers: HashMap<String, Arc<dyn PeerConnection>>,
    decoded_tx: mpsc::Sender<(String, Route, Vec<f32>)>,
    active_routes: Arc<Mutex<HashMap<String, Route>>>,
    last_decoded: Arc<Mutex<HashMap<(String, Route), Instant>>>,
    cipher: Arc<Cipher>,
    screen_channels: Arc<Mutex<HashMap<String, Arc<dyn DataChannel>>>>,
    screen_tx: mpsc::Sender<(String, Vec<u8>)>,
    screen_rx: mpsc::Receiver<(String, Vec<u8>)>,
    metrics_previous: HashMap<String, (u64, i64, u64, Instant)>,
    signaling_tx: mpsc::UnboundedSender<Value>,
    signaling_rx: mpsc::UnboundedReceiver<Value>,
    status_tx: mpsc::UnboundedSender<AudioStatus>,
    status_rx: mpsc::UnboundedReceiver<AudioStatus>,
}

impl SfuSession {
    pub async fn start(
        user: String,
        room: &str,
        input: &str,
        output: &str,
        muted: bool,
    ) -> Result<(Self, Vec<Value>), String> {
        let passphrase = std::env::var("PULSE_E2E_PASSPHRASE")
            .map_err(|_| "configure PULSE_E2E_PASSPHRASE to enable voice".to_owned())?;
        let cipher = Arc::new(Cipher::new(&passphrase, room)?);
        let (capture, frames) = audio::start_capture_with_frames(input)?;
        let (playback, playback_frames) = audio::start_playback_with_frames(output)?;
        let (decoded_tx, decoded_rx) = mpsc::channel(256);
        let (screen_tx, screen_rx) = mpsc::channel(2);
        let screen_channels = Arc::new(Mutex::new(HashMap::new()));
        let active_routes = Arc::new(Mutex::new(HashMap::new()));
        let last_decoded = Arc::new(Mutex::new(HashMap::new()));
        let (signaling_tx, signaling_rx) = mpsc::unbounded_channel();
        let (status_tx, status_rx) = mpsc::unbounded_channel();
        let outbound_tracks = Arc::new(Mutex::new(
            HashMap::<String, Arc<TrackLocalStaticSample>>::new(),
        ));
        let sender_tracks = outbound_tracks.clone();
        let muted_flag = Arc::new(AtomicBool::new(muted));
        let sender_muted = muted_flag.clone();
        let sender_cipher = cipher.clone();
        let sender_user = user.clone();
        std::thread::Builder::new()
            .name("pulse-opus-uplink".to_owned())
            .spawn(move || {
                let mut encoder =
                    match Encoder::new(SAMPLE_RATE as u32, Channels::Mono, Application::Audio) {
                        Ok(encoder) => encoder,
                        Err(error) => {
                            log::warn!("could not create Opus encoder: {error}");
                            return;
                        }
                    };
                let mut pending = Vec::new();
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Tokio runtime for Opus uplink");
                while let Ok(frame) = frames.recv() {
                    if sender_muted.load(Ordering::Relaxed) {
                        pending.clear();
                        continue;
                    }
                    pending.extend(frame);
                    while pending.len() >= FRAME_SAMPLES {
                        let pcm: Vec<f32> = pending.drain(..FRAME_SAMPLES).collect();
                        let mut encoded = vec![0u8; 4000];
                        match encoder.encode_float(&pcm, &mut encoded) {
                            Ok(length) => {
                                encoded.truncate(length);
                                let encoded = match sender_cipher.seal(&sender_user, &encoded) {
                                    Ok(encoded) => encoded,
                                    Err(error) => {
                                        log::warn!("{error}");
                                        continue;
                                    }
                                };
                                let sample = Sample {
                                    data: Bytes::from(encoded),
                                    duration: Duration::from_millis(20),
                                    ..Sample::new(Instant::now())
                                };
                                let tracks = sender_tracks
                                    .lock()
                                    .unwrap()
                                    .values()
                                    .cloned()
                                    .collect::<Vec<_>>();
                                for track in tracks {
                                    let _ =
                                        runtime.block_on(track.write_sample(1, 111, &sample, &[]));
                                }
                            }
                            Err(error) => log::debug!("Opus encode error: {error}"),
                        }
                    }
                }
            })
            .map_err(|error| format!("could not start Opus encoder: {error}"))?;

        let playback_sender = playback_frames;
        let playback_routes = active_routes.clone();
        tokio::spawn(async move {
            let mut decoded_rx = decoded_rx;
            let mut latest = HashMap::<String, Vec<f32>>::new();
            let mut tick = tokio::time::interval(Duration::from_millis(20));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    frame = decoded_rx.recv() => match frame {
                        Some((user, route, frame)) => {
                            if playback_routes.lock().unwrap().get(&user).copied()
                                .unwrap_or(Route::P2p) == route {
                                latest.insert(user, frame);
                            }
                        }
                        None => break,
                    },
                    _ = tick.tick() => {
                        if !latest.is_empty() {
                            let count = (latest.len() as f32).sqrt();
                            let mut mixed = vec![0.0; FRAME_SAMPLES];
                            for frame in latest.drain().map(|(_, frame)| frame) {
                                for (out, sample) in mixed.iter_mut().zip(frame) {
                                    *out += sample / count;
                                }
                            }
                            for sample in &mut mixed {
                                *sample = sample.clamp(-1.0, 1.0);
                            }
                            let _ = playback_sender.try_send(mixed);
                        }
                    }
                }
            }
        });

        let mut session = Self {
            user,
            outbound_tracks,
            muted: muted_flag,
            capture,
            playback,
            decoded_tx,
            active_routes,
            last_decoded,
            cipher,
            screen_channels,
            screen_tx,
            screen_rx,
            metrics_previous: HashMap::new(),
            signaling_tx,
            signaling_rx,
            status_tx,
            status_rx,
            peers: HashMap::new(),
        };
        let user_for_offer = session.user.clone();
        let offer = session
            .create_transport("uplink", &user_for_offer, true)
            .await?;
        Ok((session, vec![offer]))
    }

    pub async fn create_transport(
        &mut self,
        transport: &str,
        publisher: &str,
        send: bool,
    ) -> Result<Value, String> {
        let pc = self
            .new_peer(CandidateTarget::Sfu(transport.to_owned()), send)
            .await?;
        let offer = pc
            .create_offer(None)
            .await
            .map_err(|error| format!("could not create media offer: {error}"))?;
        pc.set_local_description(offer)
            .await
            .map_err(|error| format!("could not set local media offer: {error}"))?;
        let description = pc
            .local_description()
            .await
            .ok_or_else(|| "WebRTC did not produce a local description".to_owned())?;
        self.peers.insert(transport.to_owned(), pc);
        Ok(
            json!({"type":"media_offer","transport":transport,"publisher":publisher,"sdp":description}),
        )
    }

    async fn new_peer(
        &self,
        target: CandidateTarget,
        send: bool,
    ) -> Result<Arc<dyn PeerConnection>, String> {
        let track_key = match &target {
            CandidateTarget::Sfu(transport) => transport.clone(),
            CandidateTarget::Peer(remote) => format!("peer:{remote}"),
        };
        let mut media_engine = MediaEngine::default();
        media_engine
            .register_default_codecs()
            .map_err(|error| format!("could not register WebRTC codecs: {error}"))?;
        let registry =
            register_default_interceptors(rtc::interceptor::Registry::new(), &mut media_engine)
                .map_err(|error| format!("interceptor setup failed: {error}"))?;
        let mut configuration = RTCConfigurationBuilder::new();
        if let Ok(url) = std::env::var("PULSE_ICE_URL") {
            let server = RTCIceServer {
                urls: vec![url],
                username: std::env::var("PULSE_ICE_USERNAME").unwrap_or_default(),
                credential: std::env::var("PULSE_ICE_CREDENTIAL").unwrap_or_default(),
            };
            server
                .urls()
                .map_err(|error| format!("invalid ICE server configuration: {error}"))?;
            configuration = configuration.with_ice_servers(vec![server]);
        }
        let pc = PeerConnectionBuilder::new()
            .with_configuration(configuration.build())
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .with_handler(Arc::new(Handler {
                decoded: self.decoded_tx.clone(),
                signaling: self.signaling_tx.clone(),
                status: self.status_tx.clone(),
                last_decoded: self.last_decoded.clone(),
                cipher: self.cipher.clone(),
                screen_channels: self.screen_channels.clone(),
                screen_tx: self.screen_tx.clone(),
                target,
            }))
            .with_data_channel_send_buffer_limit(128 * 1024)
            .with_udp_addrs(vec!["0.0.0.0:0"])
            .build()
            .await
            .map_err(|error| format!("WebRTC setup failed: {error}"))?;
        let pc: Arc<dyn PeerConnection> = Arc::new(pc);

        if send {
            let track = opus_track()?;
            pc.add_track(track.clone() as Arc<dyn TrackLocal>)
                .await
                .map_err(|error| format!("could not add Opus track: {error}"))?;
            self.outbound_tracks
                .lock()
                .unwrap()
                .insert(track_key, track);
        } else {
            pc.add_transceiver_from_kind(RtpCodecKind::Audio, None)
                .await
                .map_err(|error| format!("could not add receive transceiver: {error}"))?;
        }
        Ok(pc)
    }

    pub async fn create_peer_offer(&mut self, remote: &str) -> Result<Value, String> {
        let pc = self
            .new_peer(CandidateTarget::Peer(remote.to_owned()), true)
            .await?;
        let channel = pc
            .create_data_channel("screen", None)
            .await
            .map_err(|error| format!("could not create screen channel: {error}"))?;
        register_screen_channel(
            remote.to_owned(),
            channel,
            self.screen_channels.clone(),
            self.screen_tx.clone(),
            self.cipher.clone(),
        );
        let offer = pc
            .create_offer(None)
            .await
            .map_err(|error| format!("could not create peer offer: {error}"))?;
        pc.set_local_description(offer)
            .await
            .map_err(|error| format!("could not set local peer offer: {error}"))?;
        let description = pc
            .local_description()
            .await
            .ok_or_else(|| "WebRTC did not produce a peer offer".to_owned())?;
        self.peers.insert(format!("peer:{remote}"), pc);
        Ok(json!({"type":"signal","to":remote,"description":description}))
    }

    pub async fn handle_peer_signal(
        &mut self,
        remote: &str,
        message: &Value,
    ) -> Result<Option<Value>, String> {
        let key = format!("peer:{remote}");
        if let Some(description) = message.get("description") {
            let description: RTCSessionDescription = serde_json::from_value(description.clone())
                .map_err(|error| format!("invalid peer description: {error}"))?;
            if description.sdp_type == rtc::peer_connection::sdp::RTCSdpType::Offer {
                let pc = self
                    .new_peer(CandidateTarget::Peer(remote.to_owned()), true)
                    .await?;
                pc.set_remote_description(description)
                    .await
                    .map_err(|error| format!("could not apply peer offer: {error}"))?;
                let answer = pc
                    .create_answer(None)
                    .await
                    .map_err(|error| format!("could not create peer answer: {error}"))?;
                pc.set_local_description(answer)
                    .await
                    .map_err(|error| format!("could not set local peer answer: {error}"))?;
                let answer = pc
                    .local_description()
                    .await
                    .ok_or_else(|| "WebRTC did not produce a peer answer".to_owned())?;
                self.peers.insert(key, pc);
                return Ok(Some(
                    json!({"type":"signal","to":remote,"description":answer}),
                ));
            }
            let pc = self
                .peers
                .get(&key)
                .ok_or_else(|| format!("unknown peer: {remote}"))?;
            pc.set_remote_description(description)
                .await
                .map_err(|error| format!("could not apply peer answer: {error}"))?;
        } else if let Some(candidate) = message.get("candidate") {
            let candidate: RTCIceCandidateInit = serde_json::from_value(candidate.clone())
                .map_err(|error| format!("invalid peer candidate: {error}"))?;
            let pc = self
                .peers
                .get(&key)
                .ok_or_else(|| format!("unknown peer: {remote}"))?;
            pc.add_ice_candidate(candidate)
                .await
                .map_err(|error| format!("could not apply peer candidate: {error}"))?;
        }
        Ok(None)
    }

    pub fn take_signaling(&mut self) -> Vec<Value> {
        let mut messages = Vec::new();
        while let Ok(message) = self.signaling_rx.try_recv() {
            messages.push(message);
        }
        messages
    }

    pub fn take_status(&mut self) -> Vec<AudioStatus> {
        let mut events = Vec::new();
        while let Ok(event) = self.status_rx.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn remove_peer(&mut self, remote: &str) {
        self.peers.remove(&format!("peer:{remote}"));
        self.peers.remove(remote);
        self.outbound_tracks
            .lock()
            .unwrap()
            .remove(&format!("peer:{remote}"));
        self.active_routes.lock().unwrap().remove(remote);
        self.screen_channels.lock().unwrap().remove(remote);
        self.last_decoded
            .lock()
            .unwrap()
            .retain(|(user, _), _| user != remote);
    }

    pub fn select_route(&self, user: &str, route: Route) {
        self.active_routes
            .lock()
            .unwrap()
            .insert(user.to_owned(), route);
    }

    pub async fn send_screen_frame(&self, jpeg: &[u8]) {
        if jpeg.len() > 48 * 1024 {
            return;
        }
        let Ok(packet) = self.cipher.seal(&self.user, jpeg) else {
            return;
        };
        let channels = self
            .screen_channels
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for channel in channels {
            let _ = channel.try_send(BytesMut::from(packet.as_slice())).await;
        }
    }

    pub fn take_screen_frame(&mut self) -> Option<(String, Vec<u8>)> {
        let mut latest = None;
        while let Ok(frame) = self.screen_rx.try_recv() {
            latest = Some(frame);
        }
        latest
    }

    pub fn has_decoded_since(&self, user: &str, route: Route, since: Instant) -> bool {
        self.last_decoded
            .lock()
            .unwrap()
            .get(&(user.to_owned(), route))
            .is_some_and(|time| *time > since)
    }

    pub async fn sample_metrics(&mut self) -> Vec<(Value, Option<f64>, f64, f64)> {
        let peers = self
            .peers
            .iter()
            .filter(|(key, _)| key.as_str() != "uplink")
            .map(|(key, pc)| (key.clone(), pc.clone()))
            .collect::<Vec<_>>();
        let mut messages = Vec::new();
        for (key, pc) in peers {
            let report = pc.get_stats(Instant::now(), StatsSelector::None).await;
            let mut inbound = None;
            let mut transport = None;
            for entry in report.iter() {
                match entry {
                    RTCStatsReportEntry::InboundRtp(stats) => {
                        let received = &stats.received_rtp_stream_stats;
                        inbound = Some((
                            received.packets_received,
                            received.packets_lost,
                            received.jitter,
                        ));
                    }
                    RTCStatsReportEntry::Transport(stats) => {
                        transport = Some((
                            stats.ice_state,
                            stats.dtls_state,
                            stats.selected_candidate_pair_id.clone(),
                        ))
                    }
                    _ => {}
                }
            }
            let Some((ice, dtls, selected_pair)) = transport else {
                continue;
            };
            let rtt = match report.get(&selected_pair) {
                Some(RTCStatsReportEntry::IceCandidatePair(stats))
                    if stats.current_round_trip_time.is_finite()
                        && stats.current_round_trip_time > 0.0 =>
                {
                    Some(stats.current_round_trip_time * 1000.0)
                }
                _ => None,
            };
            let (packets_total, lost_total, jitter) = inbound.unwrap_or((0, 0, 0.0));
            if !jitter.is_finite() {
                continue;
            }
            let now = Instant::now();
            let previous = self.metrics_previous.entry(key.clone()).or_insert((
                packets_total,
                lost_total,
                0,
                now,
            ));
            let window_ms = now.duration_since(previous.3).as_millis().clamp(100, 60000) as u64;
            let packets = packets_total.saturating_sub(previous.0);
            let lost = lost_total.saturating_sub(previous.1).max(0) as u64;
            previous.0 = packets_total;
            previous.1 = lost_total;
            previous.2 += 1;
            previous.3 = now;
            let loss = if packets + lost == 0 {
                0.0
            } else {
                lost as f64 / (packets + lost) as f64
            };
            let (publisher, route) = if let Some(user) = key.strip_prefix("peer:") {
                (user, "p2p")
            } else {
                (key.as_str(), "sfu")
            };
            let ice = match ice {
                rtc::peer_connection::transport::RTCIceTransportState::Connected
                | rtc::peer_connection::transport::RTCIceTransportState::Completed => "connected",
                rtc::peer_connection::transport::RTCIceTransportState::Failed => "failed",
                _ => "disconnected",
            };
            let dtls = match dtls {
                rtc::peer_connection::transport::RTCDtlsTransportState::Connected => "connected",
                rtc::peer_connection::transport::RTCDtlsTransportState::Failed => "failed",
                _ => "connecting",
            };
            let jitter_ms = (jitter * 1000.0).clamp(0.0, 60000.0);
            let message = json!({"type":"route_metrics","publisher":publisher,"route":route,"metrics":{
                "sequence":previous.2,"window_ms":window_ms,"rtt_ms":rtt,"jitter_ms":jitter_ms,
                "loss":loss,"packets":packets,"ice":ice,"dtls":dtls
            }});
            messages.push((message, rtt, jitter_ms, loss));
        }
        messages
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub async fn handle(&mut self, message: &Value) -> Result<Option<Value>, String> {
        let transport = message
            .get("transport")
            .and_then(Value::as_str)
            .ok_or_else(|| "media message has no transport".to_owned())?;
        let pc = self
            .peers
            .get(transport)
            .ok_or_else(|| format!("unknown SFU transport: {transport}"))?;
        match message.get("type").and_then(Value::as_str) {
            Some("media_answer") => {
                let sdp: RTCSessionDescription = serde_json::from_value(
                    message
                        .get("sdp")
                        .cloned()
                        .ok_or_else(|| "media answer has no sdp".to_owned())?,
                )
                .map_err(|error| format!("invalid media answer: {error}"))?;
                pc.set_remote_description(sdp)
                    .await
                    .map_err(|error| format!("could not apply media answer: {error}"))?;
            }
            Some("media_candidate") => {
                let candidate: RTCIceCandidateInit = serde_json::from_value(
                    message
                        .get("candidate")
                        .cloned()
                        .ok_or_else(|| "media candidate has no candidate".to_owned())?,
                )
                .map_err(|error| format!("invalid media candidate: {error}"))?;
                pc.add_ice_candidate(candidate)
                    .await
                    .map_err(|error| format!("could not apply media candidate: {error}"))?;
            }
            _ => {}
        }
        Ok(None)
    }
}

fn register_screen_channel(
    user: String,
    channel: Arc<dyn DataChannel>,
    channels: Arc<Mutex<HashMap<String, Arc<dyn DataChannel>>>>,
    received: mpsc::Sender<(String, Vec<u8>)>,
    cipher: Arc<Cipher>,
) {
    channels
        .lock()
        .unwrap()
        .insert(user.clone(), channel.clone());
    tokio::spawn(async move {
        while let Some(event) = channel.poll().await {
            if let DataChannelEvent::OnMessage(message) = event {
                if message.data.len() <= 64 * 1024 {
                    if let Ok(jpeg) = cipher.open(&user, &message.data) {
                        let _ = received.try_send((user.clone(), jpeg));
                    }
                }
            }
        }
    });
}

impl Drop for SfuSession {
    fn drop(&mut self) {
        self.peers.clear();
        self.outbound_tracks.lock().unwrap().clear();
        let _ = &self.capture;
        let _ = &self.playback;
        log::debug!("SFU session cleaned up");
    }
}

fn opus_track() -> Result<Arc<TrackLocalStaticSample>, String> {
    let codec = RTCRtpCodec {
        mime_type: "audio/opus".to_owned(),
        clock_rate: SAMPLE_RATE as u32,
        channels: 2,
        sdp_fmtp_line: "minptime=10;useinbandfec=1".to_owned(),
        rtcp_feedback: vec![],
    };
    let track = MediaStreamTrack::new(
        "pulse-audio".to_owned(),
        "pulse-opus".to_owned(),
        "pulse-opus".to_owned(),
        RtpCodecKind::Audio,
        vec![RTCRtpEncodingParameters {
            rtp_coding_parameters: RTCRtpCodingParameters {
                ssrc: Some(1),
                ..Default::default()
            },
            codec: codec.clone(),
            ..Default::default()
        }],
    );
    TrackLocalStaticSample::new(Instant::now(), track)
        .map(Arc::new)
        .map_err(|error| format!("could not create Opus track: {error}"))
}
