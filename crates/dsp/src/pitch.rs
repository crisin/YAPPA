//! A pitch shifter for voices (ADR-012, "Stimme höher oder tiefer"): the classic delay-line
//! shifter with two read heads. Each head slides through a short window of the past at a
//! speed that differs from the write speed by the pitch ratio, so what it reads is
//! resampled — higher or lower. A head that reaches the end of the window jumps back; two
//! heads half a window apart, crossfaded with a sine/cosine pair (constant power), hide the
//! jump. Cheap (one linear interpolation per head and sample), no FFT, no lookahead; the
//! price is a light flutter at the window rate, a comb colouring on steady tones (the two
//! heads add in or out of phase) and a delay of half the window.
//!
//! Not a studio algorithm: it is for the gaming evening, not for singing.

use std::f32::consts::PI;

/// Window the heads slide through: 30 ms. Shorter flutters faster and smears less, longer
/// adds delay — this is the usual compromise for speech.
const WINDOW_MS: f32 = 30.0;
/// Semitones the UI may ask for; beyond that the window rate becomes a clear tremolo.
pub const MAX_SEMITONES: f32 = 12.0;
/// The bound for a tracker-steered ratio: an octave plus the half scale step a voice may
/// sit off its nearest note (up to 1.5 semitones), so an octave second voice reaches its
/// note even when the singer is flat or sharp.
pub const MIN_RATIO: f32 = 0.46;
pub const MAX_RATIO: f32 = 2.18;

#[derive(Debug, Clone)]
pub struct PitchShifter {
    buf: Vec<f32>,
    write: usize,
    /// Window length in samples.
    window: f32,
    /// Head A's position in the window, 0..1 (head B is half a window further).
    phase: f32,
    /// Phase advance per sample: `(1 - ratio) / window`. Zero at unity.
    step: f32,
    semitones: f32,
    ratio: f32,
    /// Set by [`set_semitones`](Self::set_semitones) at 0: the signal passes untouched.
    /// [`set_ratio`](Self::set_ratio) never bypasses, so a ratio that crosses 1.0 while a
    /// tracker steers it stays continuous instead of jumping between the dry signal and
    /// the delayed read heads.
    bypass: bool,
}

impl PitchShifter {
    pub fn new(sample_rate: f32, semitones: f32) -> Self {
        let window = (WINDOW_MS / 1000.0 * sample_rate).max(16.0);
        let mut s = Self {
            // One extra sample for the interpolation, one for the slack between write and
            // the oldest read.
            buf: vec![0.0; window as usize + 2],
            write: 0,
            window,
            phase: 0.0,
            step: 0.0,
            semitones: 0.0,
            ratio: 1.0,
            bypass: true,
        };
        s.set_semitones(semitones);
        s
    }

    /// Positive = higher. Clamped to ±[`MAX_SEMITONES`]. Takes effect with the next sample;
    /// the heads keep their place, so a slider drag does not click.
    pub fn set_semitones(&mut self, semitones: f32) {
        let st = if semitones.is_finite() {
            semitones.clamp(-MAX_SEMITONES, MAX_SEMITONES)
        } else {
            0.0
        };
        self.semitones = st;
        self.ratio = 2f32.powf(st / 12.0);
        self.step = (1.0 - self.ratio) / self.window;
        self.bypass = st == 0.0;
    }

    /// The pitch ratio directly (2.0 = an octave up), for a tracker that steers the shifter
    /// continuously (autotune, harmonizer). Clamped to [`MIN_RATIO`]..=[`MAX_RATIO`];
    /// NaN is unity. Unlike [`set_semitones`](Self::set_semitones) at 0, unity here still
    /// runs through the read heads — see [`Self::bypass`].
    pub fn set_ratio(&mut self, ratio: f32) {
        let r = if ratio.is_finite() {
            ratio.clamp(MIN_RATIO, MAX_RATIO)
        } else {
            1.0
        };
        self.ratio = r;
        self.semitones = 12.0 * r.log2();
        self.step = (1.0 - r) / self.window;
        self.bypass = false;
    }

    pub fn semitones(&self) -> f32 {
        self.semitones
    }

    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// What the shifter delays the voice by on average: half the window.
    pub fn latency_samples(&self) -> usize {
        (self.window / 2.0) as usize
    }

    /// A read `delay` samples (fractional) behind the write position.
    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let d = delay.clamp(0.0, (len - 2) as f32);
        let whole = d as usize;
        let frac = d - whole as f32;
        let i0 = (self.write + len - 1 - whole) % len;
        let i1 = (i0 + len - 1) % len;
        self.buf[i0] + (self.buf[i1] - self.buf[i0]) * frac
    }

    /// In place, no allocation. At unity the signal passes through untouched — but the
    /// delay line is still fed, so a slider leaving 0 does not play 15 ms of stale audio
    /// first (realtime review 2026-11-02).
    pub fn process(&mut self, block: &mut [f32]) {
        let len = self.buf.len();
        for s in block.iter_mut() {
            let x = if s.is_finite() { *s } else { 0.0 };
            self.buf[self.write] = x;
            self.write = (self.write + 1) % len;
            if self.bypass {
                continue;
            }
            let pa = self.phase;
            let pb = (pa + 0.5).rem_euclid(1.0);
            let a = self.read(pa * self.window);
            let b = self.read(pb * self.window);
            // sin and cos of the same angle: the two gains always sum to unit power.
            let (ga, gb) = (PI * pa).sin_cos();
            *s = a * ga + b * gb.abs();
            self.phase = (self.phase + self.step).rem_euclid(1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Dominant frequency of a block by counting zero crossings — rough, but enough to
    /// tell an octave.
    fn zero_crossings_hz(signal: &[f32]) -> f32 {
        let crossings = signal
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        crossings as f32 / 2.0 * SR / signal.len() as f32
    }

    fn tone(hz: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * PI * hz * i as f32 / SR).sin() * 0.5)
            .collect()
    }

    #[test]
    fn unity_passes_the_signal_through_untouched() {
        let mut ps = PitchShifter::new(SR, 0.0);
        let mut block = tone(440.0, 480);
        let before = block.clone();
        ps.process(&mut block);
        assert_eq!(block, before);
        assert_eq!(ps.latency_samples(), 720);
    }

    #[test]
    fn an_octave_down_halves_the_frequency() {
        let mut ps = PitchShifter::new(SR, -12.0);
        let mut signal = tone(880.0, 48_000);
        ps.process(&mut signal);
        // Skip the first window, where the buffer is still filling.
        let hz = zero_crossings_hz(&signal[4_800..]);
        assert!((hz - 440.0).abs() < 30.0, "{hz} Hz");
    }

    #[test]
    fn seven_semitones_up_is_a_fifth() {
        let mut ps = PitchShifter::new(SR, 7.0);
        let mut signal = tone(300.0, 48_000);
        ps.process(&mut signal);
        let hz = zero_crossings_hz(&signal[4_800..]);
        let expected = 300.0 * 2f32.powf(7.0 / 12.0);
        assert!((hz - expected).abs() < 30.0, "{hz} Hz, expected {expected}");
    }

    /// Two heads half a window apart add a steady tone in phase (200 Hz: three periods
    /// per 15 ms) or out of phase (233 Hz): +2 / −4 dB — the comb colouring every
    /// delay-line shifter has on tones. A voice is never steady for 30 ms, so it averages
    /// out; this pins the spread so it does not grow unnoticed.
    #[test]
    fn the_level_stays_within_a_few_db_on_tones() {
        let input_rms = 0.5 / 2f32.sqrt();
        for hz in [200.0, 233.0, 1_000.0] {
            let mut ps = PitchShifter::new(SR, -5.0);
            let mut signal = tone(hz, 48_000);
            ps.process(&mut signal);
            let rms = crate::rms(&signal[4_800..]);
            let ratio = rms / input_rms;
            assert!(
                (0.55..=1.35).contains(&ratio),
                "{hz} Hz: {rms} vs {input_rms}"
            );
        }
    }

    /// Leaving unity must read fresh audio, not what the line held before the slider sat
    /// at 0 (or zeros on a fresh instance).
    #[test]
    fn the_delay_line_is_warm_when_leaving_unity() {
        let mut ps = PitchShifter::new(SR, 0.0);
        let mut warm = tone(300.0, 4_800);
        ps.process(&mut warm);
        ps.set_semitones(-3.0);
        let mut block = tone(300.0, 480);
        ps.process(&mut block);
        assert!(
            crate::rms(&block) > 0.2,
            "the first block after unity came from an empty line: {}",
            crate::rms(&block)
        );
    }

    /// A ratio of exactly 1.0 from `set_ratio` keeps reading through the heads: a tracker
    /// that drifts through unity must not switch between the dry signal and a 15 ms delayed
    /// one.
    #[test]
    fn a_ratio_of_one_is_not_a_bypass() {
        let mut ps = PitchShifter::new(SR, 0.0);
        ps.set_ratio(1.0);
        assert_eq!(ps.ratio(), 1.0);
        assert_eq!(ps.semitones(), 0.0);
        let mut block = tone(440.0, 4_800);
        let before = block.clone();
        ps.process(&mut block);
        assert_ne!(block[2_400..], before[2_400..], "the heads are in the path");
        ps.set_ratio(1.5);
        assert!((ps.semitones() - 7.02).abs() < 0.01);
        ps.set_ratio(f32::NAN);
        assert_eq!(ps.ratio(), 1.0);
        ps.set_ratio(9.0);
        assert_eq!(ps.ratio(), MAX_RATIO);
        ps.set_semitones(0.0);
        let mut block = tone(440.0, 480);
        let before = block.clone();
        ps.process(&mut block);
        assert_eq!(block, before, "semitones 0 is the bypass again");
    }

    #[test]
    fn out_of_range_and_nan_are_tamed() {
        let mut ps = PitchShifter::new(SR, 40.0);
        assert_eq!(ps.semitones(), MAX_SEMITONES);
        ps.set_semitones(f32::NAN);
        assert_eq!(ps.semitones(), 0.0);
        ps.set_semitones(-3.0);
        let mut block = [f32::NAN; 480];
        ps.process(&mut block);
        assert!(block.iter().all(|s| s.is_finite()));
    }
}
