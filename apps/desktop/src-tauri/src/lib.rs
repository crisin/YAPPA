//! Tauri shell. It owns the window and OS integration, starts the engine with the LiveKit
//! transport and the real sound hardware, and forwards `engine-protocol` messages in both
//! directions. It never does audio itself.

pub mod api;
pub mod logging;

use api_types::{ClientInfo, Invite};
use audio_engine::{CpalIo, EngineConfig, EngineHandle};
use client::{AppSettings, Client, Profile};
use engine_protocol::{Command, Event, LogEntry, Settings, PROTOCOL_VERSION};
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use transport::livekit::LiveKitTransport;

/// The one channel on which every [`Event`] reaches the UI.
const ENGINE_EVENT: &str = "engine-event";

/// Engine with LiveKit and the real devices — also used by the headless example.
pub fn start_engine(
    settings: Settings,
    on_event: impl Fn(Event) + Send + 'static,
) -> std::io::Result<EngineHandle> {
    let transport = LiveKitTransport::new()?;
    Ok(audio_engine::spawn(
        Box::new(transport),
        Box::new(CpalIo),
        settings,
        EngineConfig::default(),
        on_event,
    ))
}

/// The engine, until the app exits. Taken out at exit so that dropping it leaves the room
/// properly and closes the devices.
type SharedEngine = Arc<Mutex<Option<EngineHandle>>>;

pub struct App {
    engine: SharedEngine,
    logging: logging::Logging,
    client: Arc<Client>,
    app_settings: Mutex<AppSettings>,
    presence: api::PresenceSlot,
}

impl App {
    fn with_engine<T>(&self, f: impl FnOnce(&EngineHandle) -> T) -> Option<T> {
        self.engine.lock().unwrap().as_ref().map(f)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    protocol_version: u32,
    log_dir: String,
    /// Development aid: `YAPPA_DEBUG_PANEL=1` opens the debug panel at start.
    debug_panel: bool,
    /// `YAPPA_PROFILE`, "default" without it — shown so two windows can be told apart.
    profile: String,
}

#[tauri::command]
fn app_info(app: tauri::State<'_, App>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        protocol_version: PROTOCOL_VERSION,
        log_dir: app.logging.dir.display().to_string(),
        debug_panel: std::env::var_os("YAPPA_DEBUG_PANEL").is_some(),
        profile: app.client.profile().name().to_string(),
    }
}

#[tauri::command]
fn get_settings(app: tauri::State<'_, App>) -> Settings {
    app.with_engine(EngineHandle::settings).unwrap_or_default()
}

#[tauri::command]
fn engine_command(app: tauri::State<'_, App>, command: Command) -> Result<(), String> {
    app.with_engine(|engine| engine.send(command))
        .ok_or_else(|| "the engine is not running".to_string())
}

/// The UI opened (or reloaded): hand it what was logged so far and have the engine send
/// its current state again.
#[tauri::command]
fn recent_logs(app: tauri::State<'_, App>) -> Vec<LogEntry> {
    app.with_engine(|engine| engine.send(Command::Resync));
    app.logging.recent()
}

#[tauri::command]
fn decode_invite(text: String) -> Result<Invite, String> {
    Invite::decode(&text).map_err(|e| e.to_string())
}

/// Writes one file with everything needed to look into a problem and returns its path.
#[tauri::command]
fn export_logs(handle: tauri::AppHandle, app: tauri::State<'_, App>) -> Result<String, String> {
    let folder = handle
        .path()
        .download_dir()
        .or_else(|_| handle.path().app_log_dir())
        .map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = folder.join(format!("yappa-diagnose-{stamp}.txt"));
    let settings = app.with_engine(EngineHandle::settings);
    write_diagnostics(&path, settings.as_ref(), &app.logging).map_err(|e| e.to_string())?;
    tracing::info!(path = %path.display(), "diagnostics exported");
    Ok(path.display().to_string())
}

/// Links in the in-app help open in the system browser. Only web addresses: the webview
/// must never be able to start a program or open a file through this.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("not a web address: {url}"));
    }
    tauri_plugin_opener::open_url(&url, None::<&str>).map_err(|e| e.to_string())
}

fn write_diagnostics(
    path: &Path,
    settings: Option<&Settings>,
    logging: &logging::Logging,
) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(out, "yAPPA diagnostics")?;
    writeln!(out, "app version: {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(out, "protocol:    {PROTOCOL_VERSION}")?;
    writeln!(
        out,
        "system:      {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )?;
    if let Some(settings) = settings {
        let json = serde_json::to_string(settings).unwrap_or_default();
        writeln!(out, "settings:    {json}")?;
    }
    for file in logging.log_files() {
        writeln!(out, "\n===== {} =====", file.display())?;
        // Read lossy: the writer may be in the middle of a line.
        let bytes = std::fs::read(&file)?;
        out.write_all(String::from_utf8_lossy(&bytes).as_bytes())?;
    }
    out.flush()
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let paths = app.path();
    // `YAPPA_PROFILE=<name>`: a second client on the same PC with its own settings, login
    // and logs (ADR-007).
    let profile = Profile::from_env(&paths.app_config_dir()?);
    let log_dir = match profile.name() {
        "default" => paths.app_log_dir()?,
        name => paths.app_log_dir()?.join("profiles").join(name),
    };
    let logging = logging::init(&log_dir)?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        profile = profile.name(),
        log_dir = %logging.dir.display(),
        "yAPPA starting"
    );
    std::fs::create_dir_all(profile.dir())?;
    let loaded = client::settings::load(&profile);
    if loaded.migrated {
        tracing::info!("settings migrated to the current schema");
        client::settings::save_engine(&profile, &loaded.engine);
        client::settings::save_app(&profile, &loaded.app);
    }
    let settings = loaded.engine;
    let client = Arc::new(Client::new(
        profile.clone(),
        ClientInfo {
            app_version: env!("CARGO_PKG_VERSION").into(),
            protocol_version: PROTOCOL_VERSION,
            platform: std::env::consts::OS.into(),
        },
    ));

    let handle = app.handle().clone();
    logging.on_entry(move |entry| {
        let _ = handle.emit(ENGINE_EVENT, Event::Log { entry });
    });

    // The engine reports once per second anyway; that is when changed settings are saved.
    // The callback only gets a weak reference: the engine must not keep itself alive.
    let engine: SharedEngine = Arc::new(Mutex::new(None));
    let saved = Mutex::new(settings.clone());
    let (handle, for_saving) = (app.handle().clone(), Arc::downgrade(&engine));
    let started = start_engine(settings, move |event| {
        if matches!(event, Event::Stats { .. }) {
            let current = for_saving
                .upgrade()
                .and_then(|engine| engine.lock().unwrap().as_ref().map(EngineHandle::settings));
            if let Some(current) = current {
                let mut saved = saved.lock().unwrap();
                if *saved != current {
                    client::settings::save_engine(&profile, &current);
                    tracing::debug!("settings saved");
                    *saved = current;
                }
            }
        }
        let _ = handle.emit(ENGINE_EVENT, event);
    })?;

    // Development aid: `YAPPA_AUTOJOIN=<invite>` joins right after start (docs/dev-setup.md).
    if let Ok(text) = std::env::var("YAPPA_AUTOJOIN") {
        match Invite::decode(&text) {
            Ok(invite) => started.send(Command::Join {
                url: invite.url,
                token: invite.token,
            }),
            Err(e) => tracing::warn!(error = %e, "YAPPA_AUTOJOIN is not a usable invite"),
        }
    }
    *engine.lock().unwrap() = Some(started);
    app.manage(App {
        engine,
        logging,
        client,
        app_settings: Mutex::new(loaded.app),
        presence: api::PresenceSlot::default(),
    });
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_settings,
            engine_command,
            recent_logs,
            decode_invite,
            export_logs,
            open_url,
            api::api_session,
            api::api_resume,
            api::api_redeem,
            api::api_logout,
            api::api_config,
            api::api_channels,
            api::api_presence,
            api::api_set_presence,
            api::voice_join,
            api::voice_leave,
            api::app_settings,
            api::set_app_settings
        ])
        .build(tauri::generate_context!())
        .expect("error while starting yAPPA")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Take the engine out and drop it without holding the lock: its thread may
                // be in the callback above, waiting for that lock.
                let state = handle.state::<App>();
                let engine = state.engine.lock().unwrap().take();
                let presence = state.presence.0.lock().unwrap().take();
                tracing::info!("yAPPA exiting");
                drop(presence);
                drop(engine);
            }
        });
}
