//! Pitch detection for the autotune and the harmonizer (ADR-012): YIN (de Cheveigné &
//! Kawahara 2002) over the last ≈ 32 ms of the voice, once per block, looking back only —
//! it adds no latency. Steps: the difference function `d(τ) = Σ (x[j] − x[j+τ])²`, its
//! cumulative mean normalised form (≈ 1 where the signal does not repeat, towards 0 at the
//! period), the first dip under a threshold rather than the deepest one (the deepest also
//! sits at every multiple of the period — the octave-down error), then a parabola through
//! the dip's three points for a sub-sample period.
//!
//! The detector works on a copy decimated 2:1 to 24 kHz (low-passed at 5 kHz first): the
//! voice's pitch is below 600 Hz and the search is `O(window · periods)`, so this quarters
//! the cost without losing precision — the parabola gives the period to a few hundredths
//! of a sample either way.

use crate::{Biquad, BiquadKind};

/// Lowest and highest pitch looked for: a deep male voice to a high female one.
pub const MIN_HZ: f32 = 70.0;
pub const MAX_HZ: f32 = 600.0;
const DECIMATION: usize = 2;
/// Anti-alias filter before the decimation; also takes the formant energy out, which
/// helps the detector as much as it hurts nothing.
const LOWPASS_HZ: f32 = 5_000.0;
/// History the detector looks back over. The integration window is this minus the
/// longest period (14.3 ms at 70 Hz): ≈ 18 ms, more than one period at the lowest pitch.
const HISTORY_MS: f32 = 32.0;
/// A dip under this counts as the period (YIN's absolute threshold); above it the block is
/// unvoiced: consonants, breath, silence, noise.
const THRESHOLD: f32 = 0.15;
/// Below this RMS the window is silence (−60 dBFS), whatever the normalised difference says.
const SILENCE_RMS: f32 = 1e-3;
/// Chunk the input runs through the anti-alias filter in: no allocation per block.
const SCRATCH: usize = 512;

/// A detected pitch: the fundamental and how sure the detector is (1 − the normalised
/// difference at the period: 1 for a clean tone, ≈ 0.85 at the threshold).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pitch {
    pub hz: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct PitchTracker {
    lowpass: Biquad,
    scratch: Vec<f32>,
    /// Decimation phase: every `DECIMATION`-th filtered sample goes into the ring.
    phase: usize,
    /// The decimated history.
    ring: Vec<f32>,
    write: usize,
    /// The ring unrolled oldest-first for the search.
    window: Vec<f32>,
    /// Difference function, then (in place) its cumulative mean normalised form.
    d: Vec<f32>,
    tau_min: usize,
    tau_max: usize,
    /// Integration length of the difference function.
    n: usize,
    rate: f32,
}

impl PitchTracker {
    pub fn new(sample_rate: f32) -> Self {
        let rate = sample_rate / DECIMATION as f32;
        let tau_min = ((rate / MAX_HZ).floor() as usize).max(2);
        let tau_max = ((rate / MIN_HZ).ceil() as usize).max(tau_min + 2);
        let history = ((HISTORY_MS / 1000.0 * rate) as usize).max(tau_max + 64);
        Self {
            lowpass: Biquad::new(BiquadKind::LowPass, sample_rate, LOWPASS_HZ, 0.707),
            scratch: vec![0.0; SCRATCH],
            phase: 0,
            ring: vec![0.0; history],
            write: 0,
            window: vec![0.0; history],
            d: vec![0.0; tau_max + 2],
            tau_min,
            tau_max,
            n: history - tau_max,
            rate,
        }
    }

    /// Feed one block and detect the pitch of the window it completes. `None` when the
    /// window is unvoiced or silent. No allocation.
    pub fn track(&mut self, block: &[f32]) -> Option<Pitch> {
        self.push(block);
        self.detect()
    }

    fn push(&mut self, block: &[f32]) {
        let len = self.ring.len();
        for chunk in block.chunks(self.scratch.len()) {
            let scratch = &mut self.scratch[..chunk.len()];
            for (s, x) in scratch.iter_mut().zip(chunk) {
                *s = if x.is_finite() { *x } else { 0.0 };
            }
            self.lowpass.process(scratch);
            for &y in scratch.iter() {
                if self.phase == 0 {
                    self.ring[self.write] = y;
                    self.write = (self.write + 1) % len;
                }
                self.phase = (self.phase + 1) % DECIMATION;
            }
        }
    }

    fn detect(&mut self) -> Option<Pitch> {
        // Unroll the ring, oldest first.
        let len = self.ring.len();
        let (newer, older) = self.ring.split_at(self.write);
        self.window[..older.len()].copy_from_slice(older);
        self.window[older.len()..].copy_from_slice(newer);
        debug_assert_eq!(older.len() + newer.len(), len);

        if crate::rms(&self.window) < SILENCE_RMS {
            return None;
        }

        // The difference function. Eight accumulators let the compiler vectorise the sum
        // (a single f32 accumulator must stay sequential).
        let n = self.n;
        self.d[0] = 0.0;
        for tau in 1..=self.tau_max {
            let a = &self.window[..n];
            let b = &self.window[tau..tau + n];
            let mut acc = [0f32; 8];
            let (ca, ra) = a.as_chunks::<8>();
            let (cb, rb) = b.as_chunks::<8>();
            for (x, y) in ca.iter().zip(cb) {
                for k in 0..8 {
                    let e = x[k] - y[k];
                    acc[k] += e * e;
                }
            }
            let mut sum: f32 = acc.iter().sum();
            for (x, y) in ra.iter().zip(rb) {
                let e = x - y;
                sum += e * e;
            }
            self.d[tau] = sum;
        }

        // Cumulative mean normalised difference: d'(τ) = d(τ) · τ / Σ_{1..τ} d.
        let mut running = 0.0f32;
        self.d[0] = 1.0;
        for tau in 1..=self.tau_max {
            running += self.d[tau];
            self.d[tau] = if running > 0.0 {
                self.d[tau] * tau as f32 / running
            } else {
                1.0
            };
        }

        // The first dip under the threshold, followed to its bottom.
        let mut tau = self.tau_min;
        let found = loop {
            if tau > self.tau_max {
                return None;
            }
            if self.d[tau] < THRESHOLD {
                while tau < self.tau_max && self.d[tau + 1] < self.d[tau] {
                    tau += 1;
                }
                break tau;
            }
            tau += 1;
        };

        // A parabola through the three points around the dip gives a fractional period.
        let s0 = self.d[found - 1];
        let s1 = self.d[found];
        let s2 = if found < self.tau_max {
            self.d[found + 1]
        } else {
            s1
        };
        let denom = s0 - 2.0 * s1 + s2;
        let shift = if denom.abs() > 1e-12 {
            ((s0 - s2) / (2.0 * denom)).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let period = found as f32 + shift;
        let hz = self.rate / period;
        if !hz.is_finite() || !(MIN_HZ * 0.9..=MAX_HZ * 1.1).contains(&hz) {
            return None;
        }
        Some(Pitch {
            hz,
            confidence: (1.0 - s1).clamp(0.0, 1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 480;

    fn sawtooth(hz: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = (i as f32 * hz / SR).fract();
                (2.0 * t - 1.0) * 0.4
            })
            .collect()
    }

    /// A voice-like tone: fundamental plus harmonics falling off 6 dB per octave.
    fn voice(hz: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / SR;
                (1..=8)
                    .map(|k| (TAU * hz * k as f32 * t).sin() / k as f32)
                    .sum::<f32>()
                    * 0.3
            })
            .collect()
    }

    fn noise(n: usize) -> Vec<f32> {
        let mut state = 0x2545_f491u32;
        (0..n)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state as f32 / u32::MAX as f32) - 0.5
            })
            .collect()
    }

    /// Feed a signal block by block and return the detection after the last block.
    fn last(tracker: &mut PitchTracker, signal: &[f32]) -> Option<Pitch> {
        let mut out = None;
        for block in signal.chunks(BLOCK) {
            out = tracker.track(block);
        }
        out
    }

    #[test]
    fn a_220_hz_sawtooth_is_found_within_a_hertz() {
        let mut t = PitchTracker::new(SR);
        let p = last(&mut t, &sawtooth(220.0, 4_800)).expect("voiced");
        assert!((p.hz - 220.0).abs() < 1.0, "{} Hz", p.hz);
        assert!(p.confidence > 0.9, "{}", p.confidence);
    }

    #[test]
    fn the_whole_voice_range_is_found() {
        for hz in [75.0, 110.0, 150.0, 262.0, 440.0, 580.0] {
            let mut t = PitchTracker::new(SR);
            let p = last(&mut t, &voice(hz, 4_800)).unwrap_or_else(|| panic!("{hz} Hz unvoiced"));
            assert!(
                (p.hz - hz).abs() < hz * 0.01,
                "{hz} Hz detected as {}",
                p.hz
            );
        }
    }

    /// The deepest dip of a 150 Hz voice sits at its period and at twice the period; the
    /// first dip is the right one.
    #[test]
    fn harmonics_do_not_pull_the_detection_an_octave_off() {
        let mut t = PitchTracker::new(SR);
        let p = last(&mut t, &voice(150.0, 4_800)).expect("voiced");
        assert!((p.hz - 150.0).abs() < 2.0, "{} Hz", p.hz);
    }

    #[test]
    fn noise_and_silence_are_unvoiced() {
        let mut t = PitchTracker::new(SR);
        assert_eq!(last(&mut t, &noise(9_600)), None);
        assert_eq!(last(&mut t, &vec![0.0; 4_800]), None);
        assert_eq!(last(&mut t, &vec![1e-4; 4_800]), None, "DC under the floor");
    }

    #[test]
    fn a_nan_block_does_not_poison_the_state() {
        let mut t = PitchTracker::new(SR);
        assert_eq!(t.track(&[f32::NAN; BLOCK]), None);
        t.track(&[f32::INFINITY; BLOCK]);
        let p = last(&mut t, &sawtooth(220.0, 4_800)).expect("voiced again");
        assert!((p.hz - 220.0).abs() < 1.0, "{} Hz", p.hz);
    }

    #[test]
    fn odd_block_sizes_and_other_rates_work() {
        let mut t = PitchTracker::new(44_100.0);
        let signal: Vec<f32> = (0..8_820)
            .map(|i| (TAU * 200.0 * i as f32 / 44_100.0).sin() * 0.5)
            .collect();
        let mut out = None;
        for block in signal.chunks(333) {
            out = t.track(block);
        }
        let p = out.expect("voiced");
        assert!((p.hz - 200.0).abs() < 1.0, "{} Hz", p.hz);
    }
}
