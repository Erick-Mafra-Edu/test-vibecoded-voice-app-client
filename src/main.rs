//! PULSE — Low-Latency Voice Hub
//! Entry point: initializes the iced application with settings.

mod app;
mod components;
mod media;
mod runtime;
mod screens;
mod services;
mod state;
mod theme;

use app::PulseApp;
use iced::{window, Settings, Size};

fn main() -> iced::Result {
    if let Err(error) = dotenvy::dotenv() {
        if !matches!(error, dotenvy::Error::Io(ref io_error) if io_error.kind() == std::io::ErrorKind::NotFound)
        {
            log::warn!("could not load .env: {error}");
        }
    }
    apply_cli_overrides();
    env_logger::init();

    if std::env::args().any(|argument| argument == "--validate-media") {
        return validate_media();
    }

    let settings = Settings {
        fonts: vec![
            include_bytes!("../assets/fonts/Inter-Regular.ttf")
                .as_slice()
                .into(),
            include_bytes!("../assets/fonts/Inter-Bold.ttf")
                .as_slice()
                .into(),
            include_bytes!("../assets/fonts/Inter-Medium.ttf")
                .as_slice()
                .into(),
        ],
        default_font: iced::Font {
            family: iced::font::Family::Name("Inter"),
            weight: iced::font::Weight::Normal,
            stretch: iced::font::Stretch::Normal,
            style: iced::font::Style::Normal,
        },
        default_text_size: iced::Pixels(14.0),
        antialiasing: true,
        ..Settings::default()
    };

    iced::application(PulseApp::new, PulseApp::update, PulseApp::view)
        .title("PULSE — Low-Latency Hub")
        .subscription(PulseApp::subscription)
        .theme(PulseApp::theme)
        .settings(settings)
        .window(window::Settings {
            size: Size::new(1280.0, 800.0),
            min_size: Some(Size::new(900.0, 600.0)),
            position: window::Position::Centered,
            decorations: true,
            transparent: false,
            ..window::Settings::default()
        })
        .run()
}

fn apply_cli_overrides() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--token-slot")
    {
        if let Some(slot) = arguments.get(index + 1) {
            let variable = match slot.as_str() {
                "1" => Some("PULSE_AUTH_TOKEN_1"),
                "2" => Some("PULSE_AUTH_TOKEN_2"),
                _ => None,
            };
            if let Some(variable) = variable {
                if let Ok(token) = std::env::var(variable) {
                    std::env::set_var("PULSE_AUTH_TOKEN", token);
                } else {
                    log::warn!("{variable} is not configured");
                }
            } else {
                log::warn!("invalid --token-slot value; expected 1 or 2");
            }
        }
    }
    for (flag, variable) in [
        ("--token", "PULSE_AUTH_TOKEN"),
        ("--room", "PULSE_ROOM"),
        ("--signal-url", "PULSE_SIGNAL_URL"),
    ] {
        if let Some(index) = arguments.iter().position(|argument| argument == flag) {
            if let Some(value) = arguments.get(index + 1) {
                if !value.starts_with("--") && !value.trim().is_empty() {
                    std::env::set_var(variable, value);
                }
            }
        }
    }
}

fn validate_media() -> iced::Result {
    match media::audio::list_devices() {
        Ok(devices) => {
            println!("Audio devices:");
            for device in devices {
                println!(
                    "  {} [{}{}]",
                    device.name,
                    if device.is_input { "input" } else { "" },
                    if device.is_output {
                        if device.is_input {
                            ", output"
                        } else {
                            "output"
                        }
                    } else {
                        ""
                    }
                );
            }
            println!(
                "Default input: {}",
                media::audio::default_input_name()
                    .unwrap_or_else(|error| format!("unavailable ({error})"))
            );
            println!(
                "Default output: {}",
                media::audio::default_output_name()
                    .unwrap_or_else(|error| format!("unavailable ({error})"))
            );
        }
        Err(error) => println!("Audio enumeration failed: {error}"),
    }

    match media::screen::list_screens() {
        Ok(screens) => {
            println!("Screens:");
            for screen in screens {
                println!(
                    "  {}: {} ({}x{})",
                    screen.id, screen.name, screen.width, screen.height
                );
            }
            match media::screen::capture_screen(0) {
                Ok((width, height, frame)) => {
                    println!(
                        "Screen capture: OK ({width}x{height}, {} bytes)",
                        frame.len()
                    );
                }
                Err(error) => println!("Screen capture failed: {error}"),
            }
        }
        Err(error) => println!("Screen enumeration failed: {error}"),
    }

    Ok(())
}
