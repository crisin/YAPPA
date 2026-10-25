# ADR-010: Soundboard as a second track per participant, played from its own thread

Status: accepted (2026-10-25) · implements the "Soundboard" row of the planning doc's
"Quellen und Tracks" (ADR-000) · extends the seams of ADR-003

## Context

The planning doc decided the shape early: a sound plays at the clicking person as a track
of its own, so the others can turn it down per person; the clicker hears it too; no server
storage. Phase 3 on the board, but the user asked for a proof of concept now ("das wäre
lustig") after the first beta ran over the home server. What had to be decided for the
engine and the seams, with the audio block A1–A6 in place:

- **Where the sound mix comes from.** The pump is paced by the microphone and sleeps
  without one; a sound must play with or without a mic, and its timing must not depend on
  the capture device.
- **How it reaches the others and the clicker.** One stream, two destinations: the
  transport (a second track) and the own playout (the mixer).
- **How the receiving side tells the tracks apart** and keeps a volume per track, without
  the second track stealing the voice's mixer slot.
- **Which decoder.** The library is files the gang drops in — WAV, MP3, OGG, whatever a
  meme comes as.

## Decision

1. **A `yappa-sounds` thread** in `crates/audio-engine` (`soundboard.rs`) owns the clips.
   It paces itself: one 10 ms block every 10 ms, delivered one block ahead of real time
   (Windows timers are coarse unless something raised the resolution; Rust ≥ 1.75 sleeps
   with a high-resolution timer), and it sleeps on its channel while nothing plays. Up to
   eight clips overlap; the sum passes the soundboard gain (`Settings.soundboard_gain_db`)
   and a limiter (−1 dBFS) so overlapping clicks never clip the track. Decoding runs on a
   short-lived helper thread per file and never stalls a playing clip; decoded clips are
   cached (cut at 60 s, cache emptied above five minutes of audio).
2. **Two destinations per block.** The transport gets a second outgoing stream
   (`Transport::sound_out`, the same `VoiceOut` shape as the voice), published as the track
   `soundboard` with fixed options (`SOUNDBOARD_PUBLISH`: 96 kbit/s mono, DTX on, no RED)
   for the whole session, next to the voice. The mixer gets a second local slot of the
   monitor's kind (A5): a `SoundFeed` into an `rtrb` ring that the output callback drains
   like the monitor — no leveler, no per-person volume, master volume and limiter like
   everything else. Not publishing the track (an older server, a refused publish) is a
   warning, not the end of the call.
3. **The receiving side keys slots by (identity, track).** `AudioReceiver::speaker_started`
   takes a `transport::TrackKind`, read from the published track's name (`voice`,
   `soundboard`; anything unnamed is a voice from an older client). The mixer's routing and
   the remembered per-person mix are keyed the same way, so `SetPeerVolume { track:
   Soundboard }` lands on that track and the voice keeps its own. Soundboard slots skip the
   leveler (A4) — sound effects are not speech and would be pumped. `SLOTS` grows from 24
   to 48: a full channel is 19 others with two tracks each. Levels, "speaking" and the
   diagnosis stay voice-only. `VoiceStarted`/`VoiceEnded` are emitted for voice tracks only.
4. **Decoder: symphonia** (pure Rust, MPL-2.0, already allowed by `deny.toml`) with WAV,
   FLAC, Ogg Vorbis, MP3 and AAC/M4A. Stereo is averaged to mono, other rates go through
   the engine's own resampler (A1). Opus files are not read (no decoder in symphonia).
5. **The library is a folder.** `<app data>/soundboard/`, shared by every profile on the
   PC. The shell lists it (`soundboard_list`), copies a picked file in (`soundboard_add`,
   bytes base64 over IPC — a clip, not an album; 20 MB cap) and opens it in the file
   manager. No sync between clients: the planning doc's byte-stream sync by hash stays
   phase 3 — the UI says so.
6. **Protocol (additive):** `Command::PlaySound { path }`, `Command::StopSounds`,
   `Event::Sounds { playing }`, `Settings.soundboard_gain_db` (serde default 0, so a
   settings file from 0.2.0 loads), and the shell's `Sound { name, path }`. No protocol
   version bump.
7. **UI:** the voice module's inspector now picks by kind — person or soundboard. The
   soundboard inspector (search, grid, stop, my volume, add, open folder) opens from a
   button next to the room title and from the palette; the person inspector gets a
   soundboard volume and mute of its own. The browser mock has three sounds.

## Consequences

- The mixer loop walks 48 rings per callback instead of 24. An idle ring costs one length
  check; measured with 20 speakers playing (Ryzen 7 7700X, release): 42.4 µs per 10 ms
  block before, 36.9 µs after — the difference is run-to-run noise (A6 measured 37.3 on
  the same code as "before"), the 24 idle rings do not show.
- A client from 0.2.0 takes a `soundboard` track for a second voice of the same person
  (its slot map is keyed by identity alone): raise the server's minimum version when this
  ships.
- A soundboard track per participant is subscribed for the whole session. With DTX and no
  frames captured while idle, nothing is on the wire in between — to be confirmed on the
  first evening (`send_kbps` in the diagnosis with nobody clicking).
- The sound thread's clock and the output device's drift apart by a few hundred ppm; over
  a clip of seconds that is nothing, and the local slot's trim (at twice the cushion) covers
  a stalled machine. If the first evening shows clicks or gaps in the own playback, the
  slot gets the drift compensation of A6 — the speakers already have it.
- Not built, on purpose: hotkeys for sounds (the hook exists; a per-sound key table in the
  app settings is the next step), library sync between clients, per-sound volume, a
  "who is playing" marker on the tiles (the soundboard track's level is in the mixer, not
  yet in `Levels`), Opus files, a stereo soundboard track.
