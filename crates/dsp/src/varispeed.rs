//! Playback at a ratio close to 1.0 that may change at any time (A6, ADR-009): the drift
//! compensation stretches or squeezes one speaker's stream by a few hundred ppm so their
//! playout ring stays at the cushion. Four-point Hermite (Catmull-Rom) interpolation — no
//! kernel table, four multiplies per output — because the ratio never leaves 1 ± 0.5 %:
//! nothing to filter, only the sub-sample position moves. Pull style like
//! [`crate::Resampler`]: `produce` asks for inputs as it needs them. No allocation at all.

/// How far from 1.0 the ratio may go: 0.5 %. Beyond that the pitch shift is audible and the
/// mixer's emergency skip is the better tool.
pub const MAX_DEVIATION: f64 = 0.005;

#[derive(Clone, Debug)]
pub struct Varispeed {
    /// Input samples consumed per output sample.
    ratio: f64,
    /// Position of the next output between `x1` and `x2`, 0..1. Starts at 1: every output
    /// pulls one input from the first one on, so the ring sees the exact demand and nobody
    /// waits for a priming fill.
    frac: f64,
    /// The last four inputs, oldest first. The first input after a reset fills all four, so
    /// the stream starts on its first sample (held for two) instead of fading in from zero.
    x: [f32; 4],
    primed: bool,
}

impl Default for Varispeed {
    fn default() -> Self {
        Self::new()
    }
}

impl Varispeed {
    pub fn new() -> Self {
        Self {
            ratio: 1.0,
            frac: 1.0,
            x: [0.0; 4],
            primed: false,
        }
    }

    /// Sets the input-per-output ratio, clamped to 1 ± [`MAX_DEVIATION`]. Takes effect with
    /// the next output sample; the phase is continuous, so no click.
    pub fn set_ratio(&mut self, ratio: f64) {
        self.ratio = if ratio.is_finite() {
            ratio.clamp(1.0 - MAX_DEVIATION, 1.0 + MAX_DEVIATION)
        } else {
            1.0
        };
    }

    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Forgets the history; the next output starts a fresh lead-in. The ratio stays.
    pub fn reset(&mut self) {
        self.frac = 1.0;
        self.x = [0.0; 4];
        self.primed = false;
    }

    /// Delay of the interpolator itself in input samples (the two look-ahead samples).
    pub const LATENCY_SAMPLES: usize = 2;

    /// Produces up to `count` outputs, pulling inputs with `next` and handing each output to
    /// `sink`. Returns how many outputs were produced; fewer than `count` only when `next`
    /// ran dry — the next call continues seamlessly from the same phase.
    pub fn produce(
        &mut self,
        count: usize,
        mut next: impl FnMut() -> Option<f32>,
        mut sink: impl FnMut(f32),
    ) -> usize {
        for produced in 0..count {
            while self.frac >= 1.0 {
                match next() {
                    Some(sample) => {
                        self.x = if self.primed {
                            [self.x[1], self.x[2], self.x[3], sample]
                        } else {
                            self.primed = true;
                            [sample; 4]
                        };
                        self.frac -= 1.0;
                    }
                    None => return produced,
                }
            }
            sink(self.interpolate());
            self.frac += self.ratio;
        }
        count
    }

    /// Catmull-Rom between `x[1]` and `x[2]`; exact at `frac == 0`, so a ratio of 1.0 passes
    /// the input through bit for bit, two samples late.
    fn interpolate(&self) -> f32 {
        let [x0, x1, x2, x3] = self.x;
        let t = self.frac as f32;
        if t == 0.0 {
            return x1;
        }
        let c1 = 0.5 * (x2 - x0);
        let c2 = x0 - 2.5 * x1 + 2.0 * x2 - 0.5 * x3;
        let c3 = 0.5 * (x3 - x0) + 1.5 * (x1 - x2);
        ((c3 * t + c2) * t + c1) * t + x1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::f32::consts::TAU;

    fn sine(hz: f32, seconds: f32) -> Vec<f32> {
        (0..(48_000.0 * seconds) as usize)
            .map(|i| (i as f32 / 48_000.0 * hz * TAU).sin() * 0.5)
            .collect()
    }

    fn through(ratio: f64, input: &[f32]) -> Vec<f32> {
        let mut vs = Varispeed::new();
        vs.set_ratio(ratio);
        let mut source = input.iter().copied();
        let mut out = Vec::new();
        loop {
            let n = vs.produce(480, || source.next(), |y| out.push(y));
            if n < 480 {
                return out;
            }
        }
    }

    #[test]
    fn unity_ratio_passes_the_input_through_bit_for_bit_two_samples_late() {
        let input: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.37).sin()).collect();
        let out = through(1.0, &input);
        assert_eq!(
            out.len(),
            input.len(),
            "one input per output, from the first on"
        );
        assert_eq!(&out[..2], &[input[0], input[0]], "the first sample, held");
        assert!(out[2..].iter().zip(&input).all(|(a, b)| a == b));
    }

    #[test]
    fn a_stretched_or_squeezed_tone_keeps_its_level_and_changes_length_by_the_ratio() {
        let input = sine(1_000.0, 2.0);
        for ratio in [0.995, 0.9998, 1.0002, 1.005] {
            let out = through(ratio, &input);
            let expected = (input.len() as f64 / ratio) as i64;
            assert!(
                (out.len() as i64 - expected).abs() <= 3,
                "ratio {ratio}: {} outputs, expected ≈ {expected}",
                out.len()
            );
            let s = &out[480..out.len() - 480];
            let rms = (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt();
            assert!((rms - 0.3536).abs() < 0.002, "ratio {ratio}: rms {rms}");
        }
    }

    #[test]
    fn the_ratio_is_clamped_and_a_nan_means_unity() {
        let mut vs = Varispeed::new();
        vs.set_ratio(1.2);
        assert_eq!(vs.ratio(), 1.0 + MAX_DEVIATION);
        vs.set_ratio(0.0);
        assert_eq!(vs.ratio(), 1.0 - MAX_DEVIATION);
        vs.set_ratio(f64::NAN);
        assert_eq!(vs.ratio(), 1.0);
    }

    #[test]
    fn a_dry_source_resumes_where_it_stopped() {
        let input = sine(440.0, 0.1);
        let straight = through(1.003, &input);
        let mut vs = Varispeed::new();
        vs.set_ratio(1.003);
        let mut fed = 0usize;
        let mut out = Vec::new();
        while fed < input.len() {
            let budget = (fed + 97).min(input.len());
            vs.produce(
                480,
                || {
                    (fed < budget).then(|| {
                        fed += 1;
                        input[fed - 1]
                    })
                },
                |y| out.push(y),
            );
        }
        assert_eq!(out.len(), straight.len());
        assert!(out.iter().zip(&straight).all(|(a, b)| a == b));
    }

    #[test]
    fn changing_the_ratio_mid_stream_does_not_click() {
        let input = sine(200.0, 0.5);
        let mut vs = Varispeed::new();
        let mut source = input.iter().copied();
        let mut out = Vec::new();
        for block in 0..40 {
            vs.set_ratio(if block % 2 == 0 { 0.995 } else { 1.005 });
            vs.produce(480, || source.next(), |y| out.push(y));
        }
        // 200 Hz at 48 kHz moves at most 0.013 per sample; a click would be a far bigger step.
        let max_step = out
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0f32, f32::max);
        assert!(max_step < 0.02, "largest step {max_step}");
    }

    proptest! {
        /// Catmull-Rom overshoots by at most 25 %: the output never leaves 1.25 × the input
        /// range and never turns non-finite.
        #[test]
        fn output_stays_bounded(
            ratio in (1.0 - MAX_DEVIATION)..(1.0 + MAX_DEVIATION),
            input in prop::collection::vec(-1.0f32..1.0, 10..1_000),
        ) {
            let out = through(ratio, &input);
            prop_assert!(out.iter().all(|y| y.is_finite() && y.abs() <= 1.25));
        }
    }
}
