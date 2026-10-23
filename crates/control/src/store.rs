//! All SQL in one place. Plain runtime queries (no compile-time macros), so building needs no
//! database. Times are RFC 3339 strings in UTC — they sort as text.

use api_types::{Channel, ClientInfo, EventEntry, InviteCode, Member, Role, User};
use chrono::{SecondsFormat, Utc};
use sqlx::SqlitePool;

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn in_days(days: u32) -> String {
    (Utc::now() + chrono::Duration::days(i64::from(days)))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn role_text(role: Role) -> &'static str {
    match role {
        Role::Member => "member",
        Role::Admin => "admin",
    }
}

fn role_of(text: &str) -> Role {
    if text == "admin" {
        Role::Admin
    } else {
        Role::Member
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    display_name: String,
    role: String,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            display_name: row.display_name,
            role: role_of(&row.role),
        }
    }
}

// ── users and sessions ─────────────────────────────────────────────────────────────────

pub async fn user_by_session(db: &SqlitePool, hash: &str) -> sqlx::Result<Option<User>> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT u.id, u.display_name, u.role FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ?",
    )
    .bind(hash)
    .fetch_optional(db)
    .await?;
    Ok(row.map(User::from))
}

pub async fn user(db: &SqlitePool, id: &str) -> sqlx::Result<Option<User>> {
    let row: Option<UserRow> =
        sqlx::query_as("SELECT id, display_name, role FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
    Ok(row.map(User::from))
}

pub async fn user_version(db: &SqlitePool, id: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT app_version FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
        .map(Option::flatten)
}

/// Last seen now; the app version too when the request told it.
pub async fn touch(db: &SqlitePool, id: &str, version: Option<&str>) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE users SET last_seen = ?, app_version = COALESCE(?, app_version) WHERE id = ?",
    )
    .bind(now())
    .bind(version)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn create_user(
    db: &SqlitePool,
    user: &User,
    client: &ClientInfo,
    token_hash: &str,
) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    let now = now();
    sqlx::query(
        "INSERT INTO users (id, display_name, role, created_at, app_version, protocol_version,
                            platform, last_seen)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&user.id)
    .bind(&user.display_name)
    .bind(role_text(user.role))
    .bind(&now)
    .bind(&client.app_version)
    .bind(client.protocol_version)
    .bind(&client.platform)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO sessions (token_hash, user_id, created_at) VALUES (?, ?, ?)")
        .bind(token_hash)
        .bind(&user.id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

pub async fn delete_session(db: &SqlitePool, hash: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(hash)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn count_users(db: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(db)
        .await
}

pub async fn count_admins(db: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin'")
        .fetch_one(db)
        .await
}

pub async fn set_role(db: &SqlitePool, id: &str, role: Role) -> sqlx::Result<bool> {
    let done = sqlx::query("UPDATE users SET role = ? WHERE id = ?")
        .bind(role_text(role))
        .bind(id)
        .execute(db)
        .await?;
    Ok(done.rows_affected() == 1)
}

#[derive(sqlx::FromRow)]
struct MemberRow {
    id: String,
    display_name: String,
    role: String,
    app_version: Option<String>,
    protocol_version: Option<i64>,
    platform: Option<String>,
    last_seen: Option<String>,
}

pub async fn members(db: &SqlitePool) -> sqlx::Result<Vec<Member>> {
    let rows: Vec<MemberRow> = sqlx::query_as(
        "SELECT id, display_name, role, app_version, protocol_version, platform, last_seen
         FROM users ORDER BY display_name COLLATE NOCASE",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Member {
            client: r.app_version.map(|app_version| ClientInfo {
                app_version,
                protocol_version: r
                    .protocol_version
                    .and_then(|v| u32::try_from(v).ok())
                    .unwrap_or(0),
                platform: r.platform.unwrap_or_default(),
            }),
            last_seen: r.last_seen,
            user: User {
                id: r.id,
                display_name: r.display_name,
                role: role_of(&r.role),
            },
        })
        .collect())
}

// ── invites ────────────────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct InviteRow {
    code: String,
    role: String,
    created_by: String,
    created_at: String,
    expires_at: String,
    used_by: Option<String>,
}

impl InviteRow {
    fn public(self) -> InviteCode {
        InviteCode {
            code: self.code,
            created_by: self.created_by,
            created_at: self.created_at,
            expires_at: self.expires_at,
            used_by: self.used_by,
        }
    }
}

pub async fn create_invite(
    db: &SqlitePool,
    code: &str,
    role: Role,
    created_by: &str,
    days: u32,
) -> sqlx::Result<InviteCode> {
    let invite = InviteCode {
        code: code.into(),
        created_by: created_by.into(),
        created_at: now(),
        expires_at: in_days(days),
        used_by: None,
    };
    sqlx::query(
        "INSERT INTO invites (code, role, created_by, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&invite.code)
    .bind(role_text(role))
    .bind(&invite.created_by)
    .bind(&invite.created_at)
    .bind(&invite.expires_at)
    .execute(db)
    .await?;
    Ok(invite)
}

/// The role an unused, unexpired code grants.
pub async fn open_invite(db: &SqlitePool, code: &str) -> sqlx::Result<Option<Role>> {
    let row: Option<InviteRow> = sqlx::query_as(
        "SELECT code, role, created_by, created_at, expires_at, used_by FROM invites
         WHERE code = ? AND used_by IS NULL AND expires_at > ?",
    )
    .bind(code)
    .bind(now())
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| role_of(&r.role)))
}

/// Marks the code as used — only if it still is open (two people, one code, same second).
pub async fn use_invite(db: &SqlitePool, code: &str, user_id: &str) -> sqlx::Result<bool> {
    let done = sqlx::query("UPDATE invites SET used_by = ? WHERE code = ? AND used_by IS NULL")
        .bind(user_id)
        .bind(code)
        .execute(db)
        .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn invites(db: &SqlitePool) -> sqlx::Result<Vec<InviteCode>> {
    let rows: Vec<InviteRow> = sqlx::query_as(
        "SELECT code, role, created_by, created_at, expires_at, used_by FROM invites
         ORDER BY created_at DESC",
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(InviteRow::public).collect())
}

pub async fn open_invites(db: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM invites WHERE used_by IS NULL AND expires_at > ?")
        .bind(now())
        .fetch_one(db)
        .await
}

pub async fn delete_invite(db: &SqlitePool, code: &str) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM invites WHERE code = ?")
        .bind(code)
        .execute(db)
        .await?;
    Ok(done.rows_affected() == 1)
}

// ── channels ───────────────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct ChannelRow {
    id: String,
    name: String,
    position: i64,
    server_id: String,
}

impl From<ChannelRow> for Channel {
    fn from(r: ChannelRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            position: u32::try_from(r.position).unwrap_or(0),
            server_id: r.server_id,
        }
    }
}

pub async fn channels(db: &SqlitePool) -> sqlx::Result<Vec<Channel>> {
    let rows: Vec<ChannelRow> =
        sqlx::query_as("SELECT id, name, position, server_id FROM channels ORDER BY position, id")
            .fetch_all(db)
            .await?;
    Ok(rows.into_iter().map(Channel::from).collect())
}

pub async fn channel(db: &SqlitePool, id: &str) -> sqlx::Result<Option<Channel>> {
    let row: Option<ChannelRow> =
        sqlx::query_as("SELECT id, name, position, server_id FROM channels WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
    Ok(row.map(Channel::from))
}

pub async fn create_channel(
    db: &SqlitePool,
    id: &str,
    name: &str,
    server_id: &str,
) -> sqlx::Result<Channel> {
    let position: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM channels")
        .fetch_one(db)
        .await?;
    sqlx::query("INSERT INTO channels (id, name, position, server_id) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(name)
        .bind(position)
        .bind(server_id)
        .execute(db)
        .await?;
    Ok(Channel {
        id: id.into(),
        name: name.into(),
        position: u32::try_from(position).unwrap_or(0),
        server_id: server_id.into(),
    })
}

pub async fn rename_channel(db: &SqlitePool, id: &str, name: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE channels SET name = ? WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Writes positions 0..n in the given order.
pub async fn reorder_channels(db: &SqlitePool, order: &[String]) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    for (position, id) in order.iter().enumerate() {
        sqlx::query("UPDATE channels SET position = ? WHERE id = ?")
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}

pub async fn delete_channel(db: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM channels WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(done.rows_affected() == 1)
}

// ── settings and events ────────────────────────────────────────────────────────────────

pub async fn setting(db: &SqlitePool, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await
}

pub async fn set_setting(db: &SqlitePool, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn log_event(
    db: &SqlitePool,
    kind: &str,
    user_id: Option<&str>,
    channel_id: Option<&str>,
    message: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO events (time, kind, user_id, channel_id, message) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(now())
    .bind(kind)
    .bind(user_id)
    .bind(channel_id)
    .bind(message)
    .execute(db)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct EventRow {
    time: String,
    kind: String,
    user_id: Option<String>,
    channel_id: Option<String>,
    message: String,
}

/// Newest first.
pub async fn events(db: &SqlitePool, limit: u32) -> sqlx::Result<Vec<EventEntry>> {
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT time, kind, user_id, channel_id, message FROM events ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| EventEntry {
            time: r.time,
            kind: r.kind,
            user_id: r.user_id,
            channel_id: r.channel_id,
            message: r.message,
        })
        .collect())
}
