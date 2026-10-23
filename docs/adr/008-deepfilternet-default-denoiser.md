# ADR-008: DeepFilterNet 3 (low-latency) is the default denoiser, built from a pinned libDF

Status: accepted (2026-10-23) · decides "Denoiser-Default" of the planning doc's open
decisions; follows the gate set in the vision (section 2) and measured in spike S2

## Context

The vision made DeepFilterNet the default only if the whole chain adds ≤ 25 ms and costs
≤ 5 ms CPU per 10 ms block; otherwise RNNoise. Spike S2 (`docs/spikes/s2-dsp.md`) measured
on a Ryzen 7 7700X, release build, one thread:

| | RNNoise (nnnoiseless) | DeepFilterNet 3 LL (libDF + tract) |
| --- | --- | --- |
| CPU per block, mean / p99 / max | 31 / 38 / 216 µs | 646 / 1 420 / 1 930 µs |
| Latency (claimed = cross-correlation) | 10 ms | 10 ms |
| Noise floor of noisy speech at 0 dB SNR, denoiser only | −40.5 → −73.9 dBFS | −40.5 → digital silence in pauses |
| Speech level change | −0.7 dB | −1.5 dB |

DeepFilterNet passes both gates with room. It comes with three costs:

1. **Build:** libDF is not usable from crates.io (`deep_filter` 0.2.5 there is years old).
   The git version (commit `d375b2d`) only compiles against tract **0.21.4** — tract 0.21.10
   changed its API inside the minor version.
2. **Advisories:** tract 0.21.4 brings RUSTSEC-2026-0217 (overflow in the NNEF tensor
   parser on a crafted model) and two "unmaintained" notices (anymap2 at build time,
   paste).
3. **Not `Send`:** tract's state holds `Rc`s, so a DeepFilterNet instance cannot move
   between threads, and it allocates per block.

## Decision

- **Default denoiser: DeepFilterNet 3 low-latency**, model embedded in the binary,
  `crates/denoise` feature `deepfilter` (on for the desktop app). RNNoise stays the
  setting for weak machines and the fallback whenever DeepFilterNet cannot load.
- **libDF pinned** to commit `d375b2d8309e0935d165700c91da9de862a99c31`, tract pinned to
  `=0.21.4` in `crates/denoise`.
- **The three advisories are accepted** in `deny.toml` with reasons: we never parse NNEF
  and never load a model from outside the binary, so the vulnerable path is not reachable
  with attacker data; the other two are maintenance notices without a known flaw.
- **The denoiser lives on the pump thread** and is built there: the engine hands over a
  `DenoiserKind`, the pump creates the `Denoiser` (trait without `Send`). Never in a device
  callback — DeepFilterNet allocates.

## Consequences

- E1 wires `denoise` into the pump; the pump's busy-time stat (E0) shows the cost live,
  the diagnosis warns above 5 ms.
- A clean release build takes ≈ 3 minutes longer (tract); CI caches it.
- The advisory exceptions must not outlive their reason. **Revisit trigger:** a libDF
  version that builds on tract ≥ 0.21.16 — or, failing that, our own small patch of libDF
  for the newer tract API (Later task). Then the `ignore` entries are removed.
- Weaker CPUs: 0.65 ms mean is ≈ 6.5 % of one core here; if the gang's slowest PC cannot
  carry it, the Profi view's denoiser switch (RNNoise) is the answer, not a lower default.
