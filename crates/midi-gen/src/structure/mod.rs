//! Song-level arrangement: sections, bars per section, tempo, key.
//!
//! A [`SongBlueprint`] is the *contract* between the planning stage and the
//! voice generators: it fully specifies what every bar of the song contains
//! (harmony, energy, instrumentation flags) so voices can be generated
//! independently and still fit together.

pub mod blueprint;

pub use blueprint::{BarRole, Energy, Section, SectionKind, SongBlueprint};

use crate::config::KeyBias;
use crate::rng::Rand;
use crate::theory::scale::Scale;

/// Reasonable lofi tempo band (BPM).
pub const TEMPO_MIN: f64 = 60.0;
pub const TEMPO_MAX: f64 = 90.0;

/// Pick a key honouring the configured bias: tonic pitch class + scale.
pub fn pick_key(rng: &mut Rand, bias: KeyBias) -> (Scale, usize) {
    let favours_dark = [9usize, 2, 7, 4, 11, 5]; // Am, Dm, Gm, Em, Bm, Fm
    let favours_bright = [0usize, 7, 5, 2, 9, 4]; // C, G, F, D, A, E

    // Weights align with `Scale::ALL` order:
    // [major, minor, dorian, mixolydian, lydian, harmonic minor]
    let all_weights: Vec<u32> = match bias {
        KeyBias::LofiMinor => vec![2, 4, 4, 1, 1, 1],
        KeyBias::AllKeys => vec![1, 1, 1, 1, 1, 1],
        KeyBias::Bright => vec![3, 0, 1, 2, 2, 0],
        KeyBias::Dark => vec![0, 4, 3, 0, 0, 2],
    };

    let scale = rng
        .weighted(&Scale::ALL, &all_weights)
        .cloned()
        .expect("non-empty scale set");

    let tonic = match bias {
        KeyBias::LofiMinor | KeyBias::Dark => {
            if rng.chance(3, 4) {
                rng.pick(&favours_dark).expect("non-empty")
            } else {
                rng.range_i64(0, 12) as usize
            }
        }
        KeyBias::Bright => {
            if rng.chance(3, 4) {
                rng.pick(&favours_bright).expect("non-empty")
            } else {
                rng.range_i64(0, 12) as usize
            }
        }
        KeyBias::AllKeys => rng.range_i64(0, 12) as usize,
    };
    (scale, tonic)
}

/// Sample a tempo inside the lofi band, biased towards the lower end.
pub fn pick_tempo(rng: &mut Rand) -> f64 {
    let t = rng.range_f64(0.0, 1.0);
    // Quadratic bias: more 60-75 than 80-90 BPM.
    TEMPO_MIN + (TEMPO_MAX - TEMPO_MIN) * t * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_stays_in_band() {
        let mut rng = Rand::new(4);
        for _ in 0..500 {
            let t = pick_tempo(&mut rng);
            assert!((TEMPO_MIN..=TEMPO_MAX).contains(&t), "{t} out of band");
        }
    }

    #[test]
    fn key_is_deterministic_and_valid() {
        let (s1, t1) = pick_key(&mut Rand::new(9), KeyBias::LofiMinor);
        let (s2, t2) = pick_key(&mut Rand::new(9), KeyBias::LofiMinor);
        assert_eq!(s1, s2);
        assert_eq!(t1, t2);
        assert!((0..12).contains(&t1));
    }

    #[test]
    fn bright_bias_never_picks_minor() {
        let mut rng = Rand::new(31);
        for _ in 0..200 {
            let (scale, _) = pick_key(&mut rng, KeyBias::Bright);
            assert_ne!(scale.name, "minor", "bright bias picked minor");
        }
    }

    #[test]
    fn dark_bias_never_picks_major() {
        let mut rng = Rand::new(32);
        for _ in 0..200 {
            let (scale, _) = pick_key(&mut rng, KeyBias::Dark);
            assert_ne!(scale.name, "major", "dark bias picked major");
        }
    }

    #[test]
    fn plan_progression_import_works() {
        use crate::theory::progression::plan_progression;
        let (scale, tonic) = pick_key(&mut Rand::new(1), KeyBias::LofiMinor);
        let plan = plan_progression(&mut Rand::new(2), &scale, tonic, 8);
        assert_eq!(plan.total_bars(), 8);
    }
}
