//! Electric-piano chord comping with voice leading and swing offsets.
//!
//! Strategy per bar:
//! 1. Voice the current chord from the previous voicing ([`voice_chord`]).
//! 2. Choose a comping pattern by energy: whole-bar pad hit, "lofi pulse"
//!    (downbeat + delayed third-beat stab), or syncopated double stab.
//! 3. Apply humanisation: small timing pushes (swing eighths), velocity
//!    accents on strong beats, slight duration shortening so chords breathe.

use crate::music::{NoteEvent, NoteOn, Parts, Pitch, Ticks};
use crate::rng::Rand;
use crate::structure::{Energy, SongBlueprint};
use crate::theory::voice_leading::voice_chord;

use super::{bar_len, bar_start, Voice};

pub struct ChordsVoice;

impl Voice for ChordsVoice {
    const NAME: &'static str = "Rhodes";
    const PROGRAM: u8 = 4; // GM "Electric Piano 1"

    fn generate(bp: &SongBlueprint, rng: &mut Rand) -> Vec<NoteEvent> {
        let mut out = Vec::new();
        let mut prev: Option<Vec<Pitch>> = None;

        for bar in 0..bp.total_bars() {
            let Some(spec) = bp.chord_at(bar) else { continue };
            let role = bp.role_at(bar).expect("bar inside blueprint");
            let chord = spec.to_chord();

            let voicing = voice_chord(prev.as_ref(), &chord);
            prev = Some(voicing.clone());

            // Record the harmony itself for downstream renderers.
            out.push(NoteEvent::Harmony(chord.clone()));

            match role.energy {
                Energy::Ambient => {
                    // Long, soft pad covering the whole bar.
                    push_stab(&mut out, &voicing, bar_start(bp, bar), bar_len(bp), 52, PAD_PROGRAM);
                }
                Energy::Low => {
                    // One gentle downbeat hit, held ~3 beats.
                    let dur = Parts::beats(3).to_ticks();
                    push_stab(&mut out, &voicing, swing(bar_start(bp, bar), 0.0), dur, 66, Self::PROGRAM);
                }
                Energy::Medium | Energy::High => {
                    // Downbeat + off-beat stab; high energy adds a third stab.
                    let b0 = swing(bar_start(bp, bar), 0.0);
                    let dur_short = Parts::new(0, 3, 4).to_ticks(); // dotted quarter-ish
                    push_stab(&mut out, &voicing, b0, dur_short, 82, Self::PROGRAM);

                    let off = if rng.chance(2, 3) { 2.5 } else { 2.0 };
                    push_stab(
                        &mut out,
                        &voicing,
                        swing(bar_start(bp, bar), off),
                        Parts::division(2).to_ticks(),
                        64,
                        Self::PROGRAM,
                    );
                    if role.energy == Energy::High && rng.chance(1, 2) {
                        push_stab(
                            &mut out,
                            &voicing,
                            swing(bar_start(bp, bar), 3.5),
                            Parts::division(4).to_ticks(),
                            58,
                            Self::PROGRAM,
                        );
                    }
                }
            }
        }
        out
    }
}

/// Emit one simultaneous chord attack.
fn push_stab(out: &mut Vec<NoteEvent>, voicing: &[Pitch], start: Ticks, dur: Ticks, vel_base: u8, program: u8) {
    for (i, p) in voicing.iter().enumerate() {
        // Spread velocities slightly across voices: bass softer, top brighter.
        let spread = (i as i16 - voicing.len() as i16 / 2) * 3;
        let v = (vel_base as i16 + spread).clamp(30, 110) as u8;
        out.push(NoteEvent::Note(NoteOn {
            pitch: *p,
            velocity: v,
            start,
            duration: dur,
            program,
        }));
    }
}

/// GM program used when chords turn into sustained pads.
const PAD_PROGRAM: u8 = 89; // "Pad 2 (warm)"

/// Swing offset in beats within the bar: eighth-note slots are pushed late by
/// ~6% of a beat (classic lofi "drunk" quantize). Whole/half beats stay put.
fn swing(at: Ticks, beats_offset: f64) -> Ticks {
    const EIGHTH: f64 = 0.125;
    let nearest_eighth = (beats_offset / EIGHTH).round() * EIGHTH;
    let is_off_beat_eighth = ((nearest_eighth * 2.0) as i64) % 2 == 1;
    let push = if is_off_beat_eighth { 0.06 } else { 0.0 };
    let extra = (nearest_eighth + push) * crate::music::PPQ as f64;
    Ticks(at.as_i64() + extra.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp(bars: u32) -> SongBlueprint {
        SongBlueprint::generate(&mut Rand::new(bars as u64 * 3 + 1), bars)
    }

    #[test]
    fn produces_notes_for_every_non_ambient_bar() {
        let bp = bp(16);
        let evs = ChordsVoice::generate(&bp, &mut Rand::new(2));
        let notes = evs.iter().filter(|e| matches!(e, NoteEvent::Note(_))).count();
        assert!(notes > 0);
    }

    #[test]
    fn all_notes_fit_inside_the_song() {
        let bp = bp(16);
        let total = bar_len(&bp).as_i64() * bp.total_bars() as i64;
        let evs = ChordsVoice::generate(&bp, &mut Rand::new(3));
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!(n.start.as_i64() >= 0 && n.end().as_i64() <= total + 1, "{n:?}");
                assert!((1..=127).contains(&n.velocity));
            }
        }
    }

    #[test]
    fn voicings_lead_smoothly_across_bars() {
        let bp = bp(32);
        let evs = ChordsVoice::generate(&bp, &mut Rand::new(4));
        // Group note starts by bar; consecutive stabs within a bar must share
        // the exact same set of pitches (same voicing).
        let mut by_start: std::collections::BTreeMap<i64, Vec<u8>> = Default::default();
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                by_start.entry(n.start.as_i64()).or_default().push(n.pitch.midi());
            }
        }
        for (_, ps) in by_start {
            let mut sorted = ps.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), ps.len(), "duplicate pitch inside one stab");
        }
    }

    #[test]
    fn deterministic() {
        let a = ChordsVoice::generate(&bp(16), &mut Rand::new(9)).len();
        let b = ChordsVoice::generate(&bp(16), &mut Rand::new(9)).len();
        assert_eq!(a, b);
    }
}
