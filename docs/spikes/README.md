# Spikes

Each spike takes 1–3 days and ends with a markdown result here (`s1-poc.md`, …): what was
tried, measurements (latency per block, CPU, loopback-click latency), what worked, what
didn't, and the decision it feeds. Use the same measurements as variant A so the two can be
compared.

| Spike | Goal (variant B) |
| --- | --- |
| S0 | Repo skeleton: Cargo workspace, `cargo xtask check`, Tauri 2 + Svelte 5 shell, CI matrix (windows + macos) |
| S1 | PoC: two clients hear each other via local LiveKit — engine CLI (cpal → NativeAudioSource), RED, bitrate, reconnect; latency via loopback click |
| S2 | DSP chain offline: WAV → denoiser → gate → EQ → compressor → WAV; DeepFilterNet-LL (git dependency) against RNNoise; latency and CPU per block. Gate: DFN stays default only if the chain adds ≤ 25 ms and ≤ 5 ms CPU per 10 ms block |
| S3 | PTT with a fullscreen game on Windows: low-level hook / Raw Input behind `GlobalHotkey` in `crates/platform-windows` (not the Tauri plugin), tray, autostart |
| S4 | optional: clack-host loads a CLAP plugin and passes audio through |
| S6 | Control plane in Rust: axum + sqlx/SQLite, invite login, tokens for voice rooms and the presence room, `GET /config`, LiveKit webhook → event log only (presence lives in LiveKit, ADR-005); runs in the same compose as LiveKit |
| S7 | Session-Radio (phase 3): session tap → local ASR (whisper-class) → topics (local LLM) → songs from a local sidecar; measure topic quality on a gaming evening, GPU time per song, disk; pick the sidecar |

S5 (UI stack analysis) is not run in this variant — ADR-001 decides Svelte 5.
