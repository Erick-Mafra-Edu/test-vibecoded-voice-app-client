//! Runtime supervisor and simulation/signaling backends.

use std::time::Duration;

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
    let _ = events.send(ClientEvent::Connected);
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
    loop {
        let mut capture = start_capture(&state);
        let mut _screen_capture = None;
        state.media.capture_active = capture.is_some();
        let (mut connection, joined) =
            match services::signaling::SignalingConnection::connect(&config).await {
                Ok(connection) => connection,
                Err(error) => {
                    log::warn!("{error}; retrying in 5 seconds");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
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
                match crate::media::sfu::SfuSession::start(user, input, output).await {
                    Ok((session, offers)) => {
                        for offer in offers {
                            if let Err(error) = connection.send(offer).await {
                                log::warn!("could not send SFU offer: {error}");
                            }
                        }
                        Some(session)
                    }
                    Err(error) => {
                        log::warn!("SFU audio unavailable: {error}");
                        None
                    }
                }
            }
            None => None,
        };

        let _ = events.send(ClientEvent::Connected);
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
            }
        }
        let mut keepalive = tokio::time::interval(Duration::from_secs(30));

        loop {
            let disconnected = tokio::select! {
                _ = keepalive.tick() => {
                    connection.send(serde_json::json!({"type": "members"})).await.is_err()
                }
                command = commands.recv() => match command {
                    Some(command) => {
                        if let ClientCommand::SetAuthToken(token) = command {
                            config.token = token;
                            break;
                        }
                        let restart_capture = matches!(
                            &command,
                            ClientCommand::JoinVoiceChannel { .. }
                                | ClientCommand::LeaveVoiceChannel
                                | ClientCommand::SetMuted(_)
                                | ClientCommand::SetDeafened(_)
                                | ClientCommand::SetInputDevice(_)
                        );
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
                        false
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
                            }
                        }
                        if let ServerEvent::Signal(value) = &event {
                            if let Some(sfu) = sfu.as_mut() {
                                if value.get("type").and_then(serde_json::Value::as_str)
                                    == Some("media_answer")
                                    || value.get("type").and_then(serde_json::Value::as_str)
                                        == Some("media_candidate")
                                {
                                    if let Err(error) = sfu.handle(value).await {
                                        log::warn!("SFU signaling failed: {error}");
                                    }
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
                tokio::time::sleep(Duration::from_secs(5)).await;
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
        tick: state.tick,
        joined_voice: state.joined_voice,
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
