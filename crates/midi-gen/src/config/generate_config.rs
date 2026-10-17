//! Per-layer configuration structs. Every field here is a real knob that the
//! generators consume — there are no decorative settings.

use serde::{Deserialize, Serialize};

/// Song form: how long, which sections, how harmony evolves across them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArrangementConfig {
    /// Approximate requested song length in bars (rounded up to whole sections).
    pub bars_hint: u32,
    pub section_bars_intro: u32,
    pub section_bars_verse: u32,
    pub section_bars_chorus: u32,
    pub section_bars_bridge: u32,
    pub section_bars_outro: u32,
    /// Emit an intro section at all.
    pub intro_on: bool,
    /// How many verse repetitions before the final chorus/outro.
    pub verse_repeats: u32,
    /// How many chorus repetitions.
    pub chorus_repeats: u32,
    /// Include a bridge.
    pub bridge_on: bool,
    /// End with a drumless ambient outro.
    pub outro_on: bool,
    /// 0..1 — how strongly later sections ramp energy up. `0` = flat dynamics,
    /// `1` = dramatic build. Scales every voice's density curve.
    pub energy_curve: f64,
    /// Chance (%) that repeated loops reharmonise one bar (avoids copy-paste feel).
    pub reharmonise_chance_pct: u32,
    /// Chance (%) a chord is held two bars (relaxes harmonic rhythm).
    pub two_bar_hold_chance_pct: u32,
}

impl Default for ArrangementConfig {
    fn default() -> Self {
        ArrangementConfig {
            bars_hint: 48,
            section_bars_intro: 8,
            section_bars_verse: 16,
            section_bars_chorus: 16,
            section_bars_bridge: 8,
            section_bars_outro: 8,
            intro_on: true,
            verse_repeats: 2,
            chorus_repeats: 2,
            bridge_on: true,
            outro_on: true,
            energy_curve: 0.5,
            reharmonise_chance_pct: 35,
            two_bar_hold_chance_pct: 25,
        }
    }
}

/// Harmonic flavour selector for the progression planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressionStyle {
    /// Curated lofi/jazz loops (ii-V-I moves, IV-vi cycles). The house sound.
    LofiJazz,
    /// Neo-soul: richer 9ths/11ths, slow harmonic rhythm, secondary IVs.
    NeoSoul,
    /// Ambient/cinematic: pedal points, slow i - VI - III - VII drones.
    Ambient,
    /// Classic 12-bar-ish blues turnaround shuffled into the template pool.
    Bluesy,
    /// Any of the above, chosen per-section by the seed.
    Mixed,
}

impl Default for ProgressionStyle {
    fn default() -> Self {
        ProgressionStyle::LofiJazz
    }
}

impl ProgressionStyle {
    /// Stable string key used by the JSON config & schema enums.
    pub fn as_key(self) -> &'static str {
        match self {
            ProgressionStyle::LofiJazz => "lofi_jazz",
            ProgressionStyle::NeoSoul => "neo_soul",
            ProgressionStyle::Ambient => "ambient",
            ProgressionStyle::Bluesy => "bluesy",
            ProgressionStyle::Mixed => "mixed",
        }
    }
}

/// Chord comping (Rhodes/pads) parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChordsConfig {
    pub enabled: bool,
    /// Turn sustained pads into a separate layer in ambient sections.
    pub pad_enabled: bool,
    /// Which progression vocabulary to draw from.
    pub progression_style: ProgressionStyle,
    /// Octave of chord roots when voicing (2..6).
    pub voicing_octave: i32,
    /// Allow 9th/11th/13th colour tones, as a percentage chance per chord.
    pub tension_chance_pct: u32,
    /// Maximum simultaneous voices kept in a voicing (3..8).
    pub max_voices: u32,
    /// Probability (%) of attacking the stab on the downbeat in grooving bars.
    pub stab_downbeat_pct: u32,
    /// Probability (%) of the syncopated off-beat stab.
    pub stab_offbeat_pct: u32,
    /// Percentage chance the off-beat lands on the "and of 3" instead of beat 3.
    pub offbeat_syncopation_pct: u32,
    /// Sustained length (beats) of ambient pads.
    pub pad_sustain_beats: f64,
    /// GM program for comping stabs.
    pub program: u8,
    /// GM program used when chords turn into sustained pads.
    pub pad_program: u8,
}

impl Default for ChordsConfig {
    fn default() -> Self {
        ChordsConfig {
            enabled: true,
            pad_enabled: true,
            progression_style: ProgressionStyle::default(),
            voicing_octave: 3,
            tension_chance_pct: 45,
            max_voices: 5,
            stab_downbeat_pct: 100,
            stab_offbeat_pct: 70,
            offbeat_syncopation_pct: 66,
            pad_sustain_beats: 4.0,
            program: 4,
            pad_program: 89,
        }
    }
}

/// Bass line parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BassConfig {
    pub enabled: bool,
    /// Register bounds (MIDI numbers) the bass stays inside.
    pub register_low: u8,
    pub register_high: u8,
    /// Probability (%) of the mid-bar fifth response ("heartbeat" groove).
    pub walk_chance_pct: u32,
    /// Probability (%) of a late-bar pickup note into the next chord.
    pub pickup_chance_pct: u32,
    /// Probability (%) of a chromatic approach note into the next chord.
    pub approach_chance_pct: u32,
    /// Probability (%) the colour tone is an octave rather than a third.
    pub octave_jump_chance_pct: u32,
    /// Velocity of the downbeat anchor note.
    pub anchor_velocity: u8,
    /// GM program.
    pub program: u8,
}

impl Default for BassConfig {
    fn default() -> Self {
        BassConfig {
            enabled: true,
            register_low: 40,
            register_high: 57,
            walk_chance_pct: 100,
            pickup_chance_pct: 50,
            approach_chance_pct: 60,
            octave_jump_chance_pct: 50,
            anchor_velocity: 92,
            program: 33,
        }
    }
}

/// Melody generation parameters — the most artistic layer, so it has the most
/// knobs: motif library size, development operations, register, breath.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MelodyConfig {
    pub enabled: bool,
    /// Number of motif cells seeded at song start (the melodic "vocabulary").
    pub motif_cells: u32,
    /// Attacks per motif cell (min/max).
    pub note_count_min: u32,
    pub note_count_max: u32,
    /// Multiplier on the energy-derived note probability (0..2).
    pub density_scale: f64,
    /// Register bounds for the lead line.
    pub register_low: u8,
    pub register_high: u8,
    /// Phrase length in bars (antecedent+consequent span).
    pub phrase_len_bars: u32,
    /// Probability (%) the answer phrase inverts the question contour.
    pub answer_inversion_pct: u32,
    /// Probability (%) the answer retrogrades the question rhythm.
    pub answer_retrograde_pct: u32,
    /// Probability (%) a weak-beat note borrows a chromatic neighbour.
    pub chromatic_passing_pct: u32,
    /// Probability (%) a phrase is followed by a full-bar rest (breathing).
    pub rest_between_phrases_pct: u32,
    /// GM program for the lead.
    pub program: u8,
}

impl Default for MelodyConfig {
    fn default() -> Self {
        MelodyConfig {
            enabled: true,
            motif_cells: 5,
            note_count_min: 2,
            note_count_max: 4,
            density_scale: 1.0,
            register_low: 60,
            register_high: 81,
            phrase_len_bars: 2,
            answer_inversion_pct: 33,
            answer_retrograde_pct: 33,
            chromatic_passing_pct: 12,
            rest_between_phrases_pct: 20,
            program: 0,
        }
    }
}

/// Drum groove parameters (GM kit).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumsConfig {
    pub enabled: bool,
    /// Global groove multiplier applied to every hit probability & velocity.
    pub groove_intensity: f64,
    /// Swing strength for hats/kick drag (0..1), combined with humanize.swing.
    pub swing_strength: f64,
    /// Probability (%) of the extra kick hits beyond the downbeat.
    pub kick_probability_pct: u32,
    /// Probability (%) that each backbeat (beats 2 and 4) fires at all.
    pub snare_probability_pct: u32,
    /// Probability (%) of ghost notes between backbeats.
    pub ghost_note_pct: u32,
    /// Probability (%) each closed-hat eighth slot fires.
    pub hat_probability_pct: u32,
    /// Probability (%) of an open hat at the end of a bar.
    pub open_hat_pct: u32,
    /// Probability (%) sixteenth shaker texture fires in high-energy bars.
    pub shaker_pct: u32,
    /// Probability (%) a tom fill appears on cadence bars.
    pub fill_on_cadence_pct: u32,
    /// Probability (%) of a crash on section downbeats.
    pub crash_on_section_start_pct: u32,
    /// Base velocity for primary hits (kick/snare backbeat).
    pub velocity_base: u8,
    /// Maximum per-hit timing drag in ticks (the "drunk" feel).
    pub drag_ticks: i64,
}

impl Default for DrumsConfig {
    fn default() -> Self {
        DrumsConfig {
            enabled: true,
            groove_intensity: 1.0,
            swing_strength: 0.5,
            kick_probability_pct: 60,
            snare_probability_pct: 95,
            ghost_note_pct: 25,
            hat_probability_pct: 80,
            open_hat_pct: 33,
            shaker_pct: 60,
            fill_on_cadence_pct: 80,
            crash_on_section_start_pct: 90,
            velocity_base: 96,
            drag_ticks: 40,
        }
    }
}

/// Global timing/velocity feel applied to every voice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HumanizeConfig {
    /// Swing strength 0..1: how far off-beat eighths are pushed late.
    /// `0` = straight, `0.5` = classic lofi "drunk", `1` = heavy shuffle.
    pub swing: f64,
    /// Timing jitter in ticks (±), applied per note attack.
    pub timing_jitter_ticks: i64,
    /// Velocity jitter in MIDI units (±), applied per note.
    pub velocity_jitter: i64,
    /// How much strong beats are accented over weak beats, 0..1.
    pub dynamics_range: f64,
}

impl Default for HumanizeConfig {
    fn default() -> Self {
        HumanizeConfig {
            swing: 0.5,
            timing_jitter_ticks: 18,
            velocity_jitter: 8,
            dynamics_range: 0.6,
        }
    }
}

/// Which tonal centres the seeded key picker favours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyBias {
    /// Common jazzy minor keys (Am/Dm/Gm/Em/Bm/Fm), 3-in-4 chance.
    LofiMinor,
    /// Any of the twelve pitch classes uniformly.
    AllKeys,
    /// Only major-ish centres (bright daytime lofi).
    Bright,
    /// Only minor/dorian centres (late-night).
    Dark,
}

impl Default for KeyBias {
    fn default() -> Self {
        KeyBias::LofiMinor
    }
}

/// Named one-click moods. Each writes a coherent bundle of knobs; the UI can
/// list them straight from the schema without knowing their contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoodPreset {
    RainyStudy,
    LateNightDrive,
    SundayMorning,
    DreamDrift,
}

impl MoodPreset {
    /// All presets, in display order (mirrors the schema option list).
    pub const ALL: [MoodPreset; 4] = [
        MoodPreset::RainyStudy,
        MoodPreset::LateNightDrive,
        MoodPreset::SundayMorning,
        MoodPreset::DreamDrift,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            MoodPreset::RainyStudy => "Rainy Study",
            MoodPreset::LateNightDrive => "Late-Night Drive",
            MoodPreset::SundayMorning => "Sunday Morning",
            MoodPreset::DreamDrift => "Dream Drift",
        }
    }

    /// Overlay this mood's values onto `cfg` (seed and key overrides survive).
    pub fn apply(&self, cfg: &mut super::GenerateConfig) {
        use super::GenerateConfig;
        let base = GenerateConfig::default();
        *cfg = GenerateConfig { seed: cfg.seed, scale: cfg.scale.clone(), tonic_pc: cfg.tonic_pc, ..base };
        match self {
            MoodPreset::RainyStudy => {
                cfg.tempo_bpm = 68.0;
                cfg.key_bias = KeyBias::Dark;
                cfg.chords.progression_style = ProgressionStyle::LofiJazz;
                cfg.drums.groove_intensity = 0.8;
                cfg.humanize.swing = 0.6;
                cfg.melody.density_scale = 0.8;
            }
            MoodPreset::LateNightDrive => {
                cfg.tempo_bpm = 76.0;
                cfg.key_bias = KeyBias::Dark;
                cfg.chords.progression_style = ProgressionStyle::NeoSoul;
                cfg.bass.octave_jump_chance_pct = 70;
                cfg.drums.kick_probability_pct = 75;
                cfg.drums.velocity_base = 104;
                cfg.humanize.swing = 0.45;
            }
            MoodPreset::SundayMorning => {
                cfg.tempo_bpm = 82.0;
                cfg.key_bias = KeyBias::Bright;
                cfg.scale = Some("major".into());
                cfg.chords.progression_style = ProgressionStyle::LofiJazz;
                cfg.melody.density_scale = 1.2;
                cfg.drums.groove_intensity = 0.9;
                cfg.arrangement.energy_curve = 0.7;
            }
            MoodPreset::DreamDrift => {
                cfg.tempo_bpm = 60.0;
                cfg.key_bias = KeyBias::AllKeys;
                cfg.chords.progression_style = ProgressionStyle::Ambient;
                cfg.chords.pad_sustain_beats = 8.0;
                cfg.drums.enabled = false;
                cfg.melody.rest_between_phrases_pct = 45;
                cfg.melody.density_scale = 0.6;
                cfg.humanize.swing = 0.3;
            }
        }
    }
}

