//! Sample-rate conversion for devices that do not run at 48 kHz (A1, ADR-009): a windowed
//! sinc interpolator with a table of filter phases. Pull style — `produce` fills an output
//! slice and asks for input samples as it needs them — so the same type sits behind a ring
//! in the pump (device rate → 48 kHz) and in front of the mixer in the playout callback
//! (48 kHz → device rate). No allocation after `new`, no state outside the struct.

use std::f64::consts::PI;

/// Filter phases in the table; between two neighbours the kernel is interpolated linearly.
const PHASES: usize = 128;
/// Kernel length when not downsampling. Going down it grows with the ratio: the cutoff
/// moves down to the lower Nyquist frequency and the sinc gets wider with it.
const BASE_TAPS: usize = 32;

pub struct Resampler {
    /// Input samples per output sample.
    step: f64,
    /// Where the next output sits between the two input samples around the kernel centre.
    frac: f64,
    taps: usize,
    /// `(PHASES + 1) * taps` kernel values; row `p` is the kernel for `frac = p / PHASES`.
    table: Vec<f32>,
    /// The last `taps` inputs as a ring, oldest at `head`. Starts as silence, so the first
    /// `taps / 2` outputs are the kernel's lead-in and nobody waits for a priming fill
    /// (realtime review 2026-10-24: a priming wait left the pump 8–9 ms behind for good).
    history: Vec<f32>,
    head: usize,
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        assert!(in_rate > 0 && out_rate > 0, "sample rates must be positive");
        let step = f64::from(in_rate) / f64::from(out_rate);
        let taps = ((BASE_TAPS as f64 * step.max(1.0)).ceil() as usize).next_multiple_of(2);
        // Cutoff as a fraction of the input rate (0.5 = Nyquist): the lower of the two
        // Nyquist frequencies, with room for the transition band.
        let cutoff = 0.45 * (1.0 / step).min(1.0);
        let centre = (taps / 2 - 1) as f64;
        let half = (taps / 2) as f64;
        let mut table = vec![0f32; (PHASES + 1) * taps];
        for p in 0..=PHASES {
            let frac = p as f64 / PHASES as f64;
            let row = &mut table[p * taps..(p + 1) * taps];
            let mut sum = 0.0;
            for (k, h) in row.iter_mut().enumerate() {
                let x = k as f64 - centre - frac;
                // Blackman window centred on the output position, -1..1 across the kernel.
                let u = x / half;
                let window = if u.abs() >= 1.0 {
                    0.0
                } else {
                    0.42 + 0.5 * (PI * u).cos() + 0.08 * (2.0 * PI * u).cos()
                };
                let value = 2.0 * cutoff * sinc(2.0 * cutoff * x) * window;
                *h = value as f32;
                sum += value;
            }
            // Unity gain at DC for every phase, whatever the window took.
            for h in row.iter_mut() {
                *h = (f64::from(*h) / sum) as f32;
            }
        }
        Self {
            step,
            frac: 0.0,
            taps,
            table,
            history: vec![0.0; taps],
            head: 0,
        }
    }

    /// Exactly how many inputs `produce` will ask for to fill `out_len` outputs from the
    /// current state — the same arithmetic `produce` runs, replayed. Lets a caller feed a
    /// ring or render a buffer with the precise amount, so nothing piles up behind the
    /// converter.
    pub fn input_needed(&self, out_len: usize) -> usize {
        let mut frac = self.frac;
        let mut needed = 0;
        for _ in 0..out_len {
            while frac >= 1.0 {
                frac -= 1.0;
                needed += 1;
            }
            frac += self.step;
        }
        needed
    }

    /// Latency in input samples: half the kernel.
    pub fn latency_samples(&self) -> usize {
        self.taps / 2
    }

    pub fn reset(&mut self) {
        self.frac = 0.0;
        self.head = 0;
        self.history.fill(0.0);
    }

    /// Fills `out` from the front, pulling inputs with `next`. Returns how many outputs were
    /// written; fewer than `out.len()` only when `next` ran dry — the rest of `out` is left
    /// untouched, and the next call continues seamlessly.
    pub fn produce(&mut self, out: &mut [f32], mut next: impl FnMut() -> Option<f32>) -> usize {
        for (written, slot) in out.iter_mut().enumerate() {
            while self.frac >= 1.0 {
                match next() {
                    Some(sample) => {
                        self.push(sample);
                        self.frac -= 1.0;
                    }
                    None => return written,
                }
            }
            *slot = self.interpolate();
            self.frac += self.step;
        }
        out.len()
    }

    fn push(&mut self, sample: f32) {
        self.history[self.head] = sample;
        self.head = (self.head + 1) % self.taps;
    }

    fn interpolate(&self) -> f32 {
        let position = self.frac * PHASES as f64;
        let p = position as usize; // frac < 1, so p < PHASES and row p + 1 exists
        let weight = (position - p as f64) as f32;
        let row0 = &self.table[p * self.taps..(p + 1) * self.taps];
        let row1 = &self.table[(p + 1) * self.taps..(p + 2) * self.taps];
        // Oldest first: `history[head..]`, then the wrapped part `history[..head]`.
        let (newer, older) = self.history.split_at(self.head);
        let inputs = older.iter().chain(newer.iter());
        let (mut a, mut b) = (0f32, 0f32);
        for ((x, h0), h1) in inputs.zip(row0).zip(row1) {
            a += x * h0;
            b += x * h1;
        }
        a + (b - a) * weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::f32::consts::TAU;

    fn sine(rate: u32, hz: f32, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|i| (i as f32 / rate as f32 * hz * TAU).sin() * 0.5)
            .collect()
    }

    fn through(in_rate: u32, out_rate: u32, input: &[f32]) -> Vec<f32> {
        let mut rs = Resampler::new(in_rate, out_rate);
        let mut source = input.iter().copied();
        let mut out = Vec::new();
        let mut block = [0f32; 480];
        loop {
            let n = rs.produce(&mut block, || source.next());
            out.extend_from_slice(&block[..n]);
            if n < block.len() {
                return out;
            }
        }
    }

    /// The middle 80 %: the lead-in and the tail are not what we measure.
    fn middle(signal: &[f32]) -> &[f32] {
        let edge = signal.len() / 10;
        &signal[edge..signal.len() - edge]
    }

    fn frequency(signal: &[f32], rate: u32) -> f32 {
        let s = middle(signal);
        let crossings = s
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        crossings as f32 / 2.0 / (s.len() as f32 / rate as f32)
    }

    fn level(signal: &[f32]) -> f32 {
        let s = middle(signal);
        (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
    }

    #[test]
    fn a_tone_keeps_its_frequency_and_level_in_both_directions() {
        for (in_rate, out_rate) in [
            (16_000, 48_000),
            (48_000, 16_000),
            (44_100, 48_000),
            (48_000, 44_100),
            (96_000, 48_000),
        ] {
            let input = sine(in_rate, 1_000.0, 1.0);
            let out = through(in_rate, out_rate, &input);
            // No priming: every input becomes output at the new rate.
            let expected_len =
                (input.len() as f64 * f64::from(out_rate) / f64::from(in_rate)) as i64;
            assert!(
                (out.len() as i64 - expected_len).abs() <= 3,
                "{in_rate}->{out_rate}: {} outputs, expected about {expected_len}",
                out.len()
            );
            let f = frequency(&out, out_rate);
            assert!((f - 1_000.0).abs() < 10.0, "{in_rate}->{out_rate}: {f} Hz");
            let rms = level(&out);
            assert!(
                (rms - 0.3536).abs() < 0.01,
                "{in_rate}->{out_rate}: rms {rms}"
            );
        }
    }

    #[test]
    fn downsampling_removes_what_the_lower_rate_cannot_carry() {
        // 11 kHz is above 16 kHz's Nyquist; without the filter it would alias to 5 kHz.
        let out = through(48_000, 16_000, &sine(48_000, 11_000.0, 1.0));
        let rms = level(&out);
        assert!(
            rms < 0.004,
            "aliased energy left: rms {rms} (-{} dB)",
            -20.0 * rms.log10()
        );
    }

    #[test]
    fn input_needed_is_exact_and_a_dry_source_resumes() {
        let input = sine(44_100, 1_000.0, 0.2);
        let straight = through(44_100, 48_000, &input);

        // Exact: a block pulls precisely what `input_needed` said, block after block.
        let mut rs = Resampler::new(44_100, 48_000);
        let mut source = input.iter().copied();
        for _ in 0..10 {
            let said = rs.input_needed(480);
            let mut pulled = 0usize;
            let mut block = [0f32; 480];
            let n = rs.produce(&mut block, || {
                pulled += 1;
                source.next()
            });
            assert_eq!(n, block.len());
            assert_eq!(pulled, said);
        }

        // A source that dries up mid-block: the block is partial, the next call picks up.
        let mut rs = Resampler::new(44_100, 48_000);
        let mut fed = 0usize;
        let mut out = Vec::new();
        while out.len() < straight.len() {
            let budget = fed + 100; // hand over 100 inputs at a time
            let mut block = [0f32; 480];
            let n = rs.produce(&mut block, || {
                if fed < budget && fed < input.len() {
                    fed += 1;
                    Some(input[fed - 1])
                } else {
                    None
                }
            });
            out.extend_from_slice(&block[..n]);
            if fed >= input.len() && n == 0 {
                break;
            }
        }
        assert_eq!(out.len(), straight.len());
        assert!(out.iter().zip(&straight).all(|(a, b)| (a - b).abs() < 1e-6));
    }

    #[test]
    fn latency_is_half_the_kernel() {
        assert_eq!(Resampler::new(16_000, 48_000).latency_samples(), 16);
        assert_eq!(Resampler::new(48_000, 16_000).latency_samples(), 48);
    }

    proptest! {
        #[test]
        fn output_stays_bounded_and_finite(
            rates in prop::sample::select(vec![(16_000u32, 48_000u32), (48_000, 16_000), (44_100, 48_000), (48_000, 44_100)]),
            input in prop::collection::vec(-1.0f32..1.0, 200..2_000),
        ) {
            // No gain beyond the kernel's L1 norm (the worst case for any input in -1..1).
            let rs = Resampler::new(rates.0, rates.1);
            let bound = rs
                .table
                .chunks(rs.taps)
                .map(|row| row.iter().map(|h| h.abs()).sum::<f32>())
                .fold(0f32, f32::max)
                + 1e-3;
            let out = through(rates.0, rates.1, &input);
            prop_assert!(out.iter().all(|y| y.is_finite() && y.abs() <= bound), "bound {bound}");
        }
    }
}
