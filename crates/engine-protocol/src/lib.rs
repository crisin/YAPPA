//! UI <-> audio engine contract.
//!
//! Commands go into the engine, events and levels come out. Everything is plain data
//! (serde + ts-rs), so the engine can live in-process (Tauri) today and in its own process
//! later without the UI noticing. The TypeScript side is generated: `cargo xtask gen-types`.
//!
//! Bump [`PROTOCOL_VERSION`] on every breaking change; additive changes keep it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Version of this contract. The UI refuses to talk to an engine with a different major.
/// 2: `Settings` lost `theme` (ADR-007). UI and engine ship in one binary today, so this
/// guards the day the engine runs as its own process. Changes to the [`Settings`] shape
/// are tracked by [`SETTINGS_VERSION`] alone (the file on disk is what needs migrating).
pub const PROTOCOL_VERSION: u32 = 2;

/// Version of the [`Settings`] schema, stored with the settings file for migrations.
/// 2 (ADR-007): audio and sending only — the theme moved to the app settings
/// (`crates/client`), which also migrates a version-1 file.
/// 3: `voice_bitrate_kbps` became a quality profile ([`QualityProfile`]) plus the codec
/// values behind it ([`VoiceCodec`]); `crates/client` migrates a version-2 file.
pub const SETTINGS_VERSION: u32 = 3;

/// What the shell tells the UI about the running app (`app_info`). Lives here so the UI
/// type is generated, not hand-copied (realtime review 2026-10-23).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    /// 0 = browser mode, no real engine.
    pub protocol_version: u32,
    pub log_dir: String,
    /// Development aid: `YAPPA_DEBUG_PANEL=1` opens the diagnosis at start.
    pub debug_panel: bool,
    /// `YAPPA_PROFILE`, "default" without it — so two windows can be told apart.
    pub profile: String,
}

/// UI -> engine.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Command {
    /// `None` = system default device.
    SetInputDevice {
        id: Option<String>,
    },
    SetOutputDevice {
        id: Option<String>,
    },
    SetTransmitMode {
        mode: TransmitMode,
    },
    /// Microphone off. Unmuting while deafened also undeafens: nobody wants to talk into a
    /// room they cannot hear.
    SetMuted {
        muted: bool,
    },
    /// Hear nothing — and, implied, send nothing. Undeafening gives the microphone back
    /// the state it had before. The engine answers every change with
    /// [`Event::MuteState`].
    SetDeafened {
        deafened: bool,
    },
    /// The mute key (A3) or the dock button: flip the microphone. The engine owns the
    /// state, so a key in a game and a click in the window can never disagree.
    ToggleMuted,
    /// The deafen key or the dock button: flip hearing (and, implied, the microphone).
    ToggleDeafened,
    /// Receive side: per remote user and track.
    SetPeerVolume {
        identity: String,
        track: TrackKind,
        gain_db: f32,
    },
    SetPeerMuted {
        identity: String,
        track: TrackKind,
        muted: bool,
    },
    /// Replaces the whole settings object (simple and pro view write the same schema).
    ApplySettings {
        settings: Settings,
    },
    /// Connect to a LiveKit room with a token from the control plane.
    Join {
        url: String,
        token: String,
    },
    Leave,
    /// Ask for a fresh [`Event::Devices`] and take the chosen device back if it is there
    /// again (A2). The shell sends this on every hot-plug the OS reports; the UI on
    /// „Geräte neu einlesen“.
    RefreshDevices,
    /// The UI (re)attached and missed what happened so far: send the current state again —
    /// ready, devices, connection, room and participants.
    Resync,
    /// The push-to-talk key went down or up. The hook lives in the shell (platform crate,
    /// ADR-007); the engine only opens the send path while it is held, in PTT mode.
    SetPttPressed {
        pressed: bool,
    },
    /// „Mich selbst hören“ (A5): the own send chain's output — after gate, EQ, compressor
    /// and limiter, before the mute — goes into the local playout mix, without the network.
    /// The Signalkette page switches it on while it is open and off when it closes; the
    /// engine answers every change with [`Event::MonitorState`].
    SetMonitor {
        on: bool,
    },
    /// Soundboard (ADR-010): play a sound file. It goes out on the own `soundboard` track
    /// (the others can turn it down per person) and into the own playout mix. Several at
    /// once overlap; a limiter keeps the sum below clipping. The engine answers with
    /// [`Event::Sounds`] when the clip starts and when it ends.
    PlaySound {
        path: String,
    },
    /// Stops every playing sound at once.
    StopSounds,
}

/// Engine -> UI.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Event {
    Ready {
        protocol_version: u32,
    },
    Devices {
        inputs: Vec<AudioDevice>,
        outputs: Vec<AudioDevice>,
    },
    /// Sent at a fixed rate (~30 Hz) while audio runs; never from the audio callback itself.
    Levels {
        input_db: f32,
        gate_open: bool,
        peers: Vec<PeerLevel>,
    },
    Connection {
        state: ConnectionState,
    },
    Error {
        message: String,
    },
    /// We are in a room. Room and identity come from the token.
    Joined {
        room: String,
        identity: String,
    },
    /// The complete list of the others in the room, sent on every change.
    Participants {
        participants: Vec<Participant>,
    },
    /// About once per second while audio runs — for the debug panel and the log.
    Stats {
        stats: Stats,
    },
    /// What the engine and the transport log at info level and above, as it happens.
    Log {
        entry: LogEntry,
    },
    /// The send chain stage by stage, at the rate of [`Event::Levels`] — for the signal
    /// chain view in the pro settings.
    StageLevels {
        stages: Vec<StageLevel>,
    },
    /// Which device is actually open on each side (A2): the chosen one, or the default
    /// standing in while the chosen one is away. Sent when it changes and on `Resync`.
    DeviceState {
        input: DeviceSlot,
        output: DeviceSlot,
    },
    /// Microphone and hearing as the engine has them (A3). `muted` is what is in effect:
    /// true while deafened too. Sent on every change and on `Resync`; the dock and the
    /// presence follow this, never their own guess.
    MuteState {
        muted: bool,
        deafened: bool,
    },
    /// „Mich selbst hören“ as the engine has it (A5). Sent on every change and on `Resync`.
    MonitorState {
        on: bool,
    },
    /// Which of my own sounds are playing right now (paths as given to
    /// [`Command::PlaySound`]). Sent on every change and on `Resync`.
    Sounds {
        playing: Vec<String>,
    },
}

/// One sound in the soundboard folder, as the shell lists it for the UI (the folder is the
/// library — drop a file in, and it is a button).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Sound {
    /// File name without the extension — the button's label.
    pub name: String,
    /// Full path, what [`Command::PlaySound`] takes.
    pub path: String,
}

/// One side of the sound hardware as it stands right now (A2).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceSlot {
    /// Name of the open device; `None` = nothing is open on this side.
    pub open: Option<String>,
    /// The chosen device is away: the system default stands in (or nothing). Cleared by
    /// itself when the chosen device is back and open again.
    pub fallback: bool,
    /// Name of the missing chosen device, when the engine has seen it before.
    pub missing: Option<String>,
}

/// One stage of the send chain as the pro view shows it.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StageLevel {
    /// `input`, `highpass`, `denoiser`, `gate`, `eq`, `compressor`, `limiter`.
    pub id: String,
    /// Level after this stage, dBFS.
    pub level_db: f32,
    /// The stage works (false: switched off, or the gate is closed).
    pub active: bool,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TransmitMode {
    VoiceActivation,
    PushToTalk,
    AlwaysOn,
}

/// The three published tracks per participant (see planning doc, "Quellen und Tracks").
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TrackKind {
    Voice,
    Music,
    Soundboard,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DenoiserKind {
    Off,
    /// RNNoise class (nnnoiseless): little CPU.
    Rnnoise,
    /// DeepFilterNet: best quality, default on gaming PCs.
    DeepFilter,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PeerLevel {
    pub identity: String,
    pub level_db: f32,
    pub speaking: bool,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Participant {
    pub identity: String,
    pub name: String,
    /// We receive a voice track from them (otherwise they are in the room but silent to us).
    pub has_voice: bool,
    /// They muted their own microphone.
    pub muted: bool,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogEntry {
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub time_ms: u64,
    pub level: LogLevel,
    /// Where it came from, e.g. `transport::livekit`.
    pub target: String,
    pub message: String,
}

/// One snapshot of everything worth watching while a call runs. `None` = not known (yet).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Stats {
    pub network: Option<NetworkStats>,
    pub capture: Option<DeviceStats>,
    pub playout: Option<DeviceStats>,
    /// Longest gap between two 10 ms blocks handed to the transport since the last
    /// snapshot — far above 10 means the send side stalled.
    pub send_interval_max_ms: f32,
    /// Capture blocks thrown away because a backlog had built up (total).
    pub send_blocks_dropped: u32,
    /// Received frames thrown away because a speaker's playout buffer was full (total).
    pub playout_frames_dropped: u32,
    /// Times playout jumped forward because a speaker's buffer ran over — their clock is
    /// faster than our output device, or playout stalled (total).
    pub playout_skips: u32,
    /// Work per 10 ms block on the send side (from the ring to the transport, the future
    /// DSP chain included) since the last snapshot: the longest and the mean. The budget for
    /// the whole chain is 5 ms (planning doc); the "before" of every send-chain change.
    #[serde(default)]
    pub pump_busy_max_ms: f32,
    #[serde(default)]
    pub pump_busy_mean_ms: f32,
    /// What the send chain adds to the latency (the denoiser's own delay), ms.
    #[serde(default)]
    pub chain_latency_ms: f32,
    /// The receive-side leveler's current gain per speaker (A4), sorted by identity — a
    /// diagnostics file shows who arrives too quiet or too loud.
    #[serde(default)]
    pub speakers: Vec<SpeakerStats>,
    /// The monitor path (A5) while „Mich selbst hören“ is on; `None` while it is off.
    #[serde(default)]
    pub monitor: Option<MonitorStats>,
}

/// „Mich selbst hören“ in the diagnosis (A5).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorStats {
    /// Microphone to headphone on the local path, ms — estimated from the buffers it passes:
    /// the capture callback, the chain's own delay, the most that sat in the monitor ring
    /// since the last snapshot, and the playout callback. No network, no jitter buffer.
    pub latency_ms: f32,
    /// Times the monitor ran dry while playing (total since start).
    pub underruns: u32,
    /// Chain blocks that did not fit into the monitor ring (total since start) — the
    /// output callback is not draining it.
    pub dropped_blocks: u32,
}

/// One remote speaker in the diagnosis.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpeakerStats {
    pub identity: String,
    /// What the leveler adds to this voice right now, dB (−12..12; 0 while it is off).
    pub leveler_db: f32,
    /// How much faster (> 1) or slower (< 1) than real time this voice is played to hold
    /// its buffer at the cushion (A6): their clock against our output device, within
    /// 1 ± 0.005. 1.0 while they are silent.
    #[serde(default = "unity")]
    pub drift_ratio: f32,
    /// How much of this voice is queued for playout, ms, averaged over ≈ 1.5 s. Held at
    /// the cushion (≈ 25 ms for a 10 ms callback); 0 while they are silent.
    #[serde(default)]
    pub playout_fill_ms: f32,
}

fn unity() -> f32 {
    1.0
}

/// From the transport's WebRTC statistics. "Recent" values cover the time since the
/// previous snapshot, the others are totals for this connection.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkStats {
    /// Round trip to the media server (ICE connectivity checks).
    pub rtt_ms: f32,
    pub send_codec: String,
    pub send_kbps_recent: f32,
    pub packets_sent: u32,
    /// Share of our packets the server reported missing, 0..100.
    pub uplink_loss_percent: f32,
    pub packets_received: u32,
    pub packets_lost: u32,
    pub loss_percent_recent: f32,
    pub jitter_ms: f32,
    /// Mean time a sample waited in the jitter buffer.
    pub jitter_buffer_ms: f32,
    /// Share of played samples the decoder had to invent — what loss sounds like.
    pub concealed_percent_recent: f32,
    pub concealed_percent_total: f32,
    /// What the encoder is aiming at right now (WebRTC `targetBitrate`), 0 = not reported.
    /// This is the negotiated figure to compare against the profile's bitrate.
    #[serde(default)]
    pub send_target_kbps: f32,
    /// Recent send rate without RTP headers: Opus payload (and RED copies) only.
    #[serde(default)]
    pub send_payload_kbps_recent: f32,
    /// Channels of the published voice track: 1 or 2 (0 = none published).
    #[serde(default)]
    pub send_channels: u16,
}

/// The Opus parameters the transport can set today (LiveKit Rust SDK 0.9). Frame length
/// (20 ms), in-band FEC (on) and complexity (9) are fixed inside libwebrtc and reach the UI
/// as [`OPUS_FIXED`], so the pro view can show them as facts, not knobs.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VoiceCodec {
    /// Upper limit for the encoder; Opus stays variable below it. 16..=510.
    pub bitrate_kbps: u32,
    /// Publish two channels. The microphone path stays mono; the transport sends the same
    /// signal on both channels, so this measures the cost of stereo, not a stereo mic.
    pub stereo: bool,
    /// Redundant audio: each packet carries the previous frame again (RFC 2198). libwebrtc
    /// drops the copy silently once primary + copy would exceed 1 kB — with 20 ms frames
    /// that is above about 200 kbit/s (measured: `docs/spikes/quality-tiers.md`).
    pub red: bool,
    /// Stop sending during silence. Off: quality over thrift.
    pub dtx: bool,
}

/// Opus parameters libwebrtc decides and the SDK does not expose. Reported, not settable.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpusFixed {
    pub frame_ms: u32,
    pub inband_fec: bool,
    pub complexity: u8,
}

pub const OPUS_FIXED: OpusFixed = OpusFixed {
    frame_ms: 20,
    inband_fec: true,
    complexity: 9,
};

/// Voice quality as the simple view offers it. Every named profile stands for one
/// [`VoiceCodec`] ([`QualityProfile::preset`]); `Custom` uses [`Settings::voice`] as set in
/// the pro view.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum QualityProfile {
    /// „Sparsam“: 32 kbit/s mono, for a weak upload.
    Thrifty,
    /// „Standard“: 80 kbit/s mono with RED — the first evenings ran with this.
    Standard,
    /// „Hoch“: 128 kbit/s mono.
    High,
    /// „Studio“: 192 kbit/s mono — the highest step at which RED still fits into the packet.
    Studio,
    /// „Stress“: Opus's maximum asked for, 510 kbit/s stereo, no RED (the copies would not
    /// fit anyway). libwebrtc caps the encoder at about 256 kbit/s; the diagnosis shows
    /// what it really aims at. To compare against Discord and to find the ceiling.
    Stress,
    /// The values in [`Settings::voice`].
    Custom,
}

impl QualityProfile {
    /// The named profiles, in the order the UI lists them.
    pub const NAMED: [QualityProfile; 5] = [
        QualityProfile::Thrifty,
        QualityProfile::Standard,
        QualityProfile::High,
        QualityProfile::Studio,
        QualityProfile::Stress,
    ];

    /// The codec values of a named profile; `None` for `Custom`.
    pub fn preset(self) -> Option<VoiceCodec> {
        let codec = |bitrate_kbps, stereo, red| VoiceCodec {
            bitrate_kbps,
            stereo,
            red,
            dtx: false,
        };
        Some(match self {
            QualityProfile::Thrifty => codec(32, false, true),
            QualityProfile::Standard => codec(80, false, true),
            QualityProfile::High => codec(128, false, true),
            QualityProfile::Studio => codec(192, false, true),
            QualityProfile::Stress => codec(510, true, false),
            QualityProfile::Custom => return None,
        })
    }

    /// The named profile with exactly these values, else `Custom`.
    pub fn matching(codec: VoiceCodec) -> QualityProfile {
        Self::NAMED
            .into_iter()
            .find(|p| p.preset() == Some(codec))
            .unwrap_or(QualityProfile::Custom)
    }
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceStats {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Largest callback buffer seen, in frames — the effective device buffer.
    pub callback_frames: u32,
    /// Capture: samples lost because the engine did not keep up. Playout: times a
    /// speaker's buffer ran dry.
    pub xruns: u32,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqBand {
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompressorSettings {
    pub enabled: bool,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
}

/// Everything about audio and sending the user can set (ADR-007: no UI preferences here —
/// those are the app settings). Simple and pro view both read and write this.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    pub version: u32,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub transmit_mode: TransmitMode,
    /// Voice activation threshold, dBFS.
    pub vad_threshold_db: f32,
    pub vad_hold_ms: u32,
    pub denoiser: DenoiserKind,
    pub high_pass_hz: f32,
    pub eq: Vec<EqBand>,
    pub compressor: CompressorSettings,
    /// Voice quality: a named profile, or `Custom` for the values in `voice`.
    pub quality: QualityProfile,
    /// The codec values the pro view edits. Only in effect while `quality` is `Custom`;
    /// [`Settings::voice_codec`] is what is actually sent.
    pub voice: VoiceCodec,
    /// Which input channel of the microphone device carries the voice (0 = first). A mic on
    /// input 2 of a two-channel interface is channel 1. Clamped to what the device has.
    #[serde(default)]
    pub input_channel: u16,
    /// Master volume of everything I hear, dB; a limiter keeps the sum below clipping.
    #[serde(default)]
    pub output_gain_db: f32,
    /// Everybody at about the same loudness (A4): a leveler per speaker pulls quiet voices
    /// up and loud ones down (±12 dB, a few dB per second) before the per-person volume.
    /// Off, only the per-person volume applies.
    #[serde(default = "yes")]
    pub auto_level: bool,
    /// How loud my soundboard sounds go out and play back for me, dB (−30..=12) — the
    /// files come in all loudnesses; this is the one knob before the limiter.
    #[serde(default)]
    pub soundboard_gain_db: f32,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            input_device: None,
            output_device: None,
            transmit_mode: TransmitMode::VoiceActivation,
            vad_threshold_db: -50.0,
            vad_hold_ms: 300,
            denoiser: DenoiserKind::DeepFilter,
            high_pass_hz: 80.0,
            eq: Vec::new(),
            compressor: CompressorSettings {
                enabled: true,
                threshold_db: -18.0,
                ratio: 3.0,
                attack_ms: 5.0,
                release_ms: 120.0,
                makeup_db: 3.0,
            },
            quality: QualityProfile::Standard,
            voice: QualityProfile::Standard.preset().unwrap(),
            input_channel: 0,
            output_gain_db: 0.0,
            auto_level: true,
            soundboard_gain_db: 0.0,
        }
    }
}

impl Settings {
    /// What the voice track is published with: the profile's preset, or `voice` for
    /// `Custom`.
    pub fn voice_codec(&self) -> VoiceCodec {
        self.quality.preset().unwrap_or(self.voice)
    }

    /// Sanity limits the engine relies on. Returns the first problem found.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != SETTINGS_VERSION {
            return Err(format!(
                "settings version {} != {SETTINGS_VERSION}",
                self.version
            ));
        }
        if !(16..=510).contains(&self.voice.bitrate_kbps) {
            return Err(format!(
                "voice bitrate {} kbps out of 16..=510",
                self.voice.bitrate_kbps
            ));
        }
        if !(-90.0..=0.0).contains(&self.vad_threshold_db) {
            return Err(format!(
                "vad threshold {} dBFS out of -90..=0",
                self.vad_threshold_db
            ));
        }
        if !(-30.0..=12.0).contains(&self.output_gain_db) {
            return Err(format!(
                "output gain {} dB out of -30..=12",
                self.output_gain_db
            ));
        }
        if !(-30.0..=12.0).contains(&self.soundboard_gain_db) {
            return Err(format!(
                "soundboard gain {} dB out of -30..=12",
                self.soundboard_gain_db
            ));
        }
        if self.input_channel > 31 {
            return Err(format!(
                "input channel {} out of 0..=31",
                self.input_channel
            ));
        }
        if self.eq.len() > 5 {
            return Err(format!("{} EQ bands, at most 5", self.eq.len()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_internally_tagged_camel_case() {
        let json = serde_json::to_value(Command::SetPeerVolume {
            identity: "u1".into(),
            track: TrackKind::Soundboard,
            gain_db: -6.0,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "setPeerVolume", "identity": "u1", "track": "soundboard", "gainDb": -6.0
            })
        );
    }

    #[test]
    fn unit_commands_and_the_mute_state_event_keep_the_tag_shape() {
        assert_eq!(
            serde_json::to_value(Command::ToggleDeafened).unwrap(),
            serde_json::json!({ "type": "toggleDeafened" })
        );
        let ev = Event::MuteState {
            muted: true,
            deafened: false,
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "type": "muteState", "muted": true, "deafened": false })
        );
        let back: Event = serde_json::from_value(json).unwrap();
        assert_eq!(ev, back);
    }

    #[test]
    fn the_monitor_command_and_event_keep_the_tag_shape() {
        assert_eq!(
            serde_json::to_value(Command::SetMonitor { on: true }).unwrap(),
            serde_json::json!({ "type": "setMonitor", "on": true })
        );
        let ev = Event::MonitorState { on: false };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "type": "monitorState", "on": false })
        );
        let back: Event = serde_json::from_value(json).unwrap();
        assert_eq!(ev, back);
        // Older stats without the field still load.
        let stats: Stats = serde_json::from_value(serde_json::json!({
            "network": null, "capture": null, "playout": null, "sendIntervalMaxMs": 0.0,
            "sendBlocksDropped": 0, "playoutFramesDropped": 0, "playoutSkips": 0
        }))
        .unwrap();
        assert_eq!(stats.monitor, None);
    }

    #[test]
    fn the_soundboard_commands_and_event_keep_the_tag_shape() {
        assert_eq!(
            serde_json::to_value(Command::PlaySound {
                path: "C:/sounds/airhorn.mp3".into()
            })
            .unwrap(),
            serde_json::json!({ "type": "playSound", "path": "C:/sounds/airhorn.mp3" })
        );
        assert_eq!(
            serde_json::to_value(Command::StopSounds).unwrap(),
            serde_json::json!({ "type": "stopSounds" })
        );
        let ev = Event::Sounds {
            playing: vec!["a.wav".into()],
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "type": "sounds", "playing": ["a.wav"] })
        );
        let back: Event = serde_json::from_value(json).unwrap();
        assert_eq!(ev, back);
        // A settings file from before the soundboard still loads, at 0 dB.
        let mut json = serde_json::to_value(Settings::default()).unwrap();
        json.as_object_mut().unwrap().remove("soundboardGainDb");
        let old: Settings = serde_json::from_value(json).unwrap();
        assert_eq!(old.soundboard_gain_db, 0.0);
        assert!(Settings {
            soundboard_gain_db: 20.0,
            ..Settings::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn events_round_trip() {
        let ev = Event::Levels {
            input_db: -23.5,
            gate_open: true,
            peers: vec![PeerLevel {
                identity: "u2".into(),
                level_db: -40.0,
                speaking: false,
            }],
        };
        let back: Event = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(ev, back);
    }

    #[test]
    fn stats_use_null_for_unknown_and_camel_case() {
        let json = serde_json::to_value(Event::Stats {
            stats: Stats {
                send_interval_max_ms: 10.5,
                ..Stats::default()
            },
        })
        .unwrap();
        assert_eq!(json["type"], "stats");
        assert_eq!(json["stats"]["network"], serde_json::Value::Null);
        assert_eq!(json["stats"]["sendIntervalMaxMs"], 10.5);
    }

    #[test]
    fn default_settings_are_valid_and_round_trip() {
        let s = Settings::default();
        s.validate().unwrap();
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s, back);
    }

    /// Writes `Settings::default()` next to the generated TS types, so the UI uses the real
    /// defaults instead of a hand-copied mock (runs with `cargo xtask gen-types` / tests).
    #[test]
    fn export_default_settings_json() {
        let Some(dir) = std::env::var_os("TS_RS_EXPORT_DIR") else {
            return;
        };
        let json = serde_json::to_string_pretty(&Settings::default()).unwrap()
            + "
";
        let path = std::path::Path::new(&dir).join("defaultSettings.json");
        if std::fs::read_to_string(&path).ok().as_deref() != Some(json.as_str()) {
            std::fs::write(path, json).unwrap();
        }
    }

    /// The named profiles next to the defaults, so the UI shows the same numbers the engine
    /// sends and the fixed Opus facts come from one place.
    #[test]
    fn export_quality_profiles_json() {
        let Some(dir) = std::env::var_os("TS_RS_EXPORT_DIR") else {
            return;
        };
        let profiles: Vec<serde_json::Value> = QualityProfile::NAMED
            .into_iter()
            .map(|p| {
                serde_json::json!({
                    "id": p,
                    "codec": p.preset().unwrap(),
                })
            })
            .collect();
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "profiles": profiles,
            "fixed": OPUS_FIXED,
        }))
        .unwrap()
            + "\n";
        let path = std::path::Path::new(&dir).join("qualityProfiles.json");
        if std::fs::read_to_string(&path).ok().as_deref() != Some(json.as_str()) {
            std::fs::write(path, json).unwrap();
        }
    }

    #[test]
    fn validate_rejects_out_of_range_bitrate() {
        let s = Settings {
            quality: QualityProfile::Custom,
            voice: VoiceCodec {
                bitrate_kbps: 4,
                ..Settings::default().voice
            },
            ..Settings::default()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn named_profiles_win_over_the_custom_values_and_are_found_again() {
        let s = Settings {
            quality: QualityProfile::Stress,
            voice: VoiceCodec {
                bitrate_kbps: 16,
                stereo: false,
                red: true,
                dtx: true,
            },
            ..Settings::default()
        };
        let sent = s.voice_codec();
        assert_eq!(sent.bitrate_kbps, 510);
        assert!(sent.stereo && !sent.red && !sent.dtx);
        for p in QualityProfile::NAMED {
            assert_eq!(QualityProfile::matching(p.preset().unwrap()), p);
            assert!((16..=510).contains(&p.preset().unwrap().bitrate_kbps));
        }
        assert_eq!(
            QualityProfile::matching(s.voice),
            QualityProfile::Custom,
            "16 kbit/s with DTX is nobody's preset"
        );
        let custom = Settings {
            quality: QualityProfile::Custom,
            ..s
        };
        assert_eq!(custom.voice_codec(), custom.voice);
    }
}
