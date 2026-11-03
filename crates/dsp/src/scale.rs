//! Musical scales for the autotune and the harmonizer (ADR-012, "Autotune und
//! Harmonizer"): which frequencies a voice may land on, given a key and a scale, and which
//! scale note sits a few steps above a given one. Pure arithmetic on frequencies — the
//! octave is 12 equal semitones, A4 = 440 Hz.

/// Which notes of the octave belong to the scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scale {
    /// Every semitone — snaps to the nearest note, no key needed.
    Chromatic,
    #[default]
    Major,
    NaturalMinor,
    HarmonicMinor,
    PentatonicMajor,
    PentatonicMinor,
    Blues,
}

impl Scale {
    /// The scale's notes as semitones above the key, ascending, within one octave.
    pub fn degrees(self) -> &'static [u8] {
        match self {
            Scale::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::NaturalMinor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Scale::PentatonicMajor => &[0, 2, 4, 7, 9],
            Scale::PentatonicMinor => &[0, 3, 5, 7, 10],
            Scale::Blues => &[0, 3, 5, 6, 7, 10],
        }
    }
}

/// The key as semitones above C: C = 0, C♯ = 1, … H = 11. Values above 11 wrap.
pub type Key = u8;

const A4_HZ: f32 = 440.0;
const A4_MIDI: f32 = 69.0;

/// Frequency to a fractional MIDI note number (69 = A4). Non-positive and non-finite
/// input maps to A4, so a bad detector value never produces NaN downstream.
pub fn hz_to_midi(hz: f32) -> f32 {
    if hz.is_finite() && hz > 0.0 {
        A4_MIDI + 12.0 * (hz / A4_HZ).log2()
    } else {
        A4_MIDI
    }
}

pub fn midi_to_hz(midi: f32) -> f32 {
    A4_HZ * 2f32.powf((midi - A4_MIDI) / 12.0)
}

/// Position of a note in the scale: which octave (relative to the key) and which degree
/// index. Returned by [`nearest`], consumed by [`interval`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Position {
    octave: i32,
    index: usize,
}

impl Position {
    fn midi(self, key: Key, scale: Scale) -> f32 {
        let degrees = scale.degrees();
        let len = degrees.len() as i32;
        // Step past the end of the degree list into the next octave (and back).
        let octave = self.octave + (self.index as i32).div_euclid(len);
        let index = (self.index as i32).rem_euclid(len) as usize;
        (key % 12) as f32 + 12.0 * octave as f32 + degrees[index] as f32
    }
}

/// The scale note closest (in semitones, so in pitch perception) to `midi`.
fn nearest(midi: f32, key: Key, scale: Scale) -> Position {
    let degrees = scale.degrees();
    let rel = midi - (key % 12) as f32;
    let octave = rel.div_euclid(12.0) as i32;
    let frac = rel.rem_euclid(12.0);
    let mut best = Position { octave, index: 0 };
    let mut best_dist = f32::INFINITY;
    // Candidates: every degree in this octave, plus the first degree an octave up and the
    // last degree an octave down, so the wrap at the octave is covered.
    for (index, &d) in degrees.iter().enumerate() {
        let dist = (frac - d as f32).abs();
        if dist < best_dist {
            best_dist = dist;
            best = Position { octave, index };
        }
    }
    let up = (frac - (degrees[0] as f32 + 12.0)).abs();
    if up < best_dist {
        best_dist = up;
        best = Position {
            octave: octave + 1,
            index: 0,
        };
    }
    let last = degrees.len() - 1;
    let down = (frac - (degrees[last] as f32 - 12.0)).abs();
    if down < best_dist {
        best = Position {
            octave: octave - 1,
            index: last,
        };
    }
    best
}

/// The frequency of the scale note nearest to `hz` — what the autotune pulls the voice to.
pub fn nearest_note(hz: f32, key: Key, scale: Scale) -> f32 {
    midi_to_hz(nearest(hz_to_midi(hz), key, scale).midi(key, scale))
}

/// The scale note `degrees` scale steps above (negative: below) the note nearest to `hz` —
/// what the harmonizer's second voice sings. In a diatonic scale 2 steps are a third and
/// 4 a fifth; 0 is the nearest note itself.
pub fn interval_note(hz: f32, key: Key, scale: Scale, degrees: i32) -> f32 {
    let base = nearest(hz_to_midi(hz), key, scale);
    let len = scale.degrees().len() as i32;
    let index = base.index as i32 + degrees;
    let shifted = Position {
        octave: base.octave + index.div_euclid(len),
        index: index.rem_euclid(len) as usize,
    };
    midi_to_hz(shifted.midi(key, scale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const ALL: [Scale; 7] = [
        Scale::Chromatic,
        Scale::Major,
        Scale::NaturalMinor,
        Scale::HarmonicMinor,
        Scale::PentatonicMajor,
        Scale::PentatonicMinor,
        Scale::Blues,
    ];

    /// Is `midi` (rounded) a note of the scale in this key?
    fn on_scale(midi: f32, key: Key, scale: Scale) -> bool {
        let rounded = midi.round();
        assert!((midi - rounded).abs() < 1e-3, "{midi} is not a whole note");
        let rel = (rounded as i32 - key as i32).rem_euclid(12) as u8;
        scale.degrees().contains(&rel)
    }

    #[test]
    fn a_440_in_c_major_stays_and_a_quarter_tone_off_snaps_back() {
        assert!((nearest_note(440.0, 0, Scale::Major) - 440.0).abs() < 1e-3);
        let sharp = midi_to_hz(69.4);
        assert!((nearest_note(sharp, 0, Scale::Major) - 440.0).abs() < 1e-3);
        // A♯ (70) is not in C major: it goes to A (69) or H (71), whichever is nearer.
        assert!((nearest_note(midi_to_hz(70.4), 0, Scale::Major) - midi_to_hz(71.0)).abs() < 1e-2);
        assert!((nearest_note(midi_to_hz(69.6), 0, Scale::Major) - midi_to_hz(69.0)).abs() < 1e-2);
    }

    #[test]
    fn the_key_moves_the_scale() {
        // In D major (key 2) C♯ (61) is a scale note, C (60) is not.
        assert!((nearest_note(midi_to_hz(61.0), 2, Scale::Major) - midi_to_hz(61.0)).abs() < 1e-2);
        assert!((nearest_note(midi_to_hz(60.2), 2, Scale::Major) - midi_to_hz(61.0)).abs() < 1e-2);
    }

    #[test]
    fn thirds_and_fifths_in_c_major() {
        let c4 = midi_to_hz(60.0);
        // E4 (64) is two scale steps above C4, G4 (67) four.
        assert!((interval_note(c4, 0, Scale::Major, 2) - midi_to_hz(64.0)).abs() < 1e-2);
        assert!((interval_note(c4, 0, Scale::Major, 4) - midi_to_hz(67.0)).abs() < 1e-2);
        // Seven steps are the octave, minus seven the octave below.
        assert!((interval_note(c4, 0, Scale::Major, 7) - midi_to_hz(72.0)).abs() < 1e-2);
        assert!((interval_note(c4, 0, Scale::Major, -7) - midi_to_hz(48.0)).abs() < 1e-2);
        // Over the end of the octave: H4 (71) plus a third is D5 (74).
        assert!(
            (interval_note(midi_to_hz(71.0), 0, Scale::Major, 2) - midi_to_hz(74.0)).abs() < 1e-2
        );
        // A minor third in C natural minor: E♭ (63).
        assert!((interval_note(c4, 0, Scale::NaturalMinor, 2) - midi_to_hz(63.0)).abs() < 1e-2);
    }

    #[test]
    fn bad_input_lands_on_a_note_instead_of_nan() {
        for hz in [0.0, -5.0, f32::NAN, f32::INFINITY] {
            let n = nearest_note(hz, 0, Scale::Major);
            assert!(n.is_finite() && n > 0.0, "{hz} -> {n}");
            let i = interval_note(hz, 0, Scale::Blues, 3);
            assert!(i.is_finite() && i > 0.0, "{hz} -> {i}");
        }
    }

    proptest! {
        #[test]
        fn the_nearest_note_is_always_on_the_scale(
            midi in 36f32..96.0, key in 0u8..12, scale in 0usize..7
        ) {
            let scale = ALL[scale];
            let got = hz_to_midi(nearest_note(midi_to_hz(midi), key, scale));
            prop_assert!(on_scale(got, key, scale), "{got} in key {key} {scale:?}");
            // Never further than half a scale step away (the largest gap is three semitones).
            prop_assert!((got - midi).abs() <= 1.5 + 1e-3, "{midi} -> {got}");
        }

        #[test]
        fn chromatic_is_the_nearest_semitone(midi in 36f32..96.0) {
            let got = hz_to_midi(nearest_note(midi_to_hz(midi), 0, Scale::Chromatic));
            prop_assert!((got - midi.round()).abs() < 1e-2, "{midi} -> {got}");
        }

        #[test]
        fn an_interval_is_on_the_scale_and_steps_in_the_right_direction(
            midi in 36f32..96.0, key in 0u8..12, scale in 0usize..7, steps in -7i32..=7
        ) {
            let scale = ALL[scale];
            let hz = midi_to_hz(midi);
            let base = hz_to_midi(nearest_note(hz, key, scale));
            let got = hz_to_midi(interval_note(hz, key, scale, steps));
            prop_assert!(on_scale(got, key, scale));
            if steps > 0 {
                prop_assert!(got > base + 0.5);
            } else if steps < 0 {
                prop_assert!(got < base - 0.5);
            } else {
                prop_assert!((got - base).abs() < 1e-2);
            }
            // A full round of the scale is exactly one octave.
            let len = scale.degrees().len() as i32;
            let octave = hz_to_midi(interval_note(hz, key, scale, len));
            prop_assert!((octave - base - 12.0).abs() < 1e-2);
        }
    }
}
