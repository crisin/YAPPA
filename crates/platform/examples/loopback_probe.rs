//! Probe: `cargo run -p platform --example loopback_probe [pid]` lists the apps and
//! captures one for two seconds, printing how much arrived.
fn main() {
    let pid: Option<u32> = std::env::args().nth(1).and_then(|a| a.parse().ok());
    println!("supported: {}", platform::appaudio::supported());
    match platform::appaudio::list_apps() {
        Ok(apps) => {
            for a in &apps {
                println!(
                    "  {:>6}  {:<24} {}",
                    a.pid,
                    a.name,
                    if a.active { "active" } else { "" }
                );
            }
        }
        Err(e) => println!("list failed: {e}"),
    }
    let Some(pid) = pid else { return };
    let frames = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let peak = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let (f, p) = (frames.clone(), peak.clone());
    let running = platform::ProcessLoopback::start(
        pid,
        move |samples, _rate, channels| {
            f.fetch_add(
                samples.len() / channels as usize,
                std::sync::atomic::Ordering::Relaxed,
            );
            let m = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
            p.fetch_max((m * 1000.0) as u32, std::sync::atomic::Ordering::Relaxed);
        },
        |reason| println!("ended: {reason}"),
    );
    match running {
        Ok(running) => {
            std::thread::sleep(std::time::Duration::from_secs(2));
            drop(running);
            println!(
                "captured {} frames, peak {:.3}",
                frames.load(std::sync::atomic::Ordering::Relaxed),
                peak.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0
            );
        }
        Err(e) => println!("start failed: {e}"),
    }
}
