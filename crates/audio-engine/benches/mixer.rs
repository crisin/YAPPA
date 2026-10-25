//! The receive mix in the output callback: 20 speakers (a full evening) into one 10 ms
//! block. Every iteration feeds each speaker one frame and renders one block, so the rings
//! stay at the cushion like in a call. Run with `cargo xtask bench`.
//!
//! The second case adds the monitor slot (A5, „Mich selbst hören“): the own chain output
//! fed every block as well — what the switch costs the callback while it is on.

use audio_engine::mixer::mixer;
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use transport::{FrameSink, TrackKind, BLOCK};

const SPEAKERS: usize = 20;

fn render_20_speakers(c: &mut Criterion) {
    for monitor in [false, true] {
        let (control, mut rt) = mixer();
        let mut sinks: Vec<_> = (0..SPEAKERS)
            .map(|i| {
                control
                    .speaker(&format!("p{i}"), TrackKind::Voice)
                    .expect("free slot")
            })
            .collect();
        // A voice-like signal near full scale, so the sum needs the clamp / limiter.
        let frame: Vec<i16> = (0..BLOCK)
            .map(|i| ((i as f32 * 0.07).sin() * 20_000.0) as i16)
            .collect();
        let own: Vec<f32> = frame.iter().map(|s| f32::from(*s) / 32_767.0).collect();
        let mut feed = control.monitor_feed().expect("the feed");
        control.set_monitor(monitor);
        for sink in &mut sinks {
            for _ in 0..3 {
                sink.frame(&frame);
            }
        }
        for _ in 0..3 {
            feed.block(&own);
        }
        let mut out = vec![0f32; BLOCK];
        let name = if monitor {
            "mixer: 20 speakers + monitor, one 10 ms block"
        } else {
            "mixer: 20 speakers, one 10 ms block"
        };
        c.bench_function(name, |b| {
            b.iter(|| {
                for sink in &mut sinks {
                    sink.frame(&frame);
                }
                feed.block(&own);
                out.fill(0.0);
                rt.render(black_box(&mut out));
                black_box(&out);
            });
        });
    }
}

criterion_group!(benches, render_20_speakers);
criterion_main!(benches);
