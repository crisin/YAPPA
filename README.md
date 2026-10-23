# yAPPA · variant B

> yAPPA: Appa aus Avatar, und yappen ist, was wir da tun.

Self-hosted gaming voice chat as a replacement for Discord and Steam voice — for one group of
up to 20, on own hardware, with voice quality and robustness first.

yAPPA is built twice from the same plan. This repository is **variant B**:

| | Variant A | Variant B (this repo) |
| --- | --- | --- |
| Where | `App-Hub/projects/yappa`, GitHub `crisin/yappa` | `D:\Projekte\dev\YAPPA`, local |
| Workflow | App Hub board, worktree per item, review lane | Claude Code directly, `TASKS.md`, branch per task |
| Client | Tauri 2, native Rust audio engine, UI by spike S5 | Tauri 2, native Rust audio engine, **Svelte 5** |
| Control plane | NestJS + Prisma on Railway | **Rust (axum + sqlx + SQLite)** next to LiveKit |
| Media plane | self-hosted LiveKit | self-hosted LiveKit |
| Tooling | npm workspaces + Node scripts | **Cargo workspace + `cargo xtask`** |

Why and what follows from it: [ADR-001](docs/adr/001-variante-b-stack.md).

## Docs

- [docs/01-vision-v1.md](docs/01-vision-v1.md) — vision and architecture for phase 1 (German);
  wins over the baseline where they differ
- [docs/00-grobstruktur.md](docs/00-grobstruktur.md) — the planning baseline (German, ADR-000)
- [docs/ui/](docs/ui/01-ui-system-lofi-v0.md) — UI concept (lofi) and implementation plan (German)
- [docs/adr/](docs/adr/README.md) — architecture decisions
- [docs/spikes/](docs/spikes/README.md) — spike goals and results
- [docs/dev-setup.md](docs/dev-setup.md) — tooling, tests, Claude Code setup
- [docs/homeserver.md](docs/homeserver.md) — LiveKit on a home server behind a FRITZ!Box (German);
  [docs/cloudflare-tunnel.md](docs/cloudflare-tunnel.md) — the same behind a Cloudflare Tunnel
- [docs/mittesten.md](docs/mittesten.md) — for testers: install, join, report a problem (German)
- [docs/hilfe/](docs/hilfe/glossar.md) — user help and glossary (German); everything in `docs/`,
  `TASKS.md` and `CHANGELOG.md` is also readable inside the app (module „Ich“ → Hilfe)
- [TASKS.md](TASKS.md) — what is being worked on
- [CHANGELOG.md](CHANGELOG.md) · [CLAUDE.md](CLAUDE.md) — rules for coding agents

## Status

Before the first evening with the gang (2026-10-23): invite links open the app, onboarding
in three steps (name, microphone with a live level, how you talk), channel tree with who is
where, one-click join, the voice chain (high-pass, DeepFilterNet, voice activation or
push-to-talk that works in a game, EQ, compressor, limiter), settings in „Einfach“ and
„Profi“ with the signal chain, admin in the app (invites, channels, roles, versions, event
log), diagnosis with export, in-app help. Server: LiveKit + control plane + Caddy in one
compose for a home server, or behind a Cloudflare Tunnel; one command for a local server.
Not yet: auto-update (installers by hand), tray, echo cancelling, devices that are not at
48 kHz. Review of where things stand: [docs/02-review-2026-10-23.md](docs/02-review-2026-10-23.md).
Why Rust and not something else: [ADR-002](docs/adr/002-language-alternatives.md).

## Quickstart

Prerequisites: Rust via rustup (the version is pinned in `rust-toolchain.toml` and installed
automatically), Node 20+, the Tauri 2 prerequisites (Windows: WebView2 + MSVC Build Tools;
macOS: Xcode Command Line Tools), Docker Desktop for local LiveKit from S1 on.

```bash
cargo xtask setup     # once per machine: dev tools, npm deps, git hooks, then a tool report
cargo xtask check     # the gate: format, lint, types, all tests, licenses
```

## Develop

```bash
cargo xtask dev       # desktop app with hot reload (Vite for the UI, Rust rebuilds on change)
cargo xtask dev --ui  # UI only in the browser (http://localhost:1420), engine is mocked
cargo xtask test      # Rust + UI tests
```

## Build (production)

```bash
cargo xtask build          # release build + installers for the OS you are on
cargo xtask build --debug  # same, debug profile (faster, for testing the installer)
```

Windows produces `target/release/bundle/nsis/*-setup.exe` (per-user install, no admin) and
`target/release/bundle/msi/*.msi`; macOS produces `target/release/bundle/macos/*.app` and
`target/release/bundle/dmg/*.dmg`. Builds are unsigned for now — SmartScreen / Gatekeeper
warn once. Signing, update channels (nightly/beta/stable) and the updater come with D1 (phase 1, M4).

All commands, tools, test kinds and the Claude Code setup: [docs/dev-setup.md](docs/dev-setup.md).
The same commands also exist as npm scripts in [apps/desktop](apps/desktop/README.md).
