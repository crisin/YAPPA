//! A safety limiter for the end of a mix: never lets a sample past the ceiling, and lets go
//! smoothly afterwards. Instant attack (no lookahead, so no added latency), exponential
//! release. Meant for "19 people laugh at once", not as a creative effect.

use crate::db_to_gain;

#[derive(Debug, Clone)]
pub struct Limiter {
    ceiling: f32,
    /// Per-sample factor the envelope decays with.
    release: f32,
    envelope: f32,
}

impl Limiter {
    /// `ceiling_db` ≤ 0 dBFS, `release_ms` how fast the gain recovers.
    pub fn new(sample_rate: f32, ceiling_db: f32, release_ms: f32) -> Self {
        let samples = (release_ms / 1000.0 * sample_rate).max(1.0);
        Self {
            ceiling: db_to_gain(ceiling_db.min(0.0)),
            // The envelope falls by 1/e within `release_ms`.
            release: (-1.0 / samples).exp(),
            envelope: 0.0,
        }
    }

    pub fn ceiling(&self) -> f32 {
        self.ceiling
    }

    /// In place, no allocation — safe for the real-time thread. Non-finite input becomes
    /// silence instead of poisoning the envelope.
    pub fn process(&mut self, block: &mut [f32]) {
        for s in block.iter_mut() {
            let x = if s.is_finite() { *s } else { 0.0 };
            self.envelope = (self.envelope * self.release).max(x.abs());
            *s = if self.envelope > self.ceiling {
                x * (self.ceiling / self.envelope)
            } else {
                x
            };
        }
        // A decayed envelope must not drift into the denormal range during silence.
        if self.envelope < 1e-20 {
            self.envelope = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn quiet_signals_pass_unchanged() {
        let mut limiter = Limiter::new(SR, -1.0, 80.0);
        let mut block: Vec<f32> = (0..480).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let before = block.clone();
        limiter.process(&mut block);
        assert_eq!(block, before);
    }

    #[test]
    fn a_loud_burst_is_held_at_the_ceiling_and_released_afterwards() {
        let mut limiter = Limiter::new(SR, -1.0, 80.0);
        let mut loud = [3.0f32; 480];
        limiter.process(&mut loud);
        assert!(loud.iter().all(|s| (*s - limiter.ceiling()).abs() < 1e-6));
        // Right after the burst a quiet signal is still turned down…
        let mut after = [0.5f32; 480];
        limiter.process(&mut after);
        assert!(after[0] < 0.5);
        // …and back to full level a few hundred milliseconds later.
        for _ in 0..40 {
            limiter.process(&mut after);
            after.fill(0.5);
        }
        limiter.process(&mut after);
        assert!((after[479] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn nan_and_infinity_become_silence() {
        let mut limiter = Limiter::new(SR, -1.0, 80.0);
        let mut block = [f32::NAN, f32::INFINITY, 0.2];
        limiter.process(&mut block);
        assert_eq!(block[..2], [0.0, 0.0]);
        assert_eq!(block[2], 0.2);
    }

    proptest! {
        /// Whatever comes in, nothing above the ceiling and nothing non-finite goes out.
        #[test]
        fn never_exceeds_the_ceiling(input in proptest::collection::vec(-40.0f32..40.0, 1..2000)) {
            let mut limiter = Limiter::new(SR, -1.0, 50.0);
            let mut block = input;
            limiter.process(&mut block);
            for s in block {
                prop_assert!(s.is_finite());
                prop_assert!(s.abs() <= limiter.ceiling() + 1e-6);
            }
        }
    }
}
