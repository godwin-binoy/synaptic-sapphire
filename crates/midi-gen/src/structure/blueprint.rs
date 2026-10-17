//! The song blueprint: sections, per-bar roles and harmony.
//!
//! Built from a [`GenerateConfig`]: arrangement knobs decide which sections
//! exist and how long they are, the chord knobs pick the harmonic vocabulary,
//! and an *energy curve* ramps density across the form.

use crate::config::GenerateConfig;
use crate::rng::Rand;
use crate::structure::pick_key;
use crate::theory::chord::ChordSpec;
use crate::theory::progression::{plan_progression_with, HarmonicPlan, PlanOptions, Style};
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

    fn from_index(i: usize) -> Energy {
        match i {
            0 => Energy::Ambient,
            1 => Energy::Low,
            2 => Energy::Medium,
            _ => Energy::High,
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
    /// Energy after the arrangement's build curve is applied.
    pub energy: Energy,
}

impl Section {
    pub fn role_at(&self, i: u32) -> BarRole {
        BarRole {
            section: self.kind,
            energy: self.energy,
            bar_in_section: i,
            section_bars: self.bars,
        }
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
    /// Build a blueprint from a full configuration. Deterministic given
    /// `(seed, config)`; always ends with an outro when `outro_on`.
    pub fn generate(rng: &mut Rand, cfg: &GenerateConfig) -> SongBlueprint {
        let a = &cfg.arrangement;

        // --- Key: explicit override wins, otherwise seeded pick by bias. ----
        let (scale, tonic) = match (&cfg.scale, cfg.tonic_pc) {
            (Some(name), t) => match Scale::by_name(name) {
                Some(s) => (s, t.unwrap_or(pick_key(rng, cfg.key_bias).1)),
                None => pick_key(rng, cfg.key_bias),
            },
            (None, _) => pick_key(rng, cfg.key_bias),
        };

        // --- Tempo: fixed by config, nudged ±2 BPM by the seed so songs
        //     feel hand-played rather than metronomic. -----------------------
        let wobble = rng.range_f64(-2.0, 2.0);
        let tempo = (cfg.tempo_bpm + wobble).clamp(40.0, 140.0);

        // --- Section skeleton from the arrangement knobs. -------------------
        let mut kinds: Vec<(SectionKind, u32)> = Vec::new();
        if a.intro_on {
            kinds.push((SectionKind::Intro, a.section_bars_intro));
        }
        let verse_len = a.section_bars_verse.max(4);
        let chorus_len = a.section_bars_chorus.max(4);
        // Keep emitting verse/chorus pairs until the length hint is covered.
        let mut acc: u32 = kinds.iter().map(|(_, b)| *b).sum();
        let target = a.bars_hint.saturating_sub(if a.outro_on { a.section_bars_outro } else { 0 });
        let mut verses_done = 0u32;
        let mut choruses_done = 0u32;
        while acc < target || verses_done < a.verse_repeats || choruses_done < a.chorus_repeats {
            if verses_done < a.verse_repeats.max(1) && acc < target.max(a.bars_hint) {
                kinds.push((SectionKind::Verse, verse_len));
                acc += verse_len;
                verses_done += 1;
            }
            if choruses_done < a.chorus_repeats.max(1) && acc < target.max(a.bars_hint) {
                kinds.push((SectionKind::Chorus, chorus_len));
                acc += chorus_len;
                choruses_done += 1;
            }
            if a.bridge_on && verses_done >= a.verse_repeats.max(1) && choruses_done >= a.chorus_repeats.max(1) {
                break;
            }
            if acc >= target.max(a.bars_hint) && verses_done >= a.verse_repeats.max(1) && choruses_done >= a.chorus_repeats.max(1) {
                break;
            }
        }
        if a.bridge_on {
            kinds.push((SectionKind::Bridge, a.section_bars_bridge.max(2)));
        }
        // Final chorus lift before the outro (classic pop-lofi arc).
        kinds.push((SectionKind::Chorus, chorus_len));
        if a.outro_on {
            kinds.push((SectionKind::Outro, a.section_bars_outro.max(2)));
        }

        // --- Energy curve: later sections ramp up by `energy_curve`. --------
        let n = kinds.len().max(1) as f64;
        let style_base = Style::from_config(cfg.chords.progression_style.as_str());
        let mut sections: Vec<Section> = Vec::new();
        for (i, (kind, bars)) in kinds.into_iter().enumerate() {
            let progress = i as f64 / (n - 1.0).max(f64::EPSILON); // 0..1
            let lift = (a.energy_curve * progress * 1.99).floor() as usize; // 0 or 1 mostly
            let base_idx = match kind {
                SectionKind::Intro => 1,
                SectionKind::Verse => 2,
                SectionKind::Chorus => 3,
                SectionKind::Bridge => 1,
                SectionKind::Outro => 0,
            };
            let energy = Energy::from_index((base_idx.min(3)).max(base_idx.min(3)).min(3));
            let energy = if kind == SectionKind::Outro || kind == SectionKind::Intro {
                energy
            } else {
                Energy::from_index((base_idx + lift).clamp(1, 3))
            };

            let style = match style_base {
                Style::Mixed => {
                    const POOL: [Style; 4] = [Style::LofiJazz, Style::NeoSoul, Style::Ambient, Style::Bluesy];
                    *rng.pick(&POOL).expect("non-empty")
                }
                s => s,
            };
            let opts = PlanOptions {
                reharmonise_chance_pct: a.reharmonise_chance_pct,
                two_bar_hold_chance_pct: a.two_bar_hold_chance_pct,
                style,
            };
            let plan = plan_progression_with(rng, &scale, tonic, bars.max(4), &opts);
            sections.push(Section { kind, bars, plan, energy });
        }

        SongBlueprint {
            title_seed: cfg.seed,
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
    use crate::config::GenerateConfig;

    fn bp_with(f: impl FnOnce(&mut GenerateConfig)) -> SongBlueprint {
        let mut cfg = GenerateConfig::default();
        f(&mut cfg);
        cfg.clamp_all();
        SongBlueprint::generate(&mut Rand::new(cfg.seed), &cfg)
    }

    #[test]
    fn respects_length_hint_and_ends_with_outro() {
        for hint in [8u32, 16, 32, 48, 64] {
            let bp = bp_with(|c| c.arrangement.bars_hint = hint);
            assert!(bp.total_bars() >= hint, "hint={hint} got={}", bp.total_bars());
            assert_eq!(bp.sections.last().unwrap().kind, SectionKind::Outro);
        }
    }

    #[test]
    fn outro_can_be_disabled() {
        let bp = bp_with(|c| c.arrangement.outro_on = false);
        assert_ne!(bp.sections.last().unwrap().kind, SectionKind::Outro);
    }

    #[test]
    fn intro_can_be_disabled() {
        let bp = bp_with(|c| c.arrangement.intro_on = false);
        assert_ne!(bp.sections.first().unwrap().kind, SectionKind::Intro);
    }

    #[test]
    fn bridge_toggle_works() {
        let with = bp_with(|c| c.arrangement.bridge_on = true);
        let without = bp_with(|c| c.arrangement.bridge_on = false);
        assert!(with.sections.iter().any(|s| s.kind == SectionKind::Bridge));
        assert!(!without.sections.iter().any(|s| s.kind == SectionKind::Bridge));
    }

    #[test]
    fn repeats_are_honoured() {
        let bp = bp_with(|c| {
            c.arrangement.verse_repeats = 3;
            c.arrangement.bars_hint = 12;
        });
        let verses = bp.sections.iter().filter(|s| s.kind == SectionKind::Verse).count();
        assert!(verses >= 3, "asked for 3 verses, got {verses}");
    }

    #[test]
    fn key_override_wins() {
        let bp = bp_with(|c| {
            c.scale = Some("lydian".into());
            c.tonic_pc = Some(5);
        });
        assert_eq!(bp.scale.name, "lydian");
        assert_eq!(bp.tonic_pc, 5);
    }

    #[test]
    fn deterministic_for_same_config() {
        let a = bp_with(|c| c.seed = 777).describe();
        let b = bp_with(|c| c.seed = 777).describe();
        assert_eq!(a, b);
    }

    #[test]
    fn locate_round_trips() {
        let bp = bp_with(|c| c.arrangement.bars_hint = 32);
        for b in 0..bp.total_bars() {
            let (sec, local) = bp.locate(b).unwrap();
            assert!(local < sec.bars);
            assert!(bp.chord_at(b).is_some());
        }
        assert!(bp.locate(bp.total_bars()).is_none());
    }

    #[test]
    fn outro_is_ambient_energy() {
        let bp = bp_with(|_| {});
        let outro = bp.sections.iter().find(|s| s.kind == SectionKind::Outro).unwrap();
        assert_eq!(outro.energy, Energy::Ambient);
    }
}
