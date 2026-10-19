//! Minimal SFU WebRTC transport.
//!
//! The WebSocket remains the signaling channel; RTP/Opus never crosses it.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use opus::{Application, Channels, Decoder, Encoder};
use rtc::{
    media::Sample,
    media_stream::MediaStreamTrack,
    peer_connection::{
        configuration::{
            interceptor_registry::register_default_interceptors, media_engine::MediaEngine,
            RTCConfigurationBuilder,
        },
        sdp::RTCSessionDescription,
        transport::RTCIceCandidateInit,
    },
    rtp_transceiver::rtp_sender::{
        RTCRtpCodec, RTCRtpCodingParameters, RTCRtpEncodingParameters, RtpCodecKind,
    },
};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use webrtc::{
    media_stream::{
        track_local::{static_sample::TrackLocalStaticSample, TrackLocal},
        track_remote::{TrackRemote, TrackRemoteEvent},
    },
    peer_connection::{
        PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCPeerConnectionState,
    },
};

use super::audio;

const SAMPLE_RATE: usize = 48_000;
const FRAME_SAMPLES: usize = 960;

#[derive(Clone)]
struct Handler {
    decoded: mpsc::UnboundedSender<Vec<f32>>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for Handler {
    async fn on_track(&self, track: Arc<dyn TrackRemote>) {
        let decoded = self.decoded.clone();
        tokio::spawn(async move {
            let mut decoder = match Decoder::new(SAMPLE_RATE as u32, Channels::Mono) {
                Ok(decoder) => decoder,
                Err(error) => {
                    log::warn!("could not create Opus decoder: {error}");
                    return;
                }
            };
            while let Some(event) = track.poll().await {
                if let TrackRemoteEvent::OnRtpPacket(packet) = event {
                    let mut pcm = vec![0.0f32; FRAME_SAMPLES];
                    match decoder.decode_float(&packet.payload, &mut pcm, false) {
                        Ok(samples) => {
                            pcm.truncate(samples);
                            let _ = decoded.send(pcm);
                        }
                        Err(error) => log::debug!("Opus decode error: {error}"),
                    }
                }
            }
        });
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
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
    offer_track: Arc<TrackLocalStaticSample>,
    capture: audio::AudioCapture,
    playback: audio::AudioPlayback,
    peers: HashMap<String, Arc<dyn PeerConnection>>,
    decoded_tx: mpsc::UnboundedSender<Vec<f32>>,
}

impl SfuSession {
    pub async fn start(
        user: String,
        input: &str,
        output: &str,
    ) -> Result<(Self, Vec<Value>), String> {
        let (capture, frames) = audio::start_capture_with_frames(input)?;
        let (playback, playback_frames) = audio::start_playback_with_frames(output)?;
        let (decoded_tx, decoded_rx) = mpsc::unbounded_channel();
        let track = opus_track()?;
        let sender_track = track.clone();
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
                while let Ok(frame) = frames.recv() {
                    pending.extend(frame);
                    while pending.len() >= FRAME_SAMPLES {
                        let pcm: Vec<f32> = pending.drain(..FRAME_SAMPLES).collect();
                        let mut encoded = vec![0u8; 4000];
                        match encoder.encode_float(&pcm, &mut encoded) {
                            Ok(length) => {
                                encoded.truncate(length);
                                let sample = Sample {
                                    data: Bytes::from(encoded),
                                    duration: Duration::from_millis(20),
                                    ..Sample::new(Instant::now())
                                };
                                // A single Opus encoding is used for the single
                                // SSRC in this track.
                                let sender_track = sender_track.clone();
                                let runtime = tokio::runtime::Builder::new_current_thread()
                                    .enable_all()
                                    .build()
                                    .expect("Tokio runtime for Opus uplink");
                                runtime.block_on(async move {
                                    let _ = sender_track.write_sample(1, 111, &sample, &[]).await;
                                });
                            }
                            Err(error) => log::debug!("Opus encode error: {error}"),
                        }
                    }
                }
            })
            .map_err(|error| format!("could not start Opus encoder: {error}"))?;

        let playback_sender = playback_frames;
        tokio::spawn(async move {
            let mut decoded_rx = decoded_rx;
            while let Some(frame) = decoded_rx.recv().await {
                let _ = playback_sender.try_send(frame);
            }
        });

        let mut session = Self {
            user,
            offer_track: track,
            capture,
            playback,
            decoded_tx,
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
        let mut media_engine = MediaEngine::default();
        let registry =
            register_default_interceptors(rtc::interceptor::Registry::new(), &mut media_engine)
                .map_err(|error| format!("interceptor setup failed: {error}"))?;
        let pc = PeerConnectionBuilder::new()
            .with_configuration(RTCConfigurationBuilder::new().build())
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .with_handler(Arc::new(Handler {
                decoded: self.decoded_tx.clone(),
            }))
            .with_udp_addrs(vec!["0.0.0.0:0"])
            .build()
            .await
            .map_err(|error| format!("WebRTC setup failed: {error}"))?;
        let pc: Arc<dyn PeerConnection> = Arc::new(pc);

        if send {
            pc.add_track(self.offer_track.clone() as Arc<dyn TrackLocal>)
                .await
                .map_err(|error| format!("could not add Opus track: {error}"))?;
        } else {
            pc.add_transceiver_from_kind(RtpCodecKind::Audio, None)
                .await
                .map_err(|error| format!("could not add receive transceiver: {error}"))?;
        }
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

impl Drop for SfuSession {
    fn drop(&mut self) {
        self.peers.clear();
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
