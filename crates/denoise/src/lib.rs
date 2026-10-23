//! Noise suppression for the send chain, behind one trait (planning doc, "Sendekette").
//!
//! Both implementations work on the engine's unit: one 10 ms block of 48 kHz mono `f32`
//! in -1..1, in place. Neither is real-time safe in the strict sense — DeepFilterNet
//! allocates per block inside tract — so they run on the engine's pump thread, never in a
//! device callback (realtime review 2026-10-23).
//!
//! Model loading is I/O, which is why this is its own crate and not part of `dsp`.

use engine_protocol::DenoiserKind;

/// Samples per block: 10 ms at 48 kHz.
pub const BLOCK: usize = 480;

/// Not `Send`: DeepFilterNet's tract state holds `Rc`s. Build the denoiser on the thread
/// that runs it (the pump) — the engine hands over a [`DenoiserKind`], not a denoiser.
pub trait Denoiser {
    fn name(&self) -> &'static str;
    /// One block, in place. Returns how sure the model is that this block is speech, 0..1,
    /// when it really can tell (RNNoise's VAD) — the gate uses it. `None` otherwise.
    fn process(&mut self, block: &mut [f32; BLOCK]) -> Option<f32>;
    /// What the denoiser adds to the send path, in samples (its own algorithmic delay).
    fn latency_samples(&self) -> usize;
}

/// The denoiser for a setting. A kind this build lacks (DeepFilterNet without the feature)
/// degrades to RNNoise with a log line — never no denoiser at all by surprise.
pub fn open(kind: DenoiserKind) -> Box<dyn Denoiser> {
    match kind {
        DenoiserKind::Off => Box::new(Passthrough),
        DenoiserKind::Rnnoise => Box::new(Rnnoise::new()),
        DenoiserKind::DeepFilter => {
            #[cfg(feature = "deepfilter")]
            match DeepFilter::new() {
                Ok(df) => return Box::new(df),
                Err(error) => {
                    tracing::warn!(%error, "DeepFilterNet failed to load — using RNNoise");
                }
            }
            #[cfg(not(feature = "deepfilter"))]
            tracing::info!("this build has no DeepFilterNet — using RNNoise");
            Box::new(Rnnoise::new())
        }
    }
}

pub struct Passthrough;

impl Denoiser for Passthrough {
    fn name(&self) -> &'static str {
        "off"
    }
    fn process(&mut self, _: &mut [f32; BLOCK]) -> Option<f32> {
        None
    }
    fn latency_samples(&self) -> usize {
        0
    }
}

/// RNNoise (Xiph, via the pure-Rust port nnnoiseless): small, cheap, one 10 ms frame.
pub struct Rnnoise {
    state: Box<nnnoiseless::DenoiseState<'static>>,
    input: [f32; BLOCK],
}

/// nnnoiseless works on 16-bit sample values in `f32`.
const PCM: f32 = 32_768.0;

impl Rnnoise {
    pub fn new() -> Self {
        Self {
            state: nnnoiseless::DenoiseState::new(),
            input: [0.0; BLOCK],
        }
    }
}

impl Default for Rnnoise {
    fn default() -> Self {
        Self::new()
    }
}

impl Denoiser for Rnnoise {
    fn name(&self) -> &'static str {
        "rnnoise"
    }

    fn process(&mut self, block: &mut [f32; BLOCK]) -> Option<f32> {
        for (i, s) in self.input.iter_mut().zip(block.iter()) {
            *i = s * PCM;
        }
        let speech = self.state.process_frame(block, &self.input);
        for s in block.iter_mut() {
            *s /= PCM;
        }
        Some(speech)
    }

    fn latency_samples(&self) -> usize {
        // The overlap-add window of one frame.
        BLOCK
    }
}

#[cfg(feature = "deepfilter")]
pub use deepfilter::DeepFilter;

#[cfg(feature = "deepfilter")]
mod deepfilter {
    use super::{Denoiser, BLOCK};
    use df::tract::{DfParams, DfTract, RuntimeParams};
    use ndarray::{ArrayView2, ArrayViewMut2};

    /// DeepFilterNet 3, low-latency model (embedded). Best quality, most CPU.
    pub struct DeepFilter {
        model: DfTract,
        out: [f32; BLOCK],
        latency: usize,
        /// Blocks the model refused so far (see `process`).
        failed: u32,
    }

    impl DeepFilter {
        pub fn new() -> Result<Self, String> {
            let model = DfTract::new(DfParams::default(), &RuntimeParams::default())
                .map_err(|e| e.to_string())?;
            if model.hop_size != BLOCK || model.sr != 48_000 {
                return Err(format!(
                    "model works on {} samples at {} Hz, the engine on {BLOCK} at 48000",
                    model.hop_size, model.sr
                ));
            }
            // STFT window (fft_size - hop) plus the model's lookahead in frames.
            let latency = model.fft_size - model.hop_size + model.lookahead * model.hop_size;
            Ok(Self {
                model,
                out: [0.0; BLOCK],
                latency,
                failed: 0,
            })
        }
    }

    impl Denoiser for DeepFilter {
        fn name(&self) -> &'static str {
            "deepfilter"
        }

        fn process(&mut self, block: &mut [f32; BLOCK]) -> Option<f32> {
            let noisy = ArrayView2::from_shape((1, BLOCK), &block[..]).ok()?;
            let enhanced = ArrayViewMut2::from_shape((1, BLOCK), &mut self.out[..]).ok()?;
            match self.model.process(noisy, enhanced) {
                Ok(_lsnr) => {
                    block.copy_from_slice(&self.out);
                    // The model's local SNR is not a speech probability: a quiet room has a
                    // high SNR without anyone talking, and fed to the gate it held it open
                    // for seconds (E1 measurement). The gate decides on the denoised level.
                    None
                }
                Err(error) => {
                    // One line per ~10 s, not one per block: a model that keeps failing would
                    // otherwise flood the diagnosis file.
                    if self.failed.is_multiple_of(1000) {
                        let failed = self.failed + 1;
                        tracing::warn!(%error, failed, "DeepFilterNet block failed — passing it through");
                    }
                    self.failed = self.failed.wrapping_add(1);
                    None
                }
            }
        }

        fn latency_samples(&self) -> usize {
            self.latency
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(seed: u32) -> [f32; BLOCK] {
        let mut x = seed;
        std::array::from_fn(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 8) as f32 / (1u32 << 24) as f32 * 0.2 - 0.1
        })
    }

    fn rms(block: &[f32]) -> f32 {
        (block.iter().map(|s| s * s).sum::<f32>() / block.len() as f32).sqrt()
    }

    #[test]
    fn off_changes_nothing() {
        let mut d = open(DenoiserKind::Off);
        let mut block = noise(1);
        let before = block;
        assert_eq!(d.process(&mut block), None);
        assert_eq!(block, before);
        assert_eq!(d.latency_samples(), 0);
    }

    #[test]
    fn rnnoise_turns_steady_noise_down_and_says_it_is_not_speech() {
        let mut d = open(DenoiserKind::Rnnoise);
        let mut last = (0.0, 1.0);
        for i in 0..100 {
            let mut block = noise(i);
            let p = d
                .process(&mut block)
                .expect("rnnoise reports a probability");
            last = (rms(&block), p);
        }
        // Synthetic white noise is not what RNNoise was trained on: measured ≈ 4.5 dB less
        // (S2 measures real noisy speech). Down by at least 3 dB is the contract here.
        assert!(
            last.0 < rms(&noise(7)) / std::f32::consts::SQRT_2,
            "white noise down by at least 3 dB"
        );
        assert!(last.1 < 0.5, "and not taken for speech");
        assert!(last.0.is_finite());
    }

    #[test]
    fn deepfilter_falls_back_to_rnnoise_without_the_feature() {
        let d = open(DenoiserKind::DeepFilter);
        if cfg!(feature = "deepfilter") {
            assert_eq!(d.name(), "deepfilter");
        } else {
            assert_eq!(d.name(), "rnnoise");
        }
    }
}
