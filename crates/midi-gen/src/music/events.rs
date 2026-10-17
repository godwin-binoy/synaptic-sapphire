//! Track-level event vocabulary shared by all voices.

use super::{Chord, NoteOn, Ticks};

/// The only kinds of things a generated track can contain.
///
/// Restricting the vocabulary to four variants keeps the SMF writer simple and
/// makes it obvious which musical features are supported. New capabilities
/// (CC automation, program changes mid-bar, lyrics) are added here first.
#[derive(Debug, Clone, PartialEq)]
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

/// A melodic/harmonic voice track: named, routed to a GM channel, with a
/// default program and its event stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: String,
    /// MIDI channel 0..=15 (drums conventionally on 9).
    pub channel: u8,
    /// Default General MIDI program for this voice.
    pub program: u8,
    pub events: Vec<NoteEvent>,
}

impl Track {
    pub fn new(name: impl Into<String>, channel: u8, program: u8) -> Self {
        Track { name: name.into(), channel, program, events: Vec::new() }
    }
}

/// A whole generated song: metadata plus one [`Track`] per voice.
#[derive(Debug, Clone, PartialEq)]
pub struct Song {
    pub name: String,
    /// Tempo in BPM at the start of the song.
    pub tempo_bpm: f64,
    /// Beats per bar (time-signature numerator; the engine is 4-based).
    pub beats_per_bar: i64,
    /// Total length in ticks.
    pub total_ticks: Ticks,
    pub tracks: Vec<Track>,
}

impl Song {
    pub fn new(
        name: impl Into<String>,
        tempo_bpm: f64,
        beats_per_bar: i64,
        total_ticks: Ticks,
        tracks: Vec<Track>,
    ) -> Self {
        Song { name: name.into(), tempo_bpm, beats_per_bar, total_ticks, tracks }
    }

    /// Total duration in seconds at the song's tempo.
    pub fn duration_seconds(&self) -> f64 {
        self.total_ticks.to_seconds(self.tempo_bpm)
    }
}

#[cfg(test)]
mod song_tests {
    use super::*;
    use crate::music::Ticks;

    #[test]
    fn song_duration() {
        let song = Song::new("t", 120.0, 4, Ticks(2 * crate::music::PPQ as i64), vec![]);
        assert!((song.duration_seconds() - 1.0).abs() < 1e-9);
    }
}
