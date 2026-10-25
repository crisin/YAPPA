//! The receive mix: one ring per remote speaker, summed in the output callback.
//!
//! Two halves. [`MixerRt`] lives in the real-time callback and only touches `rtrb` rings and
//! atomics. [`MixerControl`] is everything else: handing rings to speakers, volume, mute,
//! levels. The number of speakers is fixed up front so the callback never allocates.
//!
//! Per slot a [`Leveler`] (A4) brings every voice to about the same level before the manual
//! per-person volume is applied on top; the sum then passes the limiter.
//!
//! Next to the speakers sits one local slot of its own kind, the **monitor** (A5, „Mich
//! selbst hören“): the pump feeds the send chain's output into it through a [`MonitorFeed`]
//! while the Signalkette page asks for it, and the callback adds it to the mix without a
//! leveler or a per-person volume — what goes out is what one hears, plus the master volume
//! and the limiter like everything else.

use dsp::{Leveler, LevelerParams, Limiter};
use rtrb::{Consumer, Producer, RingBuffer};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use transport::{FrameSink, BLOCK};

/// Speakers mixed at once. A full channel is 19 others.
pub const SLOTS: usize = 24;
/// Audio that must be queued before a speaker starts (again): at least 20 ms, and always
/// one network frame more than the device asks for in one go — a cushion that is smaller
/// than the callback runs dry on every callback.
pub const PREFILL: usize = 2 * BLOCK;
/// More than this queued means the speaker's clock runs ahead of our output device, or
/// playout stalled: the callback jumps forward to the prefill level. 100 ms.
pub const HIGH_WATER: usize = 10 * BLOCK;
/// Ring size per speaker: 200 ms. A frame that does not fit is dropped by the sink.
const RING: usize = 20 * BLOCK;
/// The monitor's ring: 200 ms like a speaker's. The pump delivers one block every 10 ms and
/// the callback drains it at the same pace; more than the cushion ever sits there only
/// while the two clocks drift apart — then the callback trims it back (the ADR-003 skip,
/// acceptable for a path that is open only while one tunes the chain). The cushion is
/// capped at half the ring, so a device with a huge callback (2048 frames and more) still
/// starts and the trim still fires (realtime review 2026-10-25).
const MONITOR_RING: usize = RING;

#[derive(Default)]
struct Slot {
    /// Linear gain as `f32` bits.
    gain: AtomicU32,
    muted: AtomicBool,
    /// Set when the slot gets a new speaker: the callback discards what the last one left.
    flush: AtomicBool,
    /// RMS of the last received frame as `f32` bits, 0..1. Written by the sink.
    level: AtomicU32,
    /// The leveler's current gain for this speaker, dB as `f32` bits. Written by the
    /// callback, read for the diagnosis.
    leveler_db: AtomicU32,
}

/// The limiter at the end of the mix: 1 dB below full scale, recovers within 80 ms.
const CEILING_DB: f32 = -1.0;
const RELEASE_MS: f32 = 80.0;

/// The monitor slot's switches and counters (A5). `on` is read by the pump (feed or not) and
/// by the callback (mix or drain).
#[derive(Default)]
struct Monitor {
    on: AtomicBool,
    /// Most samples found in the ring when the callback read it, since the control thread
    /// last took the value — the ring's share of the monitor latency.
    queued_max: AtomicU32,
    /// Times the callback found the ring short while the monitor played.
    underruns: AtomicU32,
    /// Blocks the pump could not queue because the ring was full.
    dropped_blocks: AtomicU32,
}

#[derive(Default)]
struct Shared {
    monitor: Monitor,
    slots: [Slot; SLOTS],
    /// Master gain (linear, `f32` bits), unity until set.
    master: AtomicU32,
    /// Leveling on (the default); off, every leveler slides back to 0 dB.
    leveling: AtomicBool,
    deafened: AtomicBool,
    underruns: AtomicU32,
    skips: AtomicU32,
    dropped_frames: AtomicU32,
}

/// Volume and mute for one remote person, remembered across their reconnects.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PeerMix {
    pub gain: f32,
    pub muted: bool,
}

impl Default for PeerMix {
    fn default() -> Self {
        Self {
            gain: 1.0,
            muted: false,
        }
    }
}

struct Routing {
    free: Vec<(usize, Producer<i16>)>,
    /// Who currently speaks through which slot.
    active: HashMap<String, usize>,
    peers: HashMap<String, PeerMix>,
}

pub struct MixerControl {
    shared: Arc<Shared>,
    routing: Mutex<Routing>,
    /// The monitor ring's write side, until the pump takes it ([`MixerControl::monitor_feed`]).
    monitor_feed: Mutex<Option<Producer<f32>>>,
}

pub struct MixerRt {
    shared: Arc<Shared>,
    rings: Vec<Consumer<i16>>,
    playing: [bool; SLOTS],
    /// One per slot, allocated here — never in the callback.
    levelers: Vec<Leveler>,
    /// The monitor slot (A5): the own send chain's output, in f32 as the chain has it.
    monitor: Consumer<f32>,
    monitor_playing: bool,
    limiter: Limiter,
}

pub fn mixer() -> (Arc<MixerControl>, MixerRt) {
    let shared = Arc::new(Shared::default());
    shared.master.store(1f32.to_bits(), Relaxed);
    shared.leveling.store(true, Relaxed);
    let mut free = Vec::with_capacity(SLOTS);
    let mut rings = Vec::with_capacity(SLOTS);
    let mut levelers = Vec::with_capacity(SLOTS);
    let sample_rate = transport::SAMPLE_RATE as f32;
    for index in 0..SLOTS {
        let (producer, consumer) = RingBuffer::new(RING);
        shared.slots[index].gain.store(1f32.to_bits(), Relaxed);
        free.push((index, producer));
        rings.push(consumer);
        levelers.push(Leveler::new(sample_rate, LevelerParams::default()));
    }
    free.reverse(); // hand out slot 0 first
    let (monitor_producer, monitor) = RingBuffer::new(MONITOR_RING);
    let control = Arc::new(MixerControl {
        shared: shared.clone(),
        routing: Mutex::new(Routing {
            free,
            active: HashMap::new(),
            peers: HashMap::new(),
        }),
        monitor_feed: Mutex::new(Some(monitor_producer)),
    });
    let rt = MixerRt {
        shared,
        rings,
        playing: [false; SLOTS],
        levelers,
        monitor,
        monitor_playing: false,
        limiter: Limiter::new(sample_rate, CEILING_DB, RELEASE_MS),
    };
    (control, rt)
}

impl MixerRt {
    /// Adds every speaker into `out` (mono, -1..1), which must come in zeroed — each through
    /// their leveler and their manual volume — applies the master volume and limits the sum
    /// below clipping — 19 people laughing at once used to clip hard (realtime review
    /// 2026-10-23, item c). Real-time safe: no allocation, no lock, no system call.
    pub fn render(&mut self, out: &mut [f32]) {
        let wanted = out.len();
        let deafened = self.shared.deafened.load(Relaxed);
        let leveling = self.shared.leveling.load(Relaxed);
        for (index, ring) in self.rings.iter_mut().enumerate() {
            let slot = &self.shared.slots[index];
            let playing = &mut self.playing[index];
            let leveler = &mut self.levelers[index];
            leveler.set_enabled(leveling);
            if slot.flush.swap(false, Relaxed) {
                discard(ring, ring.slots());
                *playing = false;
                leveler.reset();
                slot.leveler_db.store(0f32.to_bits(), Relaxed);
            }
            let mut queued = ring.slots();
            let cushion = PREFILL.max(wanted + BLOCK);
            if !*playing {
                if queued < cushion {
                    continue;
                }
                *playing = true;
            }
            if queued > HIGH_WATER.max(2 * cushion) {
                discard(ring, queued - cushion);
                queued = ring.slots();
                self.shared.skips.fetch_add(1, Relaxed);
            }
            let take = queued.min(wanted);
            if let Ok(chunk) = ring.read_chunk(take) {
                if !deafened && !slot.muted.load(Relaxed) {
                    // The leveler measures this block (before any gain) and applies the
                    // gain it derived from the blocks before — one pass over the chunk.
                    let auto = dsp::db_to_gain(leveler.gain_db());
                    let manual = f32::from_bits(slot.gain.load(Relaxed));
                    let gain = auto * manual;
                    let mut energy = 0.0f32;
                    let (head, tail) = chunk.as_slices();
                    for (o, s) in out.iter_mut().zip(head.iter().chain(tail)) {
                        let x = f32::from(*s) * NORM;
                        energy += x * x;
                        *o += x * gain;
                    }
                    leveler.step(energy / take as f32, take);
                    slot.leveler_db.store(leveler.gain_db().to_bits(), Relaxed);
                }
                chunk.commit_all();
            }
            if take < wanted {
                *playing = false;
                self.shared.underruns.fetch_add(1, Relaxed);
            }
        }
        self.render_monitor(out, deafened);
        let master = f32::from_bits(self.shared.master.load(Relaxed));
        if master != 1.0 {
            for o in out.iter_mut() {
                *o *= master;
            }
        }
        self.limiter.process(out);
    }

    /// Adds the own send chain's output (A5) into `out` while the monitor is on. Its own
    /// slot kind: no leveler, no per-person volume, and its own cushion — the pump delivers
    /// 10 ms blocks, so one block more than the callback asks for is enough to never run
    /// dry between two deliveries. Off, the ring is drained so switching on again starts
    /// fresh. Real-time safe like [`MixerRt::render`].
    fn render_monitor(&mut self, out: &mut [f32], deafened: bool) {
        let monitor = &self.shared.monitor;
        let ring = &mut self.monitor;
        if !monitor.on.load(Relaxed) {
            if self.monitor_playing || ring.slots() > 0 {
                discard(ring, ring.slots());
                self.monitor_playing = false;
            }
            return;
        }
        let wanted = out.len();
        let mut queued = ring.slots();
        let cushion = PREFILL.max(wanted + BLOCK).min(MONITOR_RING / 2);
        if !self.monitor_playing {
            if queued < cushion {
                return;
            }
            self.monitor_playing = true;
        }
        if queued > 2 * cushion {
            // The microphone's clock runs ahead of the output's: trim back to the cushion
            // rather than letting the latency grow.
            discard(ring, queued - cushion);
            queued = ring.slots();
        }
        monitor.queued_max.fetch_max(queued as u32, Relaxed);
        let take = queued.min(wanted);
        if let Ok(chunk) = ring.read_chunk(take) {
            if !deafened {
                let (head, tail) = chunk.as_slices();
                for (o, s) in out.iter_mut().zip(head.iter().chain(tail)) {
                    *o += *s;
                }
            }
            chunk.commit_all();
        }
        if take < wanted {
            self.monitor_playing = false;
            monitor.underruns.fetch_add(1, Relaxed);
        }
    }
}

/// i16 → -1..1.
const NORM: f32 = 1.0 / i16::MAX as f32;

fn discard<T: Copy>(ring: &mut Consumer<T>, n: usize) {
    if let Ok(chunk) = ring.read_chunk(n.min(ring.slots())) {
        chunk.commit_all();
    }
}

impl MixerControl {
    /// A sink for one remote speaker's frames, or `None` when all slots are taken. Dropping
    /// the sink frees the slot.
    pub fn speaker(self: &Arc<Self>, identity: &str) -> Option<SpeakerSink> {
        let mut routing = self.routing.lock().unwrap();
        let (index, producer) = routing.free.pop()?;
        let mix = routing.peers.get(identity).copied().unwrap_or_default();
        let slot = &self.shared.slots[index];
        slot.gain.store(mix.gain.to_bits(), Relaxed);
        slot.muted.store(mix.muted, Relaxed);
        slot.level.store(0, Relaxed);
        slot.flush.store(true, Relaxed);
        routing.active.insert(identity.to_string(), index);
        Some(SpeakerSink {
            mixer: self.clone(),
            identity: identity.to_string(),
            slot: index,
            producer: Some(producer),
        })
    }

    /// Changes one person's volume or mute — now, and for the next time they speak.
    pub fn set_peer(&self, identity: &str, change: impl FnOnce(&mut PeerMix)) {
        let mut routing = self.routing.lock().unwrap();
        let mix = routing.peers.entry(identity.to_string()).or_default();
        change(mix);
        let mix = *mix;
        if let Some(&index) = routing.active.get(identity) {
            self.shared.slots[index]
                .gain
                .store(mix.gain.to_bits(), Relaxed);
            self.shared.slots[index].muted.store(mix.muted, Relaxed);
        }
    }

    /// Master volume of the whole mix, dB.
    pub fn set_master_db(&self, db: f32) {
        self.shared
            .master
            .store(dsp::db_to_gain(db).to_bits(), Relaxed);
    }

    pub fn set_deafened(&self, deafened: bool) {
        self.shared.deafened.store(deafened, Relaxed);
    }

    /// Loudness levelling per speaker on or off (A4). Off, the gains slide back to 0 dB.
    pub fn set_leveling(&self, on: bool) {
        self.shared.leveling.store(on, Relaxed);
    }

    /// The leveler's current gain for everyone speaking, as (identity, dB) — the diagnosis
    /// reads "too quiet" off it.
    pub fn leveler_gains(&self) -> Vec<(String, f32)> {
        let routing = self.routing.lock().unwrap();
        routing
            .active
            .iter()
            .map(|(identity, &index)| {
                let db = f32::from_bits(self.shared.slots[index].leveler_db.load(Relaxed));
                (identity.clone(), db)
            })
            .collect()
    }

    /// Current level of everyone speaking, as (identity, RMS 0..1).
    pub fn levels(&self) -> Vec<(String, f32)> {
        let routing = self.routing.lock().unwrap();
        routing
            .active
            .iter()
            .map(|(identity, &index)| {
                let level = f32::from_bits(self.shared.slots[index].level.load(Relaxed));
                (identity.clone(), level)
            })
            .collect()
    }

    /// Times a speaker's ring ran dry while playing.
    pub fn underruns(&self) -> u32 {
        self.shared.underruns.load(Relaxed)
    }

    /// Times the callback jumped forward because a ring was over the high-water mark.
    pub fn skips(&self) -> u32 {
        self.shared.skips.load(Relaxed)
    }

    /// Frames a sink could not queue because its ring was full.
    pub fn dropped_frames(&self) -> u32 {
        self.shared.dropped_frames.load(Relaxed)
    }

    // ── monitor (A5) ────────────────────────────────────────────────────────────────────

    /// The write side of the monitor slot, for the pump. There is one; the second call
    /// returns `None`.
    pub fn monitor_feed(self: &Arc<Self>) -> Option<MonitorFeed> {
        let producer = self.monitor_feed.lock().unwrap().take()?;
        Some(MonitorFeed {
            shared: self.shared.clone(),
            producer,
        })
    }

    /// „Mich selbst hören“ on or off. On, the pump starts feeding the next block and the
    /// callback plays once the cushion is there; off, both stop at once and the callback
    /// empties the ring on its next run, so switching on again starts fresh. (No flush flag
    /// on the way in: the pump's first block would race it and be thrown away.)
    pub fn set_monitor(&self, on: bool) {
        let monitor = &self.shared.monitor;
        if on && !monitor.on.load(Relaxed) {
            monitor.queued_max.store(0, Relaxed);
        }
        monitor.on.store(on, Relaxed);
    }

    pub fn monitor_on(&self) -> bool {
        self.shared.monitor.on.load(Relaxed)
    }

    /// The most samples the callback found in the monitor ring since the last call — the
    /// ring's share of the monitor latency, in samples at 48 kHz.
    pub fn take_monitor_queued_max(&self) -> u32 {
        self.shared.monitor.queued_max.swap(0, Relaxed)
    }

    /// Times the monitor ran dry while playing (total).
    pub fn monitor_underruns(&self) -> u32 {
        self.shared.monitor.underruns.load(Relaxed)
    }

    /// Blocks the pump could not queue for the monitor because its ring was full (total).
    pub fn monitor_dropped_blocks(&self) -> u32 {
        self.shared.monitor.dropped_blocks.load(Relaxed)
    }
}

/// The pump's end of the monitor slot (A5): one block after the send chain, as often as the
/// chain runs. Does nothing while the monitor is off, so the pump can call it every block.
pub struct MonitorFeed {
    shared: Arc<Shared>,
    producer: Producer<f32>,
}

impl MonitorFeed {
    pub fn block(&mut self, samples: &[f32]) {
        if !self.shared.monitor.on.load(Relaxed) {
            return;
        }
        if self.producer.push_entire_slice(samples).is_err() {
            self.shared.monitor.dropped_blocks.fetch_add(1, Relaxed);
        }
    }
}

/// The producer side of one slot, owned by whoever delivers that speaker's audio.
pub struct SpeakerSink {
    mixer: Arc<MixerControl>,
    identity: String,
    slot: usize,
    producer: Option<Producer<i16>>,
}

impl FrameSink for SpeakerSink {
    fn frame(&mut self, samples: &[i16]) {
        let shared = &self.mixer.shared;
        let energy: f64 = samples.iter().map(|s| f64::from(*s).powi(2)).sum();
        let rms = (energy / samples.len().max(1) as f64).sqrt() / f64::from(i16::MAX);
        shared.slots[self.slot]
            .level
            .store((rms as f32).to_bits(), Relaxed);
        let producer = self.producer.as_mut().expect("present until drop");
        if producer.push_entire_slice(samples).is_err() {
            shared.dropped_frames.fetch_add(1, Relaxed);
        }
    }
}

impl Drop for SpeakerSink {
    fn drop(&mut self) {
        let mut routing = self.mixer.routing.lock().unwrap();
        // The same person may already speak through a newer slot (resubscribe).
        if routing.active.get(&self.identity) == Some(&self.slot) {
            routing.active.remove(&self.identity);
        }
        self.mixer.shared.slots[self.slot].level.store(0, Relaxed);
        if let Some(producer) = self.producer.take() {
            routing.free.push((self.slot, producer));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const HALF: i16 = i16::MAX / 2;

    fn render(rt: &mut MixerRt, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        rt.render(&mut out);
        out
    }

    #[test]
    fn a_speaker_starts_after_the_prefill_and_not_before() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK); // consumes the flush of a fresh slot
        anna.frame(&[HALF; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "10 ms queued: wait"
        );
        anna.frame(&[HALF; BLOCK]);
        let out = render(&mut rt, BLOCK);
        assert!(out.iter().all(|s| (*s - 0.5).abs() < 1e-3));
        assert_eq!(control.underruns(), 0);
    }

    #[test]
    fn speakers_are_summed_with_their_gain_and_the_sum_is_limited() {
        let (control, mut rt) = mixer();
        control.set_peer("ben", |m| m.gain = 0.5);
        let mut anna = control.speaker("anna").unwrap();
        let mut ben = control.speaker("ben").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..2 {
            anna.frame(&[HALF; BLOCK]);
            ben.frame(&[HALF; BLOCK]);
        }
        assert!(render(&mut rt, BLOCK)
            .iter()
            .all(|s| (*s - 0.75).abs() < 1e-3));
        control.set_peer("ben", |m| m.gain = 4.0);
        let ceiling = dsp::db_to_gain(CEILING_DB);
        assert!(
            render(&mut rt, BLOCK)
                .iter()
                .all(|s| (*s - ceiling).abs() < 1e-4),
            "held at the limiter's ceiling, not clipped"
        );
    }

    #[test]
    fn the_master_volume_scales_the_whole_mix() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..3 {
            anna.frame(&[HALF; BLOCK]);
        }
        control.set_master_db(-6.0);
        let out = render(&mut rt, BLOCK);
        assert!(out.iter().all(|s| (*s - 0.25).abs() < 2e-3), "{}", out[0]);
    }

    #[test]
    fn peer_mute_and_deafen_silence_but_keep_draining() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..4 {
            anna.frame(&[HALF; BLOCK]);
        }
        control.set_peer("anna", |m| m.muted = true);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s == 0.0));
        control.set_peer("anna", |m| m.muted = false);
        control.set_deafened(true);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s == 0.0));
        control.set_deafened(false);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s > 0.4));
    }

    #[test]
    fn running_dry_counts_once_and_waits_for_a_new_prefill() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..3 {
            anna.frame(&[HALF; BLOCK]);
        }
        render(&mut rt, BLOCK); // playing now, two blocks left
        let out = render(&mut rt, 3 * BLOCK);
        assert!(out[..2 * BLOCK].iter().all(|s| *s > 0.4));
        assert!(out[2 * BLOCK..].iter().all(|s| *s == 0.0));
        assert_eq!(control.underruns(), 1);
        anna.frame(&[HALF; BLOCK]);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s == 0.0));
        assert_eq!(control.underruns(), 1, "not playing: no second underrun");
    }

    #[test]
    fn a_backlog_over_the_high_water_mark_is_skipped() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..12 {
            anna.frame(&[HALF; BLOCK]);
        }
        render(&mut rt, BLOCK);
        assert_eq!(control.skips(), 1);
        // It jumped to the cushion (two blocks) and played one: one block is left.
        render(&mut rt, BLOCK);
        assert_eq!(control.underruns(), 0);
        render(&mut rt, BLOCK);
        assert_eq!(control.underruns(), 1);
    }

    /// A device that asks for more than the 20 ms prefill at a time (the Focusrite here:
    /// 1056 frames) must not run dry on every callback.
    #[test]
    fn a_callback_larger_than_the_prefill_does_not_underrun() {
        const CALLBACK: usize = 1056;
        let (control, mut rt) = mixer();
        control.set_leveling(false); // a steady −6 dBFS would be levelled down below 0.4
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, CALLBACK);
        let mut heard = 0;
        // 300 callbacks ≈ 6.6 s; network frames arrive every 480 samples of that time.
        for callback in 0..300 {
            let (before, after) = (
                callback * CALLBACK / BLOCK,
                (callback + 1) * CALLBACK / BLOCK,
            );
            for _ in before..after {
                anna.frame(&[HALF; BLOCK]);
            }
            heard += render(&mut rt, CALLBACK)
                .iter()
                .filter(|s| **s > 0.4)
                .count();
        }
        assert_eq!(control.underruns(), 0);
        assert_eq!(control.skips(), 0);
        assert!(
            heard > 290 * CALLBACK,
            "plays almost from the start: {heard}"
        );
    }

    /// Feeds `seconds` of a steady signal to one prefilled speaker, block by block, and
    /// returns the last rendered block.
    fn talk(rt: &mut MixerRt, sink: &mut SpeakerSink, sample: i16, seconds: f32) -> Vec<f32> {
        let mut out = Vec::new();
        for _ in 0..(seconds * 100.0) as usize {
            sink.frame(&[sample; BLOCK]);
            out = render(rt, BLOCK);
        }
        out
    }

    /// A4: a quiet person is brought up towards −20 dBFS over a few seconds, the manual
    /// volume still sits on top, and the diagnosis sees the gain.
    #[test]
    fn a_quiet_speaker_is_levelled_up_and_the_manual_volume_stays_an_offset() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        anna.frame(&[0; BLOCK]);
        anna.frame(&[0; BLOCK]);
        let quiet = (f32::from(i16::MAX) * dsp::db_to_gain(-32.0)) as i16; // −32 dBFS
        let first = talk(&mut rt, &mut anna, quiet, 0.05)[0];
        assert!(
            (first - dsp::db_to_gain(-32.0)).abs() < 2e-3,
            "starts at unity: {first}"
        );
        let later = talk(&mut rt, &mut anna, quiet, 6.0)[0];
        let gain_db = dsp::gain_to_db(later / dsp::db_to_gain(-32.0));
        assert!(
            (gain_db - 12.0).abs() < 1.0,
            "+12 dB after 6 s: {gain_db:.1}"
        );
        let gains = control.leveler_gains();
        assert_eq!(gains.len(), 1);
        assert_eq!(gains[0].0, "anna");
        assert!(
            (gains[0].1 - gain_db).abs() < 0.2,
            "reported {}",
            gains[0].1
        );
        control.set_peer("anna", |m| m.gain = 0.5);
        let halved = talk(&mut rt, &mut anna, quiet, 0.05)[0];
        assert!(
            (halved - later / 2.0).abs() < 2e-3,
            "manual −6 dB on top: {halved}"
        );
    }

    #[test]
    fn switching_leveling_off_slides_back_to_unity_and_a_new_speaker_starts_fresh() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        anna.frame(&[0; BLOCK]);
        anna.frame(&[0; BLOCK]);
        let quiet = (f32::from(i16::MAX) * dsp::db_to_gain(-32.0)) as i16;
        talk(&mut rt, &mut anna, quiet, 6.0);
        assert!(control.leveler_gains()[0].1 > 10.0);
        control.set_leveling(false);
        talk(&mut rt, &mut anna, quiet, 1.0);
        let on_the_way = control.leveler_gains()[0].1;
        assert!(
            on_the_way > 0.0 && on_the_way < 8.0,
            "slides, no jump: {on_the_way}"
        );
        talk(&mut rt, &mut anna, quiet, 3.0);
        assert_eq!(control.leveler_gains()[0].1, 0.0);
        control.set_leveling(true);
        talk(&mut rt, &mut anna, quiet, 2.0);
        assert!(control.leveler_gains()[0].1 > 3.0);
        drop(anna);
        let mut ben = control.speaker("ben").unwrap();
        render(&mut rt, BLOCK); // the flush resets the slot's leveler
        ben.frame(&[0; BLOCK]);
        assert_eq!(control.leveler_gains(), vec![("ben".to_string(), 0.0)]);
    }

    #[test]
    fn silence_between_words_does_not_pump_the_gain_up() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        anna.frame(&[0; BLOCK]);
        anna.frame(&[0; BLOCK]);
        talk(&mut rt, &mut anna, 0, 5.0);
        assert_eq!(control.leveler_gains()[0].1, 0.0, "silence: untouched");
        talk(&mut rt, &mut anna, 10, 5.0); // ≈ −70 dBFS: room noise, under the floor
        assert_eq!(control.leveler_gains()[0].1, 0.0);
    }

    #[test]
    fn a_full_ring_drops_the_frame_and_counts_it() {
        let (control, _rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        for _ in 0..21 {
            anna.frame(&[HALF; BLOCK]);
        }
        assert_eq!(control.dropped_frames(), 1);
    }

    #[test]
    fn a_reused_slot_does_not_play_the_previous_speaker() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        anna.frame(&[HALF; BLOCK]); // below the prefill: stays in the ring
        drop(anna);
        let mut ben = control.speaker("ben").unwrap();
        ben.frame(&[-HALF; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "flushed, ben not prefilled"
        );
        ben.frame(&[-HALF; BLOCK]);
        ben.frame(&[-HALF; BLOCK]);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s < -0.4), "only ben");
    }

    #[test]
    fn slots_come_back_and_levels_follow_the_frames() {
        let (control, _rt) = mixer();
        let sinks: Vec<_> = (0..SLOTS)
            .map(|i| control.speaker(&format!("p{i}")).unwrap())
            .collect();
        assert!(control.speaker("one-too-many").is_none());
        drop(sinks);
        let mut anna = control.speaker("anna").unwrap();
        anna.frame(&[HALF; BLOCK]);
        let levels = control.levels();
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].0, "anna");
        assert!((levels[0].1 - 0.5).abs() < 1e-3);
        drop(anna);
        assert!(control.levels().is_empty());
    }

    #[test]
    fn volume_survives_a_resubscribe() {
        let (control, mut rt) = mixer();
        control.set_peer("anna", |m| m.gain = 0.5);
        drop(control.speaker("anna").unwrap());
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        anna.frame(&[HALF; BLOCK]);
        anna.frame(&[HALF; BLOCK]);
        assert!(render(&mut rt, BLOCK)
            .iter()
            .all(|s| (*s - 0.25).abs() < 1e-3));
    }

    // ── monitor (A5) ────────────────────────────────────────────────────────────────────

    #[test]
    fn the_monitor_is_silent_off_and_plays_the_chain_output_on() {
        let (control, mut rt) = mixer();
        let mut feed = control.monitor_feed().unwrap();
        assert!(control.monitor_feed().is_none(), "one feed, for the pump");
        feed.block(&[0.5; BLOCK]);
        feed.block(&[0.5; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "off: nothing queued"
        );
        assert_eq!(control.monitor_dropped_blocks(), 0);
        control.set_monitor(true);
        assert!(control.monitor_on());
        feed.block(&[0.5; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "one block: cushion first"
        );
        feed.block(&[0.5; BLOCK]);
        let out = render(&mut rt, BLOCK);
        assert!(
            out.iter().all(|s| (*s - 0.5).abs() < 1e-6),
            "unity, no leveler: {}",
            out[0]
        );
        assert_eq!(control.take_monitor_queued_max(), 2 * BLOCK as u32);
        assert_eq!(control.take_monitor_queued_max(), 0, "taken");
        assert_eq!(control.monitor_underruns(), 0);
    }

    #[test]
    fn the_monitor_sums_with_the_speakers_and_follows_master_and_deafen() {
        let (control, mut rt) = mixer();
        let mut feed = control.monitor_feed().unwrap();
        let mut anna = control.speaker("anna").unwrap();
        control.set_monitor(true);
        render(&mut rt, BLOCK);
        for _ in 0..3 {
            anna.frame(&[HALF; BLOCK]);
            feed.block(&[0.25; BLOCK]);
        }
        let out = render(&mut rt, BLOCK);
        assert!(out.iter().all(|s| (*s - 0.75).abs() < 1e-3), "{}", out[0]);
        control.set_master_db(-6.0);
        let out = render(&mut rt, BLOCK);
        assert!(out.iter().all(|s| (*s - 0.375).abs() < 3e-3), "{}", out[0]);
        control.set_deafened(true);
        feed.block(&[0.25; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "deaf: silent"
        );
        control.set_deafened(false);
        assert_eq!(
            control.monitor_underruns(),
            0,
            "drained while deaf, not dry"
        );
    }

    #[test]
    fn switching_the_monitor_off_stops_it_at_once_and_on_again_starts_fresh() {
        let (control, mut rt) = mixer();
        let mut feed = control.monitor_feed().unwrap();
        control.set_monitor(true);
        for _ in 0..5 {
            feed.block(&[0.5; BLOCK]);
        }
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s > 0.4));
        control.set_monitor(false);
        feed.block(&[0.5; BLOCK]); // the pump may be a block behind the switch
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "off: silent now"
        );
        control.set_monitor(true);
        feed.block(&[-0.5; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "the old blocks are gone, the cushion is not there yet"
        );
        feed.block(&[-0.5; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s < -0.4),
            "only the new"
        );
    }

    #[test]
    fn the_monitor_trims_a_backlog_and_counts_running_dry() {
        let (control, mut rt) = mixer();
        let mut feed = control.monitor_feed().unwrap();
        control.set_monitor(true);
        for _ in 0..8 {
            feed.block(&[0.5; BLOCK]);
        }
        render(&mut rt, BLOCK);
        assert_eq!(
            control.take_monitor_queued_max(),
            2 * BLOCK as u32,
            "trimmed to the cushion before reading"
        );
        render(&mut rt, BLOCK);
        assert_eq!(control.monitor_underruns(), 0);
        let out = render(&mut rt, BLOCK);
        assert!(out.iter().all(|s| *s == 0.0));
        assert_eq!(control.monitor_underruns(), 1);
        feed.block(&[0.5; BLOCK]);
        render(&mut rt, BLOCK);
        assert_eq!(
            control.monitor_underruns(),
            1,
            "waits for the cushion, no second one"
        );
        // 20 blocks fit, one is still waiting: of 21 more, two are dropped and counted.
        for _ in 0..21 {
            feed.block(&[0.5; BLOCK]);
        }
        assert_eq!(control.monitor_dropped_blocks(), 2);
    }

    /// A device that asks for 2048 frames at a time (macOS, Bluetooth, exclusive mode) must
    /// still hear the monitor and still get the drift trim.
    #[test]
    fn the_monitor_works_with_a_large_output_callback() {
        const CALLBACK: usize = 2048;
        let (control, mut rt) = mixer();
        let mut feed = control.monitor_feed().unwrap();
        control.set_monitor(true);
        let mut heard = 0;
        for callback in 0..200 {
            let (before, after) = (
                callback * CALLBACK / BLOCK,
                (callback + 1) * CALLBACK / BLOCK,
            );
            for _ in before..after {
                feed.block(&[0.5; BLOCK]);
            }
            heard += render(&mut rt, CALLBACK)
                .iter()
                .filter(|s| **s > 0.4)
                .count();
        }
        assert_eq!(control.monitor_dropped_blocks(), 0);
        assert_eq!(control.monitor_underruns(), 0);
        assert!(
            heard > 190 * CALLBACK,
            "plays almost from the start: {heard}"
        );
        // A backlog is trimmed even though 2 × cushion would exceed the ring.
        for _ in 0..20 {
            feed.block(&[0.5; BLOCK]);
        }
        render(&mut rt, CALLBACK);
        assert!(control.take_monitor_queued_max() <= (MONITOR_RING / 2) as u32);
    }

    proptest! {
        /// Whatever arrives in whatever rhythm: the output stays in range and nothing panics.
        #[test]
        fn output_stays_in_range(
            steps in prop::collection::vec((0usize..4, 1usize..2000, any::<i16>(), 0f32..8.0), 1..60)
        ) {
            let (control, mut rt) = mixer();
            let mut sinks = vec![control.speaker("a").unwrap(), control.speaker("b").unwrap()];
            let mut feed = control.monitor_feed().unwrap();
            control.set_monitor(true);
            for (frames, wanted, sample, gain) in steps {
                control.set_peer("a", |m| m.gain = gain);
                for sink in &mut sinks {
                    for _ in 0..frames {
                        sink.frame(&[sample; BLOCK]);
                    }
                }
                for _ in 0..frames {
                    feed.block(&[f32::from(sample) * NORM * gain; BLOCK]);
                }
                let out = render(&mut rt, wanted);
                prop_assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)));
            }
        }
    }
}
