//! The commands of `yappa-control`. Bootstrap and emergency happen here, on the box, instead
//! of on a web page: whoever can run this binary already has the server.

use crate::{open_db, router, routes, store, AppState, Config};
use api_types::{InviteLink, Role, VersionPolicy};
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub const USAGE: &str = "\
usage: yappa-control [serve]
       yappa-control invite [--admin] [--days N]
       yappa-control policy --min <version> [--kill <version>]...

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

fn value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
