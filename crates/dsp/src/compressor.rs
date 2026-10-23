//! Feed-forward compressor with a soft knee, so everybody sits at a similar level: loud
//! talkers are turned down above the threshold, `makeup_db` lifts the result again.

use crate::{db_to_gain, gain_to_db};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompressorParams {
    pub threshold_db: f32,
    /// 3 = 3 dB in above the threshold give 1 dB out.
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
    /// Width of the soft knee around the threshold.
    pub knee_db: f32,
}

impl Default for CompressorParams {
    fn default() -> Self {
        Self {
            threshold_db: -18.0,
            ratio: 3.0,
            attack_ms: 5.0,
            release_ms: 120.0,
            makeup_db: 3.0,
            knee_db: 6.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Compressor {
    params: CompressorParams,
    attack: f32,
    release: f32,
    makeup: f32,
    /// Gain reduction in dB (≤ 0), smoothed.
    reduction_db: f32,
}

fn coefficient(ms: f32, sample_rate: f32) -> f32 {
    let samples = (ms / 1000.0 * sample_rate).max(1.0);
    (-1.0 / samples).exp()
}

impl Compressor {
    pub fn new(sample_rate: f32, params: CompressorParams) -> Self {
        let mut c = Self {
            params,
            attack: 0.0,
            release: 0.0,
            makeup: 1.0,
            reduction_db: 0.0,
        };
        c.set_params(sample_rate, params);
        c
    }

    pub fn set_params(&mut self, sample_rate: f32, params: CompressorParams) {
        self.params = CompressorParams {
            ratio: params.ratio.max(1.0),
            knee_db: params.knee_db.max(0.0),
            ..params
        };
        self.attack = coefficient(params.attack_ms, sample_rate);
        self.release = coefficient(params.release_ms, sample_rate);
        self.makeup = db_to_gain(params.makeup_db);
    }

    /// How much is taken off right now, dB (≥ 0) — for the pro view's meter.
    pub fn reduction_db(&self) -> f32 {
        -self.reduction_db
    }

    /// Static curve: wanted reduction (≤ 0 dB) for an input level.
    fn curve(&self, level_db: f32) -> f32 {
        let p = &self.params;
        let over = level_db - p.threshold_db;
        let slope = 1.0 / p.ratio - 1.0;
        if p.knee_db > 0.0 && over.abs() <= p.knee_db / 2.0 {
            let x = over + p.knee_db / 2.0;
            slope * x * x / (2.0 * p.knee_db)
        } else if over > 0.0 {
            slope * over
        } else {
            0.0
        }
    }

    /// In place, no allocation — safe for the real-time thread.
    pub fn process(&mut self, block: &mut [f32]) {
        for s in block.iter_mut() {
            let wanted = self.curve(gain_to_db(s.abs()));
            // More reduction = attack, less = release.
            let coef = if wanted < self.reduction_db {
                self.attack
            } else {
                self.release
            };
            self.reduction_db = wanted + (self.reduction_db - wanted) * coef;
            *s *= db_to_gain(self.reduction_db) * self.makeup;
        }
        if self.reduction_db > -1e-6 {
            self.reduction_db = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rms;

    const SR: f32 = 48_000.0;

    fn tone(db: f32) -> Vec<f32> {
        (0..24_000)
            .map(|i| (i as f32 * 0.06).sin() * db_to_gain(db) * std::f32::consts::SQRT_2)
            .collect()
    }

    fn settled_db(c: &mut Compressor, db: f32) -> f32 {
        let mut x = tone(db);
        c.process(&mut x);
        gain_to_db(rms(&x[12_000..]))
    }

    fn plain() -> CompressorParams {
        CompressorParams {
            makeup_db: 0.0,
            knee_db: 0.0,
            ..CompressorParams::default()
        }
    }

    #[test]
    fn below_the_threshold_nothing_changes_but_the_makeup() {
        let mut c = Compressor::new(
            SR,
            CompressorParams {
                knee_db: 0.0,
                ..CompressorParams::default()
            },
        );
        assert!(
            (settled_db(&mut c, -30.0) - -27.0).abs() < 0.2,
            "only +3 dB makeup"
        );
    }

    #[test]
    fn above_the_threshold_the_ratio_applies() {
        let mut c = Compressor::new(SR, plain());
        // 12 dB over the threshold at 3:1 → 4 dB over. The sine's peaks drive the detector,
        // so allow some slack against the RMS reading.
        let out = settled_db(&mut c, -6.0);
        assert!(out < -11.0 && out > -15.5, "{out}");
        assert!(c.reduction_db() > 5.0);
    }

    #[test]
    fn loud_and_quiet_end_up_closer_together() {
        let mut c = Compressor::new(SR, CompressorParams::default());
        let loud = settled_db(&mut c, -6.0);
        let mut c = Compressor::new(SR, CompressorParams::default());
        let quiet = settled_db(&mut c, -24.0);
        assert!(loud - quiet < 12.0, "18 dB apart in, {} out", loud - quiet);
    }

    #[test]
    fn silence_stays_silent_and_finite() {
        let mut c = Compressor::new(SR, CompressorParams::default());
        let mut x = vec![0.0; 960];
        c.process(&mut x);
        assert!(x.iter().all(|s| *s == 0.0));
    }
}
