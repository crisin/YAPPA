# Changelog

One line per finished task (definition of done). Newest first.

## Unreleased

- UI concept v0 (lofi: shell with module rail, context sidebar, stage, voice dock, topbar + command palette, banner slot, inspector; call screen, onboarding, settings Einfach/Profi, admin; mobile parked) in `docs/ui/`, plus the implementation plan `docs/ui/02-umsetzungsplan.md` — work packages U0–U7 on the board. Mobile apps moved from non-goal to "later" in the planning doc. The in-app help now also renders `docs/ui/`.
- In-app help: the "Hilfe" button shows the repo's docs inside the app — tester guide, "So funktioniert yAPPA", glossary (new `docs/hilfe/`), and the project side (status = TASKS.md, changelog, ADRs, spikes, dev setup, home server). Rendered at build time by `cargo xtask docs` into a generated, committed JSON; search, links between pages, web links in the system browser. Glossary entries double as ⓘ tooltips in the debug panel and on labels.
- Dependencies: `rustls` 0.23.46 / `rustls-webpki` 0.103.16 and `webrtc-sys-build` 0.3.20 (pulls `zip` 9 instead of 0.6) — clears the RUSTSEC advisories that had turned `cargo deny` red in the gate.
- Test client: the app joins a room with a pasted invite and carries real voice — new crates `transport` (trait + LiveKit) and `audio-engine` (devices, mixer with per-person volume, levels, automatic rejoin), debug panel with live network and device statistics, structured log files with one-click diagnostics export, `cargo xtask livekit token` prints invites, headless engine example. ADR-003, docs/mittesten.md.
- Home server (stage 1): `infra/livekit/home.yml` with LiveKit + Caddy/TLS, keys from `.env`, `cargo xtask livekit token <name>` for join tokens, `yappa-poc --token`; German step-by-step guide in docs/homeserver.md (FRITZ!Box, Linux VM).
- S1 PoC: `yappa-poc` (scratch crate `crates/spike-s1`) — cpal ↔ LiveKit Rust SDK, click latency, packet loss, reconnect; result in docs/spikes/s1-poc.md. Windows builds now use the static C runtime (`.cargo/config.toml`), needed by libwebrtc.
- S1 prep: local LiveKit in Docker (`infra/livekit/dev.yml`) and `cargo xtask livekit up|down|logs|loss <%>|cut <s>` — packet loss and a cut link via tc/netem; `doctor` checks for the MSVC 2022 toolset.
- Logo (for now): app icons, sidebar logo and favicon from the cat picture.
- Dev and production build: `cargo xtask dev [--ui]`, `cargo xtask build [--debug]` (installers), npm `app:*` scripts, release/dev profiles, apps/desktop README.
- Dev setup: `cargo xtask setup|doctor`, pinned toolchain, nextest/cargo-deny/insta/proptest/criterion, vitest + prettier, git hooks, Claude Code hooks/skills/agent — see docs/dev-setup.md.
- S0 CI: `.github/workflows/check.yml` — `cargo xtask check --ci` on Windows + macOS (active once pushed).
- S0 skeleton: Cargo workspace, `cargo xtask check|gen-types|tokens`, `engine-protocol` + `api-types` with generated TS, `dsp` biquad, ui-tokens, Tauri 2 + Svelte 5 desktop shell with engine stub.
- ADR-002: alternatives to Rust (C++/JUCE, Go, .NET, Electron; PocketBase, TS, Elixir for the control plane) checked — Rust stays.
- Repository created as variant B from the planning baseline (docs/00-grobstruktur.md); stack differences in ADR-001.
