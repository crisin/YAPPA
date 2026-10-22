# Architecture Decision Records

One page per decision: **Context** (forces, options), **Decision**, **Consequences**. Number
them consecutively (`003-short-title.md`). A decision in the "Offene Entscheidungen" table of
the planning doc gets an ADR when it switches to "Entschieden".

| ADR | Title | Status |
| --- | --- | --- |
| 000 | [Grobstruktur v0](../00-grobstruktur.md) — planning baseline (shared with variant A) | accepted (2026-10-01) |
| 001 | [Variant B — same core, Rust-first stack, outside App Hub](001-variante-b-stack.md) | accepted (2026-10-07) |
| 002 | [Rust stays — alternatives checked](002-language-alternatives.md) | accepted (2026-10-07) |
| 003 | [Engine seams as built, pasted invites, logging](003-engine-seams-and-test-client.md) | accepted (2026-10-10) |
| 004 | [Client UI — one shell, modules in a registry, headless components, settings as a module](004-client-ui-architecture.md) | accepted (2026-10-23) |
| 005 | [Presence and live data over a LiveKit presence room; control plane HTTP only](005-presence-over-livekit.md) | accepted (2026-10-23) |
| 006 | [Updates and remote config from our own box; repo on GitHub](006-distribution-updates.md) | accepted (2026-10-23) |
| 007 | [`crates/client`, settings split engine/app, hotkeys behind a platform trait](007-client-core-settings-hotkeys.md) | accepted (2026-10-23) |

Decided in ADR-000 already: project name yAPPA. ADR-001 overrides ADR-000 where it says so
(control plane, UI framework, scripts, work tracking). ADR-004 to 007 come from the
2026-10-23 review and are summarised in [`docs/01-vision-v1.md`](../01-vision-v1.md), which
wins over ADR-000 and ADR-001 for phase 1.
