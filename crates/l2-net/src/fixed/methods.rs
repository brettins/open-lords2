#![allow(unused_imports)]
use super::*;

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

impl Fixed {
    pub const ZERO: Fixed = Fixed(0);
    pub const ONE: Fixed = Fixed(1 << FRAC_BITS);
    pub const HALF: Fixed = Fixed(1 << (FRAC_BITS - 1));
    pub const NEG_ONE: Fixed = Fixed(-(1 << FRAC_BITS));
    /// The smallest representable step, `1/65536`.
    pub const EPSILON: Fixed = Fixed(1);
    pub const MIN: Fixed = Fixed(i32::MIN);
    pub const MAX: Fixed = Fixed(i32::MAX);

    /// Wrap a raw scaled value. `Fixed::from_raw(65536) == Fixed::ONE`.
    pub const fn from_raw(raw: i32) -> Fixed {
        Fixed(raw)
    }

    /// The raw scaled value. This is what gets serialised, and it is
    /// the only representation any two machines need to agree on.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// An integer, saturating at the ends of the range.
    pub const fn from_int(n: i32) -> Fixed {
        if n > 32767 {
            Fixed::MAX
        } else if n < -32768 {
            Fixed::MIN
        } else {
            Fixed((n as i64 * SCALE) as i32)
        }
    }

    /// `numerator / denominator`, rounded toward zero.
    ///
    /// The intended way to write a fraction from a rule file: rules
    /// hold integers (`l2-mods` has no float type, on purpose), and
    /// `Fixed::from_ratio(3, 4)` is how a "75%" becomes a number the
    /// simulation can multiply by.
    ///
    /// Panics on a zero denominator.
    pub fn from_ratio(numerator: i32, denominator: i32) -> Fixed {
        assert!(denominator != 0, "Fixed::from_ratio with a zero denominator");
        Fixed(saturate(numerator as i64 * SCALE / denominator as i64))
    }

    /// Percent as a fraction: `percent(150) == 1.5`.
    ///
/// Exists because it is what the rule files contain —
    /// `l2-mods`'s seeded ruleset writes difficulty as
    /// `scale_percent = 116` — and because `from_ratio(p, 100)` at
    /// every call site is where a `100` eventually gets typed as `10`.
    pub fn percent(percent: i32) -> Fixed {
        Fixed::from_ratio(percent, 100)
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// `-1`, `0` or `1` as a plain integer.
    pub const fn signum_int(self) -> i32 {
        if self.0 > 0 {
            1
        } else if self.0 < 0 {
            -1
        } else {
            0
        }
    }

    /// Toward zero: `2.7 -> 2`, `-2.7 -> -2`.
    pub const fn trunc(self) -> i32 {
        (self.0 as i64 / SCALE) as i32
    }

    /// Toward negative infinity: `2.7 -> 2`, `-2.7 -> -3`.
    ///
    /// This is the one to use for "which tile is this position in",
    /// because tile -1 starts at -1.0, not at -1.99998.
    pub const fn floor(self) -> i32 {
        self.0 >> FRAC_BITS
    }

    /// Toward positive infinity.
    pub const fn ceil(self) -> i32 {
        ((self.0 as i64 + SCALE - 1) >> FRAC_BITS) as i32
    }

    /// Nearest, halves away from zero: `2.5 -> 3`, `-2.5 -> -3`.
    pub const fn round(self) -> i32 {
        let v = self.0 as i64;
        if v >= 0 {
            ((v + SCALE / 2) >> FRAC_BITS) as i32
        } else {
            -((-v + SCALE / 2) >> FRAC_BITS) as i32
        }
    }

    /// The fractional part, always in `0.0 .. 1.0` — `frac()` of
    /// `-2.25` is `0.75`, matching [`Fixed::floor`] so that
    /// `floor() + frac() == self` for every value.
    pub const fn frac(self) -> Fixed {
        Fixed(self.0 & (SCALE as i32 - 1))
    }

    /// Absolute value, saturating (`MIN.abs() == MAX`).
    pub const fn abs(self) -> Fixed {
        if self.0 == i32::MIN {
            Fixed::MAX
        } else if self.0 < 0 {
            Fixed(-self.0)
        } else {
            self
        }
    }

    pub fn min(self, other: Fixed) -> Fixed {
        if self.0 <= other.0 {
            self
        } else {
            other
        }
    }

    pub fn max(self, other: Fixed) -> Fixed {
        if self.0 >= other.0 {
            self
        } else {
            other
        }
    }

    pub fn clamp(self, low: Fixed, high: Fixed) -> Fixed {
        assert!(low <= high, "Fixed::clamp with an inverted range");
        self.max(low).min(high)
    }

    // --- checked ---------------------------------------------------

    pub const fn checked_add(self, other: Fixed) -> Option<Fixed> {
        match self.0.checked_add(other.0) {
            Some(v) => Some(Fixed(v)),
            None => None,
        }
    }

    pub const fn checked_sub(self, other: Fixed) -> Option<Fixed> {
        match self.0.checked_sub(other.0) {
            Some(v) => Some(Fixed(v)),
            None => None,
        }
    }

    pub const fn checked_neg(self) -> Option<Fixed> {
        match self.0.checked_neg() {
            Some(v) => Some(Fixed(v)),
            None => None,
        }
    }

    /// Truncating toward zero. `None` on overflow.
    pub const fn checked_mul(self, other: Fixed) -> Option<Fixed> {
        // i32 * i32 is at most 2^62, so the intermediate cannot
        // overflow i64 and the only failure is the narrowing.
        let wide = (self.0 as i64 * other.0 as i64) / SCALE;
        narrow(wide)
    }

    /// Truncating toward zero. `None` on overflow **or** division by
    /// zero — the caller who wants to tell them apart should check the
    /// divisor, which is cheaper than the two-level result it would
    /// take to report it.
    pub const fn checked_div(self, other: Fixed) -> Option<Fixed> {
        if other.0 == 0 {
            return None;
        }
        narrow((self.0 as i64 * SCALE) / other.0 as i64)
    }

    // --- saturating ------------------------------------------------

    pub const fn saturating_add(self, other: Fixed) -> Fixed {
        Fixed(self.0.saturating_add(other.0))
    }

    pub const fn saturating_sub(self, other: Fixed) -> Fixed {
        Fixed(self.0.saturating_sub(other.0))
    }

    pub const fn saturating_neg(self) -> Fixed {
        Fixed(self.0.saturating_neg())
    }

    /// Truncating toward zero, saturating on overflow.
    pub const fn mul_trunc(self, other: Fixed) -> Fixed {
        Fixed(saturate((self.0 as i64 * other.0 as i64) / SCALE))
    }

    /// Truncating toward zero, saturating on overflow. Panics on a zero
    /// divisor.
    pub const fn div_trunc(self, other: Fixed) -> Fixed {
        assert!(other.0 != 0, "Fixed division by zero");
        Fixed(saturate((self.0 as i64 * SCALE) / other.0 as i64))
    }

    /// Nearest, halves away from zero, saturating on overflow.
    pub const fn mul_round(self, other: Fixed) -> Fixed {
        Fixed(saturate(div_round(self.0 as i64 * other.0 as i64, SCALE)))
    }

    /// Nearest, halves away from zero, saturating on overflow. Panics
    /// on a zero divisor.
    pub const fn div_round(self, other: Fixed) -> Fixed {
        assert!(other.0 != 0, "Fixed division by zero");
        Fixed(saturate(div_round(self.0 as i64 * SCALE, other.0 as i64)))
    }

    /// Multiply by a plain integer. Exact for any result in range, and
    /// it avoids the `from_int` round trip at the call site.
    pub const fn scale_int(self, n: i32) -> Fixed {
        Fixed(saturate(self.0 as i64 * n as i64))
    }

    /// `self * numerator / denominator`, with the multiply done in 64
    /// bits before the divide.
    ///
/// Worth having as one operation: `x.mul(a).div(b)`
    /// rounds twice and can saturate in the middle, and "apply a
    /// percentage" is the single most common fractional operation in a
    /// game's rules. Truncates toward zero. Panics on a zero
    /// denominator.
    pub const fn mul_ratio(self, numerator: i32, denominator: i32) -> Fixed {
        assert!(denominator != 0, "Fixed::mul_ratio with a zero denominator");
        Fixed(saturate(self.0 as i64 * numerator as i64 / denominator as i64))
    }

    // --- roots -----------------------------------------------------

    /// Square root, truncated. Panics on a negative input, which is a
/// programming error.
    ///
    /// Integer bit-by-bit extraction, so it is exact to the last
    /// representable step on every machine. `f32::sqrt` would be
/// faster and is what D-1 forbids: it is correctly
    /// rounded on hardware that implements IEEE-754 square root, and
    /// "hardware that implements it" is not a promise the language
    /// makes.
    pub fn sqrt(self) -> Fixed {
        assert!(self.0 >= 0, "Fixed::sqrt of a negative value");
        // sqrt(raw / 2^16) * 2^16 == sqrt(raw * 2^16).
        Fixed(isqrt((self.0 as u64) << FRAC_BITS) as i32)
    }

    /// `sqrt(x*x + y*y)`, computed without an intermediate that can
    /// overflow: the squares are summed in Q32.32 in an `i64` and the
    /// root brings them back to Q16.16 in one step.
    ///
    /// This is the reason [`Fixed::sqrt`] exists — distance between two
/// points is the one place a battle simulation wants a
    /// root, and doing it as `x.mul(x).add(y.mul(y)).sqrt()` would
    /// saturate for any separation over 181 tiles and round three times
    /// on the way.
    pub fn hypot(x: Fixed, y: Fixed) -> Fixed {
        let xx = x.0 as i64 * x.0 as i64;
        let yy = y.0 as i64 * y.0 as i64;
        // Each square is at most 2^62, so the sum can overflow i64 only
        // if both operands are near the ends of the range — which is
        // already outside anything this game represents. Saturate the
// sum.
        let sum = xx.checked_add(yy).unwrap_or(i64::MAX);
        Fixed(saturate(isqrt(sum as u64) as i64))
    }
}

