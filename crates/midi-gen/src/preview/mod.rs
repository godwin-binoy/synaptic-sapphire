//! Preview audio engine: quick stereo WAV renders of generated MIDI.
//!
//! Design goals (see `markdown/architecture.md`):
//! * **Instant** — no synthesis setup, no plugins; a 30 s song renders in ms.
//! * **Dependency-free** — pure std, DSP written here. The high-end render
//!   engine (sampling/FX/mixing) will replace this later without touching the
//!   generation pipeline.
//! * **Faithful to structure, not timbre** — simple per-voice oscillators +
//!   noise drums are enough to *hear* arrangement, groove and harmony.
//!
//! Pipeline: [`Song`] → per-channel note lists → oscillator/noise voices →
//! f32 samples → soft clip → 16-bit PCM WAV bytes.

pub mod synth;
pub mod wav;

pub use synth::{render_song, RenderedAudio};
pub use wav::write_wav;

use crate::config::GenerateConfig;
use crate::music::{NoteEvent, Song};
use crate::pipeline::composer::generate_song;

/// Convenience: generate from config and render a preview in one call.
pub fn preview(cfg: &GenerateConfig) -> Result<RenderedAudio, crate::pipeline::GenerationError> {
    let out = generate_song(cfg)?;
    Ok(render_song(&out.song))
}

/// Collect `(channel, note)` pairs sorted by start tick for the synth.
pub(crate) fn notes_by_channel(song: &Song) -> Vec<(u8, Vec<crate::music::NoteOn>)> {
    let mut map: std::collections::BTreeMap<u8, Vec<crate::music::NoteOn>> = Default::default();
    for t in &song.tracks {
        for e in &t.events {
            if let NoteEvent::Note(n) = e {
                map.entry(t.channel).or_default().push(n.clone());
            }
        }
    }
    for v in map.values_mut() {
        v.sort_by_key(|n| n.start.as_i64());
    }
    map.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_grouped_and_sorted() {
        let song = generate_song(&GenerateConfig::default()).unwrap().song;
        let groups = notes_by_channel(&song);
        assert!(groups.len() >= 3); // drums(10), bass(3), chords(5), melody(1)
        for (_, notes) in &groups {
            for w in notes.windows(2) {
                assert!(w[0].start.as_i64() <= w[1].start.as_i64());
            }
        }
    }
}
