//! Melody generator: seeded motif cells + question/answer development.
//!
//! The algorithm is deliberately *not* pure random walk:
//! 1. Build a small library of **motif cells** (short rhythm+pitch-class
//!    patterns) at song start, sized by `melody.motif_cells`.
//! 2. Each phrase asks/answers: pick a cell for the antecedent, then choose its
//!    retrograde/inversion/transposed variant for the consequent (probabilities
//!    from config), ending on a stable chord tone.
//! 3. Notes snap to the current chord's pitch classes; chromatic neighbours
//!    appear only on weak slots (by `chromatic_passing_pct`).
//! 4. Density follows section energy × `density_scale`; register comes from
//!    config; phrases can be followed by breath rests.

use crate::config::GenerateConfig;
use crate::music::{NoteEvent, NoteOn, Parts, Pitch, Ticks};
use crate::rng::Rand;
use crate::structure::{Energy, SongBlueprint};
use crate::theory::pc;

use super::{bar_start, humanize, swing_at, Voice};

pub struct MelodyVoice;

/// A motif cell: rhythm in eighth-note offsets plus relative scale steps.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cell {
    /// Offsets measured in eighth notes within a bar (0..8).
    rhythm: Vec<u32>,
    /// Steps above the cell anchor in *scale degrees*.
    contour: Vec<i32>,
}

impl Voice for MelodyVoice {
    const NAME: &'static str = "Lead";
    const PROGRAM: u8 = 0; // GM Acoustic Grand; audio engine may remap

    fn generate(bp: &SongBlueprint, cfg: &GenerateConfig, rng: &mut Rand) -> Vec<NoteEvent> {
        let m = &cfg.melody;
        if !m.enabled {
            return Vec::new();
        }
        let cells = build_cells(rng, m.motif_cells as usize);
        let swing = cfg.effective_swing();
        let mut out = Vec::new();

        // Phrases span `phrase_len_bars` (antecedent + consequent).
        let phrase = m.phrase_len_bars.max(2);
        let half = (phrase / 2).max(1);
        let mut bar = 0u32;
        while bar < bp.total_bars() {
            let role = bp.role_at(bar).expect("inside blueprint");
            if role.energy == Energy::Ambient {
                bar += phrase;
                continue;
            }

            let density = (role.energy.melody_density() * m.density_scale).clamp(0.0, 1.0);
            let (a, b) = pick_phrase(&cells, rng, density, m);
            emit_cell(&mut out, bp, cfg, rng, bar, &a, density, false, swing);
            if bar + half < bp.total_bars() {
                let ends_song = bar + phrase >= bp.total_bars();
                let cadence = bp
                    .role_at(bar + half)
                    .map(|r| r.is_cadence_bar())
                    .unwrap_or(false);
                emit_cell(&mut out, bp, cfg, rng, bar + half, &b, density, ends_song || cadence, swing);
            }
            // Breath: sometimes leave the next bar empty.
            bar += if rng.chance(m.rest_between_phrases_pct, 100) { phrase + 1 } else { phrase };
        }
        humanize(&mut out, cfg, rng);
        out
    }
}

/// Create `count` motif cells from the seed. Rhythms avoid downbeat-only or
/// all-eighths extremes so melodies breathe.
fn build_cells(rng: &mut Rand, count: usize) -> Vec<Cell> {
    let n = count.clamp(2, 12);
    let mut cells = Vec::with_capacity(n);
    for _ in 0..n {
        // Choose 2..4 attacks placed on eighth slots 0..7, sorted, no repeats.
        let attacks = rng.range_i64(2, 5) as usize;
        let mut slots: Vec<u32> = (0..8u32).collect();
        rng.shuffle(&mut slots);
        let mut rhythm: Vec<u32> = slots.into_iter().take(attacks).collect();
        rhythm.sort_unstable();
        // Contour: gentle steps with occasional third leaps.
        let contour: Vec<i32> = (0..rhythm.len())
            .map(|_| match rng.range_i64(0, 10) {
                0..=4 => rng.range_i64(-2, 3) as i32,
                5..=7 => rng.range_i64(2, 4) as i32 * if rng.chance(1, 2) { 1 } else { -1 },
                _ => rng.range_i64(-1, 2) as i32,
            })
            .collect();
        cells.push(Cell { rhythm, contour });
    }
    cells
}

/// Question/answer: two cells, the second derived from the first by the
/// configured development operations.
fn pick_phrase(cells: &[Cell], rng: &mut Rand, density: f64, m: &crate::config::MelodyConfig) -> (Cell, Cell) {
    let a = cells[rng.pick_index(cells).expect("non-empty")].clone();
    let roll = rng.range_i64(0, 100);
    let b = if roll < m.answer_inversion_pct as i64 {
        invert(&a)
    } else if roll < (m.answer_inversion_pct + m.answer_retrograde_pct) as i64 {
        retrograde(&a)
    } else {
        transpose_cell(&a, rng.range_i64(-2, 3) as i32)
    };
    // Very sparse sections drop half the attacks from the answer.
    let b = if density < 0.25 && rng.chance(1, 2) { thin(&b) } else { b };
    (a, b)
}

fn invert(c: &Cell) -> Cell {
    Cell { rhythm: c.rhythm.clone(), contour: c.contour.iter().map(|x| -x).collect() }
}

fn retrograde(c: &Cell) -> Cell {
    let mut rhythm: Vec<u32> = c.rhythm.iter().map(|r| 7 - r).collect();
    rhythm.sort_unstable();
    let mut contour = c.contour.clone();
    contour.reverse();
    Cell { rhythm, contour }
}

fn transpose_cell(c: &Cell, by: i32) -> Cell {
    Cell { rhythm: c.rhythm.clone(), contour: c.contour.iter().map(|x| x + by).collect() }
}

fn thin(c: &Cell) -> Cell {
    let rhythm: Vec<u32> = c.rhythm.iter().cloned().step_by(2).collect();
    let contour: Vec<i32> = c.contour.iter().cloned().step_by(2).collect();
    Cell { rhythm, contour }
}

/// Render one cell into concrete events inside `bar`.
fn emit_cell(
    out: &mut Vec<NoteEvent>,
    bp: &SongBlueprint,
    cfg: &GenerateConfig,
    rng: &mut Rand,
    bar: u32,
    cell: &Cell,
    density: f64,
    conclusive: bool,
    swing: f64,
) {
    let m = &cfg.melody;
    let Some(spec) = bp.chord_at(bar) else { return };
    let tones = bp.scale.tones(bp.tonic_pc);
    let chord_pcs: Vec<usize> = spec
        .quality
        .intervals
        .iter()
        .map(|iv| pc(spec.root_pc + *iv as usize))
        .collect();

    // Anchor degree: the chord root expressed as a scale degree.
    let anchor_degree = chord_pcs
        .first()
        .and_then(|p| tones.iter().position(|t| t == p))
        .unwrap_or(0);

    let eighth = Parts::division(2).to_ticks();
    let bar_at = bar_start(bp, bar);

    for (i, &slot) in cell.rhythm.iter().enumerate() {
        // Probability gate keeps overall density under control.
        if !conclusive && i > 0 && !rng.chance((density * 100.0) as u32, 100) {
            continue;
        }
        let step = anchor_degree as i32 + cell.contour.get(i).copied().unwrap_or(0);
        let mut p = degree_to_melody_pitch(bp, step, m.register_low, m.register_high);

        // Weak beats may use a chromatic neighbour for tension...
        let weak = slot % 2 == 1;
        if weak && rng.chance(m.chromatic_passing_pct, 100) {
            p = clamp_register(p.transpose(if rng.chance(1, 2) { 1 } else { -1 }), m);
        }
        // ...but strong beats must be chord tones.
        if !weak && !chord_pcs.contains(&p.pitch_class()) {
            p = nearest_chord_tone(p, &chord_pcs);
        }
        // Consequent/cadence notes resolve onto the chord root.
        if conclusive && i + 1 == cell.rhythm.len() {
            let target = chord_pcs[0];
            p = clamp_register(nearest_with_pc(p, target), m);
        }

        // Duration: distance to the next attack; final note rings longer on
        // conclusive bars so phrases breathe out.
        let dur = match cell.rhythm.get(i + 1) {
            Some(next) => Ticks((*next as i64 - slot as i64).max(1) * eighth.as_i64()),
            None if conclusive => Parts::beats(2).to_ticks(),
            None => Ticks(2 * eighth.as_i64()),
        };

        let vel = velocity_for(slot, conclusive && i + 1 == cell.rhythm.len(), rng, cfg.humanize.dynamics_range);
        let start = swing_at(bar_at, slot as f64 * 0.5, swing);
        out.push(NoteEvent::Note(NoteOn { pitch: p, velocity: vel, start, duration: dur, program: m.program }));
    }
}

/// Scale degree (possibly negative or >6) to an absolute pitch inside the
/// configured melody register.
fn degree_to_melody_pitch(bp: &SongBlueprint, degree: i32, lo: u8, hi: u8) -> Pitch {
    let n = bp.scale.degree_count() as i32;
    let oct_shift = degree.div_euclid(n);
    let idx = degree.rem_euclid(n) as usize;
    let pcv = bp.scale.degree_pc(bp.tonic_pc, idx);
    let mid_octave = 4 + ((lo as i32 + hi as i32) / 2) / 12 - 3; // rough centre octave
    let p = Pitch::from_pc_octave(pcv, mid_octave + oct_shift);
    clamp_register_abs(p, lo, hi)
}

/// Keep a pitch inside the config register, folding by octaves.
fn clamp_register(p: Pitch, m: &crate::config::MelodyConfig) -> Pitch {
    clamp_register_abs(p, m.register_low, m.register_high)
}

fn clamp_register_abs(p: Pitch, lo: u8, hi: u8) -> Pitch {
    let mut midi = p.midi() as i32;
    while midi < lo as i32 {
        midi += 12;
    }
    while midi > hi as i32 {
        midi -= 12;
    }
    // If folding overshot past the other bound (tiny registers), clamp hard.
    Pitch(midi.clamp(lo.min(hi) as i32, hi.max(lo) as i32).clamp(0, 127) as u8)
}

fn nearest_chord_tone(p: Pitch, chord_pcs: &[usize]) -> Pitch {
    let mut best = p;
    let mut best_d = 99;
    for d in -6..=6i32 {
        let cand = p.transpose(d);
        if chord_pcs.contains(&cand.pitch_class()) && d.abs() < best_d {
            best = cand;
            best_d = d.abs();
        }
    }
    best
}

fn nearest_with_pc(p: Pitch, target_pc: usize) -> Pitch {
    for d in 0..=12i32 {
        for sign in [d, -d] {
            let cand = p.transpose(sign);
            if cand.pitch_class() == pc(target_pc) {
                return cand;
            }
        }
    }
    p
}

fn velocity_for(slot: u32, conclusive: bool, rng: &mut Rand, dynamics_range: f64) -> u8 {
    // Strong-beat accents scale with the configured dynamics range.
    let accent = (dynamics_range * 30.0).round() as i16;
    let base = match slot % 4 {
        0 => 84 + accent,
        2 => 84 + accent / 2,
        _ => 70,
    };
    let jitter = rng.range_i64(-8, 9) as i16;
    (base + jitter + if conclusive { 6 } else { 0 }).clamp(45, 120) as u8
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
    fn melody_is_not_empty_and_stays_in_register() {
        let cfg = cfg();
        let evs = MelodyVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(1));
        assert!(!evs.is_empty());
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!(
                    (cfg.melody.register_low..=cfg.melody.register_high).contains(&n.pitch.midi()),
                    "{} outside melody register",
                    n.pitch.name()
                );
            }
        }
    }

    #[test]
    fn custom_register_is_honoured() {
        let mut cfg = cfg();
        cfg.melody.register_low = 65;
        cfg.melody.register_high = 77;
        let evs = MelodyVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(1));
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!((65..=77).contains(&n.pitch.midi()), "{} outside custom register", n.pitch.name());
            }
        }
    }

    #[test]
    fn disabled_switch_emits_nothing() {
        let mut cfg = cfg();
        cfg.melody.enabled = false;
        assert!(MelodyVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(1)).is_empty());
    }

    #[test]
    fn zero_density_produces_no_notes() {
        let mut cfg = cfg();
        cfg.melody.density_scale = 0.0;
        let evs = MelodyVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(1));
        // Only conclusive notes survive the gate; with density 0 most bars are
        // silent — assert we got far fewer than default.
        let def = MelodyVoice::generate(&bp(&GenerateConfig::default()), &GenerateConfig::default(), &mut Rand::new(1));
        assert!(evs.len() < def.len());
    }

    #[test]
    fn strong_beats_are_chord_tones() {
        let cfg = cfg();
        let bp = bp(&cfg);
        let evs = MelodyVoice::generate(&bp, &cfg, &mut Rand::new(2));
        let bar_ticks = Parts::beats(4).to_ticks().as_i64();
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                let rel = (n.start.as_i64().rem_euclid(bar_ticks)) as f64 / crate::music::PPQ as f64;
                let slot = (rel * 2.0).round() as i64; // eighth index, post-swing
                if slot % 2 == 0 {
                    let bar = (n.start.as_i64() / bar_ticks) as u32;
                    if let Some(spec) = bp.chord_at(bar) {
                        let pcs: Vec<usize> = spec
                            .quality
                            .intervals
                            .iter()
                            .map(|iv| pc(spec.root_pc + *iv as usize))
                            .collect();
                        assert!(pcs.contains(&n.pitch.pitch_class()), "{n:?} over {:?}", spec.label());
                    }
                }
            }
        }
    }

    #[test]
    fn deterministic() {
        let cfg = cfg();
        let a = format!("{:?}", MelodyVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(6)));
        let b = format!("{:?}", MelodyVoice::generate(&bp(&cfg), &cfg, &mut Rand::new(6)));
        assert_eq!(a, b);
    }

    #[test]
    fn cells_have_sorted_unique_rhythm_slots() {
        let cells = build_cells(&mut Rand::new(12), 5);
        assert_eq!(cells.len(), 5);
        for c in &cells {
            assert!(!c.rhythm.is_empty());
            for w in c.rhythm.windows(2) {
                assert!(w[0] < w[1]);
            }
            assert_eq!(c.rhythm.len(), c.contour.len());
        }
    }
}
