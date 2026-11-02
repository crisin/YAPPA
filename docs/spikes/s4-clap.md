# S4: CLAP hosting with clack-host — result

2026-11-02 · AMD Ryzen 7 7700X, Windows 11 Home 10.0.26300, rustc 1.97.1, `--release` ·
branch `task/effects-poc` on top of `a84d0b7` · code: `crates/spike-s4` (scratch)

## Question

The planning doc (ADR-000, "Plugins") says: CLAP first, via `clack-host`, in-process to
begin with. Before the effect slot of the send chain (ADR-012) promises a plugin, the spike
asks:

1. Does `clack-host` 0.2.0 (crates.io, MIT/Apache) load a CLAP plugin from a file on
   Windows and pass the chain's 10 ms mono blocks through it?
2. What does one block cost on the host side — is the wrapping of buffers and the call
   cheap enough for a per-block effect slot?
3. What does a host have to ask a plugin so it can sit in a mono chain at 48 kHz: ports,
   latency, thread rules?
4. Is the API stable enough for a `crates/plugin-host` behind the `Effect` seam?

## Setup

- `crates/spike-s4`: a `lib` + `cdylib` crate. The library holds a minimal CLAP effect
  written with `clack-plugin` ("yAPPA Spike Gain": one mono in, one mono out, output = input
  × 0.5, declares `audio-ports` and `latency`), exported as `clap_entry`, so the compiled
  `spike_s4.dll` is a real CLAP bundle. The same library holds the host side: load an entry
  (from disk with `unsafe { PluginEntry::load }`, or linked in with `load_from_clack`), list
  the factory's plugins, instantiate one, read ports and latency, activate for 48 kHz /
  480 frames, start processing, process blocks on one thread, stop and deactivate in order.
- `s4-host` binary: `cargo run --release -p spike-s4 --bin s4-host [-- <file.clap> [id]]`
  prints what the host learns and the cost over 3 000 blocks of a 440 Hz tone.
- No third-party `.clap` is installed on this machine (`C:\Program Files\Common Files\CLAP`
  does not exist; only two VST3 bundles are there). The DLL built from this crate stands in
  as the foreign file; a VST3 bundle was fed in as the negative case.
- Unit tests (`cargo test -p spike-s4`): description, ports, latency, −6 dB through the
  plugin, unknown id is an error, a stereo host layout against the mono plugin.

## Measurements

| What | Value | Method |
| --- | --- | --- |
| Load + instantiate the built-in entry, list, ports, latency | works; 1 plugin, Mono In/Out (main), latency 0 | `s4-host` without arguments |
| Load the same plugin from `target/release/spike_s4.dll` | works, identical answers | `s4-host target/release/spike_s4.dll` — real `LoadLibraryExW` + `clap_entry` |
| Host cost per 10 ms block, built-in | mean 0.10 µs · p99 0.30 µs · max 0.80 µs | 3 000 blocks, `Instant` around `process` |
| Host cost per 10 ms block, from DLL | mean 0.08 µs · p99 0.10 µs · max 1.60 µs | same |
| Signal through the plugin | RMS 0.3567 → 0.1784 = −6.0 dB, as the plugin promises | last block of the run |
| A VST3 bundle as input | refused: "Failed to load plugin descriptor library: LoadLibraryExW failed" — an error, no crash | `s4-host DecentSampler.vst3` |
| A third-party CLAP effect | **not measured** — none installed; the user's follow-up | — |
| Plugin GUI | **not tried** — out of scope for S4 (needs a native window handle, own spike) | — |

For scale, the own effects of ADR-012 on the same machine (`cargo xtask bench`, one 10 ms
block): pitch shifter 15.4 µs, hall 6.1 µs, robot 2.7 µs, radio 1.5 µs; the 5-band EQ
5.7 µs, the limiter 0.9 µs. The budget is 5 ms per block.

## What worked / what did not

Worked:

- `clack-host` 0.2.0 compiles on stable (`rust-version` 1.85), no C toolchain beyond what
  the workspace already needs; `clap-sys` 0.5 underneath. CLAP 1.2.2.
- The whole host side is safe Rust except one line: `PluginEntry::load(path)` — loading
  foreign code is `unsafe` by definition. The scratch crate allows `unsafe_code` for that;
  nothing else needed it.
- The buffer model fits the chain: `AudioPorts::with_capacity` once, then
  `with_input_buffers` / `with_output_buffers` per block wrap our own `Vec<f32>`s without
  allocating. `InputEvents::empty()` and `OutputEvents::void()` for an effect without
  parameters. The processor (`StartedPluginAudioProcessor`) is a separate object from the
  instance, meant to move to the audio thread — that is exactly the pump / main split the
  engine has.
- Thread rules are encoded in the types: port and latency queries need the main-thread
  handle (`plugin_handle()`), processing the started processor; mixing them up does not
  compile.
- Errors are errors: a wrong id, a non-CLAP file, a plugin that refuses to instantiate all
  come back as `Result`s.

Did not / caveats:

- A plugin that panics or corrupts memory inside `process` takes the pump thread with it —
  in-process hosting has no isolation. clack catches panics from *our* handler code, not
  from the plugin's.
- `load_from_clack` needs the `clack-plugin` feature of `clack-host`; useful for tests, not
  for production.
- The API is "0.2": the README's example and the published crate differ in small things
  (`ClapVersion` has fields, not methods; the entry is `PluginEntry`, not `PluginBundle`).
  Expect renames on the next minor.
- Mono vs. stereo: the plugin says what it wants via `audio-ports`; a host that feeds two
  channels to a mono plugin still gets the first channel processed (tested), but a stereo
  effect in a mono chain needs the voice duplicated on both sides and one side kept — the
  spike does that already (`Running::process`).
- Latency: the extension reports samples; the chain's `latency_samples()` would add it,
  like the pitch shifter's today. A plugin that changes latency at runtime calls
  `request_restart` — the host must then deactivate / reactivate; the spike's host ignores
  those requests.

## Decision it feeds

→ ADR-012 (effect slot, own effects first, CLAP later through the same slot): **hosting is
feasible and cheap; the risk is isolation and API churn, not performance.** CLAP stays the
only plugin format (VST3's SDK license does not fit MIT/Apache; ADR-000 open decision
"Plugin-Format zuerst" → CLAP). VST2 is dead, AU is macOS-only.

## Follow-up tasks (TASKS.md, "Later")

- Plugins round 1: `crates/plugin-host` (the `host.rs` of this spike, made a crate) behind
  the `Effect` seam of the chain; a plugin path and id in the settings; load and port/latency
  queries on the engine's command thread, the processor handed to the pump; a plugin crash
  is a crash of yAPPA until round 2 (out-of-process host, shared memory).
- Test with a real third-party CLAP effect (e.g. a free one from Surge XT Effects or
  Airwindows) — the user, when one is at hand.
- Plugin GUIs: own spike (native window handle from Tauri, main-thread timer callbacks).
