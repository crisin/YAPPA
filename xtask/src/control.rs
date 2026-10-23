//! The control plane against the local LiveKit — and `local`, the whole server stack for
//! development in one command.
//!
//!   cargo xtask local [--lan]              LiveKit (Docker) + control plane; the first start
//!                                          prints an admin invite link
//!   cargo xtask control [--lan] [args]     only the control plane; arguments go to
//!                                          `yappa-control`: `invite [--admin]`,
//!                                          `channel add <name>`, `policy --min 0.2.0`
//!
//! Dev keys, database in `target/control-dev.db`, port 8090 (where the dev LiveKit sends its
//! webhooks). Without `--lan` everything is `localhost` — this PC only. With `--lan` the
//! links and the media address use this PC's LAN address, so a second PC at home can join
//! (Windows asks once to allow `yappa-control` in the firewall: allow for private networks).

use crate::Result;
use std::net::{IpAddr, UdpSocket};
use std::path::Path;
use std::process::Command;

/// This PC's address in the LAN: the interface the OS would use to reach the internet. A
/// UDP "connect" sends nothing, it only picks the route.
fn lan_ip() -> Result<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket
        .connect("192.0.2.1:9") // TEST-NET-1: never reached, only routed
        .map_err(|e| format!("no network route to find the LAN address: {e}"))?;
    let ip = socket.local_addr().map_err(|e| e.to_string())?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return Err("could not find a LAN address — is this PC on a network?".into());
    }
    Ok(ip)
}

fn split_lan(args: &[String]) -> (bool, Vec<String>) {
    let lan = args.iter().any(|a| a == "--lan");
    (
        lan,
        args.iter().filter(|a| *a != "--lan").cloned().collect(),
    )
}

pub fn run(root: &Path, args: &[String]) -> Result {
    let (lan, rest) = split_lan(args);
    let ip = if lan { Some(lan_ip()?) } else { None };
    control(root, &rest, ip)
}

/// `cargo xtask local [--lan]`.
pub fn local(root: &Path, args: &[String]) -> Result {
    let (lan, _) = split_lan(args);
    let ip = if lan { Some(lan_ip()?) } else { None };
    if let Some(ip) = ip {
        // dev.yml announces this address to the clients as the media address.
        std::env::set_var("LIVEKIT_NODE_IP", ip.to_string());
    }
    crate::livekit::run(root, &["up".to_string()])?;
    let host = ip.map_or("localhost".to_string(), |ip| ip.to_string());
    println!(
        "\nyAPPA local server{}\n\
         \x20 LiveKit       ws://{host}:7880   (dev keys — never expose this to the internet)\n\
         \x20 control plane http://{host}:8090\n\
         \x20 database      target/control-dev.db\n\n\
         The first start prints an admin invite link below (\"no members yet\"). Later starts\n\
         do not — then: cargo xtask control{} invite --admin\n\
         Channels:       cargo xtask control{} channel add \"Raid-Abend\"\n\
         Stop:           Ctrl-C here, then cargo xtask livekit down\n",
        if lan { " (LAN)" } else { "" },
        if lan { " --lan" } else { "" },
        if lan { " --lan" } else { "" },
    );
    if lan {
        println!(
            "LAN: other PCs at home open the link; Windows asks once to allow yappa-control\n\
             in the firewall — allow it for private networks.\n"
        );
    }
    control(root, &[], ip)
}

fn control(root: &Path, args: &[String], lan: Option<IpAddr>) -> Result {
    let db = root.join("target").join("control-dev.db");
    let host = lan.map_or("127.0.0.1".to_string(), |ip| ip.to_string());
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
        .env("LIVEKIT_WS_URL", format!("ws://{host}:7880"))
        .env("LIVEKIT_HTTP_URL", "http://localhost:7880")
        .env("YAPPA_PUBLIC_URL", format!("http://{host}:8090"))
        .env(
            "YAPPA_LISTEN",
            if lan.is_some() {
                "0.0.0.0:8090"
            } else {
                "127.0.0.1:8090"
            },
        )
        .env(
            "YAPPA_DATABASE_URL",
            format!("sqlite://{}", db.display().to_string().replace('\\', "/")),
        );
    crate::run(&mut cmd)
}

#[cfg(test)]
mod tests {
    use super::split_lan;

    #[test]
    fn lan_is_a_flag_for_xtask_not_for_yappa_control() {
        let args = [
            "--lan".to_string(),
            "invite".to_string(),
            "--admin".to_string(),
        ];
        let (lan, rest) = split_lan(&args);
        assert!(lan);
        assert_eq!(rest, ["invite", "--admin"]);
    }
}
