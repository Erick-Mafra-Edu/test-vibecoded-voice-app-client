//! Overlay mode — compact floating strip for use during games.

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
    theme::{self, Palette},
};

pub fn overlay_screen<'a>(state: &'a AppState) -> Element<'a, Message> {
    let user = &state.local_user;

    // Compact participant avatars
    let participants: Vec<Element<'a, Message>> = state
        .participants
        .iter()
        .map(|p| {
            container(
                column![
                    avatar::avatar(
                        &p.initials(),
                        p.avatar_color,
                        32.0,
                        p.is_speaking,
                        p.speaking_level,
                        Some((p.id as usize - 2) % 4),
                    ),
                    text(if p.is_speaking { "●" } else { "" })
                        .size(8)
                        .color(Palette::CYAN),
                ]
                .spacing(2)
                .align_x(iced::alignment::Horizontal::Center),
            )
            .into()
        })
        .collect();

    // Mute indicator
    let mute_icon = if user.is_muted {
        Icon::MicOff
    } else {
        Icon::Mic
    };
    let strip = container(
        row![
            // Logo
            icon(Icon::Lightning, 14.0),
            Space::new().width(Length::Fixed(4.0)),
            // Participants
            row(participants).spacing(6),
            Space::new().width(Length::Fixed(8.0)),
            container(iced::widget::rule::vertical(1).style(theme::rule_subtle)).height(20),
            Space::new().width(Length::Fixed(4.0)),
            // Mute
            button(icon(mute_icon, 14.0))
                .on_press(Message::ToggleMute)
                .style(theme::btn_ghost)
                .padding([4, 6]),
            // Latency
            text(format!("{}ms", state.metrics.latency_ms))
                .size(11)
                .color(Palette::SUCCESS),
            Space::new().width(Length::Fixed(4.0)),
            // Exit overlay
            button(text("⊞").size(12).color(Palette::TEXT_DISABLED),)
                .on_press(Message::SetMode(crate::state::AppMode::Normal))
                .style(theme::btn_ghost)
                .padding([4, 6]),
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center)
        .padding([6, 12]),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.88,
            ..Palette::BG_ELEVATED
        })),
        border: iced::Border {
            color: Color {
                a: 0.40,
                ..Palette::CYAN
            },
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: iced::Shadow {
            color: Color {
                a: 0.50,
                ..Color::BLACK
            },
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    });

    // In a real overlay the window would be borderless & always-on-top.
    // In prototype we show it centred with a background.
    container(column![
        Space::new().height(Length::Fill),
        container(row![
            Space::new().width(Length::Fill),
            strip,
            Space::new().width(Length::Fill),
        ]),
        Space::new().height(Length::Fixed(48.0)),
    ])
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.30,
            ..Color::BLACK
        })),
        ..Default::default()
    })
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}
