//! `cargo xtask control [args]` — the control plane against the local LiveKit
//! (`cargo xtask livekit`): dev keys, database in `target/control-dev.db`, port 8090 (where
//! the dev LiveKit sends its webhooks). Arguments go to `yappa-control`:
//!
//!   cargo xtask control                    serve; the first start prints an admin invite
//!   cargo xtask control invite [--admin]   another invite link
//!   cargo xtask control policy --min 0.2.0 minimum version / kill switch

use crate::Result;
use std::path::Path;
use std::process::Command;

pub fn run(root: &Path, args: &[String]) -> Result {
    let db = root.join("target").join("control-dev.db");
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(root)
        .args([
            "run",
            "--quiet",
            "-p",
            "control",
            "--bin",
            "yappa-control",
            "--",
        ])
        .args(args)
        .env("LIVEKIT_API_KEY", "devkey")
        .env("LIVEKIT_API_SECRET", "secret")
        .env("LIVEKIT_WS_URL", "ws://localhost:7880")
        .env("YAPPA_LISTEN", "127.0.0.1:8090")
        .env(
            "YAPPA_DATABASE_URL",
            format!("sqlite://{}", db.display().to_string().replace('\\', "/")),
        );
    crate::run(&mut cmd)
}
