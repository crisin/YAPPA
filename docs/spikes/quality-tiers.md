# Q: Voice quality tiers — result

2026-10-25 · Windows PC (AMD Ryzen 7 7700X, Windows 11 Home 10.0.26300) · branch
`task/audio-quality-tiers`

**Short version:** five profiles from 32 to 510 kbit/s are selectable, a change during a
call publishes the voice track again inside the same session (no rejoin), and the
diagnosis shows what the encoder really aims at. The LiveKit Rust SDK 0.9 lets bitrate,
channels, RED and DTX through; Opus frame length (20 ms), in-band FEC (on) and complexity
(9) are fixed in libwebrtc and shown as facts. The profiles cost nothing measurable on the
send chain — the encoder lives inside libwebrtc, outside the pump. Mouth-to-ear is
unchanged per profile within the noise of the click measurement; the send rate scales as
expected and RED stops working above about 200 kbit/s; libwebrtc caps Opus at about 256 kbit/s
whatever is asked for.

## Question

The first internet call (2026-10-24: 0 % loss, jitter 6–8 ms, 80 kbit/s with RED) sounded
"almost identical to Discord". To compare on purpose, the user wants to turn the quality up
in a stress test. So:

1. Which Opus parameters can the app actually set through the SDK?
2. What does each profile cost — send rate, CPU per 10 ms block, mouth-to-ear?
3. Does a profile change survive a running call?

## What the SDK lets through

| Parameter     | Settable? | How                                                                            | Note                                                                                                                                                                |
| ------------- | --------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bitrate       | yes       | `AudioEncoding { max_bitrate }`                                                | asked up to 510 kbit/s; libwebrtc's encoder stops at ≈ 256 kbit/s (measured below), presumably its default `maxaveragebitrate`, which the SDK does not let us raise |
| Mono / stereo | yes       | `NativeAudioSource::new(…, channels, …)`; the SDK adds `stereo=1` to the offer | the source's channel count is fixed at creation, so the transport keeps one mono and one stereo source                                                              |
| RED           | yes       | `TrackPublishOptions::red`                                                     | libwebrtc drops the copy once primary + copy exceed 1 kB (`kRedMaxPacketSize`) — with 20 ms frames above ≈ 200 kbit/s RED is silently off (measured below)          |
| DTX           | yes       | `TrackPublishOptions::dtx`                                                     |                                                                                                                                                                     |
| Frame length  | **no**    | —                                                                              | libwebrtc: 20 ms (`ptime`); `RtpParameters::ptime` is `pub(crate)`                                                                                                  |
| In-band FEC   | **no**    | —                                                                              | always `useinbandfec=1` in libwebrtc's offer                                                                                                                        |
| Complexity    | **no**    | —                                                                              | libwebrtc default 9; no SDK or field-trial path                                                                                                                     |
| Sample rate   | n/a       | —                                                                              | Opus works at 48 kHz internally; the chain runs at 48 kHz (A1); more buys nothing                                                                                   |

A bitrate change without a republish would need `RtpSender::set_parameters`; the sender is
`pub(crate)` behind `LocalAudioTrack::transceiver()`, so every profile change is
unpublish + publish of the voice track. The others hear a gap of a few hundred
milliseconds; the session, the room and the presence stay.

## Profiles

| Profile  | Bitrate             | Channels | RED     | DTX |
| -------- | ------------------- | -------- | ------- | --- |
| Sparsam  | 32                  | mono     | on      | off |
| Standard | 80                  | mono     | on      | off |
| Hoch     | 128                 | mono     | on      | off |
| Studio   | 192                 | mono     | on      | off |
| Stress   | 510 (≈ 256 reached) | stereo   | **off** | off |

The brief asked for „Studio“ at 256 kbit/s stereo. Measured, that is the same thing as
„Stress“: RED no longer fits into the packet from about 200 kbit/s on, and libwebrtc caps
the Opus encoder at about 256 kbit/s whatever is asked for — so 256 stereo with RED and
510 stereo without RED both send ≈ 255–259 kbit/s of payload. „Studio“ therefore became
192 kbit/s mono with RED, the highest step at which the redundancy still works; „Stress“
keeps the 510 request so the diagnosis shows the cap („Ausgehandelte Bitrate“ turns
orange). Any other combination is one click away in Profi → Netzwerk („Eigen“). Stereo means dual mono: the microphone path stays mono, the transport puts the
same block on both channels (`interleave_dual_mono`, a stack buffer, no allocation on the
pump thread). Opus encodes two identical channels almost as cheaply as one (the side
signal is zero), so "Studio" and "Stress" measure the cost of stereo, not a stereo mic.

## Setup

- **Server:** a second dev LiveKit (`livekit/livekit-server:v1.13.8`, `--dev`) on ports
  17880/17881/17882, because the user's home server occupies 7880 on this machine (WSL2,
  mirrored network). Clients on the same machine.
- **Client:** `yappa-poc measure` (`crates/spike-s1`, now with `--stereo`), 30 s per
  profile, one click every 500 ms over a −30 dBFS noise floor. Digital path only — no
  device buffers, as in S1.
- **Bench:** `cargo xtask bench` before and after; the transport gained a bench for the
  dual-mono interleave.

## Measurements

### Per profile (digital path, 30 s, 59 clicks each, 0 packets lost)

| Request              | Channels | RED | Payload sent                    | Click latency median · p95 (ms) | Jitter buffer | `capture_frame` mean · max per block |
| -------------------- | -------- | --- | ------------------------------- | ------------------------------- | ------------- | ------------------------------------ |
| 32 kbit/s (Sparsam)  | mono     | on  | ≈ 60 kbit/s                     | 57.5 · 64.8                     | 39 ms         | 13.9 · 128.8 µs                      |
| 80 kbit/s (Standard) | mono     | on  | ≈ 162 kbit/s                    | 41.8 · 51.6                     | 24 ms         | 15.3 · 322.3 µs                      |
| 128 kbit/s (Hoch)    | mono     | on  | ≈ 257 kbit/s                    | 47.7 · 48.5                     | 32 ms         | 9.7 · 208.4 µs                       |
| 192 kbit/s (Studio)  | mono     | on  | ≈ 384 kbit/s                    | 50.2 · 51.0                     | 39 ms         | 9.8 · 131.0 µs                       |
| 256 kbit/s           | mono     | on  | ≈ 255 kbit/s — **RED gone**     | 50.6 · 51.3                     | 30 ms         | 8.8 · 181.4 µs                       |
| 256 kbit/s           | stereo   | on  | ≈ 255 kbit/s — RED gone         | 47.7 · 48.4                     | 30 ms         | 7.4 · 30.2 µs                        |
| 510 kbit/s           | mono     | off | ≈ 259 kbit/s — **capped**       | 45.8 · 46.4                     | 30 ms         | 9.7 · 208.3 µs                       |
| 510 kbit/s (Stress)  | stereo   | off | ≈ 259 kbit/s — capped           | 46.9 · 47.6                     | 30 ms         | 7.6 · 28.2 µs                        |
| 510 kbit/s           | stereo   | on  | ≈ 259 kbit/s — capped, RED gone | 48.0 · 48.6                     | 30 ms         | 7.4 · 53.1 µs                        |

"Payload sent" is `bytesSent` of the outbound RTP stream over the run (RTP headers
included, ≈ 5–6 kbit/s at 50 packets/s). Reading:

- **RED doubles the payload exactly up to 192 kbit/s** (80 → 162, 128 → 257, 192 → 384)
  and vanishes at 256: a 20 ms frame of 640 bytes plus its copy is above libwebrtc's 1 kB
  RED limit. Hence Studio = 192.
- **libwebrtc caps the encoder at ≈ 256 kbit/s.** 510 mono, 510 stereo, 256 mono all send
  ≈ 255–259 kbit/s. The SDK offers no way to raise it (no `maxaveragebitrate` fmtp, no
  sender parameters). The app's „Stress“ therefore really is 256 kbit/s stereo, and the
  diagnosis says so (Ausgehandelte Bitrate).
- **Latency does not depend on the profile.** Medians sit at 42–58 ms on the digital path
  (S1 measured 51–62 ms), within the jitter buffer's settling; the 32 kbit/s run was the
  first after a server start and carried the largest buffer. The frame length is 20 ms in
  every profile, so nothing changes mouth-to-ear.
- **Stereo costs nothing on the pump.** `capture_frame` per 10 ms block stays at 7–15 µs
  mean across all runs (the stereo runs are the lowest; the variation is run noise). The
  Opus work for a second channel is done inside libwebrtc's encoder thread.

A first round of the same runs with the Vite dev server and a cargo build running next to
it showed 80–260 ms medians and a jitter buffer up to 150 ms — the click measurement is only
meaningful on a quiet machine.

### Send chain bench (`cargo xtask bench`, before → after)

| Bench                                        | Before   | After    |
| -------------------------------------------- | -------- | -------- |
| mixer: 20 speakers, one 10 ms block          | 11.07 µs | 10.87 µs |
| 5-band EQ, one 10 ms block                   | 5.35 µs  | 5.29 µs  |
| limiter, one 10 ms block                     | 0.87 µs  | 0.84 µs  |
| resampler 16 → 48 kHz, one block             | 10.60 µs | 10.37 µs |
| resampler 48 → 16 kHz, one block             | 11.35 µs | 10.37 µs |
| resampler 44.1 → 48 kHz, one block           | 11.57 µs | 10.90 µs |
| transport: mono → dual mono, one block (new) | —        | 23 ns    |

The DSP benches are unchanged within noise; the dual-mono interleave for a stereo profile
costs 23 ns per block, 0.0005 % of the 5 ms budget.

## Decision

- The five profiles ship as above; Standard stays the default (the migration maps an old
  80 kbit/s file to it, any other bare bitrate to „Eigen“).
- No 96 kHz option — explained in the help instead (glossary: Abtastrate, Sprachqualität).
- Frame length, FEC and complexity are reported as fixed values, not stored as settings;
  if a later SDK exposes them, the schema grows then.

## Not answered

- Mouth-to-ear with real devices per profile (`measure --acoustic` needs the loopback
  cable, S1 rest).
- How the Stress profile behaves over the real home server with a second person — the
  user's stress test against Discord is the next step; the diagnosis (Ausgehandelte
  Bitrate, Senderate, Paketverlust) is what to read off.
- IP fragmentation: a 510 kbit/s 20 ms frame is ≈ 1 275 bytes plus headers, above the
  usual 1 200-byte WebRTC packet limit — whether the path to the home server carries
  that without loss is part of the stress test.
