//! The yAPPA control plane (ADR-001, ADR-005): who belongs to the gang, which channels exist,
//! LiveKit tokens for voice rooms and the presence room, remote config (minimum version, kill
//! switch) and an event log fed by LiveKit webhooks. HTTP only — nothing here is in a live
//! path: a running call and the presence view survive a restart of this process.
//!
//! The binary (`yappa-control`) serves the API and doubles as the bootstrap and emergency
//! tool on the box (`invite --admin`, `policy --min …`).

mod auth;
pub mod cli;
pub mod config;
mod error;
mod ids;
pub mod store;
mod tokens;

mod routes;

use axum::Router;
use livekit_api::services::room::RoomClient;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Arc;

pub use config::Config;

/// Everything a request handler may touch.
pub struct AppState {
    pub db: SqlitePool,
    pub config: Config,
    /// Server API of LiveKit, for removing participants (kick).
    pub rooms: RoomClient,
}

impl AppState {
    pub fn new(db: SqlitePool, config: Config) -> Arc<Self> {
        let rooms = RoomClient::with_api_key(
            &config.livekit_http_url,
            &config.livekit_api_key,
            &config.livekit_api_secret,
        );
        Arc::new(Self { db, config, rooms })
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    routes::router().with_state(state)
}

/// Opens (and creates) the database and brings the schema up to date.
pub async fn open_db(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    // In-memory databases exist per connection; tests need exactly one.
    let max = if url.contains(":memory:") { 1 } else { 4 };
    let pool = SqlitePoolOptions::new()
        .max_connections(max)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
