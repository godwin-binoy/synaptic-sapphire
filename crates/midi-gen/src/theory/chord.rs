//! Chord vocabulary: qualities, tensions, and concrete specs.

use crate::music::Chord;
use crate::theory::pc;

/// A chord quality expressed as stacked intervals above the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordQuality {
    /// Semitone intervals, always including `0` (the root), ascending.
    pub intervals: Vec<u8>,
    /// Symbol suffix used in labels (`"m7"`, `"maj9"`, ...).
    pub suffix: &'static str,
}

impl ChordQuality {
    pub fn minor_7() -> Self { ChordQuality { intervals: vec![0, 3, 7, 10], suffix: "m7" } }
    pub fn major_7() -> Self { ChordQuality { intervals: vec![0, 4, 7, 11], suffix: "maj7" } }
    pub fn dom_7() -> Self { ChordQuality { intervals: vec![0, 4, 7, 10], suffix: "7" } }
    pub fn minor_9() -> Self { ChordQuality { intervals: vec![0, 3, 7, 10, 14], suffix: "m9" } }
    pub fn major_9() -> Self { ChordQuality { intervals: vec![0, 4, 7, 11, 14], suffix: "maj9" } }
    pub fn half_dim() -> Self { ChordQuality { intervals: vec![0, 3, 6, 10], suffix: "m7b5" } }
    pub fn sus4() -> Self { ChordQuality { intervals: vec![0, 5, 7, 10], suffix: "sus4" } }

    /// Qualities considered "lofi-safe": lush but never dissonant enough to
    /// clash with a mellow sound bed. The generator picks only from these.
    pub fn lofi_set() -> Vec<ChordQuality> {
        vec![
            Self::minor_7(),
            Self::major_7(),
            Self::dom_7(),
            Self::minor_9(),
            Self::major_9(),
            Self::half_dim(),
            Self::sus4(),
        ]
    }
}

/// A fully resolved chord: quality on a root pitch class in an octave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordSpec {
    pub root_pc: usize,
    pub quality: ChordQuality,
    pub octave: i32,
    /// Duration in bars for which this harmony should be held.
    pub bars: u32,
}

impl ChordSpec {
    pub fn new(root_pc: usize, quality: ChordQuality, octave: i32, bars: u32) -> Self {
        ChordSpec { root_pc: pc(root_pc), quality, octave, bars }
    }

    /// Label such as `Am9` or `Fmaj7`.
    pub fn label(&self) -> String {
        use crate::theory::NOTE_NAMES;
        format!("{}{}", NOTE_NAMES[self.root_pc], self.quality.suffix)
    }

    /// Convert into the shared [`Chord`] event payload.
    pub fn to_chord(&self) -> Chord {
        Chord {
            root_pc: self.root_pc,
            intervals: self.quality.intervals.clone(),
            octave: self.octave,
            bars: self.bars,
            label: self.label(),
        }
    }

    /// Add an extension interval (e.g. `14` = ninth) for color. Duplicates and
    /// implausibly low intervals are ignored.
    pub fn with_tension(mut self, extra_semitones: u8) -> ChordSpec {
        if extra_semitones > 2 && !self.quality.intervals.contains(&extra_semitones) {
            self.quality.intervals.push(extra_semitones);
            self.quality.intervals.sort_unstable();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_standard() {
        assert_eq!(ChordSpec::new(9, ChordQuality::minor_9(), 3, 1).label(), "Am9");
        assert_eq!(ChordSpec::new(5, ChordQuality::major_7(), 4, 2).label(), "Fmaj7");
        assert_eq!(ChordSpec::new(0, ChordQuality::sus4(), 4, 1).label(), "Csus4");
    }

    #[test]
    fn to_chord_carries_payload() {
        let spec = ChordSpec::new(0, ChordQuality::major_7(), 4, 2);
        let c = spec.to_chord();
        assert_eq!(c.intervals, vec![0, 4, 7, 11]);
        assert_eq!(c.bars, 2);
        assert_eq!(c.label, "Cmaj7");
    }

    #[test]
    fn tension_is_added_once_and_sorted() {
        let spec = ChordSpec::new(9, ChordQuality::minor_7(), 3, 1).with_tension(14);
        assert_eq!(spec.quality.intervals, vec![0, 3, 7, 10, 14]);
        // Adding the same tension again must not duplicate it.
        let again = spec.with_tension(14);
        assert_eq!(again.quality.intervals, vec![0, 3, 7, 10, 14]);
    }

    #[test]
    fn lofi_set_has_no_raw_triads() {
        for q in ChordQuality::lofi_set() {
            assert!(q.intervals.len() >= 4, "{:?} too thin for lofi", q);
        }
    }
}
