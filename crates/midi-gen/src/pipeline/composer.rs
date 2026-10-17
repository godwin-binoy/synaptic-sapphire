//! Top-level composition API: seed in, [`Composition`] out.

use thiserror::Error;

use crate::music::{NoteEvent, TempoMark};
use crate::rng::Rand;
use crate::structure::SongBlueprint;
use crate::voices::bass::BassVoice;
use crate::voices::chords::ChordsVoice;
use crate::voices::drums::DrumsVoice;
use crate::voices::melody::MelodyVoice;
use crate::voices::Voice as _;

/// A named track of events ready for serialization or rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: String,
    pub program: u8,
    /// `true` => channel 9 (percussion), else sequential channels from 0.
    pub percussion: bool,
    pub events: Vec<NoteEvent>,
}

/// Everything downstream engines need: arrangement info + per-track events.
#[derive(Debug, Clone, PartialEq)]
pub struct Composition {
    pub blueprint: SongBlueprint,
    pub conductor: Vec<TempoMark>,
    pub tracks: Vec<Track>,
}

impl Composition {
    /// Total duration in seconds at the initial tempo. Handy for video loops.
    pub fn duration_seconds(&self) -> f64 {
        let bars = self.blueprint.total_bars() as i64;
        crate::music::Ticks(bars * 4 * crate::music::PPQ as i64).to_seconds(self.blueprint.tempo_bpm)
    }
}

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("song length must be between {min} and {max} bars, got {got}")]
    LengthOutOfRange { min: u32, max: u32, got: u32 },
}

/// Deterministically compose a full song from `seed`.
///
/// Each voice draws from its own sub-seed derived from `seed`, so changing one
/// voice's algorithm never shifts the others' randomness — a key property for
/// reproducible streaming pipelines.
pub fn compose(seed: u64, bars_hint: u32) -> Result<Composition, GenerateError> {
    const MIN_BARS: u32 = 8;
    const MAX_BARS: u32 = 256;
    if !(MIN_BARS..=MAX_BARS).contains(&bars_hint) {
        return Err(GenerateError::LengthOutOfRange { min: MIN_BARS, max: MAX_BARS, got: bars_hint });
    }

    // Blueprint uses the master stream.
    let mut master = Rand::new(seed);
    let blueprint = SongBlueprint::generate(&mut master, seed, bars_hint);

    // Derive independent streams via mixing the seed with track indices.
    let stream = |i: u64| Rand::new(seed.rotate_left(7) ^ (i.wrapping_mul(0x9E37_79B9_7F4A_7C15)));

    let chord_events = ChordsVoice::generate(&blueprint, &mut stream(1));
    let bass_events = BassVoice::generate(&blueprint, &mut stream(2));
    let melody_events = MelodyVoice::generate(&blueprint, &mut stream(3));
    let drum_events = DrumsVoice::generate(&blueprint, &mut stream(4));

    let tracks = vec![
        Track {
            name: ChordsVoice::NAME.to_string(),
            program: ChordsVoice::PROGRAM,
            percussion: false,
            events: chord_events,
        },
        Track {
            name: BassVoice::NAME.to_string(),
            program: BassVoice::PROGRAM,
            percussion: false,
            events: bass_events,
        },
        Track {
            name: MelodyVoice::NAME.to_string(),
            program: MelodyVoice::PROGRAM,
            percussion: false,
            events: melody_events,
        },
        Track {
            name: DrumsVoice::NAME.to_string(),
            program: DrumsVoice::PROGRAM,
            percussion: true,
            events: drum_events,
        },
    ];

    Ok(Composition {
        blueprint,
        conductor: vec![TempoMark::new(crate::music::Ticks::ZERO, 0.0)],
        tracks,
    })
}
