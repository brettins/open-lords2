#![allow(unused_imports)]
use super::*;
use super::vectors::*;
use super::operations::*;
use l2_net::{Canonical, Pcg32};

#[test]
fn below_is_uniform_enough_to_notice_a_bias() {
    const BOUND: u32 = 7;
    const DRAWS: u32 = 70_000;
    let mut rng = Pcg32::from_seed(0xdead_beef);
    let mut counts = [0u32; BOUND as usize];
    for _ in 0..DRAWS {
        counts[rng.below(BOUND) as usize] += 1;
    }
    let expected = DRAWS / BOUND;
    for (value, &count) in counts.iter().enumerate() {
        let drift = count.abs_diff(expected);
        assert!(
            drift < expected / 20,
            "value {value} came up {count} times, expected about {expected}"
        );
    }
}

#[test]
fn range_is_inclusive_at_both_ends() {
    let mut rng = Pcg32::from_seed(3);
    let mut saw_low = false;
    let mut saw_high = false;
    for _ in 0..1000 {
        let v = rng.range(-2, 2);
        assert!((-2..=2).contains(&v));
        saw_low |= v == -2;
        saw_high |= v == 2;
    }
    assert!(saw_low && saw_high, "range must reach both ends");
}

#[test]
fn range_of_one_value_is_that_value() {
    let mut rng = Pcg32::from_seed(3);
    assert_eq!(rng.range(9, 9), 9);
}

#[test]
fn range_spans_the_whole_of_i32() {
    let mut rng = Pcg32::from_seed(11);
    for _ in 0..1000 {
        let _ = rng.range(i32::MIN, i32::MAX);
    }
    let mut low_half = 0;
    let mut high_half = 0;
    for _ in 0..1000 {
        if rng.range(i32::MIN, i32::MAX) < 0 {
            low_half += 1;
        } else {
            high_half += 1;
        }
    }
    assert!(low_half > 400 && high_half > 400, "{low_half} negative, {high_half} non-negative");
}

#[test]
#[should_panic(expected = "is empty")]
fn an_inverted_range_panics() {
    Pcg32::from_seed(1).range(5, 4);
}

#[test]
fn chance_draws_even_at_the_extremes() {
    let mut rng = Pcg32::from_seed(8);
    let mut reference = Pcg32::from_seed(8);
    assert!(!rng.chance(0, 10));
    assert!(rng.chance(10, 10));
    reference.next_u32();
    reference.next_u32();
    assert_eq!(
        rng.parts(),
        reference.parts(),
        "a certainty must still consume its draw, or changing a balance number \
         reshuffles every later roll"
    );
}

#[test]
fn chance_is_about_right() {
    let mut rng = Pcg32::from_seed(1234);
    let hits = (0..10_000).filter(|_| rng.chance(1, 4)).count();
    assert!((2300..2700).contains(&hits), "{hits} hits in 10,000 at one in four");
}


