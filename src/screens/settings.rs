//! Settings screen placeholder.

use iced::{
    widget::{button, column, container, pick_list, row, text, text_input, Space},
    Element, Length,
};

use crate::{
    app::messages::Message,
    components::icon::{icon, Icon},
    state::AppState,
    theme,
    theme::{Fonts, Palette},
};

pub fn settings_screen<'a>(state: &'a AppState) -> Element<'a, Message> {
    let input = pick_list(
        state.media.input_devices.clone(),
        state.media.selected_input.clone(),
        Message::InputDeviceSelected,
    )
    .placeholder("Select microphone")
    .width(Length::Fixed(360.0));
    let output = pick_list(
        state.media.output_devices.clone(),
        state.media.selected_output.clone(),
        Message::OutputDeviceSelected,
    )
    .placeholder("Select speakers")
    .width(Length::Fixed(360.0));
    let screen_options = state.media.screens.clone();
    let selected_screen = state
        .media
        .screens
        .get(state.media.selected_screen)
        .cloned();
    let screen = pick_list(screen_options, selected_screen, |name| {
        Message::ScreenDeviceSelected(
            state
                .media
                .screens
                .iter()
                .position(|screen| screen == &name)
                .unwrap_or_default(),
        )
    })
    .placeholder("Select display")
    .width(Length::Fixed(360.0));
    let token_selector = pick_list(
        (0..state.auth_tokens.len())
            .map(TokenOption)
            .collect::<Vec<_>>(),
        state.selected_auth_token.as_ref().and_then(|selected| {
            state
                .auth_tokens
                .iter()
                .position(|token| token == selected)
                .map(TokenOption)
        }),
        |option| Message::AuthTokenSelected(option.0),
    )
    .placeholder("Select configured token")
    .width(Length::Fixed(360.0));
    let manual_token = text_input("Paste a token for testing", &state.auth_token_input)
        .on_input(Message::AuthTokenInputChanged)
        .on_submit(Message::ApplyAuthToken)
        .secure(true)
        .width(Length::Fixed(360.0));

    container(
        column![
            Space::new().height(Length::Fixed(60.0)),
            icon(Icon::Settings, 48.0),
            button(text("← Back").size(13).color(Palette::TEXT_SECONDARY))
                .on_press(Message::BackToHome)
                .style(theme::btn_ghost),
            text("Settings")
                .size(24)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            text("Audio and screen capture")
                .size(14)
                .color(Palette::TEXT_SECONDARY),
            row![text("Microphone").width(Length::Fixed(120.0)), input].spacing(12),
            row![text("Speakers").width(Length::Fixed(120.0)), output].spacing(12),
            row![text("Display").width(Length::Fixed(120.0)), screen].spacing(12),
            text("Backend authentication")
                .size(16)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            text("Select a configured token or enter one manually to reconnect.")
                .size(13)
                .color(Palette::TEXT_SECONDARY),
            row![text("Token").width(Length::Fixed(120.0)), token_selector].spacing(12),
            row![
                text("Manual token").width(Length::Fixed(120.0)),
                manual_token,
                button(text("Apply"))
                    .on_press(Message::ApplyAuthToken)
                    .style(theme::btn_primary)
            ]
            .spacing(12),
            state
                .auth_token_error
                .as_deref()
                .map(|error| text(error).size(12).color(Palette::DANGER))
                .unwrap_or_else(|| text("")),
            text(if state.media.capture_active {
                "Microphone capture active"
            } else {
                "Microphone capture inactive"
            })
            .size(13)
            .color(if state.media.capture_active {
                Palette::SUCCESS
            } else {
                Palette::TEXT_SECONDARY
            }),
            state
                .backend_room
                .as_ref()
                .map(|_| {
                    text(if state.media.voice_connected {
                        "Audio WebRTC conectado ao outro participante"
                    } else {
                        "Audio WebRTC aguardando conexão com outro participante"
                    })
                    .size(13)
                    .color(if state.media.voice_connected {
                        Palette::SUCCESS
                    } else {
                        Palette::AMBER
                    })
                })
                .unwrap_or_else(|| text("")),
            state
                .backend_room
                .as_ref()
                .map(|_| {
                    text(if state.media.remote_audio_received {
                        "Áudio remoto recebido"
                    } else {
                        "Nenhum pacote de áudio remoto recebido"
                    })
                    .size(13)
                    .color(if state.media.remote_audio_received {
                        Palette::SUCCESS
                    } else {
                        Palette::TEXT_SECONDARY
                    })
                })
                .unwrap_or_else(|| text("")),
            state
                .media
                .last_error
                .as_deref()
                .map(|error| text(error).size(12).color(Palette::DANGER))
                .unwrap_or_else(|| text("")),
            button(text("Refresh devices"))
                .on_press(Message::RefreshMediaDevices)
                .style(theme::btn_primary),
        ]
        .spacing(12)
        .align_x(iced::alignment::Horizontal::Center)
        .padding(40),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Palette::BG_BASE)),
        ..Default::default()
    })
    .height(Length::Fill)
    .width(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .into()
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct TokenOption(usize);

impl std::fmt::Display for TokenOption {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Token {}", self.0 + 1)
    }
}
