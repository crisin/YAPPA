//! The client against a real control plane on a random local port: invite link → session
//! stored → restart with the session → cache when the server is gone → logout. No LiveKit
//! (presence is measured by spike S6); the session token goes to the file store here so the
//! test leaves nothing in the developer's credential manager.

use api_types::{ClientInfo, InviteLink, Role};
use client::{Client, Profile};
use control::{open_db, store, Config};
use std::path::PathBuf;

fn info() -> ClientInfo {
    ClientInfo {
        app_version: "0.1.0".into(),
        protocol_version: 1,
        platform: "test".into(),
    }
}

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("yappa-client-{name}-{}", std::process::id()))
}

#[tokio::test]
async fn from_invite_link_to_logout() {
    // This test binary has one test, so nothing else reads the environment meanwhile.
    std::env::set_var("YAPPA_NO_KEYRING", "1");
    let db = temp("db").with_extension("db");
    let url = format!("sqlite://{}", db.display().to_string().replace('\\', "/"));
    let mut config = Config::local(&url);
    let (addr, server) = control::cli::start(config.clone()).await.unwrap();
    config.public_url = format!("http://{addr}");
    let pool = open_db(&url).await.unwrap();
    let invite = store::create_invite(&pool, "abcd-efgh-jkmn", Role::Admin, "test", 1)
        .await
        .unwrap();
    let link = InviteLink {
        server: config.public_url.clone(),
        code: invite.code,
    }
    .to_link();

    let base = temp("profile");
    let profile = Profile::new(&base, Some("eins"));
    let client = Client::new(profile.clone(), info());
    assert!(client.session().is_none());
    let bad = client.redeem("hallo", "Krille").await.unwrap_err();
    assert_eq!(bad.code, "invite_invalid");
    let session = client
        .redeem(&format!("  {link}\n"), "Krille")
        .await
        .unwrap();
    assert_eq!(session.user.role, Role::Admin);
    assert_eq!(session.presence.ws_url, "ws://localhost:7880");

    // A restart: the same profile knows the session without asking for anything.
    let restarted = Client::new(profile.clone(), info());
    let record = restarted.session().expect("session survives a restart");
    assert_eq!(record.user.display_name, "Krille");
    assert_eq!(restarted.me().await.unwrap().user.id, session.user.id);
    let channels = restarted.channels().await.unwrap();
    assert_eq!(channels[0].id, "lobby");
    restarted.config().await.unwrap();
    let token = restarted.voice_token("lobby").await.unwrap();
    assert!(!token.token.is_empty());
    let other = Client::new(Profile::new(&base, Some("zwei")), info());
    assert!(other.session().is_none(), "profiles do not share logins");

    // The server goes away and the app starts again: cached channels and config carry on,
    // new tokens do not. (A fresh client — the old one's keep-alive connection would still
    // reach the aborted server's connection task in-process.)
    server.abort();
    let _ = server.await;
    let offline = Client::new(profile.clone(), info());
    assert_eq!(offline.channels().await.unwrap(), channels);
    assert!(offline.config().await.is_ok());
    assert_eq!(
        offline.voice_token("lobby").await.unwrap_err().code,
        "unreachable"
    );

    offline.logout().await;
    assert!(offline.session().is_none());
    assert!(Client::new(profile, info()).session().is_none());
    let _ = std::fs::remove_dir_all(base);
    let _ = std::fs::remove_file(db);
}

/// Needs the local LiveKit (`cargo xtask livekit`); run with
/// `cargo test -p client --test against_control -- --ignored`.
#[tokio::test]
#[ignore = "needs the local LiveKit"]
async fn two_clients_see_each_other_in_the_presence_room() {
    use api_types::{PresenceEntry, PresenceState};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    std::env::set_var("YAPPA_NO_KEYRING", "1");
    let db = temp("live-db").with_extension("db");
    let url = format!("sqlite://{}", db.display().to_string().replace('\\', "/"));
    let mut config = Config::local(&url);
    let (addr, _server) = control::cli::start(config.clone()).await.unwrap();
    config.public_url = format!("http://{addr}");
    let pool = open_db(&url).await.unwrap();
    let base = temp("live-profile");

    let mut clients = Vec::new();
    for (i, name) in ["Krille", "Ben"].iter().enumerate() {
        let code = format!("live-code-{i}");
        store::create_invite(&pool, &code, Role::Member, "test", 1)
            .await
            .unwrap();
        let link = InviteLink {
            server: config.public_url.clone(),
            code,
        }
        .to_link();
        let client = Client::new(Profile::new(&base, Some(name)), info());
        let session = client.redeem(&link, name).await.unwrap();
        clients.push((client, session));
    }
    let seen: Arc<Mutex<Vec<PresenceEntry>>> = Arc::default();
    let for_ben = seen.clone();
    let krille = clients[0]
        .0
        .start_presence(&clients[0].1.presence, PresenceState::default(), |_| {})
        .unwrap();
    let _ben = clients[1]
        .0
        .start_presence(
            &clients[1].1.presence,
            PresenceState::default(),
            move |entries| {
                *for_ben.lock().unwrap() = entries;
            },
        )
        .unwrap();

    let wait = |want: &dyn Fn(&[PresenceEntry]) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if want(&seen.lock().unwrap()) {
                return true;
            }
            if Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    assert!(
        wait(&|e| e.iter().any(|p| p.display_name == "Krille")),
        "Ben sees Krille online"
    );
    krille.update(|s| s.channel_id = Some("lobby".into()));
    assert!(
        wait(&|e| e
            .iter()
            .any(|p| p.display_name == "Krille" && p.state.channel_id.as_deref() == Some("lobby"))),
        "Ben sees Krille in the lobby"
    );
    assert_eq!(seen.lock().unwrap()[0].display_name, "Ben", "me first");
    drop(krille);
    assert!(
        wait(&|e| !e.iter().any(|p| p.display_name == "Krille")),
        "Ben sees Krille leave"
    );
    let _ = std::fs::remove_dir_all(base);
}
