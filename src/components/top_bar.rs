//! Top bar component — logo, server picker, navigation and HUD.

use iced::{
    widget::{button, container, pick_list, row, space::Space, text},
    Element, Length,
};

use crate::{
    app::messages::Message,
    components::{
        hud,
        icon::{icon, Icon},
    },
    state::{AppState, NavTab},
    theme::{self, Fonts, Palette},
};

pub fn top_bar<'a>(state: &'a AppState) -> Element<'a, Message> {
    let logo = container(
        row![
            icon(Icon::Lightning, 26.0),
            text("PULSE")
                .size(25)
                .font(Fonts::BOLD)
                .color(Palette::TEXT_PRIMARY),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding([0, 16]);

    // Server selector pill
    let server = &state.servers[state.selected_server];
    let server_pill = pick_list(
        state
            .servers
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<_>>(),
        Some(server.name.clone()),
        move |name| {
            Message::ServerSelected(
                state
                    .servers
                    .iter()
                    .position(|s| s.name == name)
                    .unwrap_or(0),
            )
        },
    )
    .padding([7, 12])
    .text_size(12)
    .style(|_, _| iced::widget::pick_list::Style {
        text_color: Palette::TEXT_PRIMARY,
        placeholder_color: Palette::TEXT_SECONDARY,
        handle_color: Palette::CYAN,
        background: Palette::BG_ELEVATED.into(),
        border: iced::Border {
            color: Palette::BORDER_SUBTLE,
            width: 1.0,
            radius: 14.0.into(),
        },
    });

    // Navigation tabs
    let nav = row![
        nav_btn("HOME", NavTab::Home, &state.nav_tab),
        nav_btn("SERVERS", NavTab::Servers, &state.nav_tab),
        nav_btn("PROFILE", NavTab::Profile, &state.nav_tab),
        nav_btn("SETTINGS", NavTab::Settings, &state.nav_tab),
    ]
    .spacing(2);

    let hud_widget = hud::hud(&state.metrics);

    let bar = container(
        row![
            logo,
            server_pill,
            Space::new().width(Length::Fixed(16.0)),
            nav,
            Space::new().width(Length::Fill),
            hud_widget,
            Space::new().width(Length::Fixed(16.0)),
        ]
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .height(Length::Fill),
    )
    .style(theme::top_bar)
    .height(68)
    .width(Length::Fill);

    bar.into()
}

fn nav_btn<'a>(label: &'a str, tab: NavTab, current: &NavTab) -> Element<'a, Message> {
    let is_active = &tab == current;
    let msg = Message::NavTabSelected(tab);

    if is_active {
        button(
            text(label)
                .size(13)
                .font(Fonts::MEDIUM)
                .color(Palette::CYAN),
        )
        .on_press(msg)
        .style(theme::btn_nav_active)
        .padding([6, 14])
        .into()
    } else {
        button(text(label).size(13).color(Palette::TEXT_SECONDARY))
            .on_press(msg)
            .style(theme::btn_nav)
            .padding([6, 14])
            .into()
    }
}
