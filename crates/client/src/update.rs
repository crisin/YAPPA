//! Updates (ADR-006): where the manifest of a channel lives, and what the shell tells the
//! UI about a check. The download and the install are the Tauri updater's (shell); this
//! crate only knows the contract.

use crate::settings::UpdateChannel;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The manifest's name on the update host, below `<update_base_url>/<channel>/`.
pub const MANIFEST: &str = "latest.json";

/// What a check for updates ended in.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum UpdateStatus {
    /// The newest version of the channel is the one running.
    UpToDate,
    /// Downloaded and its signature checked; `install_update` installs it and restarts.
    Ready {
        version: String,
        /// The changelog section of that version, markdown; may be empty.
        notes: String,
    },
    /// No update host known: browser mode, not logged in, a server without one.
    Unavailable { reason: String },
    /// The check or the download failed; `reason` goes into the status line.
    Failed { reason: String },
}

/// `<base>/<channel>/latest.json`; `None` when the server names no update host.
pub fn update_endpoint(base: &str, channel: UpdateChannel) -> Option<String> {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    Some(format!("{base}/{}/{MANIFEST}", channel.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_is_base_channel_manifest() {
        assert_eq!(
            update_endpoint("https://voice.example/updates/", UpdateChannel::Beta).as_deref(),
            Some("https://voice.example/updates/beta/latest.json")
        );
        assert_eq!(update_endpoint("  ", UpdateChannel::Stable), None);
    }

    #[test]
    fn status_is_tagged_for_the_ui() {
        let json = serde_json::to_string(&UpdateStatus::Ready {
            version: "0.2.0".into(),
            notes: String::new(),
        })
        .unwrap();
        assert_eq!(json, r#"{"kind":"ready","version":"0.2.0","notes":""}"#);
        assert_eq!(
            serde_json::to_string(&UpdateStatus::UpToDate).unwrap(),
            r#"{"kind":"upToDate"}"#
        );
    }
}
