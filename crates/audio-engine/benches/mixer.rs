//! The receive mix in the output callback: 20 speakers (a full evening) into one 10 ms
//! block. Every iteration feeds each speaker one frame and renders one block, so the rings
//! stay at the cushion like in a call. Run with `cargo xtask bench`.

use audio_engine::mixer::mixer;
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use transport::{FrameSink, BLOCK};

const SPEAKERS: usize = 20;

fn render_20_speakers(c: &mut Criterion) {
    let (control, mut rt) = mixer();
    let mut sinks: Vec<_> = (0..SPEAKERS)
        .map(|i| control.speaker(&format!("p{i}")).expect("free slot"))
        .collect();
    // A voice-like signal near full scale, so the sum needs the clamp / limiter.
    let frame: Vec<i16> = (0..BLOCK)
        .map(|i| ((i as f32 * 0.07).sin() * 20_000.0) as i16)
        .collect();
    for sink in &mut sinks {
        for _ in 0..3 {
            sink.frame(&frame);
        }
    }
    let mut out = vec![0f32; BLOCK];
    c.bench_function("mixer: 20 speakers, one 10 ms block", |b| {
        b.iter(|| {
            for sink in &mut sinks {
                sink.frame(&frame);
            }
            out.fill(0.0);
            rt.render(black_box(&mut out));
            black_box(&out);
        });
    });
}

criterion_group!(benches, render_20_speakers);
criterion_main!(benches);
