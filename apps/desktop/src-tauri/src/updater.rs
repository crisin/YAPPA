//! In-app updates (ADR-006, D1). The endpoint is `<update_base_url>/<channel>/latest.json`
//! — the base from `GET /config`, the channel from the app settings — so moving the update
//! host is a server-side config change. A found update is downloaded and its signature
//! checked here, right away; it is installed only when the person clicks (never in the
//! middle of a call), and the app restarts.

use crate::App;
use client::{ApiFailure, UpdateStatus};
use engine_protocol::Command;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// An update that is downloaded and verified, waiting for the click.
pub struct Downloaded {
    update: Update,
    bytes: Vec<u8>,
}

#[derive(Default)]
pub struct UpdateSlot(pub Mutex<Option<Downloaded>>);

fn failed(reason: impl ToString) -> UpdateStatus {
    UpdateStatus::Failed {
        reason: reason.to_string(),
    }
}

/// Looks at the channel's manifest; a newer version is downloaded before this returns.
#[tauri::command]
pub async fn check_update(handle: AppHandle, app: State<'_, App>) -> Result<UpdateStatus, ()> {
    let config = match app.client.config().await {
        Ok(config) => config,
        Err(e) if e.code == "no_session" => {
            return Ok(UpdateStatus::Unavailable {
                reason: "Noch kein Server bekannt – erst anmelden.".into(),
            })
        }
        Err(e) => return Ok(failed(e.message)),
    };
    let channel = app.app_settings.lock().expect("lock").update_channel;
    let Some(endpoint) = client::update_endpoint(&config.update_base_url, channel) else {
        return Ok(UpdateStatus::Unavailable {
            reason: "Der Server nennt keinen Update-Host.".into(),
        });
    };
    let url = match endpoint.parse() {
        Ok(url) => url,
        Err(e) => return Ok(failed(format!("Update-Adresse unbrauchbar: {e}"))),
    };
    let updater = match handle
        .updater_builder()
        .endpoints(vec![url])
        .and_then(|b| b.build())
    {
        Ok(updater) => updater,
        Err(e) => return Ok(failed(e)),
    };
    tracing::info!(%endpoint, channel = channel.as_str(), "checking for updates");
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => return Ok(UpdateStatus::UpToDate),
        Err(e) => {
            tracing::warn!(error = %e, "update check failed");
            return Ok(failed(e));
        }
    };
    let version = update.version.clone();
    let notes = update.body.clone().unwrap_or_default();
    let already = app
        .update
        .0
        .lock()
        .expect("lock")
        .as_ref()
        .is_some_and(|d| d.update.version == version);
    if already {
        return Ok(UpdateStatus::Ready { version, notes });
    }
    tracing::info!(%version, "update found, downloading");
    let bytes = match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(error = %e, "update download failed");
            return Ok(failed(e));
        }
    };
    tracing::info!(%version, size = bytes.len(), "update downloaded and verified");
    *app.update.0.lock().expect("lock") = Some(Downloaded { update, bytes });
    Ok(UpdateStatus::Ready { version, notes })
}

/// Installs the downloaded update and restarts. The engine leaves the room first; on
/// Windows the installer ends this process itself, elsewhere the restart does.
#[tauri::command]
pub fn install_update(handle: AppHandle, app: State<'_, App>) -> Result<(), ApiFailure> {
    let downloaded = app.update.0.lock().expect("lock").take();
    let Some(Downloaded { update, bytes }) = downloaded else {
        return Err(ApiFailure::new(
            "no_update",
            "Es ist kein Update heruntergeladen.",
        ));
    };
    app.with_engine(|e| e.send(Command::Leave));
    tracing::info!(version = %update.version, "installing update");
    update.install(bytes).map_err(|e| {
        tracing::error!(error = %e, "update install failed");
        ApiFailure::new("install_failed", &e.to_string())
    })?;
    handle.restart();
}
