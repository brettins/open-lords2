//! Q16.16 arithmetic, at the boundaries and around zero.
//!
//! Two things are being defended here.
//!
//! **Rounding.** `docs/netcode.md` D-2 requires every operation to be
//! explicit about it, and the reason is not pedantry: a rounding rule
//! that is asymmetric about zero is a directional bias in a
//! simulation, and a directional bias is invisible until someone
//! notices that units drift left over a long battle. The property
//! tests below check symmetry as a property rather than at a handful of
//! points.
//!
//! **Overflow.** `Fixed`'s range is deliberately much wider than the
//! game needs, so every overflow is a bug — but a bug that must
//! *behave identically on both peers*, in both debug and release
//! builds. That last part is why the operators saturate rather than
//! using Rust's built-in behaviour, which panics in debug and wraps in
//! release. Two peers on different profiles would otherwise compute
//! different numbers, and no test that runs one profile at a time would
//! ever see it.

use l2_net::Fixed;

fn f(n: i32) -> Fixed {
    Fixed::from_int(n)
}

// --- construction and exact values -----------------------------------

#[test]
fn the_scale_is_65536() {
    assert_eq!(Fixed::ONE.raw(), 65_536);
    assert_eq!(Fixed::HALF.raw(), 32_768);
    assert_eq!(Fixed::EPSILON.raw(), 1);
    assert_eq!(f(7).raw(), 7 * 65_536);
    assert_eq!(f(-7).raw(), -7 * 65_536);
}

#[test]
fn from_int_saturates_rather_than_wrapping() {
    assert_eq!(f(32_767), Fixed::from_raw(32_767 * 65_536));
    assert_eq!(f(32_768), Fixed::MAX);
    assert_eq!(f(100_000), Fixed::MAX);
    assert_eq!(f(-32_768), Fixed::MIN);
    assert_eq!(f(-32_769), Fixed::MIN);
    assert_eq!(f(i32::MIN), Fixed::MIN);
}

#[test]
fn from_ratio_and_percent() {
    assert_eq!(Fixed::from_ratio(1, 2), Fixed::HALF);
    assert_eq!(Fixed::from_ratio(3, 4).raw(), 49_152);
    assert_eq!(Fixed::from_ratio(-1, 2), -Fixed::HALF);
    assert_eq!(Fixed::from_ratio(1, -2), -Fixed::HALF);
    assert_eq!(Fixed::percent(100), Fixed::ONE);
    assert_eq!(Fixed::percent(150), Fixed::ONE + Fixed::HALF);
    // The difficulty scales from l2-mods' seeded ruleset.
    assert_eq!(Fixed::percent(116).raw(), 76_021);
    assert_eq!(Fixed::percent(84).raw(), 55_050);
}

#[test]
#[should_panic(expected = "zero denominator")]
fn from_ratio_by_zero_panics() {
    Fixed::from_ratio(1, 0);
}

// --- conversion to integers ------------------------------------------

#[test]
fn trunc_goes_toward_zero() {
    assert_eq!(Fixed::from_ratio(27, 10).trunc(), 2);
    assert_eq!(Fixed::from_ratio(-27, 10).trunc(), -2);
    assert_eq!(Fixed::from_ratio(-1, 65_536).trunc(), 0);
}

#[test]
fn floor_goes_toward_negative_infinity() {
    assert_eq!(Fixed::from_ratio(27, 10).floor(), 2);
    assert_eq!(Fixed::from_ratio(-27, 10).floor(), -3);
    assert_eq!(f(-3).floor(), -3);
    // The reason floor exists: tile -1 starts at -1.0.
    assert_eq!((-Fixed::EPSILON).floor(), -1);
}

#[test]
fn ceil_goes_toward_positive_infinity() {
    assert_eq!(Fixed::from_ratio(27, 10).ceil(), 3);
    assert_eq!(Fixed::from_ratio(-27, 10).ceil(), -2);
    assert_eq!(f(3).ceil(), 3);
    assert_eq!(Fixed::EPSILON.ceil(), 1);
}

#[test]
fn round_takes_halves_away_from_zero() {
    assert_eq!((f(2) + Fixed::HALF).round(), 3);
    assert_eq!((f(-2) - Fixed::HALF).round(), -3);
    assert_eq!((f(3) + Fixed::HALF).round(), 4);
    assert_eq!(Fixed::from_ratio(24, 10).round(), 2);
    assert_eq!(Fixed::from_ratio(-24, 10).round(), -2);
    assert_eq!(Fixed::from_ratio(26, 10).round(), 3);
    assert_eq!(Fixed::from_ratio(-26, 10).round(), -3);
}

#[test]
fn rounding_at_the_extremes_does_not_overflow() {
    assert_eq!(Fixed::MAX.trunc(), 32_767);
    assert_eq!(Fixed::MAX.floor(), 32_767);
    assert_eq!(Fixed::MAX.ceil(), 32_768);
    assert_eq!(Fixed::MAX.round(), 32_768);
    assert_eq!(Fixed::MIN.trunc(), -32_768);
    assert_eq!(Fixed::MIN.floor(), -32_768);
    assert_eq!(Fixed::MIN.ceil(), -32_768);
    assert_eq!(Fixed::MIN.round(), -32_768);
}

#[test]
fn floor_plus_frac_reconstructs_the_value() {
    for raw in [0i32, 1, -1, 65_535, -65_535, 100_000, -100_000, i32::MAX, i32::MIN] {
        let v = Fixed::from_raw(raw);
        let rebuilt = f(v.floor()) + v.frac();
        // f(v.floor()) saturates at the extremes, so only check where
        // the integer part is representable.
        if v.floor().abs() < 32_768 {
            assert_eq!(rebuilt, v, "floor + frac lost {raw}");
        }
        assert!(v.frac() >= Fixed::ZERO && v.frac() < Fixed::ONE);
    }
}

// --- arithmetic -------------------------------------------------------

#[test]
fn multiplication_truncates_toward_zero() {
    // 1/3 * 3 loses the last bit rather than gaining one.
    let third = Fixed::from_ratio(1, 3);
    assert_eq!(third.raw(), 21_845);
    assert_eq!((third * f(3)).raw(), 65_535);
    assert_eq!(((-third) * f(3)).raw(), -65_535);
}

/// The property the whole rounding choice is about. An asymmetric rule
/// makes a unit walking left lose a different amount per tick from one
/// walking right.
#[test]
fn multiplication_and_division_are_symmetric_about_zero() {
    let mut rng = l2_net::Pcg32::from_seed(0x5eed);
    for _ in 0..20_000 {
        let a = Fixed::from_raw(rng.range(-1_000_000, 1_000_000));
        let b = Fixed::from_raw(rng.range(-1_000_000, 1_000_000));
        assert_eq!((-a).mul_trunc(b), -a.mul_trunc(b), "mul asymmetric at {a} * {b}");
        assert_eq!(a.mul_trunc(-b), -a.mul_trunc(b));
        if !b.is_zero() {
            assert_eq!((-a).div_trunc(b), -a.div_trunc(b), "div asymmetric at {a} / {b}");
            assert_eq!(a.div_trunc(-b), -a.div_trunc(b));
        }
        assert_eq!((-a).mul_round(b), -a.mul_round(b));
    }
}

#[test]
fn mul_round_goes_to_nearest_halves_away_from_zero() {
    // 1.5 * 1.0 has no rounding; pick values whose product lands
    // exactly on a half-step.
    let three_halves = Fixed::from_raw(98_304); // 1.5
    let one_third = Fixed::from_ratio(1, 3);
    assert_eq!(three_halves.mul_round(one_third).raw(), 32_768);
    assert_eq!(three_halves.mul_trunc(one_third).raw(), 32_767);
    assert_eq!((-three_halves).mul_round(one_third).raw(), -32_768);
    assert_eq!((-three_halves).mul_trunc(one_third).raw(), -32_767);
}

#[test]
fn div_round_goes_to_nearest_halves_away_from_zero() {
    // 1 / 3 = 0.333…; the exact value is 21845.333 raw units.
    assert_eq!(Fixed::ONE.div_trunc(f(3)).raw(), 21_845);
    assert_eq!(Fixed::ONE.div_round(f(3)).raw(), 21_845);
    // 1 / 6 = 10922.666 raw units, which rounds up.
    assert_eq!(Fixed::ONE.div_trunc(f(6)).raw(), 10_922);
    assert_eq!(Fixed::ONE.div_round(f(6)).raw(), 10_923);
    assert_eq!((-Fixed::ONE).div_round(f(6)).raw(), -10_923);
    // Exactly a half: 1 / 131072 in raw units is 0.5.
    let half_step = Fixed::from_raw(3);
    assert_eq!(half_step.div_round(f(2)).raw(), 2);
    assert_eq!(half_step.div_trunc(f(2)).raw(), 1);
    assert_eq!((-half_step).div_round(f(2)).raw(), -2);
}

#[test]
#[should_panic(expected = "division by zero")]
fn division_by_zero_panics() {
    let _ = f(1) / Fixed::ZERO;
}

#[test]
fn checked_division_by_zero_is_none() {
    assert_eq!(f(1).checked_div(Fixed::ZERO), None);
}

// --- overflow ---------------------------------------------------------

#[test]
fn addition_saturates_at_both_ends() {
    assert_eq!(Fixed::MAX + Fixed::EPSILON, Fixed::MAX);
    assert_eq!(Fixed::MAX + Fixed::MAX, Fixed::MAX);
    assert_eq!(Fixed::MIN - Fixed::EPSILON, Fixed::MIN);
    assert_eq!(Fixed::MIN + Fixed::MIN, Fixed::MIN);
}

#[test]
fn multiplication_saturates() {
    assert_eq!(Fixed::MAX * f(2), Fixed::MAX);
    assert_eq!(Fixed::MAX * f(-2), Fixed::MIN);
    assert_eq!(Fixed::MIN * f(2), Fixed::MIN);
    assert_eq!(Fixed::MIN * f(-2), Fixed::MAX);
    // The single largest intermediate: MIN * MIN in i64 is 2^62, which
    // must not overflow before the shift.
    assert_eq!(Fixed::MIN * Fixed::MIN, Fixed::MAX);
}

#[test]
fn division_saturates() {
    assert_eq!(Fixed::MAX / Fixed::EPSILON, Fixed::MAX);
    assert_eq!(Fixed::MIN / Fixed::EPSILON, Fixed::MIN);
    assert_eq!(Fixed::MAX / (-Fixed::EPSILON), Fixed::MIN);
}

#[test]
fn negation_of_the_minimum_saturates_rather_than_wrapping() {
    // The asymmetry of two's complement: -MIN is not representable.
    // Wrapping would give MIN back, which is the classic sign bug.
    assert_eq!(-Fixed::MIN, Fixed::MAX);
    assert_eq!(Fixed::MIN.abs(), Fixed::MAX);
    assert_eq!(Fixed::MIN.checked_neg(), None);
}

#[test]
fn checked_operations_report_what_saturating_ones_hide() {
    assert_eq!(Fixed::MAX.checked_add(Fixed::EPSILON), None);
    assert_eq!(Fixed::MIN.checked_sub(Fixed::EPSILON), None);
    assert_eq!(Fixed::MAX.checked_mul(f(2)), None);
    assert_eq!(Fixed::MAX.checked_div(Fixed::EPSILON), None);
    assert_eq!(f(2).checked_add(f(2)), Some(f(4)));
    assert_eq!(f(2).checked_mul(f(3)), Some(f(6)));
    assert_eq!(f(6).checked_div(f(3)), Some(f(2)));
}

/// Saturation must be identical whatever the build profile, because
/// two peers on different profiles must agree. Rust's own `+` would
/// panic here in debug and wrap in release; this asserts we do neither.
#[test]
fn overflow_behaves_the_same_in_debug_and_release() {
    let sum = Fixed::MAX + Fixed::ONE;
    assert_eq!(sum, Fixed::MAX);
    assert!(sum.is_positive(), "a wrap would have made this negative");
}

#[test]
fn mul_ratio_does_not_saturate_in_the_middle() {
    // MAX * 3 / 4 overflows if the multiply is done first in 32 bits,
    // and saturates to MAX if the operations are done separately.
    let expected = Fixed::from_raw((i32::MAX as i64 * 3 / 4) as i32);
    assert_eq!(Fixed::MAX.mul_ratio(3, 4), expected);
    assert_ne!(Fixed::MAX.mul_ratio(3, 4), Fixed::MAX);
    // Done as two operations, the multiply saturates first and the
    // three-quarters is taken of the wrong number.
    assert_eq!((Fixed::MAX * f(3)) / f(4), Fixed::from_raw(i32::MAX / 4));
}

#[test]
fn scale_int_saturates() {
    assert_eq!(f(1).scale_int(3), f(3));
    assert_eq!(f(-1).scale_int(3), f(-3));
    assert_eq!(Fixed::MAX.scale_int(2), Fixed::MAX);
    assert_eq!(Fixed::MAX.scale_int(-2), Fixed::MIN);
}

// --- roots ------------------------------------------------------------

#[test]
fn sqrt_is_exact_on_squares() {
    for n in 0..180 {
        assert_eq!(f(n * n).sqrt(), f(n), "sqrt of {}", n * n);
    }
}

#[test]
fn sqrt_truncates_and_never_overshoots() {
    let mut rng = l2_net::Pcg32::from_seed(31);
    for _ in 0..5_000 {
        let v = Fixed::from_raw(rng.range(0, i32::MAX));
        let root = v.sqrt();
        // root^2 <= v < (root + eps)^2, allowing for the truncation of
        // the squaring itself.
        assert!(root.mul_trunc(root) <= v, "sqrt({v}) = {root} overshoots");
        let next = root + Fixed::EPSILON;
        assert!(next.mul_trunc(next) >= v, "sqrt({v}) = {root} is more than one step low");
    }
}

#[test]
fn sqrt_of_a_fraction() {
    // sqrt(0.25) = 0.5, exactly representable.
    assert_eq!(Fixed::from_ratio(1, 4).sqrt(), Fixed::HALF);
    assert_eq!(Fixed::ZERO.sqrt(), Fixed::ZERO);
    assert_eq!(Fixed::ONE.sqrt(), Fixed::ONE);
}

#[test]
#[should_panic(expected = "negative")]
fn sqrt_of_a_negative_panics() {
    let _ = f(-1).sqrt();
}

#[test]
fn hypot_is_the_pythagorean_distance() {
    assert_eq!(Fixed::hypot(f(3), f(4)), f(5));
    assert_eq!(Fixed::hypot(f(-3), f(4)), f(5));
    assert_eq!(Fixed::hypot(f(0), f(-7)), f(7));
    // Across the diagonal of the 80x80 battlefield.
    let diagonal = Fixed::hypot(f(80), f(80));
    assert_eq!(diagonal.round(), 113);
}

/// The reason `hypot` exists: doing it by hand saturates long before
/// the field's diagonal does.
#[test]
fn hypot_survives_separations_that_would_saturate_a_manual_square() {
    let far = f(1_000);
    assert_eq!(far.mul_trunc(far), Fixed::MAX, "the manual version really does saturate");
    assert_eq!(Fixed::hypot(far, far).round(), 1_414);
}

// --- ordering, display, and the traits ---------------------------------

#[test]
fn ordering_is_the_numeric_ordering() {
    assert!(Fixed::MIN < f(-1));
    assert!(f(-1) < Fixed::ZERO);
    assert!(Fixed::ZERO < Fixed::EPSILON);
    assert!(Fixed::EPSILON < Fixed::ONE);
    assert!(Fixed::ONE < Fixed::MAX);
    let mut values = vec![f(3), f(-2), Fixed::ZERO, Fixed::MAX, Fixed::MIN];
    values.sort();
    assert_eq!(values, vec![Fixed::MIN, f(-2), Fixed::ZERO, f(3), Fixed::MAX]);
}

#[test]
fn min_max_and_clamp() {
    assert_eq!(f(3).min(f(5)), f(3));
    assert_eq!(f(3).max(f(5)), f(5));
    assert_eq!(f(9).clamp(f(0), f(5)), f(5));
    assert_eq!(f(-9).clamp(f(0), f(5)), f(0));
    assert_eq!(f(3).clamp(f(0), f(5)), f(3));
}

#[test]
fn display_uses_no_floating_point_and_reads_correctly() {
    assert_eq!(f(0).to_string(), "0");
    assert_eq!(f(42).to_string(), "42");
    assert_eq!(f(-42).to_string(), "-42");
    assert_eq!(Fixed::HALF.to_string(), "0.5");
    assert_eq!((-Fixed::HALF).to_string(), "-0.5");
    assert_eq!((f(1) + Fixed::HALF).to_string(), "1.5");
    assert_eq!(Fixed::from_ratio(1, 4).to_string(), "0.25");
    assert_eq!(Fixed::EPSILON.to_string(), "0.00001");
    assert_eq!(Fixed::MAX.to_string(), "32767.99998");
    assert_eq!(Fixed::MIN.to_string(), "-32768");
}

#[test]
fn assignment_operators_match_their_binary_forms() {
    let mut v = f(10);
    v += f(5);
    assert_eq!(v, f(15));
    v -= f(3);
    assert_eq!(v, f(12));
    v *= f(2);
    assert_eq!(v, f(24));
    v /= f(4);
    assert_eq!(v, f(6));
}

#[test]
fn small_integers_convert() {
    assert_eq!(Fixed::from(3i16), f(3));
    assert_eq!(Fixed::from(200u8), f(200));
}

#[test]
fn signs_and_predicates() {
    assert!(Fixed::ZERO.is_zero());
    assert!(f(1).is_positive());
    assert!(f(-1).is_negative());
    assert_eq!(f(9).signum_int(), 1);
    assert_eq!(f(-9).signum_int(), -1);
    assert_eq!(Fixed::ZERO.signum_int(), 0);
}
