//! Musical time and event primitives.
//!
//! This module is intentionally free of any domain knowledge about harmony or
//! composition rules: it only defines *what a moment in time is* ([`Ticks`],
//! [`Parts`]) and *what can happen at a moment* ([`NoteOn`], [`Chord`], ...).
//! Everything downstream (theory, structure, voices) speaks this vocabulary.

mod parts;
mod pitch;
mod events;

pub use parts::{Parts, PPQ};
pub use pitch::{Pitch, PITCH_MAX, PITCH_MIN};
pub use events::{Chord, NoteEvent, NoteOn, TempoMark};

/// A position/duration measured in MIDI ticks (PPD quanta).
///
/// Ticks are the atomic unit of the whole engine. All arithmetic on musical
/// time goes through [`Parts`] so that fractional positions never lose
/// precision. The raw value is public because tick counts are unambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Ticks(pub i64);

impl Ticks {
    pub const ZERO: Ticks = Ticks(0);

    #[inline]
    pub fn new(t: i64) -> Self {
        Ticks(t)
    }

    #[inline]
    pub fn as_i64(self) -> i64 {
        self.0
    }

    /// Seconds elapsed for this many ticks at `bpm`. Used by renderers and by
    /// the SMF writer to place tempo markers.
    #[inline]
    pub fn to_seconds(self, bpm: f64) -> f64 {
        (self.0 as f64) * 60.0 / (bpm * PPQ as f64)
    }
}

impl std::ops::Add for Ticks {
    type Output = Ticks;
    #[inline]
    fn add(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Ticks {
    type Output = Ticks;
    #[inline]
    fn sub(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 - rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_arithmetic() {
        assert_eq!(Ticks(1920) + Ticks(960), Ticks(2880));
        assert_eq!(Ticks(1920) - Ticks(960), Ticks(960));
    }

    #[test]
    fn seconds_conversion_at_ppq() {
        // One quarter note (1920 ticks) at 60 BPM is exactly one second.
        let q = Parts::beats(1).to_ticks();
        assert_eq!(q, Ticks(PPQ as i64));
        assert!((q.to_seconds(60.0) - 1.0).abs() < 1e-12);
        // And at 70 BPM it is 60/70 seconds.
        assert!((q.to_seconds(70.0) - 60.0 / 70.0).abs() < 1e-12);
    }
}
