# yAPPA hinter einem Cloudflare Tunnel

Für einen Server, auf dem schon Apps in Docker laufen und über einen **Cloudflare Tunnel**
(`cloudflared`) erreichbar sind. Dann braucht yAPPA kein Caddy, kein Zertifikat und keine
Freigaben für 80/443: Cloudflare übernimmt TLS. Ohne Tunnel gilt
[Homeserver einrichten](homeserver.md).

> **Stand 2026-10-23:** Die Compose-Datei ist da, aber noch nie mit echtem Tunnel gelaufen.
> Was beim ersten Mal hakt, bitte hier nachtragen.

## Das Wichtigste vorweg: Die Sprache geht nicht durch den Tunnel

Ein Cloudflare Tunnel leitet für öffentliche Hostnamen nur **HTTP und WebSocket** weiter.
Darüber laufen die Anmeldung, die Einladungen und das Signalisieren bei LiveKit. Die Sprache
selbst ist **UDP** und muss direkt zum Server, sonst gibt es keinen Ton. Deshalb braucht es
trotz Tunnel zwei Freigaben im Router:

```text
Gang ──https──► Cloudflare ──Tunnel──► cloudflared ─┬─► yappa-control :8090   yappa-api.example.com
                                                    └─► LiveKit       :7880   voice.example.com
Gang ──UDP 7882 / TCP 7881 ──► Router ──────────────────► LiveKit                die Sprache
```

Das heißt auch: Der Anschluss braucht eine **öffentliche IPv4-Adresse**. Bei DS-Lite oder
CGNAT (viele Kabel- und Glasfaseranschlüsse) kommt UDP von außen nicht an — daran ändert der
Tunnel nichts. Prüfen wie in [Homeserver, Schritt 1](homeserver.md#1-vorab-pruefen-hast-du-eine-oeffentliche-ipv4-adresse).

Was Cloudflare dabei sieht: Anmeldungen und Signalisierung. Die Sprache nicht — die läuft
verschlüsselt direkt zwischen App und LiveKit.

## Was du brauchst

- Einen **Linux**-Rechner mit Docker. LiveKit läuft mit Host-Netzwerk (wie beim
  Homeserver), das geht mit Docker Desktop unter Windows oder macOS nicht. Unraid, Synology,
  Proxmox-VM oder ein normales Debian/Ubuntu gehen.
- Eine Domain bei Cloudflare und einen Tunnel. Läuft `cloudflared` schon für andere Apps,
  wird er einfach mitbenutzt.
- Zugang zum Router für zwei Portfreigaben.
- 2 CPU-Kerne, 4 GB RAM für den ersten Build der Control-Plane (danach deutlich weniger).

## 1. Portfreigaben im Router

Auf die feste LAN-Adresse des Servers:

| Protokoll | Port | Wofür |
| --- | --- | --- |
| UDP | 7882 | die Sprache |
| TCP | 7881 | Ausweichweg, wenn bei jemandem UDP gesperrt ist |

80, 443 und 7880 werden **nicht** freigegeben — die gehen über den Tunnel.

## 2. yAPPA auf den Server holen

```bash
git clone https://github.com/crisin/YAPPA.git ~/yappa
cd ~/yappa/infra/livekit
```

Die `.env` (bleibt auf dem Server, nie ins Repo oder in einen Chat):

```bash
cat > .env <<EOF
LIVEKIT_API_SECRET=$(openssl rand -hex 32)
YAPPA_VOICE_HOST=voice.example.com
YAPPA_API_HOST=yappa-api.example.com
EOF
chmod 600 .env
nano .env      # die beiden Hostnamen auf deine Domain anpassen
```

Zwei Hostnamen, weil der Tunnel keinen Pfad abschneiden kann (Caddy macht beim Homeserver
aus `/api/…` den Pfad der Control-Plane; `cloudflared` kann das nicht).

**Noch kein Tunnel auf dem Server?** Im Cloudflare-Dashboard unter *Zero Trust → Networks →
Tunnels* einen anlegen, den Token aus dem Installationsbefehl kopieren und als vierte Zeile
eintragen: `CLOUDFLARE_TUNNEL_TOKEN=…`. Dann startet Schritt 4 den Tunnel gleich mit.

## 3. Hostnamen im Tunnel eintragen

Im Cloudflare-Dashboard beim Tunnel unter *Public Hostname* (oder in der `config.yml` von
`cloudflared`, falls der Tunnel lokal verwaltet wird) zwei Einträge:

| Hostname | Service |
| --- | --- |
| `voice.example.com` | `http://localhost:7880` |
| `yappa-api.example.com` | `http://localhost:8090` |

Wo `localhost` stimmt und wo nicht:

- `cloudflared` als Dienst direkt auf dem Server, oder per `--profile cloudflared` aus
  Schritt 4: `localhost` passt.
- `cloudflared` als Container in einem eigenen Docker-Netz (häufig, wenn schon andere Apps
  darüber laufen): `localhost` ist dann der Container selbst. Stattdessen die LAN-Adresse
  des Servers eintragen, z. B. `http://192.168.1.10:7880` und `http://192.168.1.10:8090`.

**Kein Cloudflare Access davor.** Eine Login-Seite vor diesen beiden Hostnamen sperrt die
App aus — sie kann sich dort nicht anmelden. Wer reindarf, regelt yAPPA mit seinen
Einladungen selbst.

WebSockets sind bei Cloudflare von Haus aus an; sonst ist nichts einzustellen.

## 4. Starten und prüfen

```bash
docker compose -f tunnel.yml up -d --build                        # Tunnel läuft schon
docker compose -f tunnel.yml --profile cloudflared up -d --build  # Tunnel kommt mit
docker compose -f tunnel.yml ps
```

Der erste Start baut die Control-Plane, das dauert ein paar Minuten.

- `https://yappa-api.example.com/health` im Browser (am Handy ohne WLAN): `ok`.
- `https://voice.example.com` zeigt eine kurze Antwort von LiveKit, keine Fehlerseite von
  Cloudflare.
- `docker compose -f tunnel.yml logs livekit | grep nodeIP` zeigt die **öffentliche**
  IPv4-Adresse des Anschlusses. Dorthin schicken die Apps die Sprache.

## 5. Admin-Link und Einladungen

```bash
docker compose -f tunnel.yml logs control | grep invite
```

Der Link sieht aus wie `yappa://invite/xxxx-xxxx-xxxx?server=https://yappa-api.example.com`.
Auf dem eigenen PC öffnen, Name, Mikro und Sprechmodus einrichten — danach gibt es in der
App links unten **Admin**: Einladungen für die Gang, Channels, Versionen. Alles Weitere wie
in [Homeserver, Schritt 8](homeserver.md#8-die-gang-einladen); in den Befehlen dort nur
`home.yml` durch `tunnel.yml` ersetzen.

## Wenn es nicht geht

| Was passiert | Warum | Was tun |
| --- | --- | --- |
| Einladung: „Server nicht erreichbar“ | Tunnel-Eintrag für die API fehlt oder zeigt ins Leere | `https://yappa-api…/health` prüfen; bei `cloudflared` im Container die LAN-Adresse statt `localhost` |
| Cloudflare-Fehlerseite 502 / 1033 | `cloudflared` erreicht den Dienst nicht, oder der Tunnel ist down | `docker compose -f tunnel.yml ps`; Ziel-Adresse im Tunnel prüfen |
| Eine Login-Seite von Cloudflare | Cloudflare Access liegt vor dem Hostnamen | Access-Regel für die zwei yAPPA-Hostnamen entfernen |
| „Verbunden“, aber niemand hört etwas | UDP 7882 / TCP 7881 kommt nicht an, oder `nodeIP` ist nicht die öffentliche Adresse | Portfreigaben prüfen; DS-Lite/CGNAT ausschließen |
| Ton bricht nach einer Weile ab, Neuverbinden hilft | Router oder Firewall vergisst die UDP-Zuordnung | Diagnose-Datei speichern ([Mittesten](mittesten.md)) und melden |

## Betrieb

Wie beim [Homeserver](homeserver.md#betrieb), nur mit `tunnel.yml`:

| Was | Befehl (im Ordner `~/yappa/infra/livekit`) |
| --- | --- |
| Status | `docker compose -f tunnel.yml ps` |
| Logs | `docker compose -f tunnel.yml logs -f livekit control` |
| yAPPA aktualisieren | `git pull && docker compose -f tunnel.yml up -d --build` — nicht während jemand spricht |
| Datenbank sichern | `docker compose -f tunnel.yml cp control:/data/yappa.db ./yappa-backup.db` |
