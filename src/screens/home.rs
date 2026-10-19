//! Home screen — voice grid, channel participants, chat panel.

use iced::{
    widget::{column, container, row, text, Space},
    Element, Length,
};

use crate::{
    app::messages::Message,
    components::{
        channel_bar, chat_panel, dock,
        icon::{icon, Icon},
        participant_card, top_bar,
    },
    state::{AppState, NavTab},
    theme::{self, Fonts, Palette},
};

pub fn home_screen<'a>(state: &'a AppState) -> Element<'a, Message> {
    // ── Top bar ──────────────────────────────────────────────────────────────
    let top = top_bar::top_bar(state);

    // ── Channel bar ──────────────────────────────────────────────────────────
    let channels = channel_bar::channel_bar(state);

    // ── Central voice grid ──────────────────────────────────────────────────
    let voice_grid = match state.nav_tab {
        NavTab::Home => voice_participant_grid(state),
        NavTab::Servers => servers_panel(state),
        NavTab::Profile => profile_panel(state),
        NavTab::Settings => super::settings::settings_screen(state),
    };

    // ── Chat panel (right side) ──────────────────────────────────────────────
    let chat = if state.chat_visible {
        chat_panel::chat_panel(state)
    } else {
        // Collapsed chat toggle button
        container(
            iced::widget::button(icon(Icon::Chat, 16.0))
                .on_press(Message::ChatVisibilityToggle)
                .style(theme::btn_ghost)
                .padding([8, 10]),
        )
        .height(Length::Fill)
        .into()
    };

    // ── Main content area (grid + chat) ──────────────────────────────────────
    let content: Element<'a, Message> = row![
        container(voice_grid)
            .width(Length::FillPortion(7))
            .height(Length::Fill),
        container(chat)
            .width(Length::FillPortion(3))
            .height(Length::Fill),
    ]
    .height(Length::Fill)
    .width(Length::Fill)
    .spacing(8)
    .into();

    // ── Dock (bottom center float) ──────────────────────────────────────────
    let dock_widget = dock::dock(state);

    let dock_row = container(
        row![
            Space::new().width(Length::Fill),
            dock_widget,
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(iced::padding::bottom(12))
    .width(Length::Fill);

    // ── Full layout ──────────────────────────────────────────────────────────
    let layout = column![top, channels, content, dock_row]
        .spacing(0)
        .height(Length::Fill)
        .width(Length::Fill);

    container(
        container(layout)
            .max_width(1440)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .align_x(iced::alignment::Horizontal::Center)
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Palette::BG_BASE)),
        ..Default::default()
    })
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

// ─────────────────────────────────────────────────────────────
//  Voice participant grid
// ─────────────────────────────────────────────────────────────

fn voice_participant_grid<'a>(state: &'a AppState) -> Element<'a, Message> {
    if !state.joined_voice {
        return empty_voice_state(state);
    }

    // Build participant cards
    let cards: Vec<Element<'a, Message>> = state
        .participants
        .iter()
        .map(|p| participant_card::participant_card(p))
        .collect();

    let grid = row(cards)
        .spacing(14)
        .padding(20)
        .wrap()
        .vertical_spacing(14)
        .align_x(iced::alignment::Horizontal::Center);

    // Channel heading
    let ch = &state.voice_channels[state.active_voice_channel];
    let heading = container(
        row![
            text("IN VOICE")
                .size(14)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            text(format!(
                "({})",
                state.participants.len() + usize::from(state.backend_room.is_some())
            ))
            .size(14)
            .font(Fonts::BOLD)
            .color(Palette::CYAN),
            text(format!("· {}", ch.name))
                .size(12)
                .color(Palette::TEXT_SECONDARY),
            Space::new().width(Length::Fill),
            // Mode switch button
            iced::widget::button(text("Overlay").size(12).color(Palette::TEXT_SECONDARY),)
                .on_press(Message::SetMode(crate::state::AppMode::Overlay))
                .style(theme::btn_ghost)
                .padding([5, 10]),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(iced::Padding {
        top: 18.0,
        right: 20.0,
        bottom: 12.0,
        left: 20.0,
    })
    .width(Length::Fill);

    container(
        column![
            heading,
            iced::widget::rule::horizontal(1).style(theme::rule_subtle),
            iced::widget::scrollable(grid)
                .style(theme::scrollable_style)
                .height(Length::Fill)
                .width(Length::Fill),
        ]
        .spacing(0),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn empty_voice_state<'a>(_state: &'a AppState) -> Element<'a, Message> {
    container(
        column![
            icon(Icon::Speaker, 48.0),
            text("Not in a voice channel")
                .size(18)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            text("Pick a voice channel above to join")
                .size(13)
                .color(Palette::TEXT_SECONDARY),
            iced::widget::button(text("Join voice").size(14).color(Palette::BG_BASE),)
                .on_press(Message::JoinVoice)
                .style(theme::btn_primary)
                .padding([10, 24]),
        ]
        .spacing(12)
        .align_x(iced::alignment::Horizontal::Center),
    )
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn servers_panel<'a>(state: &'a AppState) -> Element<'a, Message> {
    let servers = state.servers.iter().enumerate().map(|(index, server)| {
        iced::widget::button(
            row![
                text(server.icon.to_string()).size(20).color(Palette::CYAN),
                text(&server.name).size(16).font(Fonts::MEDIUM),
                Space::new().width(Length::Fill),
                text(if index == state.selected_server {
                    "CONNECTED"
                } else {
                    "SELECT"
                })
                .size(10)
                .color(if index == state.selected_server {
                    Palette::CYAN
                } else {
                    Palette::TEXT_DISABLED
                }),
            ]
            .spacing(12)
            .align_y(iced::alignment::Vertical::Center),
        )
        .on_press(Message::ServerSelected(index))
        .style(if index == state.selected_server {
            theme::btn_nav_active
        } else {
            theme::btn_nav
        })
        .padding(18)
        .width(Length::Fill)
        .into()
    });
    container(
        column![
            text("SERVERS")
                .size(20)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            column(servers.collect::<Vec<_>>())
                .spacing(10)
                .width(Length::Fill),
        ]
        .spacing(24)
        .padding(28),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn profile_panel<'a>(state: &'a AppState) -> Element<'a, Message> {
    let user = &state.local_user;
    container(
        column![
            text("PROFILE")
                .size(20)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            crate::components::avatar::avatar(
                &user.initials(),
                user.avatar_color,
                100.0,
                false,
                0.0,
                Some(3)
            ),
            text(&user.display_name)
                .size(22)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            text(if state.joined_voice {
                format!(
                    "●  Online · {}",
                    state.voice_channels[state.active_voice_channel].name
                )
            } else {
                "○  Offline".to_owned()
            })
            .size(13)
            .color(Palette::SUCCESS),
            text(format!(
                "Latency {} ms  ·  Jitter {} ms  ·  Loss {:.1}%",
                state.metrics.latency_ms, state.metrics.jitter_ms, state.metrics.packet_loss_pct
            ))
            .size(12)
            .color(Palette::TEXT_SECONDARY),
        ]
        .spacing(16)
        .padding(28),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
