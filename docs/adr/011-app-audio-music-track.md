# ADR-011: App audio as the third track per participant, captured behind an `AppCapture` seam

Status: proposed (2026-10-27) · implements the "music" row of the planning doc's "Quellen
und Tracks" (ADR-000) · extends ADR-003 (seams) and ADR-010 (second track)

## Context

The planning doc foresaw a `music` track per participant for "Anwendung oder
Ausgabe-Loopback": the sound of one application — a game, a music player, a browser tab —
into the channel, stereo, no speech processing. After the soundboard (ADR-010) proved the
second track, the user asked for this as the next step: sharing a clip or a stream today
means a second program next to the voice chat. What had to be decided:

- **Where the capture lives.** Capturing one application's sound is OS work (WASAPI
  process loopback on Windows, ScreenCaptureKit on macOS 13+), with COM and `unsafe`;
  the engine must stay free of both, and a missing implementation must hide the feature,
  not break the build (CLAUDE.md rule 3). Three places were on the table:

  | Where | Dependency direction | Verdict |
  | --- | --- | --- |
  | trait and OS code in `audio-engine` (like cpal in `device.rs`) | none new | `unsafe` and COM in the engine crate |
  | trait in `platform`, engine depends on it | `audio-engine` → `platform` (rdev, hooks) | the engine would pull the hotkey hook |
  | trait in `audio-engine`, OS code in `platform`, the shell adapts | none between the two | **chosen** — the pattern of the transport: the shell is where the pieces meet |

- **Which thread paces the stream.** The soundboard paces itself (a file has no clock).
  A capture has one: the audio engine's event every 10 ms. A second clock would drift
  against it and need a ring with trim.
- **Mono or stereo on the wire.** The transport's voice seam is mono blocks; the planning
  doc says stereo for music. The receive mix (`mixer.rs`) is mono today.
- **Whether the sender hears it at home.** The soundboard gets a local mixer slot so the
  clicker hears the clip. An application already plays on the sender's PC; a copy
  through the mixer would arrive 20–40 ms later and comb-filter it.
- **How the others see it.** The soundboard flag (`PeerLevel.sound`) is read off the
  track's level, not from presence.

## Decision

1. **Seam:** `audio_engine::appaudio::AppCapture` — `available()`, `list()` (the
   applications with a sound session: pid, name, active), `start(pid, sink)` returning a
   handle whose drop stops the capture thread and waits for it. The sink
   (`CaptureSink`) gets interleaved `f32` at the capture's own rate and channel count,
   plus `ended(reason)` when the capture dies. `NoCapture` is the empty implementation
   (tests, headless, every OS without one). The shell's `appaudio::OsCapture` adapts
   `platform::appaudio` to it; the engine is spawned with it like with `AudioIo`.
2. **Windows: WASAPI process loopback** in `crates/platform/src/appaudio.rs`:
   `ActivateAudioInterfaceAsync` on `VAD\Process_Loopback` with
   `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`, include-tree of the chosen process,
   event-driven shared-mode stream asked for in 48 kHz stereo float (so nothing is
   converted) with 44.1 kHz 16-bit stereo as the fallback format. The capture thread
   `yappa-app-audio` reads every packet and calls the sink; it polls the process handle
   once a second so a closed application ends the stream with a reason (the loopback
   itself goes on in silence). The session list comes from `IAudioSessionManager2` on
   every active render endpoint, one line per process, the own process and the system
   sounds left out. Process loopback exists from Windows 10 build 20348 (Windows 11,
   Server 2022): `available()` checks the build number; older builds hide the feature.
   macOS: `available()` is false, the UI hides the button.
3. **The capture thread is the pump.** `AppStream` (the engine's sink) converts to
   48 kHz with the A1 resampler (one per channel, only when the rate differs), applies
   the app audio gain (`Settings.app_audio_gain_db`, −30..=12 dB, read from an atomic so
   a slider move reaches the running stream) and a limiter per channel (−1 dBFS), and
   hands every 480-frame block to the transport as interleaved stereo. No local slot:
   the sender hears the application directly.
4. **Transport:** `TrackKind::Music` ("music"), `StereoOut` next to `VoiceOut`
   (`2 * BLOCK` interleaved samples), `Transport::music_out()`, published once per
   session next to the voice with `MUSIC_PUBLISH` (192 kbit/s, stereo, no RED, no DTX;
   not having it is a warning, not the end of the call). The receiving side keys the
   slot by (identity, Music), downmixed to mono by the SDK — a stereo playout is the
   follow-up, the sender is already right for it. `SLOTS` 48 → 72: 19 others with
   three tracks each, plus room for a resubscribe.
5. **Receiving side:** like a soundboard track — per-person volume and mute
   (`SetPeerVolume { track: music }`), no leveler. `PeerLevel.music` from
   `Mixer::music_levels` with the `speaking` hold: "streamt" on the card, in the person
   inspector, as a marker in the channel list. The application's name is not carried to
   the others (that would be a presence attribute, ADR-005) — "streamt App-Audio" is
   what they see.
6. **Protocol (additive):** `Command::ListApps`, `StartAppAudio { pid, name }`,
   `StopAppAudio`; `Event::Apps { available, apps }`, `Event::AppAudio { running, app }`;
   `AppSource { pid, name, active }`; `Settings.app_audio_gain_db` (serde default 0);
   `PeerLevel.music` (serde default false). No version bump.
7. **UI:** inspector "App-Audio" next to the soundboard (list of applications, playing
   ones first; a click streams; "Stopp"; "Neu einlesen"; my volume), the button only
   where `Event::Apps.available` is true; the person inspector gets an app audio volume
   and mute. Browser mock: two applications, the third person of the raid streams.

## Consequences

- A fourth kind of OS integration in `crates/platform` with its own `unsafe` module;
  the engine crate stays free of COM.
- The music track is subscribed for the whole session like the soundboard track; with
  no frames captured while idle, nothing is on the wire — to be confirmed on the first
  evening (`send_kbps` with nobody streaming). While streaming, 192 kbit/s plus headers
  go up from the sender: a weak upload notices.
- An older client (0.2.x) takes a `music` track for a second voice of the same person
  (its slot map is keyed by identity): raise the server's minimum version when this ships
  — the same note as ADR-010.
- The mixer walks 72 rings per callback instead of 48; an idle ring is one length
  check. The send chain (microphone) is untouched: no pump change, no bench needed
  (rule 5).
- Windows 10 users (build < 20348) do not get the button. If somebody needs it there,
  the fallback is a whole-device loopback (`AUDCLNT_STREAMFLAGS_LOOPBACK` on the render
  endpoint — everything the PC plays, voice chat included); not built.
- Not built, on purpose: a stereo receive mix, the application's name for the others,
  video, system-wide loopback, per-app volume for the listeners beyond the per-track
  one, a hotkey for start/stop, macOS capture (ScreenCaptureKit).
- Revisit when: the first evening shows the idle track costing bandwidth (then publish
  the music track only while streaming), or somebody streams music and wants it in
  stereo at the listeners (then the mixer goes stereo — the output callback already
  writes every channel).
