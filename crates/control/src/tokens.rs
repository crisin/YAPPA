//! LiveKit access tokens. Two kinds (ADR-005):
//! - voice: one channel room, publish and subscribe audio, ≈ one gaming evening long, so a
//!   reconnect never needs this process;
//! - presence: the room `presence`, no media, data and own attributes allowed, a week long.

use crate::config::Config;
use api_types::{User, VoiceToken};
use chrono::{SecondsFormat, Utc};
use livekit_api::access_token::{AccessToken, AccessTokenError, VideoGrants};
use std::time::Duration;

pub const PRESENCE_ROOM: &str = "presence";
pub const VOICE_TTL: Duration = Duration::from_secs(12 * 3600);
pub const PRESENCE_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

fn issue(
    config: &Config,
    user: &User,
    grants: VideoGrants,
    ttl: Duration,
) -> Result<VoiceToken, AccessTokenError> {
    let token = AccessToken::with_api_key(&config.livekit_api_key, &config.livekit_api_secret)
        .with_identity(&user.id)
        .with_name(&user.display_name)
        .with_ttl(ttl)
        .with_grants(grants)
        .to_jwt()?;
    let expires = Utc::now() + chrono::Duration::from_std(ttl).unwrap_or_default();
    Ok(VoiceToken {
        ws_url: config.livekit_ws_url.clone(),
        token,
        expires_at: expires.to_rfc3339_opts(SecondsFormat::Secs, true),
    })
}

pub fn voice(config: &Config, user: &User, room: &str) -> Result<VoiceToken, AccessTokenError> {
    let grants = VideoGrants {
        room_join: true,
        room: room.into(),
        can_publish: Some(true),
        can_subscribe: Some(true),
        can_publish_data: Some(true),
        can_update_own_metadata: Some(false),
        ..VideoGrants::default()
    };
    issue(config, user, grants, VOICE_TTL)
}

pub fn presence(config: &Config, user: &User) -> Result<VoiceToken, AccessTokenError> {
    let grants = VideoGrants {
        room_join: true,
        room: PRESENCE_ROOM.into(),
        can_publish: Some(false),
        can_subscribe: Some(true),
        can_publish_data: Some(true),
        can_update_own_metadata: Some(true),
        ..VideoGrants::default()
    };
    issue(config, user, grants, PRESENCE_TTL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_types::Role;
    use livekit_api::access_token::TokenVerifier;

    fn krille() -> User {
        User {
            id: "abc123".into(),
            display_name: "Krille".into(),
            role: Role::Admin,
        }
    }

    #[test]
    fn voice_token_is_for_one_room_with_audio() {
        let config = Config::local("sqlite::memory:");
        let token = voice(&config, &krille(), "raid").unwrap();
        let claims = TokenVerifier::with_api_key("devkey", "secret")
            .verify(&token.token)
            .unwrap();
        assert_eq!(claims.sub, "abc123");
        assert_eq!(claims.name, "Krille");
        assert_eq!(claims.video.room, "raid");
        assert!(claims.video.room_join && claims.video.can_publish());
        assert!(!claims.video.can_update_own_metadata());
        assert_eq!(token.ws_url, "ws://localhost:7880");
    }

    #[test]
    fn presence_token_carries_no_media_but_own_attributes() {
        let config = Config::local("sqlite::memory:");
        let token = presence(&config, &krille()).unwrap();
        let claims = TokenVerifier::with_api_key("devkey", "secret")
            .verify(&token.token)
            .unwrap();
        assert_eq!(claims.video.room, PRESENCE_ROOM);
        assert!(!claims.video.can_publish());
        assert!(claims.video.can_publish_data());
        assert!(claims.video.can_update_own_metadata());
    }
}
