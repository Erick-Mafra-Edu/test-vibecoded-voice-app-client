use std::{
    sync::mpsc::{self, SyncSender},
    thread,
    time::Duration,
};

use screenshots::Screen;

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
