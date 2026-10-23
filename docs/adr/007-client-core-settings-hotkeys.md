# ADR-007: `crates/client` holds the non-audio client logic; settings split into engine and app; hotkeys behind a platform trait

Status: accepted (2026-10-23) ·
amends ADR-003 (settings file, shell responsibilities)

## Context

Today the Tauri crate does everything that is not audio: settings file, logging, engine
wiring. Phase 1 adds login and session token, the API client, the presence link
(ADR-005), update checks, app settings and global hotkeys. Putting all of that into
`src-tauri` makes it untestable without a window and ties the headless example to half of it.

Two smells in the current contracts:

1. `engine_protocol::Settings` carries `theme`. The engine validates a UI colour scheme;
   the pro/simple view, PTT key, notifications and update channel would land there too.
2. Push-to-talk is an OS hook, not an engine concern — but the concept has the engine own
   `transmit_mode` end to end. `tauri-plugin-global-shortcut` uses `RegisterHotKey`, which
   delivers the hotkey to our window instead of the foreground game: the game would not see
   the PTT key.

## Decision

- **New crate `crates/client`** (no Tauri, no audio): `api` (reqwest client for
  `api-types`, cache for channels and config), `session` (session token and presence token
  in the OS credential store via `keyring`, file fallback), `presence` (drives
  `transport::PresenceLink`), `settings` (loads and saves both schemas, migrates), `updates`
  (reads `latest.json`, decides "update ready", never installs). The Tauri shell wires
  `client` + `audio-engine` + `platform-*` and exposes commands; the headless example uses
  the same login.
- **Two settings schemas, one file each:**
  - `engine_protocol::Settings` v2 — audio and sending only (`input_device`,
    `output_device`, `transmit_mode`, `vad`, `denoiser`, `high_pass_hz`, `eq`,
    `compressor`, `voice_bitrate_kbps`, `output_gain_db`). `theme` leaves. Migration v1 → v2
    in `client::settings`.
  - `client::AppSettings` v1 — `view` (simple/pro), `theme`, `hotkeys` (PTT, later
    soundboard), `notifications`, `update_channel`, window state. Exported to TS with ts-rs
    like the others (`cargo xtask gen-types`).
- **Hotkeys live behind `GlobalHotkey` in `crates/platform-windows` / `-macos`,** as the
  baseline's platform table says. Windows: low-level keyboard hook (`WH_KEYBOARD_LL`) or Raw
  Input, so the key still reaches the game; macOS: `CGEventTap` or, until then, hotkeys only
  while the app has focus. Not `tauri-plugin-global-shortcut`. The engine gets
  `Command::SetPttPressed { pressed }`; the shell translates hook events into it. S3
  verifies the mechanism against a real fullscreen game and writes the result.
- **Layering rule added:** `client` knows no audio; `transport` knows no DSP and no UI;
  `platform-*` know nothing but the OS.

## Consequences

- `PROTOCOL_VERSION` stays 1 (commands and events are additive); `SETTINGS_VERSION` becomes
  2 with a migration test.
- `api_types::Invite` (the pasted text) stays as the developer fallback; the user path is
  `yappa://invite/<code>?server=<API>` (`InviteLink` in api-types — the link must name the server, a fresh client knows none) via `tauri-plugin-deep-link` + `tauri-plugin-single-instance`.
- `YAPPA_PROFILE=<name>` picks a config and log subfolder, so two clients run on one PC.
- `keyring` 3.6 with `windows-native` + `apple-native` (C1: 4.x pulls the whole Linux
  D-Bus stack through its default feature);
  if it fails at runtime the token falls back to a file in the config dir with a warning.
- Revisit: if the hook is blocked by an anti-cheat in a game the gang plays, PTT degrades to
  "hotkey while focused" plus VAD — never breaks the call (platform rule).
