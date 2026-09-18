
#[inline]
pub fn pct(x: i32, p: i32) -> i32 {
    ((x as i64 * p as i64) / 100) as i32
}

/// `PctOf(a, b) = a * 100 / b`, and **0 when `b` is 0** — `0x00404DC1`.
#[inline]
pub fn pct_of(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        ((a as i64 * 100) / b as i64) as i32
    }
}

/// `x * p / 10000` — `0x00404D96`, which is [`pct`] with two more decimal
/// places.
#[inline]
pub fn per_myriad(x: i32, p: i32) -> i32 {
    ((x as i64 * p as i64) / 10_000) as i32
}

#[inline]
pub fn div_ceil(a: i32, b: i32) -> i32 {
    assert!(b > 0, "div_ceil with a non-positive divisor {b}");
    if a <= 0 {
        0
    } else {
        ((a as i64 + b as i64 - 1) / b as i64) as i32
    }
}

#[inline]
pub fn clamp(x: i32, low: i32, high: i32) -> i32 {
    if x < low {
        low
    } else if x > high {
        high
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct_truncates_toward_zero_in_both_directions() {
        for x in [1i32, 7, 99, 417, 1_000_000] {
            for p in [1i32, 3, 15, 20, 99, 137] {
                assert_eq!(pct(-x, p), -pct(x, p), "pct({}, {p})", -x);
                assert_eq!(pct(x, -p), -pct(x, p), "pct({x}, {})", -p);
            }
        }
    }

    #[test]
    fn pct_survives_an_intermediate_that_would_overflow_i32() {
        assert_eq!(pct(i32::MAX, 100), i32::MAX);
        assert_eq!(pct(100_000_000, 300), 300_000_000);
        assert_eq!(pct(-100_000_000, 300), -300_000_000);
    }

    #[test]
    fn div_ceil_rounds_up_only_when_there_is_a_remainder() {
        for b in 1..=12 {
            for a in 0..=60 {
                let d = div_ceil(a, b);
                assert!(d * b >= a, "div_ceil({a}, {b}) = {d} is short");
                assert!((d - 1) * b < a || a == 0, "div_ceil({a}, {b}) = {d} overshoots");
            }
        }
    }
}
