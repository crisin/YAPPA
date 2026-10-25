//! The receive mix: one ring per remote speaker, summed in the output callback.
//!
//! Two halves. [`MixerRt`] lives in the real-time callback and only touches `rtrb` rings and
//! atomics. [`MixerControl`] is everything else: handing rings to speakers, volume, mute,
//! levels. The number of speakers is fixed up front so the callback never allocates.
//!
//! Per slot a [`Leveler`] (A4) brings every voice to about the same level before the manual
//! per-person volume is applied on top; the sum then passes the limiter.
//!
//! Per slot a [`DriftController`] with a [`Varispeed`] (A6) holds the ring at its cushion:
//! the speaker's clock and our output device drift apart by a few hundred ppm, so the ring
//! is played a hair faster or slower instead of being skipped by 80 ms every few minutes.
//! The skip stays as the emergency brake for a backlog of more than two cushions.

use dsp::{DriftController, DriftParams, Leveler, LevelerParams, Limiter, Varispeed};
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
    /// The drift compensation's playback ratio (inputs per output, ≈ 1.0) as `f32` bits.
    drift_ratio: AtomicU32,
    /// The ring's averaged fill level in samples as `f32` bits.
    fill: AtomicU32,
}

/// What the diagnosis shows per speaker (A4, A6).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SlotReport {
    /// The leveler's gain, dB.
    pub leveler_db: f32,
    /// The drift compensation's playback ratio, ≈ 1.0 (1.0 while the slot is silent).
    pub drift_ratio: f32,
    /// The ring's averaged fill level, ms (0 while the slot is silent).
    pub fill_ms: f32,
}

/// The limiter at the end of the mix: 1 dB below full scale, recovers within 80 ms.
const CEILING_DB: f32 = -1.0;
const RELEASE_MS: f32 = 80.0;

#[derive(Default)]
struct Shared {
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
}

pub struct MixerRt {
    shared: Arc<Shared>,
    rings: Vec<Consumer<i16>>,
    playing: [bool; SLOTS],
    /// One per slot, allocated here — never in the callback.
    levelers: Vec<Leveler>,
    drift: Vec<DriftController>,
    varispeed: Vec<Varispeed>,
    limiter: Limiter,
}

pub fn mixer() -> (Arc<MixerControl>, MixerRt) {
    let shared = Arc::new(Shared::default());
    shared.master.store(1f32.to_bits(), Relaxed);
    shared.leveling.store(true, Relaxed);
    let mut free = Vec::with_capacity(SLOTS);
    let mut rings = Vec::with_capacity(SLOTS);
    let mut levelers = Vec::with_capacity(SLOTS);
    let mut drift = Vec::with_capacity(SLOTS);
    let mut varispeed = Vec::with_capacity(SLOTS);
    let sample_rate = transport::SAMPLE_RATE as f32;
    for index in 0..SLOTS {
        let (producer, consumer) = RingBuffer::new(RING);
        shared.slots[index].gain.store(1f32.to_bits(), Relaxed);
        shared.slots[index]
            .drift_ratio
            .store(1f32.to_bits(), Relaxed);
        free.push((index, producer));
        rings.push(consumer);
        levelers.push(Leveler::new(sample_rate, LevelerParams::default()));
        drift.push(DriftController::new(sample_rate, DriftParams::default()));
        varispeed.push(Varispeed::new());
    }
    free.reverse(); // hand out slot 0 first
    let control = Arc::new(MixerControl {
        shared: shared.clone(),
        routing: Mutex::new(Routing {
            free,
            active: HashMap::new(),
            peers: HashMap::new(),
        }),
    });
    let rt = MixerRt {
        shared,
        rings,
        playing: [false; SLOTS],
        levelers,
        drift,
        varispeed,
        limiter: Limiter::new(sample_rate, CEILING_DB, RELEASE_MS),
    };
    (control, rt)
}

impl MixerRt {
    /// Adds every speaker into `out` (mono, -1..1), which must come in zeroed — each through
    /// their drift compensation, their leveler and their manual volume — applies the master
    /// volume and limits the sum below clipping — 19 people laughing at once used to clip
    /// hard (realtime review 2026-10-23, item c). Real-time safe: no allocation, no lock, no
    /// system call.
    pub fn render(&mut self, out: &mut [f32]) {
        let wanted = out.len();
        let deafened = self.shared.deafened.load(Relaxed);
        let leveling = self.shared.leveling.load(Relaxed);
        for (index, ring) in self.rings.iter_mut().enumerate() {
            let slot = &self.shared.slots[index];
            let playing = &mut self.playing[index];
            let leveler = &mut self.levelers[index];
            let drift = &mut self.drift[index];
            let varispeed = &mut self.varispeed[index];
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
                    if drift.fill() != 0.0 {
                        // Fell silent: the diagnosis shows a resting slot, not stale values.
                        drift.reset();
                        slot.drift_ratio.store(1f32.to_bits(), Relaxed);
                        slot.fill.store(0f32.to_bits(), Relaxed);
                    }
                    continue;
                }
                *playing = true;
                drift.reset();
                varispeed.reset();
            }
            if queued > HIGH_WATER.max(2 * cushion) {
                // The emergency brake: more than two cushions is beyond what the ratio can
                // work off in reasonable time (a stall, a burst after a reconnect).
                discard(ring, queued - cushion);
                queued = ring.slots();
                self.shared.skips.fetch_add(1, Relaxed);
                drift.reset();
            }
            // A6: hold the ring in the middle of the start window. The ratio changes once per
            // callback, by at most the controller's gain times a frame of error — no click.
            let ratio = drift.step(queued, cushion + BLOCK / 2, wanted);
            varispeed.set_ratio(ratio);
            slot.drift_ratio.store((ratio as f32).to_bits(), Relaxed);
            slot.fill.store(drift.fill().to_bits(), Relaxed);

            let silent = deafened || slot.muted.load(Relaxed);
            // The leveler measures this block (before any gain) and applies the gain it
            // derived from the blocks before — one pass over the ring chunk.
            let auto = dsp::db_to_gain(leveler.gain_db());
            let manual = f32::from_bits(slot.gain.load(Relaxed));
            let gain = auto * manual;
            let mut energy = 0.0f32;
            let mut consumed = 0usize;
            let mut produced = 0usize;
            if let Ok(chunk) = ring.read_chunk(queued) {
                let (head, tail) = chunk.as_slices();
                let mut inputs = head.iter().chain(tail.iter());
                let mut outputs = out.iter_mut();
                produced = varispeed.produce(
                    wanted,
                    || {
                        let s = inputs.next()?;
                        consumed += 1;
                        Some(f32::from(*s) * NORM)
                    },
                    |y| {
                        if let Some(o) = outputs.next() {
                            if !silent {
                                energy += y * y;
                                *o += y * gain;
                            }
                        }
                    },
                );
                chunk.commit(consumed);
            }
            if !silent && produced > 0 {
                leveler.step(energy / produced as f32, produced);
                slot.leveler_db.store(leveler.gain_db().to_bits(), Relaxed);
            }
            if produced < wanted {
                *playing = false;
                self.shared.underruns.fetch_add(1, Relaxed);
            }
        }
        let master = f32::from_bits(self.shared.master.load(Relaxed));
        if master != 1.0 {
            for o in out.iter_mut() {
                *o *= master;
            }
        }
        self.limiter.process(out);
    }
}

/// i16 → -1..1.
const NORM: f32 = 1.0 / i16::MAX as f32;

fn discard(ring: &mut Consumer<i16>, n: usize) {
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

    /// Everything the diagnosis shows per speaker: the leveler's gain (A4), the drift
    /// compensation's ratio and the ring's fill level (A6).
    pub fn speakers(&self) -> Vec<(String, SlotReport)> {
        let routing = self.routing.lock().unwrap();
        let per_ms = transport::SAMPLE_RATE as f32 / 1000.0;
        routing
            .active
            .iter()
            .map(|(identity, &index)| {
                let slot = &self.shared.slots[index];
                let report = SlotReport {
                    leveler_db: f32::from_bits(slot.leveler_db.load(Relaxed)),
                    drift_ratio: f32::from_bits(slot.drift_ratio.load(Relaxed)),
                    fill_ms: f32::from_bits(slot.fill.load(Relaxed)) / per_ms,
                };
                (identity.clone(), report)
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

    /// Times the callback jumped forward because a ring was over the high-water mark — the
    /// emergency brake behind the drift compensation; in a healthy call this stays 0.
    pub fn skips(&self) -> u32 {
        self.shared.skips.load(Relaxed)
    }

    /// Frames a sink could not queue because its ring was full.
    pub fn dropped_frames(&self) -> u32 {
        self.shared.dropped_frames.load(Relaxed)
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
        // The ring sits far under the cushion of this big callback, so the drift
        // compensation already stretches by up to 0.5 %: a few samples more come out.
        assert!(out[2 * BLOCK + 8..].iter().all(|s| *s == 0.0));
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

    /// A6 with a fake clock: the sender's clock runs `ppm` off ours, so over `seconds` it
    /// delivers that many more (or fewer) frames than we play. Frames arrive whole, like
    /// from the network; the callback asks for `callback` samples at a time.
    fn drifting_call(ppm: f64, callback: usize, seconds: usize) -> (Arc<MixerControl>, u32, u32) {
        let (control, mut rt) = mixer();
        control.set_leveling(false);
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, callback);
        let mut arrived = 2.0 * BLOCK as f64; // the prefill
        let mut heard = 0usize;
        let callbacks = seconds * 48_000 / callback;
        for _ in 0..callbacks {
            arrived += callback as f64 * (1.0 + ppm * 1e-6);
            while arrived >= BLOCK as f64 {
                arrived -= BLOCK as f64;
                anna.frame(&[HALF; BLOCK]);
            }
            heard += render(&mut rt, callback)
                .iter()
                .filter(|s| **s > 0.4)
                .count();
        }
        assert!(
            heard > (callbacks - 2) * callback,
            "plays throughout: {heard} of {}",
            callbacks * callback
        );
        let (underruns, skips) = (control.underruns(), control.skips());
        drop(anna);
        (control, underruns, skips)
    }

    /// A sender 200 ppm fast used to overflow the high-water mark every ≈ 7 minutes and be
    /// skipped by 80 ms; 200 ppm slow it ran dry. Now the ring is held at the cushion for
    /// ten minutes with neither — and the diagnosis sees the ratio.
    #[test]
    fn a_drifting_speaker_is_neither_skipped_nor_run_dry_over_ten_minutes() {
        for ppm in [200.0, -200.0] {
            let (control, mut rt) = mixer();
            control.set_leveling(false);
            let mut anna = control.speaker("anna").unwrap();
            render(&mut rt, BLOCK);
            let mut arrived = 2.0 * BLOCK as f64;
            let mut reported = (0.0f32, 0.0f32);
            for callback in 0..600 * 100 {
                arrived += BLOCK as f64 * (1.0 + ppm * 1e-6);
                while arrived >= BLOCK as f64 {
                    arrived -= BLOCK as f64;
                    anna.frame(&[HALF; BLOCK]);
                }
                let out = render(&mut rt, BLOCK);
                assert!(out.iter().all(|s| *s > 0.4), "{ppm} ppm: gap at {callback}");
                if callback == 400 * 100 {
                    let report = control.speakers();
                    assert_eq!(report[0].0, "anna");
                    reported = (report[0].1.drift_ratio, report[0].1.fill_ms);
                }
            }
            assert_eq!(control.skips(), 0, "{ppm} ppm");
            assert_eq!(control.underruns(), 0, "{ppm} ppm");
            // The ring sits within a frame of its target (cushion 20 ms + half a frame),
            // and the ratio carries the drift's sign.
            assert!(
                (reported.1 - 25.0).abs() < 10.0,
                "{ppm} ppm: fill {} ms",
                reported.1
            );
            assert!(
                (reported.0 - 1.0).signum() == (ppm as f32).signum(),
                "{ppm} ppm: ratio {}",
                reported.0
            );
            assert!((reported.0 - 1.0).abs() <= 0.005);
        }
    }

    /// The same with the Focusrite's odd callback size and a 16 kHz-device-sized one.
    #[test]
    fn drift_is_held_for_other_callback_sizes_too() {
        for callback in [1056usize, 160] {
            let (_, underruns, skips) = drifting_call(150.0, callback, 300);
            assert_eq!((underruns, skips), (0, 0), "callback {callback}");
            let (_, underruns, skips) = drifting_call(-150.0, callback, 300);
            assert_eq!((underruns, skips), (0, 0), "callback {callback}");
        }
    }

    /// A resting slot reports unity and an empty ring, not the last call's values.
    #[test]
    fn the_report_rests_when_the_speaker_falls_silent() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..4 {
            anna.frame(&[HALF; BLOCK]);
        }
        render(&mut rt, BLOCK);
        assert!(control.speakers()[0].1.fill_ms > 30.0);
        for _ in 0..5 {
            render(&mut rt, BLOCK); // runs dry, then waits for a new prefill
        }
        let report = control.speakers()[0].1;
        assert_eq!((report.drift_ratio, report.fill_ms), (1.0, 0.0));
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

    proptest! {
        /// Whatever arrives in whatever rhythm: the output stays in range and nothing panics.
        #[test]
        fn output_stays_in_range(
            steps in prop::collection::vec((0usize..4, 1usize..2000, any::<i16>(), 0f32..8.0), 1..60)
        ) {
            let (control, mut rt) = mixer();
            let mut sinks = vec![control.speaker("a").unwrap(), control.speaker("b").unwrap()];
            for (frames, wanted, sample, gain) in steps {
                control.set_peer("a", |m| m.gain = gain);
                for sink in &mut sinks {
                    for _ in 0..frames {
                        sink.frame(&[sample; BLOCK]);
                    }
                }
                let out = render(&mut rt, wanted);
                prop_assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)));
            }
        }
    }
}
