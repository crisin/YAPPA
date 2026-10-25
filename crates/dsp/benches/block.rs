//! Cost per 10 ms block (480 samples at 48 kHz) — the unit of the latency budget
//! (planning doc: DSP under 5 ms per block). `cargo xtask bench`.

use criterion::{criterion_group, criterion_main, Criterion};
use dsp::{Biquad, BiquadKind, Leveler, LevelerParams, Limiter, Resampler, Varispeed};
use std::hint::black_box;

const SR: f32 = 48_000.0;
const BLOCK: usize = 480;

fn eq_chain(c: &mut Criterion) {
    let mut chain = [
        Biquad::new(BiquadKind::HighPass, SR, 80.0, 0.707),
        Biquad::new(BiquadKind::Peaking { gain_db: -3.0 }, SR, 250.0, 1.0),
        Biquad::new(BiquadKind::Peaking { gain_db: 2.0 }, SR, 1_500.0, 1.0),
        Biquad::new(BiquadKind::Peaking { gain_db: 3.0 }, SR, 4_000.0, 1.2),
        Biquad::new(BiquadKind::LowPass, SR, 16_000.0, 0.707),
    ];
    let mut block: Vec<f32> = (0..BLOCK)
        .map(|i| ((i as f32) * 0.05).sin() * 0.5)
        .collect();
    c.bench_function("5-band EQ, one 10 ms block", |b| {
        b.iter(|| {
            for f in chain.iter_mut() {
                f.process(black_box(&mut block));
            }
        })
    });
}

fn limiter(c: &mut Criterion) {
    let mut limiter = Limiter::new(SR, -1.0, 80.0);
    // Loud enough that the limiter works on every block.
    let mut block: Vec<f32> = (0..BLOCK)
        .map(|i| ((i as f32) * 0.05).sin() * 1.5)
        .collect();
    c.bench_function("limiter, one 10 ms block", |b| {
        b.iter(|| limiter.process(black_box(&mut block)))
    });
}

/// One speaker's block through the receive-side leveler (A4): measure and scale.
fn leveler(c: &mut Criterion) {
    let mut leveler = Leveler::new(SR, LevelerParams::default());
    let mut block: Vec<f32> = (0..BLOCK)
        .map(|i| ((i as f32) * 0.05).sin() * 0.05)
        .collect();
    c.bench_function("leveler, one 10 ms block", |b| {
        b.iter(|| leveler.process(black_box(&mut block)))
    });
}

/// One 10 ms block through the resampler, the three cases the gang's devices bring (A1).
fn resampler(c: &mut Criterion) {
    for (in_rate, out_rate, label) in [
        (16_000u32, 48_000u32, "16 -> 48 kHz (Bluetooth mic)"),
        (48_000, 16_000, "48 -> 16 kHz (Bluetooth out)"),
        (44_100, 48_000, "44.1 -> 48 kHz"),
    ] {
        let mut rs = Resampler::new(in_rate, out_rate);
        let input: Vec<f32> = (0..in_rate / 100 + 64)
            .map(|i| ((i as f32) * 0.05).sin() * 0.5)
            .collect();
        let mut out = vec![0f32; (out_rate / 100) as usize];
        c.bench_function(&format!("resampler {label}, one 10 ms block"), |b| {
            b.iter(|| {
                let mut source = input.iter().copied().cycle();
                rs.produce(black_box(&mut out), || source.next())
            })
        });
    }
}

/// One speaker's block through the drift compensation (A6): the varispeed at a ratio just
/// off unity — the cost the mixer pays per speaker on top of the leveler.
fn varispeed(c: &mut Criterion) {
    let mut vs = Varispeed::new();
    vs.set_ratio(1.0002);
    let input: Vec<f32> = (0..BLOCK + 8)
        .map(|i| ((i as f32) * 0.05).sin() * 0.5)
        .collect();
    let mut out = vec![0f32; BLOCK];
    c.bench_function("varispeed 1.0002, one 10 ms block", |b| {
        b.iter(|| {
            let mut source = input.iter().copied().cycle();
            let mut o = out.iter_mut();
            vs.produce(
                BLOCK,
                || source.next(),
                |y| {
                    if let Some(slot) = o.next() {
                        *slot = y;
                    }
                },
            );
            black_box(&out);
        })
    });
}

criterion_group!(benches, eq_chain, limiter, leveler, resampler, varispeed);
criterion_main!(benches);
