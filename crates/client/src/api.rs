//! HTTP client for the control plane (api-types). Every failure is an `ApiFailure` with the
//! server's stable code — or `unreachable` when there was no answer at all, which the
//! callers turn into "use the cache" or the offline banner. The `unreachable` message says
//! *why* as far as the client can tell (name, connection, IPv6-only server, TLS, timeout):
//! on the first evening (2026-10-24) a plain "Server nicht erreichbar" hid that one person
//! simply had no IPv6 for an IPv6-only home server.

use api_types::{
    ApiError, Channel, CreateChannelRequest, CreateInviteRequest, EventEntry, InviteCode, Me,
    Member, PublicConfig, RedeemInviteRequest, Role, Session, UpdateChannelRequest,
    UpdateMemberRequest, VersionPolicy, VoiceToken,
};
use reqwest::{Method, RequestBuilder, Url};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::net::IpAddr;
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

    /// No answer from `base`. Looks the host up once more to tell "no such name" from
    /// "IPv6-only server, no way there" from "server down", and logs the whole chain.
    async fn unreachable(base: &str, error: &reqwest::Error) -> Self {
        let (host, port) = host_port(base);
        let kind = NoAnswer::of(error);
        let addresses = match kind {
            NoAnswer::Connect | NoAnswer::Timeout | NoAnswer::Other => lookup(&host, port).await,
            NoAnswer::Tls => Some(Vec::new()),
        };
        let chain = error_chain(error);
        tracing::warn!(
            host,
            port,
            ?kind,
            addresses = ?addresses,
            error = %chain,
            "control plane unreachable"
        );
        Self::new(
            "unreachable",
            &describe(&host, kind, addresses.as_deref(), &chain),
        )
    }
}

/// What kind of "no answer" reqwest reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoAnswer {
    /// The connection never came up (refused, unreachable, no route, name lookup failed).
    Connect,
    Timeout,
    /// Connected, but the TLS handshake failed (certificate, protocol).
    Tls,
    Other,
}

impl NoAnswer {
    fn of(error: &reqwest::Error) -> Self {
        let text = error_chain(error).to_lowercase();
        if error.is_timeout() {
            Self::Timeout
        } else if text.contains("certificate") || text.contains("tls") || text.contains("handshake")
        {
            Self::Tls
        } else if error.is_connect() {
            Self::Connect
        } else {
            Self::Other
        }
    }
}

/// The message for the user. `addresses`: `None` = the name does not resolve, `Some([])` =
/// unknown or not asked, otherwise what DNS returned.
fn describe(host: &str, kind: NoAnswer, addresses: Option<&[IpAddr]>, chain: &str) -> String {
    let only_v6 = addresses.is_some_and(|a| !a.is_empty() && a.iter().all(IpAddr::is_ipv6));
    match (kind, addresses) {
        (NoAnswer::Connect | NoAnswer::Timeout | NoAnswer::Other, None) => format!(
            "Server nicht erreichbar: der Name „{host}“ ist nicht auflösbar. Stimmt der Link, \
             und bist du online?"
        ),
        (NoAnswer::Connect | NoAnswer::Timeout, _) if only_v6 => format!(
            "Server nicht erreichbar: „{host}“ ist nur über IPv6 erreichbar, und von hier kam \
             keine Verbindung zustande. Entweder hat dein Internetanschluss kein IPv6 \
             (test-ipv6.com zeigt es), oder der Server ist aus."
        ),
        (NoAnswer::Timeout, _) => format!(
            "Server nicht erreichbar: „{host}“ antwortet nicht (Zeitüberschreitung). Server aus, \
             oder eine Firewall blockt Port 443?"
        ),
        (NoAnswer::Connect, _) => format!(
            "Server nicht erreichbar: keine Verbindung zu „{host}“. Server aus, oder eine \
             Firewall blockt Port 443?"
        ),
        (NoAnswer::Tls, _) => format!(
            "Server nicht erreichbar: TLS-Fehler bei „{host}“ – Zertifikat ungültig oder \
             abgelaufen, oder etwas dazwischen bricht die Verbindung auf."
        ),
        (NoAnswer::Other, _) => format!("Server nicht erreichbar: {host} – {chain}"),
    }
}

/// Host and port of the base URL; the message names the host, the lookup needs the port.
fn host_port(base: &str) -> (String, u16) {
    match Url::parse(base) {
        Ok(url) => (
            url.host_str().unwrap_or(base).to_string(),
            url.port_or_known_default().unwrap_or(443),
        ),
        Err(_) => (base.to_string(), 443),
    }
}

/// `None` when the name does not resolve at all.
async fn lookup(host: &str, port: u16) -> Option<Vec<IpAddr>> {
    tokio::net::lookup_host((host, port))
        .await
        .ok()
        .map(|addrs| addrs.map(|a| a.ip()).collect())
}

/// "outer: inner: innermost" — reqwest's Display alone hides the io error underneath.
fn error_chain(error: &reqwest::Error) -> String {
    // Display includes the URL (with possible query); the chain is for the log, so strip it.
    let outer = error.to_string();
    let outer = outer
        .split(" for url (")
        .next()
        .unwrap_or(&outer)
        .to_string();
    let mut parts = vec![outer];
    let mut source = std::error::Error::source(error);
    while let Some(inner) = source {
        let text = inner.to_string();
        if parts.last() != Some(&text) {
            parts.push(text);
        }
        source = inner.source();
    }
    parts.join(": ")
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
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => return Err(ApiFailure::unreachable(&self.base, &e).await),
        };
        let status = response.status();
        if status.is_success() {
            // 204 answers deserialize from `null`.
            let bytes = match response.bytes().await {
                Ok(bytes) => bytes,
                Err(e) => return Err(ApiFailure::unreachable(&self.base, &e).await),
            };
            let body: &[u8] = if bytes.is_empty() { b"null" } else { &bytes };
            serde_json::from_slice(body)
                .map_err(|e| ApiFailure::new("bad_response", &format!("Antwort unlesbar: {e}")))
        } else if matches!(status.as_u16(), 502..=504) {
            // Caddy or a tunnel answering for a control plane that is down: for the client
            // that is no server at all — the offline cache applies (review 2026-10-23).
            tracing::warn!(%status, "control plane unreachable behind the proxy");
            let (host, _) = host_port(&self.base);
            Err(ApiFailure::new(
                "unreachable",
                &format!(
                    "Server nicht erreichbar: „{host}“ antwortet {status} – der Server dahinter \
                     läuft gerade nicht."
                ),
            ))
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

#[cfg(test)]
mod tests {
    use super::*;

    const HOST: &str = "krillehome.example.myfritz.net";
    fn v6() -> Vec<IpAddr> {
        vec!["2a02:db8::1".parse().unwrap()]
    }
    fn dual() -> Vec<IpAddr> {
        vec![
            "2a02:db8::1".parse().unwrap(),
            "203.0.113.5".parse().unwrap(),
        ]
    }

    #[test]
    fn host_and_port_come_from_the_base_url() {
        assert_eq!(
            host_port("https://krillehome.example/api"),
            ("krillehome.example".into(), 443)
        );
        assert_eq!(
            host_port("http://127.0.0.1:8090"),
            ("127.0.0.1".into(), 8090)
        );
        assert_eq!(host_port("not a url"), ("not a url".into(), 443));
    }

    #[test]
    fn an_unresolvable_name_says_so() {
        let text = describe(HOST, NoAnswer::Connect, None, "dns error");
        assert!(text.contains("nicht auflösbar"), "{text}");
        assert!(text.contains(HOST));
    }

    #[test]
    fn an_ipv6_only_server_gets_the_ipv6_hint() {
        for kind in [NoAnswer::Connect, NoAnswer::Timeout] {
            let text = describe(HOST, kind, Some(&v6()), "");
            assert!(text.contains("nur über IPv6"), "{kind:?}: {text}");
            assert!(text.contains("test-ipv6.com"), "{kind:?}: {text}");
        }
    }

    #[test]
    fn a_dual_stack_server_does_not_blame_ipv6() {
        let text = describe(HOST, NoAnswer::Connect, Some(&dual()), "");
        assert!(!text.contains("IPv6"), "{text}");
        assert!(text.contains("keine Verbindung"), "{text}");
        let text = describe(HOST, NoAnswer::Timeout, Some(&dual()), "");
        assert!(text.contains("Zeitüberschreitung"), "{text}");
    }

    #[test]
    fn tls_and_unknown_errors_are_named() {
        let text = describe(HOST, NoAnswer::Tls, Some(&[]), "");
        assert!(text.contains("TLS"), "{text}");
        let text = describe(
            HOST,
            NoAnswer::Other,
            Some(&[]),
            "error sending request: boom",
        );
        assert!(text.contains("boom"), "{text}");
    }

    #[tokio::test]
    async fn a_refused_connection_is_reported_as_such() {
        // Port 1 on localhost: nothing listens, the OS refuses at once — no network needed.
        let api = ApiClient::new("http://127.0.0.1:1", None, "test");
        let failure = api.config().await.unwrap_err();
        assert_eq!(failure.code, "unreachable");
        assert!(
            failure.message.contains("keine Verbindung zu „127.0.0.1“"),
            "{failure}"
        );
    }

    #[tokio::test]
    #[ignore = "needs a resolver (NXDOMAIN for .invalid)"]
    async fn an_unknown_name_is_reported_as_such() {
        let api = ApiClient::new("https://server.invalid/api", None, "test");
        let failure = api.config().await.unwrap_err();
        assert!(failure.message.contains("nicht auflösbar"), "{failure}");
    }

    #[test]
    fn every_message_starts_with_the_known_phrase() {
        // The UI and the guides look for this prefix; the code stays `unreachable`.
        for (kind, addrs) in [
            (NoAnswer::Connect, None),
            (NoAnswer::Connect, Some(v6())),
            (NoAnswer::Timeout, Some(dual())),
            (NoAnswer::Tls, Some(vec![])),
            (NoAnswer::Other, Some(vec![])),
        ] {
            let text = describe(HOST, kind, addrs.as_deref(), "x");
            assert!(text.starts_with("Server nicht erreichbar"), "{text}");
        }
    }
}
