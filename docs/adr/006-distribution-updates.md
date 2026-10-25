# ADR-006: Updates, channels and remote config come from our own box; the repo lives on GitHub

Status: accepted (2026-10-23) ·
amends ADR-000 ("Repo öffentlich, Nightly über GitHub Releases")

## Context

The baseline ties three things together: the repo is public, nightly builds are GitHub
Releases, the Tauri updater reads a `latest.json` from there, and minimum version and
kill switch come from the control plane. Two of those are blocked today: the repo is local
(no CI runs), and going public waits on the logo rights question. The updater itself is the
most important tool during development ("Ziel vom Merge bis zur Gang: unter 20 Minuten").

`tauri-plugin-updater` 2.x accepts any HTTPS endpoint: a static JSON (`version`, `notes`,
`pub_date`, `platforms.<target-arch>.{url, signature}`) or a dynamic one with
`{{target}}`/`{{arch}}`/`{{current_version}}` placeholders. It sends no credentials, so a
private GitHub release does not work as an endpoint.

## Decision

- **The box that runs LiveKit and the control plane also serves updates.** Caddy serves
  `https://<domain>/updates/<channel>/latest.json` and the installers next to it, from a
  directory the release step uploads to. Channels: `nightly`, `beta`, `stable`. Artifacts
  are signed with the Tauri minisign key (private key only in CI secrets / the release
  machine, public key in `tauri.conf.json`).
- **Remote config** (`GET /config`: `min_version`, `killed_versions`, `update_base_url`,
  `livekit_ws_url`) is the control plane's. The client reads it at start and hourly. A
  killed or too-old version is forced out of the call and shown the update banner; the
  control plane also refuses its token requests (`version_too_old`).
- **Client behaviour:** check at start and hourly, download in the background, never install
  during a call — on next start or by click, with "Was ist neu" from the changelog. The
  previous installer stays local for rollback.
- **Release step is `cargo xtask release <channel>`:** version from `Cargo.toml`, changelog
  cut, `tauri build`, sign, write `latest.json`, upload (scp/rsync to the box). First from
  the dev PC, later from CI on every green `main` build (nightly) and on tags (beta/stable).
- **The repo lives on GitHub** (`crisin/YAPPA`, public, default branch `main`; the user's
  choice, 2026-10-23). CI (`check.yml`, Windows + macOS) runs there. Nothing in the
  distribution path depends on GitHub; the update host is our box. Being public makes the
  no-secrets rule binding and leaves the logo rights as the one open item.

## Consequences

- Three stages, one compose set (user decision 2026-10-23): local dev, mini PC at home,
  VPS. The VPS is the always-reachable update host; `home.yml` and a new `vps.yml` share
  the services and differ only in `.env` and overrides; both get an `updates` volume and
  a Caddy route before M4.
- `tauri-plugin-updater`, `tauri-plugin-process` (restart) added in D1; the endpoint URL is
  built from `update_base_url` + channel, so moving the box is a config change.
- GitHub Releases can still mirror stable builds later; they are not the source of truth.
- Revisit trigger: more than one group/instance (not a goal) — then a neutral update host.

## Amendment 2026-10-25 (D1): GitHub Releases first, the box second

The home server that exists today (WSL2 behind a FRITZ!Box with CGNAT) is reachable over
IPv6 only; a gang member without IPv6 would get no updates from it. The repo is public by
now, so a GitHub release carries the installers and `latest.json` without credentials.
User decision: **updates are delivered through GitHub Releases by default, the box is the
second target.** `cargo xtask release <channel>` uploads to GitHub unless
`YAPPA_RELEASE_TARGET` names a folder or an scp path; the versioned release `v<version>`
holds the installers, the rolling release `<channel>` holds the manifest, so the endpoint
keeps the shape `<update_base_url>/<channel>/latest.json` with
`update_base_url = https://github.com/crisin/YAPPA/releases/download` (the default in
`home.yml`). Caddy's `/updates/` route and the `updates` folder stay for the box as the
second target; nothing else in the decision changes. GitHub thus is in the distribution
path after all — accepted for now; the revisit trigger is a reachable VPS of our own.
