//! S2 spike (scratch, `publish = false`): the send chain offline, block by block as the
//! pump will run it, against a WAV file — so it can be measured without devices and heard
//! afterwards.
//!
//!   cargo run --release -p spike-s2 -- <input.wav> [out-dir]
//!
//! For each denoiser (off, RNNoise, DeepFilterNet): HPF 80 Hz → denoiser → gate (with the
//! denoiser's speech probability) → 2-band EQ → compressor. Writes `<out>/<denoiser>.wav`
//! and prints markdown tables: CPU per stage and block, denoiser latency (claimed and by
//! cross-correlation), noise floor and speech level in and out.

use denoise::{Denoiser, BLOCK};
use dsp::{Biquad, BiquadKind, Compressor, CompressorParams, Gate, GateParams};
use engine_protocol::DenoiserKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

const SR: f32 = 48_000.0;

fn read_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    if spec.sample_rate != 48_000 || spec.channels != 1 {
        return Err(format!(
            "{}: need mono 48 kHz, got {spec:?}",
            path.display()
        ));
    }
    match spec.sample_format {
        hound::SampleFormat::Int => Ok(reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / (1i64 << (spec.bits_per_sample - 1)) as f32))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?),
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string()),
    }
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32_767.0) as i16)
            .map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

#[derive(Default)]
struct Timing {
    ns: Vec<u64>,
}

impl Timing {
    fn add(&mut self, since: Instant) {
        self.ns.push(since.elapsed().as_nanos() as u64);
    }
    /// (mean, p99, max) in µs.
    fn summary(&self) -> (f64, f64, f64) {
        let mut v = self.ns.clone();
        v.sort_unstable();
        let n = v.len().max(1);
        let mean = v.iter().sum::<u64>() as f64 / n as f64;
        let p99 = v.get(n * 99 / 100).copied().unwrap_or(0) as f64;
        let max = v.last().copied().unwrap_or(0) as f64;
        (mean / 1e3, p99 / 1e3, max / 1e3)
    }
}

struct Run {
    name: &'static str,
    claimed_latency: usize,
    load_ms: f64,
    stages: Vec<(&'static str, Timing)>,
    denoised: Vec<f32>,
    out: Vec<f32>,
}

fn run(kind: DenoiserKind, input: &[f32]) -> Run {
    let started = Instant::now();
    let mut denoiser: Box<dyn Denoiser> = denoise::open(kind);
    let load_ms = started.elapsed().as_secs_f64() * 1e3;
    let mut hpf = Biquad::new(BiquadKind::HighPass, SR, 80.0, 0.707);
    let mut gate = Gate::new(SR, GateParams::default());
    let mut eq = [
        Biquad::new(BiquadKind::Peaking { gain_db: -2.0 }, SR, 300.0, 1.0),
        Biquad::new(BiquadKind::Peaking { gain_db: 3.0 }, SR, 3_500.0, 1.0),
    ];
    let mut comp = Compressor::new(SR, CompressorParams::default());
    let names = ["HPF", "denoiser", "gate", "EQ", "compressor", "total"];
    let mut timings: Vec<Timing> = names.iter().map(|_| Timing::default()).collect();
    let mut denoised = Vec::with_capacity(input.len());
    let mut out = Vec::with_capacity(input.len());
    for chunk in input.chunks_exact(BLOCK) {
        let mut block = [0f32; BLOCK];
        block.copy_from_slice(chunk);
        let all = Instant::now();
        let t = Instant::now();
        hpf.process(&mut block);
        timings[0].add(t);
        let t = Instant::now();
        let speech = denoiser.process(&mut block);
        timings[1].add(t);
        denoised.extend_from_slice(&block);
        let t = Instant::now();
        gate.process(&mut block, speech);
        timings[2].add(t);
        let t = Instant::now();
        for f in eq.iter_mut() {
            f.process(&mut block);
        }
        timings[3].add(t);
        let t = Instant::now();
        comp.process(&mut block);
        timings[4].add(t);
        timings[5].add(all);
        out.extend_from_slice(&block);
    }
    Run {
        name: denoiser.name(),
        claimed_latency: denoiser.latency_samples(),
        load_ms,
        stages: names.into_iter().zip(timings).collect(),
        denoised,
        out,
    }
}

/// Lag (samples) at which `output` best matches `input`, searched up to `max_lag`.
fn latency_by_correlation(input: &[f32], output: &[f32], max_lag: usize) -> usize {
    let n = input.len().min(output.len()).saturating_sub(max_lag);
    (0..=max_lag)
        .map(|lag| {
            let c: f64 = (0..n)
                .step_by(2)
                .map(|i| f64::from(input[i]) * f64::from(output[i + lag]))
                .sum();
            (lag, c)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |(lag, _)| lag)
}

/// (noise floor, speech level) in dBFS: the 10th and 95th percentile of block RMS.
fn levels(samples: &[f32]) -> (f32, f32) {
    let mut db: Vec<f32> = samples
        .chunks_exact(BLOCK)
        .map(|b| dsp::gain_to_db(dsp::rms(b)))
        .collect();
    db.sort_by(f32::total_cmp);
    let pick = |q: usize| db.get(db.len() * q / 100).copied().unwrap_or(-120.0);
    (pick(10), pick(95))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let input_path = PathBuf::from(
        args.first()
            .ok_or("usage: spike-s2 <input.wav> [out-dir]")?,
    );
    let out_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| input_path.parent().unwrap_or(Path::new(".")).to_path_buf());
    let input = read_wav(&input_path)?;
    println!(
        "input {} — {:.1} s, {} blocks",
        input_path.display(),
        input.len() as f32 / SR,
        input.len() / BLOCK
    );

    let runs: Vec<Run> = [
        DenoiserKind::Off,
        DenoiserKind::Rnnoise,
        DenoiserKind::DeepFilter,
    ]
    .into_iter()
    .map(|kind| run(kind, &input))
    .collect();

    println!("\n### CPU per 10 ms block (µs: mean / p99 / max)\n");
    println!(
        "| Stage | {} |",
        runs.iter().map(|r| r.name).collect::<Vec<_>>().join(" | ")
    );
    println!("| --- |{}", " --- |".repeat(runs.len()));
    for (i, (stage, _)) in runs[0].stages.iter().enumerate() {
        let cells: Vec<String> = runs
            .iter()
            .map(|r| {
                let (mean, p99, max) = r.stages[i].1.summary();
                format!("{mean:.1} / {p99:.1} / {max:.1}")
            })
            .collect();
        println!("| {stage} | {} |", cells.join(" | "));
    }

    let (in_floor, in_speech) = levels(&input);
    println!("\n### Latency and levels\n");
    println!(
        "| Denoiser | load | latency claimed | latency measured | denoiser only: floor / speech | whole chain: floor / speech | file |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    println!("| input | | | | {in_floor:.1} / {in_speech:.1} dBFS | | |");
    for r in &runs {
        let measured = latency_by_correlation(&input, &r.denoised, 2_400);
        let (floor, speech) = levels(&r.out);
        let (d_floor, d_speech) = levels(&r.denoised);
        let file = out_dir.join(format!("{}.wav", r.name));
        write_wav(&file, &r.out)?;
        println!(
            "| {} | {:.0} ms | {} smp ({:.1} ms) | {} smp ({:.1} ms) | {d_floor:.1} / {d_speech:.1} dBFS | {floor:.1} / {speech:.1} dBFS | {} |",
            r.name,
            r.load_ms,
            r.claimed_latency,
            r.claimed_latency as f32 / 48.0,
            measured,
            measured as f32 / 48.0,
            file.display()
        );
    }
    Ok(())
}
