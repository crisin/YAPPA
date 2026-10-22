# UI-Umsetzungsplan v0

Stand: 2026-10-23 · Status: Vorschlag zur Review · Grundlage: [UI-System v0](01-ui-system-lofi-v0.md)

Der Plan übersetzt das Lofi-Konzept in Arbeitspakete für `TASKS.md`. Er beantwortet drei
Fragen: Was lässt sich heute bauen, was hängt am Control-Plane (S6) oder an der Engine, und
in welcher Reihenfolge entsteht daraus eine App, die die Gang in jedem Nightly benutzen kann?

## 1 · Ausgangslage

**Was die UI heute ist:** ein Test-Client in einer Datei (`App.svelte`): Geräte links, Einladung
einfügen oder Teilnehmer in der Mitte, Mikro/Taub/Verlassen unten, Debug-Panel und Hilfe
zuschaltbar. Zustand in einer Klasse (`Session`), eine Tür zur Engine (`engine.ts`) mit Mock
für Browser und Tests. Das bleibt alles erhalten, nur an neuen Plätzen.

**Was die Engine heute kann** (engine-protocol v1): Geräte wählen, Raum per Einladung
betreten, Teilnehmer, Pegel und „spricht“, Mute/Taub, Lautstärke und Stumm pro Person,
Statistik, Log, Rejoin. **Noch nicht:** Gate/VAD/PTT (der Sprechmodus wird nur gespeichert),
Rauschunterdrückung, EQ und Kompressor in der Kette, Pegel pro Stufe, Soundboard, App-Audio,
globale Hotkeys, Updater.

**Was es noch gar nicht gibt:** den Control-Plane. Keine Channels, keine Personen, keine
Rollen, keine Einladungslinks, keine Presence, kein Admin. Das ist Spike S6.

Daraus folgt die wichtigste Entscheidung des Plans: **Die Shell und das Voice-Modul werden
jetzt gebaut, gegen eine zweite Tür mit Mock (`api.ts`), damit die UI nicht auf S6 wartet.**
Wenn S6 fertig ist, wird nur die Tür ausgetauscht, nicht die UI.

## 2 · Entscheidungen, die der Plan trifft

| Thema | Entscheidung | Warum |
|---|---|---|
| Zwei Türen | Neben `engine.ts` (engine-protocol) kommt `api.ts` (api-types): `Api`-Interface, Tauri-Implementierung, Mock mit Fixture „Die Gang“. Die UI kennt keine andere Datenquelle. | Regel 1 aus `CLAUDE.md` (UI spricht nur engine-protocol und api-types). Mock macht Channels, Admin und Onboarding vor S6 baubar und testbar. |
| HTTP über die Rust-Shell | Der Webview ruft nie direkt `fetch` auf den Control-Plane. Tauri-Commands (`api_*`) reden HTTP; das Session-Token liegt im OS-Keychain, nie im Webview. | `api-types` sagt „kept in the OS keychain by the client“. Ein Ort für Token, Retry und Mindestversion. |
| Modul-Registry | Ein Modul ist ein Objekt: `id`, Label, Icon, Rolle, Sidebar-Komponente, Stage-Komponente, Befehle für die Palette. Die Shell rendert die Registry, kennt kein Modul beim Namen. | Skalierungsregel des Konzepts: neues Feature = neuer Eintrag, Shell unverändert. Testbar ohne Rendering. |
| Shell-Zustand | `shell.svelte.ts`: aktives Modul, Inspector (auf/zu, Inhalt), Banner-Queue mit Priorität, Palette, Settings-Overlay. Getrennt von `Session` (Engine) und vom Modulzustand. | Voice-Dock und Banner gehören der Shell, nicht dem Voice-Modul. |
| Komponenten | Bits UI (headless, Svelte 5) für Command, Dialog, Select, Slider, Tabs, Tooltip; Icons aus Lucide. Beide MIT. Entscheidung als ADR-004 mit Lizenz, Bundle-Größe und Tauri-Check. | Tastatur und Screenreader für Palette und Dialoge selbst zu bauen kostet mehr als es bringt. Styling bleibt zu 100 % bei den Tokens. |
| Einfach/Profi | Eine Eigenschaft der Settings (`view: simple \| pro`), nicht der Module. Beide Ansichten schreiben dasselbe Schema. | Konzept, Abschnitt 1 und Grobstruktur. |
| Barrierefreiheit prüfbar | `vitest-axe` im UI-Test; jeder Screen hat mindestens einen Test, der Rollen, Labels und Tastaturpfad prüft. | Die Leitplanken aus dem Konzept „gelten ab jetzt“ und sollen nicht an Disziplin hängen. |
| Tokens zuerst | Vor der Shell wächst `ui-tokens/` um das, was die Zonen brauchen (Breiten, Typo-Skala, Zustandsfarben, Fokus, Flächenebenen). Farben bleiben Platzhalter. | Optik-Runde (Konzept, offene Frage 4) soll ein JSON-Tausch sein, kein Umbau. |

## 3 · Empfehlung zu den offenen Fragen aus Runde 1

1. **Beitreten per Einzelklick**, Enter per Tastatur, „Beitreten“ beim Hover als Hinweis.
   Channelwechsel im Call ist Leave + Join ohne Rückfrage, muss sich schnell anfühlen. Klick
   auf eine Person öffnet den Inspector, nicht den Channel. Doppelklick bleibt frei.
2. **Inspector standardmäßig zu.** Er öffnet für Soundboard und Person; die Shell merkt sich
   pro Modul, ob er offen war. Kein Flow braucht ihn.
3. **Zustände werden vor U1 nicht gezeichnet, sondern als Verhalten festgelegt** (Abschnitt 5,
   „Systemzustände“). Lofi-Runde 2 zeichnet sie, wenn die Shell schon läuft, dann gegen
   echte Zustände.
4. **Optik nach U2.** Erst wenn Shell und Call-Screen stehen, lohnen 2–3 Token-Varianten.

## 4 · Arbeitspakete

Jedes Paket ist ein Task in `TASKS.md`, ein Branch, ein Review. Größe: S = ein Abend,
M = zwei bis drei, L = eine Woche. „Braucht“ nennt die harte Abhängigkeit; alles davor kann
mit Mock-Daten fertig werden.

### U0 · Fundament (S, braucht nichts)

- ADR-004: Bits UI + Lucide; `npm`-Abhängigkeiten in `apps/desktop`.
- `ui-tokens/` v1: `rail-width`, `inspector-width`, Typo-Skala (xs…xl), `line-height`,
  `color-focus`, Zustandsfarben `speaking`, `muted`, `ptt`, `offline`, `away`, Flächen
  `surface-sunken`, Motion (`duration-fast`, `duration-normal`). Dark und Light pflegen
  dieselben Keys. `cargo xtask tokens`.
- `api.ts` + `mockApi.ts`: Interface entlang `api-types` (Login per Invite-Code, Me, Channels,
  Presence-Stream, Voice-Token je Channel, Mitglieder, Einladungen, Rollen). Fixture „Die
  Gang“ wie im Konzept-Screen (Lobby, Raid-Abend, Musik-Session, AFK, sechs Leute).
- `shell.svelte.ts` + Modul-Registry mit Tests (aktives Modul, Rollen-Filter, Banner-Priorität,
  Befehle sammeln).

### U1 · Shell (M, braucht U0)

Zonen A bis G als Komponenten: `Rail`, `Sidebar`-Slot, `Stage`-Slot, `VoiceDock`, `Topbar` mit
Breadcrumb und Suchfeld, `Banner`, `Inspector`, `CommandPalette` (`Strg K`), `SettingsOverlay`
(Vollbild, `Esc`). Der Test-Client zieht um, nichts geht verloren:

| heute | morgen |
|---|---|
| Footer mit Status, Mikro, Taub, Verlassen | Voice-Dock (D) |
| Geräte-Sidebar | Settings → Einfach → Audio und Sprechen |
| JoinCard (Einladung einfügen) | Voice-Modul, Stage ohne Channel; nach U5 nur noch Fallback ohne Control-Plane |
| Participants | Voice-Modul, Stage im Call (U2 macht die Kacheln) |
| Debug-Panel | Settings → Profi → Diagnose; `yappa.debug` öffnet es weiter direkt |
| Hilfe | Palette-Befehl „Hilfe“ und Rail „Ich“ → Hilfe |
| Fehlerzeile | Banner-Slot (F), Priorität „error“ |

Systemzustände schon hier: `reconnecting` färbt das Dock und setzt den Banner „Verbindung
wird wiederhergestellt“, `disconnected` nach Verlust zeigt „Offline“ mit „Erneut verbinden“.
Ergebnis: ein Nightly mit der neuen Struktur, den die Gang sofort nutzen kann.

### U2 · Voice-Modul, Call-Screen (M, braucht U1; echte Daten ab U5)

- Sidebar: Channel-Baum mit Personen darunter, Zustand als Text (stumm, PTT, abwesend),
  Zähler, Beitreten per Klick und Enter, „Beitreten“ beim Hover.
- Stage: Personen-Kacheln (Ring für „spricht“, gestrichelt für stumm, Modus als Text),
  Aktionsleiste unten mit Soundboard und App-Audio (ausgegraut mit Tooltip „kommt später“,
  so lange die Engine es nicht kann; Plattformregel aus `CLAUDE.md`).
- Inspector „Person“: Lautstärke für mich, lokal stumm (beides gibt es in der Engine), später
  Rolle und Kick für Admins.
- Topbar: Breadcrumb „Voice / Channel“, Zähler, „Einladen“ für Admins (bis U5 kopiert es die
  `cargo xtask livekit token`-Einladung, danach einen Invite-Link).
- Palette registriert Channels und Personen.

### U3 · Settings Einfach und Profi (M, braucht U1; Protokolländerung)

Settings-Schema v2 in `engine-protocol` mit Migration v1 → v2: `view`, `ptt_key`, `hotkeys`,
`notifications`, `update_channel`, Jitter-Puffer-Ziel, Ausgabelautstärke. `cargo xtask
gen-types`, Snapshot der Defaults.

- Einfach: Audio und Sprechen (Gerät, Live-Pegel, Sprechmodus in Alltagssprache, PTT-Taste),
  Darstellung (Theme), Benachrichtigungen, Konto (Name, Abmelden; vor U5 nur Anzeige).
- Profi: Geräte, Signalkette als Stufenreihe (bis S2 sind nur „Eingang“ und „Senden“ echt,
  die anderen zeigen „noch nicht gebaut“), Effekte/VST (Platzhalter, Phase 3), Tasten,
  Netzwerk und Bitrate, Diagnose (das Debug-Panel), Updates und Kanal (Platzhalter bis
  Updater).
- Die Komponente „Audio und Sprechen“ wird so gebaut, dass U4 sie als Schritt 2 wiederverwendet.

### U4 · Onboarding (S, braucht U3 und U5 für den echten Weg)

Drei Schritte als Vollbild vor der Shell: Name, Mikro (Live-Pegel, Komponente aus U3),
Reden (Sprechmodus). Danach Lobby. Gegen Mock schon nach U3 baubar, echt erst mit
Invite-Links aus U5.

### U5 · Control-Plane anbinden (M, braucht S6)

Tauri-Seite der `Api`: HTTP-Client, Session-Token im Keychain, Mindestversion → Banner,
Presence-Stream → Channel-Baum, Voice-Token je Channel → `Join`. Invite-Link statt
eingefügter Einladung (Deep Link `yappa://invite/<code>` plus Einfügen als Fallback).
`docs/mittesten.md` wird dafür umgeschrieben.

### U6 · Admin-Modul (M, braucht U5 und die Admin-Endpunkte aus S6)

Rail-Slot nur mit Rolle Admin. Leute (Mitglieder mit Client-Version und Status, Einladungen
erstellen und widerrufen, Rollen), Struktur (Channels anlegen, umbenennen, sortieren;
Soundboard-Sounds später), Betrieb (Server und Failover, Ereignis-Log, Updates und
Kill-Switch). Was der Control-Plane noch nicht kann, ist ausgeblendet, nicht ausgegraut.

### U7 · Systemzustände, Update-Banner (S, braucht Updater und Remote-Config)

„Update bereit“, „Mindestversion“, „Kill-Switch“ im Banner-Slot; Update-Kanal in Profi.
Kommt mit dem Updater (Phase 2 der Grobstruktur).

### Engine-getriebene UI-Teile (keine eigenen UI-Tasks)

Sie gehören zum jeweiligen Engine-Task und werden dort mitgebaut, weil die Shell die Plätze
schon reserviert:

| Engine-Task | UI-Teil |
|---|---|
| Gate/VAD/PTT (S3 + Kette) | Dock zeigt Modus und Gate-Zustand, Settings-PTT-Taste wirkt |
| S2 + DSP-Kette integriert | Signalkette mit Live-Pegel je Stufe, CPU und Zusatzlatenz; braucht ein neues Event (Pegel pro Stufe, ≈ 30 Hz, zusammen mit `Levels`) |
| Soundboard | Inspector „Soundboard“ (Suche, Grid, Hotkeys, Sound hinzufügen, Lautstärke im eigenen Stream) |
| App-Audio | Breite Kachel in der Stage, „App-Audio teilen“ aktiv, nur Windows |
| Lobby-Radio | Leere Stage allein im Channel |

## 5 · Lücken in den Verträgen

Was das Konzept zeigt und die Verträge noch nicht hergeben. Alles additiv, bis auf das
Settings-Schema.

**engine-protocol**

- `Settings` v2 (siehe U3) mit `validate` und Migration.
- `Event::StageLevels { stages: Vec<StageLevel> }` für die Signalkette (Name, Pegel, aktiv),
  plus CPU-Anteil und Zusatzlatenz der Kette in `Stats`.
- `Participant.transmit_mode` ist nicht bekannt, die Kachel zeigt „Sprachaktivierung/PTT“ pro
  Person. Weg: Metadaten im LiveKit-Teilnehmer oder Presence aus dem Control-Plane. Vorerst
  zeigt die Kachel nur „spricht“ und „stumm“.
- Soundboard- und App-Audio-Commands kommen mit den Engine-Tasks.

**api-types**

- `Presence` existiert; es fehlt der Stream (SSE oder WebSocket) und `Member { user,
  client: ClientInfo, last_seen }` für die Admin-Tabelle.
- Einladungen (`Invite` ist heute die eingefügte Einladung, der Invite-Link braucht
  `InviteCode { code, created_by, expires_at, used_by }`), Rollen ändern, Kick, Channels
  anlegen und sortieren, Server und Failover, Ereignis-Log, Mindestversion und Kill-Switch.
  Das ist der Umfang von S6 plus einem Admin-Teil; der Plan setzt S6 so an, dass die
  Lese-Endpunkte für U5 zuerst kommen und die Admin-Endpunkte für U6 danach.

**Systemzustände** (Dock und Banner, in dieser Priorität, maximal ein Banner)

| Priorität | Zustand | Dock | Banner |
|---|---|---|---|
| 1 | Kill-Switch / Mindestversion | Verlassen erzwungen | „Diese Version ist gesperrt. Update …“ mit Aktion |
| 2 | Engine-Fehler | unverändert | Fehlertext, schließbar |
| 3 | Verbindung verloren, rejoin läuft | Warnfarbe, „verbinde neu …“ | „Verbindung wird wiederhergestellt“ |
| 4 | Offline nach Abbruch | „Offline“ | „Erneut verbinden“ |
| 5 | Update bereit | unverändert | „Update bereit · Neustart“ |

## 6 · Reihenfolge für TASKS.md

1. **U0 + U1** sofort, parallel zur Mac-Prüfung und zum ersten Abend mit der Gang. Reine UI,
   keine Abhängigkeit, und das nächste Nightly hat die Shell.
2. **S6** als nächster Spike, mit dem Zuschnitt aus Abschnitt 5 (Lesen zuerst, Admin danach).
3. **U2** gegen Mock, sobald U1 im `main` ist. Lofi-Runde 2 (Zustände, Lobby-Radio) läuft
   daneben beim User.
4. **U3** nach U2. Danach Optik-Runde (Token-Varianten) beim User.
5. **U5** sobald S6 steht, dann **U4**, dann **U6**.
6. **U7** mit dem Updater.

Engine-Spikes S2 und S3 bleiben in ihrer Reihenfolge und bringen ihre UI-Teile mit.

## 7 · Was bewusst nicht gebaut wird

- Kein Mobile-Code, keine responsive Shell unter 900 px Breite. Die Zonen sind so geschnitten,
  dass die Abbildung aus Konzept-Abschnitt 3 später geht.
- Kein zweiter Space, kein Space-Umschalter. Der Platz ist reserviert, die Registry hat kein
  Feld dafür.
- Keine Hub-, Games- und Tools-Module, nicht einmal als leere Einträge. Die Rail zeigt nur,
  was es gibt; der Platz entsteht durch die Registry.
- Kein eigenes Komponenten-Kit und kein CSS-Framework. Headless plus Tokens reicht.
