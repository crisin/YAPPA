//! `cargo run --release -p spike-s4 --bin s4-host [-- <file.clap> [<plugin-id>]]`
//!
//! Without arguments: the built-in test plugin, linked in. With a path: that `.clap` file
//! (foreign code!), its first plugin or the one named. Prints what the host learns and the
//! cost per 10 ms block over 3 000 blocks (30 s of audio).

use spike_s4::host::{measure, Loaded};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("spike-s4: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let loaded = match args.next() {
        None => {
            println!("entry: built-in test plugin (linked in)");
            Loaded::builtin()?
        }
        Some(path) => {
            let path = PathBuf::from(path);
            println!("entry: {}", path.display());
            Loaded::from_path(&path)?
        }
    };
    println!("CLAP version: {}", loaded.clap_version());
    let described = loaded.describe();
    if described.is_empty() {
        return Err("the entry offers no plugins (no plugin factory)".into());
    }
    for d in &described {
        println!("plugin: {} — {} ({})", d.id, d.name, d.vendor);
    }
    let id = args.next().unwrap_or_else(|| described[0].id.clone());
    println!("instantiating {id}");
    let mut instance = loaded.instantiate(&id)?;
    let channels = match instance.ports() {
        Some(ports) => {
            for p in &ports.inputs {
                println!(
                    "input port: {} ({} ch{})",
                    p.name,
                    p.channels,
                    if p.is_main { ", main" } else { "" }
                );
            }
            for p in &ports.outputs {
                println!(
                    "output port: {} ({} ch{})",
                    p.name,
                    p.channels,
                    if p.is_main { ", main" } else { "" }
                );
            }
            ports
                .inputs
                .first()
                .map(|p| p.channels as usize)
                .unwrap_or(1)
                .max(
                    ports
                        .outputs
                        .first()
                        .map(|p| p.channels as usize)
                        .unwrap_or(1),
                )
        }
        None => {
            println!("no audio-ports extension: assuming one mono port each way");
            1
        }
    };
    match instance.latency() {
        Some(samples) => println!("latency: {samples} samples"),
        None => println!("latency: not reported (no latency extension)"),
    }
    let mut running = instance.start(channels)?;
    println!("activated: 48 kHz, 480-frame blocks, {channels} channel(s) per port");
    let m = measure(&mut running, 3_000)?;
    println!(
        "{} blocks: mean {:.2} µs, p99 {:.2} µs, max {:.2} µs per 10 ms block",
        m.blocks,
        m.mean.as_secs_f64() * 1e6,
        m.p99.as_secs_f64() * 1e6,
        m.max.as_secs_f64() * 1e6
    );
    println!(
        "last block: RMS in {:.4}, out {:.4} ({:+.1} dB)",
        m.rms_in,
        m.rms_out,
        20.0 * (m.rms_out / m.rms_in).log10()
    );
    Ok(())
}
