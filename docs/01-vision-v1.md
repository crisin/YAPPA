# yAPPA — Vision und Architektur v1

Stand: 2026-10-23 · Status: Vorschlag, wird mit dem Merge von `task/vision-v1` verbindlich ·
Gesamtreview nach dem UI-Konzept (Runde 1)

Dieses Dokument ist die Wahrheit für Phase 1. Wo es von der Planungsbasis
([Grobstruktur v0](00-grobstruktur.md), ADR-000) oder von ADR-001 abweicht, gilt es. Die
Einzelentscheidungen stehen in [ADR-004](adr/004-client-ui-architecture.md) bis
[ADR-007](adr/007-client-core-settings-hotkeys.md); hier steht das Ganze.

## 1 · Was v1 ist

**Die Gang spielt einen Abend ohne Discord.** Installer, Einladungslink, Name, Mikro, reden.
Wer will, dreht im Profi-Bereich an der Kette. Ein Admin verwaltet Leute und Channels in der
App. Ein neuer Build ist in Minuten bei allen.

**v1 ist fertig, wenn:**

1. Login per Einladungslink, Channels mit „wer ist wo“, Beitreten per Klick.
2. Stimme: Rauschunterdrückung, Gate/VAD, Push-to-Talk auch im Vollbildspiel, EQ und
   Kompressor, Lautstärke pro Person. Mund-zu-Ohr gemessen, innerhalb 70–120 ms.
3. Einstellungen Einfach und Profi, Onboarding in drei Schritten.
4. Admin in der App: Mitglieder, Einladungen, Rollen, Channels, Ereignis-Log.
5. Auto-Update mit Kanälen nightly/beta/stable, Mindestversion und Kill-Switch.
6. Reconnect ohne Zutun, Diagnose-Export, In-App-Hilfe.
7. Vier Wochen Betrieb ohne Ausfall, den die Gang bemerkt hat (das ist Phase 2, aber der
   Maßstab steht jetzt).

**Nicht in v1:** Soundboard, App-Audio, Lobby-Radio und Session-Radio, Effekte und Plugins,
Text-Chat, Echo-Cancellation für Lautsprecher, zweiter Media-Server mit Failover, Mobile. Für
alle ist der Platz reserviert (Abschnitte 3, 10 und 11), gebaut wird nichts davon.

## 2 · Die Review: was bleibt, was sich ändert

**Bleibt, bestätigt**

| Was | Warum es trägt |
|---|---|
| Rust-Engine mit dem LiveKit-Rust-SDK | S1: 50–60 ms digitaler Pfad, RED wirkt, Reconnect klappt. `livekit` 0.9.4 → 0.9.6 ist Routine. |
| Tauri 2 + Svelte 5 + Vite | Tauri 3 ist Alpha (3.0.0-alpha.5), nichts drängt. |
| Control-Plane: axum + sqlx/SQLite, ein Binary neben LiveKit | Klein, eine Sprache, DTOs per ts-rs. Wird durch Abschnitt 3 noch kleiner. |
| Schichtregeln, `cargo xtask`, `TASKS.md`-Workflow, Logbuch-Commits | Haben seit S0 funktioniert; nichts davon hat gebremst. |
| UI-Konzept: eine Shell, Module, Zonen A–G | Skaliert für Hub/Games/Tools, ohne die Shell anzufassen. |
| Einfach/Profi, Themes aus Tokens, Admin in der App | Baseline, unverändert. |

**Ändert sich**

| Was | Vorher | Jetzt | Warum |
|---|---|---|---|
| Presence und Live-Daten | Control-Plane: LiveKit-Webhook → Tabelle → Push an Clients (SSE/WebSocket), Heartbeat | **Ein LiveKit-Presence-Raum.** Jeder Client sitzt dauerhaft drin (keine Medien), seine Teilnehmer-Attribute sagen Channel, Version, Sprechmodus. Alle sehen alle live, Nachzügler bekommen den Stand vom Server. Die Control-Plane ist nur noch HTTP plus ein Webhook fürs Ereignis-Log. [ADR-005](adr/005-presence-over-livekit.md) | Weniger Teile, keine Verzögerung, Control-Plane raus aus jedem Live-Pfad. Der Raum liefert gratis den Datenkanal für Chat, Soundboard-Sync und Hub. Jedes LiveKit-SDK kann das, auch ein späterer Mobile-Client. |
| Settings-Schema | Ein `Settings` in `engine-protocol`, inklusive `theme` | **Zwei Schemas:** `engine-protocol::Settings` nur Audio und Senden (v2, Migration); `AppSettings` in `crates/client` für Ansicht, Theme, Tasten, Benachrichtigungen, Update-Kanal. [ADR-007](adr/007-client-core-settings-hotkeys.md) | Die Engine soll kein Theme validieren. Hotkeys sind OS-Sache, nicht Engine-Sache. |
| Einstellungen in der UI | Vollbild-Overlay mit `Esc` | **Modul „Ich“** in der Shell: Sidebar = Settings-Navigation, Stage = Bereich. Rail und Voice-Dock bleiben sichtbar. `Esc` springt zurück. [ADR-004](adr/004-client-ui-architecture.md) | Mute muss im Call immer erreichbar sein; ein Overlay versteckt das Dock. Weniger Sonderfälle in der Shell. |
| Onboarding | Teil des Settings-Konzepts | Bleibt Vollbild, weil es vor dem ersten Call liegt. Schritt 2 ist dieselbe Komponente wie „Audio und Sprechen“. | — |
| Updates | Manifest und Artefakte auf GitHub Releases, Repo öffentlich | **Unsere Box liefert aus:** Caddy serviert `/updates/<kanal>/latest.json` plus Installer; Mindestversion und Kill-Switch kommen aus `GET /config` der Control-Plane. Repo jetzt **privat** auf GitHub (CI läuft), öffentlich erst, wenn das Logo geklärt ist. [ADR-006](adr/006-distribution-updates.md) | Entkoppelt Auslieferung von „öffentlich“. Ein Ort für Manifest, Mindestversion und Kill-Switch. |
| Client-Logik außerhalb der Engine | Im Tauri-Crate (`src-tauri/src/lib.rs`) | **`crates/client`:** API-Client, Session-Token im Schlüsselbund, Presence-Link, Settings-Speicher, Update-Prüfung. Kein Tauri, kein Audio. Die Shell verdrahtet nur. [ADR-007](adr/007-client-core-settings-hotkeys.md) | Testbar ohne Fenster; das Headless-Beispiel bekommt denselben Login wie die App. |
| Push-to-Talk-Mechanik | `tauri-plugin-global-shortcut` als Kandidat | **Eigener Low-Level-Hook/Raw-Input hinter dem `GlobalHotkey`-Trait** in `crates/platform-windows`. `RegisterHotKey` (was das Plugin nutzt) stellt die Taste dem Spiel nicht zu. S3 prüft es am echten Spiel. | Die PTT-Taste muss im Spiel weiter funktionieren. |
| Echo-Cancellation | „S1 klärt es“ | **v1 ohne AEC, Headset-Annahme.** Das Rust-SDK (0.9.6) stellt das Audio-Processing-Modul von libwebrtc nicht bereit; eine eigene AEC ist Phase-2-Arbeit. Die UI sagt es beim Onboarding (Lautsprecher → Push-to-Talk empfohlen). | Ehrlich statt halb. |
| Denoiser | DeepFilterNet als Default | **DeepFilterNet-LL als Default, aber mit Tor:** S2 misst Kette und CPU. Bleibt Default nur, wenn die Kette ≤ 25 ms Zusatzlatenz und ≤ 5 ms CPU pro 10-ms-Block kostet, sonst RNNoise (`nnnoiseless`). Das crates.io-Paket `deep_filter` ist 0.2.5 und veraltet; das Repo steht bei 0.5.7 (MIT/Apache, Inferenz über tract). Also Git-Abhängigkeit auf einen Commit, `cargo deny` bekommt die Quelle erlaubt. | Das Latenzbudget ist eng (Abschnitt 6). |
| Einladung | Eingefügter Text `yappa1.…` | **Link `yappa://invite/<code>`** über Deep Link plus Einfügen als Fallback. Das heutige Format bleibt als Entwickler-Werkzeug (`cargo xtask livekit token`). | Nicht-ITler klicken Links. |
| Soundboard- und App-Audio-Plätze im Call | ausgegraut mit „kommt später“ | **Ausgeblendet**, bis die Engine es kann. Der Platz entsteht durch die Registry, nicht durch leere Knöpfe. | Regel aus `CLAUDE.md`: fehlende Implementierung blendet aus. |
| LiveKit-Server | v1.13.8 | v1.13.10 (2026-10-14) beim nächsten Compose-Update. | Routine. |

**Geprüft und bewusst nicht geändert**

- Control-Plane auf derselben Box wie LiveKit (ADR-001). Für eine Gruppe reicht eine Box;
  Failover ist Phase 2.
- Pump-Thread, der den Encoder antreibt (ADR-003). Ob er bleibt, sagt der Engine-Befund
  (Abschnitt 5).
- Kein Komponenten-Kit, keine UI-Bibliothek mit eigenem Look: headless (Bits UI) plus Tokens.
- Kein Redis, kein Cluster, kein zweiter Space.

## 3 · Architektur v1

```
 ┌──────────────────────── Desktop-Client (Tauri 2) ─────────────────────────┐
 │  Webview: Svelte 5 UI                                                     │
 │    Shell (Rail, Sidebar, Stage, Voice-Dock, Topbar/Palette, Banner,       │
 │    Inspector) + Module: Voice · Ich (Settings, Hilfe) · Admin             │
 │    zwei Türen:  engine.ts (engine-protocol)   api.ts (api-types + App)    │
 │ ───────────────────────────── Tauri IPC ───────────────────────────────── │
 │  Shell (src-tauri): Fenster, Tray, Deep Link, Single Instance, Updater,   │
 │    Logging, Commands → verdrahtet die zwei Crates darunter                │
 │  crates/client            crates/audio-engine                             │
 │    api (reqwest)            capture → [Kette] → pump → transport          │
 │    session (keyring)        transport → mixer → playout                   │
 │    presence (PresenceLink)  Settings (Audio)                              │
 │    AppSettings, Updates     ↑ engine-protocol                             │
 │         │                   │                                             │
 │  crates/transport (LiveKit): VoiceSession (Audio) · PresenceLink (Daten)  │
 │  crates/platform-windows|macos: GlobalHotkey (PTT), später AppCapture     │
 └────────────┬────────────────────────────────┬──────────────────────────── ┘
              │ HTTPS (api-types)              │ WSS + UDP/TURN (LiveKit)
 ┌────────────▼─────────────┐   ┌──────────────▼───────────────────────────┐
 │ crates/control (axum)    │   │ LiveKit-Server (eine Box, Caddy davor)   │
 │  Auth, Einladungen,      │   │  Räume: presence · <channel-id>…         │
 │  Channels, Tokens,       │◄──│  Webhook (nur fürs Ereignis-Log)         │
 │  /config, Ereignis-Log   │   │                                          │
 │  + /updates (statisch)   │   └──────────────────────────────────────────┘
 └──────────────────────────┘
```

**Crates (Ziel, Phase 1)**

| Crate | Kennt | Kennt nicht |
|---|---|---|
| `engine-protocol` | Commands, Events, Audio-Settings | — |
| `api-types` | HTTP-DTOs, Presence-Attribute, Config, Update-Manifest | — |
| `dsp` | reine Blöcke | I/O |
| `denoise` | `Denoiser`-Trait, DeepFilterNet und RNNoise, Modelle laden | Geräte, Netz |
| `audio-engine` | Geräte, Kette, Mixer, `Transport`-Trait | Netz, Tauri, HTTP |
| `transport` | LiveKit: Voice-Session und Presence-Link | DSP, UI |
| `client` | API, Session, Presence, AppSettings, Updates | Audio, Tauri |
| `control` | HTTP-Server, SQLite, LiveKit-Tokens, Webhook-Log | Audio |
| `platform-windows`, `platform-macos` | Hotkeys, später App-Capture | alles andere |
| `apps/desktop/src-tauri` | verdrahtet client + audio-engine + platform | — |

**Live-Pfade**

| Situation | Was passiert |
|---|---|
| Erster Start | Deep Link `yappa://invite/<code>` (oder eingefügt) → Onboarding → `POST /auth/invite` → Session-Token in den Schlüsselbund, Presence-Token → Presence-Raum beitreten → Lobby. |
| Normaler Start | Token aus dem Schlüsselbund → `GET /me`, `GET /channels`, `GET /config` (Cache, wenn offline) → Presence-Raum beitreten. |
| Channel beitreten | `POST /channels/{id}/token` (gecacht, ≈ 12 h gültig) → Engine `Join` → Attribut `channel` im Presence-Raum setzen. |
| Channel wechseln | Leave + Join ohne Rückfrage, Attribut aktualisieren. |
| Verbindung weg | SDK-Reconnect, dann Engine-Rejoin (ADR-003). Dock und Banner zeigen es. Presence-Raum reconnectet unabhängig. |
| Control-Plane weg | Call läuft, Presence läuft (beides LiveKit). Nur neue Logins, neue Channel-Tokens und Admin warten. Cache deckt Channel-Liste und Config. |
| LiveKit weg | Alles weg, und zwar sichtbar. Das ist der eine Ausfall, den Phase 2 mit Failover angeht. |
| Update | Beim Start und stündlich `latest.json` des Kanals prüfen; Download im Hintergrund; Installation nie im Call, sondern beim nächsten Start oder per Klick. Kill-Switch aus `/config` erzwingt Verlassen plus Update-Banner. |

## 4 · Verträge

**engine-protocol v2** (Commands rein, Events raus; Schema nur Audio)

- `Settings { version: 2, input_device, output_device, transmit_mode, vad, denoiser,
  high_pass_hz, eq, compressor, voice_bitrate_kbps, output_gain_db }` — `theme` wandert in
  `AppSettings`. Migration v1 → v2 in der Shell beim Laden.
- Neue Commands: `SetPttPressed { pressed }` (der Hook sitzt außerhalb der Engine),
  später `PlaySound`, `ShareApp`.
- Neue Events: `StageLevels { stages: [{ id, level_db, active }] }` im Takt von `Levels`;
  `Stats` bekommt `chain_cpu_percent` und `chain_latency_ms`.

**api-types** (HTTP, additiv; Server antwortet N-1-kompatibel)

| Endpunkt | Wer | Zweck |
|---|---|---|
| `POST /auth/invite` | alle | `{ code, display_name, client }` → `Session { user, session_token, presence: VoiceToken }` |
| `GET /me` · `GET /channels` · `GET /config` | Mitglied (Config öffentlich) | Stand und Cache |
| `POST /channels/{id}/token` | Mitglied | `VoiceToken` für den Channel-Raum |
| `GET/POST/DELETE /invites` | Admin | Einladungen mit Ablauf |
| `GET /members` · `PATCH /members/{id}` · `POST /members/{id}/kick` | Admin | Rollen, Rauswurf (RoomService `remove_participant`) |
| `POST/PATCH/DELETE /channels` | Admin | Struktur |
| `GET /events` | Admin | Ereignis-Log (Join, Leave, Reconnect, Update, Kick) |
| `POST /livekit/webhook` | LiveKit | nur Log |
| `GET /updates/<kanal>/latest.json` | Updater | statisch, von Caddy |

`PublicConfig { min_version, killed_versions, update_base_url, livekit_ws_url }`.

**Presence-Raum** (`presence`, Token mit `canPublish: false`, `canPublishData: true`,
`canUpdateOwnMetadata: true`, Identity = User-ID, Name = Anzeigename)

| Attribut | Werte |
|---|---|
| `channel` | Channel-ID oder leer |
| `app` | `nightly 0.4.2` |
| `talk` | `vad` · `ptt` · `open` |
| `away` | `1` oder leer |

Datenkanäle (Topics) reserviert: `chat`, `soundboard`, `hub`. Nichts davon in v1.

## 5 · Engine-Stand und Befund

Was die Engine heute kann: Geräte, Raum per Einladung, Mixer mit 24 Plätzen, Pegel,
Lautstärke und Stumm pro Person, Statistik, Log, Rejoin. Was fehlt: die ganze Sendekette
(Denoiser, Gate/VAD/PTT, EQ, Kompressor), Resampling, Drift-Ausgleich, Pegel pro Stufe.

Der Realtime-Review vom 2026-10-23 (siehe Abschnitt 5.1) sagt, ob die Kette in den heutigen
Pump-Pfad passt oder ob vorher umgebaut wird.

### 5.1 · Realtime-Review 2026-10-23

**Urteil: tragfähige Basis.** Thread-Schnitt (Control, Pump, Geräte-Callbacks), Slot-Modell
im Mixer und die Echtzeit-Disziplin halten: In den Geräte-Callbacks nur `rtrb` und Atomics,
keine Allokation, kein Log. Schichten sauber (`dsp` ohne Abhängigkeiten, `audio-engine`
ohne LiveKit-Feature, Shell als einziger Kompositionspunkt).

**Vor S2 umbauen, klein (Paket E0):**

1. **Capture-Ring auf f32.** Heute wird im Callback auf i16 quantisiert
   (`device.rs:191`), also vor jeder Verarbeitung. Ring als `Producer<f32>`, Pump-Block
   f32, i16 erst bei `voice.send`. Jetzt billig, nach der Kette teuer.
2. **Busy-Zeit im Pump messen.** `interval_max_ns` misst Lücken, nicht Arbeit. `Stats`
   bekommt `pump_busy_max_ns` und Mittel, damit S2 ein „davor“ hat (Regel 5).
3. **Denoiser bekommt ein eigenes Crate `crates/denoise`.** Modell laden ist I/O, also
   nicht `dsp`; `audio-engine` hängt davon ab, Regel 1 bleibt intakt.
4. FTZ/DAZ auf dem Pump-Thread setzen (Biquad in f32 wird nach Gate-Schluss denormal).
5. Master-Stufe im Mixer (Ausgabelautstärke, Limiter) zwischen Summe und Clamp — heute
   clippen 19 laute Sprecher hart.
6. Capture nimmt nur Kanal 0; Kanalwahl ins Gerät (Mikro an Eingang 2 ist sonst stumm).
7. `AppInfo` aus der Shell nach `engine-protocol` (ist heute handgetippt in `engine.ts`).

**Wo die Kette hinkommt:** Sendeseite im Pump-Thread zwischen `pop_entire_slice` und
`voice.send` (`engine.rs:743-758`), Parameter über `PumpShared` (Atomics oder ein `rtrb`
mit ganzen Koeffizientensätzen); `gate_open` und `set_muted` sind die vorhandenen Haken für
Gate und PTT. Empfangsseite in `MixerRt::render` mit vorallokierten `[Biquad; SLOTS]`.

**Budget-Erkenntnis:** Der Pump ist nicht das Problem (≤ 1 ms Poll plus Blockaufbau;
`capture_frame` reicht nur an libwebrtc weiter, das selbst encodet — ADR-003 übertreibt
„encodes on the pump“). Was zählt, ist der Engine-Anteil von ≈ 50–65 ms vor Opus-Framing
und Jitterbuffer: Capture-Periode 10–22, Pump ≤ 11, Polster 20, Ausgabe-Periode 10–22,
dazu die 100-ms-Empfangsqueue des SDK als dritter Puffer in Reihe. „Drift-Ausgleich“ in
Later heißt deshalb: den Ring beim Polster halten, nicht „irgendwo zwischen 20 und 100 ms“.

## 6 · Sendekette und Budget

Mund zu Ohr, Zielwerte aus der Baseline, mit dem, was S1 gemessen hat:

| Stufe | Budget | Stand |
|---|---|---|
| Capture-Periode | 10 ms | 10–22 ms je nach Gerät (480 Frames beim Controller-Mikro, bis 1056 beim Focusrite) |
| Pump | — | ≤ 11 ms (Poll + Blockaufbau) |
| Kette (Denoiser, Gate, EQ, Kompressor) | ≤ 25 ms Latenz, ≤ 5 ms CPU je Block | **offen, S2.** DeepFilterNet-LL ohne Lookahead liegt laut Autor bei ≈ 20 ms STFT-Latenz, RNNoise bei 10 ms. |
| Opus-Frame + Encoder | 20 ms | im SDK |
| Netz | 5–15 ms | localhost gemessen, Internet offen |
| Jitterbuffer (NetEq) | 20–60 ms | 30–40 ms, mit RED unter Verlust 52–54 ms |
| SDK-Empfangsqueue | 0 | bis 100 ms, wenn sie sich füllt (drop-oldest) |
| Polster + Ausgabe-Periode | 10 ms | 20 ms Polster plus Callback 10–22 ms — **zu viel, muss runter** |
| **Summe** | 70–120 ms | digital 50–60 ms (S1) plus Engine-Anteil ≈ 50–65 ms ohne Kette |

Konsequenzen: Die Geräte- und Puffer-Seite der Engine ist der größte Posten, nicht die
Kette. S1-Rest (Loopback-Kabel) misst sie zuerst; E0 bringt die Messpunkte. Ziel für die
Engine-Seite: Polster an die tatsächliche Callback-Größe koppeln, Ring beim Polster halten,
SDK-Queue klein. Jede Änderung an der Kette trägt eine Messung davor und danach
(`cargo xtask bench`).

## 7 · UI

Das Konzept steht in [`ui/01-ui-system-lofi-v0.md`](ui/01-ui-system-lofi-v0.md), der Weg
dorthin in [`ui/02-umsetzungsplan.md`](ui/02-umsetzungsplan.md). Die Review ändert daran:

- Settings sind das Modul „Ich“ (Rail unten), kein Overlay. Hilfe hängt mit dran.
- Soundboard und App-Audio erscheinen erst, wenn die Engine sie kann.
- Sprechmodus pro Person kommt aus dem Presence-Attribut `talk`, bleibt also auf der Kachel.
- Offene Fragen aus Runde 1, entschieden: Beitreten per Einzelklick und Enter; Inspector
  standardmäßig zu. Lofi-Runde 2 zeichnet die Zustände gegen die laufende Shell.
- Barrierefreiheit wird geprüft, nicht versprochen: `vitest-axe` je Screen.

## 8 · DX

| Änderung | Warum |
|---|---|
| Repo auf GitHub, privat, jetzt | CI (`check.yml`) läuft seit S0 ins Leere. Nightly-Builds brauchen CI. |
| `cargo xtask dev --ui --scene <name>` | Mock-Szenen (im Call, Reconnect, Offline, Update, leerer Space) für Lofi-Runde 2 und UI-Tests ohne Server. |
| `YAPPA_PROFILE=<name>` | Zweite Instanz auf einem PC mit eigenem Config- und Log-Ordner: zwei Clients, ein Rechner. |
| `cargo xtask release <kanal>` | Version, Changelog-Schnitt, Build, Signatur, Manifest und Upload auf die Box. Ein Befehl vom Merge zur Gang. |
| `cargo xtask livekit presence` | Presence-Raum mitlesen (Attribute aller Teilnehmer) beim Debuggen. |
| Realtime-Reviewer vor jedem Engine-Merge | Gab es schon; wird in der `task`-Skill zur Pflicht für `crates/{dsp,audio-engine,transport}`. |

Was nicht kommt: Storybook, E2E-Fahrer, zweites Build-System. Mock-Szenen plus vitest reichen.

## 9 · Phase 1 in vier Meilensteinen

Reihenfolge ist Abhängigkeit, nicht Wunsch. `TASKS.md` trägt die Pakete.

| Meilenstein | Ergebnis für die Gang | Pakete |
|---|---|---|
| **M1 Shell** | Nightly mit der neuen Struktur; noch eingefügte Einladung | U0, U1 · parallel: Mac-Check, erster Abend, Homeserver |
| **M2 Login und Channels** | Link klicken, Name, Lobby, sehen wer wo ist, Channel per Klick | S6 (neuer Zuschnitt), C1 `crates/client`, U2, U5 |
| **M3 Stimme** | Rauschunterdrückung, Gate, PTT im Spiel, Einfach/Profi | S1-Rest, E0 Engine-Vorbereitung, S2, E1 Kette, S3 PTT, U3 |
| **M4 Betrieb** | Auto-Update, Admin, Onboarding, Banner | D1 Release-Pipeline + Updater, U4, U6, U7 · Lofi-Runde 2, Optik-Runde |

Danach Phase 2 (Qualität: Failover, Last, AEC, vier Wochen Betrieb) und Phase 3
(Erweiterungen: Soundboard, App-Audio, Effekte, Plugins, Chat, Hub).

## 10 · Mobile, nur angedacht

Zwei Wege, beide nach v1, beide ohne Rust-Engine auf dem Telefon:

| Weg | Stack | Taugt für | Haken |
|---|---|---|---|
| A · Prototyp | Tauri 2 Mobile, dieselbe Svelte-UI mit Mobile-Layout aus dem Konzept, Stimme über `livekit-client` (JS) im Webview | zuhören, kurz mitreden, Presence | `getUserMedia` im WKWebView unter `tauri://` ist nicht gesichert (Tauri-Issues #12068, #10898); Mikro wird im Hintergrund stummgeschaltet; kein Call bei gesperrtem Telefon |
| B · Richtig | Native dünne Clients mit LiveKit Swift und Android SDK, CallKit / Foreground-Service, eigene UI nach den Tokens | echter Call in der Hosentasche | zweite und dritte Codebasis für die UI |

Was v1 dafür tut, und nur das: Alle Verträge sind reines HTTP plus LiveKit (keine
Tauri-IPC-Annahmen auf Serverseite), Presence liegt im LiveKit-Raum, den jedes SDK versteht,
die Tokens sind Dateien. Der Mobile-Client ist dann ein Konsument derselben Verträge. Vor
dem ersten Mobile-Task: ADR mit der Wahl A oder B.

## 11 · Session-Radio, angedacht (Phase 3)

Die Idee (2026-10-23): Das Radio hört dem Abend zu, sammelt die Themen aus dem Gesprochenen
und gibt sie am Ende der Session an eine lokale KI-Pipeline. Die macht daraus X Songs, die
dann ab und zu im Radio laufen. **Alles lokal im Desktop-Client**, nichts verlässt den
Rechner.

Wie es in die Architektur passt, ohne sie zu biegen:

| Teil | Wo | Wie |
|---|---|---|
| Mithören | `audio-engine` | Ein **Session-Tap**: Ausgabemix plus eigene Stimme als mono 16 kHz in einen `rtrb` aus dem Output-Callback (push only, erlaubt), ein Nicht-RT-Thread schreibt Opus/WAV-Stücke in den Session-Ordner. Neuer Command `SetSessionTap { enabled }`. |
| Transkript und Themen | `crates/radio` (neu, kein Audio-I/O) | Whisper-Klasse lokal (`whisper-rs`, GGUF-Modell), danach ein lokales LLM (llama.cpp-Bindung) macht aus dem Transkript eine Themenliste. Läuft während der Session mit niedriger Priorität oder erst danach. |
| Songs | lokaler **Sidecar-Prozess** | Musikmodelle sind heute PyTorch-Welt (ACE-Step, Stable Audio Open, MusicGen). Der Client spricht einen kleinen lokalen Vertrag (`localhost`, JSON: Themen rein, Dateien raus); die erste Implementierung ist ein separat installierbarer Python-Sidecar oder ein ComfyUI-Workflow. Ein Modellabsturz reißt die App nicht mit. Läuft **nach** der Session, wenn die GPU frei ist. |
| Abspielen | Lobby-Radio (Konzept, Abschnitt 2) | Die Songs landen in der lokalen Bibliothek; das Lobby-Radio spielt sie, wenn man allein im Channel ist. Für alle hörbar = später ein `music`-Track, kein neues Feature. |

Regeln, die vorher feststehen:

- **Sichtbar für alle:** Wer mithört, trägt das Presence-Attribut `rec` und das Dock zeigt
  es; der Channel sieht ein REC-Zeichen. Kein stilles Mitschneiden, auch nicht unter Freunden.
- **Rohaudio ist flüchtig:** Nach Transkription werden die Audio-Stücke gelöscht; es bleiben
  Themen und Songs. Per Einstellung auch Transkripte nur im RAM.
- **Opt-in pro Person:** wer nicht will, dass seine Stimme in den Mix geht, kann sich
  ausnehmen (der Tap lässt den Slot aus).
- Vor dem ersten Task: Spike **S7** (Qualität der Themen aus Gaming-Gesprächen, GPU-Zeit pro
  Song, Plattenplatz, welcher Sidecar) und ein ADR.

Was dafür bewusst nicht gebaut wird: Cloud-Dienste, Verarbeitung auf dem Server, Aufnahme
ohne REC-Anzeige, Echtzeit-Generierung während des Spiels.

## 12 · Offen für dich

1. GitHub-Repo-Name und Sichtbarkeit (Vorschlag: `crisin/yappa-b`, privat).
2. Wo die Box in Phase 1 steht: Mini-PC daheim (DS-Lite-Check!) oder VPS. Beides geht mit
   demselben Compose.
3. Lofi-Runde 2: Einzelklick und Inspector-zu bestätigen, Zustände zeichnen.
