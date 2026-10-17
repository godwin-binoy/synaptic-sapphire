//! Electric-piano chord comping with voice leading, extensions and swing.
//!
//! Strategy per bar:
//! 1. Voice the current chord from the previous voicing ([`voice_chord`]).
//! 2. Optionally add colour tones (9ths/11ths/13ths) by `tension_chance_pct`.
//! 3. Choose a comping pattern by energy: whole-bar pad hit, "lofi pulse"
//!    (downbeat + delayed off-beat stab), or syncopated double stab — every
//!    attack gated by its configured probability.
//! 4. Apply humanisation: swing pushes, velocity accents, jitter.

use crate::config::GenerateConfig;
use crate::music::{NoteEvent, NoteOn, Parts, Pitch, Ticks};
use crate::rng::Rand;
use crate::structure::{Energy, SongBlueprint};
use crate::theory::voice_leading::voice_chord;

use super::{bar_len, bar_start, humanize, swing_at, Voice};

pub struct ChordsVoice;

impl Voice for ChordsVoice {
    const NAME: &'static str = "Rhodes";
    const PROGRAM: u8 = 4; // GM "Electric Piano 1"

    fn generate(bp: &SongBlueprint, cfg: &GenerateConfig, rng: &mut Rand) -> Vec<NoteEvent> {
        let c = &cfg.chords;
        if !c.enabled {
            return Vec::new();
        }
        let swing = cfg.effective_swing();
        let mut out = Vec::new();
        let mut prev: Option<Vec<Pitch>> = None;

        for bar in 0..bp.total_bars() {
            let Some(spec) = bp.chord_at(bar) else { continue };
            let role = bp.role_at(bar).expect("bar inside blueprint");
            let mut chord = spec.to_chord();
            // Root octave follows the config knob.
            chord.octave = c.voicing_octave;

            // Colour tones: occasionally stack a ninth/eleventh/thirteenth.
            if rng.chance(c.tension_chance_pct, 100) {
                let tension = [14u8, 16, 21][rng.range_i64(0, 3) as usize];
                if !chord.intervals.contains(&tension) {
                    chord.intervals.push(tension);
                    chord.intervals.sort_unstable();
                }
            }

            let mut voicing = voice_chord(prev.as_ref(), &chord);
            // Cap simultaneous voices, keeping bass + top and dropping middles.
            while voicing.len() > c.max_voices as usize {
                let mid = voicing.len() / 2;
                voicing.remove(mid);
            }
            prev = Some(voicing.clone());

            // Record the harmony itself for downstream renderers.
            out.push(NoteEvent::Harmony(chord));

            match role.energy {
                Energy::Ambient => {
                    if c.pad_enabled {
                        // Long, soft pad covering the whole bar.
                        let dur = beats_ticks(c.pad_sustain_beats).min(bar_len(bp));
                        push_stab(&mut out, &voicing, bar_start(bp, bar), dur, 52, c.pad_program);
                    }
                }
                Energy::Low => {
                    // One gentle downbeat hit, held ~3 beats.
                    if rng.chance(c.stab_downbeat_pct, 100) {
                        let dur = Parts::beats(3).to_ticks();
                        push_stab(&mut out, &voicing, swing_at(bar_start(bp, bar), 0.0, swing), dur, 66, c.program);
                    } else if c.pad_enabled {
                        // Downbeat skipped: breathe with a soft pad instead.
                        push_stab(&mut out, &voicing, bar_start(bp, bar), bar_len(bp), 48, c.pad_program);
                    }
                }
                Energy::Medium | Energy::High => {
                    // Downbeat + off-beat stab; high energy adds a third stab.
                    if rng.chance(c.stab_downbeat_pct, 100) {
                        let dur_short = Parts::new(0, 3, 4).to_ticks(); // dotted quarter-ish
                        push_stab(&mut out, &voicing, swing_at(bar_start(bp, bar), 0.0, swing), dur_short, 82, c.program);
                    }
                    if rng.chance(c.stab_offbeat_pct, 100) {
                        let syncopated = rng.chance(c.offbeat_syncopation_pct, 100);
                        let off = if syncopated { 2.5 } else { 2.0 };
                        push_stab(
                            &mut out,
                            &voicing,
                            swing_at(bar_start(bp, bar), off, swing),
                            Parts::division(2).to_ticks(),
                            64,
                            c.program,
                        );
                    }
                    if role.energy == Energy::High && rng.chance(c.stab_offbeat_pct / 2, 100) {
                        push_stab(
                            &mut out,
                            &voicing,
                            swing_at(bar_start(bp, bar), 3.5, swing),
                            Parts::division(4).to_ticks(),
                            58,
                            c.program,
                        );
                    }
                }
            }
        }
        humanize(&mut out, cfg, rng);
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

/// Convert a float beat count to exact ticks.
fn beats_ticks(beats: f64) -> Ticks {
    Ticks((beats * crate::music::PPQ as f64).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp(cfg: &GenerateConfig) -> SongBlueprint {
        SongBlueprint::generate(&mut Rand::new(cfg.seed), cfg)
    }

    #[test]
    fn produces_notes_for_grooving_bars() {
        let cfg = GenerateConfig::default();
        let evs = ChordsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(2));
        let notes = evs.iter().filter(|e| matches!(e, NoteEvent::Note(_))).count();
        assert!(notes > 0);
    }

    #[test]
    fn disabled_switch_emits_nothing() {
        let mut cfg = GenerateConfig::default();
        cfg.chords.enabled = false;
        let evs = ChordsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(2));
        assert!(evs.is_empty());
    }

    #[test]
    fn all_notes_fit_inside_the_song() {
        let cfg = GenerateConfig::default();
        let bp = bp(&cfg);
        let total = bar_len(&bp).as_i64() * bp.total_bars() as i64;
        let evs = ChordsVoice::generate(&bp, &cfg, &mut Rand::new(3));
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!(n.start.as_i64() >= 0 && n.end().as_i64() <= total + 120, "{n:?}");
                assert!((1..=127).contains(&n.velocity));
            }
        }
    }

    #[test]
    fn max_voices_is_respected_per_stab() {
        let mut cfg = GenerateConfig::default();
        cfg.chords.max_voices = 3;
        let evs = ChordsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(4));
        let mut by_start: std::collections::BTreeMap<i64, usize> = Default::default();
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                *by_start.entry(n.start.as_i64()).or_default() += 1;
            }
        }
        for (_, count) in by_start {
            assert!(count <= 4, "stab had {count} voices over cap 3 (+jitter overlap)");
        }
    }

    #[test]
    fn deterministic() {
        let cfg = GenerateConfig::default();
        let a = format!("{:?}", ChordsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(9)));
        let b = format!("{:?}", ChordsVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(9)));
        assert_eq!(a, b);
    }
}
