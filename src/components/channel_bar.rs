//! Channel bar component — voice and text channel pills.

use iced::{
    widget::{button, container, row, text},
    Color, Element, Length,
};

use crate::{
    app::messages::Message,
    state::{AppState, Channel},
    theme::{self, Fonts, Palette},
};

pub fn channel_bar<'a>(state: &'a AppState) -> Element<'a, Message> {
    let voice_channels =
        state.voice_channels.iter().enumerate().map(|(i, ch)| {
            channel_pill_voice(ch, i, state.active_voice_channel, state.joined_voice)
        });

    let divider = container(iced::widget::rule::vertical(1).style(theme::rule_subtle))
        .padding([0, 4])
        .height(20);

    let text_channels = state
        .text_channels
        .iter()
        .enumerate()
        .map(|(i, ch)| channel_pill_text(ch, i, state.active_text_channel));

    let bar = container(
        row(voice_channels
            .chain(std::iter::once(divider.into()))
            .chain(text_channels)
            .collect::<Vec<_>>())
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center)
        .height(Length::Fill),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.60,
            ..Palette::BG_SURFACE
        })),
        border: iced::Border {
            color: Palette::BORDER_SUBTLE,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .padding([0, 16])
    .height(58)
    .width(Length::Fill);

    bar.into()
}

fn channel_pill_voice<'a>(
    ch: &'a Channel,
    index: usize,
    active: usize,
    joined: bool,
) -> Element<'a, Message> {
    let is_active = joined && index == active;
    let icon = crate::components::icon::icon(crate::components::icon::Icon::Speaker, 12.0);
    let count: Element<'a, Message> = if ch.user_count > 0 {
        container(
            text(ch.user_count.to_string())
                .size(10)
                .color(Palette::TEXT_DISABLED),
        )
        .into()
    } else {
        iced::widget::Space::new().into()
    };
    let label = row![
        icon,
        text(&ch.name)
            .size(12)
            .font(if is_active {
                Fonts::MEDIUM
            } else {
                Fonts::REGULAR
            })
            .color(if is_active {
                Palette::CYAN
            } else {
                Palette::TEXT_SECONDARY
            }),
        count,
    ]
    .spacing(4)
    .align_y(iced::alignment::Vertical::Center);

    if is_active {
        container(label)
            .style(theme::pill_active)
            .padding([10, 14])
            .into()
    } else {
        button(label)
            .on_press(Message::VoiceChannelSelected(index))
            .style(theme::btn_ghost)
            .padding([10, 14])
            .into()
    }
}

fn channel_pill_text<'a>(ch: &'a Channel, index: usize, active: usize) -> Element<'a, Message> {
    let is_active = index == active;
    let icon = "#";
    let label = row![
        text(icon).size(13).color(Palette::TEXT_DISABLED),
        text(&ch.name)
            .size(12)
            .font(if is_active {
                Fonts::MEDIUM
            } else {
                Fonts::REGULAR
            })
            .color(if is_active {
                Palette::TEXT_PRIMARY
            } else {
                Palette::TEXT_SECONDARY
            }),
    ]
    .spacing(4)
    .align_y(iced::alignment::Vertical::Center);

    if is_active {
        container(label)
            .style(theme::pill_active)
            .padding([10, 14])
            .into()
    } else {
        button(label)
            .on_press(Message::TextChannelSelected(index))
            .style(theme::btn_ghost)
            .padding([10, 14])
            .into()
    }
}
