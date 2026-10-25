//! The soundboard's library (ADR-010): a folder. Every sound file in it is a button in the
//! UI; „Sound hinzufügen“ copies a file in, „Ordner öffnen“ shows it in the file manager
//! so one can drop files there by hand. The engine plays by path; nothing here is audio.

use crate::App;
use base64::Engine as _;
use engine_protocol::Sound;
use std::path::{Path, PathBuf};

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

/// The sounds in a folder, by name.
pub fn list(dir: &Path) -> std::io::Result<Vec<Sound>> {
    let mut sounds: Vec<Sound> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_sound(path))
        .map(|path| Sound {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: path.display().to_string(),
        })
        .collect();
    sounds.sort_by_key(|s| s.name.to_lowercase());
    Ok(sounds)
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
        name: Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or(name),
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
}
