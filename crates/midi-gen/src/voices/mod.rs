//! Voice generators: each turns a [`SongBlueprint`] into note events.
//!
//! Every voice is a module exposing one entry point of the shape
//! `fn generate(&SongBlueprint, &mut Rand) -> Vec<NoteEvent>` plus helpers.
//! Voices never talk to each other; they coordinate only through the blueprint
//! and (for continuity) their own internal state looped over bars.

pub mod chords;
pub mod bass;
pub mod melody;
pub mod drums;

use crate::music::{NoteEvent, Ticks};
use crate::structure::SongBlueprint;

/// Re-exported so callers can build tracks uniformly.
pub trait Voice {
    /// Human-readable track name written into the MIDI file.
    const NAME: &'static str;
    /// General MIDI program used as the default instrument.
    const PROGRAM: u8;

    /// Emit every event for the whole song.
    fn generate(blueprint: &SongBlueprint, rng: &mut crate::rng::Rand) -> Vec<NoteEvent>;
}

/// Tick position of global bar `bar`.
#[inline]
pub fn bar_start(bp: &SongBlueprint, bar: u32) -> Ticks {
    Ticks(bar as i64 * 4 * crate::music::PPQ as i64)
}

/// Length of one bar in ticks.
#[inline]
pub fn bar_len(bp: &SongBlueprint) -> Ticks {
    Ticks(4 * crate::music::PPQ as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::Parts;
    use crate::rng::Rand;
    use crate::structure::SongBlueprint;

    #[test]
    fn bar_grid_is_integral() {
        let bp = SongBlueprint::generate(&mut Rand::new(1), 16);
        assert_eq!(bar_start(&bp, 0), Ticks::ZERO);
        assert_eq!(bar_start(&bp, 3) - bar_start(&bp, 2), bar_len(&bp));
        // A bar is four beats at PPQ resolution.
        assert_eq!(bar_len(&bp).as_i64(), Parts::beats(4).to_ticks().as_i64());
    }
}
