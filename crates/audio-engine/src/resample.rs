//! The playout side of A1 (ADR-009): the mixer renders 48 kHz, the device wants its own
//! rate. [`ResampledRender`] asks the mixer once per callback for exactly the 48 kHz samples
//! that callback will consume and converts them — no allocation after construction, so it
//! may live in the device callback. The capture side converts on the pump instead
//! (`engine::run_pump`).

use dsp::Resampler;
use transport::SAMPLE_RATE;

/// Largest callback the playout path handles in one go; bigger ones are rendered in pieces.
pub const MAX_CALLBACK_FRAMES: usize = 8192;

pub struct ResampledRender {
    resampler: Resampler,
    /// The 48 kHz samples for the current callback, plus what the last one left over.
    piece: Vec<f32>,
    pos: usize,
    len: usize,
}

impl ResampledRender {
    pub fn new(device_rate: u32) -> Self {
        let resampler = Resampler::new(SAMPLE_RATE, device_rate);
        // Room for the largest callback's demand and a few leftover samples.
        let capacity = resampler.input_needed(MAX_CALLBACK_FRAMES) + 8;
        Self {
            resampler,
            piece: vec![0.0; capacity],
            pos: 0,
            len: 0,
        }
    }

    /// Fills `out` (device rate, at most `MAX_CALLBACK_FRAMES`) from `render`, which fills
    /// a zeroed 48 kHz buffer. One render per callback with the real demand: the mixer sizes
    /// its start cushion by what one render asks for (realtime review 2026-10-24).
    pub fn render(&mut self, out: &mut [f32], mut render: impl FnMut(&mut [f32])) {
        let Self {
            resampler,
            piece,
            pos,
            len,
        } = self;
        let leftover = *len - *pos;
        piece.copy_within(*pos..*len, 0);
        let need = resampler
            .input_needed(out.len())
            .saturating_sub(leftover)
            .min(piece.len() - leftover);
        let fresh = &mut piece[leftover..leftover + need];
        fresh.fill(0.0);
        render(fresh);
        *pos = 0;
        *len = leftover + need;
        let written = resampler.produce(out, || {
            if *pos == *len {
                return None;
            }
            let sample = piece[*pos];
            *pos += 1;
            Some(sample)
        });
        // `input_needed` is exact, so this never runs dry; if it ever did, silence beats
        // stale samples.
        out[written..].fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    #[test]
    fn a_tone_survives_the_way_to_a_16_khz_device_in_odd_callback_sizes() {
        let mut render = ResampledRender::new(16_000);
        let mut phase = 0usize;
        let renders = std::cell::Cell::new(0usize);
        let mut source = |buf: &mut [f32]| {
            renders.set(renders.get() + 1);
            for s in buf.iter_mut() {
                *s = (phase as f32 / 48_000.0 * 1_000.0 * TAU).sin() * 0.5;
                phase += 1;
            }
        };
        let mut out = Vec::new();
        let mut callbacks = 0usize;
        for size in [37usize, 100, 480, 7, 333].iter().cycle().take(60) {
            let mut buf = vec![0f32; *size];
            render.render(&mut buf, &mut source);
            callbacks += 1;
            // Exactly one render per callback, and nothing piles up in between.
            assert_eq!(renders.get(), callbacks);
            assert!(
                render.len - render.pos <= 2,
                "{} samples left over",
                render.len - render.pos
            );
            out.extend(buf);
        }
        let edge = out.len() / 10;
        let s = &out[edge..out.len() - edge];
        let crossings = s
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        let f = crossings as f32 / 2.0 / (s.len() as f32 / 16_000.0);
        assert!((f - 1_000.0).abs() < 10.0, "{f} Hz");
        let rms = (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt();
        assert!((rms - 0.3536).abs() < 0.01, "rms {rms}");
    }

    #[test]
    fn the_largest_callback_fits_without_allocating_more() {
        let mut render = ResampledRender::new(16_000);
        let capacity = render.piece.len();
        let mut out = vec![0f32; MAX_CALLBACK_FRAMES];
        for _ in 0..3 {
            render.render(&mut out, |buf| buf.fill(0.25));
        }
        assert_eq!(render.piece.len(), capacity);
        assert!(out.iter().skip(64).all(|s| (s - 0.25).abs() < 1e-3));
    }
}
