//! The complete, serialisable parameter surface of the generator.
//!
//! [`GenerateConfig`] is *the* contract between the UI and the engine:
//! - it derives `Serialize`/`Deserialize`, so the web front-end can round-trip
//!   a configuration as JSON;
//! - [`crate::config::ConfigSchema::derive`] describes every field (range, step,
//!   default, enum options, grouping) so the UI can **auto-build controls** for
//!   literally everything without hardcoding anything;
//! - every numeric field is clamped on ingest (`clamp_all`) so hostile or
//!   buggy input can never panic the generators.
//!
//! Adding a new knob later means: add the field here, thread it through the
//! relevant voice, extend `clamp_all` + `schema()`. Nothing else changes.

mod generate_config;
mod schema;

pub use generate_config::{
    ArrangementConfig, BassConfig, ChordsConfig, DrumsConfig, HumanizeConfig, KeyBias,
    MelodyConfig, MoodPreset, ProgressionStyle,
};
pub use schema::{ConfigSchema, SchemaField, SchemaKind, SchemaOption};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced while validating or normalising a configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("field `{field}`: {reason}")]
    Invalid { field: String, reason: String },
}

/// Everything needed to reproduce one composition, bit-for-bit.
///
/// `seed` selects the randomness; the sub-configs bias and constrain the
/// algorithms. Defaults are tuned for the classic late-night lofi aesthetic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerateConfig {
    /// Master seed. Every random choice derives from this.
    pub seed: u64,
    /// Optional musical key override (`None` = pick from `key_bias`).
    pub scale: Option<String>,
    /// Tonic pitch class 0..11 when `scale` is set (`None` = seeded pick).
    pub tonic_pc: Option<usize>,
    /// Which keys the seeded picker favours.
    pub key_bias: KeyBias,
    /// Tempo in BPM (clamped to `[40, 140]`).
    pub tempo_bpm: f64,
    pub arrangement: ArrangementConfig,
    pub chords: ChordsConfig,
    pub bass: BassConfig,
    pub melody: MelodyConfig,
    pub drums: DrumsConfig,
    /// Humanisation shared by all voices.
    pub humanize: HumanizeConfig,
}

impl Default for GenerateConfig {
    fn default() -> Self {
        GenerateConfig {
            seed: 0x5EED_1E05,
            scale: None,
            tonic_pc: None,
            key_bias: KeyBias::default(),
            tempo_bpm: 72.0,
            arrangement: ArrangementConfig::default(),
            chords: ChordsConfig::default(),
            bass: BassConfig::default(),
            melody: MelodyConfig::default(),
            drums: DrumsConfig::default(),
            humanize: HumanizeConfig::default(),
        }
    }
}

impl GenerateConfig {
    /// Clamp every field into its legal operating range. Called on all ingest
    /// paths (HTTP body, CLI file) so downstream code may assume sane values.
    pub fn clamp_all(&mut self) {
        let c = |v: f64, lo: f64, hi: f64| v.clamp(lo, hi);
        self.tempo_bpm = c(self.tempo_bpm, 40.0, 140.0);
        if let Some(t) = self.tonic_pc {
            if t >= 12 {
                self.tonic_pc = Some(t % 12);
            }
        }

        let a = &mut self.arrangement;
        a.bars_hint = a.bars_hint.clamp(8, 256);
        a.section_bars_intro = a.section_bars_intro.clamp(2, 32);
        a.section_bars_verse = a.section_bars_verse.clamp(4, 64);
        a.section_bars_chorus = a.section_bars_chorus.clamp(4, 64);
        a.section_bars_bridge = a.section_bars_bridge.clamp(2, 32);
        a.section_bars_outro = a.section_bars_outro.clamp(2, 32);
        a.verse_repeats = a.verse_repeats.clamp(0, 8);
        a.chorus_repeats = a.chorus_repeats.clamp(0, 8);
        a.energy_curve = c(a.energy_curve, 0.0, 1.0);
        a.reharmonise_chance_pct = a.reharmonise_chance_pct.clamp(0, 100);
        a.two_bar_hold_chance_pct = a.two_bar_hold_chance_pct.clamp(0, 100);

        let ch = &mut self.chords;
        ch.voicing_octave = ch.voicing_octave.clamp(2, 6);
        ch.pad_sustain_beats = c(ch.pad_sustain_beats, 0.5, 16.0);
        ch.stab_downbeat_pct = ch.stab_downbeat_pct.clamp(0, 100);
        ch.stab_offbeat_pct = ch.stab_offbeat_pct.clamp(0, 100);
        ch.offbeat_syncopation_pct = ch.offbeat_syncopation_pct.clamp(0, 100);
        ch.tension_chance_pct = ch.tension_chance_pct.clamp(0, 100);
        ch.max_voices = ch.max_voices.clamp(3, 8);
        ch.program = ch.program.min(127);
        ch.pad_program = ch.pad_program.min(127);

        let b = &mut self.bass;
        b.register_low = b.register_low.clamp(24, 72);
        b.register_high = b.register_high.clamp(36, 84);
        if b.register_high < b.register_low + 12 {
            b.register_high = (b.register_low + 12).min(84);
        }
        b.walk_chance_pct = b.walk_chance_pct.clamp(0, 100);
        b.pickup_chance_pct = b.pickup_chance_pct.clamp(0, 100);
        b.approach_chance_pct = b.approach_chance_pct.clamp(0, 100);
        b.octave_jump_chance_pct = b.octave_jump_chance_pct.clamp(0, 100);
        b.anchor_velocity = b.anchor_velocity.clamp(30, 127);
        b.program = b.program.min(127);

        let m = &mut self.melody;
        m.motif_cells = m.motif_cells.clamp(2, 12);
        m.note_count_min = m.note_count_min.clamp(1, 8);
        m.note_count_max = m.note_count_max.clamp(2, 12);
        if m.note_count_max < m.note_count_min {
            m.note_count_max = m.note_count_min;
        }
        m.density_scale = c(m.density_scale, 0.0, 2.0);
        m.register_low = m.register_low.clamp(48, 96);
        m.register_high = m.register_high.clamp(60, 108);
        if m.register_high < m.register_low + 12 {
            m.register_high = (m.register_low + 12).min(108);
        }
        m.chromatic_passing_pct = m.chromatic_passing_pct.clamp(0, 100);
        m.rest_between_phrases_pct = m.rest_between_phrases_pct.clamp(0, 100);
        m.phrase_len_bars = m.phrase_len_bars.clamp(1, 8);
        m.answer_inversion_pct = m.answer_inversion_pct.clamp(0, 100);
        m.answer_retrograde_pct = m.answer_retrograde_pct.clamp(0, 100);
        m.program = m.program.min(127);

        let d = &mut self.drums;
        d.swing_strength = c(d.swing_strength, 0.0, 1.0);
        d.kick_probability_pct = d.kick_probability_pct.clamp(0, 100);
        d.snare_probability_pct = d.snare_probability_pct.clamp(0, 100);
        d.snare_probability_pct = d.snare_probability_pct.clamp(0, 100);
        d.hat_probability_pct = d.hat_probability_pct.clamp(0, 100);
        d.ghost_note_pct = d.ghost_note_pct.clamp(0, 100);
        d.open_hat_pct = d.open_hat_pct.clamp(0, 100);
        d.shaker_pct = d.shaker_pct.clamp(0, 100);
        d.fill_on_cadence_pct = d.fill_on_cadence_pct.clamp(0, 100);
        d.crash_on_section_start_pct = d.crash_on_section_start_pct.clamp(0, 100);
        d.velocity_base = d.velocity_base.clamp(20, 127);
        d.drag_ticks = d.drag_ticks.clamp(0, 120);

        let h = &mut self.humanize;
        h.swing = c(h.swing, 0.0, 1.0);
        h.timing_jitter_ticks = h.timing_jitter_ticks.clamp(0, 120);
        h.velocity_jitter = h.velocity_jitter.clamp(0, 40);
        h.dynamics_range = c(h.dynamics_range, 0.0, 1.0);
    }

    /// Effective swing used by every voice: the global humanise swing scaled
    /// towards the drum groove strength so feel stays coherent across tracks.
    pub fn effective_swing(&self) -> f64 {
        (self.humanize.swing * (0.5 + self.drums.swing_strength / 2.0)).clamp(0.0, 1.0)
    }

    /// Validate semantics that clamping cannot fix (unknown enum strings...).
    pub fn validate(&self) -> Result<(), ConfigError> {
        const SCALES: [&str; 6] =
        ["major", "minor", "dorian", "mixolydian", "lydian", "harmonic minor"];
        if let Some(s) = &self.scale {
            if !SCALES.contains(&s.as_str()) {
                return Err(ConfigError::Invalid {
                    field: "scale".into(),
                    reason: format!("unknown scale `{s}`, expected one of {SCALES:?}"),
                });
            }
        }
        Ok(())
    }

    /// A fully-described, machine-readable schema of every knob. The web UI
    /// renders its entire form from this — nothing is hardcoded client-side.
    pub fn schema() -> ConfigSchema {
        ConfigSchema::derive()
    }

    /// Apply a named mood preset on top of the current values (keeps the seed).
    pub fn apply_preset(&mut self, p: MoodPreset) {
        p.apply(self);
        self.clamp_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip_preserves_values() {
        let mut cfg = GenerateConfig::default();
        cfg.seed = 123;
        cfg.tempo_bpm = 68.5;
        cfg.melody.density_scale = 1.4;
        cfg.drums.ghost_note_pct = 42;
        let json = serde_json::to_string(&cfg).unwrap();
        let back: GenerateConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn partial_json_deserialises_with_defaults() {
        let cfg: GenerateConfig = serde_json::from_str(r#"{"seed": 7}"#).unwrap();
        assert_eq!(cfg.seed, 7);
        assert_eq!(cfg.tempo_bpm, 72.0);
    }

    #[test]
    fn clamping_bounds_hostile_input() {
        let mut cfg = GenerateConfig::default();
        cfg.tempo_bpm = 1e9;
        cfg.arrangement.bars_hint = 0;
        cfg.melody.register_low = 200;
        cfg.drums.velocity_base = 999;
        cfg.humanize.swing = -5.0;
        cfg.clamp_all();
        assert!((40.0..=140.0).contains(&cfg.tempo_bpm));
        assert!(cfg.arrangement.bars_hint >= 8);
        assert!(cfg.melody.register_low <= cfg.melody.register_high);
        assert!(cfg.drums.velocity_base <= 127);
        assert!(cfg.humanize.swing >= 0.0);
    }

    #[test]
    fn rejects_unknown_scale_name() {
        let mut cfg = GenerateConfig::default();
        cfg.scale = Some("lydian".into());
        assert!(cfg.validate().is_err());
        cfg.scale = Some("dorian".into());
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn schema_covers_every_leaf_field() {
        let schema = GenerateConfig::schema();
        let names: Vec<&str> = schema.all_fields().iter().map(|f| f.path.as_str()).collect();
        for expect in [
            "seed",
            "tempo_bpm",
            "arrangement.bars_hint",
            "chords.voicing_octave",
            "bass.walk_chance_pct",
            "melody.density_scale",
            "drums.ghost_note_pct",
            "humanize.swing",
            "key_bias",
            "chords.progression_style",
        ] {
            assert!(names.contains(&expect), "schema missing `{expect}`");
        }
        assert!(schema.all_fields().len() >= 40, "schema feels too small");
    }

    #[test]
    fn presets_produce_valid_configs() {
        for p in [
            MoodPreset::RainyStudy,
            MoodPreset::LateNightDrive,
            MoodPreset::SundayMorning,
            MoodPreset::DreamDrift,
        ] {
            let mut cfg = GenerateConfig::default();
            cfg.apply_preset(p);
            assert!(cfg.validate().is_ok());
        }
    }
}
