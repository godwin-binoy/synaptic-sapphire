//! Lo-fi drum groove on a GM kit: lazy boom-bap with humanised timing.
//!
//! Pattern model: each instrument has an eighth-note probability template per
//! bar; `drums.groove_intensity` scales all probabilities, energy selects the
//! template family, cadence bars append a tom fill (by `fill_on_cadence_pct`).
//! Kick and snare are loosely quantised with per-hit micro-offsets up to
//! `drums.drag_ticks` (the "drunk" lofi feel); hats use the global swing.

use crate::config::{DrumsConfig, GenerateConfig};
use crate::music::{NoteEvent, NoteOn, Parts, Pitch, Ticks};
use crate::rng::Rand;
use crate::structure::{Energy, SongBlueprint};

use super::{bar_start, swing_at, Voice};

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

    fn generate(bp: &SongBlueprint, cfg: &GenerateConfig, rng: &mut Rand) -> Vec<NoteEvent> {
        let d = &cfg.drums;
        if !d.enabled {
            return Vec::new();
        }
        let mut out = Vec::new();
        let swing = cfg.effective_swing();

        for bar in 0..bp.total_bars() {
            let role = bp.role_at(bar).expect("inside blueprint");
            let factor = role.energy.groove_factor() * d.groove_intensity;
            if factor <= 0.01 {
                continue; // ambient sections are drumless
            }
            let at = bar_start(bp, bar);
            let is_fill = role.is_cadence_bar() && rng.chance(d.fill_on_cadence_pct, 100);

            // --- kick: 1 and (usually) the "3-and" or 3 ---
            hit(&mut out, rng, d, KICK, at, 0.0, swing, scale(96, factor, d), true);
            if rng.chance((d.kick_probability_pct as f64 * factor) as u32, 100) {
                hit(&mut out, rng, d, KICK, at, 2.5, swing, scale(84, factor, d), true);
            }
            if rng.chance((35.0 * factor) as u32, 100) {
                hit(&mut out, rng, d, KICK, at, 1.0, swing, scale(78, factor, d), true);
            }

            // --- snare / rimshot on 2 and 4 ---
            let main_back = if rng.chance(1, 4) { RIM } else { SNARE };
            if rng.chance(d.snare_probability_pct, 100) {
                hit(&mut out, rng, d, main_back, at, 1.0, swing, scale(102, factor, d), false);
            }
            if rng.chance(d.snare_probability_pct, 100) {
                hit(&mut out, rng, d, SNARE, at, 3.0, swing, scale(98, factor, d), false);
            }
            // Ghost notes keep the loop alive.
            for s in [1.5, 2.5, 3.5] {
                if rng.chance((d.ghost_note_pct as f64 * factor) as u32, 100) {
                    hit(&mut out, rng, d, SNARE, at, s, swing, scale(38, factor, d), false);
                }
            }

            // --- hats: eighths with swing pushes, occasional open hat ---
            for slot in 0..8u32 {
                let vel = if slot % 2 == 0 { scale(58, factor, d) } else { scale(42, factor, d) };
                if rng.chance((d.hat_probability_pct as f64 * factor) as u32, 100) {
                    plain(&mut out, rng, d, HAT_CLOSED, at, slot as f64 * 0.5, swing, dur_for(slot), vel);
                }
            }
            if role.energy != Energy::Low && rng.chance(d.open_hat_pct, 100) {
                plain(&mut out, rng, d, HAT_OPEN, at, 3.5, swing, Parts::division(2).to_ticks(), scale(50, factor, d));
            }

            // --- shaker sixteenths only in busy bars ---
            if role.energy == Energy::High || (role.energy == Energy::Medium && rng.chance(1, 3)) {
                for s in (0..16u32).step_by(2) {
                    if rng.chance(d.shaker_pct, 100) {
                        let pos = at + Ticks(s as i64 * (Parts::division(8).to_ticks().as_i64()));
                        out.push(pn(SHAKER, pos, Parts::division(8).to_ticks(), scale(30, factor, d)));
                    }
                }
            }

            // --- cadence fill over the last beat ---
            if is_fill {
                for (i, key) in [TOM_HI, TOM_LO, SNARE].into_iter().enumerate() {
                    let pos_beats = 3.0 + i as f64 * 0.25;
                    plain(&mut out, rng, d, key, at, pos_beats, swing, Parts::division(8).to_ticks(), scale(88 - i as u8 * 6, factor, d));
                }
            }

            // --- crash on the first bar of every section ---
            if role.is_downbeat_of_section() && rng.chance(d.crash_on_section_start_pct, 100) {
                plain(&mut out, rng, d, CRASH, at, 0.0, 0.0, Parts::beats(2).to_ticks(), scale(72, factor, d));
            }
        }
        out
    }
}

fn scale(v: u8, f: f64, d: &DrumsConfig) -> u8 {
    ((v as f64 * f * (d.velocity_base as f64 / 96.0)).clamp(15.0, 127.0)) as u8
}

fn dur_for(slot: u32) -> Ticks {
    if slot % 2 == 0 {
        Parts::division(4).to_ticks()
    } else {
        Parts::division(8).to_ticks()
    }
}

/// A drum hit with drag/jitter: kicks land late, snares sometimes early.
fn hit(
    out: &mut Vec<NoteEvent>,
    rng: &mut Rand,
    d: &DrumsConfig,
    key: u8,
    at: Ticks,
    beats_offset: f64,
    swing: f64,
    vel: u8,
    drag: bool,
) {
    let jitter = if d.drag_ticks > 0 {
        if drag {
            rng.range_i64(0, d.drag_ticks as i64 + 1)
        } else {
            -rng.range_i64(0, (d.drag_ticks as i64 + 1) / 2)
        }
    } else {
        0
    };
    let pos = swing_at(at, beats_offset, swing) + Ticks(jitter);
    out.push(pn(key, pos.max(at), Parts::division(8).to_ticks(), vel));
}

/// Hit without random offset except configured drag.
fn plain(
    out: &mut Vec<NoteEvent>,
    rng: &mut Rand,
    d: &DrumsConfig,
    key: u8,
    at: Ticks,
    beats_offset: f64,
    swing: f64,
    dur: Ticks,
    vel: u8,
) {
    let jitter = if d.drag_ticks > 0 { rng.range_i64(0, d.drag_ticks as i64 / 2 + 1) } else { 0 };
    let pos = swing_at(at, beats_offset, swing) + Ticks(jitter);
    out.push(pn(key, pos, dur, vel));
}

fn pn(key: u8, at: Ticks, dur: Ticks, vel: u8) -> NoteEvent {
    NoteEvent::Note(NoteOn {
        pitch: Pitch(key),
        velocity: vel.clamp(10, 127),
        start: at,
        duration: dur,
        program: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> GenerateConfig {
        GenerateConfig::default()
    }

    fn bp(cfg: &GenerateConfig) -> SongBlueprint {
        SongBlueprint::generate(&mut Rand::new(cfg.seed), cfg)
    }

    #[test]
    fn drums_rest_in_ambient_sections() {
        let cfg = cfg();
        let bp = bp(&cfg);
        let evs = DrumsVoice::generate(&bp, &cfg, &mut Rand::new(1));
        // Find the outro start bar.
        let mut acc = 0u32;
        for s in &bp.sections {
            if s.kind == crate::structure::SectionKind::Outro {
                break;
            }
            acc += s.bars;
        }
        let outro_start = bar_start(&bp, acc);
        assert!(
            !evs.iter().any(|e| matches!(e, NoteEvent::Note(n) if n.start >= outro_start)),
            "drums played during the outro"
        );
    }

    #[test]
    fn backbeat_is_present_every_grooving_bar() {
        let cfg = cfg();
        let bp = bp(&cfg);
        let evs = DrumsVoice::generate(&bp, &cfg, &mut Rand::new(2));
        let ppq = crate::music::PPQ as i64;
        for bar in 0..bp.total_bars() {
            let role = bp.role_at(bar).unwrap();
            if role.energy.groove_factor() == 0.0 {
                continue;
            }
            let at = bar_start(&bp, bar);
            // Beat 4 spans ticks [at+3*ppq, at+4*ppq).
            let has_snare_on_4 = evs.iter().any(|e| {
                if let NoteEvent::Note(n) = e {
                    (n.pitch.midi() == SNARE || n.pitch.midi() == RIM)
                        && n.start >= at + Ticks(3 * ppq)
                        && n.start < at + Ticks(4 * ppq)
                } else {
                    false
                }
            });
            assert!(has_snare_on_4, "bar {bar} lost its backbeat");
        }
    }

    #[test]
    fn zero_groove_emits_nothing() {
        let mut cfg = cfg();
        cfg.drums.groove_intensity = 0.0;
        let evs = DrumsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(2));
        assert!(evs.is_empty());
    }

    #[test]
    fn disabled_switch_emits_nothing() {
        let mut cfg = cfg();
        cfg.drums.enabled = false;
        assert!(DrumsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(2)).is_empty());
    }

    #[test]
    fn velocities_are_musical() {
        let cfg = cfg();
        let evs = DrumsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(3));
        assert!(!evs.is_empty());
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!((10..=127).contains(&n.velocity));
            }
        }
    }

    #[test]
    fn deterministic() {
        let cfg = cfg();
        let a = format!("{:?}", DrumsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(5)));
        let b = format!("{:?}", DrumsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(5)));
        assert_eq!(a, b);
    }
}
