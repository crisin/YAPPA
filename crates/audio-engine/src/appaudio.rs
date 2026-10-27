//! App audio (ADR-011): the sound of one application, streamed into the channel as the
//! `music` track.
//!
//! The capture itself is OS work and lives behind [`AppCapture`], which the shell hands to
//! the engine like the sound hardware ([`crate::AudioIo`]): Windows captures one process
//! tree through WASAPI process loopback (`crates/platform`), macOS has nothing yet and says
//! so — the UI hides the feature. The capture delivers interleaved samples at its own rate
//! and channel count from a thread of its own; that thread is the app audio's pump:
//! [`AppStream`] converts to 48 kHz stereo (the A1 resampler, one per channel), applies the
//! app audio gain and a limiter (−1 dBFS, so a game at full blast cannot clip the track)
//! and hands every 10 ms block to the transport's `music` track ([`StereoOut`]). The clock
//! is the capture's: no pacing of its own, no second clock to drift against.
//!
//! Unlike the soundboard there is no local slot: the sender hears the application directly
//! from the application, and a copy 20–40 ms later would comb-filter it. Nothing here runs
//! in the real-time callback.

use dsp::{db_to_gain, Limiter, Resampler};
use engine_protocol::AppSource;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::Arc;
use transport::{StereoOut, BLOCK, SAMPLE_RATE};

/// The limiter on the stream: a game at full blast must not clip the track.
const CEILING_DB: f32 = -1.0;
const RELEASE_MS: f32 = 80.0;

/// Where the capture delivers its samples — implemented by the engine ([`AppStream`]),
/// called from the capture's own thread. May block briefly (it hands blocks to the
/// transport, which encodes on the calling thread).
pub trait CaptureSink: Send {
    /// Interleaved samples in −1..1, `channels` per frame, at `rate` Hz — whatever the
    /// capture runs at; the sink converts. A packet may hold any number of frames.
    fn samples(&mut self, interleaved: &[f32], rate: u32, channels: u16);
    /// The capture ended without being stopped (the application closed, the device
    /// changed): the engine reports it and goes back to "not streaming".
    fn ended(&mut self, reason: String);
}

/// A running capture. Dropping it stops the capture thread and waits for it, so the sink
/// is gone when `drop` returns.
pub trait CaptureHandle: Send {}

/// The OS's way of capturing one application's sound (ADR-011), behind a trait so a missing
/// implementation hides the feature and never breaks the build (CLAUDE.md rule 3).
pub trait AppCapture: Send {
    /// False where this OS cannot capture one application — the UI hides the button.
    fn available(&self) -> bool;
    /// The running applications with a sound session; an error when the OS refuses.
    fn list(&self) -> Result<Vec<AppSource>, String>;
    /// Starts capturing the process tree of `pid`. The sink gets every packet from the
    /// capture's thread until the handle is dropped.
    fn start(
        &mut self,
        pid: u32,
        sink: Box<dyn CaptureSink>,
    ) -> Result<Box<dyn CaptureHandle>, String>;
}

/// No capture at all: for tests, the headless tool and every OS without an implementation.
#[derive(Default)]
pub struct NoCapture;

impl AppCapture for NoCapture {
    fn available(&self) -> bool {
        false
    }
    fn list(&self) -> Result<Vec<AppSource>, String> {
        Ok(Vec::new())
    }
    fn start(&mut self, _: u32, _: Box<dyn CaptureSink>) -> Result<Box<dyn CaptureHandle>, String> {
        Err("app audio is not available on this system".into())
    }
}

/// The app audio gain as the engine shares it with a running stream: dB as `f32` bits in
/// an atomic, so a slider move needs no round trip through the capture thread.
pub type SharedGain = Arc<AtomicU32>;

pub fn shared_gain(db: f32) -> SharedGain {
    Arc::new(AtomicU32::new(db.to_bits()))
}

/// The engine's end of a capture: converts, gains, limits, sends. One per started stream;
/// built on the control thread, driven from the capture thread.
pub struct AppStream {
    out: Arc<dyn StereoOut>,
    gain: SharedGain,
    on_end: Box<dyn FnMut(String) + Send>,
    /// Per channel (left, right): `None` while the capture runs at 48 kHz.
    resamplers: Option<(Resampler, Resampler)>,
    /// The rate and channel count the resamplers were built for.
    format: (u32, u16),
    limiters: (Limiter, Limiter),
    /// The block being filled, left and right, and how many frames are in.
    left: [f32; BLOCK],
    right: [f32; BLOCK],
    filled: usize,
    /// Captured frames in the capture's own rate, de-interleaved, waiting for the
    /// resampler (it pulls one sample at a time).
    pending_left: Vec<f32>,
    pending_right: Vec<f32>,
}

impl AppStream {
    pub fn new(
        out: Arc<dyn StereoOut>,
        gain: SharedGain,
        on_end: impl FnMut(String) + Send + 'static,
    ) -> Self {
        let rate = SAMPLE_RATE as f32;
        Self {
            out,
            gain,
            on_end: Box::new(on_end),
            resamplers: None,
            format: (SAMPLE_RATE, 2),
            limiters: (
                Limiter::new(rate, CEILING_DB, RELEASE_MS),
                Limiter::new(rate, CEILING_DB, RELEASE_MS),
            ),
            left: [0.0; BLOCK],
            right: [0.0; BLOCK],
            filled: 0,
            pending_left: Vec::new(),
            pending_right: Vec::new(),
        }
    }

    /// One full block: gain, limiter, out on the wire.
    fn block(&mut self) {
        let gain = db_to_gain(f32::from_bits(self.gain.load(Relaxed)));
        if gain != 1.0 {
            for (l, r) in self.left.iter_mut().zip(self.right.iter_mut()) {
                *l *= gain;
                *r *= gain;
            }
        }
        self.limiters.0.process(&mut self.left);
        self.limiters.1.process(&mut self.right);
        let mut pcm = [0i16; 2 * BLOCK];
        for (pair, (l, r)) in pcm
            .chunks_exact_mut(2)
            .zip(self.left.iter().zip(&self.right))
        {
            pair[0] = (l.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
            pair[1] = (r.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        }
        self.out.send(&pcm);
        self.filled = 0;
    }

    /// Appends one 48 kHz frame to the block in progress and flushes a full block.
    fn frame(&mut self, l: f32, r: f32) {
        self.left[self.filled] = l;
        self.right[self.filled] = r;
        self.filled += 1;
        if self.filled == BLOCK {
            self.block();
        }
    }

    /// Converts what waits in `pending_*`, in output runs the resampler can fill
    /// completely from what is there (`input_needed` is exact, so it never runs dry).
    fn resample_pending(&mut self) {
        loop {
            let Some((rl, rr)) = self.resamplers.as_mut() else {
                return;
            };
            let room = BLOCK - self.filled;
            let needed = rl.input_needed(room);
            if self.pending_left.len() < needed {
                return;
            }
            let (pl, pr) = (&self.pending_left, &self.pending_right);
            let mut il = pl.iter().copied();
            let mut ir = pr.iter().copied();
            let wrote = rl.produce(&mut self.left[self.filled..], || il.next());
            rr.produce(&mut self.right[self.filled..], || ir.next());
            self.pending_left.drain(..needed);
            self.pending_right.drain(..needed);
            self.filled += wrote;
            if self.filled == BLOCK {
                self.block();
            }
        }
    }
}

impl CaptureSink for AppStream {
    fn samples(&mut self, interleaved: &[f32], rate: u32, channels: u16) {
        let channels = channels.max(1);
        if self.format != (rate, channels) {
            self.format = (rate, channels);
            self.resamplers = (rate != SAMPLE_RATE).then(|| {
                (
                    Resampler::new(rate, SAMPLE_RATE),
                    Resampler::new(rate, SAMPLE_RATE),
                )
            });
            self.pending_left.clear();
            self.pending_right.clear();
            tracing::info!(rate, channels, "app audio format");
        }
        // Left and right; a mono capture goes on both sides, of a surround mix the front
        // pair is taken.
        let pairs = interleaved.chunks_exact(usize::from(channels)).map(|f| {
            if channels == 1 {
                (f[0], f[0])
            } else {
                (f[0], f[1])
            }
        });
        if self.resamplers.is_none() {
            for (l, r) in pairs {
                self.frame(l, r);
            }
            return;
        }
        for (l, r) in pairs {
            self.pending_left.push(l);
            self.pending_right.push(r);
        }
        self.resample_pending();
    }

    fn ended(&mut self, reason: String) {
        (self.on_end)(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Sent(Mutex<Vec<Vec<i16>>>);

    impl StereoOut for Sent {
        fn send(&self, frame: &[i16]) {
            self.0.lock().unwrap().push(frame.to_vec());
        }
    }

    fn stream(gain_db: f32) -> (AppStream, Arc<Sent>) {
        let sent = Arc::new(Sent::default());
        let stream = AppStream::new(sent.clone(), shared_gain(gain_db), |_| {});
        (stream, sent)
    }

    /// 48 kHz stereo in packets of odd sizes: blocks go out whole, left and right keep
    /// their sides on the wire.
    #[test]
    fn stereo_at_48k_goes_out_in_whole_blocks_with_the_sides_kept() {
        let (mut stream, sent) = stream(0.0);
        let mut packet = Vec::new();
        for _ in 0..(3 * BLOCK + 17) {
            packet.extend_from_slice(&[0.5, -0.25]);
        }
        stream.samples(&packet[..2 * 300], 48_000, 2);
        stream.samples(&packet[2 * 300..], 48_000, 2);
        let blocks = sent.0.lock().unwrap();
        assert_eq!(blocks.len(), 3, "17 frames stay in the buffer");
        assert_eq!(blocks[1].len(), 2 * BLOCK);
        assert!((blocks[1][0] - 16_384).abs() <= 1, "left {}", blocks[1][0]);
        assert!((blocks[1][1] - -8_192).abs() <= 1, "right {}", blocks[1][1]);
    }

    /// A 44.1 kHz mono capture is converted: about 48 000 output frames per second, on
    /// both channels, and the gain applies on the way.
    #[test]
    fn a_mono_capture_at_another_rate_is_resampled_and_gained() {
        let (mut stream, sent) = stream(-6.0);
        // One second of 0.5 in packets of 441 frames.
        for _ in 0..100 {
            stream.samples(&[0.5; 441], 44_100, 1);
        }
        let blocks = sent.0.lock().unwrap();
        assert!(
            (98..=100).contains(&blocks.len()),
            "{} blocks",
            blocks.len()
        );
        let (l, r) = (blocks[50][200], blocks[50][201]);
        assert!((l - 8_192).abs() < 60, "−6 dB on the left: {l}");
        assert_eq!(l, r, "mono on both sides");
    }

    /// A surround capture keeps its front pair, and a change of format mid-stream starts
    /// the converter over instead of mixing rates.
    #[test]
    fn surround_takes_the_front_pair_and_a_format_change_is_followed() {
        let (mut stream, sent) = stream(0.0);
        let mut packet = Vec::new();
        for _ in 0..BLOCK {
            packet.extend_from_slice(&[0.5, -0.5, 0.1, 0.1, 0.1, 0.1]);
        }
        stream.samples(&packet, 48_000, 6);
        {
            let blocks = sent.0.lock().unwrap();
            assert_eq!(blocks.len(), 1);
            assert!((blocks[0][10] - 16_384).abs() <= 1);
            assert!((blocks[0][11] - -16_384).abs() <= 1);
        }
        for _ in 0..10 {
            stream.samples(&[0.25; 2 * 441], 44_100, 2);
        }
        let blocks = sent.0.lock().unwrap();
        assert!(
            blocks.len() >= 9,
            "{} blocks after the switch",
            blocks.len()
        );
        assert!((blocks[8][100] - 8_192).abs() < 60, "{}", blocks[8][100]);
    }

    /// Full-scale input is held under the ceiling by the limiter.
    #[test]
    fn the_limiter_holds_a_hot_stream_below_the_ceiling() {
        let (mut stream, sent) = stream(12.0);
        let mut packet = Vec::new();
        for _ in 0..(5 * BLOCK) {
            packet.extend_from_slice(&[0.9, 0.9]);
        }
        stream.samples(&packet, 48_000, 2);
        let blocks = sent.0.lock().unwrap();
        let ceiling = (dsp::db_to_gain(CEILING_DB) * f32::from(i16::MAX)) as i16;
        let peak = blocks.iter().flatten().map(|s| s.abs()).max().unwrap();
        assert!(peak <= ceiling + 1, "limited: {peak} > {ceiling}");
        assert!(peak > ceiling - 2000, "but loud: {peak}");
    }
}
