//! Audio devices behind the [`AudioIo`] trait, and the cpal implementation.
//!
//! The data callbacks only move samples between the device buffer and `rtrb` rings and bump
//! atomics — no allocation, no locks, no logging. Capture keeps the device's precision
//! (`f32` all the way to the transport, E0) and takes the configured input channel;
//! playout puts the mono mix on every channel. A device that is not at 48 kHz is taken at
//! its own rate (A1, ADR-009): the capture ring carries that rate and the pump converts;
//! playout converts in the callback through [`crate::resample::ResampledRender`].

use crate::mixer::MixerRt;
use crate::resample::{ResampledRender, MAX_CALLBACK_FRAMES};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, SupportedStreamConfig};
use engine_protocol::AudioDevice;
use rtrb::Producer;
use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::mpsc;
use std::sync::Arc;
use transport::SAMPLE_RATE;

/// Written by the callbacks, read by the engine's control thread.
#[derive(Default)]
pub struct DeviceCounters {
    /// Largest callback buffer seen, in frames.
    pub callback_frames: AtomicU32,
    /// Capture: samples dropped because the ring was full.
    pub overruns: AtomicU32,
    /// Glitches the driver reported (buffer over- or underrun in the device itself).
    pub driver_xruns: AtomicU32,
    /// Set by the stream's error callback: the device is gone or broken.
    pub failed: AtomicBool,
}

/// An open stream. Dropping it closes the device.
pub struct OpenDevice {
    /// The device's id as the device list shows it (`None` when the backend has none) —
    /// so the engine can tell whether the chosen device or a stand-in is open (A2).
    pub id: Option<String>,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Capture only: the input channel actually used (after clamping to the device).
    pub channel: u16,
    pub counters: Arc<DeviceCounters>,
    /// Whatever keeps the stream alive.
    pub keep: Box<dyn Any + Send>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeviceList {
    pub inputs: Vec<AudioDevice>,
    pub outputs: Vec<AudioDevice>,
}

/// The engine's view of the sound hardware. `None` as id means the system default.
pub trait AudioIo: Send {
    fn devices(&self) -> DeviceList;
    /// Mono samples of input `channel` (clamped to what the device has) into `ring`, at the
    /// device's own rate (`OpenDevice::sample_rate`); the pump converts to 48 kHz.
    fn open_capture(
        &mut self,
        id: Option<&str>,
        channel: u16,
        ring: Producer<f32>,
    ) -> Result<OpenDevice, String>;
    /// Plays what `mixer` renders. The mixer comes back through `back` when the stream is
    /// closed — or right away if opening fails — so the next device continues with the same
    /// speakers.
    fn open_playout(
        &mut self,
        id: Option<&str>,
        mixer: MixerRt,
        back: mpsc::Sender<MixerRt>,
    ) -> Result<OpenDevice, String>;
}

/// Gives its content back when dropped — how the mixer leaves a closed stream's callback.
struct Returning<T> {
    value: Option<T>,
    back: mpsc::Sender<T>,
}

impl<T> Drop for Returning<T> {
    // Runs when the stream is torn down, not in the data callback.
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            let _ = self.back.send(value);
        }
    }
}

#[derive(Default)]
pub struct CpalIo;

impl AudioIo for CpalIo {
    fn devices(&self) -> DeviceList {
        let host = cpal::default_host();
        let default_in = host.default_input_device().and_then(|d| device_id(&d));
        let default_out = host.default_output_device().and_then(|d| device_id(&d));
        fn list(
            devices: Result<impl Iterator<Item = cpal::Device>, cpal::Error>,
            default: &Option<String>,
        ) -> Vec<AudioDevice> {
            match devices {
                Ok(all) => all
                    .filter_map(|d| {
                        let id = device_id(&d)?;
                        Some(AudioDevice {
                            is_default: Some(&id) == default.as_ref(),
                            name: device_name(&d),
                            id,
                        })
                    })
                    .collect(),
                Err(e) => {
                    tracing::warn!(error = %e, "could not list audio devices");
                    Vec::new()
                }
            }
        }
        DeviceList {
            inputs: list(host.input_devices(), &default_in),
            outputs: list(host.output_devices(), &default_out),
        }
    }

    fn open_capture(
        &mut self,
        id: Option<&str>,
        channel: u16,
        ring: Producer<f32>,
    ) -> Result<OpenDevice, String> {
        let host = cpal::default_host();
        let device = match id {
            Some(id) => find(&host, id)?,
            None => host.default_input_device().ok_or("no microphone found")?,
        };
        // The fewest channels that still contain the wanted one; else any usable mode.
        let preferred = device.default_input_config().ok().map(|c| c.sample_rate());
        let config = pick_mode(
            device.supported_input_configs().map_err(text)?,
            channel + 1,
            preferred,
        )
        .or_else(|| pick_mode(device.supported_input_configs().ok()?, 1, preferred))
        .ok_or_else(|| no_mode(&device))?;
        let channel = usize::from(channel).min(usize::from(config.channels()) - 1);
        let used = channel as u16;
        let counters = Arc::new(DeviceCounters::default());
        let c = counters.clone();
        let stream = match config.sample_format() {
            SampleFormat::F32 => capture::<f32>(&device, &config, channel, ring, c),
            SampleFormat::I16 => capture::<i16>(&device, &config, channel, ring, c),
            SampleFormat::I32 => capture::<i32>(&device, &config, channel, ring, c),
            other => Err(format!("unsupported sample format {other}")),
        }?;
        stream.play().map_err(text)?;
        Ok(OpenDevice {
            channel: used,
            ..opened(&device, &config, counters, stream)
        })
    }

    fn open_playout(
        &mut self,
        id: Option<&str>,
        mixer: MixerRt,
        back: mpsc::Sender<MixerRt>,
    ) -> Result<OpenDevice, String> {
        // From here on the holder owns the mixer: whatever happens, dropping it (directly,
        // or inside a callback cpal drops) sends the mixer back.
        let holder = Returning {
            value: Some(mixer),
            back,
        };
        let host = cpal::default_host();
        let device = match id {
            Some(id) => find(&host, id)?,
            None => host
                .default_output_device()
                .ok_or("no output device found")?,
        };
        let preferred = device.default_output_config().ok().map(|c| c.sample_rate());
        let config = pick_mode(
            device.supported_output_configs().map_err(text)?,
            1,
            preferred,
        )
        .ok_or_else(|| no_mode(&device))?;
        let counters = Arc::new(DeviceCounters::default());
        let stream = match config.sample_format() {
            SampleFormat::F32 => playout::<f32>(&device, &config, holder, counters.clone()),
            SampleFormat::I16 => playout::<i16>(&device, &config, holder, counters.clone()),
            SampleFormat::I32 => playout::<i32>(&device, &config, holder, counters.clone()),
            other => Err(format!("unsupported sample format {other}")),
        }?;
        stream.play().map_err(text)?;
        Ok(opened(&device, &config, counters, stream))
    }
}

fn capture<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    channel: usize,
    mut ring: Producer<f32>,
    counters: Arc<DeviceCounters>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels());
    let on_error = error_flag(counters.clone());
    let callback = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let frames = data.len() / channels;
        counters.callback_frames.fetch_max(frames as u32, Relaxed);
        // One chunk per callback instead of one ring operation per sample (realtime review
        // 2026-10-23); what does not fit is dropped and counted.
        let fits = frames.min(ring.slots());
        if let Ok(chunk) = ring.write_chunk_uninit(fits) {
            let samples = data
                .chunks_exact(channels)
                .map(|frame| f32::from_sample(frame[channel]));
            chunk.fill_from_iter(samples);
        }
        if fits < frames {
            counters.overruns.fetch_add((frames - fits) as u32, Relaxed);
        }
    };
    device
        .build_input_stream(config.config(), callback, on_error, None)
        .map_err(text)
}

fn playout<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    mut mixer: Returning<MixerRt>,
    counters: Arc<DeviceCounters>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels());
    let on_error = error_flag(counters.clone());
    let mut mono = vec![0f32; MAX_CALLBACK_FRAMES];
    // A device not at 48 kHz gets the mix converted on the way out (A1).
    let mut resampled =
        (config.sample_rate() != SAMPLE_RATE).then(|| ResampledRender::new(config.sample_rate()));
    let callback = move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        counters
            .callback_frames
            .fetch_max((data.len() / channels) as u32, Relaxed);
        let Some(mixer) = mixer.value.as_mut() else {
            return;
        };
        for piece in data.chunks_mut(MAX_CALLBACK_FRAMES * channels) {
            let mono = &mut mono[..piece.len() / channels];
            match resampled.as_mut() {
                None => {
                    mono.fill(0.0);
                    mixer.render(mono);
                }
                Some(converter) => converter.render(mono, |at_48k| mixer.render(at_48k)),
            }
            for (frame, sample) in piece.chunks_exact_mut(channels).zip(mono.iter()) {
                frame.fill(T::from_sample(*sample));
            }
        }
    };
    device
        .build_output_stream(config.config(), callback, on_error, None)
        .map_err(text)
}

/// cpal reports glitches through this callback too, and those come from the audio thread
/// itself: count them, nothing else. Only an error that ends the stream is logged and
/// flagged, for the control thread to reopen the device.
fn error_flag(counters: Arc<DeviceCounters>) -> impl FnMut(cpal::Error) + Send + 'static {
    move |e| match e.kind() {
        cpal::ErrorKind::Xrun => {
            counters.driver_xruns.fetch_add(1, Relaxed);
        }
        // The stream goes on: rerouted to another endpoint, or no priority boost.
        cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied => {}
        _ => {
            tracing::warn!(error = %e, "audio stream ended with an error");
            counters.failed.store(true, Relaxed);
        }
    }
}

fn opened(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    counters: Arc<DeviceCounters>,
    stream: cpal::Stream,
) -> OpenDevice {
    OpenDevice {
        id: device_id(device),
        name: device_name(device),
        sample_rate: config.sample_rate(),
        channels: config.channels(),
        channel: 0,
        counters,
        keep: Box::new(stream),
    }
}

fn find(host: &cpal::Host, id: &str) -> Result<cpal::Device, String> {
    let parsed = id
        .parse()
        .map_err(|_| format!("'{id}' is not a device id"))?;
    host.device_by_id(&parsed)
        .ok_or_else(|| "the selected device is not connected".to_string())
}

/// The mode with the fewest channels (at least `min_channels`) in a format we convert: at
/// 48 kHz when the device has it, else at the device's own rate (`preferred`, its default
/// config — on Windows the shared-mode format), else the highest rate it offers. Anything
/// but 48 kHz is resampled by the engine (A1, ADR-009).
fn pick_mode(
    configs: impl Iterator<Item = cpal::SupportedStreamConfigRange>,
    min_channels: u16,
    preferred: Option<u32>,
) -> Option<SupportedStreamConfig> {
    let usable: Vec<_> = configs
        .filter(|c| {
            matches!(
                c.sample_format(),
                SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32
            ) && c.channels() >= min_channels
        })
        .collect();
    let at = |rate: u32| {
        usable
            .iter()
            .filter_map(|c| c.try_with_sample_rate(rate))
            .min_by_key(|c| c.channels())
    };
    at(SAMPLE_RATE)
        .or_else(|| preferred.and_then(at))
        .or_else(|| {
            usable
                .into_iter()
                .map(|c| c.with_max_sample_rate())
                .min_by_key(|c| c.channels())
        })
}

fn no_mode(device: &cpal::Device) -> String {
    format!(
        "'{}' offers no mode yAPPA can use (f32, i16 or i32 samples)",
        device_name(device)
    )
}

fn device_id(device: &cpal::Device) -> Option<String> {
    device.id().ok().map(|id| id.to_string())
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.to_string())
        .unwrap_or_else(|_| "unknown device".into())
}

fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}
