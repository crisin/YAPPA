//! `yappa-control` — the control plane, and the bootstrap/emergency tool on the box.
//!
//!   yappa-control [serve]                         run the API (default)
//!   yappa-control invite [--admin] [--days N]     print a new invite link
//!   yappa-control policy --min <version> [--kill <version>]…
//!                                                 set the minimum version and kill list
//!
//! Configuration comes from the environment (`infra/livekit/.env`): `LIVEKIT_API_KEY`,
//! `LIVEKIT_API_SECRET`, `LIVEKIT_WS_URL`, optional `LIVEKIT_HTTP_URL`, `YAPPA_LISTEN`,
//! `YAPPA_DATABASE_URL`, `YAPPA_UPDATE_BASE_URL`. `invite` and `policy` only need the
//! database.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let json = std::env::var_os("YAPPA_LOG_JSON").is_some();
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,sqlx=warn".into());
    if json {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None | Some("serve") => control::cli::serve().await,
        Some("invite") => control::cli::invite(&args[1..]).await,
        Some("policy") => control::cli::policy(&args[1..]).await,
        Some("-h" | "--help" | "help") => {
            println!("{}", control::cli::USAGE);
            Ok(())
        }
        Some(other) => Err(format!(
            "unknown command {other}\n\n{}",
            control::cli::USAGE
        )),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("yappa-control: {e}");
            ExitCode::FAILURE
        }
    }
}
