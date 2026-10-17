//! Scales as pitch-class step patterns with diatonic chord derivation.

use crate::theory::{pc, pc_distance, NOTE_NAMES};

/// A named scale defined by ascending semitone steps from the tonic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scale {
    pub name: &'static str,
    /// Steps between consecutive degrees, e.g. `[2,2,1,2,2,2,1]` for major.
    pub steps: &'static [u8],
}

impl Scale {
    /// Major (ionian).
    pub const MAJOR: Scale = Scale { name: "major", steps: &[2, 2, 1, 2, 2, 2, 1] };
    /// Natural minor (aeolian).
    pub const MINOR: Scale = Scale { name: "minor", steps: &[2, 1, 2, 2, 1, 2, 2] };
    /// Dorian mode — the jazzy minor with a bright sixth, very common in lofi.
    pub const DORIAN: Scale = Scale { name: "dorian", steps: &[2, 1, 2, 2, 2, 1, 2] };
    /// Mixolydian — dominant-flavoured major, avoids the leading tone.
    pub const MIXOLYDIAN: Scale = Scale { name: "mixolydian", steps: &[2, 2, 1, 2, 2, 1, 2] };

    /// Every scale the generator may choose from.
    pub const ALL: [Scale; 4] = [Scale::MAJOR, Scale::MINOR, Scale::DORIAN, Scale::MIXOLYDIAN];

    /// Number of degrees (usually 7).
    #[inline]
    pub fn degree_count(&self) -> usize {
        self.steps.len() + 1
    }

    /// Pitch classes of all degrees, given a tonic pitch class.
    pub fn tones(&self, tonic_pc: usize) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.degree_count());
        let mut cur = pc(tonic_pc);
        out.push(cur);
        for s in &self.steps[..self.steps.len() - 1] {
            cur = pc(cur + *s as usize);
            out.push(cur);
        }
        out
    }

    /// The `i`-th degree's pitch class, wrapping across octaves. Degree `n` is
    /// the octave of degree `0`.
    pub fn degree_pc(&self, tonic_pc: usize, i: usize) -> usize {
        let n = self.degree_count();
        let octave_wrap = i / n;
        let idx = i % n;
        if idx == 0 {
            return pc(tonic_pc + 12 * octave_wrap);
        }
        let semitones: usize = self.steps[..idx].iter().map(|s| *s as usize).sum();
        pc(tonic_pc + semitones + 12 * octave_wrap)
    }

    /// Build every diatonic seventh chord (degrees stacked in thirds) on this
    /// scale, with qualities inferred from actual scale distances.
    pub fn diatonic_seventh_chords(&self, tonic_pc: usize) -> Vec<DiatonicChord> {
        let n = self.degree_count();
        let mut out = Vec::with_capacity(n);
        for d in 0..n {
            let root = self.degree_pc(tonic_pc, d);
            let third = self.degree_pc(tonic_pc, d + 2);
            let fifth = self.degree_pc(tonic_pc, d + 4);
            let seventh = self.degree_pc(tonic_pc, d + 6);
            let iv = |x: usize| pc_distance(root, x) as u8;
            let (third_iv, fifth_iv, sev_iv) = (iv(third), iv(fifth), iv(seventh));
            let label = chord_label(third_iv, fifth_iv, sev_iv);
            out.push(DiatonicChord {
                degree: d,
                root_pc: root,
                intervals: vec![0, third_iv, fifth_iv, sev_iv],
                label,
            });
        }
        out
    }

    /// Display name of the key, e.g. `F# minor`.
    pub fn tonic_name(&self, tonic_pc: usize) -> String {
        format!("{} {}", NOTE_NAMES[pc(tonic_pc)], self.name)
    }
}

/// A diatonic chord derived from a scale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiatonicChord {
    /// Zero-based scale degree.
    pub degree: usize,
    pub root_pc: usize,
    /// Semitone intervals above the root: `[0, 3rd, 5th, 7th]`.
    pub intervals: Vec<u8>,
    /// Symbol suffix describing the quality (`m7`, `maj7`, ...).
    pub label: String,
}

/// Infer a chord-quality suffix from its stacked intervals.
fn chord_label(third: u8, fifth: u8, seventh: u8) -> String {
    let triad = match (third, fifth) {
        (4, 7) => "",
        (3, 7) => "m",
        (3, 6) => "dim",
        (4, 8) => "aug",
        _ => "sus",
    };
    let ext = match seventh {
        11 => "maj7",
        10 => "7",
        _ => "",
    };
    let s = format!("{triad}{ext}");
    if s.is_empty() {
        "maj7".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn major_scale_tones() {
        assert_eq!(Scale::MAJOR.tones(0), vec![0, 2, 4, 5, 7, 9, 11]);
        assert_eq!(Scale::MAJOR.tones(5), vec![5, 7, 9, 10, 0, 2, 4]);
    }

    #[test]
    fn degrees_wrap_across_octaves() {
        assert_eq!(Scale::MAJOR.degree_pc(0, 7), 0); // octave
        assert_eq!(Scale::MAJOR.degree_pc(0, 9), 4); // 2nd degree an octave up
    }

    #[test]
    fn diatonic_sevenths_of_c_major() {
        let chords = Scale::MAJOR.diatonic_seventh_chords(0);
        assert_eq!(chords.len(), 7);
        assert_eq!(chords[0].label, "maj7"); // Cmaj7
        assert_eq!(chords[1].label, "m7"); // Dm7
        assert_eq!(chords[4].label, "7"); // G7
        assert_eq!(chords[6].label, "dim7"); // B half-dim spelled dim-ish
        assert_eq!(chords[6].intervals, vec![0, 3, 6, 10]);
    }

    #[test]
    fn minor_relative_matches_a_eolian() {
        let chords = Scale::MINOR.diatonic_seventh_chords(9);
        assert_eq!(chords[0].root_pc, 9);
        assert_eq!(chords[0].intervals, vec![0, 3, 7, 10]); // Am7
        assert_eq!(chords[2].intervals, vec![0, 4, 7, 11]); // Cmaj7
    }

    #[test]
    fn dorian_has_raised_sixth() {
        assert_eq!(Scale::DORIAN.tones(2), vec![2, 4, 5, 7, 9, 11, 0]);
    }

    #[test]
    fn mixolydian_has_flattened_seventh() {
        assert_eq!(Scale::MIXOLYDIAN.tones(7), vec![7, 9, 11, 0, 2, 4, 5]);
    }
}
