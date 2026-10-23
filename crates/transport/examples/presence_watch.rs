//! Watches the presence room and prints every change as a line — for debugging "why does X
//! show in the wrong channel". Joins as a hidden participant, so nobody sees the watcher.
//!
//!   cargo xtask livekit presence           (signs the token, runs this)
//!   cargo run -p transport --features livekit --example presence_watch -- <url> <token>

use std::time::Instant;
use transport::presence::{PresenceEvent, PresenceLink};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(url), Some(token)) = (args.first(), args.get(1)) else {
        eprintln!("usage: presence_watch <url> <token>");
        std::process::exit(2);
    };
    let start = Instant::now();
    let _link = PresenceLink::start(
        url,
        token,
        Box::new(move |event| {
            let t = start.elapsed().as_secs_f32();
            match event {
                PresenceEvent::Snapshot { peers, .. } => {
                    println!("{t:8.2}s snapshot: {} online", peers.len());
                    for p in peers {
                        println!("           {} ({}) {:?}", p.name, p.identity, p.attributes);
                    }
                }
                PresenceEvent::Upsert(p) => {
                    println!("{t:8.2}s {} ({}) {:?}", p.name, p.identity, p.attributes);
                }
                PresenceEvent::Left { identity } => println!("{t:8.2}s left: {identity}"),
                PresenceEvent::State(state) => println!("{t:8.2}s link: {state:?}"),
                PresenceEvent::Failed { message, retryable } => {
                    println!("{t:8.2}s failed (retry: {retryable}): {message}");
                }
            }
        }),
    )
    .expect("start the presence link");
    println!("watching the presence room — Ctrl-C to stop");
    loop {
        std::thread::park();
    }
}
