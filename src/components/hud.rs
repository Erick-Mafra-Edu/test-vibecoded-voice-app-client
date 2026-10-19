//! HUD component — latency, jitter and packet-loss indicators.

use iced::{
    widget::{column, container, row, text},
    Color, Element,
};

use crate::{app::messages::Message, state::NetworkMetrics, theme::Palette};

pub fn hud<'a>(metrics: &NetworkMetrics) -> Element<'a, Message> {
    let quality_color = if metrics.latency_ms < 50 && metrics.packet_loss_pct < 0.5 {
        Palette::SUCCESS
    } else if metrics.latency_ms < 100 {
        Palette::AMBER
    } else {
        Palette::DANGER
    };

    let metric = |label: &'a str, value: String, color: Color| -> Element<'a, Message> {
        column![
            text(value)
                .size(13)
                .color(color)
                .font(crate::theme::Fonts::BOLD),
            text(label).size(10).color(Palette::TEXT_DISABLED),
        ]
        .spacing(1)
        .align_x(iced::alignment::Horizontal::Center)
        .into()
    };

    let lat_color = if metrics.latency_ms < 50 {
        Palette::SUCCESS
    } else if metrics.latency_ms < 150 {
        Palette::AMBER
    } else {
        Palette::DANGER
    };

    let jit_color = if metrics.jitter_ms < 10 {
        Palette::SUCCESS
    } else if metrics.jitter_ms < 30 {
        Palette::AMBER
    } else {
        Palette::DANGER
    };

    let pl_color = if metrics.packet_loss_pct < 0.5 {
        Palette::SUCCESS
    } else if metrics.packet_loss_pct < 2.0 {
        Palette::AMBER
    } else {
        Palette::DANGER
    };

    let pill = container(
        row![
            // Quality dot
            container(text("●").size(10).color(quality_color),).padding([0, 4]),
            metric("LATENCY", format!("{}ms", metrics.latency_ms), lat_color),
            divider(),
            metric("JITTER", format!("{}ms", metrics.jitter_ms), jit_color),
            divider(),
            metric(
                "PKT LOSS",
                format!("{:.1}%", metrics.packet_loss_pct),
                pl_color
            ),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
    )
    .style(crate::theme::pill)
    .padding([6, 12]);

    pill.into()
}

fn divider<'a>() -> Element<'a, Message> {
    container(iced::widget::rule::vertical(1).style(crate::theme::rule_subtle))
        .height(12)
        .into()
}
