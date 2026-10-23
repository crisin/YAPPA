//! Random identifiers and secrets. All from the OS (getrandom); nothing guessable.

use base64::Engine as _;
use sha2::{Digest, Sha256};

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the OS provides randomness");
    bytes
}

/// User id = LiveKit identity: 12 hex characters, short enough for logs.
pub fn user_id() -> String {
    hex::encode(random::<6>())
}

/// Bearer token for a session: 256 bits, URL-safe.
pub fn session_token() -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random::<32>())
}

/// What is stored instead of the token.
pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Invite codes are read aloud and typed: lowercase, no 0/o/1/l/i, three groups of four
/// (≈ 59 bits). `xxxx-xxxx-xxxx`.
pub fn invite_code() -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let bytes = random::<12>();
    let chars: Vec<char> = bytes
        .iter()
        // 256 % 31 leaves a bias of under 1 % per character — irrelevant for a code that
        // works once and expires in days.
        .map(|b| ALPHABET[usize::from(*b) % ALPHABET.len()] as char)
        .collect();
    chars
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// Channel id = LiveKit room name, derived from the name: `Raid-Abend` → `raid-abend`.
pub fn channel_slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().to_lowercase().chars() {
        let mapped = match c {
            'ä' => "ae".to_string(),
            'ö' => "oe".to_string(),
            'ü' => "ue".to_string(),
            'ß' => "ss".to_string(),
            c if c.is_ascii_alphanumeric() => c.to_string(),
            _ => "-".to_string(),
        };
        slug.push_str(&mapped);
    }
    let collapsed = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        format!("channel-{}", hex::encode(random::<3>()))
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_readable_and_distinct() {
        let code = invite_code();
        assert_eq!(code.len(), 14);
        assert!(code
            .chars()
            .all(|c| c == '-' || (c.is_ascii_alphanumeric() && !"01ilo".contains(c))));
        assert_ne!(invite_code(), code);
    }

    #[test]
    fn tokens_are_long_and_only_their_hash_is_kept() {
        let token = session_token();
        assert_eq!(token.len(), 43);
        assert_eq!(token_hash(&token).len(), 64);
        assert_ne!(token_hash(&token), token);
    }

    #[test]
    fn channel_names_become_room_names() {
        assert_eq!(channel_slug("Raid-Abend"), "raid-abend");
        assert_eq!(channel_slug("  Musik Session!! "), "musik-session");
        assert_eq!(channel_slug("Übungsplatz"), "uebungsplatz");
        assert!(channel_slug("🎮").starts_with("channel-"));
    }
}
