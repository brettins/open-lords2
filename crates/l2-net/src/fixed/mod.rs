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
//! (`docs/formats/skr.md`) and the world map 64×64
//! uses seven of the sixteen integer bits; the remaining nine are
//! headroom for intermediate results. A value that overflows Q16.16
//! here is a bug, not a large number, and [`Fixed`] is built to make
//! that visible.
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
//! has to go
//! saturate.
//!
//! Saturation wins on the failure shape. A wrap turns a large positive
//! into a large negative, which is a unit teleporting to the far corner
//! of the map — plausible-looking, hard to trace. A panic is honest but
//! takes the game down for something that may be one bad frame in a
//! cutscene. Saturation pins the value at the end of the range, where
//! it stays visibly wrong and stays *there*
//! matches on both peers because saturation is
//! as wrapping.
//!
//! Code that wants to know uses
//! [`Fixed::checked_add`] and friends, which return `None` and cost
//! nothing extra — the saturating versions are written in terms of
//! them.
//!
//! Division by zero is the exception: `/` panics.
//! saturate to that is not a lie
//! peers panic on the same tick, which is a bug report.
//! desync. [`Fixed::checked_div`] is there for callers who would rather
//! branch.

mod methods;
pub use methods::*;

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
/// Done on magnitudes with the sign reapplied.
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
/// Written with integers. A `Display` that formatted through
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

