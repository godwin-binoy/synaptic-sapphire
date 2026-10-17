//! Melody generator: seeded motif cells + question/answer development.
//!
//! The algorithm is deliberately *not* pure random walk:
//! 1. Build a small library of **motif cells** (short rhythm+pitch-class
//!    patterns) at song start, derived from the seed.
//! 2. Each phrase asks/answers: pick a cell for the antecedent, then choose its
//!    retrograde/inversion/transposed variant for the consequent, ending on a
//!    stable chord tone (root or seventh).
//! 3. Notes are snapped to the current chord's pitch classes with chromatic
//!    approach notes only on weak slots.
//! 4. Density and register follow section energy; cadence bars get longer,
//!    more conclusive notes.

use crate::music::{NoteEvent, NoteOn, Parts, Pitch, Ticks};
use crate::rng::Rand;
use crate::structure::{Energy, SongBlueprint};
use crate::theory::pc;

use super::{bar_start, Voice};

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

    fn generate(bp: &SongBlueprint, rng: &mut Rand) -> Vec<NoteEvent> {
        let cells = build_cells(rng);
        let mut out = Vec::new();

        // Phrases are 2 bars (antecedent + consequent).
        let mut bar = 0u32;
        while bar < bp.total_bars() {
            let role = bp.role_at(bar).expect("inside blueprint");
            if role.energy == Energy::Ambient {
                bar += 2;
                continue;
            }

            let density = role.energy.melody_density();
            let (a, b) = pick_phrase(&cells, rng, density);
            emit_cell(&mut out, bp, rng, bar, &a, density, false);
            if bar + 1 < bp.total_bars() {
                let ends_song = bar + 2 >= bp.total_bars();
                emit_cell(&mut out, bp, rng, bar + 1, &b, density, ends_song || bp.role_at(bar + 1).map(|r| r.is_cadence_bar()).unwrap_or(false));
            }
            bar += 2;
        }
        out
    }
}

/// Create 4..6 motif cells from the seed. Rhythms avoid downbeat-only or
/// all-eighths extremes so melodies breathe.
fn build_cells(rng: &mut Rand) -> Vec<Cell> {
    let n = rng.range_i64(4, 7) as usize;
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

/// Question/answer: two cells, the second derived from the first.
fn pick_phrase(cells: &[Cell], rng: &mut Rand, density: f64) -> (Cell, Cell) {
    let a = cells[rng.pick_index(cells).expect("non-empty")].clone();
    let b = match rng.range_i64(0, 3) {
        0 => invert(&a),          // mirror contour
        1 => retrograde(&a),      // reverse rhythm feel
        _ => transpose_cell(&a, rng.range_i64(-2, 3) as i32),
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
    rng: &mut Rand,
    bar: u32,
    cell: &Cell,
    density: f64,
    conclusive: bool,
) {
    let Some(spec) = bp.chord_at(bar) else { return };
    let tones = bp.scale.tones(bp.tonic_pc);
    let chord_pcs: Vec<usize> = spec
        .quality
        .intervals
        .iter()
        .map(|iv| pc(spec.root_pc + *iv as usize))
        .collect();

    // Anchor degree: a chord tone nearest C4-ish octave 5 melody register.
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
        let mut p = degree_to_melody_pitch(bp, step);

        // Weak beats may use a chromatic neighbour for tension...
        let weak = slot % 2 == 1;
        if weak && rng.chance(1, 8) {
            p = p.transpose(if rng.chance(1, 2) { 1 } else { -1 });
        }
        // ...but strong beats must be chord tones.
        if !weak && !chord_pcs.contains(&p.pitch_class()) {
            p = nearest_chord_tone(p, &chord_pcs);
        }
        // Consequent/cadence notes resolve onto root or seventh.
        if conclusive && i + 1 == cell.rhythm.len() {
            let target = chord_pcs[0];
            p = nearest_with_pc(p, target);
        }

        // Duration: distance to the next attack; final note rings longer on
        // conclusive bars so phrases breathe out.
        let dur = match cell.rhythm.get(i + 1) {
            Some(next) => Ticks((*next as i64 - slot as i64).max(1) * eighth.as_i64()),
            None if conclusive => Parts::beats(2).to_ticks(),
            None => Ticks(2 * eighth.as_i64()),
        };

        let vel = velocity_for(slot, conclusive && i + 1 == cell.rhythm.len(), rng);
        let start = swing_eighth(bar_at, slot);
        out.push(NoteEvent::Note(NoteOn { pitch: p, velocity: vel, start, duration: dur, program: Self::PROGRAM }));
    }
}

/// Scale degree (possibly negative or >6) to an absolute pitch in the melody
/// register (C4..B5).
fn degree_to_melody_pitch(bp: &SongBlueprint, degree: i32) -> Pitch {
    let n = bp.scale.degree_count() as i32;
    let oct_shift = degree.div_euclid(n);
    let idx = degree.rem_euclid(n) as usize;
    let pcv = bp.scale.degree_pc(bp.tonic_pc, idx);
    Pitch::from_pc_octave(pcv, 5 + oct_shift)
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

fn velocity_for(slot: u32, conclusive: bool, rng: &mut Rand) -> u8 {
    let base = match slot % 4 {
        0 => 96,
        2 => 84,
        _ => 70,
    };
    let jitter = rng.range_i64(-8, 9) as i16;
    (base as i16 + jitter + if conclusive { 6 } else { 0 }).clamp(45, 115) as u8
}

/// Eighth-slot position with lofi late-swing on off-beats.
fn swing_eighth(at: Ticks, slot: u32) -> Ticks {
    let eighth = Parts::division(2).to_ticks().as_i64();
    let push = if slot % 2 == 1 { (eighth as f64 * 0.08) as i64 } else { 0 };
    Ticks(at.as_i64() + slot as i64 * eighth + push)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp(bars: u32) -> SongBlueprint {
        SongBlueprint::generate(&mut Rand::new(bars as u64 * 11 + 4), bars)
    }

    #[test]
    fn melody_notes_stay_in_register() {
        let evs = MelodyVoice::generate(&bp(48), &mut Rand::new(1));
        assert!(!evs.is_empty());
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                assert!(
                    (60..=81).contains(&n.pitch.midi()),
                    "{} outside melody register",
                    n.pitch.name()
                );
            }
        }
    }

    #[test]
    fn strong_beats_are_chord_tones() {
        let bp = bp(32);
        let evs = MelodyVoice::generate(&bp, &mut Rand::new(2));
        for e in &evs {
            if let NoteEvent::Note(n) = e {
                let rel = (n.start.as_i64() % (4 * 1920)) as f64 / 1920.0;
                let is_strong = (rel * 2.0).round() % 2.0 == 0.0;
                if is_strong {
                    let bar = (n.start.as_i64() / (4 * 1920)) as u32;
                    let spec = bp.chord_at(bar).unwrap();
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

    #[test]
    fn deterministic() {
        let a = format!("{:?}", MelodyVoice::generate(&bp(32), &mut Rand::new(6)));
        let b = format!("{:?}", MelodyVoice::generate(&bp(32), &mut Rand::new(6)));
        assert_eq!(a, b);
    }

    #[test]
    fn cells_have_sorted_unique_rhythm_slots() {
        let cells = build_cells(&mut Rand::new(12));
        assert!(!cells.is_empty());
        for c in &cells {
            assert!(!c.rhythm.is_empty());
            for w in c.rhythm.windows(2) {
                assert!(w[0] < w[1]);
            }
            assert_eq!(c.rhythm.len(), c.contour.len());
        }
    }
}
