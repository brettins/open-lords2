#![allow(unused_imports)]
use super::*;
use super::ranges::*;
use super::operations::*;
use l2_net::{Canonical, Pcg32};

/// `pcg32-demo.c` from `pcg-c-basic` calls `pcg32_srandom_r(&rng, 42u,
/// 54u)` and prints six 32-bit values; the published output begins
/// `0xa15c02b7 0x7b47f409 0xba1d3330 …`.
#[test]
fn matches_the_published_pcg32_demo() {
    let mut rng = Pcg32::new(42, 54);
    let got: Vec<u32> = (0..6).map(|_| rng.next_u32()).collect();
    assert_eq!(
        got,
        vec![0xa15c_02b7, 0x7b47_f409, 0xba1d_3330, 0x83d2_f293, 0xbfa4_784b, 0xcbed_606e]
    );
}

#[test]
fn the_default_stream_is_frozen() {
    let mut rng = Pcg32::from_seed(1);
    let got: Vec<u32> = (0..6).map(|_| rng.next_u32()).collect();
    assert_eq!(
        got,
        vec![0x8f23_3800, 0x33ff_c3eb, 0xc07f_0c27, 0xc880_a4ff, 0x0d00_6127, 0xb8f8_863f]
    );
}

#[test]
fn next_u64_is_high_word_first() {
    let mut wide = Pcg32::from_seed(7);
    let mut narrow = Pcg32::from_seed(7);
    let hi = narrow.next_u32() as u64;
    let lo = narrow.next_u32() as u64;
    assert_eq!(wide.next_u64(), (hi << 32) | lo);
}

#[test]
fn adjacent_seeds_give_unrelated_streams() {
    let mut a = Pcg32::from_seed(1000);
    let mut b = Pcg32::from_seed(1001);
    let first_a = a.next_u32();
    let first_b = b.next_u32();
    assert_ne!(first_a, first_b);
    assert!(
        (first_a ^ first_b).count_ones() >= 8,
        "seeds 1000 and 1001 produced first outputs differing in only {} bits",
        (first_a ^ first_b).count_ones()
    );
}

#[test]
fn different_streams_from_one_seed_do_not_coincide() {
    let mut a = Pcg32::new(99, 1);
    let mut b = Pcg32::new(99, 2);
    let sequence_a: Vec<u32> = (0..32).map(|_| a.next_u32()).collect();
    let sequence_b: Vec<u32> = (0..32).map(|_| b.next_u32()).collect();
    assert_ne!(sequence_a, sequence_b);
    assert!(!sequence_b.windows(4).any(|w| w == &sequence_a[0..4]));
}


#[test]
fn below_is_frozen() {
    let mut rng = Pcg32::from_seed(42);
    let got: Vec<u32> = (0..10).map(|_| rng.below(6)).collect();
    assert_eq!(got, vec![2, 2, 0, 4, 3, 5, 0, 4, 3, 4]);
}

#[test]
fn below_one_is_always_zero_and_still_draws() {
    let mut rng = Pcg32::from_seed(5);
    let mut reference = Pcg32::from_seed(5);
    for _ in 0..4 {
        assert_eq!(rng.below(1), 0);
        reference.next_u32();
    }
    assert_eq!(rng.parts(), reference.parts(), "below(1) must consume exactly one draw");
}

#[test]
#[should_panic(expected = "no value to return")]
fn below_zero_panics() {
    Pcg32::from_seed(1).below(0);
}

