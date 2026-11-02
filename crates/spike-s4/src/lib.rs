//! S4 spike: can `clack-host` load a CLAP plugin and pass the send chain's 10 ms blocks
//! through it, and what does that cost? Scratch code — the result is `docs/spikes/s4-clap.md`.
//!
//! Two halves:
//! * [`plugin`]: a minimal CLAP effect written with `clack-plugin` (mono in, mono out, a fixed
//!   gain), exported from this crate's `cdylib` so the host half can load it like any
//!   third-party `.clap` from disk — and linked in directly for the tests.
//! * [`host`]: the host side the engine's effect slot (ADR-012) would need: load an entry,
//!   list its plugins, instantiate one, query its audio ports and latency, activate it for
//!   48 kHz / 480-frame blocks, and process block after block on one thread.
//!
//! Loading foreign code is `unsafe` by nature; it is confined to [`host::Loaded::from_path`].

pub mod host;
pub mod plugin;

/// Block size of the send chain: 10 ms at 48 kHz.
pub const BLOCK: usize = 480;
pub const SAMPLE_RATE: f64 = 48_000.0;
