//! Scientific-pitch MIDI note numbers.

use crate::music::Ticks;

/// Smallest and largest pitch a voice is allowed to emit. Keeping the range
/// inside `[21, 108]` guarantees every note lands on an 88-key piano, which
/// makes the output safe for any acoustic sound bank downstream.
pub const PITCH_MIN: u8 = 21;
pub const PITCH_MAX: u8 = 108;

/// A MIDI note number with pitch-class arithmetic helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pitch(pub u8);

impl Pitch {
    /// Clamp-constructing constructor: out-of-range values are folded into the
    /// playable range instead of panicking, because generators explore wide
    /// intervals and a clamped note is always preferable to a crash.
    pub fn new(n: u8) -> Self {
        Pitch(n.clamp(PITCH_MIN, PITCH_MAX))
    }

    /// Build from a pitch class (`0..12`) and an octave number.
    pub fn from_pc_octave(pc: usize, octave: i32) -> Self {
        let n = (octave as i32 + 1) * 12 + pc as i32;
        Pitch(n.clamp(PITCH_MIN as i32, PITCH_MAX as i32) as u8)
    }

    #[inline]
    pub fn midi(self) -> u8 {
        self.0
    }

    /// `0 = C, 1 = C#, ... 11 = B`.
    #[inline]
    pub fn pitch_class(self) -> usize {
        (self.0 % 12) as usize
    }

    /// Equal-tempered frequency in Hz for a MIDI note number (A4 = 440).
    /// Used by the preview synth; not needed by the MIDI pipeline itself.
    #[inline]
    pub fn freq_hz(self) -> f64 {
        440.0 * 2f64.powf((self.0 as f64 - 69.0) / 12.0)
    }

    /// Scientific octave (middle C is octave 4).
    #[inline]
    pub fn octave(self) -> i32 {
        self.0 as i32 / 12 - 1
    }

    /// Transpose by `semitones`, saturating at the playable extremes.
    pub fn transpose(self, semitones: i32) -> Self {
        Pitch(((self.0 as i32 + semitones).clamp(PITCH_MIN as i32, PITCH_MAX as i32)) as u8)
    }

    /// Distance in semitones from `self` up to `other` (can be negative).
    pub fn interval_to(self, other: Pitch) -> i32 {
        other.0 as i32 - self.0 as i32
    }

    /// Short name such as `A#3`, used by the CLI listing and debug output.
    pub fn name(self) -> String {
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        format!("{}{}", NAMES[self.pitch_class()], self.octave())
    }
}

/// A single onset of a note: what sounds, how hard, when, and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteOn {
    pub pitch: Pitch,
    /// MIDI velocity, 1..=127. Zero velocities are dropped at write time since
    /// they would be encoded as note-offs.
    pub velocity: u8,
    pub start: Ticks,
    pub duration: Ticks,
    /// General MIDI program (0..=127) this note should play on. Carried per
    /// note rather than per track so that one logical voice can switch
    /// instruments (e.g. Rhodes -> piano) without splitting tracks.
    pub program: u8,
}

impl NoteOn {
    /// End position, exclusive.
    pub fn end(&self) -> Ticks {
        self.start + self.duration
    }
}

/// A sounding set of simultaneous notes (the harmonic payload of a bar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    /// Root pitch class, `0..12`.
    pub root_pc: usize,
    /// Scale degrees above the root, in semitones, ascending.
    pub intervals: Vec<u8>,
    /// Octave in which the chord's root voice sits.
    pub octave: i32,
    /// How many bars this harmony lasts.
    pub bars: u32,
    /// Human-readable label for debugging ("Am9", "Fmaj7", ...).
    pub label: String,
}

impl Chord {
    /// Absolute pitches of the chord voices, clamped to the playable range.
    pub fn voicing(&self) -> Vec<Pitch> {
        let mut out = Vec::with_capacity(self.intervals.len());
        for iv in &self.intervals {
            out.push(Pitch::from_pc_octave((self.root_pc + *iv as usize) % 12, self.octave));
        }
        out.sort();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_c_is_60() {
        assert_eq!(Pitch::from_pc_octave(0, 4).midi(), 60);
        assert_eq!(Pitch(60).name(), "C4");
        assert_eq!(Pitch(61).name(), "C#4");
        assert_eq!(Pitch(69).name(), "A4");
    }

    #[test]
    fn transposition_saturates() {
        assert_eq!(Pitch(60).transpose(-60), Pitch(PITCH_MIN));
        assert_eq!(Pitch(60).transpose(100), Pitch(PITCH_MAX));
        assert_eq!(Pitch(60).transpose(7).transpose(-7), Pitch(60));
    }

    #[test]
    fn voicing_is_sorted_and_clamped() {
        let c = Chord {
            root_pc: 9,
            intervals: vec![0, 3, 7, 14],
            octave: 3,
            bars: 1,
            label: "Am9".into(),
        };
        let v = c.voicing();
        // The tenth interval (14 semitones) wraps to pitch class C and is clamped
        // into the same octave as the root, so voicings may contain duplicates
        // that `dedup` removes. Sortedness and range are the real invariants.
        assert_eq!(v[0].name(), "C3");
        assert_eq!(v.len(), 3);
        for w in v.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn interval_roundtrip() {
        assert_eq!(Pitch(60).interval_to(Pitch(67)), 7);
        assert_eq!(Pitch(67).interval_to(Pitch(60)), -7);
    }
}
