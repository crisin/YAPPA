//! The API end to end, in memory: router + SQLite, no network, no LiveKit.

use api_types::{
    ApiError, Channel, ClientInfo, EventEntry, InviteCode, Me, Member, PublicConfig, Role, Session,
    VoiceToken,
};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use control::{open_db, router, store, AppState, Config};
use http_body_util::BodyExt;
use livekit_api::access_token::{AccessToken, TokenVerifier};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

struct Api {
    app: axum::Router,
    state: std::sync::Arc<AppState>,
}

struct Reply {
    status: StatusCode,
    body: Value,
}

impl Reply {
    fn json<T: DeserializeOwned>(&self) -> T {
        serde_json::from_value(self.body.clone()).expect("response shape")
    }

    fn code(&self) -> String {
        self.json::<ApiError>().code
    }
}

impl Api {
    async fn new() -> Self {
        let config = Config::local("sqlite::memory:");
        let db = open_db(&config.database_url).await.unwrap();
        let state = AppState::new(db, config);
        Self {
            app: router(state.clone()),
            state,
        }
    }

    async fn call(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            request = request
                .header("authorization", format!("Bearer {token}"))
                .header("x-yappa-version", "0.1.0");
        }
        let request = match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()))
        };
        Reply { status, body }
    }

    async fn invite(&self, role: Role) -> String {
        store::create_invite(
            &self.state.db,
            &format!("code-{}", rand_suffix()),
            role,
            "test",
            7,
        )
        .await
        .unwrap()
        .code
    }

    /// Redeems a fresh invite; returns the session.
    async fn join(&self, name: &str, role: Role) -> Session {
        let code = self.invite(role).await;
        let reply = self
            .call(
                "POST",
                "/auth/invite",
                None,
                Some(redeem(&code, name, "0.1.0")),
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
        reply.json()
    }
}

fn rand_suffix() -> String {
    let mut b = [0u8; 4];
    getrandom::fill(&mut b).unwrap();
    hex::encode(b)
}

fn redeem(code: &str, name: &str, version: &str) -> Value {
    json!({
        "code": code,
        "displayName": name,
        "client": ClientInfo { app_version: version.into(), protocol_version: 1, platform: "windows".into() },
    })
}

#[tokio::test]
async fn config_is_public_with_defaults() {
    let api = Api::new().await;
    let reply = api.call("GET", "/config", None, None).await;
    assert_eq!(reply.status, StatusCode::OK);
    let config: PublicConfig = reply.json();
    assert_eq!(config.min_version, "0.0.0");
    assert!(config.killed_versions.is_empty());
    assert_eq!(config.livekit_ws_url, "ws://localhost:7880");
}

#[tokio::test]
async fn an_invite_logs_in_once_and_gives_channels_and_tokens() {
    let api = Api::new().await;
    let code = api.invite(Role::Member).await;
    let reply = api
        .call(
            "POST",
            "/auth/invite",
            None,
            Some(redeem(&code, "  Ben   der  Große ", "0.1.0")),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    let session: Session = reply.json();
    assert_eq!(session.user.display_name, "Ben der Große");
    assert_eq!(session.user.role, Role::Member);
    let presence = TokenVerifier::with_api_key("devkey", "secret")
        .verify(&session.presence.token)
        .unwrap();
    assert_eq!(presence.video.room, "presence");
    assert_eq!(presence.sub, session.user.id);

    let again = api
        .call(
            "POST",
            "/auth/invite",
            None,
            Some(redeem(&code, "Ben", "0.1.0")),
        )
        .await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
    assert_eq!(again.code(), "invite_invalid");

    let token = &session.session_token;
    let me: Me = api.call("GET", "/me", Some(token), None).await.json();
    assert_eq!(me.user, session.user);
    let channels: Vec<Channel> = api.call("GET", "/channels", Some(token), None).await.json();
    assert_eq!(
        channels.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["lobby"]
    );
    let voice: VoiceToken = api
        .call("POST", "/channels/lobby/token", Some(token), None)
        .await
        .json();
    let claims = TokenVerifier::with_api_key("devkey", "secret")
        .verify(&voice.token)
        .unwrap();
    assert_eq!(claims.video.room, "lobby");
    assert!(claims.video.can_publish());
    let unknown = api
        .call("POST", "/channels/nope/token", Some(token), None)
        .await;
    assert_eq!(unknown.code(), "channel_unknown");
}

#[tokio::test]
async fn names_and_codes_are_checked() {
    let api = Api::new().await;
    let code = api.invite(Role::Member).await;
    let short = api
        .call(
            "POST",
            "/auth/invite",
            None,
            Some(redeem(&code, "B", "0.1.0")),
        )
        .await;
    assert_eq!(short.code(), "name_invalid");
    let wrong = api
        .call(
            "POST",
            "/auth/invite",
            None,
            Some(redeem("xxxx", "Ben", "0.1.0")),
        )
        .await;
    assert_eq!(wrong.code(), "invite_invalid");
    // The failed attempt with a short name did not use the code up.
    let ok = api
        .call(
            "POST",
            "/auth/invite",
            None,
            Some(redeem(&code, "Ben", "0.1.0")),
        )
        .await;
    assert_eq!(ok.status, StatusCode::OK);
}

#[tokio::test]
async fn sessions_are_required_and_admin_routes_need_the_role() {
    let api = Api::new().await;
    assert_eq!(
        api.call("GET", "/me", None, None).await.code(),
        "no_session"
    );
    assert_eq!(
        api.call("GET", "/me", Some("forged"), None).await.code(),
        "no_session"
    );
    let ben = api.join("Ben", Role::Member).await;
    let members = api
        .call("GET", "/members", Some(&ben.session_token), None)
        .await;
    assert_eq!(members.status, StatusCode::FORBIDDEN);
    let logout = api
        .call("POST", "/auth/logout", Some(&ben.session_token), None)
        .await;
    assert_eq!(logout.status, StatusCode::NO_CONTENT);
    assert_eq!(
        api.call("GET", "/me", Some(&ben.session_token), None)
            .await
            .code(),
        "no_session"
    );
}

#[tokio::test]
async fn the_version_policy_keeps_old_clients_out() {
    let api = Api::new().await;
    let admin = api.join("Krille", Role::Admin).await;
    let ben = api.join("Ben", Role::Member).await;
    let set = api
        .call(
            "PUT",
            "/admin/config",
            Some(&admin.session_token),
            Some(json!({ "minVersion": "0.2.0", "killedVersions": ["0.3.1"] })),
        )
        .await;
    assert_eq!(set.status, StatusCode::OK);
    assert_eq!(set.json::<PublicConfig>().min_version, "0.2.0");

    // Ben's client said 0.1.0 with every request: no more voice tokens.
    let token = api
        .call(
            "POST",
            "/channels/lobby/token",
            Some(&ben.session_token),
            None,
        )
        .await;
    assert_eq!(token.status, StatusCode::UPGRADE_REQUIRED);
    assert_eq!(token.code(), "version_too_old");

    let code = api.invite(Role::Member).await;
    let killed = api
        .call(
            "POST",
            "/auth/invite",
            None,
            Some(redeem(&code, "Mo", "0.3.1")),
        )
        .await;
    assert_eq!(killed.code(), "version_too_old");
    let fine = api
        .call(
            "POST",
            "/auth/invite",
            None,
            Some(redeem(&code, "Mo", "0.3.2")),
        )
        .await;
    assert_eq!(fine.status, StatusCode::OK);

    let bad = api
        .call(
            "PUT",
            "/admin/config",
            Some(&admin.session_token),
            Some(json!({ "minVersion": "neu", "killedVersions": [] })),
        )
        .await;
    assert_eq!(bad.code(), "version_invalid");
}

#[tokio::test]
async fn admins_manage_channels_with_dense_positions() {
    let api = Api::new().await;
    let admin = api.join("Krille", Role::Admin).await;
    let t = Some(admin.session_token.as_str());
    let raid = api
        .call(
            "POST",
            "/channels",
            t,
            Some(json!({ "name": "Raid-Abend" })),
        )
        .await;
    assert_eq!(raid.status, StatusCode::CREATED);
    assert_eq!(raid.json::<Channel>().id, "raid-abend");
    api.call(
        "POST",
        "/channels",
        t,
        Some(json!({ "name": "Musik Session" })),
    )
    .await;
    let twin = api
        .call(
            "POST",
            "/channels",
            t,
            Some(json!({ "name": "Raid Abend" })),
        )
        .await;
    assert_ne!(twin.json::<Channel>().id, "raid-abend", "ids stay unique");

    let moved: Vec<Channel> = api
        .call(
            "PATCH",
            "/channels/musik-session",
            t,
            Some(json!({ "position": 0, "name": "Musik" })),
        )
        .await
        .json();
    let order: Vec<(&str, u32)> = moved
        .iter()
        .map(|c| (c.name.as_str(), c.position))
        .collect();
    assert_eq!(order[0], ("Musik", 0));
    assert_eq!(order[1], ("Lobby", 1));

    let deleted = api.call("DELETE", "/channels/lobby", t, None).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let left: Vec<Channel> = api.call("GET", "/channels", t, None).await.json();
    assert_eq!(
        left.iter().map(|c| c.position).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    for c in &left[1..] {
        api.call("DELETE", &format!("/channels/{}", c.id), t, None)
            .await;
    }
    let last = api
        .call("DELETE", &format!("/channels/{}", left[0].id), t, None)
        .await;
    assert_eq!(last.code(), "last_channel");
}

#[tokio::test]
async fn roles_change_but_the_last_admin_stays() {
    let api = Api::new().await;
    let admin = api.join("Krille", Role::Admin).await;
    let ben = api.join("Ben", Role::Member).await;
    let t = Some(admin.session_token.as_str());
    let demote_self = api
        .call(
            "PATCH",
            &format!("/members/{}", admin.user.id),
            t,
            Some(json!({ "role": "member" })),
        )
        .await;
    assert_eq!(demote_self.code(), "last_admin");
    let promote = api
        .call(
            "PATCH",
            &format!("/members/{}", ben.user.id),
            t,
            Some(json!({ "role": "admin" })),
        )
        .await;
    assert_eq!(promote.status, StatusCode::NO_CONTENT);
    let members: Vec<Member> = api.call("GET", "/members", t, None).await.json();
    let ben_now = members.iter().find(|m| m.user.id == ben.user.id).unwrap();
    assert_eq!(ben_now.user.role, Role::Admin);
    assert_eq!(ben_now.client.as_ref().unwrap().app_version, "0.1.0");
    let events: Vec<EventEntry> = api.call("GET", "/events", t, None).await.json();
    assert_eq!(events[0].kind, "role_changed");
    let kick_self = api
        .call("POST", &format!("/members/{}/kick", admin.user.id), t, None)
        .await;
    assert_eq!(kick_self.code(), "kick_self");
}

#[tokio::test]
async fn invites_are_listed_created_and_revoked() {
    let api = Api::new().await;
    let admin = api.join("Krille", Role::Admin).await;
    let t = Some(admin.session_token.as_str());
    let created = api
        .call("POST", "/invites", t, Some(json!({ "days": 1000 })))
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let invite: InviteCode = created.json();
    assert_eq!(invite.code.len(), 14);
    let all: Vec<InviteCode> = api.call("GET", "/invites", t, None).await.json();
    assert!(all.iter().any(|i| i.code == invite.code));
    assert_eq!(
        api.call("DELETE", &format!("/invites/{}", invite.code), t, None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let gone = api
        .call(
            "POST",
            "/auth/invite",
            None,
            Some(redeem(&invite.code, "Mo", "0.1.0")),
        )
        .await;
    assert_eq!(gone.code(), "invite_invalid");
}

fn signed(body: &str, secret: &str) -> String {
    let hash = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(body.as_bytes()));
    AccessToken::with_api_key("devkey", secret)
        .with_sha256(&hash)
        .to_jwt()
        .unwrap()
}

async fn post_webhook(api: &Api, body: &str, auth: &str) -> StatusCode {
    let request = Request::builder()
        .method("POST")
        .uri("/livekit/webhook")
        .header("authorization", auth)
        .header("content-type", "application/webhook+json")
        .body(Body::from(body.to_string()))
        .unwrap();
    api.app.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn signed_webhooks_become_event_lines_and_forged_ones_are_refused() {
    let api = Api::new().await;
    let admin = api.join("Krille", Role::Admin).await;
    let joined = json!({
        "event": "participant_joined",
        "id": "EV_1",
        "createdAt": 1,
        "room": { "name": "lobby", "sid": "RM_1" },
        "participant": { "identity": "abc", "name": "Ben", "sid": "PA_1" }
    })
    .to_string();
    assert_eq!(
        post_webhook(&api, &joined, &signed(&joined, "secret")).await,
        StatusCode::OK
    );
    let online = joined.replace("lobby", "presence");
    assert_eq!(
        post_webhook(&api, &online, &signed(&online, "secret")).await,
        StatusCode::OK
    );
    assert_eq!(
        post_webhook(&api, &joined, &signed(&joined, "wrong-secret")).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post_webhook(&api, &joined, &signed("other body", "secret")).await,
        StatusCode::UNAUTHORIZED
    );
    let events: Vec<EventEntry> = api
        .call("GET", "/events?limit=2", Some(&admin.session_token), None)
        .await
        .json();
    assert_eq!(events[0].message, "Ben ist online");
    assert_eq!(events[1].message, "Ben betritt lobby");
    assert_eq!(events[1].channel_id.as_deref(), Some("lobby"));
}
