//! The harmonizer (ADR-012, "Autotune und Harmonizer" — „Zweite Stimme“ in the app): the
//! dry voice plus a second one that sings a scale interval above or below it. Per block
//! the [`PitchTracker`] finds the pitch, the scale note `interval` steps from the nearest
//! one is the second voice's target, and a [`PitchShifter`] makes it from the dry voice.
//! While the detector hears no pitch (consonants, breath, pauses) the second voice fades
//! out: doubling a consonant at a stale ratio sounds like a lisp. The dry voice is not
//! delayed; the second one lags by the shifter's 15 ms, which reads as a natural doubling.

use crate::autotune::Follower;
use crate::{interval_note, Key, PitchShifter, PitchTracker, Scale};

/// The second voice fades in and out over this time.
const FADE_MS: f32 = 10.0;
/// Chunk the second voice is rendered in: no allocation per block.
const SCRATCH: usize = 512;
/// Scale steps the interval may span: an octave in a seven-note scale. In a shorter scale
/// the octave comes sooner and the steps beyond it are clamped to it, so the end of the
/// knob is always "the octave" and the shifter's ratio bound is never hit.
pub const MAX_INTERVAL: i32 = 7;

#[derive(Debug, Clone)]
pub struct Harmonizer {
    tracker: PitchTracker,
    shifter: PitchShifter,
    follower: Follower,
    scratch: Vec<f32>,
    key: Key,
    scale: Scale,
    interval: i32,
    /// Level of the second voice, 0..=1.
    amount: f32,
    /// The fade gain of the second voice, 0..=1.
    wet: f32,
    fade_step: f32,
    sample_rate: f32,
}

impl Harmonizer {
    /// `interval` in scale steps (2 = a third, 4 = a fifth in a diatonic scale, negative =
    /// below), `amount` the second voice's level, `retune` how fast it follows the voice.
    pub fn new(
        sample_rate: f32,
        key: Key,
        scale: Scale,
        interval: i32,
        amount: f32,
        retune: f32,
    ) -> Self {
        let mut shifter = PitchShifter::new(sample_rate, 0.0);
        shifter.set_ratio(1.0);
        Self {
            tracker: PitchTracker::new(sample_rate),
            shifter,
            follower: Follower::new(retune),
            scratch: vec![0.0; SCRATCH],
            key: key % 12,
            scale,
            interval: interval.clamp(-MAX_INTERVAL, MAX_INTERVAL),
            amount: clamp_unit(amount),
            wet: 0.0,
            fade_step: 1.0 / (FADE_MS / 1000.0 * sample_rate).max(1.0),
            sample_rate,
        }
    }

    pub fn set_key(&mut self, key: Key) {
        self.key = key % 12;
    }

    pub fn set_scale(&mut self, scale: Scale) {
        self.scale = scale;
    }

    pub fn set_interval(&mut self, interval: i32) {
        self.interval = interval.clamp(-MAX_INTERVAL, MAX_INTERVAL);
    }

    pub fn set_amount(&mut self, amount: f32) {
        self.amount = clamp_unit(amount);
    }

    pub fn set_retune(&mut self, retune: f32) {
        self.follower.set_retune(retune);
    }

    pub fn key(&self) -> Key {
        self.key
    }

    pub fn scale(&self) -> Scale {
        self.scale
    }

    pub fn interval(&self) -> i32 {
        self.interval
    }

    pub fn amount(&self) -> f32 {
        self.amount
    }

    pub fn retune(&self) -> f32 {
        self.follower.retune()
    }

    /// The second voice's current pitch ratio to the dry voice.
    pub fn ratio(&self) -> f32 {
        self.follower.ratio()
    }

    /// The dry voice goes out undelayed; only the second voice lags.
    pub fn latency_samples(&self) -> usize {
        0
    }

    /// In place, no allocation.
    pub fn process(&mut self, block: &mut [f32]) {
        let octave = self.scale.degrees().len() as i32;
        let steps = self.interval.clamp(-octave, octave);
        let target = self
            .tracker
            .track(block)
            .map(|p| interval_note(p.hz, self.key, self.scale, steps) / p.hz);
        let ratio = self
            .follower
            .update(target, block.len() as f32 / self.sample_rate);
        self.shifter.set_ratio(ratio);
        let goal = if self.follower.holding() { 1.0 } else { 0.0 };
        let gain = self.amount;
        for chunk in block.chunks_mut(self.scratch.len()) {
            let second = &mut self.scratch[..chunk.len()];
            for (w, x) in second.iter_mut().zip(chunk.iter()) {
                *w = if x.is_finite() { *x } else { 0.0 };
            }
            self.shifter.process(second);
            for (s, w) in chunk.iter_mut().zip(second.iter()) {
                let x = if s.is_finite() { *s } else { 0.0 };
                self.wet += (goal - self.wet).clamp(-self.fade_step, self.fade_step);
                *s = x + w * self.wet * gain;
            }
        }
    }
}

fn clamp_unit(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hz_to_midi, midi_to_hz};
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 480;

    fn voice(hz: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / SR;
                (1..=6)
                    .map(|k| (TAU * hz * k as f32 * t).sin() / k as f32)
                    .sum::<f32>()
                    * 0.3
            })
            .collect()
    }

    fn run(h: &mut Harmonizer, signal: &mut [f32]) {
        for block in signal.chunks_mut(BLOCK) {
            h.process(block);
        }
    }

    /// C4 plus a third in C major: the second voice sits at E4, a ratio of 2^(4/12).
    #[test]
    fn a_third_above_in_c_major() {
        let mut h = Harmonizer::new(SR, 0, Scale::Major, 2, 1.0, 0.0);
        let c4 = midi_to_hz(60.0);
        let mut signal = voice(c4, 24_000);
        let dry = signal.clone();
        run(&mut h, &mut signal);
        let expected = midi_to_hz(64.0) / c4;
        assert!((h.ratio() - expected).abs() < 0.003, "ratio {}", h.ratio());
        // Something was added, and it is louder than the dry voice alone.
        let added: f32 = signal[12_000..]
            .iter()
            .zip(&dry[12_000..])
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / 12_000.0;
        assert!(added > 0.05, "{added}");
        assert!(signal.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn a_fifth_below_and_the_interval_is_bounded() {
        let mut h = Harmonizer::new(SR, 0, Scale::Major, -4, 0.5, 0.0);
        let g4 = midi_to_hz(67.0);
        run(&mut h, &mut voice(g4, 24_000));
        let expected = midi_to_hz(60.0) / g4;
        assert!((h.ratio() - expected).abs() < 0.003, "ratio {}", h.ratio());
        h.set_interval(20);
        assert_eq!(h.interval(), MAX_INTERVAL);
        h.set_interval(-20);
        assert_eq!(h.interval(), -MAX_INTERVAL);
        h.set_amount(f32::NAN);
        assert_eq!(h.amount(), 0.0);
    }

    /// Seven steps in a five-note scale would be an octave and a half, more than the
    /// shifter can do: they are the octave instead — and the octave is reached even for
    /// a voice a quarter tone flat.
    #[test]
    fn beyond_the_octave_of_a_short_scale_is_the_octave() {
        let mut h = Harmonizer::new(SR, 0, Scale::PentatonicMajor, 7, 1.0, 0.0);
        let c4 = midi_to_hz(60.0);
        run(&mut h, &mut voice(c4, 24_000));
        let expected = midi_to_hz(72.0) / c4;
        assert!((h.ratio() - expected).abs() < 0.003, "ratio {}", h.ratio());
        let mut h = Harmonizer::new(SR, 0, Scale::Major, 7, 1.0, 0.0);
        let flat = midi_to_hz(59.6);
        run(&mut h, &mut voice(flat, 24_000));
        let expected = midi_to_hz(72.0) / flat;
        assert!((h.ratio() - expected).abs() < 0.003, "ratio {}", h.ratio());
    }

    #[test]
    fn amount_zero_is_the_dry_voice() {
        let mut h = Harmonizer::new(SR, 0, Scale::Major, 2, 0.0, 0.0);
        let mut signal = voice(220.0, 4_800);
        let dry = signal.clone();
        run(&mut h, &mut signal);
        assert_eq!(signal, dry);
    }

    /// Noise after a voice: the second voice fades out within the hold plus the fade,
    /// so consonants are not doubled.
    #[test]
    fn the_second_voice_fades_out_when_unvoiced() {
        let mut h = Harmonizer::new(SR, 0, Scale::Major, 4, 1.0, 0.0);
        run(&mut h, &mut voice(220.0, 24_000));
        assert!(h.wet > 0.99);
        let mut state = 0x9e37_79b9u32;
        let mut noise: Vec<f32> = (0..9_600)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state as f32 / u32::MAX as f32 - 0.5) * 0.4
            })
            .collect();
        let dry = noise.clone();
        run(&mut h, &mut noise);
        assert_eq!(h.wet, 0.0, "faded out");
        // The last blocks went out as they came in.
        assert_eq!(noise[9_000..], dry[9_000..]);
    }

    #[test]
    fn nan_input_is_tamed_and_the_scale_can_change_live() {
        let mut h = Harmonizer::new(SR, 0, Scale::Major, 2, 1.0, 0.3);
        let mut block = [f32::NAN; BLOCK];
        h.process(&mut block);
        assert!(block.iter().all(|s| s.is_finite()));
        h.set_key(9);
        h.set_scale(Scale::NaturalMinor);
        h.set_retune(0.0);
        assert_eq!(
            (h.key(), h.scale(), h.retune()),
            (9, Scale::NaturalMinor, 0.0)
        );
        // A minor, A3 plus a third is C4: a minor third.
        let a3 = midi_to_hz(57.0);
        run(&mut h, &mut voice(a3, 24_000));
        let expected = midi_to_hz(60.0) / a3;
        assert!((h.ratio() - expected).abs() < 0.003, "ratio {}", h.ratio());
        assert_eq!(h.latency_samples(), 0);
        assert!((hz_to_midi(a3 * h.ratio()) - 60.0).abs() < 0.05);
    }
}
