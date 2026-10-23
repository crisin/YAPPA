# Testabend

Der erste Abend mit der Gang auf dem eigenen Server — und die Messungen, die nur mit Kabel,
Ohren und einem Spiel gehen. Für Chris; die Gang braucht nur [Mittesten](mittesten.md).

Stand: 2026-10-23 · was die App kann: Einladungslinks, Channels mit „wer ist wo“,
Rauschunterdrückung (DeepFilterNet), Sprachaktivierung, Push-to-Talk mit globaler Taste,
Einstellungen für Sprechmodus, Schwelle, Rauschunterdrückung, Eingangskanal und Lautstärke.

## 1 · Vorher, allein (≈ 1 Stunde)

### Server

**Vorher ausprobieren, ohne Mini-PC:** `cargo xtask local --lan` auf dem Entwicklungs-PC —
LiveKit und Control-Plane in einem Befehl, Admin-Link im Terminal, ein zweiter PC im
Heimnetz kommt mit seinem eigenen Link rein ([Alles nur lokal](homeserver.md#alles-nur-lokal)).

**Für die Gang, auf dem Mini-PC:** nach [Homeserver einrichten](homeserver.md), Schritte 1–7.
Die Stolpersteine:

- [ ] **DS-Lite-Check zuerst** (Schritt 1). Ohne öffentliche IPv4 geht Stufe 1 nicht.
- [ ] VM mit 4 GB RAM — der erste Build der Control-Plane braucht sie.
- [ ] Die `.env` hat nur zwei Zeilen: `LIVEKIT_API_SECRET` (zufällig, Befehl in Schritt 5)
      und `YAPPA_DOMAIN` (die MyFRITZ!-Adresse der **VM**).
- [ ] `docker compose -f home.yml up -d --build`, dann `https://<YAPPA_DOMAIN>/api/health`
      am Handy ohne WLAN: `ok`.
- [ ] Der erste Admin-Link: `docker compose -f home.yml logs control | grep invite`.

### Installer

- [ ] Auf dem Entwicklungs-PC: `cargo xtask build`. Der Installer liegt in
      `target/release/bundle/nsis/` (`…-setup.exe`). Ihn selbst installieren und mit dem
      Admin-Link einloggen — das ist der erste echte Test von Link → Name → Lobby.
- [ ] In der App unter **Ich → Konto** steht „Admin“.

### Links für die Gang

Pro Person ein Link, jeder funktioniert genau einmal: in der App **Admin → Einladungen →
Neue Einladung**, dann kopieren. Link und Installer zusammen schicken, mit dem Hinweis auf
[Mittesten](mittesten.md).

Channels unter **Admin → Channels** anlegen (z. B. „Raid-Abend“, „Musik-Session“). Der erste
in der Liste ist die Lobby, dort landen alle nach der Einrichtung. Ohne App geht beides auch
auf der Box (`yappa-control invite`, `yappa-control channel add …`, siehe
[Homeserver](homeserver.md#8-die-gang-einladen)).

### Messungen, die nur hier gehen

**Mund zu Ohr mit Kabel** (S1-Rest): Kabel vom Focusrite-**Ausgang** in **Eingang 1**
(`yappa-poc` nimmt den ersten Kanal). Eingangsverstärkung so, dass die Klicks nicht
übersteuern (die rote LED bleibt aus). Lokalen LiveKit starten und messen:

```bash
cargo xtask livekit
cargo run --release -p spike-s1 -- measure --acoustic --seconds 30 --label "focusrite loop"
```

- [ ] Ergebnis (Median, p95) in [`spikes/s1-poc.md`](spikes/s1-poc.md) unter
      „Mouth to ear“ eintragen.
- Das misst den Weg **ohne** die neue Sendekette (der Spike ist älter). Die Kette kostet
  zusätzlich 10 ms (gemessen in S2/E1) — beides zusammen ist die Zahl für die App.
- Budget: 70–120 ms.

**Rauschunterdrückung anhören** (S2): `target/s2/off.wav`, `rnnoise.wav`,
`deepfilter.wav` (neu erzeugen mit
`cargo run --release -p spike-s2 -- target/s2/noisy_snr0.wav`, die Eingangsdatei steht
in [`spikes/s2-dsp.md`](spikes/s2-dsp.md)). Klingt die Stimme bei DeepFilterNet dumpf
oder abgehackt? Notieren.

## 2 · Der Abend

### Ablauf (≈ 30 Minuten, dann einfach spielen)

1. Alle installieren, klicken den Link, geben ihren Namen ein — **wie lange hat das
   gedauert, wo hing jemand?**
2. Alle in die Lobby. Reihum sprechen: Kommt jeder an? Klingt jemand abgehackt (dann
   **Ich → Audio und Sprechen → Schwelle** runter), dumpf, zu leise?
3. Einer schaltet auf **Lautsprecher** statt Headset: Hören die anderen sich als Echo?
   (Erwartet: ja — es gibt keine Echounterdrückung; der Test sagt, wie schlimm.)
4. Einer stellt auf **Push-to-Talk**, legt eine Taste fest und startet ein Spiel im
   **Vollbild**: Kommt die Stimme beim Drücken an, und reagiert das Spiel weiter auf die
   Taste? (S3 — Anti-Cheat-Spiele gesondert notieren.)
5. Einer zieht kurz das LAN-Kabel oder wechselt ins WLAN: Wie lange war er weg, kam er
   von selbst zurück?
6. Spielen. Wer etwas Komisches hört: **Ich → Profi → Diagnose → Diagnose-Datei speichern** und
   die Uhrzeit notieren.

### Was in der Diagnose wichtig ist

| Wert | Unauffällig | Auffällig |
| --- | --- | --- |
| Laufzeit zum Server | unter 40 ms | über 100 ms |
| Paketverlust | 0 % | über 2 % |
| Ersetzte Samples | unter 1 % | über 3 % |
| Sendekette pro Block | Ø um 1–2 ms | max über 5 ms (orange) |
| Verworfene Sende-Blöcke | 0 | steigt |

### Danach

- [ ] Diagnose-Dateien einsammeln.
- [ ] Notizen in diese Liste:
  - Was hat nicht geklappt (Installation, Link, Server)?
  - Wer klang wie (abgehackt, dumpf, zu leise, Echo)?
  - PTT im Vollbild: Spiel, Ergebnis.
  - Kabel/WLAN: Dauer der Lücke.
- [ ] Die Punkte kommen als Aufgaben in [`TASKS.md`](../TASKS.md).
