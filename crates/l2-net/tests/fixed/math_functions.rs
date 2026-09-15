#![allow(unused_imports)]
use super::*;
use super::rounding_and_conversions::*;
use super::arithmetic_and_overflow::*;
use l2_net::Fixed;


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
        assert!(root.mul_trunc(root) <= v, "sqrt({v}) = {root} overshoots");
        let next = root + Fixed::EPSILON;
        assert!(next.mul_trunc(next) >= v, "sqrt({v}) = {root} is more than one step low");
    }
}

#[test]
fn sqrt_of_a_fraction() {
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
    let diagonal = Fixed::hypot(f(80), f(80));
    assert_eq!(diagonal.round(), 113);
}

#[test]
fn hypot_survives_separations_that_would_saturate_a_manual_square() {
    let far = f(1_000);
    assert_eq!(far.mul_trunc(far), Fixed::MAX, "the manual version really does saturate");
    assert_eq!(Fixed::hypot(far, far).round(), 1_414);
}


