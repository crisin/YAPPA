# S2: The send chain offline — result

2026-10-23 · Windows PC (AMD Ryzen 7 7700X, Windows 11 Home 10.0.26300) · release build ·
on top of `main` after E0, branch `task/s2-dsp`

**Short version:** the whole chain — high-pass, denoiser, gate, EQ, compressor — costs
41 µs per 10 ms block with RNNoise and 0.66 ms with DeepFilterNet 3 (low-latency model),
far below the 5 ms budget. Both denoisers add exactly 10 ms. DeepFilterNet passes the gate
of the vision and becomes the default (ADR-008); getting it to build cost a pinned old
tract and three accepted advisories. Nobody has listened to the results yet.

## Question

From the spike list and the vision (section 2):

1. What does each stage cost per 10 ms block (mean, p99, max)?
2. What latency does each denoiser add — as it claims, and as measured?
3. How much noise does each take away, and how much speech does it cost?
4. Does DeepFilterNet stay the default: chain ≤ 25 ms added latency and ≤ 5 ms CPU per
   block?

## Setup

- **Input:** `assets/noisy_snr0.wav` from the DeepFilterNet repository (commit `d375b2d`):
  10.6 s of speech in noise at 0 dB SNR, mono, 48 kHz, 16 bit. Downloaded into `target/s2/`,
  not committed.
- **Chain, block by block as the pump will run it** (`crates/spike-s2`, scratch):
  biquad high-pass 80 Hz → denoiser → `dsp::Gate` (threshold -50 dBFS, 6 dB hysteresis,
  300 ms hold, fed with the denoiser's speech probability) → two peaking EQ bands
  (-2 dB @ 300 Hz, +3 dB @ 3.5 kHz) → `dsp::Compressor` (-18 dB, 3:1, 5/120 ms, +3 dB
  makeup, 6 dB knee).
- **Denoisers** (`crates/denoise`): off; RNNoise via `nnnoiseless` 0.5.2 (pure Rust);
  DeepFilterNet 3 low-latency via libDF `d375b2d` + tract 0.21.4, model embedded.
- **Timing:** `Instant` around each stage per block, 1 059 blocks; one thread.
- **Latency:** the lag (0–50 ms) at which the denoiser's output correlates best with its
  input over the whole file.
- **Levels:** block RMS over the file; noise floor = 10th percentile, speech = 95th.

## Measurements

### CPU per 10 ms block (µs: mean / p99 / max)

| Stage | off | RNNoise | DeepFilterNet |
| --- | --- | --- | --- |
| HPF | 1.2 / 1.3 / 2.4 | 1.2 / 1.2 / 3.8 | 1.2 / 1.2 / 1.5 |
| denoiser | 0.0 / 0.1 / 1.1 | 31.3 / 38.2 / 158.6 | 646.6 / 1 420.1 / 1 764.0 |
| gate | 0.9 / 1.2 / 11.1 | 0.9 / 1.2 / 1.8 | 0.9 / 1.2 / 1.8 |
| EQ (2 bands) | 2.4 / 2.6 / 28.5 | 2.3 / 3.1 / 7.1 | 2.4 / 2.7 / 18.0 |
| compressor | 5.2 / 7.9 / 25.4 | 4.8 / 7.7 / 49.1 | 4.6 / 7.6 / 12.0 |
| **total** | 10.1 / 14.4 / 78.4 | 40.9 / 49.2 / 165.5 | 656.4 / 1 432.9 / 1 771.3 |

A second run gave the same picture (DeepFilterNet mean 645 µs, max 1.93 ms; RNNoise mean
31 µs, max 215 µs).

### Latency and levels

| Denoiser | Model load | Latency claimed | Latency measured | Denoiser only: noise floor / speech | Whole chain: noise floor / speech |
| --- | --- | --- | --- | --- | --- |
| input | | | | -40.5 / -17.9 dBFS | |
| off | — | 0 | 0 | -43.2 / -18.3 dBFS (HPF) | -40.4 / -17.6 dBFS |
| RNNoise | 0 ms | 10.0 ms | 10.0 ms | -73.9 / -18.6 dBFS | -107.4 / -17.8 dBFS |
| DeepFilterNet | 395–415 ms | 10.0 ms | 10.0 ms | silence in pauses / -19.4 dBFS | silence / -18.3 dBFS |

- With the denoiser off the noise (-40.5 dBFS) sits above the gate threshold, so the gate
  stays open — at 0 dB SNR a plain gate cannot help, which is why the denoiser goes first.
- "Whole chain" floors are mostly the gate closing in the pauses; the denoiser-only column
  is the fair comparison.
- Not measured: how it **sounds**. The three results are in `target/s2/{off,rnnoise,
  deepfilter}.wav` for listening; a 0 dB SNR file is a hard case, gaming noise (keyboard,
  fan) is untested. Also not measured: other CPUs, macOS.

### Building it

| What | Result |
| --- | --- |
| `deep_filter` from crates.io | 0.2.5, years behind the repository — not used |
| libDF from git (`d375b2d`) + current tract 0.21.10 | does not compile (tract changed its API within 0.21) |
| libDF + tract pinned to `=0.21.4` (as in libDF's own Cargo.lock) | builds; clean release build ≈ 3 min |
| `cargo deny` with tract 0.21.4 | RUSTSEC-2026-0217 (NNEF parser overflow), RUSTSEC-2026-0319 (anymap2 unmaintained), RUSTSEC-2024-0436 (paste unmaintained) — accepted with reasons, ADR-008 |
| `DfTract` | not `Send` (`Rc` inside tract) and allocates per block |

## What worked / what did not

**Worked**

- Both denoisers fit the engine's unit exactly: 480 samples in, 480 out, 10 ms delay.
- The gate profits from the denoiser's speech probability: loud non-speech stays out,
  quiet speech keeps it open (unit tests in `dsp::gate`).
- Everything except the denoiser costs about 10 µs per block together.

**Did not work, or needs care**

- DeepFilterNet's dependency situation (pinned old tract, advisories) — see ADR-008 for the
  way back.
- DeepFilterNet cannot be moved between threads: the pump must build it itself.
- Its p99 is twice its mean (1.4 ms vs 0.65 ms) — fine against a 10 ms period, but the
  pump's busy-time stat must keep showing it on real machines.
- RNNoise on synthetic white noise only takes ≈ 4.5 dB (unit test) — it is trained on real
  noise; synthetic test signals say little about denoiser quality.

## Decision it feeds

- **Gate passed → DeepFilterNet 3 LL is the default denoiser** (ADR-008), RNNoise the
  setting and fallback.
- Send-chain latency budget: denoiser 10 ms + the rest ≈ 0 (no lookahead anywhere) — well
  inside the 25 ms the vision allowed.

## Follow-up tasks (also in TASKS.md)

- E1: the chain in the pump (denoiser built on the pump thread), `Event::StageLevels`,
  chain CPU and latency in `Stats`.
- Listen (user): `target/s2/*.wav` after `cargo run --release -p spike-s2 --
  target/s2/noisy_snr0.wav`; record a gaming evening's noise (keyboard, fan, game audio
  through speakers) and run it through.
- Later: libDF on tract ≥ 0.21.16 (patch or upstream), then drop the three advisory
  exceptions.
