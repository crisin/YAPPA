//! `Authorization: Bearer <session token>` → the user. Every authenticated request also
//! carries `x-yappa-version: <app version>` (ADR-006): it is stored, so the minimum version
//! applies to token requests and the admin sees who runs what.

use crate::error::{Failure, Result};
use crate::{ids, store, AppState};
use api_types::{Role, User};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::sync::Arc;

pub const VERSION_HEADER: &str = "x-yappa-version";

/// Any logged-in member.
pub struct Member(pub User);

/// A logged-in admin.
pub struct Admin(pub User);

fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

impl FromRequestParts<Arc<AppState>> for Member {
    type Rejection = Failure;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self> {
        let token = bearer(parts).ok_or_else(Failure::unauthorized)?;
        let user = store::user_by_session(&state.db, &ids::token_hash(token))
            .await?
            .ok_or_else(Failure::unauthorized)?;
        let version = parts
            .headers
            .get(VERSION_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty() && v.len() <= 32);
        store::touch(&state.db, &user.id, version).await?;
        Ok(Self(user))
    }
}

impl FromRequestParts<Arc<AppState>> for Admin {
    type Rejection = Failure;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self> {
        let Member(user) = Member::from_request_parts(parts, state).await?;
        if user.role == Role::Admin {
            Ok(Self(user))
        } else {
            Err(Failure::forbidden())
        }
    }
}
