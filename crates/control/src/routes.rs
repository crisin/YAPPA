//! The HTTP API (vision, section 4). Read endpoints for every member, admin endpoints behind
//! the role, the LiveKit webhook for the event log. No presence here — that is the presence
//! room (ADR-005).

use crate::auth::{Admin, Member};
use crate::error::{Failure, Result};
use crate::{ids, store, tokens, AppState};
use api_types::{
    Channel, CreateChannelRequest, CreateInviteRequest, EventEntry, InviteCode, Me,
    Member as MemberInfo, PublicConfig, RedeemInviteRequest, Role, Session, UpdateChannelRequest,
    UpdateMemberRequest, User, VersionPolicy, VoiceToken, API_VERSION,
};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use livekit_api::access_token::TokenVerifier;
use livekit_api::webhooks::WebhookReceiver;
use serde::Deserialize;
use std::sync::Arc;

type St = State<Arc<AppState>>;

const MIN_VERSION: &str = "min_version";
const KILLED_VERSIONS: &str = "killed_versions";
const NAME_LEN: std::ops::RangeInclusive<usize> = 2..=32;
const DEFAULT_INVITE_DAYS: u32 = 7;
const MAX_INVITE_DAYS: u32 = 90;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/config", get(config))
        .route("/auth/invite", post(redeem))
        .route("/auth/logout", post(logout))
        .route("/me", get(me))
        .route("/channels", get(channels).post(create_channel))
        .route(
            "/channels/{id}",
            axum::routing::patch(update_channel).delete(delete_channel),
        )
        .route("/channels/{id}/token", post(channel_token))
        .route("/invites", get(invites).post(create_invite))
        .route("/invites/{code}", axum::routing::delete(revoke_invite))
        .route("/members", get(members))
        .route("/members/{id}", axum::routing::patch(update_member))
        .route("/members/{id}/kick", post(kick))
        .route("/events", get(events))
        .route("/admin/config", put(set_policy))
        .route("/livekit/webhook", post(webhook))
}

// ── config ─────────────────────────────────────────────────────────────────────────────

pub(crate) async fn public_config(state: &AppState) -> Result<PublicConfig> {
    let min_version = store::setting(&state.db, MIN_VERSION)
        .await?
        .unwrap_or_else(|| "0.0.0".into());
    let killed_versions = store::setting(&state.db, KILLED_VERSIONS)
        .await?
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    Ok(PublicConfig {
        api_version: API_VERSION,
        min_version,
        killed_versions,
        update_base_url: state.config.update_base_url.clone(),
        livekit_ws_url: state.config.livekit_ws_url.clone(),
    })
}

async fn config(State(state): St) -> Result<Json<PublicConfig>> {
    Ok(Json(public_config(&state).await?))
}

/// Also used by the CLI (`yappa-control policy`).
pub(crate) async fn store_policy(state: &AppState, policy: &VersionPolicy) -> Result<()> {
    if api_types::compare_versions(&policy.min_version, "0.0.0").is_none() {
        return Err(Failure::bad_request(
            "version_invalid",
            "Die Mindestversion muss die Form 1.2.3 haben.",
        ));
    }
    store::set_setting(&state.db, MIN_VERSION, &policy.min_version).await?;
    let killed = serde_json::to_string(&policy.killed_versions).unwrap_or_else(|_| "[]".into());
    store::set_setting(&state.db, KILLED_VERSIONS, &killed).await?;
    Ok(())
}

async fn set_policy(
    State(state): St,
    Admin(admin): Admin,
    Json(policy): Json<VersionPolicy>,
) -> Result<Json<PublicConfig>> {
    store_policy(&state, &policy).await?;
    let message = format!(
        "{} setzt Mindestversion {} (gesperrt: {})",
        admin.display_name,
        policy.min_version,
        if policy.killed_versions.is_empty() {
            "keine".to_string()
        } else {
            policy.killed_versions.join(", ")
        }
    );
    store::log_event(&state.db, "policy", Some(&admin.id), None, &message).await?;
    Ok(Json(public_config(&state).await?))
}

async fn check_version(state: &AppState, version: Option<&str>) -> Result<()> {
    let config = public_config(state).await?;
    match version {
        Some(v) if config.blocks(v) => Err(Failure::too_old(&config.min_version)),
        _ => Ok(()),
    }
}

// ── login ──────────────────────────────────────────────────────────────────────────────

fn clean_name(name: &str) -> Result<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if NAME_LEN.contains(&name.chars().count()) {
        Ok(name)
    } else {
        Err(Failure::bad_request(
            "name_invalid",
            "Der Name braucht 2 bis 32 Zeichen.",
        ))
    }
}

async fn redeem(
    State(state): St,
    Json(request): Json<RedeemInviteRequest>,
) -> Result<Json<Session>> {
    check_version(&state, Some(&request.client.app_version)).await?;
    let display_name = clean_name(&request.display_name)?;
    let code = request.code.trim().to_lowercase();
    let invalid = || {
        Failure::not_found(
            "invite_invalid",
            "Diese Einladung ist ungültig, abgelaufen oder schon benutzt.",
        )
    };
    let role = store::open_invite(&state.db, &code)
        .await?
        .ok_or_else(invalid)?;
    let user = User {
        id: ids::user_id(),
        display_name,
        role,
    };
    let token = ids::session_token();
    store::create_user(&state.db, &user, &request.client, &ids::token_hash(&token)).await?;
    if !store::use_invite(&state.db, &code, &user.id).await? {
        // Someone redeemed the same code a moment earlier; undo ours.
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(&user.id)
            .execute(&state.db)
            .await?;
        return Err(invalid());
    }
    store::log_event(
        &state.db,
        "invite_redeemed",
        Some(&user.id),
        None,
        &format!(
            "{} ist dabei ({})",
            user.display_name, request.client.app_version
        ),
    )
    .await?;
    let presence = tokens::presence(&state.config, &user).map_err(token_failure)?;
    Ok(Json(Session {
        user,
        session_token: token,
        presence,
    }))
}

async fn logout(State(state): St, headers: HeaderMap, Member(_): Member) -> Result<StatusCode> {
    if let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        store::delete_session(&state.db, &ids::token_hash(token.trim())).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn me(State(state): St, Member(user): Member) -> Result<Json<Me>> {
    let presence = tokens::presence(&state.config, &user).map_err(token_failure)?;
    Ok(Json(Me { user, presence }))
}

fn token_failure(error: livekit_api::access_token::AccessTokenError) -> Failure {
    tracing::error!(%error, "signing a LiveKit token");
    Failure::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "token_failed",
        "Der Server konnte kein Zugangs-Token ausstellen.",
    )
}

// ── channels ───────────────────────────────────────────────────────────────────────────

async fn channels(State(state): St, Member(_): Member) -> Result<Json<Vec<Channel>>> {
    Ok(Json(store::channels(&state.db).await?))
}

async fn channel_token(
    State(state): St,
    Member(user): Member,
    Path(id): Path<String>,
) -> Result<Json<VoiceToken>> {
    let version = store::user_version(&state.db, &user.id).await?;
    check_version(&state, version.as_deref()).await?;
    let channel = store::channel(&state.db, &id)
        .await?
        .ok_or_else(|| Failure::not_found("channel_unknown", "Diesen Channel gibt es nicht."))?;
    let token = tokens::voice(&state.config, &user, &channel.id).map_err(token_failure)?;
    Ok(Json(token))
}

async fn create_channel(
    State(state): St,
    Admin(admin): Admin,
    Json(request): Json<CreateChannelRequest>,
) -> Result<(StatusCode, Json<Channel>)> {
    let name = clean_name(&request.name)?;
    let mut id = ids::channel_slug(&name);
    if id == tokens::PRESENCE_ROOM || store::channel(&state.db, &id).await?.is_some() {
        id = format!("{id}-{}", &ids::user_id()[..4]);
    }
    let channel = store::create_channel(&state.db, &id, &name, &state.config.server_id).await?;
    store::log_event(
        &state.db,
        "channel_created",
        Some(&admin.id),
        Some(&id),
        &format!("{} legt den Channel {name} an", admin.display_name),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(channel)))
}

async fn update_channel(
    State(state): St,
    Admin(_): Admin,
    Path(id): Path<String>,
    Json(request): Json<UpdateChannelRequest>,
) -> Result<Json<Vec<Channel>>> {
    if store::channel(&state.db, &id).await?.is_none() {
        return Err(Failure::not_found(
            "channel_unknown",
            "Diesen Channel gibt es nicht.",
        ));
    }
    if let Some(name) = &request.name {
        store::rename_channel(&state.db, &id, &clean_name(name)?).await?;
    }
    if let Some(position) = request.position {
        let mut order: Vec<String> = store::channels(&state.db)
            .await?
            .into_iter()
            .map(|c| c.id)
            .filter(|c| *c != id)
            .collect();
        let at = usize::try_from(position)
            .unwrap_or(usize::MAX)
            .min(order.len());
        order.insert(at, id.clone());
        store::reorder_channels(&state.db, &order).await?;
    }
    Ok(Json(store::channels(&state.db).await?))
}

async fn delete_channel(
    State(state): St,
    Admin(admin): Admin,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let channels = store::channels(&state.db).await?;
    if channels.len() <= 1 && channels.iter().any(|c| c.id == id) {
        return Err(Failure::new(
            StatusCode::CONFLICT,
            "last_channel",
            "Der letzte Channel bleibt.",
        ));
    }
    if !store::delete_channel(&state.db, &id).await? {
        return Err(Failure::not_found(
            "channel_unknown",
            "Diesen Channel gibt es nicht.",
        ));
    }
    let order: Vec<String> = store::channels(&state.db)
        .await?
        .into_iter()
        .map(|c| c.id)
        .collect();
    store::reorder_channels(&state.db, &order).await?;
    store::log_event(
        &state.db,
        "channel_deleted",
        Some(&admin.id),
        Some(&id),
        &format!("{} löscht den Channel {id}", admin.display_name),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── invites and members ────────────────────────────────────────────────────────────────

/// Also used by the CLI and the bootstrap at the first start.
pub(crate) async fn new_invite(
    state: &AppState,
    role: Role,
    created_by: &str,
    days: u32,
) -> Result<InviteCode> {
    let days = days.clamp(1, MAX_INVITE_DAYS);
    Ok(store::create_invite(&state.db, &ids::invite_code(), role, created_by, days).await?)
}

async fn invites(State(state): St, Admin(_): Admin) -> Result<Json<Vec<InviteCode>>> {
    Ok(Json(store::invites(&state.db).await?))
}

async fn create_invite(
    State(state): St,
    Admin(admin): Admin,
    Json(request): Json<CreateInviteRequest>,
) -> Result<(StatusCode, Json<InviteCode>)> {
    let invite = new_invite(
        &state,
        request.role.unwrap_or(Role::Member),
        &admin.id,
        request.days.unwrap_or(DEFAULT_INVITE_DAYS),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(invite)))
}

async fn revoke_invite(
    State(state): St,
    Admin(_): Admin,
    Path(code): Path<String>,
) -> Result<StatusCode> {
    if store::delete_invite(&state.db, &code).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(Failure::not_found(
            "invite_invalid",
            "Diese Einladung gibt es nicht.",
        ))
    }
}

async fn members(State(state): St, Admin(_): Admin) -> Result<Json<Vec<MemberInfo>>> {
    Ok(Json(store::members(&state.db).await?))
}

async fn update_member(
    State(state): St,
    Admin(admin): Admin,
    Path(id): Path<String>,
    Json(request): Json<UpdateMemberRequest>,
) -> Result<StatusCode> {
    let target = store::user(&state.db, &id)
        .await?
        .ok_or_else(|| Failure::not_found("user_unknown", "Unbekannte Person."))?;
    if target.role == Role::Admin
        && request.role != Role::Admin
        && store::count_admins(&state.db).await? <= 1
    {
        return Err(Failure::new(
            StatusCode::CONFLICT,
            "last_admin",
            "Es muss mindestens einen Admin geben.",
        ));
    }
    store::set_role(&state.db, &id, request.role).await?;
    let role = match request.role {
        Role::Admin => "Admin",
        Role::Member => "Mitglied",
    };
    store::log_event(
        &state.db,
        "role_changed",
        Some(&id),
        None,
        &format!(
            "{} macht {} zu: {role}",
            admin.display_name, target.display_name
        ),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Out of every room right now (TeamSpeak-style kick); the account stays. A room the person
/// is not in answers "not found" — that is fine and ignored.
async fn kick(State(state): St, Admin(admin): Admin, Path(id): Path<String>) -> Result<StatusCode> {
    if id == admin.id {
        return Err(Failure::bad_request(
            "kick_self",
            "Dich selbst kannst du nicht rauswerfen.",
        ));
    }
    let target = store::user(&state.db, &id)
        .await?
        .ok_or_else(|| Failure::not_found("user_unknown", "Unbekannte Person."))?;
    let mut rooms: Vec<String> = store::channels(&state.db)
        .await?
        .into_iter()
        .map(|c| c.id)
        .collect();
    rooms.push(tokens::PRESENCE_ROOM.into());
    let mut removed = 0;
    for room in &rooms {
        match state.rooms.remove_participant(room, &id).await {
            Ok(()) => removed += 1,
            Err(error) => tracing::debug!(%error, room, "remove_participant"),
        }
    }
    store::log_event(
        &state.db,
        "kicked",
        Some(&id),
        None,
        &format!(
            "{} wirft {} raus ({removed} Räume)",
            admin.display_name, target.display_name
        ),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct EventQuery {
    limit: Option<u32>,
}

async fn events(
    State(state): St,
    Admin(_): Admin,
    Query(query): Query<EventQuery>,
) -> Result<Json<Vec<EventEntry>>> {
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    Ok(Json(store::events(&state.db, limit).await?))
}

// ── LiveKit webhook ────────────────────────────────────────────────────────────────────

/// LiveKit signs every webhook with the API key (a JWT whose `sha256` claim is the body's
/// hash). Only joins and leaves are kept, as human-readable lines for the admin.
async fn webhook(State(state): St, headers: HeaderMap, body: String) -> Result<StatusCode> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v).trim())
        .unwrap_or_default();
    let receiver = WebhookReceiver::new(TokenVerifier::with_api_key(
        &state.config.livekit_api_key,
        &state.config.livekit_api_secret,
    ));
    let event = receiver.receive(&body, auth).map_err(|error| {
        tracing::warn!(%error, "webhook rejected");
        Failure::new(
            StatusCode::UNAUTHORIZED,
            "webhook_invalid",
            "Signatur passt nicht.",
        )
    })?;
    let room = event
        .room
        .as_ref()
        .map(|r| r.name.clone())
        .unwrap_or_default();
    let Some(participant) = event.participant.as_ref() else {
        return Ok(StatusCode::OK);
    };
    let presence = room == tokens::PRESENCE_ROOM;
    let (kind, verb) = match (event.event.as_str(), presence) {
        ("participant_joined", true) => ("online", "ist online"),
        ("participant_left", true) => ("offline", "ist offline"),
        ("participant_joined", false) => ("joined", "betritt"),
        ("participant_left", false) => ("left", "verlässt"),
        _ => return Ok(StatusCode::OK),
    };
    let name = if participant.name.is_empty() {
        &participant.identity
    } else {
        &participant.name
    };
    let message = if presence {
        format!("{name} {verb}")
    } else {
        format!("{name} {verb} {room}")
    };
    store::log_event(
        &state.db,
        kind,
        Some(&participant.identity),
        (!presence).then_some(room.as_str()),
        &message,
    )
    .await?;
    Ok(StatusCode::OK)
}
