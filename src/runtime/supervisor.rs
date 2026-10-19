//! Runtime supervisor and simulation/signaling backends.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use tokio::sync::{broadcast, mpsc};

use crate::{
    media::{audio, screen},
    runtime::{ClientCommand, ClientEvent, RuntimeHandle, RuntimeSnapshot},
    services::{self, signaling::ServerEvent, simulation},
    state::AppState,
};

pub fn spawn(initial_state: &AppState) -> RuntimeHandle {
    let (command_sender, command_receiver) = RuntimeHandle::command_channel();
    let event_sender = RuntimeHandle::event_channel();
    let handle = RuntimeHandle::new(command_sender, event_sender.clone());

    tokio::spawn(run(initial_state.clone(), command_receiver, event_sender));
    handle
}

async fn run(
    mut state: AppState,
    commands: mpsc::Receiver<ClientCommand>,
    events: broadcast::Sender<ClientEvent>,
) {
    initialize_media(&mut state);
    if let Some(config) = services::signaling::SignalingConfig::from_env() {
        run_signaling(state, commands, events, config).await;
    } else {
        run_simulation(state, commands, events).await;
    }
}

async fn run_simulation(
    mut state: AppState,
    mut commands: mpsc::Receiver<ClientCommand>,
    events: broadcast::Sender<ClientEvent>,
) {
    let mut capture = start_capture(&state);
    let mut _screen_capture = None;
    state.media.capture_active = capture.is_some();
    let _ = events.send(ClientEvent::SimulationMode);
    publish_snapshot(&state, &events);
    let mut ticker = tokio::time::interval(Duration::from_millis(200));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                simulation::sim_tick(&mut state);
                publish_snapshot(&state, &events);
            }
            command = commands.recv() => match command {
                Some(command) => {
                    let restart_capture = matches!(
                        &command,
                        ClientCommand::JoinVoiceChannel { .. }
                            | ClientCommand::LeaveVoiceChannel
                            | ClientCommand::SetMuted(_)
                            | ClientCommand::SetDeafened(_)
                            | ClientCommand::SetInputDevice(_)
                    );
                    if matches!(command, ClientCommand::SetAuthToken(_)) {
                        let _ = events.send(ClientEvent::ConnectionFailed(
                            "Configure PULSE_SIGNAL_URL e PULSE_ROOM para conectar ao backend."
                                .into(),
                        ));
                        continue;
                    }
                    handle_local_command(command, &mut state, &events);
                    if restart_capture {
                        capture = start_capture(&state);
                        state.media.capture_active = capture.is_some();
                        publish_snapshot(&state, &events);
                    }
                    if state.media.screen_capture_active {
                        _screen_capture = start_screen_capture(&state);
                    } else {
                        _screen_capture = None;
                    }
                },
                None => return,
            }
        }
    }
}

async fn run_signaling(
    mut state: AppState,
    mut commands: mpsc::Receiver<ClientCommand>,
    events: broadcast::Sender<ClientEvent>,
    mut config: services::signaling::SignalingConfig,
) {
    'reconnect: loop {
        if config.token.trim().is_empty() {
            let _ = events.send(ClientEvent::ConnectionFailed(
                "Selecione ou informe um token para conectar ao backend.".into(),
            ));
            loop {
                match commands.recv().await {
                    Some(ClientCommand::SetAuthToken(token)) => {
                        config.token = token;
                        continue 'reconnect;
                    }
                    Some(command) => handle_local_command(command, &mut state, &events),
                    None => return,
                }
            }
        }
        let mut _screen_capture = None;
        let attempt_config = config.clone();
        let connect = services::signaling::SignalingConnection::connect(&attempt_config);
        tokio::pin!(connect);
        let result = loop {
            tokio::select! {
                result = &mut connect => break result,
                command = commands.recv() => match command {
                    Some(ClientCommand::SetAuthToken(token)) => {
                        config.token = token;
                        continue 'reconnect;
                    }
                    Some(command) => handle_local_command(command, &mut state, &events),
                    None => return,
                }
            }
        };
        let (mut connection, joined) = match result {
            Ok(connection) => connection,
            Err(error) => {
                log::warn!("{error}; retrying in 5 seconds");
                state.joined_voice = false;
                state.participants.clear();
                state.media.voice_connected = false;
                state.media.remote_audio_received = false;
                state.media.capture_active = false;
                publish_snapshot(&state, &events);
                let _ = events.send(ClientEvent::ConnectionFailed(error));
                let retry = tokio::time::sleep(Duration::from_secs(5));
                tokio::pin!(retry);
                loop {
                    tokio::select! {
                        _ = &mut retry => continue 'reconnect,
                        command = commands.recv() => match command {
                            Some(ClientCommand::SetAuthToken(token)) => {
                                config.token = token;
                                continue 'reconnect;
                            }
                            Some(command) => handle_local_command(command, &mut state, &events),
                            None => return,
                        }
                    }
                }
            }
        };

        let user = match &joined {
            ServerEvent::Joined { user, .. } => user.clone(),
            _ => unreachable!("connect returns a joined event"),
        };

        let mut sfu = match state
            .media
            .selected_input
            .as_deref()
            .zip(state.media.selected_output.as_deref())
        {
            Some((input, output)) => {
                match crate::media::sfu::SfuSession::start(
                    user.clone(),
                    input,
                    output,
                    state.local_user.is_muted || state.local_user.is_deafened,
                )
                .await
                {
                    Ok((session, offers)) => {
                        state.media.capture_active = true;
                        for offer in offers {
                            if let Err(error) = connection.send(offer).await {
                                log::warn!("could not send SFU offer: {error}");
                            }
                        }
                        Some(session)
                    }
                    Err(error) => {
                        log::warn!("SFU audio unavailable: {error}");
                        state.media.capture_active = false;
                        state.media.last_error = Some(error.clone());
                        let _ = events.send(ClientEvent::MediaError(error));
                        None
                    }
                }
            }
            None => {
                state.media.capture_active = false;
                None
            }
        };

        let _ = events.send(ClientEvent::Connected);
        state.backend_connected = true;
        apply_signaling_event(joined, &mut state, &events);
        if let Some(sfu) = sfu.as_mut() {
            let publishers = state
                .participants
                .iter()
                .map(|participant| participant.display_name.clone())
                .collect::<Vec<_>>();
            for publisher in publishers {
                match sfu.create_transport(&publisher, &publisher, false).await {
                    Ok(offer) => {
                        if let Err(error) = connection.send(offer).await {
                            log::warn!("could not send SFU downlink offer: {error}");
                        }
                    }
                    Err(error) => log::warn!("could not create SFU downlink: {error}"),
                }
                if should_offer(&user, &publisher) {
                    match sfu.create_peer_offer(&publisher).await {
                        Ok(offer) => {
                            if let Err(error) = connection.send(offer).await {
                                log::warn!("could not send peer offer: {error}");
                            }
                        }
                        Err(error) => log::warn!("could not create peer offer: {error}"),
                    }
                }
            }
        }
        let mut keepalive = tokio::time::interval(Duration::from_secs(30));
        let mut media_ticker = tokio::time::interval(Duration::from_millis(100));
        let mut metrics_ticker = tokio::time::interval(Duration::from_secs(1));
        let mut connected_peers = HashSet::new();
        let mut pending_routes =
            HashMap::<String, (u64, crate::media::sfu::Route, Instant, bool)>::new();

        loop {
            let disconnected = tokio::select! {
                _ = keepalive.tick() => {
                    connection.send(serde_json::json!({"type": "members"})).await.is_err()
                }
                _ = media_ticker.tick() => {
                    let mut failed = false;
                    if let Some(sfu) = sfu.as_mut() {
                        let mut status_changed = false;
                        for status in sfu.take_status() {
                            match status {
                                crate::media::sfu::AudioStatus::PeerConnection { user, connected } => {
                                    if connected { connected_peers.insert(user); } else { connected_peers.remove(&user); }
                                    let active = !connected_peers.is_empty();
                                    if state.media.voice_connected != active {
                                        state.media.voice_connected = active;
                                        status_changed = true;
                                    }
                                }
                                crate::media::sfu::AudioStatus::RemoteAudio => {
                                    if !state.media.remote_audio_received {
                                        state.media.remote_audio_received = true;
                                        status_changed = true;
                                    }
                                }
                            }
                        }
                        if status_changed { publish_snapshot(&state, &events); }
                        for signal in sfu.take_signaling() {
                            if let Err(error) = connection.send(signal).await {
                                log::warn!("could not send ICE candidate: {error}");
                                failed = true;
                                break;
                            }
                        }
                        for (publisher, (epoch, route, since, ready)) in &mut pending_routes {
                            if !*ready && sfu.has_decoded_since(publisher, *route, *since) {
                                if connection.send(serde_json::json!({"type":"route_ready","publisher":publisher,"epoch":epoch,"route":route_name(*route)})).await.is_err() {
                                    failed = true;
                                    break;
                                }
                                *ready = true;
                            }
                        }
                    }
                    failed
                }
                _ = metrics_ticker.tick() => {
                    let mut failed = false;
                    if let Some(sfu) = sfu.as_mut() {
                        for (message, rtt, jitter, loss) in sfu.sample_metrics().await {
                            if message["route"] == "p2p" {
                                state.metrics.latency_ms = rtt.unwrap_or(0.0).round() as u32;
                                state.metrics.jitter_ms = jitter.round() as u32;
                                state.metrics.packet_loss_pct = (loss * 100.0) as f32;
                            }
                            if connection.send(message).await.is_err() { failed = true; break; }
                        }
                        publish_snapshot(&state, &events);
                    }
                    failed
                }
                command = commands.recv() => match command {
                    Some(command) => {
                        if let ClientCommand::SetAuthToken(token) = command {
                            config.token = token;
                            state.backend_connected = false;
                            state.joined_voice = false;
                            state.participants.clear();
                            state.media.voice_connected = false;
                            state.media.remote_audio_received = false;
                            state.media.capture_active = false;
                            publish_snapshot(&state, &events);
                            break;
                        }
                        if let ClientCommand::SendChat { content } = &command {
                            let content = content.trim();
                            if content.len() > 2000 {
                                let _ = events.send(ClientEvent::ChatError("Mensagem limitada a 2.000 bytes.".into()));
                                false
                            } else if content.is_empty() {
                                false
                            } else {
                                connection.send(serde_json::json!({"type":"chat","content":content})).await.is_err()
                            }
                        } else {
                        let restart_capture = matches!(
                            &command,
                            ClientCommand::JoinVoiceChannel { .. }
                                | ClientCommand::LeaveVoiceChannel
                                | ClientCommand::SetMuted(_)
                                | ClientCommand::SetDeafened(_)
                                | ClientCommand::SetInputDevice(_)
                        );
                        handle_local_command(command, &mut state, &events);
                        if let Some(sfu) = sfu.as_ref() {
                            sfu.set_muted(!state.joined_voice || state.local_user.is_muted || state.local_user.is_deafened);
                        }
                        if restart_capture { publish_snapshot(&state, &events); }
                        if state.media.screen_capture_active {
                            _screen_capture = start_screen_capture(&state);
                        } else {
                            _screen_capture = None;
                        }
                        false
                        }
                    }
                    None => return,
                },
                incoming = connection.next_event() => match incoming {
                    Ok(Some(event)) => {
                        if let ServerEvent::PeerJoined { user } = &event {
                            if let Some(sfu) = sfu.as_mut() {
                                match sfu.create_transport(user, user, false).await {
                                    Ok(offer) => {
                                        if let Err(error) = connection.send(offer).await {
                                            log::warn!("could not send SFU downlink offer: {error}");
                                        }
                                    }
                                    Err(error) => {
                                        log::warn!("could not create SFU downlink: {error}")
                                    }
                                }
                                if should_offer(&state.local_user.display_name, user) {
                                    match sfu.create_peer_offer(user).await {
                                        Ok(offer) => {
                                            if let Err(error) = connection.send(offer).await {
                                                log::warn!("could not send peer offer: {error}");
                                            }
                                        }
                                        Err(error) => log::warn!("could not create peer offer: {error}"),
                                    }
                                }
                            }
                        }
                        if let ServerEvent::PeerLeft { user } = &event {
                            pending_routes.remove(user);
                            connected_peers.remove(user);
                            state.media.voice_connected = !connected_peers.is_empty();
                            if !state.media.voice_connected { state.media.remote_audio_received = false; }
                            if let Some(sfu) = sfu.as_mut() {
                                sfu.remove_peer(user);
                            }
                        }
                        if let ServerEvent::Signal(value) = &event {
                            if let Some(sfu) = sfu.as_mut() {
                                match value.get("type").and_then(serde_json::Value::as_str) {
                                    Some("route_prepare") => {
                                        if let (Some(user), Some(epoch), Some(route)) = (
                                            value.get("publisher").and_then(serde_json::Value::as_str),
                                            value.get("epoch").and_then(serde_json::Value::as_u64),
                                            value.get("route").and_then(serde_json::Value::as_str).and_then(parse_route),
                                        ) {
                                            pending_routes.insert(user.to_owned(), (epoch, route, Instant::now(), false));
                                        }
                                    }
                                    Some("route_commit") => {
                                        if let (Some(user), Some(epoch), Some(route)) = (
                                            value.get("publisher").and_then(serde_json::Value::as_str),
                                            value.get("epoch").and_then(serde_json::Value::as_u64),
                                            value.get("route").and_then(serde_json::Value::as_str).and_then(parse_route),
                                        ) {
                                            if pending_routes.get(user).is_some_and(|(e, r, _, ready)| *e == epoch && *r == route && *ready) {
                                                sfu.select_route(user, route);
                                                pending_routes.remove(user);
                                                if connection.send(serde_json::json!({"type":"route_ack","publisher":user,"epoch":epoch})).await.is_err() {
                                                    log::warn!("could not acknowledge route commit");
                                                }
                                            }
                                        }
                                    }
                                    Some("route_rollback") => {
                                        if let Some(user) = value.get("publisher").and_then(serde_json::Value::as_str) {
                                            pending_routes.remove(user);
                                            if let Some(route) = value.get("route").and_then(serde_json::Value::as_str).and_then(parse_route) {
                                                sfu.select_route(user, route);
                                            } else {
                                                sfu.select_route(user, crate::media::sfu::Route::None);
                                            }
                                        }
                                    }
                                    Some("media_answer" | "media_candidate") => {
                                        if let Err(error) = sfu.handle(value).await {
                                            log::warn!("SFU signaling failed: {error}");
                                        }
                                    }
                                    Some("signal") => {
                                        if let Some(from) = value.get("from").and_then(serde_json::Value::as_str) {
                                            match sfu.handle_peer_signal(from, value).await {
                                                Ok(Some(answer)) => {
                                                    if let Err(error) = connection.send(answer).await {
                                                        log::warn!("could not send peer answer: {error}");
                                                    }
                                                }
                                                Ok(None) => {}
                                                Err(error) => log::warn!("peer signaling failed: {error}"),
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        apply_signaling_event(event, &mut state, &events);
                        false
                    }
                    Ok(None) => true,
                    Err(error) => {
                        log::warn!("{error}");
                        true
                    }
                }
            };

            if disconnected {
                log::warn!("signaling disconnected; retrying in 5 seconds");
                state.joined_voice = false;
                state.backend_connected = false;
                state.participants.clear();
                state.media.voice_connected = false;
                state.media.remote_audio_received = false;
                state.media.capture_active = false;
                publish_snapshot(&state, &events);
                let _ = events.send(ClientEvent::ConnectionFailed(
                    "Conexão com o backend encerrada; tentando novamente.".into(),
                ));
                let retry = tokio::time::sleep(Duration::from_secs(5));
                tokio::pin!(retry);
                loop {
                    tokio::select! {
                        _ = &mut retry => break,
                        command = commands.recv() => match command {
                            Some(ClientCommand::SetAuthToken(token)) => {
                                config.token = token;
                                continue 'reconnect;
                            }
                            Some(command) => handle_local_command(command, &mut state, &events),
                            None => return,
                        }
                    }
                }
                break;
            }
        }
    }
}

fn handle_local_command(
    command: ClientCommand,
    state: &mut AppState,
    events: &broadcast::Sender<ClientEvent>,
) {
    match command {
        ClientCommand::JoinVoiceChannel { channel_id } => {
            state.joined_voice = true;
            let _ = events.send(ClientEvent::VoiceJoined { channel_id });
        }

        ClientCommand::LeaveVoiceChannel => {
            state.joined_voice = false;
            let _ = events.send(ClientEvent::VoiceLeft);
        }

        ClientCommand::SetMuted(value) => {
            state.local_user.is_muted = value;
            if !value {
                state.local_user.is_deafened = false;
            }
        }
        ClientCommand::SetDeafened(value) => {
            state.local_user.is_deafened = value;
            if value {
                state.local_user.is_muted = true;
            }
        }
        ClientCommand::SetScreenSharing(value) => {
            state.local_user.is_screen_sharing = value;
            state.media.screen_capture_active = value;
            if value {
                if let Err(error) = screen::capture_screen(state.media.selected_screen).map(|_| ())
                {
                    state.media.last_error = Some(error.clone());
                    let _ = events.send(ClientEvent::MediaError(error));
                }
            }
        }
        ClientCommand::SetInputDevice(name) => {
            state.media.selected_input = Some(name);
        }
        ClientCommand::SetOutputDevice(name) => match audio::ensure_output_device(&name) {
            Ok(()) => state.media.selected_output = Some(name),
            Err(error) => {
                state.media.last_error = Some(error.clone());
                let _ = events.send(ClientEvent::MediaError(error));
            }
        },
        ClientCommand::SetScreenDevice(index) => {
            state.media.selected_screen = index;
        }
        ClientCommand::RefreshMediaDevices => {
            initialize_media(state);
        }
        ClientCommand::SendChat { content } => {
            let content = content.trim();
            if !content.is_empty() {
                state.chat_messages.push_back(crate::state::ChatMessage {
                    author: state.local_user.display_name.clone(),
                    content: content.to_owned(),
                    timestamp: chrono::Local::now().format("%H:%M").to_string(),
                    is_self: true,
                });
                if state.chat_messages.len() > 200 {
                    state.chat_messages.pop_front();
                }
            }
        }
        ClientCommand::SetAuthToken(_) => {}
    }
    publish_snapshot(state, events);
}

fn apply_signaling_event(
    event: ServerEvent,
    state: &mut AppState,
    events: &broadcast::Sender<ClientEvent>,
) {
    match event {
        ServerEvent::Authenticated { user } => state.local_user.display_name = user,
        ServerEvent::Joined { user, members } => {
            state.local_user.display_name = user;
            state.joined_voice = true;
            replace_participants(state, members);
        }
        ServerEvent::Members { members } => replace_participants(state, members),
        ServerEvent::PeerJoined { user } => add_participant(state, user),
        ServerEvent::PeerLeft { user } => {
            state
                .participants
                .retain(|participant| participant.display_name != user);
        }
        ServerEvent::Chat { from, content } => {
            state.chat_messages.push_back(crate::state::ChatMessage {
                is_self: from == state.local_user.display_name,
                author: from,
                content,
                timestamp: chrono::Local::now().format("%H:%M").to_string(),
            });
            if state.chat_messages.len() > 200 {
                state.chat_messages.pop_front();
            }
        }
        ServerEvent::Signal(value) => {
            log::debug!(
                "received signaling event: {}",
                value
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
            );
        }
        ServerEvent::Error { reason } => log::warn!("signaling server error: {reason}"),
    }
    publish_snapshot(state, events);
}

fn replace_participants(state: &mut AppState, members: Vec<String>) {
    state.participants.clear();
    for member in members {
        if member != state.local_user.display_name {
            add_participant(state, member);
        }
    }
}

fn should_offer(local: &str, remote: &str) -> bool {
    local < remote
}

fn parse_route(route: &str) -> Option<crate::media::sfu::Route> {
    match route {
        "p2p" => Some(crate::media::sfu::Route::P2p),
        "sfu" => Some(crate::media::sfu::Route::Sfu),
        _ => None,
    }
}

fn route_name(route: crate::media::sfu::Route) -> &'static str {
    match route {
        crate::media::sfu::Route::P2p => "p2p",
        crate::media::sfu::Route::Sfu => "sfu",
        crate::media::sfu::Route::None => "none",
    }
}

fn add_participant(state: &mut AppState, display_name: String) {
    if state
        .participants
        .iter()
        .any(|participant| participant.display_name == display_name)
    {
        return;
    }
    state.participants.push(crate::state::Participant {
        id: state.participants.len() as u32 + 2,
        display_name,
        avatar_color: (0, 217, 245),
        is_muted: false,
        is_deafened: false,
        is_speaking: false,
        speaking_level: 0.0,
        is_screen_sharing: false,
        is_camera_on: false,
        ping_ms: 0,
    });
}

fn publish_snapshot(state: &AppState, events: &broadcast::Sender<ClientEvent>) {
    let snapshot = RuntimeSnapshot {
        backend_connected: state.backend_connected,
        tick: state.tick,
        joined_voice: state.joined_voice,
        local_user: state.local_user.clone(),
        participants: state.participants.clone(),
        metrics: state.metrics.clone(),
        chat_messages: state.chat_messages.iter().cloned().collect(),
        media: state.media.clone(),
    };
    let _ = events.send(ClientEvent::Snapshot(snapshot));
}

fn start_capture(state: &AppState) -> Option<crate::media::audio::AudioCapture> {
    if !state.joined_voice || state.local_user.is_muted || state.local_user.is_deafened {
        return None;
    }
    let name = state.media.selected_input.as_deref()?;
    match audio::start_capture(name) {
        Ok(capture) => {
            log::info!("audio capture started: {name}");
            Some(capture)
        }

        Err(error) => {
            log::warn!("{error}");
            None
        }
    }
}

fn start_screen_capture(state: &AppState) -> Option<crate::media::screen::ScreenCapture> {
    match screen::start_capture(state.media.selected_screen) {
        Ok(capture) => Some(capture),
        Err(error) => {
            log::warn!("{error}");
            None
        }
    }
}

fn initialize_media(state: &mut AppState) {
    match audio::list_devices() {
        Ok(devices) => {
            state.media.input_devices = devices
                .iter()
                .filter(|device| device.is_input)
                .map(|device| device.name.clone())
                .collect();
            state.media.output_devices = devices
                .iter()
                .filter(|device| device.is_output)
                .map(|device| device.name.clone())
                .collect();
            state.media.selected_input = audio::default_input_name()
                .ok()
                .filter(|name| state.media.input_devices.contains(name));
            state.media.selected_output = audio::default_output_name()
                .ok()
                .filter(|name| state.media.output_devices.contains(name));
        }
        Err(error) => state.media.last_error = Some(error),
    }

    match screen::list_screens() {
        Ok(screens) => {
            state.media.screens = screens.into_iter().map(|screen| screen.name).collect();
            state.media.selected_screen = state
                .media
                .selected_screen
                .min(state.media.screens.len().saturating_sub(1));
        }
        Err(error) => state.media.last_error = Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    #[test]
    fn only_one_peer_starts_the_offer() {
        assert!(should_offer("alice", "bob"));
        assert!(!should_offer("bob", "alice"));
        assert!(!should_offer("alice", "alice"));
    }

    #[tokio::test]
    async fn changing_token_retries_immediately_after_authentication_failure() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (socket, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(socket).await.unwrap();
                let auth = socket.next().await.unwrap().unwrap();
                let auth: serde_json::Value =
                    serde_json::from_str(auth.to_text().unwrap()).unwrap();
                if auth["token"] == "bad" {
                    socket
                        .send(Message::text(
                            r#"{"type":"error","reason":"invalid token"}"#,
                        ))
                        .await
                        .unwrap();
                } else {
                    assert_eq!(auth["token"], "good");
                    socket
                        .send(Message::text(r#"{"type":"authenticated","user":"alice"}"#))
                        .await
                        .unwrap();
                    let join = socket.next().await.unwrap().unwrap();
                    assert_eq!(
                        serde_json::from_str::<serde_json::Value>(join.to_text().unwrap()).unwrap()
                            ["room"],
                        "lab"
                    );
                    socket
                        .send(Message::text(
                            r#"{"type":"joined","user":"alice","members":[]}"#,
                        ))
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        });

        let (commands, receiver) = mpsc::channel(8);
        let (events, mut listener) = broadcast::channel(8);
        let mut state = AppState::default();
        state.joined_voice = false;
        let client = tokio::spawn(run_signaling(
            state,
            receiver,
            events,
            services::signaling::SignalingConfig {
                url: format!("ws://{address}"),
                token: "bad".into(),
                room: "lab".into(),
            },
        ));

        tokio::time::timeout(Duration::from_secs(2), async {
            while !matches!(
                listener.recv().await.unwrap(),
                ClientEvent::ConnectionFailed(_)
            ) {}
        })
        .await
        .unwrap();
        commands
            .send(ClientCommand::SetAuthToken("good".into()))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while !matches!(listener.recv().await.unwrap(), ClientEvent::Connected) {}
        })
        .await
        .unwrap();
        client.abort();
        server.abort();
    }
}
