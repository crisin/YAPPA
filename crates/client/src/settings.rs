//! Two settings files (ADR-007): the engine's (`engine_protocol::Settings`, audio and
//! sending) and the app's (this file: view, theme, keys, notifications, update channel).
//! Loading migrates old files; a file that cannot be used is replaced by defaults and the
//! reason logged — settings never keep the app from starting.

use crate::profile::{Profile, APP_SETTINGS_FILE, ENGINE_SETTINGS_FILE};
use engine_protocol::{Settings, SETTINGS_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

pub const APP_SETTINGS_VERSION: u32 = 1;

/// Which settings view „Ich“ shows. A property of the settings, not of the modules.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SettingsView {
    #[default]
    Simple,
    Pro,
}

/// Update channels (ADR-006).
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateChannel {
    Nightly,
    Beta,
    /// The default for people who did not choose (planning doc, "App-Updates").
    #[default]
    Stable,
}

impl UpdateChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nightly => "nightly",
            Self::Beta => "beta",
            Self::Stable => "stable",
        }
    }
}

/// Everything about the app that is not audio. Unknown fields are ignored and missing ones
/// take their defaults, so older and newer files both load.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppSettings {
    pub version: u32,
    pub view: SettingsView,
    /// A theme id from `ui-tokens/themes/`.
    pub theme: String,
    /// Push-to-talk key as the platform hotkey layer names it; `None` = not set (S3).
    pub ptt_key: Option<String>,
    pub notifications: bool,
    pub update_channel: UpdateChannel,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: APP_SETTINGS_VERSION,
            view: SettingsView::Simple,
            theme: "dark".into(),
            ptt_key: None,
            notifications: true,
            update_channel: UpdateChannel::Stable,
        }
    }
}

/// What a file held, once read.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub engine: Settings,
    pub app: AppSettings,
    /// True when an old file was converted; the caller should save both.
    pub migrated: bool,
}

/// Reads both files of a profile, migrating the engine file from version 1 (which carried
/// the theme) to 2.
pub fn load(profile: &Profile) -> Loaded {
    let raw: Option<Value> = profile.read_json(ENGINE_SETTINGS_FILE);
    // A file this build cannot use (newer version, broken) is kept as `.bak` before the
    // defaults are ever saved over it.
    if let Some(value) = &raw {
        let usable = serde_json::from_value::<Settings>(value.clone())
            .ok()
            .is_some_and(|s| s.validate().is_ok());
        let old_version = value.get("version").and_then(Value::as_u64) == Some(1);
        if !usable && !old_version {
            let from = profile.path(ENGINE_SETTINGS_FILE);
            let _ = std::fs::copy(&from, from.with_extension("json.bak"));
        }
    }
    let mut app: AppSettings = profile.read_json(APP_SETTINGS_FILE).unwrap_or_default();
    let app_file_exists = profile.path(APP_SETTINGS_FILE).exists();
    let (engine, theme, migrated) = migrate_engine(raw);
    if let Some(theme) = theme.filter(|_| !app_file_exists) {
        app.theme = theme;
    }
    Loaded {
        engine,
        app,
        migrated,
    }
}

pub fn save_engine(profile: &Profile, settings: &Settings) {
    profile.write_json(ENGINE_SETTINGS_FILE, settings);
}

pub fn save_app(profile: &Profile, settings: &AppSettings) {
    profile.write_json(APP_SETTINGS_FILE, settings);
}

/// `(settings, theme from a v1 file, migrated?)`.
fn migrate_engine(raw: Option<Value>) -> (Settings, Option<String>, bool) {
    let Some(mut value) = raw else {
        return (Settings::default(), None, false);
    };
    let version = value.get("version").and_then(Value::as_u64).unwrap_or(0);
    let mut theme = None;
    let mut migrated = false;
    if version == 1 {
        if let Some(object) = value.as_object_mut() {
            theme = object
                .remove("theme")
                .and_then(|t| t.as_str().map(str::to_string));
            object.insert("version".into(), Value::from(SETTINGS_VERSION));
            migrated = true;
        }
    }
    let settings = serde_json::from_value::<Settings>(value)
        .map_err(|e| e.to_string())
        .and_then(|s| s.validate().map(|()| s));
    match settings {
        Ok(settings) => (settings, theme, migrated),
        Err(problem) => {
            // Not "migrated": nothing is written over the file until the user changes a
            // setting, and `load` keeps a copy first (realtime review C1).
            tracing::warn!(%problem, "engine settings unusable — starting with defaults");
            (Settings::default(), theme, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn v1() -> Value {
        let mut value = serde_json::to_value(Settings::default()).unwrap();
        value["version"] = json!(1);
        value["theme"] = json!("light");
        value["voiceBitrateKbps"] = json!(96);
        value
    }

    #[test]
    fn a_version_one_file_keeps_its_values_and_hands_over_the_theme() {
        let (settings, theme, migrated) = migrate_engine(Some(v1()));
        assert!(migrated);
        assert_eq!(settings.version, SETTINGS_VERSION);
        assert_eq!(settings.voice_bitrate_kbps, 96);
        assert_eq!(theme.as_deref(), Some("light"));
    }

    #[test]
    fn current_files_load_as_they_are_and_broken_ones_become_defaults() {
        let current = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            migrate_engine(Some(current)),
            (Settings::default(), None, false)
        );
        let (settings, _, migrated) = migrate_engine(Some(json!({ "version": 2, "eq": 7 })));
        assert_eq!(settings, Settings::default());
        assert!(!migrated, "a broken file is not overwritten by itself");
    }

    #[test]
    fn loading_a_profile_moves_the_theme_into_the_app_settings_once() {
        let dir = std::env::temp_dir().join(format!("yappa-settings-{}", std::process::id()));
        let profile = Profile::new(&dir, Some("t"));
        profile.write_json(ENGINE_SETTINGS_FILE, &v1());
        let loaded = load(&profile);
        assert!(loaded.migrated);
        assert_eq!(loaded.app.theme, "light");
        save_engine(&profile, &loaded.engine);
        save_app(
            &profile,
            &AppSettings {
                theme: "dark".into(),
                ..loaded.app
            },
        );
        let again = load(&profile);
        assert!(!again.migrated);
        assert_eq!(again.app.theme, "dark");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn app_settings_tolerate_missing_and_unknown_fields() {
        let partial: AppSettings =
            serde_json::from_value(json!({ "view": "pro", "future": true })).unwrap();
        assert_eq!(partial.view, SettingsView::Pro);
        assert_eq!(partial.theme, "dark");
        assert_eq!(partial.update_channel, UpdateChannel::Stable);
    }

    /// Writes the TS for the UI (`cargo xtask gen-types`).
    #[test]
    fn export_app_settings_defaults() {
        let Some(dir) = std::env::var_os("TS_RS_EXPORT_DIR") else {
            return;
        };
        let json = serde_json::to_string_pretty(&AppSettings::default()).unwrap() + "\n";
        let path = std::path::Path::new(&dir).join("defaultAppSettings.json");
        if std::fs::read_to_string(&path).ok().as_deref() != Some(json.as_str()) {
            std::fs::write(path, json).unwrap();
        }
    }
}
