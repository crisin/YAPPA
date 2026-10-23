# ADR-009: Audio first — resampling, device return, loudness levelling, drift; D1 waits

Status: accepted (2026-10-23) · amends ADR-003 (known gaps become tasks) and the order of
phase 1 in `docs/01-vision-v1.md` section 9 (D1 moves behind the audio block)

## Context

The review before the first evening (`docs/02-review-2026-10-23.md`) listed the engine's
edges: only 48 kHz devices, a lost device is reopened once, no drift compensation, no global
mute keys. Which of them matter depends on the gang's hardware and habits, so the user was
interviewed (eleven questions, 2026-10-23). What came out:

| Question | Answer | Follows |
| --- | --- | --- |
| Hardware in the gang | USB/jack headsets, **Bluetooth headsets (mic and output)**, audio interfaces; nobody on speakers | Bluetooth mics run in hands-free mode: 16 kHz mono, "phone" quality. Today they are refused. Echo cancelling is not urgent. |
| Weakest PC | current gaming PCs (Ryzen 5/7, Core i5/i7 from ~2020) | DeepFilterNet stays the default without a safety net; an automatic fallback is nice-to-have. |
| Network | LAN and Wi-Fi, nobody on mobile/hotspot/abroad | No TURN needed now; RED stays, jitter buffer as is. |
| Default speaking mode with a headset | **Automatisch** (voice activation) | As the concept says; the gate behind DeepFilterNet carries it. |
| Chosen microphone vanishes in a call | switch to the default **and come back** when the device returns | Needs device-change notifications; cpal has none. |
| Everybody equally loud? | **yes, automatically** | A new DSP stage at the receiver, per speaker. |
| Push-to-talk | **hold only**, no toggle | Nothing changes; a hold tail is not wanted either. |
| Global keys for mute and deafen | **before the next evening** | Small: the hook exists. |
| Hear my own voice after the chain | **yes**, a button in the Profi view | Local monitor for tuning the chain, not for the call. |
| D1 (updater) before or after the audio work | **audio first**, installers by hand | D1 and U7 move behind the audio block. |

Options for the order were: D1 first (the review's recommendation — fixes reach the gang
without a new installer), audio first, or a minimal D1. The user chose audio first; the
cost is one installer file per version until D1.

## Decision

An audio block **A1–A6** goes onto the board between M3 and M4, in this order:

1. **A1 Resampling.** Capture and playout accept any device rate; the engine stays at
   48 kHz internally. A device at 44.1 kHz or a Bluetooth headset at 16 kHz (hands-free
   profile) joins instead of being refused. The device list and the onboarding say
   honestly that a Bluetooth microphone sounds like a phone — resampling makes it work,
   not good. Send-chain change: latency and CPU measured before and after, the resampler
   adds at most one block of latency on each side.
2. **A2 Device return.** A `DeviceWatcher` trait in `crates/platform` (Windows:
   `IMMNotificationClient`; macOS: CoreAudio property listeners; missing implementation =
   today's behaviour) feeds the engine: a lost device is retried, and when the chosen
   device comes back the engine switches back to it and says so.
3. **A3 Global mute and deafen keys** next to the push-to-talk key in „Tasten“, through
   the same hook; the dock shows the state.
4. **A4 Loudness levelling at the receiver.** A `dsp::Leveler` per mixer slot: short-term
   level over ≈ 400 ms, target around −20 dBFS, gain within ±12 dB, moves slowly (a few
   dB per second), does nothing below a noise floor (so silence is not pumped up). The
   manual per-person volume stays as an offset on top. Mixer bench before and after.
5. **A5 Monitor.** „Mich selbst hören“ in the Signalkette: the chain's output is fed into
   the playout mix as a local slot while the page is open; off when the page closes.
6. **A6 Drift compensation.** Hold each speaker's playout ring at the cushion with a
   gentle, continuous correction instead of the ≈ 80 ms skip (ADR-003 known gap).

Unchanged: DeepFilterNet default (ADR-008), voice activation default, push-to-talk by
holding only, RED on, no DTX, no TURN. Echo cancelling stays phase 2. The automatic
RNNoise fallback when the chain tears the 5 ms budget goes to „Später“.

## Consequences

- D1 and U7 wait until A1–A6 are through; every version in between reaches the gang as an
  installer file (`cargo xtask build`, `docs/mittesten.md`).
- A1 and A4 touch the send and receive paths: both get the measurement ritual of
  CLAUDE.md rule 5 and a realtime review before the merge.
- A2 is the first OS-specific code besides the hotkeys; it follows rule 3 (trait, one
  module per OS, absence degrades).
- Diagnosis gains the device's native rate and the leveler's current gain per speaker, so
  „too quiet“ and „phone quality“ can be read off a diagnostics file.
- Revisit triggers: someone in the gang on speakers (echo cancelling moves up); someone on
  mobile or abroad (TURN); a weak PC in the diagnostics (RNNoise fallback moves up); the
  first evening's findings override this order where they disagree.
