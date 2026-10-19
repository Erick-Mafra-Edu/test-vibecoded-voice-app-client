//! Voice participant card.

use iced::{
    widget::{column, container, row, text, Space},
    Color, Element, Length,
};

use crate::{
    app::messages::Message,
    components::avatar,
    state::Participant,
    theme::{Fonts, Palette},
};

pub fn participant_card<'a>(p: &'a Participant) -> Element<'a, Message> {
    let accent = if p.id == 3 {
        Palette::AMBER
    } else {
        Palette::CYAN
    };
    let speaking = p.is_speaking && !p.is_muted && !p.is_deafened;
    let status = if p.is_muted || p.is_deafened {
        "MIC OFF"
    } else if speaking {
        "SPEAKING"
    } else {
        "LISTENING"
    };

    let bars: Vec<Element<'a, Message>> = (0..15)
        .map(|i| {
            let wave = ((i * 7 + 3) % 11) as f32 / 10.0;
            let height = if speaking {
                7.0 + (8.0 + 20.0 * wave) * p.speaking_level
            } else {
                5.0
            };
            container(Space::new().width(5).height(height))
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(if speaking {
                        Color {
                            a: 0.45 + wave * 0.55,
                            ..accent
                        }
                    } else {
                        Palette::BG_OVERLAY
                    })),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        })
        .collect();

    container(
        column![
            container(avatar::avatar(
                &p.initials(),
                p.avatar_color,
                112.0,
                speaking,
                p.speaking_level,
                Some((p.id as usize - 2) % 4),
            ))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
            Space::new().height(10),
            text(&p.display_name)
                .size(15)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            text(status)
                .size(10)
                .font(Fonts::MEDIUM)
                .color(if speaking {
                    accent
                } else {
                    Palette::TEXT_SECONDARY
                }),
            Space::new().height(Length::Fill),
            row(bars)
                .spacing(4)
                .height(34)
                .align_y(iced::alignment::Vertical::Bottom),
            text(format!(
                "{} ms  ·  {}",
                p.ping_ms,
                if p.is_screen_sharing {
                    "SHARING"
                } else if p.is_camera_on {
                    "CAMERA"
                } else {
                    "VOICE"
                }
            ))
            .size(9)
            .color(Palette::TEXT_DISABLED),
        ]
        .spacing(7)
        .padding(14)
        .align_x(iced::alignment::Horizontal::Center),
    )
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(if speaking {
            Color { a: 0.20, ..accent }
        } else {
            Palette::BG_SURFACE
        })),
        border: iced::Border {
            color: if speaking {
                accent
            } else {
                Palette::BORDER_SUBTLE
            },
            width: if speaking { 1.5 } else { 1.0 },
            radius: 16.0.into(),
        },
        shadow: iced::Shadow {
            color: if speaking {
                Color { a: 0.22, ..accent }
            } else {
                Color::TRANSPARENT
            },
            offset: iced::Vector::new(0.0, 5.0),
            blur_radius: 14.0,
        },
        ..Default::default()
    })
    .width(Length::Fixed(205.0))
    .height(Length::Fixed(270.0))
    .into()
}
