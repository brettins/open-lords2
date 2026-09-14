#![allow(unused_imports)]
use super::*;
use super::vectors::*;
use super::ranges::*;
use l2_net::{Canonical, Pcg32};

#[test]
fn shuffle_is_frozen() {
    let mut rng = Pcg32::from_seed(2024);
    let mut items: Vec<u8> = (0..10).collect();
    rng.shuffle(&mut items);
    assert_eq!(items, vec![9, 2, 6, 7, 8, 1, 3, 5, 4, 0]);
}

#[test]
fn shuffle_of_nothing_draws_nothing() {
    let mut rng = Pcg32::from_seed(1);
    let mut reference = Pcg32::from_seed(1);
    rng.shuffle(&mut [0u8; 0]);
    rng.shuffle(&mut [7u8]);
    assert_eq!(rng.parts(), reference.parts());
    let _ = reference.next_u32();
}

#[test]
fn shuffle_is_a_permutation() {
    let mut rng = Pcg32::from_seed(77);
    for _ in 0..50 {
        let mut items: Vec<u32> = (0..40).collect();
        rng.shuffle(&mut items);
        let mut sorted = items.clone();
        sorted.sort();
        assert_eq!(sorted, (0..40).collect::<Vec<_>>());
    }
}

#[test]
fn index_of_an_empty_collection_is_none_and_draws_nothing() {
    let mut rng = Pcg32::from_seed(1);
    let before = rng.parts();
    assert_eq!(rng.index(0), None);
    assert_eq!(rng.parts(), before);
}

// --- jumping and forking ---------------------------------------------

/// The closed-form jump must agree with the loop it replaces. This is
/// the only way to believe a piece of modular exponentiation.
#[test]
fn advance_agrees_with_stepping_one_at_a_time() {
    for n in [0u64, 1, 2, 3, 17, 64, 1000, 65_536] {
        let mut jumped = Pcg32::from_seed(4);
        jumped.advance(n);
        let mut stepped = Pcg32::from_seed(4);
        for _ in 0..n {
            stepped.next_u32();
        }
        assert_eq!(jumped.parts(), stepped.parts(), "advance({n}) disagrees with {n} draws");
        assert_eq!(jumped.next_u32(), stepped.next_u32());
    }
}

#[test]
fn advance_works_on_a_non_default_stream() {
    let mut jumped = Pcg32::new(9, 3);
    jumped.advance(500);
    let mut stepped = Pcg32::new(9, 3);
    for _ in 0..500 {
        stepped.next_u32();
    }
    assert_eq!(jumped.parts(), stepped.parts());
}

#[test]
fn fork_advances_the_parent() {
    let mut parent = Pcg32::from_seed(6);
    let before = parent.parts();
    let child = parent.fork();
    assert_ne!(parent.parts(), before, "a fork that left no trace would be invisible in state");
    assert_ne!(child.parts(), parent.parts());
}

#[test]
fn fork_is_deterministic() {
    let mut a = Pcg32::from_seed(6);
    let mut b = Pcg32::from_seed(6);
    assert_eq!(a.fork().parts(), b.fork().parts());
}

// --- serialisation ---------------------------------------------------

#[test]
fn a_generator_round_trips_through_its_parts() {
    let mut rng = Pcg32::from_seed(1234);
    for _ in 0..37 {
        rng.next_u32();
    }
    let (state, increment) = rng.parts();
    let mut restored = Pcg32::from_parts(state, increment);
    for _ in 0..100 {
        assert_eq!(restored.next_u32(), rng.next_u32());
    }
}

#[test]
fn a_generator_round_trips_through_the_canonical_encoding() {
    let mut rng = Pcg32::from_seed(99);
    rng.next_u64();
    let bytes = Canonical::bytes_of(&rng);
    assert_eq!(bytes.len(), 16, "the whole generator is sixteen bytes");
    let restored: Pcg32 = l2_net::decode_all(&bytes).expect("decode");
    assert_eq!(restored, rng);
}

/// An even increment collapses the LCG's period. A snapshot carrying
/// one is corrupt, and repairing it beats generating a broken stream.
#[test]
fn an_even_increment_is_forced_odd() {
    let rng = Pcg32::from_parts(1, 4);
    assert_eq!(rng.parts().1, 5);
}

/// The generator is part of simulation state, so it must be comparable
/// and clonable — the first for desync dumps, the second for asking
/// "what would this roll be" without disturbing the stream.
#[test]
fn generators_compare_and_clone() {
    let mut rng = Pcg32::from_seed(3);
    let mut copy = rng.clone();
    assert_eq!(rng, copy);
    copy.next_u32();
    assert_ne!(rng, copy);
    rng.next_u32();
    assert_eq!(rng, copy);
}

