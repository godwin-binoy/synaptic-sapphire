//! Upright-style bass line: root-fifth groove with passing tones.
//!
//! The bass locks with the chord plan: every bar gets at least a root on beat
//! 1; medium/high energy adds fifth-and-octave motion plus chromatic approach
//! notes into chord changes. Notes stay inside the configured register so the
//! audio engine never has to compensate.

use crate::config::GenerateConfig;
use crate::music::{NoteEvent, NoteOn, Parts, Pitch, Ticks};
use crate::rng::Rand;
use crate::structure::{Energy, SongBlueprint};
use crate::theory::pc;

use super::{bar_start, humanize, swing_at, Voice};

pub struct BassVoice;

impl BassVoice {
    /// Register bounds taken from config (clamped there already).
    fn register(cfg: &GenerateConfig) -> (i32, i32) {
        (cfg.bass.register_low as i32, cfg.bass.register_high as i32)
    }
}

impl Voice for BassVoice {
    const NAME: &'static str = "Bass";
    const PROGRAM: u8 = 33; // GM "Electric Bass Finger"

    fn generate(bp: &SongBlueprint, cfg: &GenerateConfig, rng: &mut Rand) -> Vec<NoteEvent> {
        let b = &cfg.bass;
        if !b.enabled {
            return Vec::new();
        }
        let (lo, hi) = Self::register(cfg);
        let swing = cfg.effective_swing();
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

            let root = place_in_register(spec.root_pc, prev_root, lo, hi);
            let next_root = bp
                .chord_at(bar + 1)
                .map(|s| place_in_register(s.root_pc, Some(spec.root_pc), lo, hi));

            // Beat 1: the anchor. Long and warm.
            out.push(note(
                root,
                b.anchor_velocity,
                bar_start(bp, bar),
                Parts::division(2).to_ticks(),
                b.program,
            ));

            match role.energy {
                Energy::Low => {
                    // Sparse: one pickup into the next chord.
                    if rng.chance(b.pickup_chance_pct, 100) {
                        let t = swing_at(bar_start(bp, bar), 3.5, swing);
                        let p = approach_pitch(&root, &next_root.clone().unwrap_or(root), lo, hi);
                        out.push(note(p, 62, t, Parts::division(4).to_ticks(), b.program));
                    }
                }
                Energy::Medium | Energy::High => {
                    // Classic lofi heartbeat: root on 1, fifth on 2+, colour
                    // tone on 3, optional walk into the next chord.
                    if rng.chance(b.walk_chance_pct, 100) {
                        let fifth = transpose_keep_reg(&root, 7, lo, hi);
                        out.push(note(
                            fifth,
                            78,
                            swing_at(bar_start(bp, bar), 1.5, swing),
                            Parts::division(4).to_ticks(),
                            b.program,
                        ));
                    }
                    let colour = if rng.chance(b.octave_jump_chance_pct, 100) { 12 } else { 3 };
                    out.push(note(
                        transpose_keep_reg(&root, colour, lo, hi),
                        70,
                        swing_at(bar_start(bp, bar), 3.0, swing),
                        Parts::division(4).to_ticks(),
                        b.program,
                    ));
                    let wants_walk = (role.energy == Energy::High || role.is_cadence_bar())
                        && rng.chance(b.approach_chance_pct, 100);
                    if wants_walk {
                        if let Some(nr) = &next_root {
                            let ap = approach_pitch(&root, nr, lo, hi);
                            out.push(note(
                                ap,
                                66,
                                swing_at(bar_start(bp, bar), 3.5, swing),
                                Parts::division(8).to_ticks(),
                                b.program,
                            ));
                        }
                    }
                }
                _ => {}
            }
            prev_root = Some(spec.root_pc);
        }
        humanize(&mut out, cfg, rng);
        out
    }
}

/// Choose an octave for `pitch_class` near the previous root, inside register.
fn place_in_register(pitch_class: usize, prev_root: Option<usize>, lo: i32, hi: i32) -> Pitch {
    // Candidate octaves of this pitch class inside the register.
    let mut candidates: Vec<i32> = Vec::new();
    let mut m = pc(pitch_class) as i32;
    while m < lo {
        m += 12;
    }
    let mut v = m;
    while v <= hi {
        candidates.push(v);
        v += 12;
    }
    if candidates.is_empty() {
        return Pitch(pc(pitch_class) as u8 + 36);
    }
    // Anchor: previous root's absolute midi, else middle of register.
    let anchor = prev_root
        .map(|pr| {
            let mut x = pc(pr) as i32 + 36;
            while x < lo {
                x += 12;
            }
            while x > hi {
                x -= 12;
            }
            x
        })
        .unwrap_or((lo + hi) / 2);
    let best = *candidates
        .iter()
        .min_by_key(|c| (*c - anchor).abs())
        .expect("non-empty candidates");
    Pitch(best.clamp(0, 127) as u8)
}

/// Transpose keeping the result inside the register.
fn transpose_keep_reg(p: &Pitch, semitones: i32, lo: i32, hi: i32) -> Pitch {
    let mut m = p.midi() as i32 + semitones;
    while m < lo {
        m += 12;
    }
    while m > hi {
        m -= 12;
    }
    Pitch(m.clamp(lo, hi).clamp(0, 127) as u8)
}

/// Chromatic/diatonic approach note leading into `target`.
fn approach_pitch(from: &Pitch, target: &Pitch, lo: i32, hi: i32) -> Pitch {
    // Semitone below the target is the canonical jazz approach.
    let mut below = target.midi() as i32 - 1;
    if below < lo {
        below = target.midi() as i32 + 1;
    }
    let cand = Pitch(below.clamp(lo, hi).clamp(0, 127) as u8);
    // Avoid awkward leaps: if the approach is far from where we are, use the
    // whole-tone below instead.
    if from.interval_to(cand).abs() > 9 {
        let alt = (target.midi() as i32 - 2).clamp(lo, hi);
        Pitch(alt.clamp(0, 127) as u8)
    } else {
        cand
    }
}

fn note(pitch: Pitch, velocity: u8, start: Ticks, duration: Ticks, program: u8) -> NoteEvent {
    NoteEvent::Note(NoteOn { pitch, velocity, start, duration, program })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::SectionKind;

    fn bp(cfg: &GenerateConfig) -> SongBlueprint {
        SongBlueprint::generate(&mut Rand::new(cfg.seed), cfg)
    }

    #[test]
    fn bass_stays_in_register() {
        let cfg = GenerateConfig::default();
        let (lo, hi) = BassVoice::register(&cfg);
        let evs = BassVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(1));
        assert!(!evs.is_empty());
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!(
                    ((lo as u8)..=(hi as u8)).contains(&n.pitch.midi()),
                    "{} out of bass register",
                    n.pitch.name()
                );
            }
        }
    }

    #[test]
    fn custom_register_is_honoured() {
        let mut cfg = GenerateConfig::default();
        cfg.bass.register_low = 48;
        cfg.bass.register_high = 60;
        let evs = BassVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(1));
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!((48..=60).contains(&n.pitch.midi()), "{} outside custom register", n.pitch.name());
            }
        }
    }

    #[test]
    fn disabled_switch_emits_nothing() {
        let mut cfg = GenerateConfig::default();
        cfg.bass.enabled = false;
        assert!(BassVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(1)).is_empty());
    }

    #[test]
    fn downbeat_is_always_the_chord_root() {
        let cfg = GenerateConfig::default();
        let bp = bp(&cfg);
        let evs = BassVoice::generate(&bp, &cfg, &mut Rand::new(7));
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
        let cfg = GenerateConfig::default();
        let bp = bp(&cfg);
        let evs = BassVoice::generate(&bp, &cfg, &mut Rand::new(3));
        for bar in 0..bp.total_bars() {
            if bp.role_at(bar).unwrap().section == SectionKind::Outro {
                let start = bar_start(&bp, bar);
                assert!(
                    !evs.iter().any(|e| matches!(e, NoteEvent::Note(n) if n.start >= start)),
                    "bass plays during outro bar {bar}"
                );
            }
        }
    }

    #[test]
    fn deterministic() {
        let cfg = GenerateConfig::default();
        let a = format!("{:?}", BassVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(5)));
        let b = format!("{:?}", BassVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(5)));
        assert_eq!(a, b);
    }
}
