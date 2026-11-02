//! The walkie-talkie (ADR-012, "Funkgerät"): the voice squeezed into the telephone band
//! (300 Hz to 3.4 kHz) and driven into a soft clipper, then band-limited again so the
//! clipper's harmonics do not spray over the whole spectrum. One knob: how hard it is
//! driven. Costs two biquads and a polynomial per sample.

use crate::{Biquad, BiquadKind};

const LOW_CUT_HZ: f32 = 300.0;
const HIGH_CUT_HZ: f32 = 3_400.0;
/// Drive at amount 1: +18 dB into the clipper.
const MAX_DRIVE: f32 = 8.0;
/// Brings a fully driven voice back to about the dry level.
const MAKEUP: f32 = 0.6;

#[derive(Debug, Clone)]
pub struct Radio {
    high_pass: Biquad,
    low_pass: Biquad,
    amount: f32,
    drive: f32,
}

/// A rational tanh: exact to a few percent up to |x| = 3, cheaper than `tanh`.
#[inline]
fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

impl Radio {
    pub fn new(sample_rate: f32, amount: f32) -> Self {
        let mut r = Self {
            high_pass: Biquad::new(BiquadKind::HighPass, sample_rate, LOW_CUT_HZ, 0.707),
            low_pass: Biquad::new(BiquadKind::LowPass, sample_rate, HIGH_CUT_HZ, 0.707),
            amount: 0.0,
            drive: 1.0,
        };
        r.set_amount(amount);
        r
    }

    /// 0 = clean but narrow, 1 = fully driven.
    pub fn set_amount(&mut self, amount: f32) {
        self.amount = if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.drive = 1.0 + (MAX_DRIVE - 1.0) * self.amount;
    }

    pub fn amount(&self) -> f32 {
        self.amount
    }

    /// In place, no allocation.
    pub fn process(&mut self, block: &mut [f32]) {
        let makeup = 1.0 + (MAKEUP - 1.0) * self.amount;
        for s in block.iter_mut() {
            let x = if s.is_finite() { *s } else { 0.0 };
            let narrow = self.high_pass.process_sample(x);
            let driven = soft_clip(narrow * self.drive) * makeup;
            *s = self.low_pass.process_sample(driven);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    fn tone(hz: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (TAU * hz * i as f32 / SR).sin() * 0.5)
            .collect()
    }

    fn steady_rms(r: &mut Radio, hz: f32) -> f32 {
        let mut signal = tone(hz, 9_600);
        r.process(&mut signal);
        crate::rms(&signal[4_800..])
    }

    #[test]
    fn the_band_is_narrow() {
        let mut r = Radio::new(SR, 0.0);
        let voice = steady_rms(&mut r, 1_000.0);
        let rumble = steady_rms(&mut Radio::new(SR, 0.0), 60.0);
        let hiss = steady_rms(&mut Radio::new(SR, 0.0), 12_000.0);
        assert!(voice > 0.3, "{voice}");
        assert!(rumble < voice / 5.0, "{rumble}");
        assert!(hiss < voice / 5.0, "{hiss}");
    }

    #[test]
    fn full_drive_flattens_the_peaks_but_stays_at_a_sane_level() {
        let mut r = Radio::new(SR, 1.0);
        let mut signal = tone(1_000.0, 9_600);
        r.process(&mut signal);
        let tail = &signal[4_800..];
        let peak = tail.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let rms = crate::rms(tail);
        assert!(peak < 1.0, "{peak}");
        assert!(rms > 0.2 && rms < 0.8, "{rms}");
        // A clipped sine has a higher RMS-to-peak ratio than a clean one (0.707).
        assert!(rms / peak > 0.72, "{}", rms / peak);
    }

    #[test]
    fn nan_becomes_silence() {
        let mut r = Radio::new(SR, 0.5);
        let mut block = [f32::NAN; 480];
        r.process(&mut block);
        assert!(block.iter().all(|s| s.is_finite()));
    }
}
