//! A hall for the voice (ADR-012): Freeverb's network — eight parallel feedback combs with
//! a one-pole damping filter in the loop, then four allpasses in series — in mono, with the
//! delay lengths scaled from 44.1 to 48 kHz. Fixed room (medium), fixed damping; the one
//! knob is how much hall is mixed in. About twelve delay lines per sample: cheap.

/// Freeverb's comb lengths at 44.1 kHz; scaled to the sample rate at construction.
const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
const TUNING_RATE: f32 = 44_100.0;
/// Room size 0.5 on Freeverb's scale: feedback 0.7 + 0.5 × 0.28.
const COMB_FEEDBACK: f32 = 0.84;
/// Damping 0.5 on Freeverb's scale (× 0.4): the tail loses its highs like a real room.
const DAMP: f32 = 0.2;
const ALLPASS_FEEDBACK: f32 = 0.5;
/// Freeverb feeds the combs with the input × 0.015; the sum of eight combs then sits near
/// the dry level.
const INPUT_GAIN: f32 = 0.015;

#[derive(Debug, Clone)]
struct Comb {
    buf: Vec<f32>,
    pos: usize,
    filter: f32,
}

impl Comb {
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let out = self.buf[self.pos];
        self.filter = out * (1.0 - DAMP) + self.filter * DAMP;
        if self.filter.abs() < 1e-20 {
            self.filter = 0.0;
        }
        self.buf[self.pos] = x + self.filter * COMB_FEEDBACK;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

#[derive(Debug, Clone)]
struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let mut delayed = self.buf[self.pos];
        if delayed.abs() < 1e-20 {
            delayed = 0.0; // the loop halves per period: flush before it goes subnormal
        }
        self.buf[self.pos] = x + delayed * ALLPASS_FEEDBACK;
        self.pos = (self.pos + 1) % self.buf.len();
        delayed - x
    }
}

#[derive(Debug, Clone)]
pub struct Reverb {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
    /// Wet share, 0..=1.
    amount: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32, amount: f32) -> Self {
        let scale = sample_rate / TUNING_RATE;
        let len = |n: usize| ((n as f32 * scale) as usize).max(1);
        Self {
            combs: COMB_TUNING
                .iter()
                .map(|&n| Comb {
                    buf: vec![0.0; len(n)],
                    pos: 0,
                    filter: 0.0,
                })
                .collect(),
            allpasses: ALLPASS_TUNING
                .iter()
                .map(|&n| Allpass {
                    buf: vec![0.0; len(n)],
                    pos: 0,
                })
                .collect(),
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

    /// 0 = dry, 1 = as much hall as voice.
    pub fn set_amount(&mut self, amount: f32) {
        self.amount = Self::clamp(amount);
    }

    pub fn amount(&self) -> f32 {
        self.amount
    }

    /// In place, no allocation. The tail keeps running through silence (and through blocks
    /// with the amount at 0, so turning it up again does not start from nothing).
    pub fn process(&mut self, block: &mut [f32]) {
        for s in block.iter_mut() {
            let x = if s.is_finite() { *s } else { 0.0 };
            let input = x * INPUT_GAIN;
            let mut wet = 0.0;
            for c in self.combs.iter_mut() {
                wet += c.tick(input);
            }
            for a in self.allpasses.iter_mut() {
                wet = a.tick(wet);
            }
            *s = x + wet * self.amount;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn a_click_leaves_a_tail_that_dies_away() {
        let mut r = Reverb::new(SR, 1.0);
        let mut block = [0.0f32; 480];
        block[0] = 1.0;
        r.process(&mut block);
        let mut tail = Vec::new();
        for _ in 0..200 {
            let mut silence = [0.0f32; 480];
            r.process(&mut silence);
            tail.push(crate::rms(&silence));
        }
        // The shortest comb is ≈ 25 ms long: nothing comes back before the third block.
        assert_eq!(tail[0], 0.0, "the hall has no direct path");
        let loudest = tail.iter().cloned().fold(0.0f32, f32::max);
        assert!(loudest > 1e-4, "no tail after the click: {loudest}");
        assert!(tail[3] > 0.0, "the first reflections come within 40 ms");
        assert!(
            tail[100] < loudest / 4.0,
            "the tail does not decay: {}",
            tail[100]
        );
        assert!(tail[199] < 1e-5, "the tail never dies: {}", tail[199]);
    }

    #[test]
    fn amount_zero_passes_the_voice_through() {
        let mut r = Reverb::new(SR, 0.0);
        let mut block: Vec<f32> = (0..480).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let before = block.clone();
        r.process(&mut block);
        assert_eq!(block, before);
    }

    #[test]
    fn a_steady_voice_does_not_run_away() {
        let mut r = Reverb::new(SR, 1.0);
        let mut last = 0.0;
        for _ in 0..300 {
            let mut block: Vec<f32> = (0..480).map(|i| (i as f32 * 0.03).sin() * 0.5).collect();
            r.process(&mut block);
            assert!(block.iter().all(|s| s.is_finite()));
            last = crate::rms(&block);
        }
        assert!(last < 1.0, "{last}");
    }
}
