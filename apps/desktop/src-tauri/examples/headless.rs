//! The engine without a window: the same engine, transport, devices and client logic as the
//! app, events printed as JSON lines. For measuring, for debugging on a machine without a
//! display, and for checking a server before the UI is involved.
//!
//!   # like the app: log in with an invite link once, then join a channel
//!   cargo run -p yappa-desktop --example headless -- --login <yappa://invite/…> --name Bot
//!   cargo run -p yappa-desktop --example headless -- --channel lobby [--seconds 20]
//!
//!   # without a control plane (developer invites from `cargo xtask livekit token`)
//!   cargo run -p yappa-desktop --example headless -- --invite <yappa1.…>
//!   cargo run -p yappa-desktop --example headless -- --url ws://localhost:7880 --token <jwt>
//!
//! The login lives in `target/headless-profile` (`--profile <name>` for more than one).
//! Levels are printed twice per second instead of thirty times. Logs go to stderr and to
//! `target/headless-logs/`.

use api_types::{ClientInfo, Invite};
use client::{Client, Profile};
use engine_protocol::{Command, Event, Settings, PROTOCOL_VERSION};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::time::Duration;

const USAGE: &str = "usage: headless --login <link> --name <name> | --channel <id> \
                     | --invite <text> | --url <ws://…> --token <jwt>  [--seconds N] [--profile P]";

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| {
        let i = args.iter().position(|a| a == name)?;
        args.get(i + 1).cloned()
    };
    let seconds: u64 = value("--seconds")
        .map(|s| s.parse().map_err(|_| "--seconds: not a number"))
        .transpose()?
        .unwrap_or(20);
    let _logging = yappa_desktop_lib::logging::init("target/headless-logs".as_ref())?;

    let client = Client::new(
        Profile::new(
            "target/headless-profile".as_ref(),
            value("--profile").as_deref(),
        ),
        ClientInfo {
            app_version: env!("CARGO_PKG_VERSION").into(),
            protocol_version: PROTOCOL_VERSION,
            platform: format!("{}-headless", std::env::consts::OS),
        },
    );
    use tauri::async_runtime::block_on;

    let invite = match (
        value("--login"),
        value("--channel"),
        value("--invite"),
        value("--url"),
        value("--token"),
    ) {
        (Some(link), _, _, _, _) => {
            let name = value("--name").ok_or(USAGE)?;
            let session = block_on(client.redeem(&link, &name)).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::json!({ "type": "loggedIn", "user": session.user })
            );
            let channels = block_on(client.channels()).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::json!({ "type": "channels", "channels": channels })
            );
            return Ok(());
        }
        (None, Some(channel), _, _, _) => {
            let token = block_on(client.voice_token(&channel)).map_err(|e| e.to_string())?;
            Invite {
                url: token.ws_url,
                token: token.token,
            }
        }
        (None, None, Some(text), _, _) => Invite::decode(&text).map_err(|e| e.to_string())?,
        (None, None, None, Some(url), Some(token)) => Invite { url, token },
        _ => return Err(USAGE.into()),
    };

    let levels = AtomicU32::new(0);
    let engine = yappa_desktop_lib::start_engine(Settings::default(), move |event| {
        if matches!(event, Event::Levels { .. }) && !levels.fetch_add(1, Relaxed).is_multiple_of(15)
        {
            return;
        }
        println!("{}", serde_json::to_string(&event).unwrap_or_default());
    })
    .map_err(|e| e.to_string())?;

    engine.send(Command::Join {
        url: invite.url,
        token: invite.token,
    });
    std::thread::sleep(Duration::from_secs(seconds));
    engine.send(Command::Leave);
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}
