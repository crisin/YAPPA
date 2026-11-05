# Checkliste: VPS-Anbieter für yAPPA

Zum Aussuchen und Testen eines VPS (Stufe 2). Er ist der Server, der auch für Leute ohne IPv6
erreichbar ist und nicht am Upload des Heimanschlusses hängt. Das Einrichten selbst steht
noch aus (Task „VPS stage 2“ in `TASKS.md`).

> **Stand 2026-11-05:** Noch kein Anbieter getestet. Preise und Bedingungen ändern sich, und
> Vergleichsseiten widersprechen sich: Jeden Punkt beim Anbieter selbst nachlesen.

## So gehst du vor

1. Zwei bis drei Anbieter mit der [Ausschlussliste](#1-ausschlussliste-alles-muss-ja-sein)
   vorfiltern.
2. Die [Vergleichspunkte](#2-vergleichspunkte) für die übrigen ausfüllen.
3. Die beiden besten für einen Monat mieten, am besten stunden- oder monatsweise, und den
   [Praxistest](#3-praxistest-entscheidet) machen. **Der Test entscheidet, nicht das
   Datenblatt.**

## 1. Ausschlussliste: alles muss „ja“ sein

Ein einziges „nein“, und der Anbieter ist raus.

| Prüfen | Warum yAPPA das braucht |
| --- | --- |
| [ ] Eigene öffentliche **IPv4**, im Preis oder als bezahlbares Extra | Der eigentliche Grund für den VPS: Leute ohne IPv6 |
| [ ] Eigene öffentliche **IPv6** (mindestens eine Adresse, üblich ein /64) | Direkter Weg für alle mit IPv6, kein Umweg über NAT |
| [ ] Kein NAT-VPS und keine geteilten Ports | LiveKit braucht UDP 7882 und TCP 7881, Caddy 80 und 443, genau auf diesen Nummern |
| [ ] **KVM** oder vergleichbar, kein OpenVZ oder LXC | Docker mit Host-Netzwerk und eigene Netzwerkpuffer (`net.core.rmem_max`) |
| [ ] Eingehendes **UDP nicht gefiltert oder gedrosselt** | Die Sprache ist UDP. Das Datenblatt sagt das selten, also nachfragen und testen |
| [ ] Rechenzentrum in Deutschland oder direkt daneben | Budget Mund zu Ohr: 70–120 ms; jede Millisekunde Weg zählt doppelt |
| [ ] **x86_64** | Getestet ist bisher nur das. ARM wäre ein eigener Test |
| [ ] Monatlich kündbar | Kein Jahresvertrag, bevor der Test durch ist |
| [ ] AGB erlauben einen dauerhaften Sprachserver | Manche günstigen Tarife schließen Dauerlast aus |

## 2. Vergleichspunkte

Mit zwei Spalten pro Anbieter ausfüllen: Wert und Quelle.

### Größe: Was wirklich gebraucht wird

| Punkt | Mindestens | Anmerkung |
| --- | --- | --- |
| vCPU | 2 | Für 20 Leute reichlich, sofern die Kerne nicht überbucht sind |
| RAM | 4 GB | Der Betrieb braucht weniger, aber der Build der Control-Plane auf dem Server nicht |
| Platte | 20 GB SSD/NVMe | Images, Datenbank, Backups, optional Installer |
| Traffic | 500 GB/Monat | Rechnung unten |
| Anbindung | 1 Gbit/s geteilt | 100 Mbit/s reichen rechnerisch auch, aber nur, wenn sie garantiert sind |

**Traffic-Rechnung:** Standard-Profil ≈ 162 kbit/s pro Sprecher. Zwei gleichzeitig bei 19
Hörern ergeben rund 6–7 Mbit/s, App-Audio kommt mit rund 3,6 Mbit/s dazu, also grob 10 Mbit/s
oder 18 GB an einem Abend mit vier Stunden. Acht solche Abende im Monat sind etwa 150 GB. DTX
ist aus, stille Clients senden also auch; wie viel das ausmacht, ist noch nicht gemessen.
Deshalb 500 GB als Puffer.

### Kosten

- [ ] Monatspreis **mit** IPv4 und Steuer, im Rahmen von etwa 4–6 €
- [ ] Was passiert über dem Traffic-Limit: drosseln oder nachzahlen? Drosseln ist mitten im
      Call genauso schlecht, aber ohne Überraschung auf der Rechnung
- [ ] Einrichtungsgebühr oder Mindestlaufzeit
- [ ] Preis nach dem Aktionszeitraum

### Netz und Betrieb

- [ ] Dedizierte oder geteilte vCPU; bei geteilter: Praxistest auf Steal Time (unten)
- [ ] Firewall beim Anbieter, zusätzlich zu `ufw`
- [ ] Snapshots oder Backups, Kosten dafür
- [ ] Konsole im Browser, falls SSH nicht mehr geht (sonst sperrt ein Firewall-Fehler dich aus)
- [ ] Rescue-System, Neuinstallation per Klick
- [ ] Debian 12/13 oder Ubuntu 24.04 als fertiges Image
- [ ] DDoS-Schutz: Was passiert bei Angriffsverdacht? Wird UDP dann gefiltert oder der Server
      vom Netz genommen?
- [ ] Statusseite und Wartungsankündigungen; Wartung möglichst nicht am Abend

### Vertrag und Daten

- [ ] Sitz in der EU, Vertrag und Support auf Deutsch oder Englisch
- [ ] Zahlungsart passt; wird eine Ausweisprüfung verlangt?
- [ ] Kündigung im Kundenportal, ohne Anruf oder Brief
- [ ] Datenschutz: Auf dem Server liegen nur Namen, Einladungen und das Ereignis-Log, die Stimme
      wird nicht gespeichert. Ein EU-Anbieter reicht

### Ruf

- [ ] Erfahrungen aus den letzten zwölf Monaten suchen, gezielt zu **VoIP, WebRTC oder
      Gameserver**: Webseiten merken Jitter und UDP-Filter nicht
- [ ] Vergleichsseiten skeptisch lesen: Viele leben von Partnerlinks

## 3. Praxistest entscheidet

Auf dem gemieteten Server, vor dem Umzug. Werte pro Anbieter notieren.

| Test | Wie | Bestanden, wenn |
| --- | --- | --- |
| Ping aus zwei Haushalten der Gang | `ping -n 100 <server>` (Windows), einmal abends | Mittel unter 30 ms, kein Ausreißer-Muster |
| Weg durchs Netz | `tracert` oder `mtr` | Keine Umwege über andere Länder |
| Steal Time | `vmstat 5` über einen Abend, Spalte `st` | Dauerhaft unter 2 %, ohne Spitzen zur Spielzeit |
| UDP kommt durch | Während eines Calls aus zwei Haushalten auf dem Server `sudo tcpdump -ni any -c 200 udp port 7882` | Pakete von beiden Adressen, in beide Richtungen; kein Ausweichen auf TCP 7881 |
| IPv4 und IPv6 getrennt | Einmal von einem Anschluss nur mit IPv4 (etwa Hotspot mit abgeschaltetem IPv6), einmal mit IPv6 | Beide klingen gleich |
| Last | `lk load-test` mit 20 Sendern, dazu 20 Presence-Clients | CPU unter 50 %, kein Verlust, Steal Time bleibt niedrig |
| Echter Abend | Ein Spieleabend, danach die Diagnose-Dateien ansehen | Verlust unter 1 %, Jitter ähnlich wie beim Heimserver, Mund zu Ohr 70–120 ms |

Vergleichswert: Der erste Call über das Internet am 2026-10-24 hatte 0 % Verlust und 6–8 ms
Jitter.

## 4. Was sonst noch gebraucht wird

- [ ] **Eigene Domain** beim Registrar deiner Wahl: Ein MyFRITZ!-Name zieht nicht mit um
- [ ] A- und AAAA-Eintrag auf den Server, TTL anfangs 300 Sekunden, damit ein Wechsel schnell
      geht
- [ ] DNS-Verwaltung erlaubt CNAME, A, AAAA und ist mit Zwei-Faktor gesichert
- [ ] Domain läuft auf dich persönlich und verlängert sich automatisch

## Bewusst nicht wichtig

- 10 Gbit/s, viele Kerne oder große Platten: Für 20 Leute nur Geld
- Mehrere Standorte und Failover: kommt erst in Phase 2
- Verwaltete Kubernetes- oder Container-Dienste: `docker compose` auf einem Server reicht
- Der allerbilligste Preis: Ein Euro weniger im Monat ist keinen schlechteren Jitter wert

## Erste Kandidaten

Häufig genannt für Deutschland, ohne eigene Prüfung und ohne Reihenfolge: Hetzner, netcup,
Contabo. Für jeden gilt die Liste oben genauso. Im Praxistest besonders auf Steal Time achten,
gerade bei den günstigsten Tarifen.
