//! Avatar widget — renders a coloured circle with initials.

use iced::{
    widget::{canvas, container, image},
    Color, Element, Length, Rectangle,
};
use once_cell::sync::Lazy;

use crate::{app::messages::Message, theme::Palette};

static PORTRAITS: Lazy<image::Handle> = Lazy::new(|| {
    image::Handle::from_bytes(include_bytes!("../../assets/avatar-atlas.png").as_slice())
});

/// Render a simple avatar circle with initials.
/// `size` is the diameter in logical pixels.
/// `color` is the RGB tuple for the avatar background.
pub fn avatar<'a>(
    initials: &str,
    color: (u8, u8, u8),
    size: f32,
    speaking: bool,
    speaking_level: f32,
    portrait: Option<usize>,
) -> Element<'a, Message> {
    if let Some(index) = portrait {
        let accent = Color::from_rgb8(color.0, color.1, color.2);
        let image = image(PORTRAITS.clone())
            .crop(Rectangle {
                x: (index as u32 % 2) * 627,
                y: (index as u32 / 2) * 627,
                width: 627,
                height: 627,
            })
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .border_radius(size / 2.0);
        return container(image)
            .padding(3)
            .style(move |_| iced::widget::container::Style {
                border: iced::Border {
                    color: if speaking {
                        accent
                    } else {
                        Palette::BORDER_SUBTLE
                    },
                    width: if speaking { 2.0 + speaking_level } else { 1.0 },
                    radius: (size / 2.0 + 3.0).into(),
                },
                ..Default::default()
            })
            .into();
    }
    let avatar_color = Color::from_rgb(
        color.0 as f32 / 255.0,
        color.1 as f32 / 255.0,
        color.2 as f32 / 255.0,
    );

    let initials = initials.to_string();
    let font_size = (size * 0.38).round();

    // Use a Canvas to draw the avatar circle + optional speaking ring
    let c = canvas(AvatarCanvas {
        color: avatar_color,
        initials: initials.clone(),
        size,
        speaking,
        speaking_level,
        font_size,
    })
    .width(size)
    .height(size);

    c.into()
}

struct AvatarCanvas {
    color: Color,
    initials: String,
    size: f32,
    speaking: bool,
    speaking_level: f32,
    font_size: f32,
}

impl<Message> canvas::Program<Message> for AvatarCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = (self.size / 2.0)
            .min(bounds.width / 2.0)
            .min(bounds.height / 2.0);

        // Speaking ring (outer glow)
        if self.speaking && self.speaking_level > 0.05 {
            let ring_alpha = (self.speaking_level * 0.85).clamp(0.1, 0.85);
            let ring_radius = radius + 4.0 + self.speaking_level * 6.0;
            frame.fill(
                &canvas::Path::circle(center, ring_radius),
                canvas::Fill {
                    style: canvas::Style::Solid(Color {
                        a: ring_alpha,
                        ..Palette::CYAN
                    }),
                    rule: canvas::fill::Rule::NonZero,
                },
            );
        }

        // Background circle
        frame.fill(
            &canvas::Path::circle(center, radius),
            canvas::Fill {
                style: canvas::Style::Solid(Color {
                    a: 0.85,
                    ..self.color
                }),
                rule: canvas::fill::Rule::NonZero,
            },
        );

        // Inner stroke
        frame.stroke(
            &canvas::Path::circle(center, radius),
            canvas::Stroke {
                style: canvas::stroke::Style::Solid(Color {
                    a: 0.30,
                    ..Color::WHITE
                }),
                width: 1.5,
                ..Default::default()
            },
        );

        // Initials text
        frame.fill_text(canvas::Text {
            content: self.initials.clone(),
            position: center,
            color: Color::WHITE,
            size: iced::Pixels(self.font_size),
            font: crate::theme::Fonts::BOLD,
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..Default::default()
        });

        vec![frame.into_geometry()]
    }
}
