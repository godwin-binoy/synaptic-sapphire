//! Harmonic rhythm planning: which chord functions occur in which bars.
//!
//! The core idea is a **template library** of lofi-friendly progressions plus a
//! Markov-style transition table for "fill" bars, all expressed as *scale
//! degrees* so they can be rendered in any key. Everything random happens
//! through an injected [`Rand`], keeping the plan reproducible.

use crate::rng::Rand;
use crate::theory::chord::{ChordQuality, ChordSpec};
use crate::theory::scale::Scale;

/// One harmony slot in the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarmonicStep {
    /// Zero-based diatonic degree (`0` = I/i).
    pub degree: usize,
    pub quality: ChordQuality,
    /// Bars this step occupies (harmonic rhythm).
    pub bars: u32,
}

impl HarmonicStep {
    pub fn new(degree: usize, quality: ChordQuality, bars: u32) -> Self {
        HarmonicStep { degree, quality, bars }
    }

    /// Resolve to a concrete [`ChordSpec`] given the scale/tonic/octave.
    pub fn resolve(&self, scale: &Scale, tonic_pc: usize, root_octave: i32) -> ChordSpec {
        let root_pc = scale.degree_pc(tonic_pc, self.degree);
        ChordSpec::new(root_pc, self.quality.clone(), root_octave, self.bars)
    }
}

/// A complete harmonic plan for a song section or whole track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarmonicPlan {
    pub scale: Scale,
    pub tonic_pc: usize,
    /// Octave used for chord roots when resolving.
    pub root_octave: i32,
    pub steps: Vec<HarmonicStep>,
}

impl HarmonicPlan {
    /// Total bars covered by the plan.
    pub fn total_bars(&self) -> u32 {
        self.steps.iter().map(|s| s.bars).sum()
    }

    /// The resolved spec active at `bar` (0-based), or `None` past the end.
    pub fn chord_at_bar(&self, bar: u32) -> Option<ChordSpec> {
        let mut acc = 0u32;
        for step in &self.steps {
            if bar < acc + step.bars {
                return Some(step.resolve(&self.scale, self.tonic_pc, self.root_octave));
            }
            acc += step.bars;
        }
        None
    }

    /// All resolved chords, one entry per bar.
    pub fn chords(&self) -> Vec<ChordSpec> {
        let mut out = Vec::new();
        for step in &self.steps {
            let spec = step.resolve(&self.scale, self.tonic_pc, self.root_octave);
            // Multi-bar steps repeat the same harmony each bar.
            for _ in 0..step.bars {
                out.push(spec.clone());
            }
        }
        out
    }

    /// Human-readable summary like `A minor | Am9 -> Fmaj7 -> Dm7 -> Em7`.
    pub fn describe(&self) -> String {
        let labels: Vec<String> = self
            .steps
            .iter()
            .map(|s| {
                let c = s.resolve(&self.scale, self.tonic_pc, self.root_octave);
                format!("{} ({})", c.label(), roman_degree(s.degree))
            })
            .collect();
        format!("{} | {}", self.scale.tonic_name(self.tonic_pc), labels.join(" -> "))
    }
}

/// Roman-numeral-ish label for debug output. Degrees are zero-based.
fn roman_degree(d: usize) -> String {
    const NUMERALS: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
    NUMERALS.get(d % 7).copied().unwrap_or("?").to_string()
}

/// Harmonic vocabulary a plan may draw from. Mirrors the UI-facing
/// `ProgressionStyle` but lives in the theory layer so this module stays free
/// of config dependencies (the pipeline translates between them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Curated lofi/jazz loops — the house sound.
    LofiJazz,
    /// Neo-soul: dense extensions, slow motion, minor 4/5 cycles.
    NeoSoul,
    /// Ambient: long pedal-point drones, minimal movement.
    Ambient,
    /// Bluesy turnarounds and quick changes.
    Bluesy,
    /// Pick per-plan by seed from all of the above.
    Mixed,
}

/// Curated template progressions grouped by style, as `(degree, quality-suffix)`
/// sequences. Degrees are zero-based scale steps; qualities are looked up with
/// [`quality_from_suffix`]. These encode classic moves: ii-V-I variants, IV-vi
/// loops, minor cycles around i-VII-VI, soul 4/5 vamp chains, blues runarounds.
fn templates_for(style: Style) -> Vec<Vec<(usize, &'static str)>> {
    let lofi = vec![
        // Major-flavoured loops
        vec![(1, "m7"), (5, "m7"), (0, "maj9"), (3, "maj7")], // ii vi I maj7 IV
        vec![(3, "maj7"), (5, "m7"), (1, "m7"), (4, "dom7")], // IV vi ii V
        vec![(0, "maj7"), (3, "maj7"), (1, "m7"), (4, "dom7")], // I IV ii V
        vec![(5, "m7"), (1, "m7"), (4, "maj7"), (0, "maj9")], // vi ii IV I
        // Minor-flavoured loops
        vec![(0, "m9"), (5, "maj7"), (3, "m7"), (4, "dom7")], // i VI iv V
        vec![(0, "m7"), (6, "maj7"), (5, "maj7"), (4, "m7")], // i VII VI v
        vec![(1, "m7b5"), (4, "dom7"), (0, "m9"), (0, "m7")], // ii° V i i
        vec![(3, "m7"), (6, "maj7"), (0, "m9"), (4, "sus4")], // iv VII i vsus
    ];
    match style {
        Style::LofiJazz => lofi,
        Style::NeoSoul => vec![
            vec![(1, "m9"), (4, "dom7"), (0, "maj9"), (0, "maj9")],       // ii9 V9 I9 I9
            vec![(3, "maj9"), (4, "m9"), (0, "maj7"), (5, "m9")],         // IVmaj9 v m7-style soul loop
            vec![(0, "m9"), (1, "m7"), (5, "maj7"), (3, "m9")],           // i9 ii7 VI iv9
            vec![(5, "m9"), (1, "m9"), (4, "dom7"), (0, "maj9")],         // vi9 ii9 V9 Imaj9
            vec![(2, "m7"), (3, "maj7"), (5, "m9"), (0, "add9")],         // iii IV vi add9
            vec![(0, "maj9"), (5, "m9"), (3, "maj7"), (4, "dom7")],       // I vi IV V (doo-wop, soulified)
        ],
        Style::Ambient => vec![
            vec![(0, "m9"), (0, "m9"), (5, "maj7"), (5, "maj7")],         // tonic drone drifting to VI
            vec![(0, "maj9"), (0, "maj9"), (3, "maj7"), (6, "maj7")],     // I pedal, IV then VII shimmer
            vec![(5, "m9"), (5, "m9"), (2, "m7"), (0, "maj9")],           // vi drift into relative major
            vec![(0, "sus4"), (0, "sus4"), (3, "maj7"), (3, "maj7")],     // sus pedal breathing
        ],
        Style::Bluesy => vec![
            vec![(0, "dom7"), (3, "dom7"), (0, "dom7"), (0, "dom7")],     // quick-to-IV blues bar one
            vec![(0, "dom7"), (5, "m7"), (1, "m7"), (4, "dom7")],         // I vi ii V (jazz blues)
            vec![(4, "dom7"), (3, "dom7"), (0, "dom7"), (5, "dom7")],     // V IV I VI runaround
            vec![(0, "m9"), (0, "m9"), (4, "dom7"), (3, "m7")],           // minor blues vamp
        ],
        Style::Mixed => {
            let mut all = lofi;
            all.extend(templates_for(Style::NeoSoul));
            all.extend(templates_for(Style::Ambient));
            all.extend(templates_for(Style::Bluesy));
            all
        }
    }
}

/// Weighted targets/weights for moving away from each scale degree.
const NEIGHBOURS: [(usize, &[usize], &[u32]); 7] = [
    (0, &[5, 3, 4], &[3, 2, 1]),
    (1, &[0, 4, 5], &[2, 3, 1]),
    (2, &[1, 6, 3], &[3, 1, 2]),
    (3, &[1, 4, 0], &[3, 2, 1]),
    (4, &[5, 1, 0], &[2, 3, 2]),
    (5, &[3, 6, 1], &[3, 1, 2]),
    (6, &[4, 1, 5], &[3, 2, 1]),
];

/// Tunables for the progression planner; defaults reproduce the original
/// behaviour exactly (swap weight 1, hold chance 1-in-4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanOptions {
    /// Chance numerator (out of 100) that an interior bar gets reharmonised
    /// via the degree walk instead of copying the template.
    pub reharmonise_chance_pct: u32,
    /// Chance numerator (out of 100) that adjacent chords merge into 2-bar holds.
    pub two_bar_hold_chance_pct: u32,
    /// Which harmonic vocabulary to draw templates from.
    pub style: Style,
}

impl Default for PlanOptions {
    fn default() -> Self {
        PlanOptions { reharmonise_chance_pct: 100, two_bar_hold_chance_pct: 25, style: Style::LofiJazz }
    }
}

/// Build a progression covering exactly `bars` bars using default options.
pub fn plan_progression(rng: &mut Rand, scale: &Scale, tonic_pc: usize, bars: u32) -> HarmonicPlan {
    plan_progression_with(rng, scale, tonic_pc, bars, &PlanOptions::default())
}

/// Build a progression covering exactly `bars` bars.
///
/// Algorithm: pick a template loop from the style's pool, repeat it (with
/// probabilistic re-rolls of interior bars via a Markov degree walk) until the
/// length is reached, then randomly hold some chords for two bars to relax the
/// harmonic rhythm. Panics if `bars < 4`.
pub fn plan_progression_with(
    rng: &mut Rand,
    scale: &Scale,
    tonic_pc: usize,
    bars: u32,
    opts: &PlanOptions,
) -> HarmonicPlan {
    assert!(bars >= 4, "a harmonic plan needs at least 4 bars");

    let pool = templates_for(opts.style);
    let chosen = rng.pick_index(&pool).expect("non-empty templates");
    let template: Vec<(usize, ChordQuality)> = pool[chosen]
        .iter()
        .map(|(d, q)| (*d, quality_from_suffix(q)))
        .collect();

    // Repeat the loop until we have enough steps.
    let mut steps: Vec<HarmonicStep> = Vec::new();
    while (steps.len() as u32) < bars {
        for (d, q) in &template {
            steps.push(HarmonicStep::new(*d, q.clone(), 1));
            if steps.len() as u32 >= bars {
                break;
            }
        }
    }

    // Cadence variation: replace some interior chords via the degree walk so
    // repeated loops do not feel like exact copies.
    let swap_budget = (bars / 8).max(1) as usize;
    for _ in 0..swap_budget {
        if !rng.chance(opts.reharmonise_chance_pct, 100) {
            continue;
        }
        let idx = rng.range_i64(1, bars as i64 - 1) as usize;
        if idx >= steps.len() {
            continue;
        }
        let last = steps[idx - 1].degree % 7;
        let (_, targets, weights) = NEIGHBOURS[last];
        let next = *rng.weighted(targets, weights).expect("non-empty neighbour table");
        let quality = pick_quality(rng, scale, tonic_pc, next);
        steps[idx] = HarmonicStep::new(next, quality, 1);
    }

    // Relax the harmonic rhythm: merge adjacent pairs into 2-bar holds.
    let mut idx = 0usize;
    while idx + 1 < steps.len() {
        if rng.chance(opts.two_bar_hold_chance_pct, 100) {
            steps[idx].bars = 2;
            steps.remove(idx + 1);
            idx += 1; // skip ahead, avoid cascading merges
        } else {
            idx += 1;
        }
    }

    // Exact fix-up on total bars.
    let mut total: u32 = steps.iter().map(|s| s.bars).sum();
    while total > bars {
        let last = steps.len() - 1;
        if steps[last].bars > 1 {
            steps[last].bars -= 1;
        } else {
            steps.remove(last);
        }
        total -= 1;
    }
    while total < bars {
        let last = steps.len() - 1;
        steps[last].bars += 1;
        total += 1;
    }

    HarmonicPlan { scale: scale.clone(), tonic_pc, root_octave: 3, steps }
}

/// Choose a plausible quality for a degree within a scale by inspecting the
/// actual stacked thirds. Keeps harmony diatonic.
fn pick_quality(rng: &mut Rand, scale: &Scale, tonic_pc: usize, degree: usize) -> ChordQuality {
    let chords = scale.diatonic_seventh_chords(tonic_pc);
    let dc = &chords[degree % chords.len()];
    // Occasionally upgrade plain sevenths to ninths for extra colour.
    if rng.chance(1, 3) {
        match dc.label.as_str() {
            "m7" => return ChordQuality::minor_9(),
            "maj7" => return ChordQuality::major_9(),
            _ => {}
        }
    }
    quality_from_suffix(&dc.label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_fills_exact_bar_count() {
        for bars in [4u32, 5, 8, 12, 16, 24, 32] {
            for seed in 0..25u64 {
                let mut rng = Rand::new(seed * 31 + bars as u64);
                let plan = plan_progression(&mut rng, &Scale::MINOR, 9, bars);
                assert_eq!(plan.total_bars(), bars, "bars={bars} seed={seed} steps={:?}", plan.steps);
            }
        }
    }

    #[test]
    fn chord_at_bar_is_consistent_with_expansion() {
        let mut rng = Rand::new(11);
        let plan = plan_progression(&mut rng, &Scale::MAJOR, 0, 8);
        let expanded = plan.chords();
        assert_eq!(expanded.len(), 8);
        for b in 0..8 {
            assert_eq!(plan.chord_at_bar(b).unwrap().label(), expanded[b].label());
        }
        assert!(plan.chord_at_bar(8).is_none());
    }

    #[test]
    fn deterministic_for_same_seed() {
        let a = plan_progression(&mut Rand::new(123), &Scale::DORIAN, 2, 16).describe();
        let b = plan_progression(&mut Rand::new(123), &Scale::DORIAN, 2, 16).describe();
        assert_eq!(a, b);
    }

    #[test]
    fn degrees_stay_diatonic() {
        let mut rng = Rand::new(77);
        let plan = plan_progression(&mut rng, &Scale::MINOR, 9, 16);
        let tones: Vec<usize> = plan.scale.tones(plan.tonic_pc);
        for step in &plan.steps {
            let root = plan.scale.degree_pc(plan.tonic_pc, step.degree);
            assert!(tones.contains(&root), "borrowed chord {:?}", step);
        }
    }

    #[test]
    fn describe_contains_key_and_arrows() {
        let plan = plan_progression(&mut Rand::new(3), &Scale::MINOR, 9, 8);
        let d = plan.describe();
        assert!(d.starts_with("A minor |"));
        assert!(d.contains("->"));
    }
}
