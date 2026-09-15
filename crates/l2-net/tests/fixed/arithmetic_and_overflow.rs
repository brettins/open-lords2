#![allow(unused_imports)]
use super::*;
use super::rounding_and_conversions::*;
use super::math_functions::*;
use l2_net::Fixed;

#[test]
fn multiplication_truncates_toward_zero() {
    let third = Fixed::from_ratio(1, 3);
    assert_eq!(third.raw(), 21_845);
    assert_eq!((third * f(3)).raw(), 65_535);
    assert_eq!(((-third) * f(3)).raw(), -65_535);
}

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
    let three_halves = Fixed::from_raw(98_304); // 1.5
    let one_third = Fixed::from_ratio(1, 3);
    assert_eq!(three_halves.mul_round(one_third).raw(), 32_768);
    assert_eq!(three_halves.mul_trunc(one_third).raw(), 32_767);
    assert_eq!((-three_halves).mul_round(one_third).raw(), -32_768);
    assert_eq!((-three_halves).mul_trunc(one_third).raw(), -32_767);
}

#[test]
fn div_round_goes_to_nearest_halves_away_from_zero() {
    assert_eq!(Fixed::ONE.div_trunc(f(3)).raw(), 21_845);
    assert_eq!(Fixed::ONE.div_round(f(3)).raw(), 21_845);
    assert_eq!(Fixed::ONE.div_trunc(f(6)).raw(), 10_922);
    assert_eq!(Fixed::ONE.div_round(f(6)).raw(), 10_923);
    assert_eq!((-Fixed::ONE).div_round(f(6)).raw(), -10_923);
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

#[test]
fn overflow_behaves_the_same_in_debug_and_release() {
    let sum = Fixed::MAX + Fixed::ONE;
    assert_eq!(sum, Fixed::MAX);
    assert!(sum.is_positive(), "a wrap would have made this negative");
}

#[test]
fn mul_ratio_does_not_saturate_in_the_middle() {
    let expected = Fixed::from_raw((i32::MAX as i64 * 3 / 4) as i32);
    assert_eq!(Fixed::MAX.mul_ratio(3, 4), expected);
    assert_ne!(Fixed::MAX.mul_ratio(3, 4), Fixed::MAX);
    assert_eq!((Fixed::MAX * f(3)) / f(4), Fixed::from_raw(i32::MAX / 4));
}

#[test]
fn scale_int_saturates() {
    assert_eq!(f(1).scale_int(3), f(3));
    assert_eq!(f(-1).scale_int(3), f(-3));
    assert_eq!(Fixed::MAX.scale_int(2), Fixed::MAX);
    assert_eq!(Fixed::MAX.scale_int(-2), Fixed::MIN);
}

