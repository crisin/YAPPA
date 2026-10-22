//! Client <-> control-plane contract (HTTP JSON).
//!
//! The control plane (`crates/control`, spike S6) and the desktop client both depend on this
//! crate; the client's TypeScript side is generated with `cargo xtask gen-types`. Changes are
//! additive only once a server exists, so an N-1 client keeps working (planning doc,
//! "Kompatibilität"). Presence is not an HTTP resource: it lives in a LiveKit room as
//! participant attributes (ADR-005), and [`PresenceState`] is the shape of those attributes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Bumped only on breaking changes; the server answers N-1 compatibly.
pub const API_VERSION: u32 = 1;

/// Every endpoint answers with this envelope.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApiError {
    /// Stable machine-readable code, e.g. `invite_invalid`, `version_too_old`.
    pub code: String,
    pub message: String,
}

/// First login: an invite link carries the code.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RedeemInviteRequest {
    pub code: String,
    pub display_name: String,
    pub client: ClientInfo,
}

/// Sent on login so the control plane can enforce the minimum version.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClientInfo {
    pub app_version: String,
    pub protocol_version: u32,
    pub platform: String,
}

/// Answer of `POST /auth/invite`.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Session {
    pub user: User,
    /// Bearer token for later requests; kept in the OS credential store by the client.
    pub session_token: String,
    /// Token for the presence room (ADR-005).
    pub presence: VoiceToken,
}

/// Answer of `GET /me`: who I am plus a fresh presence-room token for this start.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Me {
    pub user: User,
    pub presence: VoiceToken,
}

/// `GET /config`, readable without a session: what every client must know before it talks
/// to anything else (ADR-006).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicConfig {
    pub api_version: u32,
    /// Clients below this app version are refused a token and shown the update banner.
    pub min_version: String,
    /// Kill switch: exact app versions that must leave the call and update.
    pub killed_versions: Vec<String>,
    /// `<base>/<channel>/latest.json` is the updater manifest.
    pub update_base_url: String,
    pub livekit_ws_url: String,
}

/// A member as the admin sees them.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Member {
    pub user: User,
    /// What their client reported at the last login; `None` before the first login.
    pub client: Option<ClientInfo>,
    /// RFC 3339, last request seen by the control plane.
    pub last_seen: Option<String>,
}

/// An invite link's code, as the admin manages it (`yappa://invite/<code>`).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InviteCode {
    pub code: String,
    pub created_by: String,
    /// RFC 3339.
    pub created_at: String,
    pub expires_at: String,
    /// User id once redeemed; a code works once.
    pub used_by: Option<String>,
}

/// One line of the admin's event log (`GET /events`): joins, leaves, reconnects, kicks,
/// updates — from LiveKit webhooks and the control plane's own actions.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EventEntry {
    /// RFC 3339.
    pub time: String,
    /// Stable machine-readable kind, e.g. `joined`, `left`, `kicked`, `invite_redeemed`.
    pub kind: String,
    pub user_id: Option<String>,
    pub channel_id: Option<String>,
    pub message: String,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Role {
    Member,
    Admin,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct User {
    pub id: String,
    pub display_name: String,
    pub role: Role,
}

/// A voice channel. Its id is the LiveKit room name.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub position: u32,
    /// Media server the channel is currently assigned to (changes on failover).
    pub server_id: String,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VoiceTokenRequest {
    pub channel_id: String,
}

/// Long-lived (≈ one gaming evening) so reconnects survive a control-plane outage.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VoiceToken {
    pub ws_url: String,
    pub token: String,
    /// RFC 3339.
    pub expires_at: String,
}

/// What a person needs to join a voice room: where the media server is and a token for it.
///
/// Until the control plane hands out a [`VoiceToken`] per login (S6), an invite is one line
/// of text (`cargo xtask livekit token`) that is pasted into the client. The text form is
/// `yappa1.` followed by the base64url of the JSON — made to survive a chat window, not to
/// hide anything: whoever has the text can join.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Invite {
    /// `wss://…` (or `ws://localhost:7880` in development).
    pub url: String,
    pub token: String,
}

const INVITE_PREFIX: &str = "yappa1.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteError {
    /// Does not start with the prefix — some other text was pasted.
    NotAnInvite,
    /// Starts right but does not decode — usually copied incompletely.
    Damaged,
    /// Decodes, but the server address is not a `ws://` or `wss://` URL.
    BadUrl,
}

impl std::fmt::Display for InviteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotAnInvite => "not a yAPPA invite (it starts with \"yappa1.\")",
            Self::Damaged => "the invite is damaged or was copied incompletely",
            Self::BadUrl => "the invite has no valid server address",
        })
    }
}

impl Invite {
    pub fn encode(&self) -> String {
        use base64::Engine as _;
        let json = serde_json::to_vec(self).expect("an invite is two strings");
        INVITE_PREFIX.to_string() + &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
    }

    /// Accepts what [`encode`](Self::encode) produced, also with whitespace around it or
    /// line breaks inside it (chat programs wrap long lines).
    pub fn decode(text: &str) -> Result<Self, InviteError> {
        use base64::Engine as _;
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let payload = compact
            .strip_prefix(INVITE_PREFIX)
            .ok_or(InviteError::NotAnInvite)?;
        let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| InviteError::Damaged)?;
        let invite: Self = serde_json::from_slice(&json).map_err(|_| InviteError::Damaged)?;
        if !(invite.url.starts_with("wss://") || invite.url.starts_with("ws://")) {
            return Err(InviteError::BadUrl);
        }
        Ok(invite)
    }
}

/// How a person's microphone goes on air; shown on their tile.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TalkMode {
    #[default]
    Vad,
    Ptt,
    Open,
}

impl TalkMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vad => "vad",
            Self::Ptt => "ptt",
            Self::Open => "open",
        }
    }

    /// Unknown or missing values fall back to the default rather than failing: an older
    /// client must still be able to show a newer one.
    pub fn parse(text: Option<&str>) -> Self {
        match text {
            Some("ptt") => Self::Ptt,
            Some("open") => Self::Open,
            _ => Self::Vad,
        }
    }
}

/// A client's state in the presence room, carried as LiveKit participant attributes
/// (ADR-005). Identity = user id, participant name = display name; this is the rest.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresenceState {
    /// The voice channel they sit in, `None` = in the lobby view, nowhere.
    pub channel_id: Option<String>,
    /// `<update channel> <version>`, e.g. `nightly 0.4.2`.
    pub app: String,
    pub talk: TalkMode,
    pub away: bool,
}

impl PresenceState {
    pub const KEY_CHANNEL: &'static str = "channel";
    pub const KEY_APP: &'static str = "app";
    pub const KEY_TALK: &'static str = "talk";
    pub const KEY_AWAY: &'static str = "away";

    /// Attributes to send with `set_attributes`. Every key is always present: an empty
    /// value deletes the key on the server, which is how "no channel" and "not away" are
    /// expressed without a second round trip.
    pub fn to_attributes(&self) -> HashMap<String, String> {
        HashMap::from([
            (
                Self::KEY_CHANNEL.to_string(),
                self.channel_id.clone().unwrap_or_default(),
            ),
            (Self::KEY_APP.to_string(), self.app.clone()),
            (Self::KEY_TALK.to_string(), self.talk.as_str().to_string()),
            (
                Self::KEY_AWAY.to_string(),
                if self.away { "1" } else { "" }.to_string(),
            ),
        ])
    }

    /// Tolerant on purpose: missing keys are defaults, unknown values are ignored.
    pub fn from_attributes(attributes: &HashMap<String, String>) -> Self {
        let get = |key: &str| {
            attributes
                .get(key)
                .map(String::as_str)
                .filter(|v| !v.is_empty())
        };
        Self {
            channel_id: get(Self::KEY_CHANNEL).map(str::to_string),
            app: get(Self::KEY_APP).unwrap_or_default().to_string(),
            talk: TalkMode::parse(get(Self::KEY_TALK)),
            away: get(Self::KEY_AWAY).is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_on_the_wire() {
        let json = serde_json::to_value(VoiceToken {
            ws_url: "wss://voice.example".into(),
            token: "t".into(),
            expires_at: "2026-10-07T23:00:00Z".into(),
        })
        .unwrap();
        assert_eq!(json["wsUrl"], "wss://voice.example");
        assert_eq!(json["expiresAt"], "2026-10-07T23:00:00Z");
    }

    #[test]
    fn role_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Role::Admin).unwrap(), "\"admin\"");
    }

    #[test]
    fn presence_round_trips_through_attributes() {
        let state = PresenceState {
            channel_id: Some("raid".into()),
            app: "nightly 0.4.2".into(),
            talk: TalkMode::Ptt,
            away: true,
        };
        let attributes = state.to_attributes();
        assert_eq!(attributes["channel"], "raid");
        assert_eq!(attributes["talk"], "ptt");
        assert_eq!(attributes["away"], "1");
        assert_eq!(PresenceState::from_attributes(&attributes), state);
    }

    #[test]
    fn presence_clears_with_empty_values_and_tolerates_unknown_ones() {
        let cleared = PresenceState::default().to_attributes();
        // Empty = delete on the server; every key is sent so nothing stale survives.
        assert_eq!(cleared["channel"], "");
        assert_eq!(cleared["away"], "");
        assert_eq!(cleared.len(), 4);
        let odd = HashMap::from([
            ("channel".to_string(), String::new()),
            ("talk".to_string(), "telepathy".to_string()),
        ]);
        let state = PresenceState::from_attributes(&odd);
        assert_eq!(state.channel_id, None);
        assert_eq!(state.talk, TalkMode::Vad);
        assert!(!state.away);
    }

    #[test]
    fn invite_round_trips_and_survives_a_chat_window() {
        let invite = Invite {
            url: "wss://voice.example".into(),
            token: "eyJ0eXAi.payload.signature".into(),
        };
        let text = invite.encode();
        assert!(text.starts_with("yappa1."));
        assert_eq!(Invite::decode(&text), Ok(invite.clone()));
        let wrapped = format!("  {}\n{}  \n", &text[..20], &text[20..]);
        assert_eq!(Invite::decode(&wrapped), Ok(invite));
    }

    #[test]
    fn invite_errors_say_what_is_wrong() {
        assert_eq!(Invite::decode("hallo"), Err(InviteError::NotAnInvite));
        assert_eq!(Invite::decode("yappa1.%%%"), Err(InviteError::Damaged));
        let text = Invite {
            url: "wss://voice.example".into(),
            token: "t".into(),
        }
        .encode();
        assert_eq!(
            Invite::decode(&text[..text.len() - 3]),
            Err(InviteError::Damaged)
        );
        let http = Invite {
            url: "https://voice.example".into(),
            token: "t".into(),
        }
        .encode();
        assert_eq!(Invite::decode(&http), Err(InviteError::BadUrl));
    }
}
