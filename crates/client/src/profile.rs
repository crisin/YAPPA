//! Where a client keeps its files. The default profile uses the app's config dir as it is
//! (so existing settings stay where they are); `YAPPA_PROFILE=<name>` gives a second client
//! on the same PC its own folder, settings, session and logs.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const ENGINE_SETTINGS_FILE: &str = "settings.json";
pub const APP_SETTINGS_FILE: &str = "app-settings.json";
pub const SESSION_FILE: &str = "session.json";
pub const CHANNELS_CACHE: &str = "cache/channels.json";
pub const CONFIG_CACHE: &str = "cache/config.json";
pub const TOKEN_FALLBACK_FILE: &str = "session-token.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    name: Option<String>,
    dir: PathBuf,
}

impl Profile {
    /// `name` = `None` or empty → the default profile in `base` itself.
    pub fn new(base: &Path, name: Option<&str>) -> Self {
        let name = name.map(str::trim).filter(|n| !n.is_empty()).map(|n| {
            n.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        });
        let dir = match &name {
            Some(n) => base.join("profiles").join(n),
            None => base.to_path_buf(),
        };
        Self { name, dir }
    }

    /// From `YAPPA_PROFILE`.
    pub fn from_env(base: &Path) -> Self {
        Self::new(base, std::env::var("YAPPA_PROFILE").ok().as_deref())
    }

    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("default")
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, file: &str) -> PathBuf {
        self.dir.join(file)
    }

    /// `None` when missing or unreadable — a broken cache must never stop the app.
    pub fn read_json<T: DeserializeOwned>(&self, file: &str) -> Option<T> {
        let text = std::fs::read_to_string(self.path(file)).ok()?;
        match serde_json::from_str(&text) {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::warn!(file, %error, "ignoring an unreadable file");
                None
            }
        }
    }

    /// Best effort, logged on failure: losing a cache write is not worth an error dialog.
    pub fn write_json<T: Serialize>(&self, file: &str, value: &T) {
        let path = self.path(file);
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
                // Write next to it, then rename: a crash mid-write leaves the old file.
                let temp = path.with_extension("json.tmp");
                std::fs::write(&temp, json)?;
                std::fs::rename(&temp, &path)
            });
        if let Err(error) = result {
            tracing::warn!(file, %error, "could not write");
        }
    }

    pub fn remove(&self, file: &str) {
        let _ = std::fs::remove_file(self.path(file));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_profile_is_the_base_dir_and_names_are_sanitised() {
        let base = Path::new("/cfg");
        assert_eq!(Profile::new(base, None).dir(), base);
        assert_eq!(Profile::new(base, Some("  ")).name(), "default");
        let second = Profile::new(base, Some("zweiter Client/../x"));
        assert_eq!(second.name(), "zweiter_Client____x");
        assert_eq!(
            second.dir(),
            base.join("profiles").join("zweiter_Client____x")
        );
    }

    #[test]
    fn json_files_round_trip_and_broken_ones_are_none() {
        let dir = std::env::temp_dir().join(format!("yappa-profile-{}", std::process::id()));
        let profile = Profile::new(&dir, Some("t"));
        profile.write_json("cache/x.json", &vec![1, 2, 3]);
        assert_eq!(
            profile.read_json::<Vec<i32>>("cache/x.json"),
            Some(vec![1, 2, 3])
        );
        std::fs::write(profile.path("cache/x.json"), "{ kaputt").unwrap();
        assert_eq!(profile.read_json::<Vec<i32>>("cache/x.json"), None);
        profile.remove("cache/x.json");
        assert_eq!(profile.read_json::<Vec<i32>>("cache/x.json"), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
