//! Q16.16 fixed-point arithmetic.
//!
//! `docs/netcode.md` D-1 forbids floating point in the simulation and
//! D-2 asks for "a single `Fixed` type — Q16.16 in an `i32`, with `i64`
//! intermediates for multiply and divide — in a leaf crate", with every
//! operation explicit about rounding. This is that type.
//!
//! # The representation
//!
//! One `i32` holding a value scaled by 65536. Sixteen integer bits and
//! sixteen fractional bits, so the range is roughly `-32768.0 ..=
//! 32767.99998` in steps of `1/65536`.
//!
//! That range is not tight for this game. The battlefield is 80×80
//! (`docs/formats/skr.md`) and the world map 64×64, so a coordinate
//! uses seven of the sixteen integer bits; the remaining nine are
//! headroom for intermediate results. A value that overflows Q16.16
//! here is a bug, not a large number, and [`Fixed`] is built to make
//! that visible rather than to accommodate it.
//!
//! # Rounding
//!
//! Every lossy operation names its rounding in its own name.
//!
//! * [`Fixed::mul_trunc`] and [`Fixed::div_trunc`] - and the `*` and
//!   `/` operators, which delegate to them - **truncate toward zero**.
//! * [`Fixed::mul_round`] and [`Fixed::div_round`] round to nearest,
//!   **halves away from zero**.
//! * [`Fixed::floor`], [`Fixed::ceil`], [`Fixed::trunc`] and
//!   [`Fixed::round`] do what they say, and `round` is also
//!   half-away-from-zero.
//!
//! Toward zero is the default because it is *symmetric*: `(-a) * b ==
//! -(a * b)` for every pair. The obvious alternative — the arithmetic
//! shift `(a * b) >> 16`, which is one instruction cheaper — floors,
//! so it rounds negative results away from zero and positive results
//! toward it. In a simulation that is a directional bias: a unit
//! walking left loses a different amount per tick than the same unit
//! walking right, and after ten thousand ticks the two are measurably
//! not mirror images. That is the sort of asymmetry nobody looks for,
//! because nobody suspects the rounding mode.
//!
//! Banker's rounding is not offered. It is the right default for
//! accumulating money and the wrong one here: it makes the result of
//! `a * b` depend on the parity of a bit nobody is thinking about, and
//! the errors it balances out are errors this game does not accumulate.
//!
//! # Overflow
//!
//! The operators **saturate**, and this is the decision in this file
//! most worth arguing about.
//!
//! Rust's built-in integer operators panic on overflow in debug and
//! wrap in release. Either behaviour alone would be fine; *having both*
//! is not, because it means a debug build and a release build of the
//! same simulation compute different numbers from the same inputs. Two
//! peers must agree, and "are you running a release build?" cannot be
//! part of the determinism contract. So the profile-dependent behaviour
//! has to go, and the choice is between always-panic and always-
//! saturate.
//!
//! Saturation wins on the failure shape. A wrap turns a large positive
//! into a large negative, which is a unit teleporting to the far corner
//! of the map — plausible-looking, hard to trace. A panic is honest but
//! takes the game down for something that may be one bad frame in a
//! cutscene. Saturation pins the value at the end of the range, where
//! it stays visibly wrong and stays *there*, and the checksum still
//! matches on both peers because saturation is exactly as deterministic
//! as wrapping.
//!
//! Code that wants to know rather than to continue uses
//! [`Fixed::checked_add`] and friends, which return `None` and cost
//! nothing extra — the saturating versions are written in terms of
//! them.
//!
//! Division by zero is the exception: `/` panics. There is no value to
//! saturate to that is not a lie, and a panic is deterministic — both
//! peers panic on the same tick, which is a bug report rather than a
//! desync. [`Fixed::checked_div`] is there for callers who would rather
//! branch.

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// The number of fractional bits. Changing it changes every saved
/// replay and every checksum in existence.
pub const FRAC_BITS: u32 = 16;

/// `1.0` in raw units.
const SCALE: i64 = 1 << FRAC_BITS;

/// A signed Q16.16 fixed-point number.
///
/// `Ord` and `Eq` are derived and are exactly the numeric ordering,
/// because the two's-complement representation is monotonic in the
/// value. That matters more than it looks: D-7 requires total orderings
/// inside the simulation, and a comparison operator that agreed with
/// `<` only sometimes — which is what `f32`'s `PartialOrd` is, thanks
/// to NaN — is one of the reasons floats are banned in the first place.
///
/// `Hash` is deliberately **not** implemented. `docs/netcode.md` §6 is
/// explicit that the `Hash` trait must never reach a checksum; feeding
/// a `Fixed` to [`Canonical`](crate::Canonical) is the supported way,
/// and it writes four little-endian bytes on every platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Fixed(i32);

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
    /// Exists because it is what the rule files actually contain —
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
    /// Worth having as one operation rather than two: `x.mul(a).div(b)`
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
    /// programming error rather than a game state.
    ///
    /// Integer bit-by-bit extraction, so it is exact to the last
    /// representable step on every machine. `f32::sqrt` would be
    /// faster and is precisely what D-1 forbids: it is correctly
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
    /// points is the one place a battle simulation genuinely wants a
    /// root, and doing it as `x.mul(x).add(y.mul(y)).sqrt()` would
    /// saturate for any separation over 181 tiles and round three times
    /// on the way.
    pub fn hypot(x: Fixed, y: Fixed) -> Fixed {
        let xx = x.0 as i64 * x.0 as i64;
        let yy = y.0 as i64 * y.0 as i64;
        // Each square is at most 2^62, so the sum can overflow i64 only
        // if both operands are near the ends of the range — which is
        // already outside anything this game represents. Saturate the
        // sum rather than wrap.
        let sum = xx.checked_add(yy).unwrap_or(i64::MAX);
        Fixed(saturate(isqrt(sum as u64) as i64))
    }
}

/// Narrow an i64 to Q16.16, clamping.
const fn saturate(wide: i64) -> i32 {
    if wide > i32::MAX as i64 {
        i32::MAX
    } else if wide < i32::MIN as i64 {
        i32::MIN
    } else {
        wide as i32
    }
}

const fn narrow(wide: i64) -> Option<Fixed> {
    if wide > i32::MAX as i64 || wide < i32::MIN as i64 {
        None
    } else {
        Some(Fixed(wide as i32))
    }
}

/// `numerator / denominator`, nearest, halves away from zero.
///
/// Done on magnitudes with the sign reapplied, rather than by nudging
/// the numerator in place. The in-place version needs four sign cases
/// and gets one of them wrong the first time it is written — as this
/// one did, rounding `-2.5` to `-2`.
const fn div_round(numerator: i64, denominator: i64) -> i64 {
    let negative = (numerator < 0) != (denominator < 0);
    // Both operands here derive from `i32`s, so neither magnitude can
    // be `i64::MIN` and `unsigned_abs` cannot lose anything.
    let n = numerator.unsigned_abs();
    let d = denominator.unsigned_abs();
    let q = ((n + d / 2) / d) as i64;
    if negative {
        -q
    } else {
        q
    }
}

/// Integer square root of a `u64`, by bit-by-bit restoring extraction.
///
/// Newton's method would converge faster and would need a division per
/// iteration and a seed; this needs neither and its iteration count is
/// fixed, which makes it constant-time in the input and impossible to
/// get subtly wrong near a boundary.
fn isqrt(value: u64) -> u64 {
    if value == 0 {
        return 0;
    }
    let mut remainder = value;
    let mut result: u64 = 0;
    // The largest power of four not exceeding `value`.
    let mut bit: u64 = 1u64 << ((63 - value.leading_zeros() as u64) & !1);
    while bit != 0 {
        if remainder >= result + bit {
            remainder -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    result
}

impl Add for Fixed {
    type Output = Fixed;
    fn add(self, other: Fixed) -> Fixed {
        self.saturating_add(other)
    }
}

impl Sub for Fixed {
    type Output = Fixed;
    fn sub(self, other: Fixed) -> Fixed {
        self.saturating_sub(other)
    }
}

impl Mul for Fixed {
    type Output = Fixed;
    fn mul(self, other: Fixed) -> Fixed {
        self.mul_trunc(other)
    }
}

impl Div for Fixed {
    type Output = Fixed;
    fn div(self, other: Fixed) -> Fixed {
        self.div_trunc(other)
    }
}

impl Neg for Fixed {
    type Output = Fixed;
    fn neg(self) -> Fixed {
        self.saturating_neg()
    }
}

impl AddAssign for Fixed {
    fn add_assign(&mut self, other: Fixed) {
        *self = *self + other;
    }
}

impl SubAssign for Fixed {
    fn sub_assign(&mut self, other: Fixed) {
        *self = *self - other;
    }
}

impl MulAssign for Fixed {
    fn mul_assign(&mut self, other: Fixed) {
        *self = *self * other;
    }
}

impl DivAssign for Fixed {
    fn div_assign(&mut self, other: Fixed) {
        *self = *self / other;
    }
}

impl From<i16> for Fixed {
    fn from(n: i16) -> Fixed {
        Fixed::from_int(n as i32)
    }
}

impl From<u8> for Fixed {
    fn from(n: u8) -> Fixed {
        Fixed::from_int(n as i32)
    }
}

/// Five fractional digits, trailing zeros trimmed. `1.5` prints as
/// `1.5`, not `1.50000`, and `Fixed::EPSILON` prints as `0.00002` —
/// which is a truncation of `0.0000152…`, because five digits is the
/// most that ever distinguishes two Q16.16 values.
///
/// Written with integers, of course. A `Display` that formatted through
/// `f64` would be correct here and would also be the first `f64` in the
/// simulation crate's call graph, which is exactly the sort of thing
/// the CI grep in `docs/netcode.md` is meant to catch.
impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let negative = self.0 < 0;
        let magnitude = (self.0 as i64).unsigned_abs();
        let whole = magnitude >> FRAC_BITS;
        let frac = magnitude & (SCALE as u64 - 1);
        // 5 digits: 65536 distinct fractions need ceil(log10(65536)) = 5.
        let digits = (frac * 100_000) >> FRAC_BITS;
        if negative {
            write!(f, "-")?;
        }
        if digits == 0 {
            write!(f, "{whole}")
        } else {
            let mut text = format!("{digits:05}");
            while text.ends_with('0') {
                text.pop();
            }
            write!(f, "{whole}.{text}")
        }
    }
}
