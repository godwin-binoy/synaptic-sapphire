//! Exact rational musical durations expressed as integer beats plus a fraction.

use super::Ticks;

/// Resolution of the internal clock: ticks per quarter note.
///
/// `1920` divides every useful subdivision down to 1/64 notes times tuplets
/// up to 5/7/11, which keeps all generated grids integral.
pub const PPQ: u32 = 1920;

/// A duration given as whole beats plus an exact fraction of a beat.
///
/// Using `(whole, numerator, denominator)` instead of floating point means a
/// bar grid such as "sixteenth swing" is represented precisely and always maps
/// back to an integer number of [`Ticks`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts {
    /// Whole beats.
    pub whole: i64,
    /// Fraction numerator, `0 <= num < den`.
    pub num: u32,
    /// Fraction denominator (a power of two or a tuplet divisor). Never zero.
    pub den: u32,
}

impl Parts {
    /// Build a part from beats and a reduced-or-not fraction.
    ///
    /// # Panics
    /// If `den == 0`.
    pub fn new(beats: i64, num: u32, den: u32) -> Self {
        assert!(den != 0, "Parts denominator must be non-zero");
        Parts { whole: beats, num, den }
    }

    /// Exactly `n` whole beats.
    pub fn beats(n: i64) -> Self {
        Parts { whole: n, num: 0, den: 1 }
    }

    /// `1 / den` of a single beat, e.g. `Parts::division(4)` is a quarter note.
    pub fn division(den: u32) -> Self {
        Parts { whole: 0, num: 1, den }
    }

    /// Convert to an integer tick count.
    ///
    /// Every grid used by the generator divides `PPQ`, so the result is exact;
    /// a non-divisible fraction is rounded to the nearest tick rather than
    /// silently truncated.
    pub fn to_ticks(self) -> Ticks {
        let whole = self.whole * PPQ as i64;
        let frac = (PPQ as i64 * self.num as i64) as f64 / self.den as f64;
        Ticks(whole + frac.round() as i64)
    }

    /// Tick offset of the `i`-th step when a span of `total` beats is divided
    /// into `steps` equal parts. Provided so voice code can slice bars without
    /// ever touching floats.
    pub fn step_ticks(total_beats: i64, steps: u32, i: u32) -> Ticks {
        debug_assert!(steps > 0);
        let span = Parts::beats(total_beats).to_ticks().as_i64();
        Ticks(span * i as i64 / steps as i64)
    }

    /// Human-readable form used in error messages and CLI output.
    pub fn describe(self) -> String {
        if self.num == 0 {
            format!("{} beat(s)", self.whole)
        } else {
            format!("{} + {}/{} beat(s)", self.whole, self.num, self.den)
        }
    }
}

impl Default for Parts {
    fn default() -> Self {
        Parts::beats(1)
    }
}

impl std::ops::Add for Parts {
    type Output = Parts;
    fn add(self, rhs: Parts) -> Parts {
        let den = lcm(self.den, rhs.den);
        let num = self.num * (den / self.den) + rhs.num * (den / rhs.den);
        let carry = (num / den) as i64;
        Parts { whole: self.whole + rhs.whole + carry, num: num % den, den }
    }
}

/// Smallest common multiple, used to add [`Parts`] exactly.
fn lcm(a: u32, b: u32) -> u32 {
    if a == 0 || b == 0 {
        return 0;
    }
    a / gcd(a, b) * b
}

fn gcd(a: u32, b: u32) -> u32 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divisions_are_integral() {
        for den in [1u32, 2, 3, 4, 6, 8, 12, 16, 24, 32] {
            let t = Parts::division(den).to_ticks();
            assert!(t.as_i64() > 0, "division {den} collapsed to zero ticks");
            assert_eq!(PPQ as i64 % t.as_i64(), 0, "division {den} not integral");
        }
    }

    #[test]
    fn adding_parts_is_exact() {
        // PPQ is beats-per-minute-style quarter notes: a beat is a quarter note,
        // so 1/4 of a beat + 1/8 of a beat = 3/8 of a beat = 720 ticks at 1920 PPQ.
        let sum = Parts::division(4) + Parts::division(8);
        assert_eq!(sum.to_ticks(), Parts::new(0, 3, 8).to_ticks());
        assert_eq!(sum.to_ticks().as_i64(), 720);
    }

    #[test]
    fn step_ticks_slices_a_span_evenly() {
        // 4 beats = 7680 ticks split into 16 steps => 480 ticks per step.
        for i in 0..16u32 {
            let a = Parts::step_ticks(4, 16, i);
            assert_eq!(a.as_i64(), i as i64 * 480);
        }
    }

    #[test]
    fn describe_is_informative() {
        assert_eq!(Parts::beats(2).describe(), "2 beat(s)");
        assert_eq!(Parts::new(1, 1, 3).describe(), "1 + 1/3 beat(s)");
    }
}
