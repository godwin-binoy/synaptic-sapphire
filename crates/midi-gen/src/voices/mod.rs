//! Voice generators: each turns a [`SongBlueprint`] + [`GenerateConfig`] into
//! note events.
//!
//! Every voice is a module exposing one entry point of the shape
//! `fn generate(&SongBlueprint, &GenerateConfig, &mut Rand) -> Vec<NoteEvent>`
//! plus helpers. Voices never talk to each other; they coordinate only through
//! the blueprint and their own internal continuity state.

pub mod chords;
pub mod bass;
pub mod melody;
pub mod drums;

use crate::config::GenerateConfig;
use crate::music::{NoteEvent, Ticks};
use crate::structure::SongBlueprint;

/// Re-exported so callers can build tracks uniformly.
pub trait Voice {
    /// Human-readable track name written into the MIDI file.
    const NAME: &'static str;
    /// General MIDI program used as the default instrument (may be overridden
    /// by the config's per-voice `program` knob).
    const PROGRAM: u8;

    /// Emit every event for the whole song.
    fn generate(bp: &SongBlueprint, cfg: &GenerateConfig, rng: &mut crate::rng::Rand) -> Vec<NoteEvent>;
}

/// Tick position of global bar `bar`.
#[inline]
pub fn bar_start(_bp: &SongBlueprint, bar: u32) -> Ticks {
    Ticks(bar as i64 * 4 * crate::music::PPQ as i64)
}

/// Length of one bar in ticks.
#[inline]
pub fn bar_len(_bp: &SongBlueprint) -> Ticks {
    Ticks(4 * crate::music::PPQ as i64)
}

/// Shared humanisation pass applied to a finished voice part: timing jitter,
/// velocity jitter and accent shaping from `cfg.humanize`, drawn from the
/// voice's own stream so results stay deterministic.
pub(crate) fn humanize(events: &mut [NoteEvent], cfg: &GenerateConfig, rng: &mut crate::rng::Rand) {
    use crate::music::NoteEvent;
    let h = &cfg.humanize;
    if h.timing_jitter_ticks == 0 && h.velocity_jitter == 0 {
        return;
    }
    for e in events.iter_mut() {
        if let NoteEvent::Note(n) = e {
            if h.timing_jitter_ticks > 0 {
                let j = rng.range_i64(-h.timing_jitter_ticks, h.timing_jitter_ticks + 1);
                n.start = Ticks((n.start.as_i64() + j).max(0));
            }
            if h.velocity_jitter > 0 {
                let j = rng.range_i64(-h.velocity_jitter, h.velocity_jitter + 1);
                n.velocity = (n.velocity as i32 + j as i32).clamp(15, 127) as u8;
            }
        }
    }
}

/// Beat offset inside a bar with configured swing pushed onto off-eighths.
/// `beats_offset` is measured in beats from the bar start.
pub(crate) fn swing_at(at: Ticks, beats_offset: f64, swing_strength: f64) -> Ticks {
    const EIGHTH: f64 = 0.125;
    let nearest_eighth = (beats_offset / EIGHTH).round() * EIGHTH;
    let is_off_beat_eighth = ((nearest_eighth * 2.0) as i64) % 2 == 1;
    // A full-strength swing pushes an off-eighth late by ~25% of the eighth
    // (classic triplet-ish shuffle at max); lofi defaults land near 12%.
    let push = if is_off_beat_eighth { 0.25 * swing_strength } else { 0.0 };
    let extra = (nearest_eighth + push) * crate::music::PPQ as f64;
    Ticks(at.as_i64() + extra.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::Parts;
    use crate::rng::Rand;
    use crate::structure::SongBlueprint;

    #[test]
    fn bar_grid_is_integral() {
        let bp = SongBlueprint::generate(&mut Rand::new(1), &GenerateConfig::default());
        assert_eq!(bar_start(&bp, 0), Ticks::ZERO);
        assert_eq!(bar_start(&bp, 3) - bar_start(&bp, 2), bar_len(&bp));
        // A bar is four beats at PPQ resolution.
        assert_eq!(bar_len(&bp).as_i64(), Parts::beats(4).to_ticks().as_i64());
    }

    #[test]
    fn swing_pushes_only_off_eighths() {
        let base = Ticks(0);
        let straight = swing_at(base, 1.0, 1.0); // on-beat: unchanged
        let pushed = swing_at(base, 1.5, 1.0); // off-eighth: late
        assert_eq!(straight.as_i64(), Parts::beats(1).to_ticks().as_i64());
        assert!(pushed.as_i64() > Parts::new(0, 3, 4).to_ticks().as_i64());
        // Zero swing leaves everything on the grid.
        assert_eq!(swing_at(base, 1.5, 0.0), Parts::new(0, 3, 4).to_ticks());
    }
}
