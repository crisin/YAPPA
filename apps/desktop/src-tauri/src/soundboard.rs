//! The soundboard's library (ADR-010): a folder. Every sound file in it is a button in the
//! UI; „Sound hinzufügen“ copies a file in, „Ordner öffnen“ shows it in the file manager
//! so one can drop files there by hand, „Löschen“ moves a file to the recycle bin. Each
//! sound's own volume lives in `soundboard.json` next to the files, keyed by file name, so
//! it travels with the folder. The engine plays by path; nothing here is audio.

use crate::App;
use base64::Engine as _;
use engine_protocol::Sound;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The sidecar with everything the folder itself cannot hold.
const SIDECAR: &str = "soundboard.json";
/// A sound's own volume, dB — the same range as the soundboard gain.
const GAIN_DB: std::ops::RangeInclusive<f32> = -30.0..=12.0;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Sidecar {
    /// File name → own volume in dB; absent = 0 dB.
    #[serde(default)]
    gains: BTreeMap<String, f32>,
}

fn read_sidecar(dir: &Path) -> Sidecar {
    match std::fs::read(dir.join(SIDECAR)) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "soundboard.json is damaged, starting over");
            Sidecar::default()
        }),
        Err(_) => Sidecar::default(),
    }
}

fn write_sidecar(dir: &Path, sidecar: &Sidecar) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(sidecar).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(SIDECAR), json)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A path the UI hands back must be a file of this folder — the UI only ever sees paths
/// from [`list`], but the IPC boundary is not the place to trust that.
fn in_folder(dir: &Path, path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    let ok = path.parent() == Some(dir) && path.is_file() && is_sound(path);
    ok.then(|| path.to_path_buf())
        .ok_or_else(|| "Dieser Sound liegt nicht im Soundboard-Ordner.".into())
}

/// What the engine's decoder reads (symphonia: WAV, FLAC, Ogg Vorbis, MP3, AAC/M4A).
const EXTENSIONS: [&str; 7] = ["wav", "flac", "ogg", "oga", "mp3", "m4a", "aac"];
/// A soundboard clip, not an album: the engine cuts at 60 s anyway.
const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;

/// The folder under the app's data directory, shared by every profile — two windows on
/// one PC should have the same sounds.
pub fn folder(paths: &tauri::path::PathResolver<tauri::Wry>) -> std::io::Result<PathBuf> {
    let dir = paths
        .app_data_dir()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .join("soundboard");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn is_sound(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The sounds in a folder, by name, with their own volume from the sidecar.
pub fn list(dir: &Path) -> std::io::Result<Vec<Sound>> {
    let sidecar = read_sidecar(dir);
    let mut sounds: Vec<Sound> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_sound(path))
        .map(|path| Sound {
            name: stem(&path),
            gain_db: sidecar.gains.get(&file_name(&path)).copied().unwrap_or(0.0),
            path: path.display().to_string(),
        })
        .collect();
    sounds.sort_by_key(|s| s.name.to_lowercase());
    Ok(sounds)
}

/// Remembers one sound's own volume in the sidecar.
pub fn set_gain(dir: &Path, path: &str, gain_db: f32) -> Result<(), String> {
    if !GAIN_DB.contains(&gain_db) || gain_db.is_nan() {
        return Err(format!("Lautstärke {gain_db} dB außerhalb von −30..12."));
    }
    let path = in_folder(dir, path)?;
    let mut sidecar = read_sidecar(dir);
    if gain_db == 0.0 {
        sidecar.gains.remove(&file_name(&path));
    } else {
        sidecar.gains.insert(file_name(&path), gain_db);
    }
    write_sidecar(dir, &sidecar).map_err(|e| e.to_string())
}

/// Moves a sound to the recycle bin (a wrong click is not the end of a meme) and forgets
/// its volume.
pub fn remove(dir: &Path, path: &str) -> Result<(), String> {
    let path = in_folder(dir, path)?;
    trash::delete(&path)
        .map_err(|e| format!("Konnte die Datei nicht in den Papierkorb legen: {e}"))?;
    let mut sidecar = read_sidecar(dir);
    if sidecar.gains.remove(&file_name(&path)).is_some() {
        write_sidecar(dir, &sidecar).map_err(|e| e.to_string())?;
    }
    tracing::info!(path = %path.display(), "sound removed");
    Ok(())
}

/// A file name as it may land in the folder: the base name only, no path, no control
/// characters; `None` when nothing usable is left or the type is not a sound.
pub fn safe_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?.trim();
    let clean: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        .collect();
    (!clean.is_empty() && clean != "." && clean != ".." && is_sound(Path::new(&clean)))
        .then_some(clean)
}

#[tauri::command]
pub fn soundboard_list(app: tauri::State<'_, App>) -> Result<Vec<Sound>, String> {
    list(&app.sound_dir).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn soundboard_set_gain(
    app: tauri::State<'_, App>,
    path: String,
    gain_db: f32,
) -> Result<(), String> {
    set_gain(&app.sound_dir, &path, gain_db)
}

#[tauri::command]
pub fn soundboard_remove(app: tauri::State<'_, App>, path: String) -> Result<(), String> {
    remove(&app.sound_dir, &path)
}

#[tauri::command]
pub fn soundboard_folder(app: tauri::State<'_, App>) -> String {
    app.sound_dir.display().to_string()
}

/// Shows the folder in the file manager (Explorer, Finder).
#[tauri::command]
pub fn soundboard_open_folder(app: tauri::State<'_, App>) -> Result<(), String> {
    tauri_plugin_opener::open_path(&app.sound_dir, None::<&str>).map_err(|e| e.to_string())
}

/// Copies a file the UI picked into the folder. The bytes come base64-encoded over IPC —
/// a clip is a few hundred kilobytes, the simplest path is fine.
#[tauri::command]
pub fn soundboard_add(
    app: tauri::State<'_, App>,
    name: String,
    base64: String,
) -> Result<Sound, String> {
    let name = safe_name(&name).ok_or_else(|| {
        format!(
            "„{name}“ ist keine Sounddatei (erlaubt: {})",
            EXTENSIONS.join(", ")
        )
    })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64.as_bytes())
        .map_err(|e| format!("damaged upload: {e}"))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err("Die Datei ist größer als 20 MB – ein Soundboard-Clip ist kürzer.".into());
    }
    let path = app.sound_dir.join(&name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    tracing::info!(path = %path.display(), "sound added");
    Ok(Sound {
        name: stem(&path),
        gain_db: read_sidecar(&app.sound_dir)
            .gains
            .get(&name)
            .copied()
            .unwrap_or(0.0),
        path: path.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stripped_to_a_safe_base_name_and_must_be_a_sound() {
        assert_eq!(
            safe_name("C:\\Users\\x\\airhorn.MP3"),
            Some("airhorn.MP3".into())
        );
        assert_eq!(safe_name("../../evil.wav"), Some("evil.wav".into()));
        assert_eq!(safe_name("a:b?c.ogg"), Some("abc.ogg".into()));
        assert_eq!(safe_name("notes.txt"), None);
        assert_eq!(safe_name(".wav"), None, "a dot file has no extension");
        assert_eq!(safe_name(""), None);
    }

    #[test]
    fn the_folder_lists_sounds_by_name_and_ignores_the_rest() {
        let dir = std::env::temp_dir().join(format!("yappa-sb-list-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for file in ["Zonk.wav", "airhorn.mp3", "readme.txt", "cover.png"] {
            std::fs::write(dir.join(file), b"x").unwrap();
        }
        let names: Vec<String> = list(&dir).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["airhorn", "Zonk"]);
    }

    #[test]
    fn a_sounds_volume_lives_in_the_sidecar_and_only_folder_files_are_touched() {
        let dir = std::env::temp_dir().join(format!("yappa-sb-gain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zonk = dir.join("Zonk.wav");
        std::fs::write(&zonk, b"x").unwrap();
        let zonk = zonk.display().to_string();
        set_gain(&dir, &zonk, -6.0).unwrap();
        assert_eq!(list(&dir).unwrap()[0].gain_db, -6.0);
        let json = std::fs::read_to_string(dir.join(SIDECAR)).unwrap();
        assert!(json.contains("\"Zonk.wav\": -6.0"), "{json}");
        // 0 dB is the default and is not written down.
        set_gain(&dir, &zonk, 0.0).unwrap();
        assert!(!std::fs::read_to_string(dir.join(SIDECAR))
            .unwrap()
            .contains("Zonk"));
        assert!(set_gain(&dir, &zonk, 40.0).is_err());
        // A path outside the folder, or one that is not a sound, is refused — not touched.
        let elsewhere = dir.join("..").join("elsewhere.wav");
        std::fs::write(&elsewhere, b"x").unwrap();
        assert!(set_gain(&dir, &elsewhere.display().to_string(), -3.0).is_err());
        assert!(remove(&dir, &elsewhere.display().to_string()).is_err());
        assert!(elsewhere.is_file());
        std::fs::remove_file(&elsewhere).unwrap();
        let sidecar = dir.join(SIDECAR).display().to_string();
        assert!(remove(&dir, &sidecar).is_err());
        // A damaged sidecar is not fatal: the sounds still list, at 0 dB.
        std::fs::write(dir.join(SIDECAR), b"{ nope").unwrap();
        assert_eq!(list(&dir).unwrap()[0].gain_db, 0.0);
    }
}
