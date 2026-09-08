//! The two arithmetic primitives every rule in `docs/kingdom.md` is written in.
//!
//! Both are integer-only and both round the way the original's C rounds, which
//! is the only reason they exist as named functions rather than as inline
//! expressions: a lockstep simulation cannot afford one call site to round a
//! percentage differently from another (`docs/netcode.md` §3).

/// `Pct(x, p) = x * p / 100` — the original's percentage helper, quoted by
/// name throughout `docs/kingdom.md` §4 and §5.
///
/// The intermediate is `i64` so a large population times a large percentage
/// cannot overflow, and the division truncates **toward zero**, which is what
/// C's `/` does and therefore what the original does. That matters for the
/// random-event modifiers, which are the only place `p` is ever negative:
/// `pct(-100, 7)` is `-7`, not `-8`.
///
/// ```
/// # use l2_kingdom::math::pct;
/// assert_eq!(pct(417, 15), 62);   // docs/kingdom.md §9, the births chain
/// assert_eq!(pct(417, 11), 45);   // ... and the deaths chain
/// assert_eq!(pct(-100, 7), -7);   // truncation toward zero, both signs
/// assert_eq!(pct(100, -7), -7);
/// ```
#[inline]
pub fn pct(x: i32, p: i32) -> i32 {
    ((x as i64 * p as i64) / 100) as i32
}

/// `PctOf(a, b) = a * 100 / b`, and **0 when `b` is 0** — `0x00404DC1`.
///
/// The inverse of [`pct`]: what percentage of `b` is `a`. The zero case is the
/// original's own guard rather than ours, and it is load-bearing — a county
/// with no cattle asks `PctOf(labour, 0)` for its herd staffing and gets 0
/// rather than a division by zero (`docs/kingdom.md` §13).
///
/// ```
/// # use l2_kingdom::math::pct_of;
/// assert_eq!(pct_of(218, 222), 98);   // county 1 of lastturn.sav, understaffed
/// assert_eq!(pct_of(323, 162), 199);  // county 2, one short of the cap
/// assert_eq!(pct_of(5, 0), 0);
/// ```
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
///
/// The herd's births and deaths are the only rules written in it, and they need
/// it: at the least crowded band a herd breeds at 1,400 per ten thousand a
/// season, which is 14% and could not survive being said in whole percent once
/// the staffing multiplier has been through it.
///
/// ```
/// # use l2_kingdom::math::per_myriad;
/// assert_eq!(per_myriad(74, 196), 1);      // county 1 of lastturn.sav: one calf
/// assert_eq!(per_myriad(74 * 100, 7), 5);  // ... and five losses
/// ```
#[inline]
pub fn per_myriad(x: i32, p: i32) -> i32 {
    ((x as i64 * p as i64) / 10_000) as i32
}

/// `DivCeil(a, b)` — used by the ration requirement and by the two
/// people-per-animal / people-per-sack conversions in `docs/kingdom.md` §4.3.
///
/// Only ever called with a non-negative `a` and a positive `b`; a negative `a`
/// would mean "a negative number of people to feed", which is a bug rather
/// than a quantity, so it clamps to zero instead of producing a negative
/// requirement.
///
/// ```
/// # use l2_kingdom::math::div_ceil;
/// assert_eq!(div_ceil(121, 10), 13);  // docs/kingdom.md §4.3, the reproduction
/// assert_eq!(div_ceil(120, 10), 12);
/// assert_eq!(div_ceil(0, 10), 0);
/// ```
///
/// # Panics
/// If `b` is zero.
#[inline]
pub fn div_ceil(a: i32, b: i32) -> i32 {
    assert!(b > 0, "div_ceil with a non-positive divisor {b}");
    if a <= 0 {
        0
    } else {
        ((a as i64 + b as i64 - 1) / b as i64) as i32
    }
}

/// Clamp, spelled out so the intent reads at the call site. `i32::clamp`
/// panics on an inverted range; this is only ever handed literal bounds.
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
        // The symmetry that makes an event modifier of -p the mirror of +p.
        for x in [1i32, 7, 99, 417, 1_000_000] {
            for p in [1i32, 3, 15, 20, 99, 137] {
                assert_eq!(pct(-x, p), -pct(x, p), "pct({}, {p})", -x);
                assert_eq!(pct(x, -p), -pct(x, p), "pct({x}, {})", -p);
            }
        }
    }

    #[test]
    fn pct_survives_an_intermediate_that_would_overflow_i32() {
        // The `i64` intermediate is the point of the function. `x * p`
        // overflows i32 long before `x * p / 100` does, and an overflowing i32
        // multiply panics in debug and wraps in release - two different answers
        // from the same inputs, which is the hazard docs/netcode.md errata 8
        // records.
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
