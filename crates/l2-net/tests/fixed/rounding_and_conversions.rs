#![allow(unused_imports)]
use super::*;
use super::arithmetic_and_overflow::*;
use super::math_functions::*;
use l2_net::Fixed;

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

