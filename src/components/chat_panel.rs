//! Chat panel with simulated message history and a working composer.

use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space},
    Element, Length,
};

use crate::{
    app::messages::Message,
    components::avatar,
    state::{AppState, ChatMessage},
    theme::{self, Fonts, Palette},
};

pub fn chat_panel<'a>(state: &'a AppState) -> Element<'a, Message> {
    let header = container(
        row![
            text("TEXT CHAT")
                .size(13)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
            Space::new().width(Length::Fill),
            text("♙  12").size(12).color(Palette::TEXT_SECONDARY),
            button(text("✕").size(13))
                .on_press(Message::ChatVisibilityToggle)
                .style(theme::btn_ghost)
                .padding([3, 8]),
        ]
        .spacing(12)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding([12, 16])
    .width(Length::Fill);

    let messages = scrollable(
        column(state.chat_messages.iter().map(chat_message))
            .spacing(14)
            .padding([12, 16])
            .width(Length::Fill),
    )
    .style(theme::scrollable_style)
    .height(Length::Fill)
    .anchor_bottom();

    let composer = container(
        row![
            text_input("Mensagem em Quads Central...", &state.chat_input)
                .on_input(Message::ChatInputChanged)
                .on_submit(Message::ChatSubmit)
                .style(theme::text_input_style)
                .padding([10, 12])
                .size(12)
                .width(Length::Fill),
            button(text("➤").size(18).color(Palette::CYAN))
                .on_press_maybe(
                    (!state.chat_input.trim().is_empty()).then_some(Message::ChatSubmit)
                )
                .style(theme::btn_ghost)
                .padding([8, 10]),
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding([12, 14])
    .width(Length::Fill);

    container(column![
        header,
        iced::widget::rule::horizontal(1).style(theme::rule_subtle),
        messages,
        iced::widget::rule::horizontal(1).style(theme::rule_subtle),
        composer,
    ])
    .style(theme::chat_panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn chat_message<'a>(msg: &'a ChatMessage) -> Element<'a, Message> {
    let (index, color) = match msg.author.as_str() {
        "Alpha" => (0, Palette::CYAN),
        "Bravo" => (1, Palette::AMBER),
        "Charlie" => (2, Palette::TEXT_PRIMARY),
        _ => (3, Palette::TEXT_PRIMARY),
    };
    let color = if msg.is_self { Palette::CYAN } else { color };
    row![
        avatar::avatar("", (0, 217, 245), 34.0, false, 0.0, Some(index)),
        column![
            row![
                text(&msg.author).size(12).font(Fonts::BOLD).color(color),
                text(&msg.timestamp).size(10).color(Palette::TEXT_DISABLED),
            ]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center),
            text(&msg.content).size(12).color(Palette::TEXT_PRIMARY),
        ]
        .spacing(3)
        .width(Length::Fill),
    ]
    .spacing(9)
    .align_y(iced::alignment::Vertical::Top)
    .into()
}
