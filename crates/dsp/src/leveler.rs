//! Loudness levelling at the receiver (ADR-009, A4): one per speaker, so everybody arrives
//! at about the same level. Slow and gentle by design — it corrects "Ben's mic is quiet",
//! not syllables: the level is measured over ≈ 400 ms, the gain moves a few dB per second
//! and stays within ±12 dB. Blocks below a noise floor are neither measured nor acted on,
//! so a pause or a quiet room is not pumped up to the target. The manual per-person volume
//! is an offset on top.
//!
//! The gain is derived from the signal *before* the gain (open loop), so there is nothing
//! that can oscillate. One block of lag between measurement and gain does not matter at
//! these time constants, which lets the mixer measure and apply in one pass.

use crate::{db_to_gain, gain_to_db};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelerParams {
    /// Where every voice should sit, dBFS (RMS).
    pub target_db: f32,
    /// The gain never leaves ±this, dB.
    pub max_gain_db: f32,
    /// Time constant of the level measurement, ms.
    pub window_ms: f32,
    /// How fast a quiet voice is pulled up, dB per second.
    pub rise_db_per_s: f32,
    /// How fast a loud voice is pulled down, dB per second.
    pub fall_db_per_s: f32,
    /// A block under this level (RMS, dBFS) is a pause: not measured, gain holds still.
    pub floor_db: f32,
}

impl Default for LevelerParams {
    fn default() -> Self {
        Self {
            target_db: -20.0,
            max_gain_db: 12.0,
            window_ms: 400.0,
            rise_db_per_s: 3.0,
            fall_db_per_s: 6.0,
            floor_db: -50.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Leveler {
    params: LevelerParams,
    sample_rate: f32,
    /// Smoothed mean square of the input, before the gain.
    mean_square: f32,
    gain_db: f32,
    enabled: bool,
}

impl Leveler {
    pub fn new(sample_rate: f32, params: LevelerParams) -> Self {
        Self {
            params: LevelerParams {
                max_gain_db: params.max_gain_db.max(0.0),
                window_ms: params.window_ms.max(1.0),
                rise_db_per_s: params.rise_db_per_s.max(0.0),
                fall_db_per_s: params.fall_db_per_s.max(0.0),
                ..params
            },
            sample_rate: sample_rate.max(1.0),
            mean_square: 0.0,
            gain_db: 0.0,
            enabled: true,
        }
    }

    pub fn params(&self) -> LevelerParams {
        self.params
    }

    /// Switched off, the gain slides back to 0 dB at the fall/rise rate — no click when
    /// the user toggles it during a call.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Forget the speaker: unity gain, empty measurement (a new person on this slot).
    pub fn reset(&mut self) {
        self.mean_square = 0.0;
        self.gain_db = 0.0;
    }

    /// The gain applied right now, dB — for the diagnosis.
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    /// The smoothed input level, dBFS.
    pub fn level_db(&self) -> f32 {
        gain_to_db(self.mean_square.sqrt())
    }

    /// One block of `samples` samples with the given mean square (before the gain) went by:
    /// update the measurement and move the gain one step. Returns the linear gain for the
    /// *next* block. No allocation — safe for the real-time thread.
    pub fn step(&mut self, mean_square: f32, samples: usize) -> f32 {
        if samples == 0 {
            return db_to_gain(self.gain_db);
        }
        let mean_square = if mean_square.is_finite() {
            mean_square.max(0.0)
        } else {
            0.0
        };
        let seconds = samples as f32 / self.sample_rate;
        let p = &self.params;
        // Speech-gated measurement: a block under the floor (a pause, room noise) neither
        // enters the level nor moves the gain. Measuring the tail of a word fading into
        // silence would otherwise read "too quiet" and pull the gain up during every pause.
        let speech = gain_to_db(mean_square.sqrt()) >= p.floor_db;
        if speech {
            // One-pole smoothing with the window as time constant.
            let alpha = 1.0 - (-seconds / (p.window_ms / 1000.0)).exp();
            self.mean_square += (mean_square - self.mean_square) * alpha;
        }
        let wanted = if !self.enabled {
            0.0
        } else if !speech {
            self.gain_db
        } else {
            (p.target_db - self.level_db()).clamp(-p.max_gain_db, p.max_gain_db)
        };
        self.gain_db = if wanted > self.gain_db {
            (self.gain_db + p.rise_db_per_s * seconds).min(wanted)
        } else {
            (self.gain_db - p.fall_db_per_s * seconds).max(wanted)
        };
        db_to_gain(self.gain_db)
    }

    /// In place: the block is scaled with the current gain, then the gain is updated from
    /// this block's level. Non-finite samples count as silence.
    pub fn process(&mut self, block: &mut [f32]) {
        let gain = db_to_gain(self.gain_db);
        let mut energy = 0.0f32;
        for s in block.iter_mut() {
            let x = if s.is_finite() { *s } else { 0.0 };
            energy += x * x;
            *s = x * gain;
        }
        if !block.is_empty() {
            self.step(energy / block.len() as f32, block.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 480;

    fn sine(amplitude: f32) -> Vec<f32> {
        (0..BLOCK)
            .map(|i| (i as f32 * 0.07).sin() * amplitude)
            .collect()
    }

    /// Feeds `seconds` of a steady sine and returns the RMS of the last block out.
    fn run(leveler: &mut Leveler, amplitude: f32, seconds: f32) -> f32 {
        let mut last = 0.0;
        for _ in 0..(seconds * 100.0) as usize {
            let mut b = sine(amplitude);
            leveler.process(&mut b);
            last = crate::rms(&b);
        }
        last
    }

    #[test]
    fn a_quiet_voice_is_pulled_up_to_the_target_within_seconds() {
        let mut l = Leveler::new(SR, LevelerParams::default());
        // −32 dBFS RMS: 12 dB under the target.
        let out = run(&mut l, db_to_gain(-32.0) * std::f32::consts::SQRT_2, 6.0);
        assert!(
            (gain_to_db(out) + 20.0).abs() < 1.0,
            "{} dBFS",
            gain_to_db(out)
        );
        assert!((l.gain_db() - 12.0).abs() < 1.0);
    }

    #[test]
    fn a_loud_voice_is_pulled_down_faster_than_a_quiet_one_is_pulled_up() {
        let mut l = Leveler::new(SR, LevelerParams::default());
        run(&mut l, db_to_gain(-10.0) * std::f32::consts::SQRT_2, 1.0);
        let after_one_second = l.gain_db();
        assert!(
            after_one_second < -3.0 && after_one_second > -7.0,
            "{after_one_second}"
        );
        run(&mut l, db_to_gain(-10.0) * std::f32::consts::SQRT_2, 3.0);
        assert!((l.gain_db() + 10.0).abs() < 0.5);
    }

    #[test]
    fn it_moves_slowly_not_per_syllable() {
        let mut l = Leveler::new(SR, LevelerParams::default());
        run(&mut l, db_to_gain(-20.0) * std::f32::consts::SQRT_2, 4.0);
        let before = l.gain_db();
        // A 100 ms shout at −6 dBFS.
        run(&mut l, db_to_gain(-6.0) * std::f32::consts::SQRT_2, 0.1);
        assert!(before - l.gain_db() < 0.7, "{} → {}", before, l.gain_db());
    }

    #[test]
    fn silence_and_room_noise_do_not_raise_the_gain() {
        let mut l = Leveler::new(SR, LevelerParams::default());
        run(&mut l, 0.0, 3.0);
        assert_eq!(l.gain_db(), 0.0);
        run(&mut l, db_to_gain(-60.0), 3.0);
        assert_eq!(l.gain_db(), 0.0, "−60 dBFS is under the floor: hold");
        // A speaker who then goes quiet keeps the gain they had.
        run(&mut l, db_to_gain(-30.0) * std::f32::consts::SQRT_2, 4.0);
        let while_talking = l.gain_db();
        assert!(while_talking > 5.0);
        run(&mut l, 0.0, 3.0);
        assert_eq!(l.gain_db(), while_talking);
    }

    #[test]
    fn switching_off_slides_back_to_unity_and_reset_is_instant() {
        let mut l = Leveler::new(SR, LevelerParams::default());
        run(&mut l, db_to_gain(-35.0) * std::f32::consts::SQRT_2, 5.0);
        assert!(l.gain_db() > 10.0);
        l.set_enabled(false);
        run(&mut l, db_to_gain(-35.0) * std::f32::consts::SQRT_2, 1.0);
        assert!(l.gain_db() > 0.0 && l.gain_db() < 8.0, "on its way down");
        run(&mut l, db_to_gain(-35.0) * std::f32::consts::SQRT_2, 3.0);
        assert_eq!(l.gain_db(), 0.0);
        l.set_enabled(true);
        run(&mut l, db_to_gain(-35.0) * std::f32::consts::SQRT_2, 2.0);
        assert!(l.gain_db() > 0.0);
        l.reset();
        assert_eq!(l.gain_db(), 0.0);
        assert_eq!(l.level_db(), crate::level::MIN_DB);
    }

    #[test]
    fn non_finite_input_counts_as_silence() {
        let mut l = Leveler::new(SR, LevelerParams::default());
        let mut b = [f32::NAN, f32::INFINITY, 0.1];
        l.process(&mut b);
        assert_eq!(b[..2], [0.0, 0.0]);
        assert!(l.level_db().is_finite());
        assert!(
            (l.step(f32::NAN, 480) - 1.0).abs() < 1e-4,
            "NaN is a pause: hold"
        );
    }

    proptest! {
        /// Whatever the level does, the gain stays within ±max and never goes non-finite.
        #[test]
        fn gain_stays_in_range(
            levels in prop::collection::vec(0.0f32..1.5, 1..400),
            max in 0.0f32..24.0,
        ) {
            let params = LevelerParams { max_gain_db: max, ..LevelerParams::default() };
            let mut l = Leveler::new(SR, params);
            for amplitude in levels {
                let mut b = sine(amplitude);
                l.process(&mut b);
                prop_assert!(l.gain_db().is_finite());
                prop_assert!(l.gain_db().abs() <= max + 1e-4, "{}", l.gain_db());
                prop_assert!(b.iter().all(|s| s.is_finite()));
            }
        }

        /// Anything under the floor, in any rhythm, never moves the gain off 0 dB.
        #[test]
        fn silence_is_never_pumped_up(
            levels in prop::collection::vec(0.0f32..0.002, 1..400),
        ) {
            let mut l = Leveler::new(SR, LevelerParams::default());
            for amplitude in levels {
                let mut b = sine(amplitude); // peak 0.002 = −54 dBFS, RMS lower still
                l.process(&mut b);
                prop_assert_eq!(l.gain_db(), 0.0);
            }
        }
    }
}
