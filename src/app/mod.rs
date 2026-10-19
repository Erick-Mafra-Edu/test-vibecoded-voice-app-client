//! App module — Elm-style update / view / subscription wiring.

pub mod messages;

use iced::{
    widget::{column, pick_list, text},
    Element, Length, Subscription, Task, Theme,
};
use tokio_stream::{Stream, StreamExt};

use crate::{
    components::modal,
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
    device_modal_open: bool,
}

impl PulseApp {
    /// `run_with` constructor — returns (Self, Task).
    pub fn new() -> (Self, Task<Message>) {
        let mut state = AppState::default();
        match crate::services::signaling::SignalingConfig::from_env() {
            None => {
                state.connection_notice =
                    Some("Modo offline: dados de demonstração, sem conexão ao backend.".into());
            }
            Some(config) => {
                state.use_backend_room(&config.room);
                if config.token.trim().is_empty() {
                    state.connection_notice =
                        Some("Offline: selecione um token para conectar.".into());
                } else {
                    state.connection_notice = Some("Conectando ao backend...".into());
                }
            }
        }
        let runtime = runtime::spawn(&state);
        (
            Self {
                state,
                runtime,
                device_modal_open: false,
            },
            Task::none(),
        )
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
            Message::OpenDeviceModal => {
                self.device_modal_open = true;
            }
            Message::CloseModal => self.device_modal_open = false,
            Message::AuthTokenSelected(index) => {
                if let Some(token) = self.state.auth_tokens.get(index).cloned() {
                    self.state.selected_auth_token = Some(token.clone());
                    self.state.auth_token_input.clear();
                    self.state.auth_token_error = None;
                    self.state.connection_notice = Some("Conectando ao backend...".into());
                    self.runtime.send(ClientCommand::SetAuthToken(token));
                }
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
                    self.state.connection_notice = Some("Conectando ao backend...".into());
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
        let screen = match self.state.mode {
            AppMode::ScreenShare => screen_share::screen_share_screen(&self.state),
            AppMode::Overlay => overlay::overlay_screen(&self.state),
            AppMode::Normal if self.state.nav_tab == crate::state::NavTab::Settings => {
                settings::settings_screen(&self.state)
            }
            AppMode::Normal => home::home_screen(&self.state),
        };

        let screen = if let Some(notice) = &self.state.connection_notice {
            column![
                iced::widget::container(text(notice).color(crate::theme::Palette::BG_BASE))
                    .width(Length::Fill)
                    .padding(8)
                    .style(|_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(crate::theme::Palette::AMBER)),
                        ..Default::default()
                    }),
                screen,
            ]
            .into()
        } else {
            screen
        };

        if !self.device_modal_open {
            return screen;
        }

        let devices = column![
            text("Microphone"),
            pick_list(
                self.state.media.input_devices.clone(),
                self.state.media.selected_input.clone(),
                Message::InputDeviceSelected,
            )
            .placeholder("Select microphone")
            .width(Length::Fill),
            text("Speakers"),
            pick_list(
                self.state.media.output_devices.clone(),
                self.state.media.selected_output.clone(),
                Message::OutputDeviceSelected,
            )
            .placeholder("Select speakers")
            .width(Length::Fill),
        ]
        .spacing(10);

        modal::modal(screen, "Audio devices", devices.into(), Message::CloseModal)
    }

    // ─────────────────────────────────────────────────────────
    //  Subscription
    // ─────────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        let runtime = iced::Subscription::run_with(self.runtime.clone(), Self::runtime_events);
        let escape = self.device_modal_open.then(|| {
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) => Some(Message::CloseModal),
                _ => None,
            })
        });
        Subscription::batch(std::iter::once(runtime).chain(escape))
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
                self.state.backend_connected = snapshot.backend_connected;
                if snapshot.backend_connected {
                    self.state.connection_notice = None;
                } else if self.state.connection_notice.is_none() {
                    self.state.connection_notice =
                        Some("Offline: sem conexão com o backend.".into());
                }
                self.state.tick = snapshot.tick;
                self.state.joined_voice = snapshot.joined_voice;
                self.state.local_user = snapshot.local_user;
                self.state.participants = snapshot.participants;
                if self.state.backend_room.is_some() {
                    self.state.voice_channels[0].user_count =
                        self.state.participants.len() + usize::from(self.state.joined_voice);
                }
                self.state.metrics = snapshot.metrics;
                self.state.chat_messages = snapshot.chat_messages.into_iter().collect();
                self.state.media = snapshot.media;
                if self.state.media.screen_frame.is_some() && self.state.mode == AppMode::Normal {
                    self.state.mode = AppMode::ScreenShare;
                } else if self.state.media.screen_frame.is_none()
                    && self.state.mode == AppMode::ScreenShare
                    && !self.state.local_user.is_screen_sharing
                {
                    self.state.mode = AppMode::Normal;
                }
            }
            ClientEvent::Connected => {
                self.state.auth_token_error = None;
                self.state.connection_notice = None;
            }
            ClientEvent::SimulationMode => {
                self.state.connection_notice =
                    Some("Modo offline: dados de demonstração, sem conexão ao backend.".into());
            }
            ClientEvent::ConnectionFailed(error) => {
                self.state.auth_token_error = Some(error);
                self.state.connection_notice = Some(
                    if self.state.backend_room.is_some() {
                        "Offline: sem conexão com o backend."
                    } else {
                        "Modo offline: dados de demonstração, sem conexão ao backend."
                    }
                    .into(),
                );
            }
            ClientEvent::MediaError(error) => {
                self.state.media.last_error = Some(error);
            }
            ClientEvent::ChatError(error) => {
                self.state.connection_notice = Some(error);
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
