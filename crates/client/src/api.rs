//! HTTP client for the control plane (api-types). Every failure is an `ApiFailure` with the
//! server's stable code — or `unreachable` when there was no answer at all, which the
//! callers turn into "use the cache" or the offline banner.

use api_types::{
    ApiError, Channel, CreateChannelRequest, CreateInviteRequest, EventEntry, InviteCode, Me,
    Member, PublicConfig, RedeemInviteRequest, Role, Session, UpdateChannelRequest,
    UpdateMemberRequest, VersionPolicy, VoiceToken,
};
use reqwest::{Method, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;

/// Same header the control plane reads (ADR-006).
pub const VERSION_HEADER: &str = "x-yappa-version";
const TIMEOUT: Duration = Duration::from_secs(8);

/// `ApiError` plus the cases that never reached the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiFailure {
    pub code: String,
    pub message: String,
}

impl ApiFailure {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn no_session() -> Self {
        Self::new("no_session", "Nicht angemeldet.")
    }

    fn unreachable(error: &reqwest::Error) -> Self {
        tracing::info!(%error, "control plane unreachable");
        Self::new("unreachable", "Server nicht erreichbar.")
    }
}

impl std::fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for ApiFailure {}

#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
    version: String,
}

impl ApiClient {
    pub fn new(base: &str, token: Option<String>, version: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(format!("yappa/{version}"))
            .build()
            .unwrap_or_default();
        Self {
            http,
            base: base.trim_end_matches('/').into(),
            token,
            version: version.into(),
        }
    }

    pub fn server(&self) -> &str {
        &self.base
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header(VERSION_HEADER, &self.version);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        request
    }

    async fn send<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, ApiFailure> {
        let response = request
            .send()
            .await
            .map_err(|e| ApiFailure::unreachable(&e))?;
        let status = response.status();
        if status.is_success() {
            // 204 answers deserialize from `null`.
            let bytes = response
                .bytes()
                .await
                .map_err(|e| ApiFailure::unreachable(&e))?;
            let body: &[u8] = if bytes.is_empty() { b"null" } else { &bytes };
            serde_json::from_slice(body)
                .map_err(|e| ApiFailure::new("bad_response", &format!("Antwort unlesbar: {e}")))
        } else {
            let fallback = ApiFailure::new("http_error", &format!("Server antwortet {status}."));
            Err(response
                .json::<ApiError>()
                .await
                .map(|e| ApiFailure {
                    code: e.code,
                    message: e.message,
                })
                .unwrap_or(fallback))
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiFailure> {
        self.send(self.request(Method::GET, path)).await
    }

    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiFailure> {
        self.send(self.request(Method::POST, path).json(body)).await
    }

    pub async fn config(&self) -> Result<PublicConfig, ApiFailure> {
        self.get("/config").await
    }

    pub async fn redeem(&self, request: &RedeemInviteRequest) -> Result<Session, ApiFailure> {
        self.post("/auth/invite", request).await
    }

    pub async fn logout(&self) -> Result<(), ApiFailure> {
        self.send(self.request(Method::POST, "/auth/logout")).await
    }

    pub async fn me(&self) -> Result<Me, ApiFailure> {
        self.get("/me").await
    }

    pub async fn channels(&self) -> Result<Vec<Channel>, ApiFailure> {
        self.get("/channels").await
    }

    pub async fn voice_token(&self, channel_id: &str) -> Result<VoiceToken, ApiFailure> {
        self.send(self.request(Method::POST, &format!("/channels/{channel_id}/token")))
            .await
    }

    // ── admin (U6) ─────────────────────────────────────────────────────────────────────

    pub async fn members(&self) -> Result<Vec<Member>, ApiFailure> {
        self.get("/members").await
    }

    pub async fn set_role(&self, user_id: &str, role: Role) -> Result<(), ApiFailure> {
        let body = UpdateMemberRequest { role };
        self.send(
            self.request(Method::PATCH, &format!("/members/{user_id}"))
                .json(&body),
        )
        .await
    }

    pub async fn kick(&self, user_id: &str) -> Result<(), ApiFailure> {
        self.send(self.request(Method::POST, &format!("/members/{user_id}/kick")))
            .await
    }

    pub async fn invites(&self) -> Result<Vec<InviteCode>, ApiFailure> {
        self.get("/invites").await
    }

    pub async fn create_invite(
        &self,
        request: &CreateInviteRequest,
    ) -> Result<InviteCode, ApiFailure> {
        self.post("/invites", request).await
    }

    pub async fn revoke_invite(&self, code: &str) -> Result<(), ApiFailure> {
        self.send(self.request(Method::DELETE, &format!("/invites/{code}")))
            .await
    }

    pub async fn create_channel(&self, name: &str) -> Result<Channel, ApiFailure> {
        self.post("/channels", &CreateChannelRequest { name: name.into() })
            .await
    }

    pub async fn update_channel(
        &self,
        id: &str,
        request: &UpdateChannelRequest,
    ) -> Result<Vec<Channel>, ApiFailure> {
        self.send(
            self.request(Method::PATCH, &format!("/channels/{id}"))
                .json(request),
        )
        .await
    }

    pub async fn delete_channel(&self, id: &str) -> Result<(), ApiFailure> {
        self.send(self.request(Method::DELETE, &format!("/channels/{id}")))
            .await
    }

    pub async fn events(&self, limit: u32) -> Result<Vec<EventEntry>, ApiFailure> {
        self.get(&format!("/events?limit={limit}")).await
    }

    pub async fn set_policy(&self, policy: &VersionPolicy) -> Result<PublicConfig, ApiFailure> {
        self.send(self.request(Method::PUT, "/admin/config").json(policy))
            .await
    }
}
