//! Pipeline: orchestration of planning + voices, plus the SMF (`.mid`) writer.
//!
//! [`composer`] turns a seed into finished multi-track note events;
//! [`smf`] serialises them to a Standard MIDI File (format 1). The writer is
//! hand-rolled and dependency-free on purpose: it is small, fully testable,
//! and lets the audio/video engines share one canonical event model.

pub mod composer;
pub mod smf;

pub use composer::{Composition, GenerateError};
pub use smf::MidiWriteError;
