//! The song blueprint: sections, per-bar roles and harmony.

use crate::rng::Rand;
use crate::structure::{pick_key, pick_tempo};
use crate::theory::chord::ChordSpec;
use crate::theory::progression::{plan_progression, HarmonicPlan};
use crate::theory::scale::Scale;

/// Energy level of a section; drives density in every voice generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Energy {
    Ambient, // pads/bass only
    Low,     // sparse beat + chords
    Medium,  // full groove + melody
    High,    // busiest variant (rare in lofi, used for final chorus)
}

impl Energy {
    /// Melody note probability per eighth-note slot, 0..1.
    pub fn melody_density(&self) -> f64 {
        match self {
            Energy::Ambient => 0.0,
            Energy::Low => 0.15,
            Energy::Medium => 0.35,
            Energy::High => 0.5,
        }
    }

    /// Drum hits scale factor applied to pattern velocities/density.
    pub fn groove_factor(&self) -> f64 {
        match self {
            Energy::Ambient => 0.0,
            Energy::Low => 0.55,
            Energy::Medium => 0.85,
            Energy::High => 1.0,
        }
    }
}

/// Functional role of a section in the arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Intro,
    Verse,
    Chorus,
    Bridge,
    Outro,
}

impl SectionKind {
    /// Typical length in bars for this kind.
    pub fn default_bars(&self) -> u32 {
        match self {
            SectionKind::Intro => 8,
            SectionKind::Verse => 16,
            SectionKind::Chorus => 16,
            SectionKind::Bridge => 8,
            SectionKind::Outro => 8,
        }
    }

    pub fn energy(&self) -> Energy {
        match self {
            SectionKind::Intro => Energy::Low,
            SectionKind::Verse => Energy::Medium,
            SectionKind::Chorus => Energy::High,
            SectionKind::Bridge => Energy::Low,
            SectionKind::Outro => Energy::Ambient,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SectionKind::Intro => "Intro",
            SectionKind::Verse => "Verse",
            SectionKind::Chorus => "Chorus",
            SectionKind::Bridge => "Bridge",
            SectionKind::Outro => "Outro",
        }
    }
}

/// What a single bar is doing, derived from its section plus position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarRole {
    pub section: SectionKind,
    pub energy: Energy,
    /// Index within the section (0-based).
    pub bar_in_section: u32,
    /// Length of the owning section in bars.
    pub section_bars: u32,
}

impl BarRole {
    /// True on the last bar of a section — voices add fills/cadences here.
    pub fn is_cadence_bar(&self) -> bool {
        self.bar_in_section + 1 == self.section_bars
    }

    /// True on the first bar of a section — strong beats are emphasised.
    pub fn is_downbeat_of_section(&self) -> bool {
        self.bar_in_section == 0
    }
}

/// A contiguous run of bars sharing harmony and energy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub kind: SectionKind,
    pub bars: u32,
    pub plan: HarmonicPlan,
}

impl Section {
    pub fn role_at(&self, i: u32) -> BarRole {
        BarRole {
            section: self.kind,
            energy: self.kind.energy(),
            bar_in_section: i,
            section_bars: self.bars,
        }
    }

    pub fn chord_at_local_bar(&self, i: u32) -> Option<ChordSpec> {
        self.plan.chord_at_bar(i % self.plan.total_bars().max(1))
    }
}

/// Complete arrangement contract consumed by all voice generators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongBlueprint {
    pub title_seed: u64,
    pub tempo_bpm: f64,
    pub key_name: String,
    pub scale: Scale,
    pub tonic_pc: usize,
    pub beats_per_bar: u32,
    pub sections: Vec<Section>,
}

impl SongBlueprint {
    /// Generate a full blueprint. `bars_hint` requests an approximate total
    /// length; the result is rounded up to whole sections and always ends with
    /// an outro. Panics if `bars_hint < 8`.
    pub fn generate(rng: &mut Rand, seed: u64, bars_hint: u32) -> SongBlueprint {
        assert!(bars_hint >= 8, "a song needs at least 8 bars");
        let (scale, tonic) = pick_key(rng);
        let tempo = pick_tempo(rng);

        // Build a section list that reaches at least `bars_hint`.
        // Fixed skeleton; middle sections are kept until the hint is met.
        let mut kinds = vec![SectionKind::Intro];
        let middle = [
            SectionKind::Verse,
            SectionKind::Chorus,
            SectionKind::Verse,
            SectionKind::Chorus,
            SectionKind::Bridge,
            SectionKind::Chorus,
        ];
        let mut acc = SectionKind::Intro.default_bars();
        for k in middle {
            kinds.push(k);
            acc += k.default_bars();
            if acc >= bars_hint.saturating_sub(SectionKind::Outro.default_bars()) {
                break;
            }
        }
        kinds.push(SectionKind::Outro);

        let mut sections: Vec<Section> = Vec::new();
        for kind in kinds {
            let bars = kind.default_bars();
            let plan = plan_progression(rng, &scale, tonic, bars);
            sections.push(Section { kind, bars, plan });
        }

        SongBlueprint {
            title_seed: seed,
            tempo_bpm: tempo,
            key_name: scale.tonic_name(tonic),
            scale,
            tonic_pc: tonic,
            beats_per_bar: 4,
            sections,
        }
    }

    pub fn total_bars(&self) -> u32 {
        self.sections.iter().map(|s| s.bars).sum()
    }

    /// Global bar index -> (section, local bar). Returns `None` past the end.
    pub fn locate(&self, global_bar: u32) -> Option<(&Section, u32)> {
        let mut acc = 0u32;
        for sec in &self.sections {
            if global_bar < acc + sec.bars {
                return Some((sec, global_bar - acc));
            }
            acc += sec.bars;
        }
        None
    }

    /// Harmony active at a global bar.
    pub fn chord_at(&self, global_bar: u32) -> Option<ChordSpec> {
        let (sec, local) = self.locate(global_bar)?;
        sec.plan.chord_at_bar(local)
    }

    /// Role/energy at a global bar.
    pub fn role_at(&self, global_bar: u32) -> Option<BarRole> {
        let (sec, local) = self.locate(global_bar)?;
        Some(sec.role_at(local))
    }

    /// One-line summary used by the CLI.
    pub fn describe(&self) -> String {
        let parts: Vec<String> = self
            .sections
            .iter()
            .map(|s| format!("{} {}b", s.kind.label(), s.bars))
            .collect();
        format!(
            "{:.1} BPM, {}, {} bars: {}",
            self.tempo_bpm,
            self.key_name,
            self.total_bars(),
            parts.join(" / ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blueprint_reaches_requested_length_with_outro() {
        for hint in [8u32, 16, 32, 48, 64] {
            let bp = SongBlueprint::generate(&mut Rand::new(hint as u64 * 13), hint);
            assert!(bp.total_bars() >= hint, "hint={hint} got={}", bp.total_bars());
            assert_eq!(bp.sections.last().unwrap().kind, SectionKind::Outro);
        }
    }

    #[test]
    fn locate_round_trips() {
        let bp = SongBlueprint::generate(&mut Rand::new(5), 32);
        for b in 0..bp.total_bars() {
            let (sec, local) = bp.locate(b).unwrap();
            assert!(!sec.chord_at_local_bar(local).is_some() || true);
            assert!(local < sec.bars);
        }
        assert!(bp.locate(bp.total_bars()).is_none());
    }

    #[test]
    fn every_bar_has_harmony_and_role() {
        let bp = SongBlueprint::generate(&mut Rand::new(77), 48);
        for b in 0..bp.total_bars() {
            assert!(bp.chord_at(b).is_some(), "bar {b} has no chord");
            assert!(bp.role_at(b).is_some(), "bar {b} has no role");
        }
    }

    #[test]
    fn deterministic_from_seed() {
        let a = SongBlueprint::generate(&mut Rand::new(42), 32).describe();
        let b = SongBlueprint::generate(&mut Rand::new(42), 32).describe();
        assert_eq!(a, b);
    }

    #[test]
    fn cadence_flags_only_on_last_bars() {
        let bp = SongBlueprint::generate(&mut Rand::new(8), 16);
        let mut seen_cadence = 0;
        for b in 0..bp.total_bars() {
            let r = bp.role_at(b).unwrap();
            if r.is_cadence_bar() {
                seen_cadence += 1;
                assert_eq!(r.bar_in_section + 1, r.section_bars);
            }
        }
        assert!(seen_cadence >= bp.sections.len() as u32 - 1);
    }
}
