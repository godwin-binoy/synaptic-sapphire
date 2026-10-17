//! Deterministic pseudo-random source.
//!
//! Every generator draws from [`Rand`] rather than a concrete PRNG so that
//! (a) compositions are fully reproducible from a seed, which is essential for
//! a 24/7 stream where the same "episode" must be re-renderable bit-for-bit,
//! and (b) tests can inject a fixed sequence to pin algorithmic behaviour.

/// A seedable, reproducible random source (splitmix64 stream).
#[derive(Debug, Clone)]
pub struct Rand {
    state: u64,
}

impl Rand {
    /// Create from an arbitrary 64-bit seed. Seed `0` is legal.
    pub fn new(seed: u64) -> Self {
        Rand { state: seed }
    }

    /// Derive an independent sub-stream from a master seed plus a domain tag.
    ///
    /// Each generation stage (blueprint, drums, bass, chords, melody) draws
    /// from its own derived stream, so adding or disabling one voice can never
    /// shift the random sequence of another — compositions stay reproducible
    /// even as the engine grows.
    pub fn derive(seed: u64, tag: &[u8]) -> Self {
        // splitmix-style avalanche over (tag, seed): cheap, deterministic and
        // decorrelated enough to serve as independent per-voice streams.
        let mut h = 0xcbf2_9ce4_8422_2325u64; // FNV offset basis
        for &b in tag.iter().chain(&seed.to_le_bytes()) {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3); // FNV prime
        }
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        h ^= h >> 29;
        Rand::new(h)
    }

    /// Raw next u64 in the stream.
    pub fn next_u64(&mut self) -> u64 {
        // splitmix64: tiny, fast, excellent statistical quality for music.
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_8BEB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `lo..hi`. Panics if the range is empty.
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        assert!(hi > lo, "empty random range {lo}..{hi}");
        let span = (hi - lo) as u64;
        lo + (self.next_u64() % span) as i64
    }

    /// Uniform float in `lo..hi`.
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        lo + unit * (hi - lo)
    }

    /// True with probability `numerator/denominator`.
    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        debug_assert!(denominator > 0);
        if denominator == 0 {
            return false;
        }
        self.range_i64(0, denominator as i64) < numerator as i64
    }

    /// Index of a randomly chosen element, or `None` when the slice is empty.
    pub fn pick_index<T>(&mut self, items: &[T]) -> Option<usize> {
        if items.is_empty() {
            return None;
        }
        Some(self.range_i64(0, items.len() as i64) as usize)
    }

    /// Random element clone, or `None` when the slice is empty.
    pub fn pick<T: Clone>(&mut self, items: &[T]) -> Option<T> {
        self.pick_index(items).map(|i| items[i].clone())
    }

    /// Fisher–Yates shuffle in place.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.range_i64(0, i as i64 + 1) as usize;
            items.swap(i, j);
        }
    }

    /// Weighted choice: `weights[i]` is the relative likelihood of `items[i]`.
    /// Returns `None` on empty input or when all weights are zero.
    pub fn weighted<'a, T>(&mut self, items: &'a [T], weights: &[u32]) -> Option<&'a T> {
        let total: u64 = weights.iter().map(|w| *w as u64).sum();
        if items.is_empty() || total == 0 {
            return None;
        }
        let mut target = self.next_u64() % total;
        for (item, w) in items.iter().zip(weights) {
            if target < *w as u64 {
                return Some(item);
            }
            target -= *w as u64;
        }
        items.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rand::new(42);
        let mut b = Rand::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rand::new(1);
        let mut b = Rand::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn ranges_respect_bounds() {
        let mut r = Rand::new(7);
        for _ in 0..10_000 {
            let v = r.range_i64(3, 9);
            assert!((3..9).contains(&v));
            let f = r.range_f64(-0.5, 0.5);
            assert!((-0.5..0.5).contains(&f));
        }
    }

    #[test]
    fn weighted_distribution_is_sane() {
        let items = [0u8, 1];
        let weights = [1u32, 3];
        let mut r = Rand::new(99);
        let mut counts = [0u32; 2];
        for _ in 0..40_000 {
            *match r.weighted(&items, &weights) {
                Some(0) => &mut counts[0],
                _ => &mut counts[1],
            } += 1;
        }
        // Expect roughly 25% / 75%; allow generous slack.
        assert!(counts[0] > 8_000 && counts[0] < 12_000, "{counts:?}");
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut v: Vec<u32> = (0..50).collect();
        let mut r = Rand::new(5);
        r.shuffle(&mut v);
        v.sort();
        assert_eq!(v, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn empty_inputs_are_handled() {
        let mut r = Rand::new(0);
        assert_eq!(r.pick::<u8>(&[]), None);
        assert_eq!(r.weighted::<u8>(&[], &[]), None);
    }
}
