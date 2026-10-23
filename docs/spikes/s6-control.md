# S6: Control plane in Rust — result

2026-10-23 · Windows PC (AMD Ryzen 7 7700X, Windows 11 Home 10.0.26300) · on top of
`5da15d1`, branch `task/s6-control`

**Short version:** the control plane works as one small Rust binary, built in well under a
day; the PocketBase fallback from ADR-002 is not needed. The presence room of ADR-005
carries "who is where": a channel switch reaches the others in about a millisecond (on this
machine), a late joiner sees everybody's state the moment it connects, a kick lands in
10 ms, and the token grants hold. The one weak spot is a client that dies without saying
goodbye: the others see it as online for 20–23 s.

## Question

From the spike list and ADR-005:

1. Does axum + sqlx/SQLite + `livekit-api` carry invites, members, channels, tokens and the
   event log within the time box (≤ 3 days), or does the PocketBase fallback apply?
2. Does presence over LiveKit participant attributes work — speed of a change, state for a
   late joiner, how fast a leave, a crash and a kick show?
3. Do the two token kinds hold their limits: presence token = no media, voice token = no own
   attributes?
4. Do LiveKit's signed webhooks reach the event log?

## Setup

- **Server:** `livekit/livekit-server:v1.13.8` in Docker Desktop (`cargo xtask livekit`),
  `--dev`, dev keys; webhook to `http://host.docker.internal:8090/livekit/webhook` (new in
  `infra/livekit/livekit.yaml`).
- **Control plane:** `crates/control` (`yappa-control`), axum 0.8.9, sqlx 0.9 (SQLite,
  WAL, migrations embedded), `livekit-api` 0.8.2 (tokens, webhook verification, RoomService),
  started in-process by the spike on `127.0.0.1:8090` with a fresh SQLite file.
- **Clients:** `crates/spike-s6` (scratch), `livekit` 0.9.4 Rust SDK, `--release`; four
  people (Krille admin, Ben, Sasha, Tim) log in with invites over HTTP (`reqwest`), then
  join the room `presence` with their presence tokens, `auto_subscribe` off, no media.
  The "killed" client runs in a child process that the spike terminates.
- Everything on one machine: the numbers are LiveKit's processing plus the SDK, **not** an
  internet path.

## Measurements

Two runs; where they differ both values are given.

| What | Value | Method |
| --- | --- | --- |
| Login per invite (`POST /auth/invite`) | 3.4 – 3.5 ms | HTTP round trip incl. SQLite insert and two JWTs, loopback |
| Connect to the presence room | 83 – 98 ms | `Room::connect`, no media |
| Attribute change A → B (a channel switch) | median 1.1 · p95 2.6 – 2.8 · max 2.8 ms (20 of 20) | `set_attributes` until `ParticipantAttributesChanged` at B, changes 500 ms apart |
| Late joiner sees the current state | yes — channel, talk mode and away present when `connect` returns | `remote_participants()` of a third client right after `Room::connect` |
| Clean leave seen by the others | 2.0 s | `Room::close` until `ParticipantDisconnected` at B (second run; see below) |
| Client killed without goodbye | 20.0 – 23.0 s | process terminated until `ParticipantDisconnected` at B |
| Kick via API reaches the client | 9 – 10 ms | `POST /members/{id}/kick` (RoomService `remove_participant`) until `RoomEvent::Disconnected` at the kicked client |
| Presence token refuses media | yes, but only after a 10.0 s timeout | `publish_track` with the presence token |
| Voice token refuses own attributes | yes, at once: `NotAllowed - does not have permission to update own metadata` | `set_attributes` in a channel room |
| Webhooks in the event log | 10 lines (online, offline, joined, left), signature checked | counted after the run |
| `yappa-control` image | 140 MB, release build in the container ≈ 60 s | `docker build -f infra/control/Dockerfile` |
| API test suite | 9 integration + 6 unit tests | `cargo test -p control`, in-memory SQLite, no network |

Not measured: an internet path between clients and server, more than four clients
(`lk load-test` stays on the board), macOS.

## What worked / what did not

**Worked**

- **The stack.** axum + sqlx + `livekit-api` needed no workaround: tokens with exact
  grants, webhook verification (`WebhookReceiver`), and RoomService for the kick are all in
  the crate. Plain runtime queries — no database at build time. One binary that is also
  the bootstrap and emergency tool (`invite --admin`, `policy --min … --kill …`).
- **Bootstrap without a web page.** The first start of an empty database writes a one-time
  admin invite link to the log. Whoever can read the server log owns the box anyway, so
  the "tiny bootstrap page" of the baseline is not built.
- **Presence as attributes.** Fast, ordered, and the server hands late joiners the full
  state — the client needs no snapshot request and the control plane no presence table.
- **Token limits.** Both kinds are enforced by LiveKit itself, not by our client.
- **Webhooks** arrive signed and become readable German lines ("Ben betritt lobby").

**Did not work, or needs care**

- **A dead client stays "online" for 20–23 s.** That is LiveKit's own timeout for a peer
  without a goodbye (same order as the ≈ 14 s the S1 client needed to notice a dead link).
  For presence it means a crashed or unplugged PC is shown for half a minute. Acceptable
  for v1; the UI may grey a person out whose voice has gone silent — see follow-up.
- **A join and a leave in one server update vanish.** In the first run B never saw the
  clean leave of a client that left within milliseconds of joining: the SDK had not yet
  created that participant, so there was nothing to disconnect. Harmless in real use (no
  one leaves within milliseconds), but the client must build its view from
  `remote_participants()` plus events, never from events alone.
- **A refused publish takes 10 s.** The server ignores the presence token's publish attempt
  instead of answering, and the SDK times out. Our client never tries it; noted so nobody
  is surprised.
- **The webhook key name is fixed in YAML.** LiveKit signs webhooks with the key named in
  `livekit.home.yaml`, which cannot read `.env`. The guide now says: name the key `yappa`.
  A mismatch costs only the event log lines.
- First attempt failed on a self-made bug: the spike created invite codes with capitals,
  the server lowercases codes before the lookup. Fixed in the spike; the server behaviour
  (codes are case-insensitive) is intended.

## Decision it feeds

- **ADR-002 revisit trigger "S6 overruns → PocketBase": not triggered.** The control plane
  stays Rust.
- **ADR-005 confirmed** by measurement: presence in the LiveKit room, control plane HTTP
  only, webhook for the log.
- No new ADR. Two implementation decisions recorded here and in `crates/control`:
  bootstrap and emergency via the binary's subcommands instead of a web page; kick =
  out of every room now (TeamSpeak-style), the account stays. Every request carries
  `x-yappa-version`, so the minimum version also blocks token requests, not only logins.

## Follow-up tasks (also in TASKS.md)

- C1: `PresenceLink` builds its map from `remote_participants()` at connect plus attribute,
  connect and disconnect events; reconnects re-read the full list.
- Phase 2: show a person as "maybe gone" when their presence connection is stale (no
  attribute or heartbeat change, voice silent) instead of waiting for LiveKit's 20 s.
- The VPS compose (`vps.yml`) reuses the `control` service and the Caddy `/api/` route from
  `home.yml`.
- `lk load-test` with 20 publishers plus 20 presence clients.
