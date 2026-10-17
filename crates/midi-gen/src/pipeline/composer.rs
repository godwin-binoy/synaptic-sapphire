//! Top-level orchestration of one generation request.

use std::path::PathBuf;

use crate::config::{ConfigError, GenerateConfig};
use crate::music::{NoteEvent, Parts, Song, Ticks};
use crate::rng::Rand;
use crate::structure::SongBlueprint;
use crate::voices::Voice;

/// Anything that can stop a render.
#[derive(Debug, thiserror::Error)]
pub enum GenerationError {
    #[error("config produced an empty song (all voices disabled?)")]
    EmptySong,
    #[error("invalid configuration: {0}")]
    Config(#[from] ConfigError),
    #[error("could not write output file: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of a successful generation.
pub struct GenerationOutput {
    pub song: Song,
    pub blueprint_summary: String,
    pub written_path: Option<PathBuf>,
}

impl GenerationOutput {
    /// Serialise the song to Standard MIDI (SMF type 1) bytes.
    pub fn midi_bytes(&self) -> Vec<u8> {
        super::midi_writer::to_midi_bytes(&self.song)
    }
}

/// Generate a full song from `cfg`. Deterministic: same `(seed, config)` pair
/// always yields byte-identical MIDI.
pub fn generate_song(cfg: &GenerateConfig) -> Result<GenerationOutput, GenerationError> {
    let mut cfg = cfg.clone();
    cfg.clamp_all();
    cfg.validate()?;

    // Each stage draws from its own stream so enabling/disabling one voice
    // never changes another's output.
    let mut bp_rng = Rand::derive(cfg.seed, b"blueprint");
    let bp = SongBlueprint::generate(&mut bp_rng, &cfg);

    let tracks = vec![
        build_track::<crate::voices::drums::DrumsVoice>(&bp, &cfg, b"drums"),
        build_track::<crate::voices::bass::BassVoice>(&bp, &cfg, b"bass"),
        build_track::<crate::voices::chords::ChordsVoice>(&bp, &cfg, b"chords"),
        build_track::<crate::voices::melody::MelodyVoice>(&bp, &cfg, b"melody"),
    ];
    let total_bars = bp.total_bars() as i64;
    let song = Song::new(
        format!("lofi seed {}", cfg.seed),
        cfg.tempo_bpm,
        4,
        Ticks(total_bars * Parts::beats(4).to_ticks().as_i64()),
        tracks,
    );

    if !song.tracks.iter().any(|t| t.events.iter().any(|e| matches!(e, NoteEvent::Note(_)))) {
        return Err(GenerationError::EmptySong);
    }

    Ok(GenerationOutput {
        song,
        blueprint_summary: bp.describe(),
        written_path: None,
    })
}

fn build_track<V: Voice>(bp: &SongBlueprint, cfg: &GenerateConfig, tag: &'static [u8]) -> crate::music::Track {
    let mut rng = Rand::derive(cfg.seed, tag);
    let events = V::generate(bp, cfg, &mut rng);
    crate::music::Track { name: V::NAME.to_string(), channel: channel_for(tag), program: V::PROGRAM, events }
}

/// Fixed GM channel map: drums on 10, melodic voices on 1/3/5.
fn channel_for(tag: &[u8]) -> u8 {
    match tag {
        b"drums" => 10,
        b"bass" => 3,
        b"chords" => 5,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_to_end_produces_a_song() {
        let out = generate_song(&GenerateConfig::default()).unwrap();
        assert_eq!(out.song.tracks.len(), 4);
        assert!(out.song.total_ticks.as_i64() > 0);
        assert!(!out.blueprint_summary.is_empty());
    }

    #[test]
    fn all_notes_are_in_midi_range() {
        let out = generate_song(&GenerateConfig::default()).unwrap();
        for t in &out.song.tracks {
            for e in &t.events {
                if let NoteEvent::Note(n) = e {
                    assert!(n.pitch.midi() <= 127);
                    assert!(n.duration.as_i64() > 0);
                    assert!(n.start.as_i64() >= 0);
                }
            }
        }
    }

    #[test]
    fn deterministic_output_bytes() {
        let a = generate_song(&GenerateConfig::default()).unwrap().midi_bytes();
        let b = generate_song(&GenerateConfig::default()).unwrap().midi_bytes();
        assert_eq!(a, b);
        assert!(a.starts_with(b"MThd"));
    }

    #[test]
    fn different_seeds_differ() {
        let mut cfg = GenerateConfig::default();
        cfg.seed = 1;
        let a = generate_song(&cfg).unwrap().midi_bytes();
        cfg.seed = 2;
        let b = generate_song(&cfg).unwrap().midi_bytes();
        assert_ne!(a, b);
    }

    #[test]
    fn all_voices_off_is_an_error() {
        let mut cfg = GenerateConfig::default();
        cfg.drums.enabled = false;
        cfg.bass.enabled = false;
        cfg.chords.enabled = false;
        cfg.melody.enabled = false;
        assert!(matches!(generate_song(&cfg), Err(GenerationError::EmptySong)));
    }

    #[test]
    fn voice_streams_are_independent() {
        // Turning chords off must not change the drum track.
        let a = generate_song(&GenerateConfig::default()).unwrap();
        let mut cfg = GenerateConfig::default();
        cfg.chords.enabled = false;
        let b = generate_song(&cfg).unwrap();
        let drums_a = a.song.tracks.iter().find(|t| t.name == "Drums").unwrap();
        let drums_b = b.song.tracks.iter().find(|t| t.name == "Drums").unwrap();
        assert_eq!(format!("{drums_a:?}"), format!("{drums_b:?}"));
    }

    #[test]
    fn extreme_configs_still_render() {
        let mut cfg = GenerateConfig::default();
        cfg.arrangement.bars_hint = 8;
        cfg.humanize.timing_jitter_ticks = 30;
        cfg.humanize.swing = 0.6;
        cfg.melody.density_scale = 2.0;
        cfg.drums.groove_intensity = 2.0;
        let out = generate_song(&cfg).unwrap();
        assert!(!out.midi_bytes().is_empty());
    }
}
