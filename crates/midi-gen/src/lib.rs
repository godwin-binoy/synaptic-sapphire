//! # midi-gen — algorithmic lofi composition engine
//!
//! A deterministic, dependency-light MIDI generator built for a 24/7 ambient
//! stream: the same `(seed, config)` pair always renders the exact same song,
//! which lets the audio/video engines re-render any past "episode" bit for bit.
//!
//! ## Layered architecture (bottom to top)
//!
//! | Layer      | Module        | Knows about                          | Knows nothing about |
//! |------------|---------------|--------------------------------------|---------------------|
//! | Primitives | [`music`]     | ticks, pitches, note events          | harmony, songs      |
//! | Randomness | [`rng`]       | reproducible streams                 | music               |
//! | Theory     | [`theory`]    | scales, chords, voice leading        | bars, randomness    |
//! | Structure  | [`structure`] | sections, arrangement, blueprint     | concrete notes      |
//! | Voices     | [`voices`]    | chord/bass/melody/drums generation   | other voices        |
//! | Pipeline   | [`pipeline`]  | orchestration + SMF serialisation    | transport/UI        |
//! | Config     | [`config`]    | the fully-serdeable parameter surface| algorithms          |
//!
//! The public entry points are [`compose_with_config`] (in-process events) and
//! [`render_midi_to_bytes`] (Standard MIDI File bytes). The web UI in `crates/ui`
//! is a thin shell over exactly these two calls plus [`ConfigSchema`] for
//! auto-generating controls.

pub mod config;
pub mod music;
pub mod pipeline;
pub mod rng;
pub mod structure;
pub mod theory;
pub mod voices;

// ---- Ergonomic root-level re-exports (the API most callers actually use) ----
pub use config::{
    ArrangementConfig, BassConfig, ChordsConfig, ConfigError, ConfigSchema, DrumsConfig,
    GenerateConfig, HumanizeConfig, KeyBias, MelodyConfig, MoodPreset, SchemaField, SchemaOption,
};
pub use music::{Chord, NoteEvent, NoteOn, Parts, Pitch, TempoMark, Ticks, PPQ};
pub use pipeline::{
    compose, compose_with_config, render_midi_to_bytes, Composition, GenerateError, MidiWriteError,
    Track,
};
pub use rng::Rand;
pub use structure::{Energy, SectionKind, SongBlueprint};
pub use theory::{ChordQuality, ChordSpec, HarmonicPlan, Scale};

/// Engine version stamped into generated files as a comment event.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
