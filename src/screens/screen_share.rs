//! Screen share mode — full video area + compact participant strip.

use iced::{
    widget::{column, container, row, text, Space},
    Color, Element, Length,
};

use crate::{
    app::messages::Message,
    components::{
        avatar, dock,
        icon::{icon, Icon},
    },
    state::AppState,
    theme::{self, Fonts, Palette},
};

pub fn screen_share_screen<'a>(state: &'a AppState) -> Element<'a, Message> {
    // Main "video" area (placeholder gradient)
    let video_area = container(
        column![
            Space::new().height(Length::Fill),
            container(
                column![
                    icon(Icon::Screen, 64.0),
                    text("Screen Share — Vex Ryder")
                        .size(20)
                        .font(Fonts::BOLD)
                        .color(Palette::TEXT_PRIMARY),
                    text("Simulated feed • 1920×1080 @ 60fps")
                        .size(13)
                        .color(Palette::TEXT_SECONDARY),
                ]
                .spacing(10)
                .align_x(iced::alignment::Horizontal::Center),
            )
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill),
            Space::new().height(Length::Fill),
        ]
        .align_x(iced::alignment::Horizontal::Center),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgb(0.02, 0.04, 0.08))),
        border: iced::Border {
            color: Color {
                a: 0.30,
                ..Palette::CYAN
            },
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    })
    .height(Length::Fill)
    .width(Length::Fill);

    // Compact participant strip (right sidebar)
    let participants_strip = container(
        column(
            state
                .participants
                .iter()
                .map(|p| compact_participant_card(p))
                .collect::<Vec<_>>(),
        )
        .spacing(8)
        .padding(8),
    )
    .style(theme::chat_panel)
    .width(Length::Fixed(120.0))
    .height(Length::Fill);

    // Top control bar
    let top_bar = container(
        row![
            row![
                icon(Icon::Lightning, 16.0),
                text("PULSE")
                    .size(16)
                    .font(Fonts::BOLD)
                    .color(Palette::CYAN)
            ]
            .spacing(6),
            Space::new().width(Length::Fill),
            row![
                icon(Icon::Screen, 16.0),
                text("Screen Share Mode").size(13).color(Palette::AMBER)
            ]
            .spacing(6),
            Space::new().width(Length::Fixed(16.0)),
            iced::widget::button(text("✕ Exit").size(12).color(Palette::DANGER),)
                .on_press(Message::SetMode(crate::state::AppMode::Normal))
                .style(theme::btn_danger)
                .padding([6, 12]),
        ]
        .align_y(iced::alignment::Vertical::Center)
        .spacing(12),
    )
    .style(theme::top_bar)
    .padding([0, 20])
    .height(56)
    .width(Length::Fill);

    // Dock
    let dock_row = container(
        row![
            Space::new().width(Length::Fill),
            dock::dock(state),
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(iced::padding::bottom(12))
    .width(Length::Fill);

    container(
        column![
            top_bar,
            row![
                container(video_area).padding(12).width(Length::Fill),
                participants_strip,
            ]
            .height(Length::Fill),
            dock_row,
        ]
        .spacing(0)
        .height(Length::Fill),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Palette::BG_BASE)),
        ..Default::default()
    })
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn compact_participant_card<'a>(p: &'a crate::state::Participant) -> Element<'a, Message> {
    let av = avatar::avatar(
        &p.initials(),
        p.avatar_color,
        44.0,
        p.is_speaking,
        p.speaking_level,
        Some((p.id as usize - 2) % 4),
    );

    container(
        column![
            container(av)
                .align_x(iced::alignment::Horizontal::Center)
                .width(Length::Fill),
            text(&p.display_name).size(9).color(if p.is_speaking {
                Palette::CYAN
            } else {
                Palette::TEXT_SECONDARY
            }),
        ]
        .spacing(4)
        .align_x(iced::alignment::Horizontal::Center)
        .padding(6),
    )
    .style(theme::card)
    .width(Length::Fill)
    .into()
}
