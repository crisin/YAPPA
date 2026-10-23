//! The control thread: takes [`Command`]s, drives devices and transport, emits [`Event`]s.
//!
//! Everything here may block, allocate and log — the real-time work happens in the device
//! callbacks, which this thread only talks to through rings and atomics.

use crate::chain::{Chain, ChainConfig, STAGES};
use crate::device::{AudioIo, OpenDevice};
use crate::mixer::{mixer, MixerControl, MixerRt};
use crate::stats::network_stats;
use dsp::{db_to_gain, gain_to_db};
use engine_protocol::{
    Command, ConnectionState, DeviceStats, Event, NetworkStats, Participant, PeerLevel, Settings,
    StageLevel, Stats, TrackKind, PROTOCOL_VERSION,
};
use rtrb::{Consumer, RingBuffer};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use transport::{
    AudioReceiver, FrameSink, LinkState, NetStats, PublishOptions, Transport, TransportEvent,
    VoiceOut, BLOCK,
};

/// How often levels go to the UI.
const LEVEL_INTERVAL: Duration = Duration::from_millis(33);
const STATS_INTERVAL: Duration = Duration::from_secs(1);
/// Capture ring between the device callback and the pump: 100 ms.
const CAPTURE_RING: usize = 10 * BLOCK;
/// A remote voice counts as speaking above this level, and for a short while after.
const SPEAKING_DB: f32 = -45.0;
const SPEAKING_HOLD: Duration = Duration::from_millis(250);
/// Per-person volume range offered to the UI.
const PEER_GAIN_DB: std::ops::RangeInclusive<f32> = -60.0..=12.0;

#[derive(Debug, Clone, Copy)]
pub struct EngineConfig {
    /// Pause before joining again after an established session was lost.
    pub rejoin_delay: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            rejoin_delay: Duration::from_secs(3),
        }
    }
}

enum Msg {
    Command(Command),
    Transport(TransportEvent),
    Shutdown,
}

/// The running engine. Dropping it stops the threads and closes the devices.
pub struct EngineHandle {
    tx: mpsc::Sender<Msg>,
    settings: Arc<Mutex<Settings>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl EngineHandle {
    pub fn send(&self, command: Command) {
        let _ = self.tx.send(Msg::Command(command));
    }

    /// The settings as the engine currently has them (for saving).
    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Starts the engine. `on_event` is called from the control thread.
pub fn spawn(
    mut transport: Box<dyn Transport>,
    io: Box<dyn AudioIo>,
    settings: Settings,
    config: EngineConfig,
    on_event: impl Fn(Event) + Send + 'static,
) -> EngineHandle {
    let (tx, rx) = mpsc::channel();
    let (mixer, mixer_rt) = mixer();
    mixer.set_master_db(settings.output_gain_db);
    let events = tx.clone();
    transport.attach(
        Box::new(move |event| {
            let _ = events.send(Msg::Transport(event));
        }),
        Arc::new(Speakers(mixer.clone())),
    );

    let pump = Arc::new(PumpShared::default());
    let (ring_tx, ring_rx) = mpsc::channel();
    let (chain_tx, chain_rx) = mpsc::channel();
    let pump_thread = {
        let (pump, voice) = (pump.clone(), transport.voice_out());
        let first = ChainConfig::from(&settings);
        std::thread::Builder::new()
            .name("yappa-pump".into())
            .spawn(move || run_pump(&pump, ring_rx, chain_rx, first, voice.as_ref()))
            .expect("spawn the pump thread")
    };

    let settings = Arc::new(Mutex::new(settings));
    let (mixer_back, mixer_returned) = mpsc::channel();
    let mut engine = Engine {
        transport,
        io,
        on_event: Box::new(on_event),
        pending_errors: RefCell::new(BTreeMap::new()),
        settings: settings.clone(),
        config,
        mixer,
        mixer_rt: Some(mixer_rt),
        mixer_back,
        mixer_returned,
        capture: None,
        playout: None,
        pump,
        ring_tx,
        chain_tx,
        state: ConnectionState::Disconnected,
        wanted: None,
        was_joined: false,
        rejoin_at: None,
        rejoin_attempts: 0,
        room: None,
        participants: BTreeMap::new(),
        last_loud: HashMap::new(),
        net_previous: None,
        net: None,
    };
    let thread = std::thread::Builder::new()
        .name("yappa-engine".into())
        .spawn(move || {
            engine.run(&rx);
            engine.shutdown();
            let _ = pump_thread.join();
        })
        .expect("spawn the engine thread");
    EngineHandle {
        tx,
        settings,
        thread: Some(thread),
    }
}

/// Where an error came from, so a later success from the same source resolves it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ErrorSource {
    Settings,
    Microphone,
    Output,
    Room,
}

struct Engine {
    transport: Box<dyn Transport>,
    io: Box<dyn AudioIo>,
    on_event: Box<dyn Fn(Event) + Send>,
    /// Errors that still stand, by source — for a UI that attaches after they happened
    /// (no microphone at start: the message went out before the window listened). A later
    /// success from the same source resolves its entry; `Resync` hands the rest over once.
    pending_errors: RefCell<BTreeMap<ErrorSource, String>>,
    settings: Arc<Mutex<Settings>>,
    config: EngineConfig,

    mixer: Arc<MixerControl>,
    /// Here while no playout stream owns it.
    mixer_rt: Option<MixerRt>,
    mixer_back: mpsc::Sender<MixerRt>,
    mixer_returned: mpsc::Receiver<MixerRt>,
    capture: Option<OpenDevice>,
    playout: Option<OpenDevice>,
    pump: Arc<PumpShared>,
    ring_tx: mpsc::Sender<Consumer<f32>>,
    /// New send-chain settings for the pump.
    chain_tx: mpsc::Sender<ChainConfig>,

    state: ConnectionState,
    /// Where the user wants to be: url and token of the last `Join`, until `Leave`.
    wanted: Option<(String, String)>,
    /// The current `wanted` session was established at least once — only then a loss is
    /// answered with a rejoin (a token that never worked will not start working).
    was_joined: bool,
    rejoin_at: Option<Instant>,
    rejoin_attempts: u32,
    /// Room and our identity while a session is established.
    room: Option<(String, String)>,
    participants: BTreeMap<String, Participant>,
    last_loud: HashMap<String, Instant>,
    net_previous: Option<(Instant, NetStats)>,
    net: Option<NetworkStats>,
}

impl Engine {
    fn run(&mut self, rx: &mpsc::Receiver<Msg>) {
        tracing::info!(protocol = PROTOCOL_VERSION, "engine starting");
        self.emit(Event::Ready {
            protocol_version: PROTOCOL_VERSION,
        });
        self.emit_devices();
        let (input, output) = {
            let settings = self.settings.lock().unwrap();
            (
                settings.input_device.clone(),
                settings.output_device.clone(),
            )
        };
        self.open_capture(input.as_deref());
        self.open_playout(output.as_deref());

        let mut next_levels = Instant::now();
        let mut next_stats = Instant::now() + STATS_INTERVAL;
        loop {
            match rx.recv_timeout(LEVEL_INTERVAL) {
                Ok(Msg::Command(command)) => self.command(command),
                Ok(Msg::Transport(event)) => self.transport_event(event),
                Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            let now = Instant::now();
            if now >= next_levels {
                next_levels = now + LEVEL_INTERVAL;
                self.emit_levels(now);
                if self.capture.is_some() {
                    self.emit_stage_levels();
                }
            }
            if now >= next_stats {
                next_stats = now + STATS_INTERVAL;
                self.check_devices();
                self.emit_stats();
            }
            if self.rejoin_at.is_some_and(|at| now >= at) {
                self.rejoin();
            }
        }
    }

    fn shutdown(&mut self) {
        tracing::info!("engine stopping");
        self.transport.leave();
        self.pump.stop.store(true, Relaxed);
        self.capture = None;
        self.playout = None;
    }

    fn emit(&self, event: Event) {
        (self.on_event)(event);
    }

    fn error(&self, source: ErrorSource, message: String) {
        tracing::error!("{message}");
        self.pending_errors
            .borrow_mut()
            .insert(source, message.clone());
        self.emit(Event::Error { message });
    }

    fn resolved(&self, source: ErrorSource) {
        self.pending_errors.borrow_mut().remove(&source);
    }

    // ── commands ────────────────────────────────────────────────────────────────────────

    fn command(&mut self, command: Command) {
        match command {
            Command::SetInputDevice { id } => {
                self.settings.lock().unwrap().input_device = id.clone();
                self.open_capture(id.as_deref());
            }
            Command::SetOutputDevice { id } => {
                self.settings.lock().unwrap().output_device = id.clone();
                self.open_playout(id.as_deref());
            }
            Command::SetTransmitMode { mode } => {
                let config = {
                    let mut settings = self.settings.lock().unwrap();
                    settings.transmit_mode = mode;
                    ChainConfig::from(&*settings)
                };
                tracing::info!(?mode, "transmit mode");
                let _ = self.chain_tx.send(config);
            }
            Command::SetPttPressed { pressed } => {
                self.pump.ptt.store(pressed, Relaxed);
            }
            Command::SetMuted { muted } => {
                tracing::info!(muted, "microphone mute");
                self.pump.muted.store(muted, Relaxed);
                self.transport.set_muted(muted);
            }
            Command::SetDeafened { deafened } => {
                tracing::info!(deafened, "deafen");
                self.mixer.set_deafened(deafened);
            }
            Command::SetPeerVolume {
                identity,
                track: TrackKind::Voice,
                gain_db,
            } => {
                let gain_db = gain_db.clamp(*PEER_GAIN_DB.start(), *PEER_GAIN_DB.end());
                tracing::info!(%identity, gain_db, "peer volume");
                self.mixer
                    .set_peer(&identity, |mix| mix.gain = db_to_gain(gain_db));
            }
            Command::SetPeerMuted {
                identity,
                track: TrackKind::Voice,
                muted,
            } => {
                tracing::info!(%identity, muted, "peer mute");
                self.mixer.set_peer(&identity, |mix| mix.muted = muted);
            }
            Command::SetPeerVolume { track, .. } | Command::SetPeerMuted { track, .. } => {
                tracing::debug!(?track, "only the voice track exists so far");
            }
            Command::ApplySettings { settings } => self.apply_settings(settings),
            Command::Join { url, token } => {
                tracing::info!(%url, "join requested");
                // The transport drops a session we replace without telling us more about
                // it: forget its room here.
                self.clear_room();
                self.wanted = Some((url, token));
                self.was_joined = false;
                self.rejoin_at = None;
                self.rejoin_attempts = 0;
                self.join();
            }
            Command::Leave => {
                tracing::info!("leave requested");
                self.wanted = None;
                self.rejoin_at = None;
                self.transport.leave();
                // Do not wait for the transport: between two rejoin attempts there is no
                // session that could report its end.
                self.clear_room();
                self.set_state(ConnectionState::Disconnected);
            }
            Command::RefreshDevices => self.emit_devices(),
            Command::Resync => {
                self.emit(Event::Ready {
                    protocol_version: PROTOCOL_VERSION,
                });
                self.emit_devices();
                self.emit(Event::Connection { state: self.state });
                if let Some((room, identity)) = self.room.clone() {
                    self.emit(Event::Joined { room, identity });
                }
                self.emit_participants();
                for (_, message) in std::mem::take(&mut *self.pending_errors.borrow_mut()) {
                    self.emit(Event::Error { message });
                }
            }
        }
    }

    fn apply_settings(&mut self, new: Settings) {
        if let Err(problem) = new.validate() {
            self.error(
                ErrorSource::Settings,
                format!("settings rejected: {problem}"),
            );
            return;
        }
        self.resolved(ErrorSource::Settings);
        let old = std::mem::replace(&mut *self.settings.lock().unwrap(), new.clone());
        if old.input_device != new.input_device || old.input_channel != new.input_channel {
            self.open_capture(new.input_device.as_deref());
        }
        if old.output_gain_db != new.output_gain_db {
            self.mixer.set_master_db(new.output_gain_db);
        }
        let chain = ChainConfig::from(&new);
        if chain != ChainConfig::from(&old) {
            let _ = self.chain_tx.send(chain);
        }
        if old.output_device != new.output_device {
            self.open_playout(new.output_device.as_deref());
        }
        if old.voice_bitrate_kbps != new.voice_bitrate_kbps {
            tracing::info!(
                kbps = new.voice_bitrate_kbps,
                "voice bitrate applies from the next join"
            );
        }
    }

    fn join(&mut self) {
        let Some((url, token)) = self.wanted.clone() else {
            return;
        };
        let options = PublishOptions {
            bitrate_bps: self.settings.lock().unwrap().voice_bitrate_kbps * 1000,
            ..PublishOptions::default()
        };
        self.transport.join(&url, &token, options);
    }

    fn rejoin(&mut self) {
        self.rejoin_at = None;
        self.rejoin_attempts += 1;
        tracing::warn!(
            attempt = self.rejoin_attempts,
            "joining again after a lost session"
        );
        self.join();
    }

    // ── transport ───────────────────────────────────────────────────────────────────────

    fn transport_event(&mut self, event: TransportEvent) {
        match event {
            TransportEvent::State(link) => self.link_state(link),
            TransportEvent::Joined { room, identity } => {
                self.was_joined = true;
                self.rejoin_attempts = 0;
                self.resolved(ErrorSource::Room);
                self.room = Some((room.clone(), identity.clone()));
                self.emit(Event::Joined { room, identity });
            }
            TransportEvent::Failed { message, retryable } => {
                if self.wanted.is_some() && self.was_joined && retryable {
                    // 3 s, 6 s, … up to ten times the base delay.
                    let pause = self.config.rejoin_delay * (self.rejoin_attempts + 1).min(10);
                    self.rejoin_at = Some(Instant::now() + pause);
                } else {
                    self.wanted = None;
                }
                self.error(ErrorSource::Room, message);
            }
            TransportEvent::ParticipantJoined { identity, name } => {
                self.participants.insert(
                    identity.clone(),
                    Participant {
                        identity,
                        name,
                        has_voice: false,
                        muted: false,
                    },
                );
                self.emit_participants();
            }
            TransportEvent::ParticipantLeft { identity } => {
                self.participants.remove(&identity);
                self.last_loud.remove(&identity);
                self.emit_participants();
            }
            TransportEvent::ParticipantMuted { identity, muted } => {
                self.participant(&identity, |p| p.muted = muted);
            }
            TransportEvent::VoiceStarted { identity } => {
                self.participant(&identity, |p| p.has_voice = true);
            }
            TransportEvent::VoiceEnded { identity } => {
                self.participant(&identity, |p| p.has_voice = false);
            }
            TransportEvent::Stats(now) => {
                let at = Instant::now();
                let previous = self.net_previous.as_ref();
                let seconds = previous.map_or(STATS_INTERVAL, |(then, _)| at - *then);
                let stats = network_stats(previous.map(|(_, s)| s), &now, seconds.as_secs_f32());
                self.net = Some(stats);
                self.net_previous = Some((at, now));
            }
        }
    }

    fn link_state(&mut self, link: LinkState) {
        let state = match link {
            LinkState::Connected => ConnectionState::Connected,
            LinkState::Reconnecting => ConnectionState::Reconnecting,
            // A rejoin after a lost session is still "reconnecting" for the user.
            LinkState::Connecting if self.was_joined => ConnectionState::Reconnecting,
            LinkState::Connecting => ConnectionState::Connecting,
            LinkState::Disconnected if self.rejoin_at.is_some() => {
                // The others are gone, the room is still where we want to be.
                let room = self.room.clone();
                self.clear_room();
                self.room = room;
                ConnectionState::Reconnecting
            }
            LinkState::Disconnected => {
                self.clear_room();
                ConnectionState::Disconnected
            }
        };
        self.set_state(state);
    }

    fn set_state(&mut self, state: ConnectionState) {
        if state != self.state {
            tracing::info!(from = ?self.state, to = ?state, "connection state");
            self.state = state;
            self.emit(Event::Connection { state });
        }
    }

    /// Forgets everything about the room we were in.
    fn clear_room(&mut self) {
        self.room = None;
        self.net = None;
        self.net_previous = None;
        self.last_loud.clear();
        if !self.participants.is_empty() {
            self.participants.clear();
            self.emit_participants();
        }
    }

    fn participant(&mut self, identity: &str, change: impl FnOnce(&mut Participant)) {
        if let Some(participant) = self.participants.get_mut(identity) {
            change(participant);
            self.emit_participants();
        }
    }

    fn emit_participants(&self) {
        let mut participants: Vec<Participant> = self.participants.values().cloned().collect();
        participants.sort_by_key(|p| p.name.to_lowercase());
        self.emit(Event::Participants { participants });
    }

    // ── devices ─────────────────────────────────────────────────────────────────────────

    fn emit_devices(&self) {
        let list = self.io.devices();
        tracing::info!(
            inputs = list.inputs.len(),
            outputs = list.outputs.len(),
            "audio devices listed"
        );
        self.emit(Event::Devices {
            inputs: list.inputs,
            outputs: list.outputs,
        });
    }

    /// Opens the microphone; if the wanted one fails, the system default.
    fn open_capture(&mut self, id: Option<&str>) {
        self.capture = None;
        self.pump.input_level.store(0, Relaxed);
        for candidate in candidates(id) {
            let (producer, ring) = RingBuffer::new(CAPTURE_RING);
            let channel = self.settings.lock().unwrap().input_channel;
            match self.io.open_capture(candidate, channel, producer) {
                Ok(device) => {
                    tracing::info!(
                        device = %device.name, rate = device.sample_rate, channels = device.channels,
                        channel = device.channel, asked = channel, "microphone open"
                    );
                    let _ = self.ring_tx.send(ring);
                    self.capture = Some(device);
                    // The asked-for device works: a stored error is resolved. A fallback to
                    // the default keeps it — "your microphone is not connected" stays true.
                    if candidate == id {
                        self.resolved(ErrorSource::Microphone);
                    }
                    return;
                }
                Err(e) => self.error(ErrorSource::Microphone, format!("microphone: {e}")),
            }
        }
    }

    /// Opens the output; if the wanted one fails, the system default.
    fn open_playout(&mut self, id: Option<&str>) {
        if self.playout.take().is_some() {
            self.reclaim_mixer();
        }
        for candidate in candidates(id) {
            let Some(mixer_rt) = self.mixer_rt.take() else {
                self.error(
                    ErrorSource::Output,
                    "output: the mixer did not come back from the last device".into(),
                );
                return;
            };
            match self
                .io
                .open_playout(candidate, mixer_rt, self.mixer_back.clone())
            {
                Ok(device) => {
                    tracing::info!(
                        device = %device.name, rate = device.sample_rate, channels = device.channels,
                        "output open"
                    );
                    self.playout = Some(device);
                    if candidate == id {
                        self.resolved(ErrorSource::Output);
                    }
                    return;
                }
                Err(e) => {
                    self.reclaim_mixer();
                    self.error(ErrorSource::Output, format!("output: {e}"));
                }
            }
        }
    }

    fn reclaim_mixer(&mut self) {
        match self.mixer_returned.recv_timeout(Duration::from_secs(2)) {
            Ok(mixer_rt) => self.mixer_rt = Some(mixer_rt),
            Err(_) => tracing::error!("the closed output stream did not give the mixer back"),
        }
    }

    /// A device that reported an error (unplugged, driver reset) is reopened on the default.
    fn check_devices(&mut self) {
        if failed(&self.capture) {
            self.error(
                ErrorSource::Microphone,
                "the microphone stopped working — switching to the default device".into(),
            );
            self.open_capture(None);
            self.emit_devices();
        }
        if failed(&self.playout) {
            self.error(
                ErrorSource::Output,
                "the output stopped working — switching to the default device".into(),
            );
            self.open_playout(None);
            self.emit_devices();
        }
    }

    // ── levels and stats ────────────────────────────────────────────────────────────────

    fn emit_levels(&mut self, now: Instant) {
        if self.capture.is_none() && self.state == ConnectionState::Disconnected {
            return;
        }
        let input = f32::from_bits(self.pump.input_level.load(Relaxed));
        let mut peers: Vec<PeerLevel> = self
            .mixer
            .levels()
            .into_iter()
            .map(|(identity, level)| {
                let level_db = gain_to_db(level);
                if level_db > SPEAKING_DB {
                    self.last_loud.insert(identity.clone(), now);
                }
                let speaking = self
                    .last_loud
                    .get(&identity)
                    .is_some_and(|at| now - *at < SPEAKING_HOLD);
                PeerLevel {
                    identity,
                    level_db,
                    speaking,
                }
            })
            .collect();
        peers.sort_by(|a, b| a.identity.cmp(&b.identity));
        self.emit(Event::Levels {
            input_db: gain_to_db(input),
            gate_open: !self.pump.muted.load(Relaxed) && self.pump.gate_open.load(Relaxed),
            peers,
        });
    }

    /// The send chain stage by stage, for the pro view (sent with every `Levels`).
    fn emit_stage_levels(&mut self) {
        let stages = STAGES
            .iter()
            .enumerate()
            .map(|(i, id)| StageLevel {
                id: (*id).to_string(),
                level_db: gain_to_db(f32::from_bits(self.pump.stage_levels[i].load(Relaxed))),
                active: self.pump.stage_active[i].load(Relaxed),
            })
            .collect();
        self.emit(Event::StageLevels { stages });
    }

    fn emit_stats(&mut self) {
        let device = |open: &Option<OpenDevice>, xruns: u32| {
            open.as_ref().map(|d| DeviceStats {
                name: d.name.clone(),
                sample_rate: d.sample_rate,
                channels: d.channels,
                callback_frames: d.counters.callback_frames.load(Relaxed),
                xruns,
            })
        };
        let counted = |open: &Option<OpenDevice>,
                       pick: fn(&crate::device::DeviceCounters) -> u32| {
            open.as_ref().map_or(0, |d| pick(&d.counters))
        };
        let overruns = counted(&self.capture, |c| c.overruns.load(Relaxed))
            + counted(&self.capture, |c| c.driver_xruns.load(Relaxed));
        let underruns =
            self.mixer.underruns() + counted(&self.playout, |c| c.driver_xruns.load(Relaxed));
        let busy_blocks = self.pump.busy_blocks.swap(0, Relaxed);
        let busy_sum = self.pump.busy_sum_ns.swap(0, Relaxed);
        let stats = Stats {
            network: self.net.clone(),
            capture: device(&self.capture, overruns),
            playout: device(&self.playout, underruns),
            send_interval_max_ms: self.pump.interval_max_ns.swap(0, Relaxed) as f32 / 1e6,
            send_blocks_dropped: self.pump.dropped_blocks.load(Relaxed),
            playout_frames_dropped: self.mixer.dropped_frames(),
            playout_skips: self.mixer.skips(),
            chain_latency_ms: self.pump.chain_latency.load(Relaxed) as f32
                / (transport::SAMPLE_RATE as f32 / 1000.0),
            pump_busy_max_ms: self.pump.busy_max_ns.swap(0, Relaxed) as f32 / 1e6,
            pump_busy_mean_ms: if busy_blocks == 0 {
                0.0
            } else {
                busy_sum as f32 / busy_blocks as f32 / 1e6
            },
        };
        if self.state != ConnectionState::Disconnected {
            // One line per second in the log file: enough to reconstruct an evening.
            let net = stats.network.clone().unwrap_or_default();
            tracing::debug!(
                target: "stats",
                state = ?self.state,
                rtt_ms = net.rtt_ms,
                send_kbps = net.send_kbps_recent,
                uplink_loss = net.uplink_loss_percent,
                loss = net.loss_percent_recent,
                jitter_ms = net.jitter_ms,
                jitter_buffer_ms = net.jitter_buffer_ms,
                concealed = net.concealed_percent_recent,
                send_interval_max_ms = stats.send_interval_max_ms,
                send_blocks_dropped = stats.send_blocks_dropped,
                pump_busy_max_ms = stats.pump_busy_max_ms,
                pump_busy_mean_ms = stats.pump_busy_mean_ms,
                capture_overruns = overruns,
                playout_underruns = underruns,
                playout_skips = stats.playout_skips,
                playout_frames_dropped = stats.playout_frames_dropped,
                participants = self.participants.len(),
                "stats"
            );
        }
        self.emit(Event::Stats { stats });
    }
}

/// The wanted device first, then the default — or only the default.
fn candidates(id: Option<&str>) -> Vec<Option<&str>> {
    match id {
        Some(id) => vec![Some(id), None],
        None => vec![None],
    }
}

fn failed(device: &Option<OpenDevice>) -> bool {
    device
        .as_ref()
        .is_some_and(|d| d.counters.failed.load(Relaxed))
}

/// Remote audio from the transport goes into mixer slots.
struct Speakers(Arc<MixerControl>);

impl AudioReceiver for Speakers {
    fn speaker_started(&self, identity: &str) -> Box<dyn FrameSink> {
        match self.0.speaker(identity) {
            Some(sink) => Box::new(sink),
            None => {
                tracing::error!(%identity, "no free mixer slot — this person will not be heard");
                Box::new(Discard)
            }
        }
    }
}

struct Discard;

impl FrameSink for Discard {
    fn frame(&mut self, _: &[i16]) {}
}

#[derive(Default)]
struct PumpShared {
    stop: AtomicBool,
    muted: AtomicBool,
    /// RMS of the last captured block as `f32` bits, 0..1 — measured before the mute.
    input_level: AtomicU32,
    /// Longest gap between two blocks since the stats timer last read it.
    interval_max_ns: AtomicU64,
    /// Work per block since the stats timer last read it: longest, sum, count.
    busy_max_ns: AtomicU64,
    busy_sum_ns: AtomicU64,
    busy_blocks: AtomicU64,
    /// Push-to-talk key held (set by the shell's hook through a command).
    ptt: AtomicBool,
    /// The chain lets the voice out (gate open, PTT held, always on).
    gate_open: AtomicBool,
    /// RMS after each chain stage as `f32` bits, and whether the stage works.
    stage_levels: [AtomicU32; STAGES.len()],
    stage_active: [AtomicBool; STAGES.len()],
    /// The chain's own delay in samples.
    chain_latency: AtomicU32,
    dropped_blocks: AtomicU32,
}

/// Which stages work and what the chain delays — right after (re)building it, so the pro
/// view is correct before the first block, and after every block (the gate moves).
fn publish_chain(shared: &PumpShared, chain: &Chain) {
    for i in 0..STAGES.len() {
        shared.stage_active[i].store(chain.active(i), Relaxed);
    }
    shared
        .chain_latency
        .store(chain.latency_samples() as u32, Relaxed);
}

/// Capture ring → 10 ms blocks → transport. Paced by the device: a block goes out as soon
/// as it is complete. Replacement rings arrive when the microphone is reopened.
fn run_pump(
    shared: &PumpShared,
    rings: mpsc::Receiver<Consumer<f32>>,
    configs: mpsc::Receiver<ChainConfig>,
    first: ChainConfig,
    voice: &dyn VoiceOut,
) {
    // Built here: DeepFilterNet cannot move between threads (ADR-008).
    let mut chain = Chain::new(first);
    publish_chain(shared, &chain);
    let mut ring: Option<Consumer<f32>> = None;
    // f32 until the hand-over (E0): the DSP chain (S2) works here at the device precision;
    // the transport wants 16-bit PCM.
    let mut block = [0f32; BLOCK];
    let mut pcm = [0i16; BLOCK];
    let mut last: Option<Instant> = None;
    while !shared.stop.load(Relaxed) {
        if let Ok(new) = rings.try_recv() {
            ring = Some(new);
            last = None;
        }
        if let Some(config) = configs.try_iter().last() {
            if chain.reconfigure(config) {
                // Loading the new denoiser took hundreds of milliseconds; what queued up
                // meanwhile is stale. Drop it instead of sending one old block first.
                if let Some(open) = ring.as_mut() {
                    let stale = open.slots();
                    if let Ok(chunk) = open.read_chunk(stale) {
                        chunk.commit_all();
                    }
                    shared
                        .dropped_blocks
                        .fetch_add((stale / BLOCK) as u32, Relaxed);
                }
                last = None;
            }
            publish_chain(shared, &chain);
        }
        let Some(open) = ring.as_mut() else {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        };
        let queued = open.slots();
        if queued < BLOCK {
            // The microphone was closed and nothing replaced it: the meter must not keep
            // showing the last level.
            if open.is_abandoned() {
                shared.input_level.store(0, Relaxed);
                ring = None;
            }
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }
        let ring = open;
        // A backlog (we stalled, or capture ran before anyone read it) must not go out as
        // a burst: it would sit in every listener's jitter buffer as standing delay.
        if queued > 3 * BLOCK {
            let stale = (queued - BLOCK) / BLOCK;
            if let Ok(chunk) = ring.read_chunk(stale * BLOCK) {
                chunk.commit_all();
            }
            shared.dropped_blocks.fetch_add(stale as u32, Relaxed);
        }
        let started = Instant::now();
        if ring.pop_entire_slice(&mut block).is_err() {
            continue;
        }
        shared
            .input_level
            .store(dsp::rms(&block).to_bits(), Relaxed);
        chain.process(&mut block, shared.ptt.load(Relaxed));
        shared.gate_open.store(chain.open, Relaxed);
        for (i, level) in chain.levels.iter().enumerate() {
            shared.stage_levels[i].store(level.to_bits(), Relaxed);
        }
        publish_chain(shared, &chain);
        if shared.muted.load(Relaxed) {
            block.fill(0.0);
        }
        let now = Instant::now();
        if let Some(last) = last {
            let gap = (now - last).as_nanos() as u64;
            shared.interval_max_ns.fetch_max(gap, Relaxed);
        }
        last = Some(now);
        for (out, s) in pcm.iter_mut().zip(&block) {
            *out = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        }
        voice.send(&pcm);
        let busy = started.elapsed().as_nanos() as u64;
        shared.busy_max_ns.fetch_max(busy, Relaxed);
        shared.busy_sum_ns.fetch_add(busy, Relaxed);
        shared.busy_blocks.fetch_add(1, Relaxed);
    }
}
