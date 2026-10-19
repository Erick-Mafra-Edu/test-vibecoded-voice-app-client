//! Dock component — floating audio control strip at the bottom.

use iced::{
    widget::{button, column, container, row, text, Space},
    Color, Element, Length,
};

use crate::{
    app::messages::Message,
    components::{
        avatar,
        icon::{icon, Icon},
    },
    state::AppState,
    theme::{self, Fonts, Palette},
};

pub fn dock<'a>(state: &'a AppState) -> Element<'a, Message> {
    if !state.dock_expanded {
        return dock_collapsed(state);
    }

    let user = &state.local_user;

    // User profile section (left)
    let av = avatar::avatar(
        &user.initials(),
        user.avatar_color,
        36.0,
        false,
        0.0,
        Some(3),
    );

    let profile = row![
        av,
        column![
            text(&user.display_name)
                .size(12)
                .font(Fonts::MEDIUM)
                .color(Palette::TEXT_PRIMARY),
            text(if state.joined_voice {
                let ch = &state.voice_channels[state.active_voice_channel];
                format!("VOICE · {}", ch.name)
            } else {
                "Offline".into()
            })
            .size(10)
            .color(if state.joined_voice {
                Palette::SUCCESS
            } else {
                Palette::TEXT_DISABLED
            }),
        ]
        .spacing(1),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    // Mute button
    let mute_btn = if user.is_muted {
        button(icon(Icon::MicOff, 18.0))
            .on_press(Message::ToggleMute)
            .style(theme::btn_danger)
            .padding([8, 12])
    } else {
        button(icon(Icon::Mic, 18.0))
            .on_press(Message::ToggleMute)
            .style(theme::btn_ghost)
            .padding([8, 12])
    };

    // Deafen button
    let deaf_btn = if user.is_deafened {
        button(icon(Icon::Deafen, 18.0))
            .on_press(Message::ToggleDeafen)
            .style(theme::btn_danger)
            .padding([8, 12])
    } else {
        button(icon(Icon::DeafenOff, 18.0))
            .on_press(Message::ToggleDeafen)
            .style(theme::btn_ghost)
            .padding([8, 12])
    };

    // Screen share button
    let screen_btn = if user.is_screen_sharing {
        button(icon(Icon::Screen, 18.0))
            .on_press(Message::ToggleScreenShare)
            .style(|theme, status| {
                let base = crate::theme::btn_ghost(theme, status);
                button::Style {
                    background: Some(iced::Background::Color(Color {
                        a: 0.15,
                        ..Palette::AMBER
                    })),
                    text_color: Palette::AMBER,
                    border: iced::Border {
                        color: Color {
                            a: 0.40,
                            ..Palette::AMBER
                        },
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..base
                }
            })
            .padding([8, 12])
    } else {
        button(icon(Icon::Screen, 18.0))
            .on_press(Message::ToggleScreenShare)
            .style(theme::btn_ghost)
            .padding([8, 12])
    };

    // Leave voice button (only if joined)
    let leave_btn = if state.joined_voice {
        button(icon(Icon::Phone, 18.0))
            .on_press(Message::LeaveVoice)
            .style(theme::btn_danger)
            .padding([8, 12])
    } else {
        button(icon(Icon::Phone, 18.0))
            .on_press(Message::JoinVoice)
            .style(theme::btn_ghost)
            .padding([8, 12])
    };

    // Collapse button
    let collapse_btn = button(text("▾").size(14).color(Palette::TEXT_SECONDARY))
        .on_press(Message::DockToggle)
        .style(theme::btn_ghost)
        .padding([6, 8]);

    let settings_btn = button(icon(Icon::Settings, 20.0))
        .on_press(Message::NavTabSelected(crate::state::NavTab::Settings))
        .style(theme::btn_ghost)
        .padding([8, 12]);

    let dock_row = row![
        profile,
        Space::new().width(Length::Fill),
        mute_btn,
        deaf_btn,
        screen_btn,
        settings_btn,
        leave_btn,
        container(iced::widget::rule::vertical(1).style(theme::rule_subtle)).height(28),
        collapse_btn,
    ]
    .spacing(4)
    .align_y(iced::alignment::Vertical::Center)
    .padding([10, 20]);

    container(dock_row)
        .style(theme::dock)
        .width(Length::Fixed(680.0))
        .into()
}

/// Collapsed dock — just a small bar with expand button.
fn dock_collapsed<'a>(state: &'a AppState) -> Element<'a, Message> {
    let user = &state.local_user;

    let row_inner = row![
        icon(
            if user.is_muted {
                Icon::MicOff
            } else {
                Icon::Mic
            },
            14.0,
        ),
        if user.is_deafened {
            icon(Icon::Deafen, 14.0)
        } else {
            Space::new().width(Length::Fixed(14.0)).into()
        },
        Space::new().width(Length::Fixed(8.0)),
        text(state.metrics.latency_ms.to_string())
            .size(11)
            .color(Palette::SUCCESS),
        text("ms").size(10).color(Palette::TEXT_DISABLED),
        Space::new().width(Length::Fill),
        button(text("▴").size(14).color(Palette::TEXT_SECONDARY),)
            .on_press(Message::DockToggle)
            .style(theme::btn_ghost)
            .padding([4, 8]),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center)
    .padding([6, 16]);

    container(row_inner)
        .style(theme::dock)
        .width(Length::Fixed(280.0))
        .into()
}
