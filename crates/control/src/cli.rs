//! The commands of `yappa-control`. Bootstrap and emergency happen here, on the box, instead
//! of on a web page: whoever can run this binary already has the server.

use crate::{open_db, router, routes, store, AppState, Config};
use api_types::{InviteLink, Role, VersionPolicy};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub const USAGE: &str = "\
usage: yappa-control [serve]
       yappa-control invite [--admin] [--days N]
       yappa-control policy --min <version> [--kill <version>]...
       yappa-control channel [list | add <name>]
       yappa-control backup <file>
       yappa-control health

environment: LIVEKIT_API_KEY, LIVEKIT_API_SECRET, LIVEKIT_WS_URL, [LIVEKIT_HTTP_URL],
             [YAPPA_LISTEN=127.0.0.1:8090], [YAPPA_DATABASE_URL=sqlite://yappa.db],
             [YAPPA_UPDATE_BASE_URL], [YAPPA_PUBLIC_URL=https://<domain>/api],
             [YAPPA_LOG_JSON=1]";

/// The link a person clicks (or pastes) to join; the desktop app registers the scheme.
pub fn invite_link(config: &Config, code: &str) -> String {
    InviteLink {
        server: config.public_url.clone(),
        code: code.into(),
    }
    .to_link()
}

/// `invite` and `policy` need the database and the public address, not the LiveKit keys.
fn db_only_config() -> Config {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    let mut config =
        Config::local(&var("YAPPA_DATABASE_URL").unwrap_or_else(|| "sqlite://yappa.db".into()));
    if let Some(url) = var("YAPPA_PUBLIC_URL") {
        config.public_url = url.trim_end_matches('/').into();
    } else if let Some(ws) = var("LIVEKIT_WS_URL").filter(|ws| ws.starts_with("wss://")) {
        config.public_url = format!("https://{}/api", &ws["wss://".len()..]);
    }
    config
}

/// Runs the API until Ctrl-C / SIGTERM.
pub async fn serve() -> Result<(), String> {
    let config = Config::from_env()?;
    let (addr, server) = start(config).await?;
    tracing::info!(%addr, "yappa-control listening");
    server.await.map_err(|e| e.to_string())?
}

/// Binds, bootstraps, and returns the address plus the running server — also what the
/// spike and integration tests use.
pub async fn start(
    config: Config,
) -> Result<(SocketAddr, tokio::task::JoinHandle<Result<(), String>>), String> {
    let db = open_db(&config.database_url)
        .await
        .map_err(|e| format!("database {}: {e}", config.database_url))?;
    let listener = TcpListener::bind(config.listen)
        .await
        .map_err(|e| format!("listen on {}: {e}", config.listen))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let state = AppState::new(db, config);
    bootstrap(&state).await?;
    let app = router(state);
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown())
            .await
            .map_err(|e| e.to_string())
    });
    Ok((addr, server))
}

/// First start: nobody exists and no invite is open → make an admin invite and say so.
async fn bootstrap(state: &AppState) -> Result<(), String> {
    let db = &state.db;
    let empty = store::count_users(db).await.map_err(|e| e.to_string())? == 0;
    let none_open = store::open_invites(db).await.map_err(|e| e.to_string())? == 0;
    if empty && none_open {
        let invite = routes::new_invite(state, Role::Admin, "bootstrap", 7)
            .await
            .map_err(|e| e.message)?;
        tracing::warn!(
            link = %invite_link(&state.config, &invite.code),
            "no members yet — this admin invite works once, for seven days"
        );
    }
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
    tracing::info!("shutting down");
}

pub async fn invite(args: &[String]) -> Result<(), String> {
    let admin = args.iter().any(|a| a == "--admin");
    let days = value(args, "--days")
        .map(|d| {
            d.parse::<u32>()
                .map_err(|_| format!("--days {d}: not a number"))
        })
        .transpose()?
        .unwrap_or(7);
    let config = db_only_config();
    let db = open_db(&config.database_url)
        .await
        .map_err(|e| e.to_string())?;
    let state = AppState::new(db, config);
    let role = if admin { Role::Admin } else { Role::Member };
    let invite = routes::new_invite(&state, role, "cli", days)
        .await
        .map_err(|e| e.message)?;
    println!("{}", invite_link(&state.config, &invite.code));
    eprintln!(
        "{} invite, valid until {}, works once",
        if admin { "admin" } else { "member" },
        invite.expires_at
    );
    Ok(())
}

pub async fn policy(args: &[String]) -> Result<(), String> {
    let min_version = value(args, "--min").ok_or("policy needs --min <version>")?;
    let killed_versions = args
        .windows(2)
        .filter(|w| w[0] == "--kill")
        .map(|w| w[1].clone())
        .collect();
    let config = db_only_config();
    let db = open_db(&config.database_url)
        .await
        .map_err(|e| e.to_string())?;
    let state = AppState::new(db, config);
    let policy = VersionPolicy {
        min_version,
        killed_versions,
    };
    routes::store_policy(&state, &policy)
        .await
        .map_err(|e| e.message)?;
    store::log_event(
        &state.db,
        "policy",
        None,
        None,
        &format!(
            "CLI setzt Mindestversion {} (gesperrt: {:?})",
            policy.min_version, policy.killed_versions
        ),
    )
    .await
    .map_err(|e| e.to_string())?;
    println!(
        "min version {} · killed {:?}",
        policy.min_version, policy.killed_versions
    );
    Ok(())
}

/// Channels from the box, until the admin module (U6) does it in the app.
pub async fn channel(args: &[String]) -> Result<(), String> {
    let config = db_only_config();
    let db = open_db(&config.database_url)
        .await
        .map_err(|e| e.to_string())?;
    if args.first().map(String::as_str) == Some("add") {
        let name = args[1..].join(" ");
        let name = name.trim();
        if !(2..=32).contains(&name.chars().count()) {
            return Err("channel add <name>: the name needs 2 to 32 characters".into());
        }
        let mut id = crate::ids::channel_slug(name);
        if store::channel(&db, &id)
            .await
            .map_err(|e| e.to_string())?
            .is_some()
            || id == "presence"
        {
            id = format!("{id}-2");
        }
        store::create_channel(&db, &id, name, &config.server_id)
            .await
            .map_err(|e| e.to_string())?;
        store::log_event(
            &db,
            "channel_created",
            None,
            Some(&id),
            &format!("CLI legt den Channel {name} an"),
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    for c in store::channels(&db).await.map_err(|e| e.to_string())? {
        println!("{:>2}  {:<24} {}", c.position, c.name, c.id);
    }
    Ok(())
}

/// `backup <file>`: safe while the server runs. The counts are the check that this was the
/// right database — `open_db` creates a missing one, and an empty backup looks like a backup.
pub async fn backup(args: &[String]) -> Result<(), String> {
    let [target] = args else {
        return Err("backup needs exactly one target file".into());
    };
    let config = db_only_config();
    let db = open_db(&config.database_url)
        .await
        .map_err(|e| format!("database {}: {e}", config.database_url))?;
    store::backup_to(&db, std::path::Path::new(target)).await?;
    let members = store::count_users(&db).await.map_err(|e| e.to_string())?;
    let channels = store::channels(&db).await.map_err(|e| e.to_string())?.len();
    println!("{target}: {members} members, {channels} channels");
    Ok(())
}

/// `health`, for the container healthcheck: the image has no curl, and this asks the same
/// route Caddy publishes. Uses `YAPPA_LISTEN` and nothing else.
pub async fn health() -> Result<(), String> {
    let listen = std::env::var("YAPPA_LISTEN")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1:8090".into());
    let addr: SocketAddr = listen.parse().map_err(|e| format!("YAPPA_LISTEN: {e}"))?;
    check_health(addr).await
}

/// Asks `GET /health` at `addr`; fails on anything but a 200 within five seconds.
pub async fn check_health(addr: SocketAddr) -> Result<(), String> {
    let ask = async {
        let mut stream = TcpStream::connect(addr).await?;
        stream
            .write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .await?;
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).await?;
        Ok::<_, std::io::Error>(answer)
    };
    let answer = tokio::time::timeout(Duration::from_secs(5), ask)
        .await
        .map_err(|_| format!("{addr}: no answer within five seconds"))?
        .map_err(|e| format!("{addr}: {e}"))?;
    let answer = String::from_utf8_lossy(&answer);
    let status = answer.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) == Some("200") {
        Ok(())
    } else {
        Err(format!("{addr}: {status}"))
    }
}

fn value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
