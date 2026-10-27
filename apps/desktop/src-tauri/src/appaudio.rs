//! App audio (ADR-011): the engine's capture seam, implemented with the OS's process
//! loopback from `crates/platform`. The engine defines what it needs
//! ([`audio_engine::AppCapture`]), the platform crate knows the OS; this adapter is where
//! they meet — like the shell picks the LiveKit transport. Nothing here is audio.

use audio_engine::{AppCapture, CaptureHandle, CaptureSink};
use engine_protocol::AppSource;
use std::sync::{Arc, Mutex};

pub struct OsCapture;

/// The running loopback; dropping it stops the capture thread (and with it the sink).
struct Running(#[allow(dead_code)] platform::ProcessLoopback);

impl CaptureHandle for Running {}

impl AppCapture for OsCapture {
    fn available(&self) -> bool {
        platform::appaudio::supported()
    }

    fn list(&self) -> Result<Vec<AppSource>, String> {
        Ok(platform::appaudio::list_apps()?
            .into_iter()
            .map(|a| AppSource {
                pid: a.pid,
                name: a.name,
                active: a.active,
            })
            .collect())
    }

    fn start(
        &mut self,
        pid: u32,
        sink: Box<dyn CaptureSink>,
    ) -> Result<Box<dyn CaptureHandle>, String> {
        // Both callbacks run on the capture thread, one after the other, never at once:
        // the lock is uncontended and only keeps the borrow checker honest about the
        // sink being shared between the packet and the end callback.
        let sink = Arc::new(Mutex::new(sink));
        let for_packets = sink.clone();
        let loopback = platform::ProcessLoopback::start(
            pid,
            move |samples, rate, channels| {
                if let Ok(mut sink) = for_packets.lock() {
                    sink.samples(samples, rate, channels);
                }
            },
            move |reason| {
                if let Ok(mut sink) = sink.lock() {
                    sink.ended(reason);
                }
            },
        )?;
        Ok(Box::new(Running(loopback)))
    }
}
