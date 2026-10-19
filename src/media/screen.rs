use std::{
    sync::mpsc::{self, SyncSender},
    thread,
    time::Duration,
};

use screenshots::Screen;
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenDevice {
    pub id: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

pub struct ScreenCapture {
    stop: SyncSender<()>,
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
    }
}

pub fn list_screens() -> Result<Vec<ScreenDevice>, String> {
    Screen::all()
        .map_err(|error| format!("could not enumerate screens: {error}"))?
        .into_iter()
        .enumerate()
        .map(|(id, screen)| {
            let image = screen
                .capture()
                .map_err(|error| format!("could not capture screen {id}: {error}"))?;
            Ok(ScreenDevice {
                id,
                name: format!("Display {id}"),
                width: image.width(),
                height: image.height(),
            })
        })
        .collect()
}

pub fn capture_screen(screen_id: usize) -> Result<(u32, u32, Vec<u8>), String> {
    let screens = Screen::all().map_err(|error| format!("could not enumerate screens: {error}"))?;
    let screen = screens
        .get(screen_id)
        .ok_or_else(|| format!("screen not found: {screen_id}"))?;
    let image = screen
        .capture()
        .map_err(|error| format!("could not capture screen {screen_id}: {error}"))?;
    Ok((image.width(), image.height(), image.into_raw()))
}

pub fn capture_jpeg(screen_id: usize) -> Result<Vec<u8>, String> {
    let (width, height, pixels) = capture_screen(screen_id)?;
    let image = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| "invalid screen image".to_owned())?;
    let image = image::DynamicImage::ImageRgba8(image).thumbnail(640, 360);
    for quality in [55, 40, 25] {
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
            .encode_image(&image)
            .map_err(|error| format!("could not encode screen frame: {error}"))?;
        if jpeg.len() <= 48 * 1024 {
            return Ok(jpeg);
        }
    }
    Err("screen frame exceeds transport limit".into())
}

pub fn valid_jpeg(bytes: &[u8]) -> bool {
    if bytes.len() > 48 * 1024 {
        return false;
    }
    image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Jpeg)
        .into_dimensions()
        .is_ok_and(|(width, height)| width > 0 && height > 0 && width <= 640 && height <= 360)
}

pub fn start_capture(screen_id: usize) -> Result<ScreenCapture, String> {
    Screen::all()
        .map_err(|error| format!("could not enumerate screens: {error}"))?
        .get(screen_id)
        .ok_or_else(|| format!("screen not found: {screen_id}"))?;

    let (stop_sender, stop_receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("pulse-screen-capture".to_owned())
        .spawn(move || loop {
            if stop_receiver.try_recv().is_ok() {
                break;
            }
            match capture_screen(screen_id) {
                Ok((width, height, frame)) => {
                    log::trace!(
                        "captured screen frame: {width}x{height}, {} bytes",
                        frame.len()
                    );
                }
                Err(error) => {
                    log::warn!("screen capture failed: {error}");
                    break;
                }
            }
            thread::sleep(Duration::from_millis(66));
        })
        .map_err(|error| format!("could not start screen capture thread: {error}"))?;

    Ok(ScreenCapture { stop: stop_sender })
}
