//! The send chain (planning doc, "Sendekette"): high-pass → denoiser → gate / push-to-talk
//! → EQ → compressor, one 10 ms block at a time on the pump thread.
//!
//! The pump is not a real-time thread (it hands blocks to the transport, which encodes), so
//! the chain may allocate when it is reconfigured — never per block. The denoiser is built
//! here, on the thread that runs it: DeepFilterNet is not `Send` (ADR-008).

use denoise::Denoiser;
use dsp::{Biquad, BiquadKind, Coefficients, Compressor, CompressorParams, Gate, GateParams};
use engine_protocol::{CompressorSettings, DenoiserKind, EqBand, Settings, TransmitMode};
use transport::{BLOCK, SAMPLE_RATE};

const SR: f32 = SAMPLE_RATE as f32;

/// Stage ids, in order — also the order of [`Chain::levels`].
pub const STAGES: [&str; 6] = ["input", "highpass", "denoiser", "gate", "eq", "compressor"];

/// Fade for push-to-talk and the always-on mode switch: 5 ms, inaudible but click-free.
const PTT_FADE_MS: f32 = 5.0;

/// The part of the settings the chain needs; sent to the pump when it changes.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainConfig {
    pub denoiser: DenoiserKind,
    pub high_pass_hz: f32,
    pub transmit_mode: TransmitMode,
    pub vad_threshold_db: f32,
    pub vad_hold_ms: u32,
    pub eq: Vec<EqBand>,
    pub compressor: CompressorSettings,
}

impl From<&Settings> for ChainConfig {
    fn from(s: &Settings) -> Self {
        Self {
            denoiser: s.denoiser,
            high_pass_hz: s.high_pass_hz,
            transmit_mode: s.transmit_mode,
            vad_threshold_db: s.vad_threshold_db,
            vad_hold_ms: s.vad_hold_ms,
            eq: s.eq.clone(),
            compressor: s.compressor,
        }
    }
}

fn gate_params(c: &ChainConfig) -> GateParams {
    GateParams {
        threshold_db: c.vad_threshold_db,
        hold_ms: c.vad_hold_ms as f32,
        ..GateParams::default()
    }
}

fn compressor_params(c: &CompressorSettings) -> CompressorParams {
    CompressorParams {
        threshold_db: c.threshold_db,
        ratio: c.ratio,
        attack_ms: c.attack_ms,
        release_ms: c.release_ms,
        makeup_db: c.makeup_db,
        ..CompressorParams::default()
    }
}

fn high_pass_coefficients(hz: f32) -> Coefficients {
    Coefficients::new(BiquadKind::HighPass, SR, hz.min(1_000.0), 0.707)
}

fn high_pass(hz: f32) -> Option<Biquad> {
    (hz > 0.0).then(|| Biquad::new(BiquadKind::HighPass, SR, hz.min(1_000.0), 0.707))
}

/// A 0 dB band stays a filter (it is transparent): dragging a slider through 0 must not
/// make the filter disappear and come back with empty state.
fn band(b: &EqBand) -> Coefficients {
    Coefficients::new(
        BiquadKind::Peaking { gain_db: b.gain_db },
        SR,
        b.freq_hz.clamp(20.0, 20_000.0),
        b.q.max(0.1),
    )
}

fn eq(bands: &[EqBand]) -> Vec<Biquad> {
    bands
        .iter()
        .map(|b| {
            let mut f = Biquad::new(BiquadKind::HighPass, SR, 100.0, 0.707);
            f.set_coefficients(band(b));
            f
        })
        .collect()
}

pub struct Chain {
    config: ChainConfig,
    hpf: Option<Biquad>,
    denoiser: Box<dyn Denoiser>,
    gate: Gate,
    eq: Vec<Biquad>,
    compressor: Compressor,
    /// Push-to-talk / always-on gain, faded.
    manual_gain: f32,
    /// Gain change per sample: a linear 5 ms ramp, fully open after exactly 5 ms.
    manual_step: f32,
    /// After a denoiser swap the next block is faded in: the new model starts from empty
    /// state and the pump drops the backlog that built up while it loaded.
    fade_in_next: bool,
    /// RMS after each stage of the last block, 0..1, in [`STAGES`] order.
    pub levels: [f32; 6],
    /// Is the voice going out (gate open, PTT held, or always on)?
    pub open: bool,
}

fn rms(block: &[f32]) -> f32 {
    dsp::rms(block)
}

impl Chain {
    pub fn new(config: ChainConfig) -> Self {
        let denoiser = denoise::open(config.denoiser);
        tracing::info!(denoiser = denoiser.name(), "send chain ready");
        Self {
            hpf: high_pass(config.high_pass_hz),
            gate: Gate::new(SR, gate_params(&config)),
            eq: eq(&config.eq),
            compressor: Compressor::new(SR, compressor_params(&config.compressor)),
            denoiser,
            // Always on starts open; push-to-talk starts closed.
            manual_gain: if config.transmit_mode == TransmitMode::AlwaysOn {
                1.0
            } else {
                0.0
            },
            manual_step: 1.0 / (PTT_FADE_MS / 1000.0 * SR),
            fade_in_next: false,
            levels: [0.0; 6],
            open: false,
            config,
        }
    }

    /// New settings. Returns true when the denoiser was rebuilt (DeepFilterNet takes ≈ 0.4 s
    /// to load — a short gap in the voice; the pump then drops what queued up meanwhile).
    /// Filters keep their state: changing an EQ band or the high-pass while talking must
    /// not click (realtime review E1).
    pub fn reconfigure(&mut self, config: ChainConfig) -> bool {
        if config == self.config {
            return false;
        }
        let swapped = config.denoiser != self.config.denoiser;
        if swapped {
            self.denoiser = denoise::open(config.denoiser);
            self.fade_in_next = true;
            tracing::info!(denoiser = self.denoiser.name(), "denoiser switched");
        }
        if config.high_pass_hz != self.config.high_pass_hz {
            match (self.hpf.as_mut(), config.high_pass_hz > 0.0) {
                (Some(f), true) => f.set_coefficients(high_pass_coefficients(config.high_pass_hz)),
                _ => self.hpf = high_pass(config.high_pass_hz),
            }
        }
        self.gate.set_params(SR, gate_params(&config));
        if config.eq != self.config.eq {
            if config.eq.len() == self.eq.len() {
                for (f, b) in self.eq.iter_mut().zip(&config.eq) {
                    f.set_coefficients(band(b));
                }
            } else {
                self.eq = eq(&config.eq);
            }
        }
        if config.transmit_mode != self.config.transmit_mode {
            // Hand the current state over, so the new mode starts where the old one was.
            match config.transmit_mode {
                TransmitMode::VoiceActivation => {
                    self.gate.take_over(self.open, self.manual_gain);
                }
                _ => {
                    self.manual_gain = if self.config.transmit_mode == TransmitMode::VoiceActivation
                    {
                        self.gate.gain()
                    } else {
                        self.manual_gain
                    };
                }
            }
        }
        self.compressor
            .set_params(SR, compressor_params(&config.compressor));
        self.config = config;
        swapped
    }

    /// The denoiser's own delay — what the chain adds to the latency.
    pub fn latency_samples(&self) -> usize {
        self.denoiser.latency_samples()
    }

    pub fn active(&self, stage: usize) -> bool {
        match stage {
            1 => self.hpf.is_some(),
            2 => self.config.denoiser != DenoiserKind::Off,
            3 => self.open,
            4 => self.config.eq.iter().any(|b| b.gain_db != 0.0),
            5 => self.config.compressor.enabled,
            _ => true,
        }
    }

    /// One block in place. `ptt_held` only matters in push-to-talk mode.
    pub fn process(&mut self, block: &mut [f32; BLOCK], ptt_held: bool) {
        self.levels[0] = rms(block);
        if let Some(hpf) = self.hpf.as_mut() {
            hpf.process(block);
        }
        self.levels[1] = rms(block);
        let speech = self.denoiser.process(block);
        if std::mem::take(&mut self.fade_in_next) {
            let n = block.len() as f32;
            for (i, s) in block.iter_mut().enumerate() {
                *s *= i as f32 / n;
            }
        }
        self.levels[2] = rms(block);
        match self.config.transmit_mode {
            TransmitMode::VoiceActivation => {
                self.gate.process(block, speech);
                self.open = self.gate.is_open();
            }
            TransmitMode::PushToTalk => {
                self.open = ptt_held;
                self.fade(block, ptt_held);
            }
            TransmitMode::AlwaysOn => {
                self.open = true;
                self.fade(block, true);
            }
        }
        self.levels[3] = rms(block);
        for f in self.eq.iter_mut() {
            f.process(block);
        }
        self.levels[4] = rms(block);
        if self.config.compressor.enabled {
            self.compressor.process(block);
        }
        self.levels[5] = rms(block);
    }

    fn fade(&mut self, block: &mut [f32], on: bool) {
        // Linear, not exponential: an exponential 5 ms constant needs ≈ 20 ms to be fully
        // open and would eat into the first syllable after the PTT key.
        let step = if on {
            self.manual_step
        } else {
            -self.manual_step
        };
        for s in block.iter_mut() {
            self.manual_gain = (self.manual_gain + step).clamp(0.0, 1.0);
            *s *= self.manual_gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(mode: TransmitMode) -> ChainConfig {
        ChainConfig {
            denoiser: DenoiserKind::Off,
            transmit_mode: mode,
            ..ChainConfig::from(&Settings::default())
        }
    }

    fn voice(amplitude: f32) -> [f32; BLOCK] {
        std::array::from_fn(|i| (i as f32 * 0.06).sin() * amplitude)
    }

    fn run(chain: &mut Chain, amplitude: f32, blocks: usize, ptt: bool) -> f32 {
        let mut last = 0.0;
        for _ in 0..blocks {
            let mut b = voice(amplitude);
            chain.process(&mut b, ptt);
            last = rms(&b);
        }
        last
    }

    #[test]
    fn voice_activation_sends_a_voice_and_not_the_silence_around_it() {
        let mut chain = Chain::new(config(TransmitMode::VoiceActivation));
        assert!(
            run(&mut chain, 0.001, 20, false) < 1e-4,
            "-60 dBFS stays out"
        );
        assert!(!chain.open);
        assert!(run(&mut chain, 0.2, 20, false) > 0.05);
        assert!(chain.open);
        assert!(chain.levels.iter().all(|l| l.is_finite()));
    }

    #[test]
    fn push_to_talk_sends_only_while_held() {
        let mut chain = Chain::new(config(TransmitMode::PushToTalk));
        assert_eq!(run(&mut chain, 0.2, 10, false), 0.0);
        assert!(run(&mut chain, 0.2, 10, true) > 0.05);
        assert!(chain.open);
        assert!(run(&mut chain, 0.2, 10, false) < 1e-6);
        assert!(!chain.open);
    }

    #[test]
    fn always_on_sends_even_quiet_signals() {
        let mut chain = Chain::new(config(TransmitMode::AlwaysOn));
        assert!(run(&mut chain, 0.001, 20, false) > 1e-4);
    }

    #[test]
    fn reconfiguring_keeps_the_denoiser_unless_it_changes() {
        let mut chain = Chain::new(config(TransmitMode::AlwaysOn));
        assert_eq!(chain.latency_samples(), 0);
        let mut next = config(TransmitMode::AlwaysOn);
        next.eq = vec![EqBand {
            freq_hz: 3_000.0,
            gain_db: 3.0,
            q: 1.0,
        }];
        chain.reconfigure(next.clone());
        assert!(chain.active(4), "EQ on");
        next.denoiser = DenoiserKind::Rnnoise;
        assert!(chain.reconfigure(next));
        assert_eq!(chain.latency_samples(), BLOCK, "RNNoise adds one block");
        assert!(chain.active(2));
    }

    #[test]
    fn dragging_an_eq_band_keeps_the_filter_state() {
        let mut cfg = config(TransmitMode::AlwaysOn);
        cfg.eq = vec![EqBand {
            freq_hz: 1_000.0,
            gain_db: 3.0,
            q: 1.0,
        }];
        let mut chain = Chain::new(cfg.clone());
        run(&mut chain, 0.2, 5, false);
        // Through 0 dB and on: the filter stays (no rebuild, no click).
        for gain in [1.5, 0.0, -1.5] {
            cfg.eq[0].gain_db = gain;
            assert!(!chain.reconfigure(cfg.clone()), "no denoiser swap");
            assert_eq!(chain.eq.len(), 1);
        }
        assert!(chain.active(4));
        cfg.eq[0].gain_db = 0.0;
        chain.reconfigure(cfg);
        assert!(!chain.active(4), "a flat EQ counts as off");
    }

    #[test]
    fn switching_modes_hands_the_state_over() {
        let mut chain = Chain::new(config(TransmitMode::AlwaysOn));
        let mut va = config(TransmitMode::VoiceActivation);
        chain.reconfigure(va.clone());
        // The gate takes over open at full gain: no cut in the middle of a word.
        let mut b = voice(0.2);
        chain.process(&mut b, false);
        assert!((b[0] - voice(0.2)[0]).abs() < 1e-6);
        va.transmit_mode = TransmitMode::PushToTalk;
        chain.reconfigure(va);
        assert!(
            run(&mut chain, 0.2, 3, false) < 1e-6,
            "PTT without the key: closed"
        );
    }

    #[test]
    fn a_new_denoiser_is_faded_in() {
        let mut cfg = config(TransmitMode::AlwaysOn);
        let mut chain = Chain::new(cfg.clone());
        cfg.denoiser = DenoiserKind::Rnnoise;
        assert!(chain.reconfigure(cfg));
        let mut b = [0.5f32; BLOCK];
        chain.process(&mut b, false);
        assert!(chain.levels[2] < 0.5, "faded in, not at full level");
    }

    #[test]
    fn the_high_pass_can_be_switched_off() {
        let mut off = config(TransmitMode::AlwaysOn);
        off.high_pass_hz = 0.0;
        let chain = Chain::new(off);
        assert!(!chain.active(1));
    }
}
