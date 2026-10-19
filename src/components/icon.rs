use iced::{
    widget::{svg, Svg},
    Element,
};

use crate::app::messages::Message;

#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Chat,
    Deafen,
    DeafenOff,
    Lightning,
    Mic,
    MicOff,
    Phone,
    Screen,
    Settings,
    Speaker,
}

pub fn icon<'a>(name: Icon, size: f32) -> Element<'a, Message> {
    let bytes = match name {
        Icon::Chat => include_bytes!("../../assets/icons/chat.svg").as_slice(),
        Icon::Deafen => include_bytes!("../../assets/icons/deafen.svg").as_slice(),
        Icon::DeafenOff => include_bytes!("../../assets/icons/deafen-off.svg").as_slice(),
        Icon::Lightning => include_bytes!("../../assets/icons/lightning.svg").as_slice(),
        Icon::Mic => include_bytes!("../../assets/icons/mic.svg").as_slice(),
        Icon::MicOff => include_bytes!("../../assets/icons/mic-off.svg").as_slice(),
        Icon::Phone => include_bytes!("../../assets/icons/phone.svg").as_slice(),
        Icon::Screen => include_bytes!("../../assets/icons/screen.svg").as_slice(),
        Icon::Settings => include_bytes!("../../assets/icons/settings.svg").as_slice(),
        Icon::Speaker => include_bytes!("../../assets/icons/speaker.svg").as_slice(),
    };

    Svg::new(svg::Handle::from_memory(bytes))
        .width(size)
        .height(size)
        .into()
}
