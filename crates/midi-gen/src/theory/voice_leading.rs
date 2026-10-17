//! Close-position voicing with voice-leading continuity.
//!
//! Given a previous chord voicing and a new [`Chord`], pick absolute pitches
//! that (a) share as many common tones as possible, (b) move each replaced
//! voice by the smallest distance, and (c) stay inside a sane register.

use crate::music::{Chord, Pitch};
use crate::theory::pc;

/// Voice-leading state: current absolute pitches, ascending.
pub type Voicing = Vec<Pitch>;

/// The pitch class of chord tone `iv` above `root_pc`.
fn tone_pc(root_pc: usize, iv: u8) -> usize {
    pc(root_pc + iv as usize)
}

/// Build a root-position close voicing from scratch.
fn root_position(chord: &Chord) -> Voicing {
    chord.voicing()
}

/// Place `target_pc` in the octave closest to `reference`, clamped to range.
fn nearest_with_pc(reference: Pitch, target_pc: usize) -> Pitch {
    let mut best = reference.transpose(0);
    // Scan +-12 semitones for the nearest note with the wanted pitch class.
    for d in 0..=12i32 {
        for sign in [d, -d] {
            let cand = reference.transpose(sign);
            if cand.pitch_class() == target_pc {
                best = cand;
                return clamp(best);
            }
        }
    }
    clamp(best)
}

fn clamp(p: Pitch) -> Pitch {
    Pitch(p.midi().clamp(crate::music::PITCH_MIN, crate::music::PITCH_MAX))
}

/// Produce a voicing for `chord` that leads smoothly from `prev`.
///
/// Algorithm ("nearest common tone"):
/// 1. Without a previous voicing, emit a root-position close voicing.
/// 2. Otherwise walk the old voices low-to-high; keep any whose pitch class is
///    still in the new chord (common tone), otherwise re-spell it as the
///    *lowest* missing chord tone placed at minimal distance. Extra voices are
///    dropped after all chord tones are covered.
pub fn voice_chord(prev: Option<&Voicing>, chord: &Chord) -> Voicing {
    let pcs: Vec<usize> = chord.intervals.iter().map(|iv| tone_pc(chord.root_pc, *iv)).collect();

    let Some(prev) = prev else {
        return root_position(chord);
    };

    let mut out: Voicing = Vec::new();
    let mut covered: Vec<usize> = Vec::new();

    for p in prev {
        let want_missing = pcs.iter().find(|t| !covered.contains(t));
        if pcs.contains(&p.pitch_class()) && !covered.contains(&p.pitch_class()) {
            out.push(*p);
            covered.push(p.pitch_class());
        } else if let Some(&missing_pc) = want_missing {
            let moved = nearest_with_pc(*p, missing_pc);
            out.push(moved);
            covered.push(missing_pc);
        }
        // else: duplicate pitch class or no tone left to cover -> drop voice.
    }

    // Any chord tone nobody took (e.g. chord grew more voices): add near top.
    for t in &pcs {
        if !covered.contains(t) {
            let anchor = out.last().copied().unwrap_or(Pitch::from_pc_octave(*t, chord.octave));
            let added = nearest_with_pc(anchor, *t);
            out.push(added);
            covered.push(*t);
        }
    }

    out.sort();
    out.dedup_by(|a, b| a.midi() == b.midi());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::chord::{ChordQuality, ChordSpec};

    fn spec(root: usize, q: ChordQuality, oct: i32) -> Chord {
        ChordSpec::new(root, q, oct, 1).to_chord()
    }

    #[test]
    fn first_voicing_is_root_position() {
        let c = spec(9, ChordQuality::minor_7(), 3);
        let v = voice_chord(None, &c);
        assert_eq!(v[0].name(), "A2");
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn common_tones_are_preserved() {
        let a = spec(1, ChordQuality::minor_7(), 3); // Dm7: D F A C
        let va = voice_chord(None, &a);
        let b = spec(4, ChordQuality::dom_7(), 3); // G7: G B D F
        let vb = voice_chord(Some(&va), &b);
        // D and F are shared; they should be literally unchanged.
        for p in &va {
            if b.intervals.iter().any(|iv| tone_pc(b.root_pc, *iv) == p.pitch_class()) {
                assert!(vb.contains(p), "{} lost from {:?}", p.name(), vb);
            }
        }
    }

    #[test]
    fn movement_is_small() {
        let chain = [
            spec(0, ChordQuality::major_7(), 3),
            spec(9, ChordQuality::minor_7(), 3),
            spec(5, ChordQuality::major_7(), 3),
            spec(7, ChordQuality::dom_7(), 3),
        ];
        let mut prev: Option<Voicing> = None;
        for c in &chain {
            let v = voice_chord(prev.as_ref(), c);
            if let Some(p) = &prev {
                let moved: Vec<i32> = p
                    .iter()
                    .filter(|x| !v.contains(x))
                    .map(|x| {
                        v.iter()
                            .map(|y| (y.midi() as i32 - x.midi() as i32).abs())
                            .min()
                            .unwrap()
                    })
                    .collect();
                for m in moved {
                    assert!(m <= 6, "voice leapt {m} semitones: {p:?} -> {v:?}");
                }
            }
            prev = Some(v);
        }
    }

    #[test]
    fn voicings_stay_sorted_bounded_and_cover_all_tones() {
        let mut prev: Option<Voicing> = None;
        for root in 0..12u8 {
            let c = spec(root as usize, ChordQuality::minor_9(), 3);
            let v = voice_chord(prev.as_ref(), &c);
            for w in v.windows(2) {
                assert!(w[0] < w[1]);
            }
            let pcs: Vec<usize> = c.intervals.iter().map(|iv| tone_pc(c.root_pc, *iv)).collect();
            for t in &pcs {
                assert!(v.iter().any(|p| p.pitch_class() == *t), "tone {t} missing in {v:?}");
            }
            assert!(v.len() <= 6);
            prev = Some(v);
        }
    }
}
