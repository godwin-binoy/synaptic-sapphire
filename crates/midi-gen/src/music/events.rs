//! Track-level event vocabulary shared by all voices.

use super::{Chord, NoteOn, Ticks};

/// The only kinds of things a generated track can contain.
///
/// Restricting the vocabulary to four variants keeps the SMF writer simple and
/// makes it obvious which musical features are supported. New capabilities
/// (CC automation, program changes mid-bar, lyrics) are added here first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteEvent {
    /// A melodic/harmonic note.
    Note(NoteOn),
    /// A static harmony pad marker. Renderers may ignore this; the audio
    /// engine uses it to drive pads and side-chain behaviour.
    Harmony(Chord),
    /// A tempo change effective at `at`.
    Tempo(Ticks, f64),
    /// A text marker (section names, generation metadata).
    MetaText(MetaTextKind, String),
}

/// Which kind of textual annotation an event carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaTextKind {
    TrackName,
    SectionLabel,
    Comment,
}

/// Convenience alias used by the drum generator.
pub type Note = NoteOn;

/// A tempo instruction placed on the conductor track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoMark {
    pub at: Ticks,
    pub bpm: f64,
}

impl TempoMark {
    pub fn new(at: Ticks, bpm: f64) -> Self {
        TempoMark { at, bpm }
    }

    /// Microseconds per quarter note, the unit MIDI Standard Midi Files use.
    pub fn us_per_quarter(&self) -> u32 {
        ((60_000_000.0 / self.bpm).round() as i64).clamp(1, u32::MAX as i64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::{Parts, Pitch};

    #[test]
    fn tempo_conversion() {
        let t = TempoMark::new(Ticks::ZERO, 70.0);
        // 70 BPM => 857142.857 us/quarter, rounded.
        assert_eq!(t.us_per_quarter(), 857143);
        assert_eq!(TempoMark::new(Ticks::ZERO, 120.0).us_per_quarter(), 500000);
    }

    #[test]
    fn note_end_is_exclusive() {
        let n = NoteOn {
            pitch: Pitch(60),
            velocity: 90,
            start: Parts::beats(2).to_ticks(),
            duration: Parts::division(2).to_ticks(),
            program: 4,
        };
        assert_eq!(n.end(), Parts::beats(2).to_ticks() + Parts::division(2).to_ticks());
    }
}
