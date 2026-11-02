# ADR-012: One effect slot in the send chain — own effects first, CLAP plugins later through the same slot

Status: proposed (2026-11-02) · implements step 5 of the planning doc's "Sendekette"
(ADR-000) · answers the open decision "Plugin-Format zuerst" → CLAP · fed by spike S4
(`docs/spikes/s4-clap.md`)

## Context

The planning doc placed an "Effekt-Slot" in the send chain after EQ and compressor: own
effects (pitch, robot, reverb) first, CLAP/VST3 plugins through the same slot later. With
no test evening in sight (2026-11-02) the user chose to use the time for exactly this
experimental block. What had to be decided:

- **Where in the chain.** Before the limiter, so the limiter still catches what an effect
  adds (a hall's sum, a driven radio voice); after the compressor, so the compressor works
  on the clean voice and the effect's character is not levelled away. Before the gate would
  open the gate on a hall tail; after the limiter would clip.
- **One slot or a rack.** A rack (ordered list of effects) is what a DAW has; for a gaming
  voice chat one effect at a time is the use case ("mach mal Roboter"), and one slot keeps
  settings, UI and the latency sum trivial. A rack can replace the slot later without a
  protocol break (`effect` → `effects`).
- **Which own effects, which algorithms.** The budget is 5 ms per 10 ms block, the chain
  already spends ≈ 1.5 ms (DeepFilterNet). Options for pitch: a phase vocoder (good on
  tones, 20–40 ms latency, FFT), PSOLA (speech-grade, needs pitch tracking), or the
  delay-line shifter with two crossfaded heads (cheap, 15 ms, flutter, comb colouring on
  steady tones). Fun beats fidelity here: the delay-line shifter at 15 µs per block.
- **Plugins: CLAP or VST3, in- or out-of-process.** VST3's SDK is GPLv3 or a proprietary
  license — it does not fit `MIT OR Apache-2.0`. CLAP is MIT, `clack-host` 0.2 is MIT/Apache
  and, per S4, loads and runs a plugin from a DLL on Windows at ≈ 0.1 µs host cost per
  block. In-process means a plugin crash is a yAPPA crash; out-of-process means a host
  process and shared-memory audio (planning doc, phase 3).

| Option for the first step | Effort | Risk | Verdict |
| --- | --- | --- | --- |
| CLAP host straight into the engine, no own effects | medium | crash isolation, GUI, API churn ("0.2") — before anyone has a plugin | later |
| Own effects in `dsp`, one `Effect` seam in the chain | small | none beyond taste | **first — this ADR** |
| Both at once | large | two unknowns in one review | no |

## Decision

1. **The slot.** `chain.rs` gets a stage `effect` between `compressor` and `limiter`
   (`STAGES` = 8). An `Effect` enum wraps the `dsp` blocks; a kind change rebuilds the
   instance on the pump (allocation at reconfigure time, never per block), a parameter
   change reaches the running instance without a rebuild (no click). `latency_samples()`
   is the denoiser's plus the effect's.
2. **Own effects, pure `dsp`, no I/O:** `PitchShifter` (two-head delay line, 30 ms window,
   ±12 semitones, latency 15 ms), `Robot` (ring modulator 60 Hz + feedback comb 120 Hz),
   `Reverb` (Freeverb mono, tunings scaled to 48 kHz, fixed room, mix knob), `Radio`
   (300 Hz–3.4 kHz band + rational-tanh drive). Every `process` is allocation-free, tames
   NaN and denormals, and sits under the limiter.
3. **Protocol, additive:** `Settings.effect: EffectSettings { kind: VoiceEffect, semitones,
   amount }` with `#[serde(default)]` — old settings files load with the slot off; no
   `PROTOCOL_VERSION` or `SETTINGS_VERSION` bump. `validate` bounds semitones to ±12 and
   amount to 0..=1 (NaN fails both).
4. **UI:** the stage "Effekt" in the Profi signal chain with a select and one slider;
   "Mich selbst hören" is the way to hear it; glossary entry "Stimmeffekt".
5. **Plugins:** CLAP only, via `clack-host`, as `crates/plugin-host` behind the same
   `Effect` seam — a later task ("Plugins round 1", TASKS.md). In-process first; the
   out-of-process host is phase 3. VST3 is not built (license). Plugin GUIs get their own
   spike.

## Consequences

- Easier: a new own effect is one `dsp` module, one enum arm and one option in the UI.
  The plugin host slots in without touching the chain's shape.
- Harder: the pitch shifter adds 15 ms to the chain while on — the diagnosis shows it; the
  mouth-to-ear budget (70–120 ms) still holds with ≈ 60 ms measured digital path. A
  plugin's latency will add the same way.
- The receive side has no effect slot (per-person "Spaß-Effekt" of the planning doc):
  deliberately not built — it would multiply the cost by the number of speakers and nobody
  asked for it.
- Revisit when: someone wants two effects at once (rack), a plugin crashes an evening
  (out-of-process host earlier), or the pitch flutter annoys (PSOLA / phase vocoder with a
  larger budget).
