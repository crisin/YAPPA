# Homeserver einrichten (Stufe 1)

yAPPA auf einem PC zu Hause, erreichbar für die Gang über die FRITZ!Box. Diese Anleitung
ist für genau diesen Aufbau geschrieben: FRITZ!Box 6660 Cable, ein Windows-PC, darauf
Linux — als VM (Schritt 2) oder als WSL2 ([Variante WSL2](#variante-wsl2-statt-vm)). Erst
ausprobieren ohne Freigaben? → [Alles nur lokal](#alles-nur-lokal). Läuft auf dem Server
schon ein Cloudflare Tunnel für andere Apps? →
[yAPPA hinter einem Cloudflare Tunnel](cloudflare-tunnel.md).

> **Stand 2026-10-24:** Einmal komplett durchgelaufen auf dem Entwicklungs-PC (Windows 11,
> WSL2-Ubuntu, FRITZ!Box 6660 Cable): Freigaben, Let's-Encrypt-Zertifikat über Caddy (die
> Prüfer von Let's Encrypt kamen über IPv6 auf Port 443), Admin-Einladung. Der Anschluss hatte **keine öffentliche IPv4**
> (CGNAT), deshalb läuft dieser Server [nur über IPv6](#nur-ipv6-wenn-der-anschluss-keine-oeffentliche-ipv4-hat).
> Noch nicht geprüft: ein Call von außen über diesen Weg. Menünamen der FRITZ!Box können je
> nach FRITZ!OS-Version leicht abweichen.

## Was am Ende läuft

```text
Gang (Internet) ──► FRITZ!Box ──► Linux-VM auf dem Mini-PC
                     4 Freigaben    ├─ Caddy          TCP 80, 443   TLS, Zertifikat automatisch
                                    │    ├─ /api/  ─► yappa-control  Einladungen, Channels, Tokens
                                    │    └─ sonst  ─► LiveKit        Anmeldung am Sprachserver
                                    └─ LiveKit        TCP 7881, UDP 7882   die Sprache selbst
```

Die App holt sich über `https://<deine-adresse>/api/` ihre Zugänge, meldet sich über
`wss://<deine-adresse>` bei LiveKit an und schickt die Sprache danach direkt per UDP an
LiveKit. Wer dabei ist, entscheidest du mit Einladungslinks (Schritt 8).

## 1. Vorab prüfen: Hast du eine öffentliche IPv4-Adresse?

Das ist der Punkt, an dem das Vorhaben scheitern kann — also zuerst klären. Kabelanschlüsse
laufen oft mit **DS-Lite**: dann teilst du dir die IPv4-Adresse mit anderen Kunden, und
Portfreigaben für IPv4 funktionieren nicht.

1. FRITZ!Box-Oberfläche (`http://fritz.box`) → **Internet → Online-Monitor**.
2. Steht dort **„DS-Lite"** oder „IPv4 über DS-Lite-Tunnel", hast du keine eigene IPv4.
3. Die verlässliche Probe ist die IPv4-Adresse, die die FRITZ!Box **selbst** hat: **Internet
   → Freigaben → Portfreigaben**, bei irgendeiner Freigabe die Spalte „IP-Adresse im
   Internet" (notfalls eine Freigabe anlegen, nachsehen, wieder löschen). Beginnt sie mit
   `100.64.` bis `100.127.` (oder mit `10.`, `192.168.`), hängt der Anschluss hinter
   **CGNAT**: keine eigene IPv4, auch wenn der Online-Monitor nichts von DS-Lite sagt.
   Seiten wie `https://ifconfig.me` helfen hier **nicht** — sie zeigen die Adresse des
   Anbieter-NATs, die sieht öffentlich aus, ist es für dich aber nicht.

**Bei DS-Lite oder CGNAT:** Beim Anbieter nach „Dual Stack" oder einer „öffentlichen
IPv4-Adresse" fragen (bei Kabelanbietern oft auf Zuruf möglich). Bis dahin, oder wenn das
nicht geht, gibt es zwei Wege: [nur über IPv6](#nur-ipv6-wenn-der-anschluss-keine-oeffentliche-ipv4-hat)
(funktioniert, aber nur für Leute, die selbst IPv6 haben) oder ein VPS (Stufe 2).

## 2. Die Linux-VM

LiveKit soll auf Linux laufen, nicht in Docker Desktop unter Windows: dort geht die Sprache
durch eine zusätzliche Netzwerkschicht, und Docker Desktop läuft nur, solange jemand
angemeldet ist.

- **Virtualisierung:** Hyper-V (Windows Pro) oder VirtualBox (geht auch mit Windows Home).
- **System:** Debian 12 oder Ubuntu Server 24.04, ohne Desktop. 2 CPU-Kerne, **4 GB RAM**
  (der Betrieb braucht weniger, aber der erste Build der Control-Plane in Schritt 6 nicht),
  20 GB Platte.
- **Netzwerk: Netzwerkbrücke** (VirtualBox: „Netzwerkbrücke", Hyper-V: „Externer Switch").
  Die VM muss eine eigene Adresse aus deinem Heimnetz bekommen, wie ein eigenes Gerät. Mit
  „NAT" funktioniert es nicht.
- **Feste Adresse:** FRITZ!Box → **Heimnetz → Netzwerk** → die VM bearbeiten → „Diesem
  Netzwerkgerät immer die gleiche IPv4-Adresse zuweisen".
- **Dauerbetrieb:** Am Mini-PC Energiesparen/Standby abschalten und die VM so einstellen,
  dass sie mit Windows automatisch startet.

## 3. Docker in der VM

```bash
curl -fsSL https://get.docker.com | sudo sh
sudo usermod -aG docker $USER      # danach einmal ab- und wieder anmelden
```

LiveKit möchte größere Netzwerkpuffer, sonst warnt es beim Start:

```bash
echo "net.core.rmem_max=5000000" | sudo tee /etc/sysctl.d/90-livekit.conf
echo "net.core.wmem_max=5000000" | sudo tee -a /etc/sysctl.d/90-livekit.conf
sudo sysctl --system
```

## 4. Adresse und Freigaben in der FRITZ!Box

**MyFRITZ!-Konto:** **Internet → MyFRITZ!-Konto** einrichten, falls noch nicht geschehen. Du
bekommst eine Adresse der Form `xxxxxxxx.myfritz.net`, die immer auf deinen Anschluss zeigt.

**Freigaben:** **Internet → Freigaben → Portfreigaben → Gerät für Freigaben hinzufügen** →
die VM auswählen → vier Freigaben anlegen:

| Art | Protokoll | Port (außen = innen) | Wofür |
| --- | --- | --- | --- |
| MyFRITZ!-Freigabe (HTTPS-Server) | TCP | 443 | Anmeldung der Clients (`wss://`) |
| Portfreigabe | TCP | 80 | Let's Encrypt stellt darüber das Zertifikat aus |
| Portfreigabe | TCP | 7881 | Sprache über TCP, falls bei jemandem UDP gesperrt ist |
| Portfreigabe | UDP | 7882 | Sprache — der Normalfall |

- Bei jeder Freigabe „Internetzugriff über IPv4 und IPv6" wählen, wenn es angeboten wird.
- „Selbstständige Portfreigaben für dieses Gerät erlauben" bleibt **aus**.
- Bei der MyFRITZ!-Freigabe gibt es kein Port-Feld: die Anwendung „HTTPS-Server" **ist**
  TCP 443. „Verzeichnis" leer lassen.
- Die MyFRITZ!-Freigabe zeigt nach dem Speichern eine eigene Adresse für die VM an, in der
  Form `<vm-name>.xxxxxxxx.myfritz.net`. **Diese Adresse ist ab jetzt deine `YAPPA_DOMAIN`.**
  Die Adresse der FRITZ!Box selbst (`xxxxxxxx.myfritz.net`) zeigt bei IPv6 auf die Box
  statt auf die VM; damit kann die Zertifikatsausstellung scheitern.
- **IPv6 Interface-ID** oben auf der Geräteseite prüfen: Das muss das Ende der
  IPv6-Adresse sein, die die VM wirklich hat (`ip -6 addr`, die mit `2a…`/`2001:…`). Die
  Box merkt sich dort gern eine alte. Stimmt sie nicht, zeigt die MyFRITZ!-Adresse per IPv6
  ins Leere.
- Die Tabelle zeigt jede Freigabe **zweimal** (einmal IPv4, einmal IPv6) — das ist eine
  Freigabe, nicht zwei. Löschen entfernt beide.
- **Fehlercode 1** beim Übernehmen: Löschen einer alten Freigabe und Anlegen einer neuen auf
  demselben Port nicht in einem Rutsch, sondern erst löschen, Übernehmen, Seite neu laden,
  dann anlegen.

Port 7880 wird **nicht** freigegeben — den spricht nur Caddy innerhalb der VM an.

## 5. yAPPA auf die VM holen

```bash
sudo apt install -y git openssl
git clone https://github.com/crisin/YAPPA.git ~/yappa
cd ~/yappa/infra/livekit
```

Die `.env` braucht genau zwei Werte — ein Geheimnis und deine Adresse aus Schritt 4:

```bash
cat > .env <<EOF
LIVEKIT_API_SECRET=$(openssl rand -hex 32)
YAPPA_DOMAIN=<vm-name>.xxxxxxxx.myfritz.net
EOF
chmod 600 .env
nano .env      # nur die Adresse in der zweiten Zeile anpassen
```

Die `.env` enthält das Geheimnis des Servers. Sie gehört nicht ins Repo (ist per
`.gitignore` ausgeschlossen) und nicht in einen Chat. Den Schlüssel-**Namen** setzt
`home.yml` fest auf `yappa`; ein `LIVEKIT_API_KEY` in einer älteren `.env` wird ignoriert.

## 6. Starten und prüfen

```bash
docker compose -f home.yml up -d --build     # beim ersten Mal 2–3 Minuten (baut yappa-control)
docker compose -f home.yml ps                # livekit, caddy, control: alle "running"
```

- **LiveKit:** `docker compose -f home.yml logs livekit | grep nodeIP` muss deine
  **öffentliche** IPv4-Adresse zeigen (dieselbe wie im Online-Monitor).
- **Caddy:** `docker compose -f home.yml logs caddy | grep certificate` zeigt nach kurzer
  Zeit `certificate obtained successfully`.
- **Von außen:** Am Handy WLAN ausschalten und `https://<YAPPA_DOMAIN>/api/health` öffnen.
  Steht dort `ok` ohne Zertifikatswarnung, ist alles erreichbar.

## 7. Dein eigener Zugang

Beim allerersten Start legt die Control-Plane eine **Admin-Einladung** an:

```bash
docker compose -f home.yml logs control | grep invite
# … no members yet — this admin invite works once … link=yappa://invite/xxxx-xxxx-xxxx?server=https://…/api
```

Den Link (alles ab `yappa://`) auf dem eigenen PC öffnen: Ist yAPPA installiert, startet es
mit eingefügtem Link; sonst in der App unter **Voice** ins Feld **Einladung** einfügen.
Namen eingeben, **Beitreten**, Mikro und Sprechmodus einrichten — danach steht unter
**Ich → Konto** „Admin“.

Link verpasst oder schon verbraucht? Eine neue Admin-Einladung:

```bash
docker compose -f home.yml exec control yappa-control invite --admin
```

## 8. Die Gang einladen

Jede Person bekommt den Installer (`cargo xtask build` auf dem Entwicklungs-PC, Datei unter
`target/release/bundle/nsis/`), einen **eigenen** Link und die Anleitung
[mittesten.md](mittesten.md).

Am einfachsten in der App: In der Leiste links unten auf **Admin** (nur für Admins
sichtbar).

- **Einladungen → Neue Einladung:** Der Link steht sofort zum Kopieren da. Er gilt 7 Tage
  und genau einmal.
- **Channels:** anlegen, umbenennen, verschieben, löschen. Der erste Channel ist die Lobby.
- **Leute:** wer welche Version hat, wer gerade wo ist, Rollen, Rauswerfen.
- **Betrieb:** Mindestversion, gesperrte Versionen und das Ereignis-Log.

Dasselbe geht auch auf der VM, etwa bevor dein eigener Zugang steht:

```bash
docker compose -f home.yml exec control yappa-control invite             # 7 Tage gültig
docker compose -f home.yml exec control yappa-control invite --days 2    # kürzer
docker compose -f home.yml exec control yappa-control channel add "Raid-Abend"
docker compose -f home.yml exec control yappa-control channel            # Liste
docker compose -f home.yml exec control yappa-control policy --min 0.2.0 --kill 0.2.1
```

## Betrieb

| Was | Befehl (auf der VM, im Ordner `~/yappa/infra/livekit`) |
| --- | --- |
| Status | `docker compose -f home.yml ps` |
| Logs mitlesen | `docker compose -f home.yml logs -f livekit control` |
| Neu starten | `docker compose -f home.yml restart` |
| Stoppen | `docker compose -f home.yml down` (Daten bleiben im Volume) |
| yAPPA aktualisieren | `git pull && docker compose -f home.yml up -d --build` — nicht während jemand spricht |
| LiveKit aktualisieren | Version in `home.yml` ändern, dann wie oben |
| Datenbank sichern | `docker compose -f home.yml cp control:/data/yappa.db ./yappa-backup.db` |

Alle Container starten nach einem Neustart der VM von selbst (`restart: unless-stopped`).
Ein Neustart der Control-Plane beendet keinen laufenden Call.

## Wenn es nicht geht

| Symptom | Wahrscheinliche Ursache | Prüfen |
| --- | --- | --- |
| Caddy bekommt kein Zertifikat | Port 80 kommt nicht an, oder die Adresse zeigt per IPv6 auf die FRITZ!Box | Freigabe TCP 80; `YAPPA_DOMAIN` ist die Adresse der **VM**, nicht der Box |
| `https://…` von außen nicht erreichbar | DS-Lite, oder Freigabe 443 fehlt | Schritt 1 und 4 |
| App: „Server nicht erreichbar: …“ beim Einladungslink | Der Zusatz sagt es: Name nicht auflösbar (Link/Internet), nur über IPv6 erreichbar (die Person hat kein IPv6), keine Verbindung oder Zeitüberschreitung (Caddy/`control` aus, Port 443 geblockt), TLS-Fehler (Zertifikat) | `https://<YAPPA_DOMAIN>/api/health` von dort aus; der Link muss `server=https://<YAPPA_DOMAIN>/api` enthalten; Details im Log der App („control plane unreachable“) |
| App: „Diese Einladung ist ungültig …“ | Link schon benutzt oder abgelaufen | neuen Link ausstellen (Schritt 8) |
| App: „Diese yAPPA-Version ist zu alt“ | Mindestversion gesetzt | neuen Installer verteilen, oder `policy --min` zurücknehmen |
| `control` startet nicht: „LIVEKIT_API_SECRET is not set“ | `.env` fehlt oder liegt im falschen Ordner | die `.env` gehört nach `~/yappa/infra/livekit/` |
| Admin sieht kein Ereignis-Log, Sprache geht aber | Webhooks erreichen `control` nicht | `docker compose -f home.yml logs livekit \| grep webhook` |
| App: „could not connect … 401 Unauthorized" | Token mit anderen Schlüsseln signiert oder abgelaufen | nach einem neuen Secret: alle melden sich mit neuem Link an |
| App: „… 500" direkt nach dem Serverstart | Server noch nicht bereit | ein paar Sekunden warten |
| Jemand fliegt immer wieder raus | dieselbe Einladung läuft auf zwei Rechnern | pro Person und Rechner eine eigene Einladung |
| Beitritt klappt, aber niemand hört sich | UDP 7882 kommt nicht an | Freigabe UDP 7882; `nodeIP` im LiveKit-Log ist die öffentliche Adresse |
| Ein Einzelner hört nichts, die anderen schon | sein Netz sperrt UDP | Freigabe TCP 7881 muss stehen |
| Geht von außen, aber nicht aus dem eigenen WLAN | Rückweg über die FRITZ!Box | über Hotspot gegenprüfen |
| LiveKit-Log: `nodeIP` ist eine `192.168.…`-Adresse | öffentliche Adresse nicht ermittelt | hat die VM Internet? `use_external_ip: true` in `livekit.home.yaml` (bei [nur IPv6](#nur-ipv6-wenn-der-anschluss-keine-oeffentliche-ipv4-hat) ist das so gewollt) |
| Freigaben stehen, trotzdem kommt nichts an; „IP-Adresse im Internet" beginnt mit `100.` | CGNAT | Schritt 1; [nur IPv6](#nur-ipv6-wenn-der-anschluss-keine-oeffentliche-ipv4-hat) oder Anbieter |
| WSL: Container starten dauernd neu, `journalctl` sagt „power off" | Distro wird ohne Terminal beendet | Keep-alive-Aufgabe ([Variante WSL2](#variante-wsl2-statt-vm), Punkt 5) |
| WSL: vom PC selbst geht `https://<YAPPA_DOMAIN>` nicht, von außen schon | Windows erreicht WSL nicht über die eigene IPv6 | Hosts-Eintrag ([Variante WSL2](#variante-wsl2-statt-vm), Punkt 6) |

## Was offen ist

- Kein TURN: wer hinter einer Firewall sitzt, die nur Port 443 durchlässt, kommt nicht
  rein. Kommt mit Stufe 2 (eigene Subdomain und Zertifikat nötig).
- Keine Überwachung: fällt der Mini-PC aus, merkt es niemand außer den Leuten im Call.
- Die VM selbst braucht Updates (`sudo apt update && sudo apt upgrade`).

## Nur IPv6, wenn der Anschluss keine öffentliche IPv4 hat

Bei CGNAT oder DS-Lite kommen IPv4-Pakete aus dem Internet gar nicht erst bis zur FRITZ!Box.
IPv6 schon: Die Box öffnet mit den Freigaben aus Schritt 4 ihre IPv6-Firewall für den
Server, und LiveKit (ab 1.13) bietet den Clients seine IPv6-Adresse direkt als Kandidaten
an. Was das kostet:

- **Jeder in der Gang braucht IPv6** an seinem Anschluss. Die großen deutschen Anbieter
  (Telekom, Vodafone, 1&1, o2, alle drei Mobilfunknetze) haben es. Firmennetze, manche
  Uni-WLANs und einzelne kleine Anbieter nicht — wer es nicht hat, kommt nicht rein, und es
  gibt keinen Ausweichweg. Beim Anbieter trotzdem eine öffentliche IPv4 anfragen; sobald sie
  da ist, `LIVEKIT_CONFIG_FILE` wieder leeren.
- Wechselt das IPv6-Präfix des Anschlusses (bei Kabel selten), einmal
  `docker compose -f home.yml restart livekit`. MyFRITZ! zieht den DNS-Eintrag selbst nach.

Einrichten, zusätzlich zu den Schritten oben:

1. In der `.env` eine dritte Zeile: `LIVEKIT_CONFIG_FILE=livekit.home-ipv6.yaml`. Das ist
   dieselbe LiveKit-Konfiguration ohne STUN-Abfrage (`use_external_ip: false`): per STUN
   fände LiveKit nur die Adresse des Anbieter-NATs und würde sie den Clients nennen.
2. Die MyFRITZ!-Freigabe ist hier Pflicht (keine normale Portfreigabe 443): Nur sie gibt
   dem Server einen Namen, dessen IPv6-Eintrag auf den Server zeigt. Die Interface-ID
   (Schritt 4) muss stimmen.
3. Prüfen wie in Schritt 6; `nodeIP` im LiveKit-Log darf eine lokale Adresse sein. Von außen
   geht der Test nur von einem Gerät **mit** IPv6 — Handy im Mobilfunk ist eins.

## Variante: WSL2 statt VM

So lief der Durchlauf vom 2026-10-24 auf einem Windows-11-PC. Geht ohne Hyper-V/VirtualBox,
hat aber Eigenheiten: WSL läuft nur, solange jemand angemeldet ist, und fährt die Distro
herunter, sobald kein Terminal mehr offen ist — beides unten abgefangen.

1. **Ubuntu 24.04 in WSL2** mit systemd (`/etc/wsl.conf`: `[boot] systemd=true`) und
   Docker **in** der Distro (`curl -fsSL https://get.docker.com | sudo sh`), nicht Docker
   Desktop. Läuft Docker Desktop daneben, bei ihm die WSL-Integration für diese Distro aus.
2. **Netzwerk spiegeln:** `%USERPROFILE%\.wslconfig`

   ```ini
   [wsl2]
   networkingMode=mirrored
   dnsTunneling=true
   autoProxy=true

   [experimental]
   hostAddressLoopback=true
   ```

   dann `wsl --shutdown`. Die Distro hat danach dieselben Adressen wie der PC; die
   Freigaben in der FRITZ!Box zeigen einfach auf den PC. (Docker Desktop startet dabei
   seine Engine neu.)
3. **Firewall** (Administrator-PowerShell): Für WSL zählt die Hyper-V-Firewall, nicht die
   normale.

   ```powershell
   $wsl = '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}'
   New-NetFirewallHyperVRule -Name yappa-wsl-tcp -DisplayName "yAPPA (TCP) in WSL" -Direction Inbound -VMCreatorId $wsl -Protocol TCP -LocalPorts 80,443,7881 -Action Allow
   New-NetFirewallHyperVRule -Name yappa-wsl-udp -DisplayName "yAPPA (UDP) in WSL" -Direction Inbound -VMCreatorId $wsl -Protocol UDP -LocalPorts 7882 -Action Allow
   Set-NetFirewallHyperVVMSetting -Name $wsl -LoopbackEnabled True
   ```

   `LoopbackEnabled` ist nötig, damit der PC selbst (die App darauf) an den Server kommt.
4. **Temporäre IPv6-Adressen aus** (Administrator-PowerShell), sonst bietet LiveKit auch eine
   täglich wechselnde Adresse an, die in der FRITZ!Box nicht freigegeben ist:
   `netsh interface ipv6 set privacy state=disabled`, danach `wsl --shutdown`, damit die
   Spiegelung die stabile Adresse als Quelladresse nimmt (`ip -6 route get 2600::` in der
   Distro muss `src 2a…` zeigen, nicht `fdf8…`).
5. **Distro offen halten** — eine Aufgabe bei der Anmeldung (normale PowerShell):

   ```powershell
   $a = New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\conhost.exe" -Argument "--headless wsl.exe -d Ubuntu-24.04 --exec sleep infinity"
   $t = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
   $s = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit 0 -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
   Register-ScheduledTask -TaskName "yAPPA WSL keep-alive" -Action $a -Trigger $t -Settings $s -Force
   Start-ScheduledTask -TaskName "yAPPA WSL keep-alive"
   ```

   Ohne das steht im Journal der Distro „The system will power off now" wenige Sekunden
   nach dem letzten Terminal, und die Container starten beim nächsten Befehl neu.
6. **Die App auf demselben PC:** Windows erreicht WSL über die eigene IPv6-Adresse nicht,
   über IPv4 und `localhost` schon. In `C:\Windows\System32\drivers\etc\hosts` eine Zeile
   `192.168.178.xx <YAPPA_DOMAIN>` (die LAN-Adresse des PCs), damit die App auf diesem
   Rechner den Server findet. Andere Geräte im Haus brauchen das nicht.
7. Ab hier wie ab Schritt 3 (Netzwerkpuffer) — alles in der Distro, `docker compose` genauso.

## Alles nur lokal

Zum Ausprobieren ohne Mini-PC — auf dem Entwicklungs-PC mit Docker Desktop, ein Befehl:

```bash
cargo xtask local          # nur dieser PC
cargo xtask local --lan    # auch andere PCs im Heimnetz (Windows-Firewall: "Private Netzwerke" erlauben)
```

Das startet LiveKit (Docker, Entwicklungsschlüssel) und die Control-Plane und zeigt beim
ersten Start den Admin-Link. Danach `cargo xtask dev` (oder die installierte App) und den
Link anklicken. Weitere Links und Channels:

```bash
cargo xtask control --lan invite                    # ohne --lan, wenn nur dieser PC
cargo xtask control --lan channel add "Raid-Abend"
cargo xtask livekit down                            # LiveKit wieder stoppen
```

Lokal heißt: Entwicklungsschlüssel, kein TLS, nicht aus dem Internet erreichbar — nur zum
Testen, nie mit Portfreigabe.
