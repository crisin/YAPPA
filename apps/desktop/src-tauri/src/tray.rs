//! The icon in the notification area: yAPPA keeps running there when the window is closed
//! (`OnClose::Tray`, the default), so a reflex click on X never ends a call. Left click
//! brings the window back; the menu shows it, flips mute and deafen, or ends the app.
//!
//! Where the OS gives no tray (or it fails to build), the app degrades: the window closes
//! into nothing, so the close request is then let through — see [`close_requested`].

use crate::{show_window, App};
use client::OnClose;
use engine_protocol::Command;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const SHOW: &str = "show";
const MUTE: &str = "mute";
const DEAFEN: &str = "deafen";
const QUIT: &str = "quit";

/// The menu entries whose check mark follows the engine's `MuteState`.
pub struct Tray {
    mute: CheckMenuItem<tauri::Wry>,
    deafen: CheckMenuItem<tauri::Wry>,
}

impl Tray {
    /// Mute and deafen are the engine's state (A3): the marks only mirror it.
    pub fn set_mute_state(&self, muted: bool, deafened: bool) {
        let _ = self.mute.set_checked(muted);
        let _ = self.deafen.set_checked(deafened);
    }
}

/// Builds the icon and its menu. `None` (with a warning in the log) when the OS refuses:
/// the app then runs without a tray, and closing the window ends it as before.
pub fn build(handle: &AppHandle) -> Option<Tray> {
    match try_build(handle) {
        Ok(tray) => Some(tray),
        Err(e) => {
            tracing::warn!(error = %e, "no tray icon — closing the window ends the app");
            None
        }
    }
}

fn try_build(handle: &AppHandle) -> tauri::Result<Tray> {
    let show = MenuItem::with_id(handle, SHOW, "Fenster zeigen", true, None::<&str>)?;
    let mute = CheckMenuItem::with_id(handle, MUTE, "Stumm", true, false, None::<&str>)?;
    let deafen = CheckMenuItem::with_id(handle, DEAFEN, "Taub", true, false, None::<&str>)?;
    let quit = MenuItem::with_id(handle, QUIT, "Beenden", true, None::<&str>)?;
    let menu = Menu::with_items(
        handle,
        &[
            &show,
            &PredefinedMenuItem::separator(handle)?,
            &mute,
            &deafen,
            &PredefinedMenuItem::separator(handle)?,
            &quit,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("yAPPA")
        .on_menu_event(|handle, event| on_menu(handle, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = handle.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(handle)?;
    Ok(Tray { mute, deafen })
}

fn on_menu(handle: &AppHandle, id: &str) {
    match id {
        SHOW => show_window(handle),
        // The check mark flipped on the click; the engine's `MuteState` sets it to the truth.
        MUTE => send(handle, Command::ToggleMuted),
        DEAFEN => send(handle, Command::ToggleDeafened),
        // `RunEvent::Exit` in lib.rs takes the engine down and leaves the room.
        QUIT => handle.exit(0),
        _ => {}
    }
}

fn send(handle: &AppHandle, command: Command) {
    if let Some(app) = handle.try_state::<App>() {
        app.with_engine(|engine| engine.send(command));
    }
}

/// The X of the window: `true` = handled (hidden into the tray), the close must be
/// prevented; `false` = let it through, the app ends.
pub fn close_requested(window: &tauri::Window) -> bool {
    let Some(app) = window.try_state::<App>() else {
        return false;
    };
    let on_close = app.app_settings.lock().expect("lock").on_close;
    if on_close == OnClose::Quit || app.tray.is_none() {
        return false;
    }
    let _ = window.hide();
    tracing::info!("window closed into the tray; the app keeps running");
    true
}

/// Started by the autostart entry: stay in the tray until clicked.
pub fn started_minimized() -> bool {
    std::env::args().skip(1).any(|arg| arg == MINIMIZED_ARG)
}

pub const MINIMIZED_ARG: &str = "--minimized";
