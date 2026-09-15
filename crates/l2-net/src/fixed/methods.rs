#![allow(unused_imports)]
use super::*;

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

impl Fixed {
    pub const ZERO: Fixed = Fixed(0);
    pub const ONE: Fixed = Fixed(1 << FRAC_BITS);
    pub const HALF: Fixed = Fixed(1 << (FRAC_BITS - 1));
    pub const NEG_ONE: Fixed = Fixed(-(1 << FRAC_BITS));
    pub const EPSILON: Fixed = Fixed(1);
    pub const MIN: Fixed = Fixed(i32::MIN);
    pub const MAX: Fixed = Fixed(i32::MAX);

    pub const fn from_raw(raw: i32) -> Fixed {
        Fixed(raw)
    }

    pub const fn raw(self) -> i32 {
        self.0
    }

    pub const fn from_int(n: i32) -> Fixed {
        if n > 32767 {
            Fixed::MAX
        } else if n < -32768 {
            Fixed::MIN
        } else {
            Fixed((n as i64 * SCALE) as i32)
        }
    }

    pub fn from_ratio(numerator: i32, denominator: i32) -> Fixed {
        assert!(denominator != 0, "Fixed::from_ratio with a zero denominator");
        Fixed(saturate(numerator as i64 * SCALE / denominator as i64))
    }

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

    pub const fn signum_int(self) -> i32 {
        if self.0 > 0 {
            1
        } else if self.0 < 0 {
            -1
        } else {
            0
        }
    }

    pub const fn trunc(self) -> i32 {
        (self.0 as i64 / SCALE) as i32
    }

    pub const fn floor(self) -> i32 {
        self.0 >> FRAC_BITS
    }

    pub const fn ceil(self) -> i32 {
        ((self.0 as i64 + SCALE - 1) >> FRAC_BITS) as i32
    }

    pub const fn round(self) -> i32 {
        let v = self.0 as i64;
        if v >= 0 {
            ((v + SCALE / 2) >> FRAC_BITS) as i32
        } else {
            -((-v + SCALE / 2) >> FRAC_BITS) as i32
        }
    }

    pub const fn frac(self) -> Fixed {
        Fixed(self.0 & (SCALE as i32 - 1))
    }

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

    pub const fn checked_mul(self, other: Fixed) -> Option<Fixed> {
        let wide = (self.0 as i64 * other.0 as i64) / SCALE;
        narrow(wide)
    }

    pub const fn checked_div(self, other: Fixed) -> Option<Fixed> {
        if other.0 == 0 {
            return None;
        }
        narrow((self.0 as i64 * SCALE) / other.0 as i64)
    }


    pub const fn saturating_add(self, other: Fixed) -> Fixed {
        Fixed(self.0.saturating_add(other.0))
    }

    pub const fn saturating_sub(self, other: Fixed) -> Fixed {
        Fixed(self.0.saturating_sub(other.0))
    }

    pub const fn saturating_neg(self) -> Fixed {
        Fixed(self.0.saturating_neg())
    }

    pub const fn mul_trunc(self, other: Fixed) -> Fixed {
        Fixed(saturate((self.0 as i64 * other.0 as i64) / SCALE))
    }

    pub const fn div_trunc(self, other: Fixed) -> Fixed {
        assert!(other.0 != 0, "Fixed division by zero");
        Fixed(saturate((self.0 as i64 * SCALE) / other.0 as i64))
    }

    pub const fn mul_round(self, other: Fixed) -> Fixed {
        Fixed(saturate(div_round(self.0 as i64 * other.0 as i64, SCALE)))
    }

    pub const fn div_round(self, other: Fixed) -> Fixed {
        assert!(other.0 != 0, "Fixed division by zero");
        Fixed(saturate(div_round(self.0 as i64 * SCALE, other.0 as i64)))
    }

    pub const fn scale_int(self, n: i32) -> Fixed {
        Fixed(saturate(self.0 as i64 * n as i64))
    }

    pub const fn mul_ratio(self, numerator: i32, denominator: i32) -> Fixed {
        assert!(denominator != 0, "Fixed::mul_ratio with a zero denominator");
        Fixed(saturate(self.0 as i64 * numerator as i64 / denominator as i64))
    }


    pub fn sqrt(self) -> Fixed {
        assert!(self.0 >= 0, "Fixed::sqrt of a negative value");
        Fixed(isqrt((self.0 as u64) << FRAC_BITS) as i32)
    }

    pub fn hypot(x: Fixed, y: Fixed) -> Fixed {
        let xx = x.0 as i64 * x.0 as i64;
        let yy = y.0 as i64 * y.0 as i64;
        let sum = xx.checked_add(yy).unwrap_or(i64::MAX);
        Fixed(saturate(isqrt(sum as u64) as i64))
    }
}

