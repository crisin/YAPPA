# ADR-005: Presence and live data over a LiveKit presence room; the control plane is HTTP only

Status: proposed (2026-10-23) · becomes accepted with the merge of `task/vision-v1` ·
amends ADR-000 (Control-Plane: PresenceModule, WebSocket gateway) and the S6 cut in ADR-001

## Context

The baseline gives the control plane a presence module: LiveKit webhooks
(`participant_joined/left`) update a table, a WebSocket gateway pushes "who is where" to the
clients, a heartbeat covers lost webhooks. That is four moving parts (webhook receiver,
table, push channel with reconnect, heartbeat) for one question the media plane already
answers, and it puts the control plane into a live path the baseline otherwise keeps it out
of ("never in the voice path").

LiveKit has what is needed on the client side: a participant carries **attributes**
(key/value, synchronised by the server to late joiners, meant for state that changes every
few seconds at most) and can publish **data streams** by topic. The Rust SDK exposes
`set_attributes`, `publish_data`, `send_bytes`/`stream_bytes` (livekit 0.9.6). Rooms are
created on first join.

Options:

| Option | Parts | Latency | When control plane is down |
| --- | --- | --- | --- |
| A · baseline: webhook → table → SSE/WS push | webhook, table, push, heartbeat, client consumer | seconds (webhook + push) | presence freezes |
| B · **presence room on LiveKit** | one extra room connection per client, attributes | immediate | presence keeps working |
| C · poll `GET /presence` | endpoint, table from webhooks | poll interval | freezes |

## Decision

- **Every client sits in one LiveKit room `presence` for as long as the app runs.** It
  publishes no media. Its attributes are the presence: `channel` (id or empty), `app`
  (`<channel> <version>`), `talk` (`vad`/`ptt`/`open`), `away`. Identity = user id, name =
  display name. Token grants: `roomJoin`, `canPublish: false`, `canSubscribe: true`,
  `canPublishData: true`, `canUpdateOwnMetadata: true`; issued at login
  (`POST /auth/invite`, `GET /me`), valid ≈ 7 days, refreshed on start.
- **Voice rooms are unchanged:** room name = channel id, token per channel from
  `POST /channels/{id}/token`, ≈ 12 h.
- **The control plane does HTTP only:** invites, members, roles, channels, tokens,
  `GET /config` (minimum version, killed versions, update base URL, LiveKit URL), and
  `POST /livekit/webhook` purely for the admin **event log** (join, leave, room started and
  finished). No presence table, no push channel, no heartbeat.
- **Where it lives in the client:** `PresenceLink` in `crates/transport` (feature `livekit`,
  same runtime as the voice session), driven by `crates/client` (ADR-007). The audio engine
  does not know the presence room exists.
- **Data topics are reserved, not built:** `chat`, `soundboard` (library sync by hash,
  byte streams), `hub`. They ride on the same room later.
- **Admin actions that must take effect now** (kick, role removal) use the LiveKit
  RoomService (`remove_participant`) from the control plane, plus a new token policy on the
  next join — as in the baseline.

## Consequences

- S6 shrinks: auth, invites, channels, tokens, config, webhook log, SQLite. Read endpoints
  first (for U5), admin endpoints second (for U6).
- `api-types` gains `PresenceAttributes` (the keys above as constants), `PublicConfig`,
  `InviteCode`, `Member`, `EventEntry`; `Presence { channel_id, user_ids }` is removed
  before it is ever used.
- One more WebSocket and peer connection per client; for 20 people irrelevant. Attribute
  updates happen on channel change and mode change only (the docs warn against updates more
  often than every few seconds — we are far below).
- If LiveKit is down, voice and presence are both down. That was true before (no voice
  without LiveKit); presence now says so honestly instead of showing stale rows.
- A future mobile client gets presence from the same room with its native SDK; nothing
  server-side is Tauri- or Rust-specific.
- Revisit trigger: a second media server with failover (phase 2). Then the presence room
  needs a home that survives the primary server — either the fallback server runs it, or the
  control plane takes presence back for the failover case only.
