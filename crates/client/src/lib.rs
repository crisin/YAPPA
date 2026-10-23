//! Everything the desktop client does that is not audio (ADR-007): talk to the control
//! plane, keep the session in the OS credential store, sit in the presence room, keep the
//! app settings. No Tauri and no audio here — the shell wires this crate and the engine
//! together, and the headless example logs in the same way as the app.
//!
//! Layout on disk, per profile (`YAPPA_PROFILE`, so two clients run on one PC):
//!
//! ```text
//! <config dir>/[profiles/<name>/]
//!   settings.json        engine settings (engine-protocol, v2)
//!   app-settings.json    app settings (this crate, v1)
//!   session.json         server + user of the login (not secret)
//!   cache/channels.json, cache/config.json   last answers, for when the server is away
//! ```
//! The async calls write small JSON files and touch the credential store directly instead
//! of via `spawn_blocking`: a few kilobytes and one OS call, rare, far from any audio thread
//! (realtime review C1 accepted this).
//!
//! The session token itself lives in the credential store (Windows Credential Manager,
//! macOS Keychain), with a file next to `session.json` as the fallback.

pub mod api;
pub mod presence;
pub mod profile;
pub mod secrets;
pub mod settings;

pub use api::{ApiClient, ApiFailure};
pub use presence::{Presence, PresenceView};
pub use profile::Profile;
pub use settings::{AppSettings, SettingsView, UpdateChannel};

use api_types::{
    Channel, ClientInfo, InviteLink, PresenceState, PublicConfig, RedeemInviteRequest, User,
    VoiceToken,
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Who is logged in on this profile, where. The token is stored separately.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub server: String,
    pub user: User,
}

/// The client's state machine around the API. Cheap to share behind an `Arc`.
pub struct Client {
    profile: Profile,
    secrets: Box<dyn secrets::SecretStore>,
    info: ClientInfo,
    api: Mutex<Option<ApiClient>>,
    session: Mutex<Option<SessionRecord>>,
}

impl Client {
    pub fn new(profile: Profile, info: ClientInfo) -> Self {
        let secrets = secrets::open(&profile);
        let session = profile.read_json::<SessionRecord>(profile::SESSION_FILE);
        let api = session.as_ref().and_then(|record| {
            let token = secrets.get(&secret_key(&record.server, &record.user.id))?;
            Some(ApiClient::new(
                &record.server,
                Some(token),
                &info.app_version,
            ))
        });
        Self {
            profile,
            secrets,
            info,
            api: Mutex::new(api),
            session: Mutex::new(session),
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Logged in on this machine (a token is stored); says nothing about the server yet.
    pub fn session(&self) -> Option<SessionRecord> {
        let has_api = self.api.lock().expect("lock").is_some();
        self.session
            .lock()
            .expect("lock")
            .clone()
            .filter(|_| has_api)
    }

    fn api(&self) -> Result<ApiClient, ApiFailure> {
        self.api
            .lock()
            .expect("lock")
            .clone()
            .ok_or_else(ApiFailure::no_session)
    }

    /// First login: an invite link (or its text) plus the name the others will see.
    pub async fn redeem(
        &self,
        link: &str,
        display_name: &str,
    ) -> Result<api_types::Session, ApiFailure> {
        let link = InviteLink::parse(link)
            .map_err(|e| ApiFailure::new("invite_invalid", &format!("{e}")))?;
        let api = ApiClient::new(&link.server, None, &self.info.app_version);
        let session = api
            .redeem(&RedeemInviteRequest {
                code: link.code,
                display_name: display_name.into(),
                client: self.info.clone(),
            })
            .await?;
        let record = SessionRecord {
            server: link.server.clone(),
            user: session.user.clone(),
        };
        self.secrets.set(
            &secret_key(&record.server, &record.user.id),
            &session.session_token,
        );
        self.profile.write_json(profile::SESSION_FILE, &record);
        *self.api.lock().expect("lock") = Some(ApiClient::new(
            &link.server,
            Some(session.session_token.clone()),
            &self.info.app_version,
        ));
        *self.session.lock().expect("lock") = Some(record);
        Ok(session)
    }

    /// Ends the session here and on the server (if reachable).
    pub async fn logout(&self) {
        if let Ok(api) = self.api() {
            let _ = api.logout().await;
        }
        if let Some(record) = self.session.lock().expect("lock").take() {
            self.secrets
                .delete(&secret_key(&record.server, &record.user.id));
        }
        *self.api.lock().expect("lock") = None;
        self.profile.remove(profile::SESSION_FILE);
    }

    /// Who I am plus a fresh presence token. A refused session is forgotten.
    pub async fn me(&self) -> Result<api_types::Me, ApiFailure> {
        let me = self.api()?.me().await;
        if let Err(e) = &me {
            if e.code == "no_session" {
                self.logout().await;
            }
        }
        me
    }

    /// Public config; the cached one when the server cannot be reached.
    pub async fn config(&self) -> Result<PublicConfig, ApiFailure> {
        let server = self
            .session
            .lock()
            .expect("lock")
            .as_ref()
            .map(|s| s.server.clone())
            .ok_or_else(ApiFailure::no_session)?;
        let api = ApiClient::new(&server, None, &self.info.app_version);
        self.cached(profile::CONFIG_CACHE, api.config().await)
    }

    /// Channels; the cached list when the server cannot be reached (a running evening goes
    /// on without the control plane).
    pub async fn channels(&self) -> Result<Vec<Channel>, ApiFailure> {
        let fresh = self.api()?.channels().await;
        self.cached(profile::CHANNELS_CACHE, fresh)
    }

    pub async fn voice_token(&self, channel_id: &str) -> Result<VoiceToken, ApiFailure> {
        self.api()?.voice_token(channel_id).await
    }

    /// Admin calls go through unchanged; the shell exposes them to the admin module.
    pub fn admin(&self) -> Result<ApiClient, ApiFailure> {
        self.api()
    }

    fn cached<T: Serialize + serde::de::DeserializeOwned>(
        &self,
        file: &str,
        fresh: Result<T, ApiFailure>,
    ) -> Result<T, ApiFailure> {
        match fresh {
            Ok(value) => {
                self.profile.write_json(file, &value);
                Ok(value)
            }
            Err(e) if e.code == "unreachable" => {
                self.profile.read_json(file).ok_or(e).inspect(|_| {
                    tracing::info!(file, "server unreachable — using the cached answer");
                })
            }
            Err(e) => Err(e),
        }
    }

    /// `<update channel> <version>` for the presence attribute `app`.
    pub fn app_label(&self, settings: &AppSettings) -> String {
        format!(
            "{} {}",
            settings.update_channel.as_str(),
            self.info.app_version
        )
    }

    /// Joins the presence room with the token from `me()` / `redeem()`.
    pub fn start_presence(
        &self,
        token: &VoiceToken,
        initial: PresenceState,
        on_change: impl Fn(Vec<api_types::PresenceEntry>) + Send + Sync + 'static,
    ) -> std::io::Result<Presence> {
        Presence::start(token, initial, on_change)
    }
}

/// One credential per server and user, so profiles and servers never collide.
fn secret_key(server: &str, user_id: &str) -> String {
    format!("{user_id}@{server}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_keys_separate_servers_and_users() {
        assert_ne!(
            secret_key("https://a/api", "u1"),
            secret_key("https://b/api", "u1")
        );
        assert_eq!(secret_key("https://a/api", "u1"), "u1@https://a/api");
    }
}
