
mod methods;
pub use methods::*;

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub const FRAC_BITS: u32 = 16;

const SCALE: i64 = 1 << FRAC_BITS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Fixed(i32);

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

const fn div_round(numerator: i64, denominator: i64) -> i64 {
    let negative = (numerator < 0) != (denominator < 0);
    let n = numerator.unsigned_abs();
    let d = denominator.unsigned_abs();
    let q = ((n + d / 2) / d) as i64;
    if negative {
        -q
    } else {
        q
    }
}

fn isqrt(value: u64) -> u64 {
    if value == 0 {
        return 0;
    }
    let mut remainder = value;
    let mut result: u64 = 0;
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

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let negative = self.0 < 0;
        let magnitude = (self.0 as i64).unsigned_abs();
        let whole = magnitude >> FRAC_BITS;
        let frac = magnitude & (SCALE as u64 - 1);
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

