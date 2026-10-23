# Homeserver einrichten (Stufe 1)

yAPPA auf dem Mini-PC zu Hause, erreichbar für die Gang über die FRITZ!Box. Diese
Anleitung ist für genau diesen Aufbau geschrieben: FRITZ!Box 6660 Cable, Mini-PC mit
Windows, darauf eine Linux-VM. Erst ausprobieren ohne Mini-PC? → [Alles nur lokal](#alles-nur-lokal).
Läuft auf dem Server schon ein Cloudflare Tunnel für andere Apps? →
[yAPPA hinter einem Cloudflare Tunnel](cloudflare-tunnel.md).

> **Stand 2026-10-23:** Lokal geprüft sind LiveKit, die Control-Plane (`yappa-control`, als
> Docker-Image gebaut), Einladungslinks, Beitritt und Presence. Nicht geprüft: Caddy mit
> Let's-Encrypt-Zertifikat, die FRITZ!Box-Freigaben und der Zugriff von außen. Menünamen der
> FRITZ!Box können je nach FRITZ!OS-Version leicht abweichen. Was beim ersten Durchlauf
> hakt, bitte hier nachtragen.

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
3. Gegenprobe: Die dort angezeigte IPv4-Adresse muss dieselbe sein, die eine Seite wie
   `https://ifconfig.me` anzeigt. Beginnt sie mit `100.64.` bis `100.127.`, ist es
   ebenfalls keine öffentliche Adresse.

**Bei DS-Lite:** Beim Anbieter nach „Dual Stack" oder einer „öffentlichen IPv4-Adresse"
fragen (bei Kabelanbietern oft auf Zuruf möglich). Geht das nicht, ist der Homeserver für
diesen Anschluss raus und wir nehmen einen VPS (Stufe 2) — dann hier nicht weitermachen.

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
- Die MyFRITZ!-Freigabe zeigt nach dem Speichern eine eigene Adresse für die VM an, in der
  Form `<vm-name>.xxxxxxxx.myfritz.net`. **Diese Adresse ist ab jetzt deine `YAPPA_DOMAIN`.**
  Die Adresse der FRITZ!Box selbst (`xxxxxxxx.myfritz.net`) zeigt bei IPv6 auf die Box
  statt auf die VM; damit kann die Zertifikatsausstellung scheitern.

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
| App: „Server nicht erreichbar“ beim Einladungslink | Caddy oder `control` läuft nicht, oder die Adresse im Link stimmt nicht | `https://<YAPPA_DOMAIN>/api/health`; der Link muss `server=https://<YAPPA_DOMAIN>/api` enthalten |
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
| LiveKit-Log: `nodeIP` ist eine `192.168.…`-Adresse | öffentliche Adresse nicht ermittelt | hat die VM Internet? `use_external_ip: true` in `livekit.home.yaml` |

## Was offen ist

- Kein TURN: wer hinter einer Firewall sitzt, die nur Port 443 durchlässt, kommt nicht
  rein. Kommt mit Stufe 2 (eigene Subdomain und Zertifikat nötig).
- Keine Überwachung: fällt der Mini-PC aus, merkt es niemand außer den Leuten im Call.
- Die VM selbst braucht Updates (`sudo apt update && sudo apt upgrade`).

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
