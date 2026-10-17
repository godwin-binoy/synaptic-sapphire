//! Lo-fi drum groove on a GM kit: lazy boom-bap with humanised timing.
//!
//! Pattern model: each instrument has an eighth-note probability template per
//! bar; energy selects the template family, cadence bars append a tom fill.
//! Kick and snare are quantised loosely around the beat grid with per-hit
//! micro-offsets (the "drunk" lofi feel), hats stay straight but quiet.

use crate::music::{NoteEvent, NoteOn, Parts, Pitch, Ticks};
use crate::rng::Rand;
use crate::structure::{Energy, SongBlueprint};

use super::{bar_start, Voice};

pub struct DrumsVoice;

/// GM percussion keys used here.
const KICK: u8 = 36;
const SNARE: u8 = 38;
const RIM: u8 = 37; // side stick, softer backbeat option
const HAT_CLOSED: u8 = 44;
const HAT_OPEN: u8 = 46;
const SHAKER: u8 = 70; // maracas for texture
const TOM_HI: u8 = 50;
const TOM_LO: u8 = 45;
const CRASH: u8 = 49;

impl Voice for DrumsVoice {
    const NAME: &'static str = "Drums";
    const PROGRAM: u8 = 0; // percussion track ignores programs

    fn generate(bp: &SongBlueprint, rng: &mut Rand) -> Vec<NoteEvent> {
        let mut out = Vec::new();
        let eighth = Parts::division(2).to_ticks();

        for bar in 0..bp.total_bars() {
            let role = bp.role_at(bar).expect("inside blueprint");
            let factor = role.energy.groove_factor();
            if factor == 0.0 {
                continue; // ambient sections are drumless
            }
            let at = bar_start(bp, bar);
            let is_fill = role.is_cadence_bar();

            // --- kick: 1 and (usually) the "3-and" or 3 ---
            hit(&mut out, rng, KICK, at, 0, eighth, scale(96, factor), true);
            if rng.chance((60.0 * factor) as u32, 100) {
                hit(&mut out, rng, KICK, at, 5, eighth, scale(84, factor), true);
            }
            if rng.chance((35.0 * factor) as u32, 100) {
                hit(&mut out, rng, KICK, at, 2, eighth, scale(78, factor), true);
            }

            // --- snare / rimshot on 2 and 4 ---
            let main_back = if rng.chance(1, 4) { RIM } else { SNARE };
            hit(&mut out, rng, main_back, at, 2, eighth, scale(102, factor), false);
            hit(&mut out, rng, SNARE, at, 6, eighth, scale(98, factor), false);
            // Ghost notes keep the loop alive.
            for s in [3, 4, 7] {
                if rng.chance((25.0 * factor) as u32, 100) {
                    hit(&mut out, rng, SNARE, at, s, eighth, scale(38, factor), false);
                }
            }

            // --- hats: eighths with swing pushes, occasional open hat ---
            for s in 0..8u32 {
                let vel = if s % 2 == 0 { scale(58, factor) } else { scale(42, factor) };
                if rng.chance((80.0 * factor) as u32, 100) {
                    hit_plain(&mut out, HAT_CLOSED, at + swing(s, eighth), dur_for(s), vel);
                }
            }
            if role.energy != Energy::Low && rng.chance(1, 3) {
                hit_plain(&mut out, HAT_OPEN, at + swing(7, eighth), eighth, scale(50, factor));
            }

            // --- shaker sixteenths only in high energy ---
            if role.energy == Energy::High {
                for s in (0..16u32).step_by(2) {
                    if rng.chance(3, 5) {
                        let pos = at + Ticks(s as i64 * (eighth.as_i64() / 2));
                        hit_plain(&mut out, SHAKER, pos, Parts::division(8).to_ticks(), scale(30, factor));
                    }
                }
            }

            // --- cadence fill ---
            if is_fill {
                let base = at + Ticks(3 * eighth.as_i64());
                for (i, key) in [TOM_HI, TOM_LO, SNARE].into_iter().enumerate() {
                    let pos = base + Ticks(i as i64 * (eighth.as_i64() / 2));
                    hit_plain(&mut out, *key, pos, Parts::division(8).to_ticks(), scale(88 - i as u8 * 6, factor));
                }
            }

            // --- crash on the first bar of every section ---
            if role.is_downbeat_of_section() {
                hit_plain(&mut out, CRASH, at, Parts::beats(2).to_ticks(), scale(72, factor));
            }
        }
        out
    }
}

fn scale(v: u8, f: f64) -> u8 {
    ((v as f64 * f).clamp(15.0, 120.0)) as u8
}

fn dur_for(slot: u32) -> Ticks {
    if slot % 2 == 0 {
        Parts::division(4).to_ticks()
    } else {
        Parts::division(8).to_ticks()
    }
}

/// Absolute position of eighth slot `s`, pushing odd slots late by 12%.
fn swing(s: u32, eighth: Ticks) -> Ticks {
    let base = eighth.as_i64() * s as i64;
    let push = if s % 2 == 1 { (eighth.as_i64() as f64 * 0.12) as i64 } else { 0 };
    Ticks(base + push)
}

fn hit(out: &mut Vec<NoteEvent>, rng: &mut Rand, key: u8, at: Ticks, slot: u32, eighth: Ticks, vel: u8, drag: bool) {
    // Kick drags slightly late; snare pushes fractionally early sometimes.
    let jitter = if drag { rng.range_i64(0, 40) } else { rng.range_i64(-25, 26) };
    let pos = at + swing(slot, eighth) + Ticks(jitter);
    out.push(NoteEvent::Note(NoteOn {
        pitch: Pitch(key),
        velocity: vel.clamp(10, 127),
        start: Ticks(pos.as_i64().max(at.as_i64())),
        duration: Parts::division(8).to_ticks(),
        program: 0,
    }));
}

fn hit_plain(out: &mut Vec<NoteEvent>, key: u8, at: Ticks, dur: Ticks, vel: u8) {
    out.push(NoteEvent::Note(NoteOn {
        pitch: Pitch(key),
        velocity: vel.clamp(10, 127),
        start: at,
        duration: dur,
        program: 0,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp(bars: u32) -> SongBlueprint {
        SongBlueprint::generate(&mut Rand::new(bars as u64 * 17 + 9), bars)
    }

    #[test]
    fn drums_rest_in_ambient_sections() {
        let bp = bp(16);
        let evs = DrumsVoice::generate(&bp, &mut Rand::new(1));
        let outro_start = {
            let mut acc = 0u32;
            for s in &bp.sections {
                if s.kind == crate::structure::SectionKind::Outro {
                    break;
                }
                acc += s.bars;
            }
            bar_start(&bp, acc)
        };
        assert!(
            !evs.iter().any(|e| matches!(e, NoteEvent::Note(n) if n.start >= outro_start)),
            "drums played during the outro"
        );
    }

    #[test]
    fn backbeat_is_present_every_grooving_bar() {
        let bp = bp(32);
        let evs = DrumsVoice::generate(&bp, &mut Rand::new(2));
        for bar in 0..bp.total_bars() {
            let role = bp.role_at(bar).unwrap();
            if role.energy.groove_factor() == 0.0 {
                continue;
            }
            let at = bar_start(&bp, bar);
            let has_snare_on_4 = evs.iter().any(|e| {
                if let NoteEvent::Note(n) = e {
                    (n.pitch.midi() == SNARE || n.pitch.midi() == RIM)
                        && n.start >= at + 2 * Parts::division(2).to_ticks()
                        && n.start < at + 4 * Parts::division(2).to_ticks()
                } else {
                    false
                }
            });
            assert!(has_snare_on_4, "bar {bar} lost its backbeat");
        }
    }

    #[test]
    fn velocities_are_musical() {
        let evs = DrumsVoice::generate(&bp(24), &mut Rand::new(3));
        assert!(!evs.is_empty());
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!((10..=127).contains(&n.velocity));
            }
        }
    }

    #[test]
    fn deterministic() {
        let a = format!("{:?}", DrumsVoice::generate(&bp(24), &mut Rand::new(5)));
        let b = format!("{:?}", DrumsVoice::generate(&bp(24), &mut Rand::new(5)));
        assert_eq!(a, b);
    }
}
