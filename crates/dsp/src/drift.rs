//! The control loop of the drift compensation (A6, ADR-009). A remote speaker's clock and
//! our output device never run at exactly the same rate: a few hundred ppm apart, their
//! playout ring gains or loses a 10 ms frame every minute or so, and after a few minutes it
//! hit the high-water mark and was skipped by ≈ 80 ms (ADR-003 known gap). The controller
//! watches the ring's fill level at the start of every callback, averages it over about
//! 1.5 s (frames arrive in 10 ms lumps, the callback takes its own sizes) and turns the
//! distance to the cushion into a playback ratio around 1.0 that [`crate::Varispeed`]
//! applies: too full → play a little faster, too empty → a little slower. PI: the
//! proportional part drains a lump within seconds, the integral part learns the drift
//! itself so the ring sits exactly at the cushion in the long run. Pure arithmetic, no
//! allocation.

use crate::varispeed::MAX_DEVIATION;

/// The most the learned drift (integral part) may contribute to the ratio: 2000 ppm.
const INTEGRAL_SHARE: f64 = 0.002;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DriftParams {
    /// Time constant of the fill-level average, seconds.
    pub window_s: f32,
    /// Over how many seconds the proportional part would work off a fill-level error:
    /// `ratio - 1 = error / (sample_rate * horizon_s)`. Shorter is faster but closer to
    /// ringing against the window. The integral gain follows from it (critically damped
    /// PI, `k_i = k_p² / 4`).
    pub horizon_s: f32,
}

impl Default for DriftParams {
    fn default() -> Self {
        Self {
            window_s: 1.5,
            horizon_s: 3.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DriftController {
    sample_rate: f32,
    params: DriftParams,
    /// Averaged fill level, samples. `None` until the first measurement.
    average: Option<f32>,
    /// Integrated error, sample-seconds, clamped so its share of the ratio stays in range.
    integral: f64,
    ratio: f64,
}

impl DriftController {
    pub fn new(sample_rate: f32, params: DriftParams) -> Self {
        Self {
            sample_rate,
            params,
            average: None,
            integral: 0.0,
            ratio: 1.0,
        }
    }

    /// Forgets average and learned drift; the next `step` starts from the fill level it is
    /// given, at a ratio of 1.0. For a speaker who (re)starts, or after an emergency skip.
    pub fn reset(&mut self) {
        self.average = None;
        self.integral = 0.0;
        self.ratio = 1.0;
    }

    /// One callback: `fill` samples were queued when it began, `target` is where the ring
    /// should sit, `consumed` output samples are about to be played. Returns the ratio
    /// (inputs per output) for this callback, within 1 ± [`MAX_DEVIATION`].
    pub fn step(&mut self, fill: usize, target: usize, consumed: usize) -> f64 {
        let fill = fill as f32;
        let dt = consumed as f32 / self.sample_rate;
        let average = match self.average {
            None => fill,
            Some(previous) => {
                let alpha = (dt / self.params.window_s).clamp(0.0, 1.0);
                previous + alpha * (fill - previous)
            }
        };
        self.average = Some(average);
        let error = f64::from(average - target as f32);
        let sr = f64::from(self.sample_rate);
        let horizon = f64::from(self.params.horizon_s);
        // k_p in 1/s is 1/horizon; k_i = k_p² / 4; both divided by the rate to turn samples
        // of error into a dimensionless ratio.
        let k_p = 1.0 / (sr * horizon);
        let k_i = 1.0 / (4.0 * sr * horizon * horizon);
        // Anti-windup: while the proportional part alone saturates the ratio (a backlog of
        // several frames), the integral does not learn from it — otherwise it would carry
        // the ring far past the cushion once the backlog is gone. Its own share is capped
        // at ±2000 ppm, more drift than any sound card has.
        let proportional = k_p * error;
        if proportional.abs() < MAX_DEVIATION {
            let cap = INTEGRAL_SHARE / k_i;
            self.integral = (self.integral + error * f64::from(dt)).clamp(-cap, cap);
        }
        let deviation = proportional + k_i * self.integral;
        self.ratio = 1.0 + deviation.clamp(-MAX_DEVIATION, MAX_DEVIATION);
        self.ratio
    }

    /// The ratio of the last `step`, 1.0 after a reset.
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// The averaged fill level in samples, 0 before the first `step`.
    pub fn fill(&self) -> f32 {
        self.average.unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 480;

    struct Trace {
        /// Fill at callback start and the ratio chosen, per callback.
        steps: Vec<(f64, f64)>,
        cushion: usize,
        per_second: usize,
    }

    impl Trace {
        fn error(&self, t: &(f64, f64)) -> f64 {
            t.0 - self.cushion as f64
        }

        fn last(&self, seconds: usize) -> &[(f64, f64)] {
            &self.steps[self.steps.len() - seconds * self.per_second..]
        }

        fn mean_error(&self, seconds: usize) -> f64 {
            let s = self.last(seconds);
            s.iter().map(|t| self.error(t)).sum::<f64>() / s.len() as f64
        }

        fn mean_ppm(&self, seconds: usize) -> f64 {
            let s = self.last(seconds);
            (s.iter().map(|t| t.1).sum::<f64>() / s.len() as f64 - 1.0) * 1e6
        }
    }

    /// A speaker's ring under drift: the sender's clock runs `drift_ppm` faster than ours,
    /// the callback takes `callback` samples at the controller's ratio. `lumpy` delivers
    /// whole 10 ms frames like the network does, otherwise the samples flow continuously.
    fn simulate(drift_ppm: f64, callback: usize, seconds: usize, lumpy: bool) -> Trace {
        // The mixer's cushion: at least two frames, always one frame more than a callback,
        // targeted at the middle of the start window.
        let cushion = (2 * BLOCK).max(callback + BLOCK) + BLOCK / 2;
        let mut c = DriftController::new(SR, DriftParams::default());
        let mut fill = cushion as f64;
        let mut arrived = 0.0f64;
        let mut steps = Vec::new();
        let per_second = (SR / callback as f32) as usize;
        for _ in 0..seconds * per_second {
            arrived += callback as f64 * (1.0 + drift_ppm * 1e-6);
            if lumpy {
                while arrived >= BLOCK as f64 {
                    arrived -= BLOCK as f64;
                    fill += BLOCK as f64;
                }
            } else {
                fill += arrived;
                arrived = 0.0;
            }
            let ratio = c.step(fill.round() as usize, cushion, callback);
            steps.push((fill, ratio));
            let take = callback as f64 * ratio;
            assert!(fill >= take, "ran dry after {} callbacks", steps.len());
            fill -= take;
        }
        Trace {
            steps,
            cushion,
            per_second,
        }
    }

    #[test]
    fn without_drift_the_ratio_settles_at_unity_and_the_ring_at_the_cushion() {
        let trace = simulate(0.0, BLOCK, 60, true);
        let (fill, ratio) = trace.steps.last().copied().unwrap();
        assert!(
            ((ratio - 1.0) * 1e6).abs() < 5.0,
            "{:+.1} ppm",
            (ratio - 1.0) * 1e6
        );
        assert!((fill - trace.cushion as f64).abs() < 2.0, "{fill}");
    }

    /// A smooth sender shows the loop itself: it learns a 200 ppm drift within 30 s and
    /// then holds the ring at the cushion.
    #[test]
    fn a_smooth_drift_is_learned_exactly() {
        for ppm in [200.0, -200.0] {
            let trace = simulate(ppm, BLOCK, 60, false);
            assert!(
                (trace.mean_ppm(10) - ppm).abs() < 2.0,
                "settled at {:+.1} ppm for {ppm:+.0}",
                trace.mean_ppm(10)
            );
            assert!(
                trace.mean_error(10).abs() < 2.0,
                "{:+.1} samples off the cushion",
                trace.mean_error(10)
            );
        }
    }

    /// The real thing: frames arrive whole, so a 200 ppm drift shows up as one extra frame
    /// every 50 s. Without compensation the ring would be 2880 samples (6 frames) over after
    /// 5 minutes and skipped by the high-water mark; with it the ring stays within a frame of
    /// the cushion and nothing is skipped.
    #[test]
    fn a_fast_sender_is_played_faster_and_the_ring_stays_at_the_cushion() {
        let trace = simulate(200.0, BLOCK, 300, true);
        let peak = trace
            .steps
            .iter()
            .map(|t| trace.error(t))
            .fold(0.0, f64::max);
        assert!(peak < BLOCK as f64 + 60.0, "peak {peak} over the cushion");
        assert!(
            trace.mean_error(100).abs() < 100.0,
            "{:+.0} samples off on average",
            trace.mean_error(100)
        );
        // One frame per 50 s quantises the mean over 100 s to ±100 ppm.
        assert!(
            (trace.mean_ppm(100) - 200.0).abs() < 110.0,
            "played at {:+.0} ppm",
            trace.mean_ppm(100)
        );
    }

    #[test]
    fn reset_starts_from_the_given_fill_at_unity() {
        let mut c = DriftController::new(SR, DriftParams::default());
        for _ in 0..200 {
            c.step(6 * BLOCK, 3 * BLOCK, BLOCK);
        }
        assert!(c.ratio() > 1.001);
        c.reset();
        assert_eq!(c.ratio(), 1.0);
        assert_eq!(c.fill(), 0.0);
        let ratio = c.step(3 * BLOCK, 3 * BLOCK, BLOCK);
        assert_eq!(ratio, 1.0, "no memory of the old backlog");
        assert_eq!(c.fill(), 3.0 * BLOCK as f32);
    }

    #[test]
    fn a_huge_error_is_clamped_to_half_a_percent_and_the_integral_does_not_wind_up() {
        let mut c = DriftController::new(SR, DriftParams::default());
        for _ in 0..10_000 {
            assert_eq!(c.step(100 * BLOCK, 3 * BLOCK, BLOCK), 1.0 + MAX_DEVIATION);
        }
        // Back at the cushion (held there by force — open loop, so the integral cannot
        // unwind) the proportional part lets go as fast as the 1.5 s average forgets, and
        // what the integral picked up on the way down stays within its cap.
        let mut back = 0;
        while c.step(3 * BLOCK, 3 * BLOCK, BLOCK) > 1.0 + INTEGRAL_SHARE {
            back += 1;
            assert!(back < 1_200, "still over the integral's cap after 12 s");
        }
        for _ in 0..3_000 {
            let ratio = c.step(3 * BLOCK, 3 * BLOCK, BLOCK);
            assert!(ratio <= 1.0 + INTEGRAL_SHARE, "{ratio}");
        }
        c.reset();
        assert_eq!(c.step(0, 3 * BLOCK, BLOCK), 1.0 - MAX_DEVIATION);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        /// Any drift within ±200 ppm, any callback size the devices bring, frames in lumps:
        /// over five minutes the ratio never leaves its range, the ring never reaches the
        /// skip threshold nor runs dry, the fill does not build up, and the mean error ends
        /// near zero.
        #[test]
        fn converges_without_ringing(
            drift_ppm in -200.0f64..200.0,
            callback in prop::sample::select(vec![96usize, 240, 480, 1056, 2048]),
        ) {
            let trace = simulate(drift_ppm, callback, 300, true);
            prop_assert!(trace.steps.iter().all(|t| (1.0 - MAX_DEVIATION..=1.0 + MAX_DEVIATION).contains(&t.1)));
            let worst = trace.steps.iter().map(|t| trace.error(t).abs()).fold(0.0, f64::max);
            // Frames arrive in lumps between two callbacks: the fill wobbles by a frame, by
            // more than a frame when a callback is longer than one.
            let wobble = BLOCK.max(callback) as f64;
            prop_assert!(worst < wobble + BLOCK as f64, "worst {worst} off the cushion (cushion {})", trace.cushion);
            prop_assert!(trace.mean_error(100).abs() < wobble / 2.0 + 100.0, "{:+.0} samples off on average", trace.mean_error(100));
            prop_assert!((trace.mean_ppm(100) - drift_ppm).abs() < 110.0, "played at {:+.0} ppm for {drift_ppm:+.0}", trace.mean_ppm(100));
        }
    }
}
