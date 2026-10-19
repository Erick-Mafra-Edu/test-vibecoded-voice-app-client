use std::{
    sync::mpsc::{self, SyncSender},
    thread,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Sample, Stream,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
}

pub struct AudioCapture {
    stop: SyncSender<()>,
}

pub struct AudioPlayback {
    stop: SyncSender<()>,
}

impl Drop for AudioPlayback {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
    }
}

pub type AudioFrameReceiver = mpsc::Receiver<Vec<f32>>;
pub type AudioFrameSender = SyncSender<Vec<f32>>;

impl Drop for AudioCapture {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
    }
}

pub fn list_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let mut devices = std::collections::BTreeMap::<String, (bool, bool)>::new();

    for device in host
        .input_devices()
        .map_err(|error| format!("could not enumerate input devices: {error}"))?
    {
        let name = device
            .name()
            .map_err(|error| format!("could not read input device name: {error}"))?;
        devices.entry(name).or_default().0 = true;
    }

    for device in host
        .output_devices()
        .map_err(|error| format!("could not enumerate output devices: {error}"))?
    {
        let name = device
            .name()
            .map_err(|error| format!("could not read output device name: {error}"))?;
        devices.entry(name).or_default().1 = true;
    }

    Ok(devices
        .into_iter()
        .map(|(name, (is_input, is_output))| AudioDevice {
            name,
            is_input,
            is_output,
        })
        .collect())
}

pub fn default_input_name() -> Result<String, String> {
    cpal::default_host()
        .default_input_device()
        .ok_or_else(|| "no default input device is available".to_owned())?
        .name()
        .map_err(|error| format!("could not read default input device name: {error}"))
}

pub fn default_output_name() -> Result<String, String> {
    cpal::default_host()
        .default_output_device()
        .ok_or_else(|| "no default output device is available".to_owned())?
        .name()
        .map_err(|error| format!("could not read default output device name: {error}"))
}

pub fn ensure_output_device(device_name: &str) -> Result<(), String> {
    let available = cpal::default_host()
        .output_devices()
        .map_err(|error| format!("could not enumerate output devices: {error}"))?
        .any(|device| device.name().is_ok_and(|name| name == device_name));
    if available {
        Ok(())
    } else {
        Err(format!("output device not found: {device_name}"))
    }
}

pub fn start_capture(device_name: &str) -> Result<AudioCapture, String> {
    let (capture, _frames) = start_capture_with_frames(device_name)?;
    Ok(capture)
}

pub fn start_capture_with_frames(
    device_name: &str,
) -> Result<(AudioCapture, AudioFrameReceiver), String> {
    let available = cpal::default_host()
        .input_devices()
        .map_err(|error| format!("could not enumerate input devices: {error}"))?
        .any(|device| device.name().is_ok_and(|name| name == device_name));
    if !available {
        return Err(format!("input device not found: {device_name}"));
    }

    let (stop_sender, stop_receiver) = mpsc::sync_channel(1);
    let device_name = device_name.to_owned();
    let (frame_sender, frame_receiver) = mpsc::sync_channel(8);
    thread::Builder::new()
        .name("pulse-audio-capture".to_owned())
        .spawn(move || run_capture(device_name, stop_receiver, frame_sender))
        .map_err(|error| format!("could not start audio capture thread: {error}"))?;
    Ok((AudioCapture { stop: stop_sender }, frame_receiver))
}

pub fn start_playback_with_frames(
    device_name: &str,
) -> Result<(AudioPlayback, AudioFrameSender), String> {
    let device = cpal::default_host()
        .output_devices()
        .map_err(|error| format!("could not enumerate output devices: {error}"))?
        .find(|device| device.name().is_ok_and(|name| name == device_name))
        .ok_or_else(|| format!("output device not found: {device_name}"))?;
    let config = device
        .default_output_config()
        .map_err(|error| format!("could not read output configuration: {error}"))?;
    let (stop_sender, stop_receiver) = mpsc::sync_channel(1);
    let (frame_sender, frame_receiver) = mpsc::sync_channel(32);
    thread::Builder::new()
        .name("pulse-audio-playback".to_owned())
        .spawn(move || run_playback(device, config, stop_receiver, frame_receiver))
        .map_err(|error| format!("could not start audio playback thread: {error}"))?;
    Ok((AudioPlayback { stop: stop_sender }, frame_sender))
}

fn run_playback(
    device: Device,
    config: cpal::SupportedStreamConfig,
    stop_receiver: mpsc::Receiver<()>,
    frames: AudioFrameReceiver,
) {
    let stream_config = config.config();
    let channels = usize::from(stream_config.channels).max(1);
    let queue = std::sync::Arc::new(std::sync::Mutex::new(
        std::collections::VecDeque::<f32>::new(),
    ));
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build_output_stream::<f32>(&device, &stream_config, channels, queue.clone())
        }
        cpal::SampleFormat::I16 => {
            build_output_stream::<i16>(&device, &stream_config, channels, queue.clone())
        }
        cpal::SampleFormat::U16 => {
            build_output_stream::<u16>(&device, &stream_config, channels, queue.clone())
        }
        format => {
            log::warn!("unsupported output sample format: {format:?}");
            return;
        }
    };
    let Ok(stream) = stream else { return };
    if let Err(error) = stream.play() {
        log::warn!("could not start audio playback stream: {error}");
        return;
    }
    loop {
        if stop_receiver.try_recv().is_ok() {
            break;
        }
        if let Ok(frame) = frames.recv_timeout(std::time::Duration::from_millis(20)) {
            if let Ok(mut queue) = queue.lock() {
                queue.extend(frame);
            }
        }
    }
}

fn build_output_stream<T>(
    device: &Device,
    config: &cpal::StreamConfig,
    channels: usize,
    queue: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<f32>>>,
) -> Result<Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                if let Ok(mut queue) = queue.lock() {
                    for frame in data.chunks_mut(channels) {
                        let sample = queue.pop_front().unwrap_or(0.0);
                        for output in frame {
                            *output = sample.to_sample::<T>();
                        }
                    }
                }
            },
            |error| log::warn!("audio playback stream error: {error}"),
            None,
        )
        .map_err(|error| format!("could not create audio playback stream: {error}"))
}

fn run_capture(
    device_name: String,
    stop_receiver: mpsc::Receiver<()>,
    frame_sender: SyncSender<Vec<f32>>,
) {
    let result = (|| {
        let device = cpal::default_host()
            .input_devices()
            .map_err(|error| format!("could not enumerate input devices: {error}"))?
            .find(|device| device.name().is_ok_and(|name| name == device_name))
            .ok_or_else(|| format!("input device not found: {device_name}"))?;
        let config = device
            .default_input_config()
            .map_err(|error| format!("could not read input configuration: {error}"))?;
        let stream = build_stream(device, config, frame_sender)?;
        stream
            .play()
            .map_err(|error| format!("could not start audio capture: {error}"))?;

        loop {
            if stop_receiver.try_recv().is_ok() {
                break;
            }
            if stop_receiver
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_ok()
            {
                break;
            }
        }
        Ok::<(), String>(())
    })();

    if let Err(error) = result {
        log::warn!("audio capture failed: {error}");
    }
}

fn build_stream(
    device: Device,
    config: cpal::SupportedStreamConfig,
    sender: SyncSender<Vec<f32>>,
) -> Result<Stream, String> {
    let stream_config = config.config();
    let channels = usize::from(stream_config.channels).max(1);
    let error_callback = |error| log::warn!("audio capture stream error: {error}");

    match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build_sample_stream::<f32>(&device, &stream_config, channels, sender, error_callback)
        }
        cpal::SampleFormat::I16 => {
            build_sample_stream::<i16>(&device, &stream_config, channels, sender, error_callback)
        }
        cpal::SampleFormat::U16 => {
            build_sample_stream::<u16>(&device, &stream_config, channels, sender, error_callback)
        }
        format => Err(format!("unsupported input sample format: {format:?}")),
    }
}

fn build_sample_stream<T>(
    device: &Device,
    config: &cpal::StreamConfig,
    channels: usize,
    sender: SyncSender<Vec<f32>>,
    error_callback: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String>
where
    T: cpal::Sample + cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let samples = data
                    .chunks(channels)
                    .map(|frame| {
                        frame
                            .iter()
                            .map(|sample| sample.to_sample::<f32>())
                            .sum::<f32>()
                            / frame.len().max(1) as f32
                    })
                    .collect::<Vec<_>>();
                let _ = sender.try_send(samples);
            },
            error_callback,
            None,
        )
        .map_err(|error| format!("could not create audio capture stream: {error}"))
}
