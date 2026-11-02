//! The robot voice (ADR-012): a ring modulator — the voice multiplied with a low sine —
//! followed by a short feedback comb filter that gives the result one metallic pitch.
//! The ring modulator removes the voice's own pitch and leaves sum and difference tones,
//! which is why every sentence comes out on the same monotone; the comb adds the "tin can".

use std::f32::consts::TAU;

/// Carrier of the ring modulator. Lower buzzes, higher whistles; 60 Hz is the classic.
const CARRIER_HZ: f32 = 60.0;
/// The comb's resonance: 120 Hz, two octaves under a male voice's range.
const COMB_HZ: f32 = 120.0;
const COMB_FEEDBACK: f32 = 0.6;
/// The comb resonance can raise peaks to 1 / (1 − feedback); this keeps the wet path at
/// about the dry level.
const WET_GAIN: f32 = 0.5;

#[derive(Debug, Clone)]
pub struct Robot {
    phase: f32,
    phase_step: f32,
    comb: Vec<f32>,
    comb_pos: usize,
    /// Wet share, 0..=1.
    amount: f32,
}

impl Robot {
    pub fn new(sample_rate: f32, amount: f32) -> Self {
        Self {
            phase: 0.0,
            phase_step: TAU * CARRIER_HZ / sample_rate,
            comb: vec![0.0; (sample_rate / COMB_HZ).max(1.0) as usize],
            comb_pos: 0,
            amount: Self::clamp(amount),
        }
    }

    fn clamp(amount: f32) -> f32 {
        if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// 0 = only the voice, 1 = only the robot.
    pub fn set_amount(&mut self, amount: f32) {
        self.amount = Self::clamp(amount);
    }

    pub fn amount(&self) -> f32 {
        self.amount
    }

    /// In place, no allocation.
    pub fn process(&mut self, block: &mut [f32]) {
        let len = self.comb.len();
        for s in block.iter_mut() {
            let x = if s.is_finite() { *s } else { 0.0 };
            let carrier = self.phase.sin();
            self.phase += self.phase_step;
            if self.phase >= TAU {
                self.phase -= TAU;
            }
            let ring = x * carrier;
            let delayed = self.comb[self.comb_pos];
            let mut wet = ring + delayed * COMB_FEEDBACK;
            if wet.abs() < 1e-20 {
                wet = 0.0; // keep denormals out of the feedback path
            }
            self.comb[self.comb_pos] = wet;
            self.comb_pos = (self.comb_pos + 1) % len;
            *s = x * (1.0 - self.amount) + wet * WET_GAIN * self.amount;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn voice(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (TAU * 180.0 * i as f32 / SR).sin() * 0.4)
            .collect()
    }

    #[test]
    fn amount_zero_is_the_dry_voice() {
        let mut r = Robot::new(SR, 0.0);
        let mut block = voice(480);
        let before = block.clone();
        r.process(&mut block);
        assert_eq!(block, before);
    }

    #[test]
    fn the_robot_changes_the_voice_but_keeps_its_level_in_bounds() {
        let mut r = Robot::new(SR, 1.0);
        let mut signal = voice(9_600);
        let before = signal.clone();
        r.process(&mut signal);
        assert!(signal.iter().all(|s| s.is_finite()));
        let diff: f32 = signal
            .iter()
            .zip(&before)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / signal.len() as f32;
        assert!(diff > 0.05, "the voice went through unchanged");
        let rms = crate::rms(&signal[4_800..]);
        assert!(rms > 0.05 && rms < 0.6, "{rms}");
    }

    #[test]
    fn nan_input_does_not_poison_the_comb() {
        let mut r = Robot::new(SR, 1.0);
        let mut block = [f32::NAN; 480];
        r.process(&mut block);
        let mut next = voice(480);
        r.process(&mut next);
        assert!(next.iter().all(|s| s.is_finite()));
    }
}
