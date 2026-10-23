//! Noise gate / voice activation: opens above a threshold, closes a bit lower (hysteresis)
//! and only after a hold time, and fades instead of switching so no click is heard. A
//! denoiser's speech probability can be given per block to keep the gate shut on loud
//! non-speech (a keyboard) and open on quiet speech.

use crate::{db_to_gain, gain_to_db};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateParams {
    /// Opens when the level rises above this, dBFS.
    pub threshold_db: f32,
    /// Closes only below `threshold_db - hysteresis_db`.
    pub hysteresis_db: f32,
    /// Stays open this long after the level fell below the closing point.
    pub hold_ms: f32,
    /// Fade-in when opening.
    pub attack_ms: f32,
    /// Fade-out when closing.
    pub release_ms: f32,
}

impl Default for GateParams {
    fn default() -> Self {
        Self {
            threshold_db: -50.0,
            hysteresis_db: 6.0,
            hold_ms: 300.0,
            attack_ms: 2.0,
            release_ms: 60.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Gate {
    open_level: f32,
    close_level: f32,
    hold: u32,
    attack: f32,
    release: f32,
    /// Peak envelope of the input.
    envelope: f32,
    env_release: f32,
    open: bool,
    held: u32,
    /// The gain actually applied, faded towards 0 or 1.
    gain: f32,
}

fn coefficient(ms: f32, sample_rate: f32) -> f32 {
    let samples = (ms / 1000.0 * sample_rate).max(1.0);
    (-1.0 / samples).exp()
}

impl Gate {
    pub fn new(sample_rate: f32, params: GateParams) -> Self {
        let mut gate = Self {
            open_level: 0.0,
            close_level: 0.0,
            hold: 0,
            attack: 0.0,
            release: 0.0,
            envelope: 0.0,
            // The level detector itself falls with 20 ms — fast enough for word gaps.
            env_release: coefficient(20.0, sample_rate),
            open: false,
            held: 0,
            gain: 0.0,
        };
        gate.set_params(sample_rate, params);
        gate
    }

    /// New parameters while audio runs; the state (open, fade) carries over.
    pub fn set_params(&mut self, sample_rate: f32, params: GateParams) {
        self.open_level = db_to_gain(params.threshold_db);
        self.close_level = db_to_gain(params.threshold_db - params.hysteresis_db.max(0.0));
        self.hold = (params.hold_ms.max(0.0) / 1000.0 * sample_rate) as u32;
        self.attack = coefficient(params.attack_ms, sample_rate);
        self.release = coefficient(params.release_ms, sample_rate);
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Takes over from another way of opening (push-to-talk, always on): starts open or
    /// closed at the given gain, so switching modes neither clicks nor leaks a stale state.
    pub fn take_over(&mut self, open: bool, gain: f32) {
        self.open = open;
        self.held = 0;
        self.gain = gain.clamp(0.0, 1.0);
    }

    /// The gain applied right now (0..1).
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Current input level the gate decides on, dBFS — for the threshold slider's meter.
    pub fn level_db(&self) -> f32 {
        gain_to_db(self.envelope)
    }

    /// In place. `speech`: the denoiser's probability (0..1) for this block, if there is
    /// one — below 0.2 the gate does not open, above 0.8 it does not close.
    pub fn process(&mut self, block: &mut [f32], speech: Option<f32>) {
        let veto = speech.is_some_and(|p| p < 0.2);
        let sure = speech.is_some_and(|p| p > 0.8);
        for s in block.iter_mut() {
            let x = s.abs();
            self.envelope = (self.envelope * self.env_release).max(x);
            if self.envelope > self.open_level && !veto {
                self.open = true;
                self.held = 0;
            } else if self.open && self.envelope < self.close_level && !sure {
                self.held += 1;
                if self.held > self.hold {
                    self.open = false;
                }
            } else if self.open {
                self.held = 0;
            }
            let (target, coef) = if self.open {
                (1.0, self.attack)
            } else {
                (0.0, self.release)
            };
            self.gain = target + (self.gain - target) * coef;
            *s *= self.gain;
        }
        if self.gain < 1e-20 {
            self.gain = 0.0;
        }
        if self.envelope < 1e-20 {
            self.envelope = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rms;

    const SR: f32 = 48_000.0;

    fn tone(amplitude: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 * 0.06).sin() * amplitude)
            .collect()
    }

    fn run(gate: &mut Gate, signal: &[f32]) -> Vec<f32> {
        let mut out = signal.to_vec();
        for block in out.chunks_mut(480) {
            gate.process(block, None);
        }
        out
    }

    #[test]
    fn quiet_noise_stays_out_and_speech_level_comes_through() {
        let mut gate = Gate::new(SR, GateParams::default());
        let noise = run(&mut gate, &tone(db_to_gain(-60.0), 4_800));
        assert!(rms(&noise) < 1e-6, "noise below the threshold is muted");
        let speech = run(&mut gate, &tone(db_to_gain(-20.0), 4_800));
        assert!(gate.is_open());
        assert!(
            rms(&speech[960..]) > 0.05,
            "after the fade-in the voice is there"
        );
    }

    #[test]
    fn the_gate_holds_through_a_word_gap_and_closes_after() {
        let params = GateParams::default();
        let mut gate = Gate::new(SR, params);
        run(&mut gate, &tone(db_to_gain(-20.0), 4_800));
        // 200 ms of silence: still inside the 300 ms hold.
        run(&mut gate, &vec![0.0; 9_600]);
        assert!(
            gate.is_open(),
            "a pause between words does not cut the voice"
        );
        run(&mut gate, &vec![0.0; 9_600]);
        assert!(!gate.is_open(), "closed after the hold");
    }

    #[test]
    fn hysteresis_keeps_a_voice_near_the_threshold_from_chattering() {
        let mut gate = Gate::new(SR, GateParams::default());
        run(&mut gate, &tone(db_to_gain(-45.0), 4_800));
        assert!(gate.is_open());
        // 3 dB under the threshold, but above the closing point (6 dB under): stays open.
        run(&mut gate, &tone(db_to_gain(-53.0), 48_000));
        assert!(gate.is_open());
    }

    #[test]
    fn a_speech_probability_vetoes_loud_noise_and_keeps_quiet_speech() {
        let mut gate = Gate::new(SR, GateParams::default());
        let mut keyboard = tone(db_to_gain(-20.0), 480);
        gate.process(&mut keyboard, Some(0.05));
        assert!(
            !gate.is_open(),
            "loud, but the denoiser says it is not speech"
        );
        gate.process(&mut tone(db_to_gain(-20.0), 480), Some(0.9));
        assert!(gate.is_open());
        for _ in 0..100 {
            gate.process(&mut vec![0.0; 480], Some(0.95));
        }
        assert!(gate.is_open(), "a quiet but sure voice keeps it open");
    }

    #[test]
    fn opening_and_closing_fade_instead_of_clicking() {
        let mut gate = Gate::new(SR, GateParams::default());
        let out = run(&mut gate, &vec![0.5; 480]);
        // The first samples come in faded, not at full level.
        assert!(out[0] < 0.25 && out[479] > 0.45);
    }
}
