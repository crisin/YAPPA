//! The UI's second door on the Rust side (ADR-004, ADR-007): Tauri commands over
//! `crates/client`. Session and voice tokens never reach the webview — joining a channel
//! happens here (`voice_join`): token from the control plane, `Join` to the engine, channel
//! into the presence room.

use crate::App;
use api_types::{Channel, PresenceEntry, PresenceState, PublicConfig, TalkMode, User};
use client::{ApiFailure, AppSettings, Client, KeyAction};
use engine_protocol::Command;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// Everybody online, pushed on every change.
pub const PRESENCE_EVENT: &str = "presence";

/// The live presence connection, once logged in.
#[derive(Default)]
pub struct PresenceSlot(pub Mutex<Option<client::Presence>>);

type Reply<T> = Result<T, ApiFailure>;

/// Ending a presence connection joins its thread. That never happens under the slot's lock
/// (the sync commands would freeze the UI) nor on an async worker (realtime review C1).
fn retire(old: Option<client::Presence>) {
    if let Some(old) = old {
        std::thread::spawn(move || drop(old));
    }
}

fn talk_mode(settings: &engine_protocol::Settings) -> TalkMode {
    match settings.transmit_mode {
        engine_protocol::TransmitMode::PushToTalk => TalkMode::Ptt,
        engine_protocol::TransmitMode::AlwaysOn => TalkMode::Open,
        engine_protocol::TransmitMode::VoiceActivation => TalkMode::Vad,
    }
}

/// Joins the presence room with a fresh token (after login, at start). Replaces an older
/// connection.
pub async fn start_presence(handle: &AppHandle, app: &App, client: &Arc<Client>) -> Reply<User> {
    let me = client.me().await?;
    let initial = PresenceState {
        channel_id: None,
        app: client.app_label(&app.app_settings.lock().expect("lock")),
        talk: app
            .with_engine(|e| talk_mode(&e.settings()))
            .unwrap_or_default(),
        away: false,
        ..PresenceState::default()
    };
    let emitter = handle.clone();
    let presence = client
        .start_presence(&me.presence, initial, move |entries| {
            let _ = emitter.emit(PRESENCE_EVENT, entries);
        })
        .map_err(|e| ApiFailure::new("presence_failed", &e.to_string()))?;
    let old = app.presence.0.lock().expect("lock").replace(presence);
    retire(old);
    // Mute and deafen are the engine's state (A3): a resync makes it send `MuteState`
    // again, and the event callback writes it into the presence just created — so someone
    // who muted before logging in is shown muted from the first moment.
    app.with_engine(|e| e.send(Command::Resync));
    Ok(me.user)
}

#[tauri::command]
pub fn api_session(app: State<'_, App>) -> Option<User> {
    app.client.session().map(|s| s.user)
}

/// Log in with an invite link (clicked or pasted) and the name the others will see.
#[tauri::command]
pub async fn api_redeem(
    handle: AppHandle,
    app: State<'_, App>,
    link: String,
    display_name: String,
) -> Reply<User> {
    let client = app.client.clone();
    client.redeem(&link, &display_name).await?;
    start_presence(&handle, &app, &client).await
}

/// At start: if a session is stored, check it and join the presence room.
#[tauri::command]
pub async fn api_resume(handle: AppHandle, app: State<'_, App>) -> Reply<Option<User>> {
    let client = app.client.clone();
    if client.session().is_none() {
        return Ok(None);
    }
    start_presence(&handle, &app, &client).await.map(Some)
}

#[tauri::command]
pub async fn api_logout(app: State<'_, App>) -> Reply<()> {
    app.with_engine(|e| e.send(Command::Leave));
    let old = app.presence.0.lock().expect("lock").take();
    retire(old);
    app.client.logout().await;
    Ok(())
}

#[tauri::command]
pub async fn api_config(app: State<'_, App>) -> Reply<PublicConfig> {
    app.client.config().await
}

#[tauri::command]
pub async fn api_channels(app: State<'_, App>) -> Reply<Vec<Channel>> {
    app.client.channels().await
}

#[tauri::command]
pub fn api_presence(app: State<'_, App>) -> Vec<PresenceEntry> {
    app.presence
        .0
        .lock()
        .expect("lock")
        .as_ref()
        .map(client::Presence::entries)
        .unwrap_or_default()
}

/// What the UI may change about its own presence; the channel follows `voice_join`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresencePatch {
    talk: Option<TalkMode>,
    away: Option<bool>,
}

#[tauri::command]
pub fn api_set_presence(app: State<'_, App>, patch: PresencePatch) {
    if let Some(presence) = app.presence.0.lock().expect("lock").as_ref() {
        presence.update(|state| {
            if let Some(talk) = patch.talk {
                state.talk = talk;
            }
            if let Some(away) = patch.away {
                state.away = away;
            }
        });
    }
}

/// Into a channel: token from the control plane, the engine joins, the others see where.
#[tauri::command]
pub async fn voice_join(app: State<'_, App>, channel_id: String) -> Reply<()> {
    let token = app.client.voice_token(&channel_id).await?;
    let set_channel = |id: Option<String>| {
        if let Some(presence) = app.presence.0.lock().expect("lock").as_ref() {
            presence.update(|state| state.channel_id = id);
        }
    };
    // Presence first, then the join: a failed join ends in `Disconnected`, which clears the
    // channel again (lib.rs) — that must not be overtaken by a late set from here.
    set_channel(Some(channel_id));
    let sent = app.with_engine(|e| {
        e.send(Command::Join {
            url: token.ws_url.clone(),
            token: token.token.clone(),
        });
    });
    if sent.is_none() {
        set_channel(None);
        return Err(ApiFailure::new(
            "engine_down",
            "Die Audio-Engine läuft nicht.",
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn voice_leave(app: State<'_, App>) {
    app.with_engine(|e| e.send(Command::Leave));
    if let Some(presence) = app.presence.0.lock().expect("lock").as_ref() {
        presence.update(|state| state.channel_id = None);
    }
}

#[tauri::command]
pub fn app_settings(app: State<'_, App>) -> AppSettings {
    app.app_settings.lock().expect("lock").clone()
}

#[tauri::command]
pub fn set_app_settings(app: State<'_, App>, mut settings: AppSettings) {
    settings.dedupe_keys();
    client::settings::save_app(app.client.profile(), &settings);
    crate::bind_keys(&app.hotkeys, &settings);
    *app.app_settings.lock().expect("lock") = settings;
}

/// Waits (up to 10 s) for the next key or mouse button and makes it the key of `action`
/// (push-to-talk, mute or deafen). `None`: cancelled with Escape, or nothing pressed. A key
/// another action already has is refused with `key_taken` and left as it was.
#[tauri::command]
pub async fn capture_key(app: State<'_, App>, action: KeyAction) -> Reply<Option<String>> {
    let hotkeys = app.hotkeys.clone();
    let key = tauri::async_runtime::spawn_blocking(move || {
        hotkeys.capture_next(std::time::Duration::from_secs(10))
    })
    .await
    .map_err(|e| ApiFailure::new("capture_failed", &e.to_string()))?;
    let Some(key) = key else {
        return Ok(None);
    };
    let settings = {
        let mut settings = app.app_settings.lock().expect("lock");
        if let Err(holder) = settings.assign_key(action, &key) {
            return Err(ApiFailure::new(
                "key_taken",
                &format!("Diese Taste ist schon für „{}“ belegt.", holder.label()),
            ));
        }
        settings.clone()
    };
    client::settings::save_app(app.client.profile(), &settings);
    crate::bind_keys(&app.hotkeys, &settings);
    tracing::info!(%key, ?action, "global key set");
    Ok(Some(key))
}

/// False when the global hook could not start: the key then only works in the window.
#[tauri::command]
pub fn hotkeys_global(app: State<'_, App>) -> bool {
    app.hotkeys.is_global()
}
