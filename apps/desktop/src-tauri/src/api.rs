//! The UI's second door on the Rust side (ADR-004, ADR-007): Tauri commands over
//! `crates/client`. Session and voice tokens never reach the webview — joining a channel
//! happens here (`voice_join`): token from the control plane, `Join` to the engine, channel
//! into the presence room.

use crate::App;
use api_types::{Channel, PresenceEntry, PresenceState, PublicConfig, TalkMode, User};
use client::{ApiFailure, AppSettings, Client};
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
    };
    let emitter = handle.clone();
    let presence = client
        .start_presence(&me.presence, initial, move |entries| {
            let _ = emitter.emit(PRESENCE_EVENT, entries);
        })
        .map_err(|e| ApiFailure::new("presence_failed", &e.to_string()))?;
    let old = app.presence.0.lock().expect("lock").replace(presence);
    retire(old);
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
    app.with_engine(|e| {
        e.send(Command::Join {
            url: token.ws_url.clone(),
            token: token.token.clone(),
        });
    })
    .ok_or_else(|| ApiFailure::new("engine_down", "Die Audio-Engine läuft nicht."))?;
    if let Some(presence) = app.presence.0.lock().expect("lock").as_ref() {
        presence.update(|state| state.channel_id = Some(channel_id));
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
pub fn set_app_settings(app: State<'_, App>, settings: AppSettings) {
    client::settings::save_app(app.client.profile(), &settings);
    *app.app_settings.lock().expect("lock") = settings;
}
