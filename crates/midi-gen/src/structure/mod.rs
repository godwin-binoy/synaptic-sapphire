//! Song-level arrangement: sections, bars per section, tempo, key.
//!
//! A [`SongBlueprint`] is the *contract* between the planning stage and the
//! voice generators: it fully specifies what every bar of the song contains
//! (harmony, energy, instrumentation flags) so voices can be generated
//! independently and still fit together.

pub mod blueprint;

pub use blueprint::{BarRole, Energy, Section, SectionKind, SongBlueprint};

use crate::rng::Rand;
use crate::theory::progression::plan_progression;
use crate::theory::scale::Scale;

/// Reasonable lofi tempo band (BPM).
pub const TEMPO_MIN: f64 = 60.0;
pub const TEMPO_MAX: f64 = 90.0;

/// Pick a key: tonic pitch class + scale. Minor/dorian dominate because that
/// is the lofi aesthetic, with occasional major/mixolydian for brightness.
pub fn pick_key(rng: &mut Rand) -> (Scale, usize) {
    let weights = [2u32, 4, 4, 1]; // major, minor, dorian, mixolydian
    let scale = rng
        .weighted(&Scale::ALL, &weights)
        .cloned()
        .expect("non-empty scale set");
    // Tonics avoid the "too low for bass" extremes by preferring common keys.
    let favours = [9usize, 2, 7, 4, 11, 5]; // Am, Dm, Gm, Em, Bm, Fm
    let tonic = if rng.chance(3, 4) {
        *rng.pick(&favours).expect("non-empty")
    } else {
        rng.range_i64(0, 12) as usize
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
        let (s1, t1) = pick_key(&mut Rand::new(9));
        let (s2, t2) = pick_key(&mut Rand::new(9));
        assert_eq!(s1, s2);
        assert_eq!(t1, t2);
        assert!((0..12).contains(&t1));
    }

    #[test]
    fn plan_progression_import_works() {
        let (scale, tonic) = pick_key(&mut Rand::new(1));
        let plan = plan_progression(&mut Rand::new(2), &scale, tonic, 8);
        assert_eq!(plan.total_bars(), 8);
    }
}
