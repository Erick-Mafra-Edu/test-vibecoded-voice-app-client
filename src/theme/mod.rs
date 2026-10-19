//! Theme module — Lo-Fi Cyberpunk colour palette, typography and widget styles.

use iced::{
    widget::{button, container, rule, scrollable, text_input},
    Border, Color, Font, Shadow, Vector,
};

// ─────────────────────────────────────────────────────────────
//  Colour palette
// ─────────────────────────────────────────────────────────────

pub struct Palette;

impl Palette {
    // Backgrounds
    pub const BG_BASE: Color = Color::from_rgb(0.035, 0.055, 0.094); // #090E18
    pub const BG_SURFACE: Color = Color::from_rgb(0.082, 0.114, 0.173); // #151D2C
    pub const BG_ELEVATED: Color = Color::from_rgb(0.110, 0.149, 0.220); // #1C2638
    pub const BG_OVERLAY: Color = Color::from_rgb(0.141, 0.184, 0.263); // #242F43

    // Accent
    pub const CYAN: Color = Color::from_rgb(0.0, 0.851, 0.961); // #00D9F5
    pub const CYAN_DIM: Color = Color::from_rgb(0.0, 0.502, 0.565); // #008090
    pub const AMBER: Color = Color::from_rgb(1.0, 0.710, 0.278); // #FFB547
    pub const AMBER_DIM: Color = Color::from_rgb(0.600, 0.420, 0.160); // #996B29

    // Semantic
    pub const SUCCESS: Color = Color::from_rgb(0.196, 0.804, 0.392); // #32CD63
    pub const DANGER: Color = Color::from_rgb(0.937, 0.267, 0.267); // #EF4444
    pub const MUTED: Color = Color::from_rgb(0.45, 0.52, 0.64); // #728394

    // Text
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.922, 0.937, 0.953); // #EBEFFE  (off-white)
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.620, 0.678, 0.753); // #9EADC0
    pub const TEXT_DISABLED: Color = Color::from_rgb(0.380, 0.427, 0.502); // #616D80

    // Misc
    pub const BORDER_SUBTLE: Color = Color::from_rgba(0.0, 0.851, 0.961, 0.12);
    pub const TRANSPARENT: Color = Color::TRANSPARENT;
}

// ─────────────────────────────────────────────────────────────
//  Fonts
// ─────────────────────────────────────────────────────────────

pub struct Fonts;

impl Fonts {
    pub const REGULAR: Font = Font {
        family: iced::font::Family::Name("Inter"),
        weight: iced::font::Weight::Normal,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    pub const MEDIUM: Font = Font {
        family: iced::font::Family::Name("Inter"),
        weight: iced::font::Weight::Medium,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    pub const BOLD: Font = Font {
        family: iced::font::Family::Name("Inter"),
        weight: iced::font::Weight::Bold,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    pub const MONO: Font = Font {
        family: iced::font::Family::Monospace,
        weight: iced::font::Weight::Normal,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };
}

// ─────────────────────────────────────────────────────────────
//  Custom iced Theme
// ─────────────────────────────────────────────────────────────

/// Build the global iced `Theme` using a custom dark palette.
pub fn pulse_theme() -> iced::Theme {
    use iced::theme::Palette as IcedPalette;

    iced::Theme::custom(
        String::from("PULSE"),
        IcedPalette {
            background: Palette::BG_BASE,
            text: Palette::TEXT_PRIMARY,
            primary: Palette::CYAN,
            success: Palette::SUCCESS,
            danger: Palette::DANGER,
            warning: Palette::AMBER,
        },
    )
}

// ─────────────────────────────────────────────────────────────
//  Container styles
// ─────────────────────────────────────────────────────────────

/// Base surface card with border.
pub fn card(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Palette::BG_SURFACE)),
        border: Border {
            color: Palette::BORDER_SUBTLE,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..container::Style::default()
    }
}

/// Elevated card (hover/focused state).
pub fn card_elevated(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Palette::BG_ELEVATED)),
        border: Border {
            color: Color {
                a: 0.25,
                ..Palette::CYAN
            },
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: Shadow {
            color: Color {
                a: 0.20,
                ..Palette::CYAN
            },
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..container::Style::default()
    }
}

/// Top-bar / navigation bar surface.
pub fn top_bar(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.92,
            ..Palette::BG_SURFACE
        })),
        border: Border {
            color: Palette::BORDER_SUBTLE,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// Dock container (floating audio controls).
pub fn dock(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.95,
            ..Palette::BG_ELEVATED
        })),
        border: Border {
            color: Color {
                a: 0.30,
                ..Palette::CYAN
            },
            width: 1.0,
            radius: 16.0.into(),
        },
        shadow: Shadow {
            color: Color {
                a: 0.35,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 32.0,
        },
        ..container::Style::default()
    }
}

/// Chat panel.
pub fn chat_panel(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Palette::BG_SURFACE)),
        border: Border {
            color: Palette::BORDER_SUBTLE,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// Pill container (channel tabs, badges, etc.).
pub fn pill(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Palette::BG_OVERLAY)),
        border: Border {
            color: Palette::BORDER_SUBTLE,
            width: 1.0,
            radius: 20.0.into(),
        },
        ..container::Style::default()
    }
}

/// Active pill (selected channel tab).
pub fn pill_active(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.15,
            ..Palette::CYAN
        })),
        border: Border {
            color: Color {
                a: 0.70,
                ..Palette::CYAN
            },
            width: 1.0,
            radius: 20.0.into(),
        },
        ..container::Style::default()
    }
}

/// Transparent background (for overlays or voice grid).
pub fn transparent_bg(_theme: &iced::Theme) -> container::Style {
    container::Style::default()
}

// ─────────────────────────────────────────────────────────────
//  Button styles
// ─────────────────────────────────────────────────────────────

/// Primary cyan button.
pub fn btn_primary(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(Palette::CYAN)),
        text_color: Palette::BG_BASE,
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        shadow: Shadow {
            color: Color {
                a: 0.30,
                ..Palette::CYAN
            },
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..button::Style::default()
    };

    match status {
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(Color {
                a: 0.85,
                ..Palette::CYAN
            })),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(Palette::CYAN_DIM)),
            shadow: Shadow::default(),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(iced::Background::Color(Palette::CYAN_DIM)),
            text_color: Palette::TEXT_DISABLED,
            ..base
        },
        _ => base,
    }
}

/// Ghost / icon button (no background by default).
pub fn btn_ghost(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        text_color: Palette::TEXT_SECONDARY,
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(Palette::BG_OVERLAY)),
            text_color: Palette::TEXT_PRIMARY,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(Palette::BG_ELEVATED)),
            ..base
        },
        _ => base,
    }
}

/// Danger / active-muted button.
pub fn btn_danger(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.20,
            ..Palette::DANGER
        })),
        text_color: Palette::DANGER,
        border: Border {
            color: Color {
                a: 0.50,
                ..Palette::DANGER
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(Color {
                a: 0.35,
                ..Palette::DANGER
            })),
            ..base
        },
        _ => base,
    }
}

/// Navigation pill button (top nav).
pub fn btn_nav(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        text_color: Palette::TEXT_SECONDARY,
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(Palette::BG_OVERLAY)),
            text_color: Palette::TEXT_PRIMARY,
            ..base
        },
        _ => base,
    }
}

/// Active nav button.
pub fn btn_nav_active(_theme: &iced::Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.15,
            ..Palette::CYAN
        })),
        text_color: Palette::CYAN,
        border: Border {
            color: Color {
                a: 0.40,
                ..Palette::CYAN
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
        ..button::Style::default()
    }
}

// ─────────────────────────────────────────────────────────────
//  Text input
// ─────────────────────────────────────────────────────────────

pub fn text_input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let base = text_input::Style {
        background: iced::Background::Color(Palette::BG_ELEVATED),
        border: Border {
            color: Palette::BORDER_SUBTLE,
            width: 1.0,
            radius: 8.0.into(),
        },
        icon: Palette::TEXT_SECONDARY,
        placeholder: Palette::TEXT_DISABLED,
        value: Palette::TEXT_PRIMARY,
        selection: Color {
            a: 0.30,
            ..Palette::CYAN
        },
    };

    match status {
        text_input::Status::Focused { .. } => text_input::Style {
            border: Border {
                color: Color {
                    a: 0.70,
                    ..Palette::CYAN
                },
                width: 1.5,
                ..base.border
            },
            ..base
        },
        text_input::Status::Hovered => text_input::Style {
            border: Border {
                color: Color {
                    a: 0.40,
                    ..Palette::CYAN
                },
                ..base.border
            },
            ..base
        },
        _ => base,
    }
}

// ─────────────────────────────────────────────────────────────
//  Rule / divider
// ─────────────────────────────────────────────────────────────

pub fn rule_subtle(_theme: &iced::Theme) -> rule::Style {
    rule::Style {
        color: Palette::BORDER_SUBTLE,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

// ─────────────────────────────────────────────────────────────
//  Scrollable
// ─────────────────────────────────────────────────────────────

pub fn scrollable_style(_theme: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
    let base_rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Palette::BG_OVERLAY.into(),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
        },
    };

    let active_rail = scrollable::Rail {
        scroller: scrollable::Scroller {
            background: Palette::MUTED.into(),
            ..base_rail.scroller
        },
        ..base_rail
    };

    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: match status {
            scrollable::Status::Dragged { .. } | scrollable::Status::Hovered { .. } => active_rail,
            _ => base_rail,
        },
        horizontal_rail: base_rail,
        gap: None,
        auto_scroll: scrollable::default(_theme, status).auto_scroll,
    }
}
