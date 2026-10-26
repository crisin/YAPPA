//! The engine with fakes on both sides: no sound hardware, no network.

use audio_engine::device::{AudioIo, DeviceCounters, DeviceList, OpenDevice};
use audio_engine::mixer::MixerRt;
use audio_engine::{spawn, EngineConfig, EngineHandle};
use engine_protocol::{
    AudioDevice, Command, ConnectionState, DeviceSlot, Event, QualityProfile, Settings, TrackKind,
    VoiceCodec,
};
use rtrb::Producer;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use transport::{
    AudioReceiver, EventFn, LinkState, PublishOptions, Transport, TransportEvent, VoiceOut, BLOCK,
};

const VOICE: transport::TrackKind = transport::TrackKind::Voice;

// ── fake transport ──────────────────────────────────────────────────────────────────────

/// What the engine attached: where events go and where remote audio goes.
type Hooks = (Arc<EventFn>, Arc<dyn AudioReceiver>);

#[derive(Default)]
struct Wire {
    calls: Mutex<Vec<String>>,
    hooks: Mutex<Option<Hooks>>,
    sent: Mutex<Vec<Vec<i16>>>,
    /// Blocks on the soundboard track (ADR-010).
    sound_sent: Mutex<Vec<Vec<i16>>>,
}

/// The soundboard's end of the fake wire.
struct SoundWire(Arc<Wire>);

impl VoiceOut for SoundWire {
    fn send(&self, block: &[i16]) {
        self.0.sound_sent.lock().unwrap().push(block.to_vec());
    }
}

impl Wire {
    fn emit(&self, event: TransportEvent) {
        let events = self.hooks.lock().unwrap().as_ref().unwrap().0.clone();
        events(event);
    }

    fn receiver(&self) -> Arc<dyn AudioReceiver> {
        self.hooks.lock().unwrap().as_ref().unwrap().1.clone()
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    /// What a real transport reports when a join goes through.
    fn establish(&self) {
        self.emit(TransportEvent::State(LinkState::Connecting));
        self.emit(TransportEvent::Joined {
            room: "gang".into(),
            identity: "me".into(),
        });
        self.emit(TransportEvent::State(LinkState::Connected));
    }
}

impl VoiceOut for Wire {
    fn send(&self, block: &[i16]) {
        self.sent.lock().unwrap().push(block.to_vec());
    }
}

struct FakeTransport(Arc<Wire>);

impl Transport for FakeTransport {
    fn attach(&mut self, events: EventFn, receiver: Arc<dyn AudioReceiver>) {
        *self.0.hooks.lock().unwrap() = Some((Arc::new(events), receiver));
    }
    fn join(&mut self, url: &str, token: &str, options: PublishOptions) {
        let call = format!("join {url} {token} {}", describe(options));
        self.0.calls.lock().unwrap().push(call);
    }
    fn set_publish(&mut self, options: PublishOptions) {
        let call = format!("publish {}", describe(options));
        self.0.calls.lock().unwrap().push(call);
    }
    fn leave(&mut self) {
        self.0.calls.lock().unwrap().push("leave".into());
    }
    fn set_muted(&mut self, muted: bool) {
        self.0.calls.lock().unwrap().push(format!("muted {muted}"));
    }
    fn voice_out(&self) -> Arc<dyn VoiceOut> {
        self.0.clone()
    }
    fn sound_out(&self) -> Arc<dyn VoiceOut> {
        Arc::new(SoundWire(self.0.clone()))
    }
}

/// `<bit/s> <channels>ch red=<bool> dtx=<bool>` — what the engine asked the transport for.
fn describe(options: PublishOptions) -> String {
    format!(
        "{} {}ch red={} dtx={}",
        options.bitrate_bps,
        options.channels(),
        options.red,
        options.dtx
    )
}

/// Defaults, but RNNoise: with the workspace's features unified the engine has the real
/// DeepFilterNet, and loading it in every test rig (debug build, in parallel) takes longer
/// than the tests wait. DeepFilterNet is tested in `denoise` and spike S2.
fn settings() -> Settings {
    Settings {
        denoiser: engine_protocol::DenoiserKind::Rnnoise,
        ..Settings::default()
    }
}

/// A send chain that changes nothing — for the tests of the plumbing around it.
fn transparent() -> Settings {
    Settings {
        denoiser: engine_protocol::DenoiserKind::Off,
        high_pass_hz: 0.0,
        transmit_mode: engine_protocol::TransmitMode::AlwaysOn,
        eq: Vec::new(),
        compressor: engine_protocol::CompressorSettings {
            enabled: false,
            ..Settings::default().compressor
        },
        ..settings()
    }
}

/// A 16-bit sample value as the f32 a device delivers; the pump turns it back exactly.
fn pcm(value: i16) -> f32 {
    f32::from(value) / f32::from(i16::MAX)
}

// ── fake sound hardware ─────────────────────────────────────────────────────────────────

/// Two microphones (`mic1`, `mic2`) and two outputs (`out1`, `out2`); `gone` never exists.
#[derive(Default)]
struct Hardware {
    opened: Mutex<Vec<String>>,
    microphone: Mutex<Option<Producer<f32>>>,
    counters: Mutex<Option<Arc<DeviceCounters>>>,
    speaker: Arc<Mutex<Option<MixerRt>>>,
    /// Unplugged right now: not listed, not openable (A2).
    absent: Mutex<Vec<String>>,
    /// The system default microphone; `None` = `mic1`.
    default_input: Mutex<Option<String>>,
}

impl Hardware {
    fn unplug(&self, name: &str) {
        self.absent.lock().unwrap().push(name.into());
    }

    fn plug_in(&self, name: &str) {
        self.absent.lock().unwrap().retain(|n| n != name);
    }

    fn is_absent(&self, name: &str) -> bool {
        name == "gone" || self.absent.lock().unwrap().iter().any(|n| n == name)
    }

    fn default_input(&self) -> String {
        self.default_input
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| "mic1".into())
    }
}

struct FakeIo(Arc<Hardware>);

/// Like a real stream: closing it hands the mixer back.
struct FakeStream {
    speaker: Arc<Mutex<Option<MixerRt>>>,
    back: mpsc::Sender<MixerRt>,
}

impl Drop for FakeStream {
    fn drop(&mut self) {
        if let Some(mixer) = self.speaker.lock().unwrap().take() {
            let _ = self.back.send(mixer);
        }
    }
}

fn open(name: &str, keep: Box<dyn std::any::Any + Send>) -> OpenDevice {
    OpenDevice {
        id: Some(name.into()),
        name: name.into(),
        sample_rate: 48_000,
        channels: 1,
        channel: 0,
        counters: Arc::new(DeviceCounters::default()),
        keep,
    }
}

impl AudioIo for FakeIo {
    fn devices(&self) -> DeviceList {
        let default_input = self.0.default_input();
        let device = |id: &str, default: &str| AudioDevice {
            id: id.into(),
            name: id.to_uppercase(),
            is_default: id == default,
        };
        let present = |names: &[&str], default: &str| -> Vec<AudioDevice> {
            names
                .iter()
                .filter(|n| !self.0.is_absent(n))
                .map(|n| device(n, default))
                .collect()
        };
        DeviceList {
            inputs: present(&["mic1", "mic2"], &default_input),
            outputs: present(&["out1", "out2"], "out1"),
        }
    }

    fn open_capture(
        &mut self,
        id: Option<&str>,
        channel: u16,
        ring: Producer<f32>,
    ) -> Result<OpenDevice, String> {
        let default = self.0.default_input();
        let name = id.unwrap_or(&default);
        let mut opened = self.0.opened.lock().unwrap();
        opened.push(format!("capture {name}"));
        if channel > 0 {
            opened.push(format!("channel {channel}"));
        }
        drop(opened);
        if self.0.is_absent(name) {
            return Err(match id {
                Some(_) => "the selected device is not connected".into(),
                None => "no microphone found".into(),
            });
        }
        *self.0.microphone.lock().unwrap() = Some(ring);
        let mut device = open(name, Box::new(()));
        // A Bluetooth headset in hands-free mode: the device delivers 16 kHz (A1).
        if name == "mic16" {
            device.sample_rate = 16_000;
        }
        *self.0.counters.lock().unwrap() = Some(device.counters.clone());
        Ok(device)
    }

    fn open_playout(
        &mut self,
        id: Option<&str>,
        mixer: MixerRt,
        back: mpsc::Sender<MixerRt>,
    ) -> Result<OpenDevice, String> {
        let name = id.unwrap_or("out1");
        self.0
            .opened
            .lock()
            .unwrap()
            .push(format!("playout {name}"));
        if self.0.is_absent(name) {
            let _ = back.send(mixer);
            return Err("the selected device is not connected".into());
        }
        *self.0.speaker.lock().unwrap() = Some(mixer);
        let stream = FakeStream {
            speaker: self.0.speaker.clone(),
            back,
        };
        Ok(open(name, Box::new(stream)))
    }
}

// ── harness ─────────────────────────────────────────────────────────────────────────────

struct Rig {
    engine: EngineHandle,
    events: mpsc::Receiver<Event>,
    wire: Arc<Wire>,
    hardware: Arc<Hardware>,
}

fn rig(settings: Settings) -> Rig {
    let wire = Arc::new(Wire::default());
    let hardware = Arc::new(Hardware::default());
    let (tx, events) = mpsc::channel();
    let config = EngineConfig {
        rejoin_delay: Duration::from_millis(50),
    };
    let engine = spawn(
        Box::new(FakeTransport(wire.clone())),
        Box::new(FakeIo(hardware.clone())),
        settings,
        config,
        move |event| {
            let _ = tx.send(event);
        },
    );
    Rig {
        engine,
        events,
        wire,
        hardware,
    }
}

impl Rig {
    /// The first event that satisfies `wanted`, skipping levels and stats in between.
    fn wait<T>(&self, what: &str, wanted: impl Fn(&Event) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        while let Ok(event) = self
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            if let Some(found) = wanted(&event) {
                return found;
            }
        }
        panic!("no event within 5 s: {what}");
    }

    fn wait_state(&self, state: ConnectionState) {
        self.wait(&format!("{state:?}"), |e| {
            matches!(e, Event::Connection { state: s } if *s == state).then_some(())
        });
    }

    /// The next [`Event::DeviceState`] that satisfies `wanted`; an [`Event::Error`] on
    /// the way fails the test — a missing or returning device is a state, not an error.
    fn wait_device_state(
        &self,
        what: &str,
        wanted: impl Fn(&DeviceSlot, &DeviceSlot) -> bool,
    ) -> (DeviceSlot, DeviceSlot) {
        self.wait(what, |e| match e {
            Event::DeviceState { input, output } if wanted(input, output) => {
                Some((input.clone(), output.clone()))
            }
            Event::Error { message } => panic!("error while waiting for {what}: {message}"),
            _ => None,
        })
    }

    fn wait_until(&self, what: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "not within 5 s: {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn join(&self) {
        self.engine.send(Command::Join {
            url: "ws://test".into(),
            token: "tok".into(),
        });
        self.wait_until("join reaches the transport", || {
            self.wire.calls().iter().any(|c| c.starts_with("join"))
        });
        self.wire.establish();
        self.wait_state(ConnectionState::Connected);
    }
}

// ── tests ───────────────────────────────────────────────────────────────────────────────

#[test]
fn starts_ready_lists_devices_and_opens_the_defaults() {
    let rig = rig(settings());
    rig.wait("ready", |e| matches!(e, Event::Ready { .. }).then_some(()));
    let inputs = rig.wait("devices", |e| match e {
        Event::Devices { inputs, .. } => Some(inputs.clone()),
        _ => None,
    });
    assert_eq!(inputs.len(), 2);
    assert!(inputs[0].is_default);
    rig.wait("stats", |e| match e {
        Event::Stats { stats } => Some(stats.clone()),
        _ => None,
    });
    assert_eq!(
        *rig.hardware.opened.lock().unwrap(),
        ["capture mic1", "playout out1"]
    );
}

#[test]
fn a_missing_device_falls_back_to_the_default_and_says_so() {
    let rig = rig(Settings {
        input_device: Some("gone".into()),
        output_device: Some("gone".into()),
        ..Settings::default()
    });
    // Not an error (the call works on the default), but a state the UI shows (A2). The
    // engine never saw "gone", so it has no name for it.
    let (input, output) = rig.wait_device_state("both on the stand-in", |i, o| {
        i.fallback && o.fallback && o.open.is_some()
    });
    assert_eq!(input.open.as_deref(), Some("mic1"));
    assert_eq!(input.missing, None);
    assert_eq!(output.open.as_deref(), Some("out1"));
    rig.wait_until("fallbacks opened", || {
        *rig.hardware.opened.lock().unwrap()
            == [
                "capture gone",
                "capture mic1",
                "playout gone",
                "playout out1",
            ]
    });
    // The mixer survived the failed attempt: remote audio still reaches the output.
    assert!(rig.hardware.speaker.lock().unwrap().is_some());
}

#[test]
fn an_error_from_before_the_ui_listened_is_handed_over_on_resync() {
    // No microphone at all at start: the error goes out before the window listens. The
    // next resync carries it again, once.
    let wire = Arc::new(Wire::default());
    let hardware = Arc::new(Hardware::default());
    hardware.unplug("mic1");
    hardware.unplug("mic2");
    let (tx, events) = mpsc::channel();
    let engine = spawn(
        Box::new(FakeTransport(wire.clone())),
        Box::new(FakeIo(hardware.clone())),
        settings(),
        EngineConfig::default(),
        move |event| {
            let _ = tx.send(event);
        },
    );
    let rig = Rig {
        engine,
        events,
        wire,
        hardware,
    };
    let first = rig.wait("error", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert!(first.contains("no microphone found"), "{first}");
    rig.engine.send(Command::Resync);
    let again = rig.wait("error again", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert_eq!(again, first);
}

#[test]
fn join_uses_the_bitrate_from_the_settings_and_reports_the_room() {
    let rig = rig(Settings {
        quality: QualityProfile::Custom,
        voice: VoiceCodec {
            bitrate_kbps: 96,
            stereo: false,
            red: true,
            dtx: false,
        },
        ..Settings::default()
    });
    rig.engine.send(Command::Join {
        url: "ws://test".into(),
        token: "tok".into(),
    });
    rig.wait_until("join", || {
        rig.wire.calls() == ["join ws://test tok 96000 1ch red=true dtx=false"]
    });
    rig.wire.emit(TransportEvent::State(LinkState::Connecting));
    rig.wait_state(ConnectionState::Connecting);
    rig.wire.emit(TransportEvent::Joined {
        room: "gang".into(),
        identity: "me".into(),
    });
    let room = rig.wait("joined", |e| match e {
        Event::Joined { room, .. } => Some(room.clone()),
        _ => None,
    });
    assert_eq!(room, "gang");
    rig.wire.emit(TransportEvent::State(LinkState::Connected));
    rig.wait_state(ConnectionState::Connected);
}

#[test]
fn participants_are_listed_sorted_with_voice_and_mute() {
    let rig = rig(settings());
    rig.join();
    for (identity, name) in [("u2", "ben"), ("u1", "Anna")] {
        rig.wire.emit(TransportEvent::ParticipantJoined {
            identity: identity.into(),
            name: name.into(),
        });
    }
    rig.wire.emit(TransportEvent::VoiceStarted {
        identity: "u2".into(),
    });
    rig.wire.emit(TransportEvent::ParticipantMuted {
        identity: "u1".into(),
        muted: true,
    });
    let list = rig.wait("both listed, with state", |e| match e {
        Event::Participants { participants }
            if participants.len() == 2 && participants[0].muted && participants[1].has_voice =>
        {
            Some(participants.clone())
        }
        _ => None,
    });
    assert_eq!(list[0].name, "Anna");
    assert_eq!(list[1].name, "ben");

    rig.wire.emit(TransportEvent::ParticipantLeft {
        identity: "u1".into(),
    });
    rig.wait("anna gone", |e| match e {
        Event::Participants { participants } if participants.len() == 1 => Some(()),
        _ => None,
    });
}

#[test]
fn a_lost_session_is_joined_again_until_the_user_leaves() {
    let rig = rig(settings());
    rig.join();
    rig.wire.emit(TransportEvent::ParticipantJoined {
        identity: "u1".into(),
        name: "Anna".into(),
    });

    rig.wire.emit(TransportEvent::Failed {
        message: "disconnected by the server".into(),
        retryable: true,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    rig.wait("the others are gone", |e| match e {
        Event::Participants { participants } if participants.is_empty() => Some(()),
        _ => None,
    });
    rig.wait_state(ConnectionState::Reconnecting);
    rig.wait_until("second join", || {
        rig.wire
            .calls()
            .iter()
            .filter(|c| c.starts_with("join"))
            .count()
            == 2
    });

    // The second attempt fails as well: keep trying.
    rig.wire.emit(TransportEvent::Failed {
        message: "could not connect".into(),
        retryable: true,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    rig.wait_until("third join", || {
        rig.wire
            .calls()
            .iter()
            .filter(|c| c.starts_with("join"))
            .count()
            == 3
    });

    // Leave in the pause before the next attempt: no session exists that could report
    // its end, the engine has to say "disconnected" by itself.
    rig.wire.emit(TransportEvent::Failed {
        message: "could not connect".into(),
        retryable: true,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    rig.engine.send(Command::Leave);
    rig.wait_state(ConnectionState::Disconnected);
    rig.wait_until("leave", || rig.wire.calls().last().unwrap() == "leave");
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        rig.wire
            .calls()
            .iter()
            .filter(|c| c.starts_with("join"))
            .count(),
        3,
        "no rejoin after leave"
    );
}

#[test]
fn a_join_that_never_worked_is_not_repeated() {
    let rig = rig(settings());
    rig.engine.send(Command::Join {
        url: "ws://test".into(),
        token: "expired".into(),
    });
    rig.wait_until("join", || rig.wire.calls().len() == 1);
    rig.wire.emit(TransportEvent::State(LinkState::Connecting));
    rig.wire.emit(TransportEvent::Failed {
        message: "401 Unauthorized".into(),
        retryable: false,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    let message = rig.wait("error", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert!(message.contains("401"));
    rig.wait_state(ConnectionState::Disconnected);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(rig.wire.calls().len(), 1);
}

#[test]
fn a_16_khz_microphone_is_resampled_into_48_khz_blocks() {
    use std::f32::consts::TAU;
    let rig = rig(Settings {
        input_device: Some("mic16".into()),
        ..transparent()
    });
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    // 25 ms of a 1 kHz tone at the device's 16 kHz — two and a half blocks' worth, which
    // stays under the pump's backlog rule (more than three blocks queued = stale).
    {
        let mut mic = rig.hardware.microphone.lock().unwrap();
        let mic = mic.as_mut().unwrap();
        for i in 0..400 {
            let s = (i as f32 / 16_000.0 * 1_000.0 * TAU).sin() * 0.5;
            mic.push(s).unwrap();
        }
    }
    rig.wait_until("two blocks sent", || {
        rig.wire.sent.lock().unwrap().len() >= 2
    });
    let block = rig.wire.sent.lock().unwrap()[1].clone();
    assert_eq!(block.len(), BLOCK);
    // 1 kHz in 10 ms: ten periods, twenty zero crossings; amplitude 0.5 = 16384.
    let crossings = block
        .windows(2)
        .filter(|w| (w[0] < 0) != (w[1] < 0))
        .count();
    assert!((18..=22).contains(&crossings), "{crossings} zero crossings");
    let peak = block.iter().map(|s| i32::from(*s).abs()).max().unwrap();
    assert!((14_000..=18_000).contains(&peak), "peak {peak}");
    // The diagnosis shows the device's own rate.
    rig.wait("stats with the device rate", |e| match e {
        Event::Stats { stats } if stats.capture.as_ref().map(|c| c.sample_rate) == Some(16_000) => {
            Some(())
        }
        _ => None,
    });
}

#[test]
fn the_microphone_reaches_the_transport_in_blocks_and_mute_sends_silence() {
    let rig = rig(transparent());
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    let speak = |blocks: usize| {
        let mut mic = rig.hardware.microphone.lock().unwrap();
        for _ in 0..blocks * BLOCK {
            mic.as_mut().unwrap().push(pcm(8000)).unwrap();
        }
    };

    speak(2);
    rig.wait_until("two blocks sent", || {
        rig.wire.sent.lock().unwrap().len() == 2
    });
    assert!(rig.wire.sent.lock().unwrap()[1] == vec![8000; BLOCK]);
    let level = rig.wait("input level", |e| match e {
        Event::Levels {
            input_db,
            gate_open: true,
            ..
        } if *input_db > -20.0 => Some(*input_db),
        _ => None,
    });
    assert!(
        (level - -12.25).abs() < 0.1,
        "8000/32767 is -12.25 dBFS, got {level}"
    );

    rig.engine.send(Command::SetMuted { muted: true });
    rig.wait_until("mute reaches the transport", || {
        rig.wire.calls().contains(&"muted true".to_string())
    });
    speak(1);
    rig.wait_until("third block sent", || {
        rig.wire.sent.lock().unwrap().len() == 3
    });
    assert!(rig.wire.sent.lock().unwrap()[2] == vec![0; BLOCK]);
    // The meter still shows the microphone works, the gate shows it is not sent.
    rig.wait("muted level", |e| match e {
        Event::Levels {
            input_db,
            gate_open: false,
            ..
        } if *input_db > -20.0 => Some(()),
        _ => None,
    });
}

/// A3: the mute and deafen keys flip the state in the engine; the UI and the presence
/// follow `MuteState`. Deafened implies muted, and undeafening gives the microphone back.
#[test]
fn toggling_mute_and_deafen_reports_one_state_and_silences_the_wire() {
    let rig = rig(transparent());
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    let mute_state = |e: &Event| match e {
        Event::MuteState { muted, deafened } => Some((*muted, *deafened)),
        _ => None,
    };

    rig.engine.send(Command::ToggleDeafened);
    assert_eq!(rig.wait("deafened", mute_state), (true, true));
    rig.wait_until("deafen mutes the transport too", || {
        rig.wire.calls().contains(&"muted true".to_string())
    });
    {
        let mut mic = rig.hardware.microphone.lock().unwrap();
        for _ in 0..BLOCK {
            mic.as_mut().unwrap().push(pcm(8000)).unwrap();
        }
    }
    rig.wait_until("a block sent", || !rig.wire.sent.lock().unwrap().is_empty());
    assert!(
        rig.wire.sent.lock().unwrap()[0] == vec![0; BLOCK],
        "deafened sends silence"
    );

    rig.engine.send(Command::ToggleDeafened);
    assert_eq!(
        rig.wait("undeafened", mute_state),
        (false, false),
        "the microphone was on before"
    );
    rig.wait_until("unmute reaches the transport", || {
        rig.wire.calls().contains(&"muted false".to_string())
    });

    rig.engine.send(Command::ToggleMuted);
    assert_eq!(rig.wait("muted", mute_state), (true, false));
    rig.engine.send(Command::ToggleDeafened);
    assert_eq!(rig.wait("deafened while muted", mute_state), (true, true));
    rig.engine.send(Command::ToggleDeafened);
    assert_eq!(
        rig.wait("still muted", mute_state),
        (true, false),
        "undeafening keeps a microphone that was off"
    );
    rig.engine.send(Command::SetMuted { muted: true });
    rig.engine.send(Command::Resync);
    // No second `MuteState` for an unchanged mute — the next one is the resync's answer.
    assert_eq!(
        rig.wait("resync repeats the state", mute_state),
        (true, false)
    );
}

#[test]
fn a_capture_backlog_is_dropped_not_sent_as_a_burst() {
    let rig = rig(transparent());
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    {
        let mut mic = rig.hardware.microphone.lock().unwrap();
        let mic = mic.as_mut().unwrap();
        // Eight blocks at once, numbered — the pump never gets a chance in between.
        let backlog: Vec<f32> = (0..8).flat_map(|n| [pcm(n); BLOCK]).collect();
        mic.push_entire_slice(&backlog).unwrap();
    }
    rig.wait_until("something sent", || {
        !rig.wire.sent.lock().unwrap().is_empty()
    });
    std::thread::sleep(Duration::from_millis(50));
    let sent = rig.wire.sent.lock().unwrap();
    assert!(sent.len() <= 3, "{} blocks went out", sent.len());
    assert_eq!(sent.last().unwrap()[0], 7, "the newest block is kept");
    drop(sent);
    let dropped = rig.wait("dropped blocks in the stats", |e| match e {
        Event::Stats { stats } if stats.send_blocks_dropped > 0 => Some(stats.send_blocks_dropped),
        _ => None,
    });
    assert!(dropped >= 5);
}

#[test]
fn remote_voices_reach_the_output_with_level_volume_and_a_freed_slot() {
    // Leveling off: the leveler would pull a steady −6 dBFS down by 6 dB/s from the first
    // block on, and the exact 0.25 below only holds while it has not moved — on a slow CI
    // box the volume command arrives after that (flaky on windows-latest, 2026-10-25).
    let rig = rig(Settings {
        auto_level: false,
        ..settings()
    });
    rig.join();
    rig.wait_until("output open", || {
        rig.hardware.speaker.lock().unwrap().is_some()
    });
    let render = |frames: usize| {
        let mut out = vec![0f32; frames];
        rig.hardware
            .speaker
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .render(&mut out);
        out
    };

    let mut anna = rig.wire.receiver().speaker_started("u1", VOICE);
    render(BLOCK); // a fresh slot is flushed first
    anna.frame(&[16384; BLOCK]);
    anna.frame(&[16384; BLOCK]);
    assert!(render(BLOCK).iter().all(|s| (*s - 0.5).abs() < 1e-3));

    let peer = rig.wait("anna speaking", |e| match e {
        Event::Levels { peers, .. } if peers.len() == 1 && peers[0].speaking => {
            Some(peers[0].clone())
        }
        _ => None,
    });
    assert_eq!(peer.identity, "u1");
    assert!((peer.level_db - -6.02).abs() < 0.1);

    rig.engine.send(Command::SetPeerVolume {
        identity: "u1".into(),
        track: TrackKind::Voice,
        gain_db: -6.0206,
    });
    rig.wait_until("volume applied", || {
        anna.frame(&[16384; BLOCK]);
        anna.frame(&[16384; BLOCK]);
        let out = render(2 * BLOCK);
        (out[0] - 0.25).abs() < 1e-3
    });

    drop(anna);
    rig.wait("nobody speaking", |e| match e {
        Event::Levels { peers, .. } if peers.is_empty() => Some(()),
        _ => None,
    });
}

#[test]
fn changing_the_output_keeps_the_speakers() {
    let rig = rig(settings());
    rig.wait_until("output open", || {
        rig.hardware.speaker.lock().unwrap().is_some()
    });
    let mut anna = rig.wire.receiver().speaker_started("u1", VOICE);

    rig.engine.send(Command::SetOutputDevice {
        id: Some("out2".into()),
    });
    rig.wait_until("second output opened", || {
        rig.hardware.opened.lock().unwrap().last().unwrap() == "playout out2"
            && rig.hardware.speaker.lock().unwrap().is_some()
    });
    assert_eq!(rig.engine.settings().output_device.as_deref(), Some("out2"));

    let mut out = vec![0f32; BLOCK];
    let mut speaker = rig.hardware.speaker.lock().unwrap();
    speaker.as_mut().unwrap().render(&mut out);
    for _ in 0..2 {
        anna.frame(&[16384; BLOCK]);
    }
    speaker.as_mut().unwrap().render(&mut out);
    assert!(
        out.iter().all(|s| (*s - 0.5).abs() < 1e-3),
        "same mixer, same slot"
    );
}

/// Pulls the open microphone's plug: the stream reports an error and the device leaves
/// the list, like a USB headset unplugged in a call.
fn unplug_open_microphone(rig: &Rig, name: &str) {
    rig.wait_until("microphone open", || {
        rig.hardware.counters.lock().unwrap().is_some()
    });
    rig.hardware.unplug(name);
    rig.hardware
        .counters
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .failed
        .store(true, Relaxed);
}

#[test]
fn an_unplugged_microphone_is_replaced_by_the_default_and_taken_back_when_it_returns() {
    let rig = rig(Settings {
        input_device: Some("mic2".into()),
        ..Settings::default()
    });
    rig.wait_device_state("mic2 open", |i, _| i.open.as_deref() == Some("mic2"));
    unplug_open_microphone(&rig, "mic2");

    // Within the next second: mic2 tried first (it is the chosen one), then the default.
    let (input, _) = rig.wait_device_state("stand-in", |i, _| i.fallback);
    assert_eq!(input.open.as_deref(), Some("mic1"));
    assert_eq!(
        input.missing.as_deref(),
        Some("MIC2"),
        "named from the earlier list"
    );
    assert_eq!(
        *rig.hardware.opened.lock().unwrap(),
        [
            "capture mic2",
            "playout out1",
            "capture mic2",
            "capture mic1"
        ]
    );
    let inputs = rig.wait("list without mic2", |e| match e {
        Event::Devices { inputs, .. } => Some(inputs.clone()),
        _ => None,
    });
    assert_eq!(inputs.len(), 1);
    assert_eq!(
        rig.engine.settings().input_device.as_deref(),
        Some("mic2"),
        "the choice stays"
    );

    // Plugged in again: found by polling (no OS notification in this rig), opened, state
    // cleared — the call never stopped.
    rig.hardware.plug_in("mic2");
    let (input, _) = rig.wait_device_state("mic2 back", |i, _| !i.fallback);
    assert_eq!(input.open.as_deref(), Some("mic2"));
    assert_eq!(input.missing, None);
    assert_eq!(
        rig.hardware.opened.lock().unwrap().last().unwrap(),
        "capture mic2"
    );
}

#[test]
fn a_refresh_takes_the_chosen_output_back_and_keeps_the_speakers() {
    let rig = rig(Settings {
        output_device: Some("out2".into()),
        ..Settings::default()
    });
    rig.hardware.unplug("out2");
    let (_, output) = rig.wait_device_state("stand-in", |_, o| o.fallback && o.open.is_some());
    assert_eq!(output.open.as_deref(), Some("out1"));
    let mut anna = rig.wire.receiver().speaker_started("u1", VOICE);

    // The shell's device watcher reports a change (or the person clicks „Geräte neu
    // einlesen“): the list is read again and out2 opened at once.
    rig.hardware.plug_in("out2");
    rig.engine.send(Command::RefreshDevices);
    let (_, output) = rig.wait_device_state("out2 back", |_, o| !o.fallback);
    assert_eq!(output.open.as_deref(), Some("out2"));
    rig.wait_until("mixer in the new stream", || {
        rig.hardware.speaker.lock().unwrap().is_some()
    });
    // Same mixer, same slot: Anna is still heard.
    let mut out = vec![0f32; BLOCK];
    let mut speaker = rig.hardware.speaker.lock().unwrap();
    speaker.as_mut().unwrap().render(&mut out); // a fresh slot is flushed first
    for _ in 0..2 {
        anna.frame(&[16384; BLOCK]);
    }
    speaker.as_mut().unwrap().render(&mut out);
    assert!(out.iter().all(|s| (*s - 0.5).abs() < 1e-3));
}

#[test]
fn with_the_default_chosen_a_new_system_default_is_followed() {
    // „Standardgerät“ and a headset is plugged in: Windows makes it the default, the open
    // stream would stay on the old one — a refresh moves over.
    let rig = rig(settings());
    rig.wait_device_state("default open", |i, _| i.open.as_deref() == Some("mic1"));
    *rig.hardware.default_input.lock().unwrap() = Some("mic2".into());
    rig.engine.send(Command::RefreshDevices);
    let (input, _) = rig.wait_device_state("new default", |i, _| i.open.as_deref() == Some("mic2"));
    assert!(!input.fallback);
    assert_eq!(
        rig.engine.settings().input_device,
        None,
        "still „Standardgerät“"
    );
    // A second refresh with nothing new changes nothing.
    let before = rig.hardware.opened.lock().unwrap().len();
    rig.engine.send(Command::RefreshDevices);
    rig.wait("devices again", |e| {
        matches!(e, Event::Devices { .. }).then_some(())
    });
    assert_eq!(rig.hardware.opened.lock().unwrap().len(), before);
}

#[test]
fn invalid_settings_are_rejected_and_the_old_ones_stay() {
    let rig = rig(settings());
    rig.engine.send(Command::ApplySettings {
        settings: Settings {
            quality: QualityProfile::Custom,
            voice: VoiceCodec {
                bitrate_kbps: 4,
                ..Settings::default().voice
            },
            ..Settings::default()
        },
    });
    let message = rig.wait("error", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert!(message.contains("settings rejected"));
    assert_eq!(rig.engine.settings().voice_codec().bitrate_kbps, 80);
}

/// A new profile during a call reaches the transport as new publish options — the session
/// stays; a profile with the same values does not.
#[test]
fn a_quality_change_republishes_without_a_rejoin() {
    let rig = rig(settings());
    rig.join();
    rig.engine.send(Command::ApplySettings {
        settings: Settings {
            quality: QualityProfile::Stress,
            ..settings()
        },
    });
    rig.wait_until("republish", || {
        rig.wire.calls()
            == [
                "join ws://test tok 80000 1ch red=true dtx=false",
                "publish 510000 2ch red=false dtx=false",
            ]
    });
    // Custom with exactly the Stress values: nothing to do.
    rig.engine.send(Command::ApplySettings {
        settings: Settings {
            quality: QualityProfile::Custom,
            voice: QualityProfile::Stress.preset().unwrap(),
            ..settings()
        },
    });
    rig.engine.send(Command::ApplySettings {
        settings: Settings {
            quality: QualityProfile::Thrifty,
            ..settings()
        },
    });
    rig.wait_until("second republish", || rig.wire.calls().len() == 3);
    assert_eq!(rig.wire.calls()[2], "publish 32000 1ch red=true dtx=false");
    assert_eq!(rig.engine.settings().quality, QualityProfile::Thrifty);
}

#[test]
fn a_ui_that_attaches_late_gets_the_current_state_again() {
    let rig = rig(settings());
    rig.join();
    rig.wire.emit(TransportEvent::ParticipantJoined {
        identity: "u1".into(),
        name: "Anna".into(),
    });
    rig.wait("anna listed", |e| match e {
        Event::Participants { participants } if participants.len() == 1 => Some(()),
        _ => None,
    });

    rig.engine.send(Command::Resync);
    rig.wait("ready", |e| matches!(e, Event::Ready { .. }).then_some(()));
    rig.wait("devices", |e| {
        matches!(e, Event::Devices { .. }).then_some(())
    });
    rig.wait_state(ConnectionState::Connected);
    let room = rig.wait("room", |e| match e {
        Event::Joined { room, identity } => Some(format!("{room}/{identity}")),
        _ => None,
    });
    assert_eq!(room, "gang/me");
    rig.wait("anna again", |e| match e {
        Event::Participants { participants } if participants.len() == 1 => Some(()),
        _ => None,
    });
}

#[test]
fn being_thrown_out_is_not_answered_with_a_rejoin() {
    let rig = rig(settings());
    rig.join();
    // The same invite was opened on a second machine: joining again would only throw
    // that one out in turn, forever.
    rig.wire.emit(TransportEvent::Failed {
        message: "disconnected by the server or the network: DuplicateIdentity".into(),
        retryable: false,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    rig.wait_state(ConnectionState::Disconnected);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(rig.wire.calls().len(), 1, "one join, no second");
}

#[test]
fn the_pause_between_rejoin_attempts_grows() {
    let rig = rig(settings());
    rig.join();
    let joins = || {
        rig.wire
            .calls()
            .iter()
            .filter(|c| c.starts_with("join"))
            .count()
    };
    let fail = || {
        rig.wire.emit(TransportEvent::Failed {
            message: "could not connect".into(),
            retryable: true,
        });
        rig.wire
            .emit(TransportEvent::State(LinkState::Disconnected));
    };
    // Base delay in this rig: 50 ms. Attempts come after 50, 100, 150 ms.
    let started = Instant::now();
    fail();
    rig.wait_until("second join", || joins() == 2);
    fail();
    rig.wait_until("third join", || joins() == 3);
    fail();
    rig.wait_until("fourth join", || joins() == 4);
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn joining_another_room_forgets_the_people_of_the_first() {
    let rig = rig(settings());
    rig.join();
    rig.wire.emit(TransportEvent::ParticipantJoined {
        identity: "u1".into(),
        name: "Anna".into(),
    });
    rig.wait("anna listed", |e| match e {
        Event::Participants { participants } if participants.len() == 1 => Some(()),
        _ => None,
    });
    // A real transport says nothing more about the session it replaces.
    rig.engine.send(Command::Join {
        url: "ws://other".into(),
        token: "tok2".into(),
    });
    rig.wait("the first room is empty for us", |e| match e {
        Event::Participants { participants } if participants.is_empty() => Some(()),
        _ => None,
    });
    rig.wait_until("second join", || rig.wire.calls().len() == 2);
}

#[test]
fn a_new_input_channel_reopens_the_microphone_on_it() {
    let rig = rig(settings());
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    rig.engine.send(Command::ApplySettings {
        settings: Settings {
            input_channel: 1,
            ..Settings::default()
        },
    });
    rig.wait_until("reopened on channel 1", || {
        rig.hardware
            .opened
            .lock()
            .unwrap()
            .contains(&"channel 1".to_string())
    });
    assert_eq!(rig.engine.settings().input_channel, 1);
}

#[test]
fn push_to_talk_sends_only_while_the_key_is_held() {
    let rig = rig(Settings {
        transmit_mode: engine_protocol::TransmitMode::PushToTalk,
        ..transparent()
    });
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    let speak = |blocks: usize| {
        let mut mic = rig.hardware.microphone.lock().unwrap();
        for _ in 0..blocks * BLOCK {
            mic.as_mut().unwrap().push(pcm(8000)).unwrap();
        }
    };
    speak(1);
    rig.wait_until("one block", || rig.wire.sent.lock().unwrap().len() == 1);
    assert!(
        rig.wire.sent.lock().unwrap()[0] == vec![0; BLOCK],
        "key up: silence"
    );
    rig.engine.send(Command::SetPttPressed { pressed: true });
    // The command and the next block race; give the pump a moment to see the key.
    std::thread::sleep(Duration::from_millis(20));
    speak(3);
    rig.wait_until("three more", || rig.wire.sent.lock().unwrap().len() == 4);
    let last = rig.wire.sent.lock().unwrap()[3].clone();
    let blocks: Vec<(i16, i16)> = rig
        .wire
        .sent
        .lock()
        .unwrap()
        .iter()
        .map(|b| (*b.iter().min().unwrap(), *b.iter().max().unwrap()))
        .collect();
    assert!(
        last == vec![8000; BLOCK],
        "key down: the voice, fully open after the 5 ms ramp {blocks:?}"
    );
}

#[test]
fn the_send_chain_reports_its_stages() {
    let rig = rig(settings());
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    let stages = rig.wait("stage levels", |e| match e {
        Event::StageLevels { stages } => Some(stages.clone()),
        _ => None,
    });
    let ids: Vec<&str> = stages.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "input",
            "highpass",
            "denoiser",
            "gate",
            "eq",
            "compressor",
            "limiter"
        ]
    );
    assert!(stages[1].active, "high-pass on by default");
    assert!(stages[2].active, "a denoiser by default");
    assert!(stages[6].active, "the limiter is always on");
}

// ── soundboard (ADR-010) ────────────────────────────────────────────────────────────────

/// A 16-bit PCM WAV in memory.
fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

/// A click plays the file out on the soundboard track and into the own playout, the UI
/// sees the start and the end, a bad file is an error and not a crash, and the others'
/// soundboard tracks get a volume of their own.
#[test]
fn a_sound_goes_out_on_its_track_and_into_the_own_mix() {
    let rig = rig(transparent());
    rig.join();
    rig.wait_until("output open", || {
        rig.hardware.speaker.lock().unwrap().is_some()
    });
    let dir = std::env::temp_dir().join(format!("yappa-engine-sound-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sting.wav");
    let samples = vec![12_000i16; 9600]; // 200 ms
    std::fs::write(&path, wav(48_000, 1, &samples)).unwrap();
    let name = path.display().to_string();

    // The output callback, stood in by a thread: drains the own mix in real time until the
    // sound is over (plus a tail — a slow CI runner decodes late) and counts what it heard.
    let speaker = rig.hardware.speaker.clone();
    let over = Arc::new(AtomicBool::new(false));
    let callback = {
        let over = over.clone();
        std::thread::spawn(move || {
            let mut out = vec![0f32; BLOCK];
            let mut heard = 0;
            let mut tail = 20;
            while tail > 0 {
                if over.load(Relaxed) {
                    tail -= 1;
                }
                out.fill(0.0);
                speaker.lock().unwrap().as_mut().unwrap().render(&mut out);
                heard += out.iter().filter(|s| **s > 0.3).count();
                std::thread::sleep(Duration::from_millis(10));
            }
            heard
        })
    };
    rig.engine.send(Command::PlaySound {
        path: name.clone(),
        gain_db: 0.0,
    });
    let started = rig.wait("sound playing", |e| match e {
        Event::Sounds { playing } if !playing.is_empty() => Some(playing.clone()),
        _ => None,
    });
    assert_eq!(started, vec![name.clone()]);
    rig.wait("sound ended", |e| {
        matches!(e, Event::Sounds { playing } if playing.is_empty()).then_some(())
    });
    over.store(true, Relaxed);
    let blocks = rig.wire.sound_sent.lock().unwrap();
    assert!(
        (19..=21).contains(&blocks.len()),
        "{} blocks on the wire",
        blocks.len()
    );
    assert!(
        blocks[5].iter().all(|s| (*s - 12_000).abs() < 50),
        "{}",
        blocks[5][0]
    );
    assert!(
        rig.wire
            .sent
            .lock()
            .unwrap()
            .iter()
            .all(|b| b.iter().all(|s| *s == 0)),
        "nothing of it on the voice track"
    );
    drop(blocks);
    // The own mix played it too: the sound ring was fed, the callback drained it.
    let heard = callback.join().unwrap();
    assert!(heard > 10 * BLOCK, "heard {heard} loud samples at home");
    let mut out = vec![0f32; BLOCK];

    rig.engine.send(Command::PlaySound {
        path: dir.join("missing.mp3").display().to_string(),
        gain_db: 0.0,
    });
    let message = rig.wait("error", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert!(message.contains("missing.mp3"), "{message}");

    // A per-person soundboard volume lands on that track, not on the voice.
    let mut board = rig
        .wire
        .receiver()
        .speaker_started("u1", transport::TrackKind::Soundboard);
    let mut voice = rig.wire.receiver().speaker_started("u1", VOICE);
    rig.engine.send(Command::SetPeerMuted {
        identity: "u1".into(),
        track: TrackKind::Soundboard,
        muted: true,
    });
    rig.engine.send(Command::ApplySettings {
        settings: Settings {
            auto_level: false,
            ..transparent()
        },
    });
    std::thread::sleep(Duration::from_millis(100));
    let render = |out: &mut Vec<f32>| {
        out.fill(0.0);
        rig.hardware
            .speaker
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .render(out);
    };
    render(&mut out);
    for _ in 0..3 {
        board.frame(&[16_000; BLOCK]);
        voice.frame(&[-8_000; BLOCK]);
    }
    render(&mut out);
    assert!(
        out.iter().all(|s| (*s - -0.244).abs() < 0.01),
        "the voice alone, the muted soundboard track not: {}",
        out[0]
    );
}
