//! Two settings files (ADR-007): the engine's (`engine_protocol::Settings`, audio and
//! sending) and the app's (this file: view, theme, keys, notifications, update channel).
//! Loading migrates old files; a file that cannot be used is replaced by defaults and the
//! reason logged — settings never keep the app from starting.

use crate::profile::{Profile, APP_SETTINGS_FILE, ENGINE_SETTINGS_FILE};
use engine_protocol::{QualityProfile, Settings, VoiceCodec, SETTINGS_VERSION};
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

/// What a global key does (A3). The shell maps this onto the platform hook; the UI uses it
/// to say which key it is choosing.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KeyAction {
    /// Hold to talk (in the „Taste halten“ mode).
    Ptt,
    /// Press to flip the microphone.
    Mute,
    /// Press to flip hearing (and, implied, the microphone).
    Deafen,
}

impl KeyAction {
    pub const ALL: [KeyAction; 3] = [KeyAction::Ptt, KeyAction::Mute, KeyAction::Deafen];

    /// How the UI names the action when it reports a conflict.
    pub fn label(self) -> &'static str {
        match self {
            KeyAction::Ptt => "Push-to-Talk",
            KeyAction::Mute => "Stumm",
            KeyAction::Deafen => "Taub",
        }
    }
}

/// What the person has on their ears, as chosen in the onboarding („Was hast du auf den
/// Ohren?“). Speakers feed the output back into the microphone, so „Mich selbst hören“ (A5)
/// would howl: the UI warns and does not start it by itself.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Listening {
    /// Set up before this field existed, or never asked.
    #[default]
    Unknown,
    Headset,
    Speakers,
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
    /// Global key that flips the microphone (A3); `None` = not set.
    pub mute_key: Option<String>,
    /// Global key that flips hearing (A3); `None` = not set.
    pub deafen_key: Option<String>,
    pub notifications: bool,
    pub update_channel: UpdateChannel,
    /// Headset or speakers (A5); `Unknown` for a file from before.
    pub listening: Listening,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: APP_SETTINGS_VERSION,
            view: SettingsView::Simple,
            theme: "dark".into(),
            ptt_key: None,
            mute_key: None,
            deafen_key: None,
            notifications: true,
            update_channel: UpdateChannel::Stable,
            listening: Listening::Unknown,
        }
    }
}

impl AppSettings {
    pub fn key(&self, action: KeyAction) -> Option<&str> {
        match action {
            KeyAction::Ptt => self.ptt_key.as_deref(),
            KeyAction::Mute => self.mute_key.as_deref(),
            KeyAction::Deafen => self.deafen_key.as_deref(),
        }
    }

    fn key_mut(&mut self, action: KeyAction) -> &mut Option<String> {
        match action {
            KeyAction::Ptt => &mut self.ptt_key,
            KeyAction::Mute => &mut self.mute_key,
            KeyAction::Deafen => &mut self.deafen_key,
        }
    }

    /// The action a key is bound to, if any.
    pub fn action_of(&self, key: &str) -> Option<KeyAction> {
        KeyAction::ALL
            .into_iter()
            .find(|a| self.key(*a) == Some(key))
    }

    /// Gives `action` the key. One key does one thing: a key another action already has is
    /// refused and that action returned, so the UI can say „schon Push-to-Talk“. Setting
    /// the key an action already has is fine.
    pub fn assign_key(&mut self, action: KeyAction, key: &str) -> Result<(), KeyAction> {
        match self.action_of(key) {
            Some(holder) if holder != action => Err(holder),
            _ => {
                *self.key_mut(action) = Some(key.to_string());
                Ok(())
            }
        }
    }

    /// Settings as the UI sent them may carry one key twice (two writers, a stale copy):
    /// the later action in [`KeyAction::ALL`] loses its key, so the hook never fires two
    /// actions on one press.
    pub fn dedupe_keys(&mut self) {
        for (n, action) in KeyAction::ALL.into_iter().enumerate() {
            let Some(key) = self.key(action).map(str::to_string) else {
                continue;
            };
            if KeyAction::ALL[..n]
                .iter()
                .any(|earlier| self.key(*earlier) == Some(key.as_str()))
            {
                *self.key_mut(action) = None;
            }
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
    if let Some(object) = value.as_object_mut() {
        // 1 -> 2 (ADR-007): the theme leaves for the app settings.
        if version == 1 {
            theme = object
                .remove("theme")
                .and_then(|t| t.as_str().map(str::to_string));
        }
        // 2 -> 3: the bare bitrate becomes a quality profile. The named profile with that
        // bitrate (mono, RED, no DTX — the only shape before) if there is one, else Custom
        // with the old value kept.
        if version <= 2 {
            let bitrate = object
                .remove("voiceBitrateKbps")
                .and_then(|b| b.as_u64())
                .and_then(|b| u32::try_from(b).ok())
                .unwrap_or(Settings::default().voice.bitrate_kbps);
            let voice = VoiceCodec {
                bitrate_kbps: bitrate,
                stereo: false,
                red: true,
                dtx: false,
            };
            object.insert(
                "quality".into(),
                serde_json::to_value(QualityProfile::matching(voice)).unwrap(),
            );
            object.insert("voice".into(), serde_json::to_value(voice).unwrap());
        }
        if (1..SETTINGS_VERSION as u64).contains(&version) {
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

    /// A file as version 2 wrote it: no profile, one bitrate.
    fn v2(bitrate: u32) -> Value {
        let mut value = serde_json::to_value(Settings::default()).unwrap();
        value["version"] = json!(2);
        let object = value.as_object_mut().unwrap();
        object.remove("quality");
        object.remove("voice");
        object.insert("voiceBitrateKbps".into(), json!(bitrate));
        value
    }

    fn v1() -> Value {
        let mut value = v2(96);
        value["version"] = json!(1);
        value["theme"] = json!("light");
        value
    }

    #[test]
    fn a_version_one_file_keeps_its_values_and_hands_over_the_theme() {
        let (settings, theme, migrated) = migrate_engine(Some(v1()));
        assert!(migrated);
        assert_eq!(settings.version, SETTINGS_VERSION);
        assert_eq!(settings.quality, QualityProfile::Custom);
        assert_eq!(settings.voice_codec().bitrate_kbps, 96);
        assert_eq!(theme.as_deref(), Some("light"));
    }

    #[test]
    fn a_version_two_bitrate_becomes_the_matching_profile_or_custom() {
        let (standard, _, migrated) = migrate_engine(Some(v2(80)));
        assert!(migrated);
        assert_eq!(standard.quality, QualityProfile::Standard);
        assert_eq!(standard.version, SETTINGS_VERSION);
        let (thrifty, _, _) = migrate_engine(Some(v2(32)));
        assert_eq!(thrifty.quality, QualityProfile::Thrifty);
        let (custom, _, _) = migrate_engine(Some(v2(64)));
        assert_eq!(custom.quality, QualityProfile::Custom);
        assert_eq!(
            custom.voice,
            VoiceCodec {
                bitrate_kbps: 64,
                stereo: false,
                red: true,
                dtx: false
            }
        );
    }

    #[test]
    fn current_files_load_as_they_are_and_broken_ones_become_defaults() {
        let current = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            migrate_engine(Some(current)),
            (Settings::default(), None, false)
        );
        let (settings, _, migrated) = migrate_engine(Some(json!({ "version": 3, "eq": 7 })));
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

    #[test]
    fn one_key_does_one_thing() {
        let mut app = AppSettings::default();
        app.assign_key(KeyAction::Ptt, "Mouse4").unwrap();
        assert_eq!(
            app.assign_key(KeyAction::Mute, "Mouse4"),
            Err(KeyAction::Ptt),
            "the holder is named"
        );
        assert_eq!(app.mute_key, None);
        app.assign_key(KeyAction::Ptt, "Mouse4").unwrap(); // the same again is fine
        app.assign_key(KeyAction::Mute, "F7").unwrap();
        app.assign_key(KeyAction::Deafen, "F8").unwrap();
        assert_eq!(app.action_of("F8"), Some(KeyAction::Deafen));
        assert_eq!(app.action_of("F9"), None);
        app.assign_key(KeyAction::Deafen, "F7").unwrap_err();
        assert_eq!(app.key(KeyAction::Deafen), Some("F8"));

        let mut doubled = AppSettings {
            ptt_key: Some("F7".into()),
            mute_key: Some("F7".into()),
            deafen_key: Some("F7".into()),
            ..AppSettings::default()
        };
        doubled.dedupe_keys();
        assert_eq!(doubled.ptt_key.as_deref(), Some("F7"));
        assert_eq!((doubled.mute_key, doubled.deafen_key), (None, None));
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
