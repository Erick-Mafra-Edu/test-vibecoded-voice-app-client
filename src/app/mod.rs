//! App module — Elm-style update / view / subscription wiring.

pub mod messages;

use iced::{Element, Subscription, Task, Theme};
use tokio_stream::{Stream, StreamExt};

use crate::{
    runtime::{self, ClientCommand, ClientEvent},
    screens::{home, overlay, screen_share, settings},
    state::{AppMode, AppState, ChatMessage},
    theme,
};

pub use messages::Message;

// ─────────────────────────────────────────────────────────────
//  App struct
// ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct PulseApp {
    pub state: AppState,
    runtime: runtime::RuntimeHandle,
}

impl PulseApp {
    /// `run_with` constructor — returns (Self, Task).
    pub fn new() -> (Self, Task<Message>) {
        let state = AppState::default();
        let runtime = runtime::spawn(&state);
        (Self { state, runtime }, Task::none())
    }

    // ─────────────────────────────────────────────────────────
    //  Update (Elm reducer)
    // ─────────────────────────────────────────────────────────

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            // ── Navigation ───────────────────────────────────
            Message::NavTabSelected(tab) => {
                self.state.nav_tab = tab;
            }
            Message::BackToHome => {
                self.state.nav_tab = crate::state::NavTab::Home;
            }
            Message::ServerSelected(idx) => {
                if idx < self.state.servers.len() {
                    self.state.selected_server = idx;
                }
            }

            // ── Channels ─────────────────────────────────────
            Message::VoiceChannelSelected(idx) => {
                if let Some(channel) = self.state.voice_channels.get(idx) {
                    self.state.active_voice_channel = idx;
                    self.runtime.send(ClientCommand::JoinVoiceChannel {
                        channel_id: channel.id,
                    });
                }
            }
            Message::TextChannelSelected(idx) => {
                self.state.active_text_channel = idx;
            }
            Message::JoinVoice => {
                if let Some(channel) = self
                    .state
                    .voice_channels
                    .get(self.state.active_voice_channel)
                {
                    self.runtime.send(ClientCommand::JoinVoiceChannel {
                        channel_id: channel.id,
                    });
                }
            }
            Message::LeaveVoice => {
                self.runtime.send(ClientCommand::LeaveVoiceChannel);
            }

            // ── Audio controls ───────────────────────────────
            Message::ToggleMute => {
                let muted = !self.state.local_user.is_muted;
                self.runtime.send(ClientCommand::SetMuted(muted));
            }
            Message::ToggleDeafen => {
                let deafened = !self.state.local_user.is_deafened;
                self.runtime.send(ClientCommand::SetDeafened(deafened));
            }
            Message::ToggleScreenShare => {
                let sharing = !self.state.local_user.is_screen_sharing;
                self.runtime.send(ClientCommand::SetScreenSharing(sharing));
                self.state.mode = if sharing {
                    AppMode::ScreenShare
                } else {
                    AppMode::Normal
                };
            }
            Message::InputDeviceSelected(name) => {
                self.runtime.send(ClientCommand::SetInputDevice(name));
            }
            Message::OutputDeviceSelected(name) => {
                self.runtime.send(ClientCommand::SetOutputDevice(name));
            }
            Message::ScreenDeviceSelected(index) => {
                self.runtime.send(ClientCommand::SetScreenDevice(index));
            }
            Message::RefreshMediaDevices => {
                self.runtime.send(ClientCommand::RefreshMediaDevices);
            }
            Message::AuthTokenSelected(token) => {
                self.state.selected_auth_token = Some(token.clone());
                self.state.auth_token_input.clear();
                self.state.auth_token_error = None;
                self.runtime.send(ClientCommand::SetAuthToken(token));
            }
            Message::AuthTokenInputChanged(input) => {
                self.state.auth_token_input = input;
                self.state.auth_token_error = None;
            }
            Message::ApplyAuthToken => {
                let token = self.state.auth_token_input.trim().to_owned();
                if token.is_empty() {
                    self.state.auth_token_error = Some("Informe um token antes de aplicar.".into());
                } else {
                    if !self.state.auth_tokens.contains(&token) {
                        self.state.auth_tokens.push(token.clone());
                    }
                    self.state.selected_auth_token = Some(token.clone());
                    self.state.auth_token_input.clear();
                    self.state.auth_token_error = None;
                    self.runtime.send(ClientCommand::SetAuthToken(token));
                }
            }
            Message::DockToggle => {
                self.state.dock_expanded = !self.state.dock_expanded;
            }

            // ── Chat ─────────────────────────────────────────
            Message::ChatInputChanged(s) => {
                self.state.chat_input = s;
            }
            Message::ChatSubmit => {
                if !self.state.chat_input.trim().is_empty() {
                    let ts = chrono::Local::now().format("%H:%M").to_string();
                    let msg = ChatMessage {
                        author: self.state.local_user.display_name.clone(),
                        content: self.state.chat_input.trim().to_string(),
                        timestamp: ts,
                        is_self: true,
                    };
                    self.runtime.send(ClientCommand::SendChat {
                        content: msg.content,
                    });
                    self.state.chat_input.clear();
                }
            }
            Message::ChatVisibilityToggle => {
                self.state.chat_visible = !self.state.chat_visible;
            }

            Message::RuntimeEvent(event) => {
                self.apply_runtime_event(event);
            }

            // ── App mode ─────────────────────────────────────
            Message::SetMode(mode) => {
                // Sync screen-share state
                match &mode {
                    AppMode::ScreenShare => {
                        self.state.local_user.is_screen_sharing = true;
                    }
                    AppMode::Normal => {
                        self.state.local_user.is_screen_sharing = false;
                    }
                    _ => {}
                }
                self.state.mode = mode;
            }
        }

        Task::none()
    }

    // ─────────────────────────────────────────────────────────
    //  View
    // ─────────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        match self.state.mode {
            AppMode::ScreenShare => screen_share::screen_share_screen(&self.state),
            AppMode::Overlay => overlay::overlay_screen(&self.state),
            AppMode::Normal if self.state.nav_tab == crate::state::NavTab::Settings => {
                settings::settings_screen(&self.state)
            }
            AppMode::Normal => home::home_screen(&self.state),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Subscription
    // ─────────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        iced::Subscription::run_with(self.runtime.clone(), Self::runtime_events)
    }

    fn apply_runtime_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::VoiceJoined { channel_id } => {
                if let Some(index) = self
                    .state
                    .voice_channels
                    .iter()
                    .position(|channel| channel.id == channel_id)
                {
                    self.state.active_voice_channel = index;
                }
            }
            ClientEvent::VoiceLeft => {
                self.state.joined_voice = false;
            }
            ClientEvent::Snapshot(snapshot) => {
                self.state.tick = snapshot.tick;
                self.state.joined_voice = snapshot.joined_voice;
                self.state.participants = snapshot.participants;
                self.state.metrics = snapshot.metrics;
                self.state.chat_messages = snapshot.chat_messages.into_iter().collect();
                self.state.media = snapshot.media;
            }
            ClientEvent::Connected => {}
            ClientEvent::MediaError(error) => {
                self.state.media.last_error = Some(error);
            }
        }
    }

    fn runtime_events(runtime: &runtime::RuntimeHandle) -> impl Stream<Item = Message> {
        runtime
            .events()
            .filter_map(|event| event.ok().map(Message::RuntimeEvent))
    }

    // ─────────────────────────────────────────────────────────
    //  Theme
    // ─────────────────────────────────────────────────────────

    pub fn theme(&self) -> Theme {
        theme::pulse_theme()
    }
}
