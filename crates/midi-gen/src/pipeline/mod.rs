//! The generation pipeline: config → blueprint → voices → Song → MIDI bytes.

pub mod composer;
pub mod midi_writer;

pub use composer::{GenerationError, GenerationOutput};
pub use midi_writer::to_midi_bytes;
