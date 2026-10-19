use iced::{
    advanced::{layout, mouse, renderer, widget::tree, Layout, Widget},
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, mouse_area, opaque, row, stack, text, Space},
    Color, Element, Length, Rectangle, Renderer, Size, Theme,
};

use crate::theme::{self, Fonts, Palette};

pub fn modal<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    title: impl Into<String>,
    content: Element<'a, Message>,
    on_close: Message,
) -> Element<'a, Message> {
    let backdrop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color {
                    a: 0.72,
                    ..Color::BLACK
                })),
                ..Default::default()
            }),
    )
    .on_press(on_close.clone());

    let header = row![
        text(title.into())
            .size(18)
            .font(Fonts::BOLD)
            .color(Palette::TEXT_PRIMARY),
        Space::new().width(Length::Fill),
        button(text("×").size(20))
            .on_press(on_close)
            .style(theme::btn_ghost),
    ]
    .align_y(Vertical::Center);

    let card = opaque(
        container(column![header, content].spacing(20))
            .width(Length::Fixed(440.0))
            .padding(24)
            .style(theme::card_elevated),
    );
    let dialog = container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    stack([
        Element::new(InertBase(base)),
        backdrop.into(),
        dialog.into(),
    ])
    .into()
}

// Renders the underlying screen without allowing its focused controls to receive keyboard input.
struct InertBase<'a, Message>(Element<'a, Message>);

impl<Message> Widget<Message, Theme, Renderer> for InertBase<'_, Message> {
    fn tag(&self) -> tree::Tag {
        self.0.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.0.as_widget().state()
    }

    fn children(&self) -> Vec<tree::Tree> {
        self.0.as_widget().children()
    }

    fn diff(&self, tree: &mut tree::Tree) {
        self.0.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.0.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.0.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut tree::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.0.as_widget_mut().layout(tree, renderer, limits)
    }

    fn draw(
        &self,
        tree: &tree::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.0.as_widget().draw(
            tree,
            renderer,
            theme,
            style,
            layout,
            mouse::Cursor::Unavailable,
            viewport,
        );
    }
}
