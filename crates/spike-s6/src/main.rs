//! S6 spike (scratch, `publish = false`): does the presence room of ADR-005 carry "who is
//! where", and does the control plane hand out the right tokens? Needs the local LiveKit
//! (`cargo xtask livekit`) and port 8090 free (the dev webhook points there).
//!
//!   cargo run --release -p spike-s6            the whole run, prints a markdown table
//!   cargo run --release -p spike-s6 -- child <url> <token>
//!                                              (internal) a presence client that gets killed
//!
//! Measured on one machine: everything is loopback, so the numbers are LiveKit's own
//! processing plus the SDK, not the internet.

use api_types::{ClientInfo, PresenceState, Role, Session, TalkMode, VoiceToken};
use control::{store, Config};
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_source::{AudioSourceOptions, RtcAudioSource};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;

const LIVEKIT: &str = "ws://localhost:7880";
const CONTROL: &str = "127.0.0.1:8090";
const ATTRIBUTE_ROUNDS: usize = 20;

struct Row {
    what: String,
    value: String,
    method: String,
}

fn row(rows: &mut Vec<Row>, what: &str, value: impl Into<String>, method: &str) {
    let value = value.into();
    println!("  {what}: {value}");
    rows.push(Row {
        what: what.into(),
        value,
        method: method.into(),
    });
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("child") {
        return child(&args[1], &args[2]).await;
    }

    let db = std::env::temp_dir().join(format!("yappa-s6-{}.db", std::process::id()));
    let mut config = Config::local(&format!("sqlite://{}", db.display()));
    config.listen = CONTROL.parse().expect("address");
    let (addr, _server) = control::cli::start(config).await?;
    let base = format!("http://{addr}");
    let http = reqwest::Client::new();
    let state_db = control::open_db(&format!("sqlite://{}", db.display()))
        .await
        .map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    println!("control plane on {base}, database {}", db.display());

    // ── login ──────────────────────────────────────────────────────────────────────────
    let mut sessions = Vec::new();
    for (name, role) in [
        ("Krille", Role::Admin),
        ("Ben", Role::Member),
        ("Sasha", Role::Member),
        ("Tim", Role::Member),
    ] {
        let code = store::create_invite(
            &state_db,
            &format!("s6-{}", name.to_lowercase()),
            role,
            "spike",
            1,
        )
        .await
        .map_err(|e| e.to_string())?
        .code;
        let started = Instant::now();
        let session: Session = http
            .post(format!("{base}/auth/invite"))
            .json(&serde_json::json!({
                "code": code,
                "displayName": name,
                "client": ClientInfo { app_version: "0.1.0".into(), protocol_version: 1, platform: "windows".into() },
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if name == "Krille" {
            row(
                &mut rows,
                "Login per invite (POST /auth/invite)",
                format!("{:.1} ms", ms(started.elapsed())),
                "HTTP round trip incl. SQLite + JWT, loopback",
            );
        }
        sessions.push(session);
    }
    let [krille, ben, sasha, tim] = <[Session; 4]>::try_from(sessions).map_err(|_| "four")?;

    // ── presence: connect ──────────────────────────────────────────────────────────────
    let started = Instant::now();
    let (room_a, _events_a) = connect(&krille.presence).await?;
    row(
        &mut rows,
        "Connect to the presence room",
        format!("{:.0} ms", ms(started.elapsed())),
        "Room::connect, no media, auto_subscribe off",
    );
    let (room_b, mut events_b) = connect(&ben.presence).await?;

    // ── attribute latency A → B ────────────────────────────────────────────────────────
    let mut latencies = Vec::new();
    for round in 0..ATTRIBUTE_ROUNDS {
        let state = PresenceState {
            channel_id: Some(if round % 2 == 0 { "lobby" } else { "raid" }.into()),
            app: "nightly 0.1.0".into(),
            talk: TalkMode::Vad,
            away: false,
        };
        let sent = Instant::now();
        room_a
            .local_participant()
            .set_attributes(state.to_attributes())
            .await
            .map_err(|e| format!("set_attributes: {e}"))?;
        let want = state.channel_id.clone();
        let seen = wait_for(&mut events_b, Duration::from_secs(5), |e| {
            matches!(e, RoomEvent::ParticipantAttributesChanged { participant, changed_attributes }
                if participant.identity().as_str() == krille.user.id
                    && changed_attributes.get(PresenceState::KEY_CHANNEL) == want.as_ref())
        })
        .await;
        match seen {
            Some(at) => latencies.push(ms(at - sent)),
            None => println!("  round {round}: not seen within 5 s"),
        }
        // Attributes are for state that changes every few seconds at most (LiveKit docs);
        // a channel switch is that kind of change.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    latencies.sort_by(f64::total_cmp);
    if latencies.is_empty() {
        row(&mut rows, "Attribute change A → B", "never arrived", "");
    } else {
        let median = latencies[latencies.len() / 2];
        let p95 = latencies[(latencies.len() * 95 / 100).min(latencies.len() - 1)];
        row(
            &mut rows,
            "Attribute change A → B (channel switch)",
            format!(
                "median {median:.1} · p95 {p95:.1} · max {:.1} ms (n={}/{ATTRIBUTE_ROUNDS})",
                latencies[latencies.len() - 1],
                latencies.len()
            ),
            "set_attributes until ParticipantAttributesChanged at B, 500 ms apart",
        );
    }

    // ── late joiner ────────────────────────────────────────────────────────────────────
    room_a
        .local_participant()
        .set_attributes(
            PresenceState {
                channel_id: Some("raid".into()),
                app: "nightly 0.1.0".into(),
                talk: TalkMode::Ptt,
                away: true,
            }
            .to_attributes(),
        )
        .await
        .map_err(|e| e.to_string())?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (room_c, _events_c) = connect(&sasha.presence).await?;
    let seen_by_c: Option<PresenceState> = room_c
        .remote_participants()
        .values()
        .find(|p| p.identity().as_str() == krille.user.id)
        .map(|p| PresenceState::from_attributes(&p.attributes()));
    row(
        &mut rows,
        "Late joiner sees the current state at connect",
        match &seen_by_c {
            Some(s)
                if s.channel_id.as_deref() == Some("raid") && s.talk == TalkMode::Ptt && s.away =>
            {
                "yes — channel, talk mode and away present right after connect".to_string()
            }
            Some(s) => format!("partly: {s:?}"),
            None => "no — participant not listed yet".to_string(),
        },
        "remote_participants() of a third client immediately after Room::connect returns",
    );

    // ── leaves: clean, killed, kicked ──────────────────────────────────────────────────
    // B must know C before C leaves: a join and a leave in the same server update are
    // folded into nothing by the SDK (first run of this spike).
    let known = wait_for(&mut events_b, Duration::from_secs(10), |e| {
        matches!(e, RoomEvent::ParticipantConnected(p) if p.identity().as_str() == sasha.user.id)
    })
    .await;
    if known.is_none() {
        println!("  B never saw C arrive");
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    let started = Instant::now();
    room_c.close().await.map_err(|e| e.to_string())?;
    let gone = wait_disconnect(&mut events_b, &sasha.user.id, Duration::from_secs(60)).await;
    row(
        &mut rows,
        "Clean leave noticed by others",
        seconds(gone.map(|t| t - started)),
        "Room::close at C until ParticipantDisconnected at B",
    );

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = tokio::process::Command::new(exe)
        .args(["child", &tim.presence.ws_url, &tim.presence.token])
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;
    let connected = wait_for(
        &mut events_b,
        Duration::from_secs(20),
        |e| matches!(e, RoomEvent::ParticipantConnected(p) if p.identity().as_str() == tim.user.id),
    )
    .await;
    if connected.is_some() {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let started = Instant::now();
        child.kill().await.map_err(|e| e.to_string())?;
        let gone = wait_disconnect(&mut events_b, &tim.user.id, Duration::from_secs(90)).await;
        row(
            &mut rows,
            "Killed client (process ended, no goodbye) noticed by others",
            seconds(gone.map(|t| t - started)),
            "TerminateProcess on a child client until ParticipantDisconnected at B",
        );
    } else {
        row(
            &mut rows,
            "Killed client noticed by others",
            "not measured — child never connected",
            "",
        );
    }

    let started = Instant::now();
    let kicked = http
        .post(format!("{base}/members/{}/kick", ben.user.id))
        .bearer_auth(&krille.session_token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .status();
    let out = wait_for(&mut events_b, Duration::from_secs(10), |e| {
        matches!(e, RoomEvent::Disconnected { .. })
    })
    .await;
    row(
        &mut rows,
        "Kick via API reaches the kicked client",
        match out {
            Some(t) => format!("{:.0} ms (HTTP {kicked})", ms(t - started)),
            None => format!("not within 10 s (HTTP {kicked})"),
        },
        "POST /members/{id}/kick (RoomService remove_participant) until RoomEvent::Disconnected at B",
    );
    drop(room_b);

    // ── grants ─────────────────────────────────────────────────────────────────────────
    let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
    let track = LocalAudioTrack::create_audio_track("probe", RtcAudioSource::Native(source));
    let started = Instant::now();
    let publish = room_a
        .local_participant()
        .publish_track(LocalTrack::Audio(track), TrackPublishOptions::default())
        .await;
    let refused_after = started.elapsed();
    row(
        &mut rows,
        "Presence token refuses media",
        match publish {
            Err(e) => format!(
                "yes — publish_track failed after {:.1} s: {e}",
                refused_after.as_secs_f64()
            ),
            Ok(_) => "NO — the presence token could publish audio".into(),
        },
        "publish_track with the presence token",
    );

    let voice: VoiceToken = http
        .post(format!("{base}/channels/lobby/token"))
        .bearer_auth(&krille.session_token)
        .header("x-yappa-version", "0.1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let (voice_room, _ev) = connect(&voice).await?;
    let attrs = voice_room
        .local_participant()
        .set_attributes(HashMap::from([("channel".to_string(), "x".to_string())]))
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let applied = voice_room
        .local_participant()
        .attributes()
        .contains_key("channel");
    row(
        &mut rows,
        "Voice token refuses own attributes",
        match (attrs, applied) {
            (Err(e), _) => format!("yes — set_attributes failed: {e}"),
            (Ok(()), false) => "yes — the server ignored the change".into(),
            (Ok(()), true) => "NO — attributes changed with a voice token".into(),
        },
        "set_attributes in a channel room",
    );
    voice_room.close().await.ok();
    room_a.close().await.ok();

    // ── webhooks ───────────────────────────────────────────────────────────────────────
    tokio::time::sleep(Duration::from_secs(3)).await;
    let events = store::events(&state_db, 100)
        .await
        .map_err(|e| e.to_string())?;
    let from_webhook = events
        .iter()
        .filter(|e| ["online", "offline", "joined", "left"].contains(&e.kind.as_str()))
        .count();
    row(
        &mut rows,
        "Webhook lines in the event log",
        if from_webhook == 0 {
            "none — LiveKit could not reach the control plane (dev webhook config?)".to_string()
        } else {
            format!("{from_webhook} (online/offline/joined/left)")
        },
        "LiveKit → http://host.docker.internal:8090/livekit/webhook, signed, counted after the run",
    );
    for e in events.iter().take(12) {
        println!("    {} {:<16} {}", e.time, e.kind, e.message);
    }

    println!("\n| What | Value | Method |\n| --- | --- | --- |");
    for r in &rows {
        println!("| {} | {} | {} |", r.what, r.value, r.method);
    }
    let _ = std::fs::remove_file(&db);
    Ok(())
}

async fn connect(token: &VoiceToken) -> Result<(Room, UnboundedReceiver<RoomEvent>), String> {
    let mut options = RoomOptions::default();
    options.auto_subscribe = false;
    let url = if token.ws_url.is_empty() {
        LIVEKIT
    } else {
        &token.ws_url
    };
    Room::connect(url, &token.token, options)
        .await
        .map_err(|e| format!("connect: {e}"))
}

/// The first event matching `want` within `timeout`, stamped when it was taken off the queue.
async fn wait_for(
    events: &mut UnboundedReceiver<RoomEvent>,
    timeout: Duration,
    want: impl Fn(&RoomEvent) -> bool,
) -> Option<Instant> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, events.recv()).await {
            Ok(Some(event)) if want(&event) => return Some(Instant::now()),
            Ok(Some(_)) => continue,
            _ => return None,
        }
    }
}

async fn wait_disconnect(
    events: &mut UnboundedReceiver<RoomEvent>,
    identity: &str,
    timeout: Duration,
) -> Option<Instant> {
    wait_for(
        events,
        timeout,
        |e| matches!(e, RoomEvent::ParticipantDisconnected(p) if p.identity().as_str() == identity),
    )
    .await
}

/// A presence client that just sits there until its process is killed.
async fn child(url: &str, token: &str) -> Result<(), String> {
    let mut options = RoomOptions::default();
    options.auto_subscribe = false;
    let (_room, mut events) = Room::connect(url, token, options)
        .await
        .map_err(|e| e.to_string())?;
    while events.recv().await.is_some() {}
    Ok(())
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn seconds(d: Option<Duration>) -> String {
    d.map_or_else(
        || "not within the timeout".to_string(),
        |d| format!("{:.1} s", d.as_secs_f64()),
    )
}
