//! The soundboard (ADR-010): sound files, decoded once, played from a thread of their own.
//!
//! One `yappa-sounds` thread owns the clips. It is not the pump: the pump is paced by the
//! microphone and stops when there is none, while a sound must play with or without a mic.
//! So this thread paces itself — one 10 ms block every 10 ms, kept a block ahead of real
//! time — and sleeps entirely while nothing plays. Every block goes two ways at once:
//! to the transport's `soundboard` track ([`VoiceOut`]) and into the own playout mix
//! ([`SoundFeed`]), so the clicker hears what the others get. Decoding is I/O and CPU, so it
//! happens on a short-lived helper thread per file and never stalls a playing clip; a
//! decoded clip stays in a cache for the next click.
//!
//! Nothing here touches the real-time callback: the feed is an `rtrb` ring, the mixer drains
//! it like the monitor slot (A5).

use crate::mixer::SoundFeed;
use dsp::{db_to_gain, Limiter, Resampler};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use transport::{VoiceOut, BLOCK, SAMPLE_RATE};

/// Longest clip kept, in seconds: 60 s of 48 kHz mono f32 is 11.5 MB. A longer file is cut
/// — a soundboard plays stings, not albums.
pub const MAX_CLIP_SECONDS: usize = 60;
/// Decoded clips kept in memory, in samples: about five minutes of audio (57 MB). Over
/// this the cache is emptied; a library of short clips never gets there.
const CACHE_SAMPLES: usize = 300 * SAMPLE_RATE as usize;
/// Sounds playing at once. One more replaces the oldest.
pub const MAX_VOICES: usize = 8;
const BLOCK_PERIOD: Duration = Duration::from_millis(10);
/// How far ahead of real time the thread delivers blocks: one block, so a late wake-up
/// (Windows timers are coarse without a raised resolution) does not run the local ring
/// dry — the mixer's cushion is two blocks, its trim starts at four.
const LEAD: Duration = BLOCK_PERIOD;
/// The limiter on the sound mix: overlapping clips must not clip the track.
const CEILING_DB: f32 = -1.0;
const RELEASE_MS: f32 = 80.0;

/// What the engine tells the sound thread.
pub enum SoundCommand {
    /// Play this file at its own volume in dB (decoded on first use, cached after). A click
    /// while the same file is still being decoded is one click.
    Play(String, f32),
    /// Stop everything at once.
    Stop,
    /// The soundboard volume, dB.
    Gain(f32),
    /// One file's own volume, dB — for the clips of it playing right now.
    ClipGain(String, f32),
    Shutdown,
    /// A helper thread finished decoding; the stamp is the file as it was read.
    Decoded(String, Stamp, Result<Vec<f32>, String>),
}

/// A file as the cache knows it: changed on disk means decode again — „Sound hinzufügen“
/// may overwrite a file with a better take of the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
}

impl Stamp {
    fn of(path: &Path) -> Option<Stamp> {
        let meta = std::fs::metadata(path).ok()?;
        Some(Stamp {
            len: meta.len(),
            modified: meta.modified().ok(),
        })
    }
}

/// What the sound thread tells the engine.
pub enum SoundEvent {
    /// The set of playing files changed (paths as given to `Play`).
    Playing(Vec<String>),
    /// A file could not be played.
    Failed { path: String, reason: String },
}

/// The running sound thread. Dropping it stops the thread.
pub struct SoundThread {
    tx: mpsc::Sender<SoundCommand>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl SoundThread {
    pub fn send(&self, command: SoundCommand) {
        let _ = self.tx.send(command);
    }
}

impl Drop for SoundThread {
    fn drop(&mut self) {
        let _ = self.tx.send(SoundCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Starts the thread. `gain_db` is the soundboard volume to begin with; `on_event` is called
/// from the sound thread.
pub fn spawn(
    out: Arc<dyn VoiceOut>,
    feed: SoundFeed,
    gain_db: f32,
    on_event: impl Fn(SoundEvent) + Send + 'static,
) -> SoundThread {
    let (tx, rx) = mpsc::channel();
    let decoded = tx.clone();
    let thread = std::thread::Builder::new()
        .name("yappa-sounds".into())
        .spawn(move || {
            Player::new(out, feed, gain_db, decoded, Box::new(on_event)).run(&rx);
        })
        .expect("spawn the sound thread");
    SoundThread {
        tx,
        thread: Some(thread),
    }
}

struct Voice {
    path: String,
    clip: Arc<Vec<f32>>,
    at: usize,
    /// This clip's own volume, as a factor.
    gain: f32,
}

struct Player {
    out: Arc<dyn VoiceOut>,
    feed: SoundFeed,
    gain: f32,
    /// Where the helper threads report their result.
    decoded: mpsc::Sender<SoundCommand>,
    on_event: Box<dyn Fn(SoundEvent) + Send>,
    voices: Vec<Voice>,
    cache: HashMap<String, (Stamp, Arc<Vec<f32>>)>,
    cached_samples: usize,
    /// Files being decoded right now, with the volume the click wanted.
    decoding: HashMap<String, f32>,
    limiter: Limiter,
    /// When the next block is due; `None` while nothing plays.
    next: Option<Instant>,
}

impl Player {
    fn new(
        out: Arc<dyn VoiceOut>,
        feed: SoundFeed,
        gain_db: f32,
        decoded: mpsc::Sender<SoundCommand>,
        on_event: Box<dyn Fn(SoundEvent) + Send>,
    ) -> Self {
        Self {
            out,
            feed,
            gain: db_to_gain(gain_db),
            decoded,
            on_event,
            voices: Vec::new(),
            cache: HashMap::new(),
            cached_samples: 0,
            decoding: HashMap::new(),
            limiter: Limiter::new(SAMPLE_RATE as f32, CEILING_DB, RELEASE_MS),
            next: None,
        }
    }

    fn run(&mut self, rx: &mpsc::Receiver<SoundCommand>) {
        loop {
            // Idle: sleep on the channel. Playing: take what is there, then pace.
            let first = match self.next {
                None => match rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => return,
                },
                Some(_) => rx.try_recv().ok(),
            };
            for command in first.into_iter().chain(rx.try_iter()) {
                if !self.handle(command) {
                    return;
                }
            }
            let Some(next) = self.next else {
                continue;
            };
            let now = Instant::now();
            if now + LEAD < next {
                // High-resolution on Windows 10+ since Rust 1.75: wakes within a millisecond.
                std::thread::sleep(next - LEAD - now);
                continue;
            }
            if now > next + 10 * BLOCK_PERIOD {
                // Far behind (the machine stalled): start over from now rather than
                // bursting ten blocks into everyone's jitter buffer.
                self.next = Some(now);
            }
            self.block();
            self.next = self.next.map(|t| t + BLOCK_PERIOD);
            if self.voices.is_empty() {
                self.next = None;
                self.limiter = Limiter::new(SAMPLE_RATE as f32, CEILING_DB, RELEASE_MS);
                (self.on_event)(SoundEvent::Playing(Vec::new()));
            }
        }
    }

    /// `false` = shut down.
    fn handle(&mut self, command: SoundCommand) -> bool {
        match command {
            SoundCommand::Play(path, gain_db) => {
                let stamp = Stamp::of(Path::new(&path));
                let cached = self
                    .cache
                    .get(&path)
                    .filter(|(known, _)| Some(*known) == stamp)
                    .map(|(_, clip)| clip.clone());
                if let Some(clip) = cached {
                    self.start(path, clip, gain_db);
                } else if let std::collections::hash_map::Entry::Vacant(slot) =
                    self.decoding.entry(path.clone())
                {
                    slot.insert(gain_db);
                    let decoded = self.decoded.clone();
                    let file = path.clone();
                    let _ = std::thread::Builder::new()
                        .name("yappa-sound-decode".into())
                        .spawn(move || {
                            // A decoder panic on a malformed file must not swallow every
                            // later click on it: it is reported like any other failure.
                            let result = std::panic::catch_unwind(|| decode(Path::new(&file)))
                                .unwrap_or_else(|_| Err("the decoder crashed on this file".into()));
                            let stamp = stamp.unwrap_or(Stamp {
                                len: 0,
                                modified: None,
                            });
                            let _ = decoded.send(SoundCommand::Decoded(file, stamp, result));
                        });
                }
            }
            SoundCommand::Decoded(path, stamp, result) => {
                // Stop while decoding withdraws the click: cache the clip, do not play it.
                let wanted = self.decoding.remove(&path);
                match result {
                    Ok(samples) => {
                        let clip = Arc::new(samples);
                        if let Some((_, old)) = self.cache.remove(&path) {
                            self.cached_samples -= old.len();
                        }
                        if self.cached_samples + clip.len() > CACHE_SAMPLES {
                            self.cache.clear();
                            self.cached_samples = 0;
                        }
                        self.cached_samples += clip.len();
                        self.cache.insert(path.clone(), (stamp, clip.clone()));
                        if let Some(gain_db) = wanted {
                            self.start(path, clip, gain_db);
                        }
                    }
                    Err(reason) => {
                        tracing::warn!(%path, %reason, "sound file could not be played");
                        (self.on_event)(SoundEvent::Failed { path, reason });
                    }
                }
            }
            SoundCommand::Stop => {
                self.decoding.clear();
                if !self.voices.is_empty() {
                    tracing::info!("sounds stopped");
                    self.voices.clear();
                }
            }
            SoundCommand::Gain(db) => self.gain = db_to_gain(db),
            SoundCommand::ClipGain(path, db) => {
                let gain = db_to_gain(db);
                for voice in self.voices.iter_mut().filter(|v| v.path == path) {
                    voice.gain = gain;
                }
                if let Some(pending) = self.decoding.get_mut(&path) {
                    *pending = db;
                }
            }
            SoundCommand::Shutdown => return false,
        }
        true
    }

    fn start(&mut self, path: String, clip: Arc<Vec<f32>>, gain_db: f32) {
        if clip.is_empty() {
            return;
        }
        tracing::info!(%path, gain_db, seconds = clip.len() as f32 / SAMPLE_RATE as f32, "sound starts");
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push(Voice {
            path,
            clip,
            at: 0,
            gain: db_to_gain(gain_db),
        });
        if self.next.is_none() {
            self.next = Some(Instant::now());
        }
        (self.on_event)(SoundEvent::Playing(self.playing()));
    }

    fn playing(&self) -> Vec<String> {
        self.voices.iter().map(|v| v.path.clone()).collect()
    }

    /// One block of every voice, summed, through the gain and the limiter, out both ways.
    fn block(&mut self) {
        let mut block = [0f32; BLOCK];
        for voice in &mut self.voices {
            let end = (voice.at + BLOCK).min(voice.clip.len());
            for (o, s) in block.iter_mut().zip(&voice.clip[voice.at..end]) {
                *o += *s * voice.gain;
            }
            voice.at = end;
        }
        let before = self.voices.len();
        self.voices.retain(|v| v.at < v.clip.len());
        if self.gain != 1.0 {
            for s in block.iter_mut() {
                *s *= self.gain;
            }
        }
        self.limiter.process(&mut block);
        self.feed.block(&block);
        let mut pcm = [0i16; BLOCK];
        for (out, s) in pcm.iter_mut().zip(&block) {
            *out = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        }
        self.out.send(&pcm);
        if self.voices.len() != before && !self.voices.is_empty() {
            (self.on_event)(SoundEvent::Playing(self.playing()));
        }
    }
}

/// Decodes a sound file to 48 kHz mono samples in −1..1 — WAV, FLAC, MP3, Ogg Vorbis,
/// AAC/M4A, whatever symphonia reads. Stereo is averaged; other rates are converted with
/// the engine's own resampler (A1). Cut at [`MAX_CLIP_SECONDS`].
pub fn decode(path: &Path) -> Result<Vec<f32>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("cannot open: {e}"))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("not a sound file I can read: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no audio track in the file")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("codec not supported: {e}"))?;

    let mut mono: Vec<f32> = Vec::new();
    let mut rate = 0u32;
    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // The end of the file arrives as an I/O error, by symphonia's design.
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(format!("damaged file: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A damaged packet is skipped, the rest still plays.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("damaged file: {e}")),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        rate = spec.rate;
        let buffer =
            buffer.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        buffer.copy_interleaved_ref(decoded);
        for frame in buffer.samples().chunks_exact(channels) {
            mono.push(frame.iter().sum::<f32>() / channels as f32);
        }
        if mono.len() >= MAX_CLIP_SECONDS * rate as usize {
            tracing::warn!(path = %path.display(), "sound longer than {MAX_CLIP_SECONDS} s — cut");
            mono.truncate(MAX_CLIP_SECONDS * rate as usize);
            break;
        }
    }
    if mono.is_empty() || rate == 0 {
        return Err("the file holds no sound".into());
    }
    Ok(to_engine_rate(&mono, rate))
}

/// Converts to 48 kHz with the engine's resampler; the kernel's delay is flushed with
/// silence so the clip ends where the file ends.
fn to_engine_rate(mono: &[f32], rate: u32) -> Vec<f32> {
    if rate == SAMPLE_RATE {
        return mono.to_vec();
    }
    let mut resampler = Resampler::new(rate, SAMPLE_RATE);
    let tail = resampler.latency_samples();
    let expected = (mono.len() as u64 * u64::from(SAMPLE_RATE) / u64::from(rate)) as usize;
    let mut out = vec![0f32; expected + BLOCK];
    let mut input = mono
        .iter()
        .copied()
        .chain(std::iter::repeat_n(0.0, tail + BLOCK));
    let written = resampler.produce(&mut out, || input.next());
    out.truncate(written.min(expected));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A WAV file (16-bit PCM) in memory — the one format every decoder test needs.
    pub(crate) fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
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

    fn temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("yappa-soundboard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_stereo_wav_at_44100_becomes_mono_at_48000() {
        // 0.5 s, left +0.5, right −0.5 → mono 0; then left = right = 0.25 → 0.25.
        let half = 22_050usize;
        let mut samples = Vec::new();
        for _ in 0..half {
            samples.extend_from_slice(&[i16::MAX / 2, -(i16::MAX / 2)]);
        }
        for _ in 0..half {
            samples.extend_from_slice(&[i16::MAX / 4, i16::MAX / 4]);
        }
        let path = temp("stereo.wav", &wav(44_100, 2, &samples));
        let mono = decode(&path).unwrap();
        assert!(
            (mono.len() as i64 - 48_000).abs() < 48,
            "{} samples",
            mono.len()
        );
        assert!(mono[1000].abs() < 1e-3, "cancelled: {}", mono[1000]);
        assert!(
            (mono[40_000] - 0.25).abs() < 1e-2,
            "averaged: {}",
            mono[40_000]
        );
    }

    #[test]
    fn a_48k_mono_wav_comes_through_sample_exact_and_a_long_one_is_cut() {
        let samples: Vec<i16> = (0..4800).map(|i| (i % 100) as i16 * 100).collect();
        let path = temp("mono.wav", &wav(48_000, 1, &samples));
        let mono = decode(&path).unwrap();
        assert_eq!(mono.len(), 4800);
        // symphonia scales by 32768, the transport by 32767: a hair apart, not a sample.
        assert!((mono[150] - f32::from(samples[150]) / f32::from(i16::MAX)).abs() < 1e-4);

        let long = vec![1000i16; (MAX_CLIP_SECONDS + 5) * 8000];
        let path = temp("long.wav", &wav(8_000, 1, &long));
        let mono = decode(&path).unwrap();
        assert!(
            (mono.len() as i64 - (MAX_CLIP_SECONDS * 48_000) as i64).abs() < 100,
            "{}",
            mono.len()
        );
    }

    #[test]
    fn garbage_and_a_missing_file_are_errors_not_panics() {
        let path = temp("garbage.mp3", b"this is not audio at all, just bytes");
        assert!(decode(&path).is_err());
        assert!(decode(&path.with_file_name("none.wav")).is_err());
    }

    // ── the thread ──────────────────────────────────────────────────────────────────────

    #[derive(Default)]
    struct Sent(Mutex<Vec<Vec<i16>>>);

    impl VoiceOut for Sent {
        fn send(&self, block: &[i16]) {
            self.0.lock().unwrap().push(block.to_vec());
        }
    }

    /// Two clicks of a 100 ms clip, the second as soon as the first plays: both go out,
    /// overlapping, limited; the playing set is reported at every change, and the thread
    /// goes quiet at the end. (The second click waits for the first `Playing` on purpose:
    /// a click while the file is still being decoded counts as the same click, and a slow
    /// CI runner decodes slower than 50 ms.)
    #[test]
    fn clips_play_overlapping_out_both_ways_and_the_state_is_reported() {
        let (control, mut rt) = crate::mixer::mixer();
        let feed = control.sound_feed().unwrap();
        let sent = Arc::new(Sent::default());
        let (events_tx, events) = mpsc::channel();
        let player = spawn(sent.clone(), feed, 0.0, move |e| {
            let _ = events_tx.send(e);
        });
        let loud = vec![i16::MAX - 100; 4800];
        let path = temp("loud.wav", &wav(48_000, 1, &loud));
        let name = path.display().to_string();
        // The output callback, stood in by a thread: drains the local ring in real time
        // until the sounds are over (plus a tail) and counts what it heard.
        let over = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback = {
            let over = over.clone();
            std::thread::spawn(move || {
                let mut out = vec![0f32; BLOCK];
                let mut heard = 0;
                let mut tail = 20;
                while tail > 0 {
                    if over.load(std::sync::atomic::Ordering::Relaxed) {
                        tail -= 1;
                    }
                    out.fill(0.0);
                    rt.render(&mut out);
                    heard += out.iter().filter(|s| s.abs() > 0.5).count();
                    std::thread::sleep(Duration::from_millis(10));
                }
                heard
            })
        };
        let next = |events: &mpsc::Receiver<SoundEvent>| match events
            .recv_timeout(Duration::from_secs(10))
            .expect("an event in time")
        {
            SoundEvent::Playing(p) => p,
            SoundEvent::Failed { reason, .. } => panic!("{reason}"),
        };
        player.send(SoundCommand::Play(name.clone(), 0.0));
        let started = Instant::now();
        let first = next(&events);
        assert_eq!(first, vec![name.clone()]);
        player.send(SoundCommand::Play(name.clone(), 0.0));
        let mut seen = vec![first];
        loop {
            let p = next(&events);
            let done = p.is_empty();
            seen.push(p);
            if done {
                break;
            }
        }
        let lasted = started.elapsed();
        over.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(seen.iter().any(|p| p.len() == 2), "overlapped: {seen:?}");
        assert_eq!(seen.last().unwrap(), &Vec::<String>::new());
        let blocks = sent.0.lock().unwrap();
        // 10 blocks of the first clip plus the offset of the second (a few blocks at most
        // on a busy machine) — never fewer than one clip, never two clips back to back.
        assert!(
            (10..20).contains(&blocks.len()),
            "{} blocks in {lasted:?}",
            blocks.len()
        );
        let peak = blocks.iter().flatten().map(|s| s.abs()).max().unwrap();
        let ceiling = (dsp::db_to_gain(CEILING_DB) * f32::from(i16::MAX)) as i16;
        assert!(peak <= ceiling + 1, "limited: {peak} > {ceiling}");
        assert!(peak > ceiling - 2000, "but loud: {peak}");
        drop(blocks);
        // The same blocks reached the local mix.
        let heard = callback.join().unwrap();
        assert!(heard > 8 * BLOCK, "heard {heard} loud samples");
        drop(player);
    }

    #[test]
    fn a_bad_file_reports_a_failure_and_stop_silences_at_once() {
        let (control, _rt) = crate::mixer::mixer();
        let feed = control.sound_feed().unwrap();
        let sent = Arc::new(Sent::default());
        let (events_tx, events) = mpsc::channel();
        let player = spawn(sent.clone(), feed, -6.0, move |e| {
            let _ = events_tx.send(e);
        });
        let missing = temp("marker.txt", b"").with_file_name("none.wav");
        let missing = missing.display().to_string();
        player.send(SoundCommand::Play(missing.clone(), 0.0));
        match events.recv_timeout(Duration::from_secs(5)).unwrap() {
            SoundEvent::Failed { path, .. } => assert_eq!(path, missing),
            SoundEvent::Playing(_) => panic!("nothing to play"),
        }
        let long = vec![10_000i16; 48_000];
        let path = temp("second.wav", &wav(48_000, 1, &long));
        player.send(SoundCommand::Play(path.display().to_string(), 0.0));
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            SoundEvent::Playing(p) if p.len() == 1
        ));
        std::thread::sleep(Duration::from_millis(60));
        player.send(SoundCommand::Stop);
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            SoundEvent::Playing(p) if p.is_empty()
        ));
        let blocks = sent.0.lock().unwrap().len();
        assert!(blocks < 20, "stopped after {blocks} blocks, not 100");
        // −6 dB on the way out.
        let sample = sent.0.lock().unwrap()[2][100];
        assert!((sample - 5_000).abs() < 60, "{sample}");
    }

    /// A clip's own volume applies on top of the soundboard gain, follows a change while
    /// it plays, and a file replaced on disk under the same name is decoded again.
    #[test]
    fn a_clip_has_its_own_volume_and_a_replaced_file_is_read_again() {
        let (control, _rt) = crate::mixer::mixer();
        let feed = control.sound_feed().unwrap();
        let sent = Arc::new(Sent::default());
        let (events_tx, events) = mpsc::channel();
        let player = spawn(sent.clone(), feed, 0.0, move |e| {
            let _ = events_tx.send(e);
        });
        let path = temp("own.wav", &wav(48_000, 1, &vec![10_000i16; 48_000]));
        let name = path.display().to_string();
        player.send(SoundCommand::Play(name.clone(), -6.0));
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            SoundEvent::Playing(p) if p.len() == 1
        ));
        std::thread::sleep(Duration::from_millis(60));
        player.send(SoundCommand::ClipGain(name.clone(), -12.0));
        std::thread::sleep(Duration::from_millis(60));
        player.send(SoundCommand::Stop);
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            SoundEvent::Playing(p) if p.is_empty()
        ));
        {
            let blocks = sent.0.lock().unwrap();
            let early = blocks[2][100];
            assert!((early - 5_000).abs() < 60, "−6 dB: {early}");
            // The very last block is the silence the loop sends once the voices are gone.
            let late = blocks[blocks.len() - 2][100];
            assert!((late - 2_500).abs() < 60, "−12 dB after the change: {late}");
        }
        sent.0.lock().unwrap().clear();

        // Same name, other content (and a later mtime): the second click plays the new file.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&path, wav(48_000, 1, &vec![-10_000i16; 48_000])).unwrap();
        player.send(SoundCommand::Play(name.clone(), 0.0));
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            SoundEvent::Playing(p) if p.len() == 1
        ));
        std::thread::sleep(Duration::from_millis(60));
        player.send(SoundCommand::Stop);
        let sample = sent.0.lock().unwrap()[2][100];
        assert!((sample - -10_000).abs() < 60, "the new file: {sample}");
    }
}
