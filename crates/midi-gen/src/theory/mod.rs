//! Music-theory data: scales, chords, diatonic functions and voice leading.
//!
//! This layer knows *nothing* about songs, bars or randomness — it is a pure,
//! testable catalogue of relationships. The composition layers (`structure`,
//! `voices`) query it.

pub mod scale;
pub mod chord;
pub mod progression;
pub mod voice_leading;

pub use chord::{ChordQuality, ChordSpec};
pub use progression::{HarmonicPlan, HarmonicStep};
pub use scale::Scale;
pub use voice_leading::voice_chord;

/// Pitch-class helpers shared across the theory submodules.

/// Transpose a pitch class into `0..12`.
pub(crate) fn pc(x: usize) -> usize {
    x % 12
}

/// Distance in semitones between two pitch classes, always in `0..12`.
pub(crate) fn pc_distance(from: usize, to: usize) -> usize {
    (to + 12 - from) % 12
}

/// Whether a triad's third is minor, given its root and third pitch classes.
pub fn third_is_minor(root_pc: usize, third_pc: usize) -> bool {
    pc_distance(root_pc, third_pc) == 3
}

/// Canonical sharp-side note names for display.
pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_class_math() {
        assert_eq!(pc(25), 1);
        assert_eq!(pc_distance(11, 2), 3);
        assert_eq!(pc_distance(2, 11), 9);
        assert!(third_is_minor(0, 3));
        assert!(!third_is_minor(0, 4));
    }
}
