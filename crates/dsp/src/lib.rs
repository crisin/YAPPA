//! Pure DSP building blocks. No I/O, no threads, no allocation inside `process*`.
//!
//! Everything here works on `f32` mono samples at a sample rate given at construction, so it
//! is testable with synthetic signals (`cargo test -p dsp`) and usable from the real-time
//! thread of `audio-engine`.

mod biquad;
mod compressor;
mod gate;
mod level;
mod limiter;

pub use biquad::{Biquad, BiquadKind, Coefficients};
pub use compressor::{Compressor, CompressorParams};
pub use gate::{Gate, GateParams};
pub use level::{db_to_gain, gain_to_db, rms};
pub use limiter::Limiter;
