//! Every failure answers with `api_types::ApiError`: a stable code and a German sentence the
//! client can show as it is.

use api_types::ApiError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(Debug)]
pub struct Failure {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, Failure>;

impl Failure {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "no_session",
            "Nicht angemeldet – bitte mit einer Einladung neu verbinden.",
        )
    }

    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", "Nur für Admins.")
    }

    pub fn not_found(code: &'static str, message: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub fn bad_request(code: &'static str, message: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn too_old(min: &str) -> Self {
        Self::new(
            StatusCode::UPGRADE_REQUIRED,
            "version_too_old",
            format!("Diese yAPPA-Version ist zu alt. Bitte auf mindestens {min} aktualisieren."),
        )
    }
}

impl From<sqlx::Error> for Failure {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(%error, "database");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Interner Fehler auf dem Server.",
        )
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let body = ApiError {
            code: self.code.into(),
            message: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}
