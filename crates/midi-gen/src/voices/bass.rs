//! Upright-style bass line: root-fifth groove with passing tones.
//!
//! The bass locks with the chord plan: every bar gets at least a root on beat
//! 1; medium/high energy adds fifth-and-octave motion plus chromatic approach
//! notes into chord changes. Notes sit in a fixed register (E1..A2) so the
//! audio engine never has to compensate.

use crate::music::{NoteEvent, NoteOn, Parts, Pitch, Ticks};
use crate::rng::Rand;
use crate::structure::{Energy, SongBlueprint};
use crate::theory::pc;

use super::{bar_start, Voice};

pub struct BassVoice;

/// Lowest/highest pitch the bass may use.
const REG_LOW: u8 = 40; // E2
const REG_HIGH: u8 = 57; // A3

impl Voice for BassVoice {
    const NAME: &'static str = "Bass";
    const PROGRAM: u8 = 33; // GM "Electric Bass Finger"

    fn generate(bp: &SongBlueprint, rng: &mut Rand) -> Vec<NoteEvent> {
        let mut out = Vec::new();
        let mut prev_root: Option<usize> = None;

        for bar in 0..bp.total_bars() {
            let Some(spec) = bp.chord_at(bar) else { continue };
            let role = bp.role_at(bar).expect("inside blueprint");
            if role.energy == Energy::Ambient {
                // Outro/intro pads: bass rests entirely.
                prev_root = Some(spec.root_pc);
                continue;
            }

            let root = place_in_register(spec.root_pc, prev_root);
            let next_root = bp
                .chord_at(bar + 1)
                .map(|s| place_in_register(s.root_pc, Some(spec.root_pc)));

            // Beat 1: the anchor. Long and warm.
            out.push(note(root, 92, bar_start(bp, bar), Parts::division(2).to_ticks(), Self::PROGRAM));

            match role.energy {
                Energy::Low => {
                    // Sparse: one pickup into the next chord.
                    if rng.chance(1, 2) {
                        let t = bar_start(bp, bar) + Parts::new(0, 3, 4).to_ticks();
                        let p = approach_pitch(&root, &next_root.clone().unwrap_or(root));
                        out.push(note(p, 62, t, Parts::division(4).to_ticks(), Self::PROGRAM));
                    }
                }
                Energy::Medium | Energy::High => {
                    // Classic lofi heartbeat: root on 1, fifth on 2+, octave-ish
                    // or sixth on 3, optional walk into the next chord.
                    let fifth = transpose_keep_reg(&root, 7);
                    out.push(note(
                        fifth,
                        78,
                        swing_tick(bar_start(bp, bar), 1.5),
                        Parts::division(4).to_ticks(),
                        Self::PROGRAM,
                    ));
                    let colour = if rng.chance(1, 2) { 12 } else { 3 };
                    out.push(note(
                        transpose_keep_reg(&root, colour),
                        70,
                        swing_tick(bar_start(bp, bar), 3.0),
                        Parts::division(4).to_ticks(),
                        Self::PROGRAM,
                    ));
                    if role.energy == Energy::High || role.is_cadence_bar() {
                        if let Some(nr) = &next_root {
                            let ap = approach_pitch(&root, nr);
                            out.push(note(
                                ap,
                                66,
                                swing_tick(bar_start(bp, bar), 3.5),
                                Parts::division(8).to_ticks(),
                                Self::PROGRAM,
                            ));
                        }
                    }
                }
                _ => {}
            }
            prev_root = Some(spec.root_pc);
        }
        out
    }
}

/// Choose an octave for `pitch_class` near the previous root, inside register.
fn place_in_register(pitch_class: usize, prev_root: Option<usize>) -> Pitch {
    // Candidate octaves of this pitch class inside the register.
    let mut candidates: Vec<i32> = Vec::new();
    let mut m = pitch_class as i32;
    while m % 12 != pc(pitch_class) as i32 {
        m += 1; // normalise (defensive; pc() already keeps it in range)
    }
    while m < REG_LOW as i32 {
        m += 12;
    }
    let mut v = m;
    while v <= REG_HIGH as i32 {
        candidates.push(v);
        v += 12;
    }
    if candidates.is_empty() {
        return Pitch::new(pc(pitch_class) as u8 + 36);
    }
    // Anchor: previous root's absolute midi, else middle of register.
    let anchor = prev_root
        .map(|pr| {
            let mut x = pr as i32 + 36;
            while x < REG_LOW as i32 {
                x += 12;
            }
            while x > REG_HIGH as i32 {
                x -= 12;
            }
            x
        })
        .unwrap_or(((REG_LOW + REG_HIGH) / 2) as i32);
    let best = candidates
        .into_iter()
        .min_by_key(|c| (c - anchor).abs())
        .expect("non-empty candidates");
    Pitch(best as u8)
}

/// Transpose keeping the result inside the register.
fn transpose_keep_reg(p: &Pitch, semitones: i32) -> Pitch {
    let mut m = p.midi() as i32 + semitones;
    while m < REG_LOW as i32 {
        m += 12;
    }
    while m > REG_HIGH as i32 {
        m -= 12;
    }
    Pitch(m.clamp(REG_LOW as i32, REG_HIGH as i32) as u8)
}

/// Chromatic/diatonic approach note leading into `target`.
fn approach_pitch(from: &Pitch, target: &Pitch) -> Pitch {
    // Semitone below the target is the canonical jazz approach.
    let below = target.transpose(-1);
    // Avoid awkward unison clashes with the following downbeat octave jumps.
    if from.interval_to(&below).abs() > 9 {
        target.transpose(-2)
    } else {
        below
    }
}

/// Beat offset inside a bar with light swing on off-eighths.
fn swing_tick(at: Ticks, beats_offset: f64) -> Ticks {
    const EIGHTH: f64 = 0.125;
    let nearest_eighth = (beats_offset / EIGHTH).round() * EIGHTH;
    let push = if ((nearest_eighth * 2.0) as i64) % 2 == 1 { 0.05 } else { 0.0 };
    Ticks(at.as_i64() + ((nearest_eighth + push) * crate::music::PPQ as f64).round() as i64)
}

fn note(pitch: Pitch, velocity: u8, start: Ticks, duration: Ticks, program: u8) -> NoteEvent {
    NoteEvent::Note(NoteOn { pitch, velocity, start, duration, program })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::SectionKind;

    fn bp(bars: u32) -> SongBlueprint {
        SongBlueprint::generate(&mut Rand::new(bars as u64 * 5 + 2), bars)
    }

    #[test]
    fn bass_stays_in_register() {
        let evs = BassVoice::generate(&bp(48), &mut Rand::new(1));
        assert!(!evs.is_empty());
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!(
                    (REG_LOW..=REG_HIGH).contains(&n.pitch.midi()),
                    "{} out of bass register",
                    n.pitch.name()
                );
            }
        }
    }

    #[test]
    fn downbeat_is_always_the_chord_root() {
        let bp = bp(32);
        let evs = BassVoice::generate(&bp, &mut Rand::new(7));
        for bar in 0..bp.total_bars() {
            let role = bp.role_at(bar).unwrap();
            if role.energy == Energy::Ambient {
                continue;
            }
            let start = bar_start(&bp, bar);
            let anchor = evs
                .iter()
                .find_map(|e| if let NoteEvent::Note(n) = e { (n.start == start).then_some(n) } else { None });
            let chord = bp.chord_at(bar).unwrap();
            assert_eq!(
                anchor.expect("missing bass anchor").pitch.pitch_class(),
                pc(chord.root_pc),
                "bar {bar}"
            );
        }
    }

    #[test]
    fn ambient_sections_have_no_bass() {
        let bp = bp(16);
        let evs = BassVoice::generate(&bp, &mut Rand::new(3));
        for bar in 0..bp.total_bars() {
            if bp.role_at(bar).unwrap().section == SectionKind::Outro {
                let start = bar_start(&bp, bar);
                assert!(
                    !evs.iter().any(|e| matches!(e, NoteEvent::Note(n) if n.start >= start && n.start < start + crate::music::Parts::beats(4).to_ticks())),
                    "bass plays during outro bar {bar}"
                );
            }
        }
    }

    #[test]
    fn deterministic() {
        let a = format!("{:?}", BassVoice::generate(&bp(24), &mut Rand::new(5)));
        let b = format!("{:?}", BassVoice::generate(&bp(24), &mut Rand::new(5)));
        assert_eq!(a, b);
    }
}
