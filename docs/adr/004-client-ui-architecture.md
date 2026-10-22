# ADR-004: Client UI — one shell, modules in a registry, headless components, settings as a module

Status: accepted (2026-10-23) ·
builds on ADR-001 (Svelte 5), decides the UI concept `docs/ui/01-ui-system-lofi-v0.md`

## Context

The UI concept (round 1) settles a shell with seven zones (module rail, context sidebar,
stage, voice dock, topbar with command palette, system banner, inspector) and modules that
live in it (Voice today; Hub, Games, Tools later; Admin by role). The test client is one
Svelte file. Four things had to be decided before the shell is built:

1. How modules plug into the shell without the shell knowing them.
2. Which component library, if any. Candidates: hand-rolled, Bits UI (headless, Svelte 5,
   MIT, 2.20), shadcn-svelte (copies Bits UI components with Tailwind styling).
3. Whether settings are a fullscreen overlay (concept) or a module.
4. How the UI reaches data that is not the engine: channels, members, presence.

## Decision

- **Module registry.** A module is a plain object: `id`, label, icon, required role, sidebar
  component, stage component, commands for the palette. The shell renders the registry
  (`shell.svelte.ts` holds active module, inspector, banner queue, palette state). The voice
  dock, topbar, banner and inspector belong to the shell. A new feature is a new entry; the
  shell does not change.
- **Bits UI + Lucide.** Headless parts (Command, Dialog, Select, Slider, Tabs, Tooltip,
  Popover) from `bits-ui` 2.x, icons from `@lucide/svelte`. All styling from `ui-tokens`;
  no Tailwind, no shadcn copy. Rejected: hand-rolled (keyboard and screen-reader behaviour
  of a palette and a select are not where the project earns anything), shadcn-svelte (brings
  Tailwind and a look).
- **Settings are the module „Ich“.** Sidebar = settings navigation with the Einfach/Profi
  switch, stage = the section. Rail and voice dock stay visible, so mute is reachable during
  a call; `Esc` returns to the previous module. Help lives in the same module. Onboarding
  stays fullscreen: it runs before the first call.
- **Two doors.** `engine.ts` (engine-protocol) and `api.ts` (api-types + app settings).
  Both go through Tauri IPC; the webview never speaks HTTP or LiveKit itself. Both have mocks
  for the browser and tests; the mock API ships a fixture „Die Gang“ and scenes (in call,
  reconnecting, offline, update ready).
- **Feature slots are hidden, not greyed.** Soundboard and app audio appear when the engine
  reports the capability. No "coming soon" buttons.
- **Accessibility is tested.** `vitest-axe` on every screen test; states as text, not only
  colour; every icon-only button has a label.
- **Round-1 questions:** join on single click and Enter (hover shows „Beitreten“; a click on
  a person opens the inspector); inspector closed by default, remembered per module.

## Consequences

- U0 adds `bits-ui`, `@lucide/svelte`, `vitest-axe` to `apps/desktop`; `ui-tokens` grows
  (widths, type scale, state colours, focus, motion). Theme stays a JSON file.
- The UI never waits for the control plane: U1–U4 run against the mock API; U5 swaps the
  door.
- The concept doc keeps "overlay" in its decision table; this ADR overrides it. The lofi
  round 2 draws against the running shell.
- Revisit: if Bits UI's Command palette cannot be styled to the tokens without fighting it,
  replace that one part, not the library.
