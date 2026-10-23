//! Configuration from the environment (`infra/livekit/.env` on the box). Secrets never come
//! from files in the repo (CLAUDE.md rule 6).

use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    /// `sqlite://…` URL.
    pub database_url: String,
    pub livekit_api_key: String,
    pub livekit_api_secret: String,
    /// What clients connect to (`wss://voice.example`).
    pub livekit_ws_url: String,
    /// Where this process reaches LiveKit's server API; on the box that is localhost.
    pub livekit_http_url: String,
    /// `<base>/<channel>/latest.json` is the updater manifest (ADR-006).
    pub update_base_url: String,
    /// Media server the channels live on; one for now (failover is phase 2).
    pub server_id: String,
    /// Where clients reach this API from outside; goes into invite links
    /// (`https://voice.example/api` behind Caddy).
    pub public_url: String,
}

impl Config {
    /// `YAPPA_*` and `LIVEKIT_*` variables. Keys are required; the rest has local defaults.
    pub fn from_env() -> Result<Self, String> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let required = |name: &str| var(name).ok_or_else(|| format!("{name} is not set"));
        let ws = var("LIVEKIT_WS_URL").unwrap_or_else(|| "ws://localhost:7880".into());
        let listen: SocketAddr = var("YAPPA_LISTEN")
            .unwrap_or_else(|| "127.0.0.1:8090".into())
            .parse()
            .map_err(|e| format!("YAPPA_LISTEN: {e}"))?;
        // Behind Caddy on the box the API is https://<domain>/api; locally it is this port.
        let public_url = var("YAPPA_PUBLIC_URL").unwrap_or_else(|| {
            if ws.starts_with("wss://") {
                format!("{}/api", http_from_ws(&ws))
            } else {
                format!("http://{listen}")
            }
        });
        Ok(Self {
            listen,
            database_url: var("YAPPA_DATABASE_URL").unwrap_or_else(|| "sqlite://yappa.db".into()),
            livekit_api_key: required("LIVEKIT_API_KEY")?,
            livekit_api_secret: required("LIVEKIT_API_SECRET")?,
            livekit_http_url: var("LIVEKIT_HTTP_URL").unwrap_or_else(|| http_from_ws(&ws)),
            livekit_ws_url: ws,
            update_base_url: var("YAPPA_UPDATE_BASE_URL")
                .unwrap_or_else(|| "http://localhost:8090/updates".into()),
            server_id: var("YAPPA_SERVER_ID").unwrap_or_else(|| "main".into()),
            public_url: public_url.trim_end_matches('/').to_string(),
        })
    }

    /// For tests and local tools: LiveKit's public dev keys, nothing listens.
    pub fn local(database_url: &str) -> Self {
        Self {
            listen: "127.0.0.1:0".parse().expect("valid address"),
            database_url: database_url.into(),
            livekit_api_key: "devkey".into(),
            livekit_api_secret: "secret".into(),
            livekit_ws_url: "ws://localhost:7880".into(),
            livekit_http_url: "http://localhost:7880".into(),
            update_base_url: "http://localhost:8090/updates".into(),
            server_id: "main".into(),
            public_url: "http://127.0.0.1:8090".into(),
        }
    }
}

/// `wss://x` → `https://x`, `ws://x` → `http://x`.
fn http_from_ws(ws: &str) -> String {
    if let Some(rest) = ws.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = ws.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        ws.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::http_from_ws;

    #[test]
    fn server_api_url_follows_the_websocket_url() {
        assert_eq!(http_from_ws("wss://voice.example"), "https://voice.example");
        assert_eq!(http_from_ws("ws://localhost:7880"), "http://localhost:7880");
    }
}
