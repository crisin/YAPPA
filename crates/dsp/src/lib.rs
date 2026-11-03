//! Pure DSP building blocks. No I/O, no threads, no allocation inside `process*`.
//!
//! Everything here works on `f32` mono samples at a sample rate given at construction, so it
//! is testable with synthetic signals (`cargo test -p dsp`) and usable from the real-time
//! thread of `audio-engine`.

mod autotune;
mod biquad;
mod compressor;
mod drift;
mod gate;
mod harmonizer;
mod level;
mod leveler;
mod limiter;
mod pitch;
mod pitch_track;
mod radio;
mod resampler;
mod reverb;
mod robot;
mod scale;
mod varispeed;

pub use autotune::Autotune;
pub use biquad::{Biquad, BiquadKind, Coefficients};
pub use compressor::{Compressor, CompressorParams};
pub use drift::{DriftController, DriftParams};
pub use gate::{Gate, GateParams};
pub use harmonizer::Harmonizer;
pub use level::{db_to_gain, gain_to_db, rms};
pub use leveler::{Leveler, LevelerParams};
pub use limiter::Limiter;
pub use pitch::{PitchShifter, MAX_RATIO, MAX_SEMITONES, MIN_RATIO};
pub use pitch_track::{Pitch, PitchTracker, MAX_HZ, MIN_HZ};
pub use radio::Radio;
pub use resampler::Resampler;
pub use reverb::Reverb;
pub use robot::Robot;
pub use scale::{hz_to_midi, interval_note, midi_to_hz, nearest_note, Key, Scale};
pub use varispeed::{Varispeed, MAX_DEVIATION};
