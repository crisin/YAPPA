//! Autotune (ADR-012, "Autotune und Harmonizer"): the voice snaps to the notes of a scale.
//! Per block the [`PitchTracker`] finds the voice's pitch, the nearest note of the chosen
//! key and scale is the target, and the [`PitchShifter`] is steered by the ratio between
//! the two. The ratio is smoothed by the retune time: 0 is the hard snap everybody knows
//! from the radio, 1 glides to the note over ≈ 200 ms and sounds like a steadier singer.
//! No lookahead: the delay is the shifter's 15 ms, the detector only looks back.
//!
//! The "live mode" design of the classic open implementations (YIN plus a pitch-synchronous
//! delay line), written here from scratch — nothing GPL-licensed is copied.

use crate::pitch::{MAX_RATIO, MIN_RATIO};
use crate::{nearest_note, Key, PitchShifter, PitchTracker, Scale};

/// Retune 1.0 = this time constant; the ratio reaches the note in ≈ 3× that.
const RETUNE_MAX_S: f32 = 0.2;
/// How long the last ratio is held once the detector loses the voice: a missed block in
/// the middle of a vowel must not let the correction drift away.
const HOLD_S: f32 = 0.05;
/// After the hold the ratio relaxes to unity with this time constant, so the next vowel
/// starts from a neutral shifter instead of the last word's correction.
const RELAX_S: f32 = 0.15;

/// Turns a stream of per-block target ratios (or `None` when unvoiced) into a smooth ratio
/// for the shifter: retune smoothing while voiced, a short hold, then relaxation to unity.
/// Shared by the autotune and the harmonizer.
#[derive(Debug, Clone)]
pub(crate) struct Follower {
    ratio: f32,
    target: f32,
    retune: f32,
    hold_left: f32,
    voiced: bool,
}

impl Follower {
    pub(crate) fn new(retune: f32) -> Self {
        Self {
            ratio: 1.0,
            target: 1.0,
            retune: clamp_unit(retune),
            hold_left: 0.0,
            voiced: false,
        }
    }

    pub(crate) fn set_retune(&mut self, retune: f32) {
        self.retune = clamp_unit(retune);
    }

    pub(crate) fn retune(&self) -> f32 {
        self.retune
    }

    /// The voice is there, or was within the hold.
    pub(crate) fn holding(&self) -> bool {
        self.voiced || self.hold_left > 0.0
    }

    /// One block: the new target (if voiced) and the block's length in seconds. Returns
    /// the ratio the shifter should run at for this block.
    pub(crate) fn update(&mut self, target: Option<f32>, block_s: f32) -> f32 {
        match target {
            Some(t) if t.is_finite() => {
                self.target = t.clamp(MIN_RATIO, MAX_RATIO);
                self.hold_left = HOLD_S;
                self.voiced = true;
            }
            _ => {
                self.voiced = false;
                if self.hold_left > 0.0 {
                    self.hold_left -= block_s;
                } else {
                    self.target = 1.0;
                }
            }
        }
        let time_constant = if self.voiced {
            self.retune * RETUNE_MAX_S
        } else if self.hold_left > 0.0 {
            // Holding: keep the ratio where it is.
            return self.ratio;
        } else {
            RELAX_S
        };
        let coeff = if time_constant <= 0.0 {
            1.0
        } else {
            1.0 - (-block_s / time_constant).exp()
        };
        self.ratio += coeff * (self.target - self.ratio);
        self.ratio
    }

    pub(crate) fn ratio(&self) -> f32 {
        self.ratio
    }
}

fn clamp_unit(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[derive(Debug, Clone)]
pub struct Autotune {
    tracker: PitchTracker,
    shifter: PitchShifter,
    follower: Follower,
    key: Key,
    scale: Scale,
    sample_rate: f32,
}

impl Autotune {
    /// `retune`: 0 = hard snap, 1 = ≈ 200 ms glide.
    pub fn new(sample_rate: f32, key: Key, scale: Scale, retune: f32) -> Self {
        let mut shifter = PitchShifter::new(sample_rate, 0.0);
        shifter.set_ratio(1.0);
        Self {
            tracker: PitchTracker::new(sample_rate),
            shifter,
            follower: Follower::new(retune),
            key: key % 12,
            scale,
            sample_rate,
        }
    }

    pub fn set_key(&mut self, key: Key) {
        self.key = key % 12;
    }

    pub fn set_scale(&mut self, scale: Scale) {
        self.scale = scale;
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

    pub fn retune(&self) -> f32 {
        self.follower.retune()
    }

    /// The ratio the shifter currently runs at (1.0 = the voice is on a note or absent).
    pub fn ratio(&self) -> f32 {
        self.follower.ratio()
    }

    /// Half the shifter's window: 15 ms.
    pub fn latency_samples(&self) -> usize {
        self.shifter.latency_samples()
    }

    /// In place, no allocation.
    pub fn process(&mut self, block: &mut [f32]) {
        let target = self
            .tracker
            .track(block)
            .map(|p| nearest_note(p.hz, self.key, self.scale) / p.hz);
        let ratio = self
            .follower
            .update(target, block.len() as f32 / self.sample_rate);
        self.shifter.set_ratio(ratio);
        self.shifter.process(block);
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

    fn run(at: &mut Autotune, signal: &mut [f32]) {
        for block in signal.chunks_mut(BLOCK) {
            at.process(block);
        }
    }

    /// A voice a quarter tone sharp of A (69.5) is pulled to A (69) — or H (71) — and
    /// the output, measured with a fresh tracker, sits on a scale note.
    #[test]
    fn a_sharp_voice_lands_on_a_note_of_the_scale() {
        let mut at = Autotune::new(SR, 0, Scale::Major, 0.0);
        let hz = midi_to_hz(69.45);
        let mut signal = voice(hz, 48_000);
        run(&mut at, &mut signal);
        assert!(
            (at.ratio() - 440.0 / hz).abs() < 0.003,
            "ratio {}",
            at.ratio()
        );
        let mut probe = PitchTracker::new(SR);
        let mut out = None;
        for block in signal[24_000..].chunks(BLOCK) {
            out = probe.track(block);
        }
        let got = hz_to_midi(out.expect("the output is still voiced").hz);
        assert!((got - 69.0).abs() < 0.1, "output at midi {got}");
    }

    #[test]
    fn a_voice_already_on_a_note_is_left_alone() {
        let mut at = Autotune::new(SR, 0, Scale::Major, 0.3);
        let mut signal = voice(440.0, 24_000);
        run(&mut at, &mut signal);
        assert!((at.ratio() - 1.0).abs() < 0.003, "ratio {}", at.ratio());
    }

    /// Retune 1 glides: after one 10 ms block the ratio has only begun to move; retune 0
    /// is there at once.
    #[test]
    fn the_retune_time_slows_the_correction() {
        let hz = midi_to_hz(69.45);
        let mut hard = Autotune::new(SR, 0, Scale::Major, 0.0);
        let mut soft = Autotune::new(SR, 0, Scale::Major, 1.0);
        // Warm the trackers, then look at the first corrected block.
        let mut warm = voice(hz, 2_400);
        run(&mut hard, &mut warm.clone());
        run(&mut soft, &mut warm);
        let target = 440.0 / hz;
        assert!((hard.ratio() - target).abs() < 0.003, "{}", hard.ratio());
        assert!(
            (soft.ratio() - 1.0).abs() < (target - 1.0).abs() * 0.5,
            "soft ratio {} after 50 ms, target {target}",
            soft.ratio()
        );
    }

    /// Silence after a voice: the ratio holds for a moment, then relaxes to unity.
    #[test]
    fn unvoiced_holds_then_relaxes_to_unity() {
        let hz = midi_to_hz(69.45);
        let mut at = Autotune::new(SR, 0, Scale::Major, 0.0);
        run(&mut at, &mut voice(hz, 9_600));
        // The detector still hears the voice in its 32 ms window for the first silent
        // blocks; from the fourth on the window is silent and the hold runs.
        let mut silence = vec![0.0f32; BLOCK * 4];
        run(&mut at, &mut silence);
        let corrected = at.ratio();
        assert!((corrected - 1.0).abs() > 0.01);
        let mut silence = vec![0.0f32; BLOCK * 3];
        run(&mut at, &mut silence);
        assert_eq!(at.ratio(), corrected, "held for another 30 ms");
        let mut silence = vec![0.0f32; BLOCK * 100];
        run(&mut at, &mut silence);
        assert!((at.ratio() - 1.0).abs() < 1e-3, "relaxed: {}", at.ratio());
    }

    #[test]
    fn parameters_change_without_a_rebuild_and_bad_values_are_tamed() {
        let mut at = Autotune::new(SR, 14, Scale::Blues, f32::NAN);
        assert_eq!(at.key(), 2, "keys wrap at the octave");
        assert_eq!(at.retune(), 0.0);
        at.set_key(7);
        at.set_scale(Scale::PentatonicMinor);
        at.set_retune(2.0);
        assert_eq!(
            (at.key(), at.scale(), at.retune()),
            (7, Scale::PentatonicMinor, 1.0)
        );
        assert_eq!(at.latency_samples(), 720);
        let mut block = [f32::NAN; BLOCK];
        at.process(&mut block);
        assert!(block.iter().all(|s| s.is_finite()));
        let mut next = voice(200.0, BLOCK);
        at.process(&mut next);
        assert!(next.iter().all(|s| s.is_finite()));
    }
}
