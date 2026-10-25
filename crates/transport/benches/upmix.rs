//! What a stereo profile costs on the pump thread per 10 ms block: the mono → dual-mono
//! interleave before the hand-over to the encoder. Run with `cargo xtask bench`.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use transport::{interleave_dual_mono, BLOCK};

fn upmix(c: &mut Criterion) {
    let block: [i16; BLOCK] = std::array::from_fn(|i| ((i as f32 * 0.07).sin() * 20_000.0) as i16);
    let mut out = [0i16; 2 * BLOCK];
    c.bench_function("transport: mono -> dual mono, one 10 ms block", |b| {
        b.iter(|| {
            interleave_dual_mono(black_box(&block), &mut out);
            black_box(&out);
        })
    });
}

criterion_group!(benches, upmix);
criterion_main!(benches);
