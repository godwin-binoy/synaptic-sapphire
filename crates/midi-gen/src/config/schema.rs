//! Machine-readable description of every knob in [`super::GenerateConfig`].
//!
//! The web UI fetches `GET /api/schema`, then renders one control per field.
//! Because the schema is derived from the *same* defaults as the config, the
//! two can never drift apart — a new engine knob appears in the interface with
//! zero front-end work.

use serde::{Deserialize, Serialize};

/// What kind of control the UI should build for a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaKind {
    /// Free integer input (with min/max).
    Int,
    /// Slider-style float input (min/max/step).
    Float,
    /// Checkbox.
    Bool,
    /// Dropdown over a fixed option list.
    Enum,
    /// Seed: number input plus a "randomise" affordance in the UI.
    Seed,
    /// 0..12 pitch-class picker (note names).
    PitchClass,
    /// 0..127 General MIDI instrument picker.
    GmProgram,
    /// Free text (scale name).
    Text,
}

/// One selectable option for `Enum` fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaOption {
    pub value: String,
    pub label: String,
}

impl SchemaOption {
    fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        SchemaOption { value: value.into(), label: label.into() }
    }
}

/// A single leaf field of the configuration tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaField {
    /// Dotted path into the JSON config, e.g. `"melody.density_scale"`.
    pub path: String,
    /// Human label shown next to the control.
    pub label: String,
    /// Docstring/help text surfaced as a tooltip.
    pub help: String,
    /// Grouping card the field belongs to ("Melody", "Feel", ...).
    pub group: String,
    pub kind: SchemaKind,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    /// Default value (from `GenerateConfig::default()`), serialised naturally.
    pub default: serde_json::Value,
    /// Options for `Enum`-like kinds.
    pub options: Vec<SchemaOption>,
}

/// The whole schema: ordered groups of ordered fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigSchema {
    pub groups: Vec<SchemaGroup>,
}

/// A UI section (one card) containing related fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaGroup {
    pub id: String,
    pub title: String,
    pub blurb: String,
    pub fields: Vec<SchemaField>,
}

// ---- tiny builder helpers keep the table below declarative -------------------

fn num(
    path: &str,
    label: &str,
    help: &str,
    group: &str,
    kind: SchemaKind,
    default: serde_json::Value,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
) -> SchemaField {
    SchemaField {
        path: path.into(),
        label: label.into(),
        help: help.into(),
        group: group.into(),
        kind,
        min,
        max,
        step,
        default,
        options: vec![],
    }
}

fn slider_int(path: &str, label: &str, help: &str, group: &str, d: u32, lo: u32, hi: u32) -> SchemaField {
    num(path, label, help, group, SchemaKind::Int, serde_json::json!(d), Some(lo as f64), Some(hi as f64), Some(1.0))
}

fn pct(path: &str, label: &str, help: &str, group: &str, d: u32) -> SchemaField {
    slider_int(path, label, help, group, d, 0, 100)
}

fn slider_f(path: &str, label: &str, help: &str, group: &str, d: f64, lo: f64, hi: f64, step: f64) -> SchemaField {
    num(path, label, help, group, SchemaKind::Float, serde_json::json!(d), Some(lo), Some(hi), Some(step))
}

fn toggle(path: &str, label: &str, help: &str, group: &str, d: bool) -> SchemaField {
    num(path, label, help, group, SchemaKind::Bool, serde_json::json!(d), None, None, None)
}

fn en(path: &str, label: &str, help: &str, group: &str, default: &str, opts: &[(&str, &str)]) -> SchemaField {
    let mut f = num(path, label, help, group, SchemaKind::Enum, serde_json::json!(default), None, None, None);
    f.options = opts.iter().map(|(v, l)| SchemaOption::new(*v, *l)).collect();
    f
}

fn gm(path: &str, label: &str, help: &str, group: &str, d: u8) -> SchemaField {
    num(path, label, help, group, SchemaKind::GmProgram, serde_json::json!(d), Some(0.0), Some(127.0), Some(1.0))
}

const NOTE_OPTIONS: [(&str, &str); 12] = [
    ("0", "C"), ("1", "C#"), ("2", "D"), ("3", "D#"), ("4", "E"), ("5", "F"),
    ("6", "F#"), ("7", "G"), ("8", "G#"), ("9", "A"), ("10", "A#"), ("11", "B"),
];

const SCALE_OPTIONS: [(&str, &str); 7] = [
    ("", "auto (seeded)"),
    ("major", "Major"),
    ("minor", "Minor"),
    ("dorian", "Dorian"),
    ("mixolydian", "Mixolydian"),
    ("lydian", "Lydian"),
    ("harmonic minor", "Harmonic Minor"),
];

const PRESET_OPTIONS: [(&str, &str); 4] = [
    ("rainy_study", "🌧 Rainy Study"),
    ("late_night_drive", "🌃 Late-Night Drive"),
    ("sunday_morning", "☀ Sunday Morning"),
    ("dream_drift", "💭 Dream Drift"),
];

const INSTRUMENTS: [(&str, &str); 128] = [
    ("0","Acoustic Grand Piano"),("1","Bright Acoustic Piano"),("2","Electric Grand Piano"),
    ("3","Honky-tonk Piano"),("4","Electric Piano 1"),("5","Electric Piano 2"),("6","Harpsichord"),
    ("7","Clavi"),("8","Celesta"),("9","Glockenspiel"),("10","Music Box"),("11","Vibraphone"),
    ("12","Marimba"),("13","Xylophone"),("14","Tubular Bells"),("15","Dulcimer"),("16","Drawbar Organ"),
    ("17","Percussive Organ"),("18","Rock Organ"),("19","Church Organ"),("20","Reed Organ"),
    ("21","Accordion"),("22","Harmonica"),("23","Tango Accordion"),("24","Acoustic Bass (nylon)"),
    ("25","Acoustic Bass (steel)"),("26","Electric Bass (finger)"),("27","Electric Bass (pick)"),
    ("28","Fretless Bass"),("29","Slap Bass 1"),("30","Slap Bass 2"),("31","Synth Bass 1"),
    ("32","Synth Bass 2"),("33","Violin"),("34","Viola"),("35","Cello"),("36","Contrabass"),
    ("37","Tremolo Strings"),("38","Pizzicato Strings"),("39","Orchestral Harp"),("40","Timpani"),
    ("41","String Ensemble 1"),("42","String Ensemble 2"),("43","Synth Strings 1"),("44","Synth Strings 2"),
    ("45","Choir Aahs"),("46","Voice Oohs"),("47","Synth Voice"),("48","Orchestra Hit"),("49","Trumpet"),
    ("50","Trombone"),("51","Tuba"),("52","Muted Trumpet"),("53","French Horn"),("54","Tuba(Warm)"),
    ("55","Brass Section"),("56","Synth Brass 1"),("57","Synth Brass 2"),("58","Reed Saxophone"),
    ("59","Harmonic Sax"),("60","Contrabass Sax"),("61","Muted Trumpet(Soft)"),("62","Baritone Sax"),
    ("63","Oboe"),("64","English Horn"),("65","Bassoon"),("66","Clarinet"),("67","Piccolo"),
    ("68","Flute"),("69","Recorder"),("70","Shakuhachi"),("71","Whistle"),("72","Ocarina"),
    ("73","Lead 1 (square)"),("74","Lead 2 (sawtooth)"),("75","Lead 3 (calliope)"),("76","Lead 4 (chiff)"),
    ("77","Lead 5 (portato)"),("78","Lead 6 (voice)"),("79","Lead 7 (fifths)"),("80","Lead 8 (bass+lead)"),
    ("81","Pad 1 (new age)"),("82","Pad 2 (warm)"),("83","Pad 3 (polysynth)"),("84","Pad 4 (choke)"),
    ("85","Pad 5 (glass)"),("86","Pad 6 (metallic)"),("87","Pad 7 (halo)"),("88","Pad 8 (sweep)"),
    ("89","Rain"),("90","Soundtrack"),("91","Crystal"),("92","Atmosphere"),("93","Brightness"),
    ("94","Fairyglobe"),("95","Canonic"),("96","Wallpaper"),("97","FX 1 (glitch)"),("98","FX 2 (flash)"),
    ("99","FX 3 (cry)"),("100","FX 4 (wind)"),("101","FX 5 (pyramid)"),("102","FX 6 (sci-fi)"),
    ("103","Sitar"),("104","Banjo"),("105","Shamisen"),("106","Koto"),("107","Kalimba"),
    ("108","Bag pipe"),("109","Fiddle"),("110","Shanai"),("111","Tinkle Bell"),("112","Agogo"),
    ("113","Steel Drums"),("114","Woodblock"),("115","Taiko Drum"),("116","Melodic Tom"),
    ("117","Synth Drum"),("118","Reverse Cymbal"),("119","Guitar Noise"),("120","Breath Noise"),
    ("121","Seashore"),("122","Bird Tweet"),("123","Telephone Ring"),("124","Helicopter"),("125","Applause"),
    ("126","Gunshot"),("127","(reserved)"),
];

impl ConfigSchema {
    /// Build the schema. Values/ranges mirror `GenerateConfig` exactly; the
    /// unit test below guarantees the paths stay valid against real JSON.
    pub fn derive() -> ConfigSchema {
        use super::generate_config::ProgressionStyle;

        let style_opts = |mixed_default: bool| {
            let mut v: Vec<(&str, &str)> = vec![
                ("lofi_jazz", "Lofi Jazz (house sound)"),
                ("neo_soul", "Neo-Soul"),
                ("ambient", "Ambient"),
                ("bluesy", "Bluesy"),
            ];
            if mixed_default {
                v.push(("mixed", "Mixed (per-section)"));
            }
            v
        };

        let core = vec![
            num("seed", "Seed", "Same seed + same settings ⇒ identical song.", "Song",
                SchemaKind::Seed, serde_json::json!(0x5EED_1E05u64), Some(0.0), None, Some(1.0)),
            en("preset", "Mood preset", "One-click bundles; tweak anything afterwards.", "Song", "", &PRESET_OPTIONS),
            en("scale", "Scale", "Leave on auto for a seeded key.", "Song", "", &SCALE_OPTIONS),
            {
                let mut f = num("tonic_pc", "Key / tonic", "Tonic note when a scale is chosen.", "Song",
                    SchemaKind::PitchClass, serde_json::Value::Null, None, None, None);
                f.options = NOTE_OPTIONS.iter().map(|(v, l)| SchemaOption::new(*v, *l)).collect();
                f
            },
            en("key_bias", "Key preference", "Which tonal centres the auto-picker favours.", "Song",
                "lofi_minor",
                &[("lofi_minor", "Lofi minor keys"), ("all_keys", "All twelve keys"),
                  ("bright", "Bright (major-ish)"), ("dark", "Dark (minor/dorian)")]),
            slider_f("tempo_bpm", "Tempo (BPM)", "Slow is the point.", "Song", 72.0, 40.0, 140.0, 0.5),
        ];

        let arrangement = vec![
            slider_int("arrangement.bars_hint", "Target length (bars)", "Rounded up to whole sections.", "Arrangement", 48, 8, 256),
            toggle("arrangement.intro_on", "Intro", "Start with a low-energy intro.", "Arrangement", true),
            slider_int("arrangement.section_bars_intro", "Intro bars", "", "Arrangement", 8, 2, 32),
            slider_int("arrangement.section_bars_verse", "Verse bars", "", "Arrangement", 16, 4, 64),
            slider_int("arrangement.verse_repeats", "Verse repeats", "", "Arrangement", 2, 0, 8),
            slider_int("arrangement.section_bars_chorus", "Chorus bars", "", "Arrangement", 16, 4, 64),
            slider_int("arrangement.chorus_repeats", "Chorus repeats", "", "Arrangement", 2, 0, 8),
            toggle("arrangement.bridge_on", "Bridge", "Insert a contrast section.", "Arrangement", true),
            slider_int("arrangement.section_bars_bridge", "Bridge bars", "", "Arrangement", 8, 2, 32),
            toggle("arrangement.outro_on", "Outro", "End with a drumless ambient outro.", "Arrangement", true),
            slider_int("arrangement.section_bars_outro", "Outro bars", "", "Arrangement", 8, 2, 32),
            slider_f("arrangement.energy_curve", "Energy build", "How much later sections intensify.", "Arrangement", 0.5, 0.0, 1.0, 0.05),
            pct("arrangement.reharmonise_chance_pct", "Reharmonise chance %", "Swap one bar of repeated loops for variety.", "Arrangement", 35),
            pct("arrangement.two_bar_hold_chance_pct", "Two-bar holds %", "Relax harmonic rhythm by sustaining chords.", "Arrangement", 25),
        ];

        let chords = vec![
            toggle("chords.enabled", "Chords on", "Rhodes/pad comping layer.", "Chords", true),
            toggle("chords.pad_enabled", "Pads in ambient", "Sustained pad layer for intro/outro bars.", "Chords", true),
            en("chords.progression_style", "Progression style", "The harmonic vocabulary.", "Chords",
                match ProgressionStyle::default() { ProgressionStyle::LofiJazz => "lofi_jazz", _ => "mixed" },
                &style_opts(true)),
            slider_int("chords.voicing_octave", "Voicing octave", "Register of chord roots.", "Chords", 3, 2, 6),
            pct("chords.tension_chance_pct", "Extensions %", "Chance of adding 9ths/11ths/13ths.", "Chords", 45),
            slider_int("chords.max_voices", "Max voices", "Notes per chord voicing.", "Chords", 5, 3, 8),
            pct("chords.stab_downbeat_pct", "Downbeat stab %", "", "Chords", 100),
            pct("chords.stab_offbeat_pct", "Off-beat stab %", "Syncopated second hit.", "Chords", 70),
            pct("chords.offbeat_syncopation_pct", "Off-beat pushes %", "'And of 3' instead of beat 3.", "Chords", 66),
            slider_f("chords.pad_sustain_beats", "Pad sustain (beats)", "Length of ambient pads.", "Chords", 4.0, 0.5, 16.0, 0.5),
            gm("chords.program", "Chord instrument", "", "Chords", 4),
            gm("chords.pad_program", "Pad instrument", "", "Chords", 89),
        ];

        let bass = vec![
            toggle("bass.enabled", "Bass on", "", "Bass", true),
            slider_int("bass.register_low", "Lowest note", "MIDI number floor.", "Bass", 40, 24, 72),
            slider_int("bass.register_high", "Highest note", "MIDI number ceiling.", "Bass", 57, 36, 84),
            pct("bass.walk_chance_pct", "Walk %", "Mid-bar fifth responses.", "Bass", 100),
            pct("bass.pickup_chance_pct", "Pickups %", "Late-bar pickup into the next chord.", "Bass", 50),
            pct("bass.approach_chance_pct", "Approach notes %", "Chromatic lead-ins to changes.", "Bass", 60),
            pct("bass.octave_jump_chance_pct", "Octave jumps %", "", "Bass", 50),
            slider_int("bass.anchor_velocity", "Anchor velocity", "Downbeat loudness.", "Bass", 92, 30, 127),
            gm("bass.program", "Bass instrument", "", "Bass", 33),
        ];

        let melody = vec![
            toggle("melody.enabled", "Melody on", "", "Melody", true),
            slider_int("melody.motif_cells", "Motif vocabulary", "Distinct motif cells invented per song.", "Melody", 5, 2, 12),
            slider_int("melody.note_count_min", "Notes per cell (min)", "", "Melody", 2, 1, 8),
            slider_int("melody.note_count_max", "Notes per cell (max)", "", "Melody", 4, 2, 12),
            slider_f("melody.density_scale", "Density", "Global multiplier on note probability.", "Melody", 1.0, 0.0, 2.0, 0.05),
            slider_int("melody.register_low", "Lowest note", "", "Melody", 60, 48, 96),
            slider_int("melody.register_high", "Highest note", "", "Melody", 81, 60, 108),
            slider_int("melody.phrase_len_bars", "Phrase length (bars)", "Question+answer span.", "Melody", 2, 1, 8),
            pct("melody.answer_inversion_pct", "Inverted answers %", "Mirror the question contour.", "Melody", 33),
            pct("melody.answer_retrograde_pct", "Retrograded answers %", "Flip the question rhythm.", "Melody", 33),
            pct("melody.chromatic_passing_pct", "Chromatic passing %", "Tension notes on weak beats.", "Melody", 12),
            pct("melody.rest_between_phrases_pct", "Breath rests %", "Full-bar silence after phrases.", "Melody", 20),
            gm("melody.program", "Lead instrument", "", "Melody", 0),
        ];

        let drums = vec![
            toggle("drums.enabled", "Drums on", "", "Drums", true),
            slider_f("drums.groove_intensity", "Groove intensity", "Scales all hit probabilities & velocities.", "Drums", 1.0, 0.0, 1.5, 0.05),
            slider_f("drums.swing_strength", "Swing strength", "Hat/kick push amount.", "Drums", 0.5, 0.0, 1.0, 0.05),
            pct("drums.kick_probability_pct", "Extra kicks %", "Beyond the downbeat.", "Drums", 60),
            pct("drums.snare_probability_pct", "Backbeat %", "Chance beats 2 & 4 fire.", "Drums", 95),
            pct("drums.ghost_note_pct", "Ghost notes %", "Whisper snares between backbeats.", "Drums", 25),
            pct("drums.hat_probability_pct", "Hi-hat density %", "Per eighth-note slot.", "Drums", 80),
            pct("drums.open_hat_pct", "Open hats %", "", "Drums", 33),
            pct("drums.shaker_pct", "Shaker texture %", "Sixteenth shaker in busy bars.", "Drums", 60),
            pct("drums.fill_on_cadence_pct", "Cadence fills %", "Tom fills ending sections.", "Drums", 80),
            pct("drums.crash_on_section_start_pct", "Section crashes %", "", "Drums", 90),
            slider_int("drums.velocity_base", "Base velocity", "", "Drums", 96, 20, 127),
            slider_int("drums.drag_ticks", "Drag (ticks)", "Max late timing per hit.", "Drums", 40, 0, 120),
        ];

        let feel = vec![
            slider_f("humanize.swing", "Global swing", "Pushes off-beat eighths late.", "Feel", 0.5, 0.0, 1.0, 0.05),
            slider_int("humanize.timing_jitter_ticks", "Timing jitter (ticks)", "± random offset per attack.", "Feel", 18, 0, 120),
            slider_int("humanize.velocity_jitter", "Velocity jitter", "± random loudness per note.", "Feel", 8, 0, 40),
            slider_f("humanize.dynamics_range", "Dynamics range", "Accent spread strong vs weak beats.", "Feel", 0.6, 0.0, 1.0, 0.05),
        ];

        ConfigSchema {
            groups: vec![
                SchemaGroup { id: "song".into(), title: "🎛 Song".into(),
                    blurb: "Identity, key and tempo of the piece.".into(), fields: core },
                SchemaGroup { id: "arrangement".into(), title: "🏗 Arrangement".into(),
                    blurb: "Form: sections, lengths, harmonic motion.".into(), fields: arrangement },
                SchemaGroup { id: "chords".into(), title: "🎹 Chords".into(),
                    blurb: "Rhodes comping, voicings and extensions.".into(), fields: chords },
                SchemaGroup { id: "bass".into(), title: "🎸 Bass".into(),
                    blurb: "Root-fifth groove with approach notes.".into(), fields: bass },
                SchemaGroup { id: "melody".into(), title: "🎺 Melody".into(),
                    blurb: "Motif development, register and breath.".into(), fields: melody },
                SchemaGroup { id: "drums".into(), title: "🥁 Drums".into(),
                    blurb: "Boom-bap patterns, ghost notes, fills.".into(), fields: drums },
                SchemaGroup { id: "feel".into(), title: "🫀 Feel".into(),
                    blurb: "Humanisation shared by every player.".into(), fields: feel },
            ],
        }
    }

    /// Flat list of every field (used by tests and search boxes in the UI).
    pub fn all_fields(&self) -> Vec<&SchemaField> {
        self.groups.iter().flat_map(|g| g.fields.iter()).collect()
    }

    /// GM program number → human instrument name (shared with the front-end
    /// via `/api/gm`; also handy server-side for logs).
    pub fn gm_name(program: u8) -> &'static str {
        INSTRUMENTS
            .iter()
            .find(|(v, _)| *v == program.to_string())
            .map(|(_, l)| *l)
            .unwrap_or("Unknown")
    }

    /// The full GM table for the client's dropdowns.
    pub fn gm_table() -> Vec<SchemaOption> {
        INSTRUMENTS.iter().map(|(v, l)| SchemaOption::new(*v, *l)).collect()
    }

    /// Render the schema as pretty JSON for HTTP responses.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("schema is serialisable by construction")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerateConfig;

    #[test]
    fn every_schema_path_exists_in_serialised_config() {
        // Serialise the default config, then walk each dotted path through it.
        let value = serde_json::to_value(GenerateConfig::default()).unwrap();
        let schema = GenerateConfig::schema();
        for f in schema.all_fields() {
            if f.path == "preset" {
                continue; // virtual field, not part of the config itself
            }
            let mut cur = &value;
            for seg in f.path.split('.') {
                cur = cur.get(seg).unwrap_or_else(|| panic!("schema path `{}` missing in config JSON", f.path));
            }
        }
    }

    #[test]
    fn schema_defaults_match_config_defaults() {
        let cfg = GenerateConfig::default();
        let value = serde_json::to_value(&cfg).unwrap();
        let schema = GenerateConfig::schema();
        for f in schema.all_fields() {
            if f.path == "preset" {
                continue;
            }
            let mut cur = &value;
            for seg in f.path.split('.') {
                cur = cur.get(seg).unwrap();
            }
            if matches!(f.kind, SchemaKind::PitchClass) && cur.is_null() {
                continue; // Option<usize> default null
            }
            if f.path == "scale" && cur.is_null() {
                continue; // Option<String> auto-pick default null
            }
            assert_eq!(&f.default, cur, "default mismatch at `{}`", f.path);
        }
    }

    #[test]
    fn gm_table_is_complete_and_lookup_works() {
        assert_eq!(ConfigSchema::gm_table().len(), 128);
        assert_eq!(ConfigSchema::gm_name(4), "Electric Piano 1");
    }

    #[test]
    fn schema_itself_roundtrips_as_json() {
        let s = GenerateConfig::schema();
        let j = s.to_json_pretty();
        let back: ConfigSchema = serde_json::from_str(&j).unwrap();
        assert_eq!(s, back);
    }
}
