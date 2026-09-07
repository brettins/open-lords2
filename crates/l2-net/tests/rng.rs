//! The PRNG's value stream, pinned.
//!
//! `docs/netcode.md` D-3 requires the generator's output to be frozen
//! *forever*, because a change to it is a silent desync between a
//! player who updated and one who did not, and between a replay and its
//! recording. This file is the mechanism that makes "forever" mean
//! something: **if you change `src/rng.rs` and a test here fails, the
//! test is right.**
//!
//! The vectors come in two kinds and the difference matters.
//!
//! * [`matches_the_published_pcg32_demo`] checks our transcription
//!   against *somebody else's* published numbers — the output of
//!   O'Neill's `pcg32-demo` from `pcg-c-basic` seeded `(42, 54)`, which
//!   is quoted on pcg-random.org. That is the only test here that can
//!   catch a mistake in the algorithm itself; every other vector below
//!   would happily pin a wrong implementation.
//! * The rest pin *our* conventions — the seeding dance, the order of
//!   the two draws in `next_u64`, the rejection method in `below`, the
//!   direction of the shuffle. Those are ours to choose and ours to
//!   freeze, and they are recorded here because they are not written
//!   down anywhere else in the world.

use l2_net::{Canonical, Pcg32};

/// The one vector in this file that is somebody else's.
///
/// `pcg32-demo.c` from `pcg-c-basic` calls `pcg32_srandom_r(&rng, 42u,
/// 54u)` and prints six 32-bit values; the published output begins
/// `0xa15c02b7 0x7b47f409 0xba1d3330 …`.
///
/// It exercises the whole algorithm: the two-step seeding, the LCG
/// advance, the xorshift, and the variable rotation — a rotation that
/// is wrong in one direction still produces plausible noise and would
/// pass every statistical check a hobby project would think to write.
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

/// Two draws, high word first. Swapping them would change every 64-bit
/// value the game has ever produced while breaking nothing visible.
#[test]
fn next_u64_is_high_word_first() {
    let mut wide = Pcg32::from_seed(7);
    let mut narrow = Pcg32::from_seed(7);
    let hi = narrow.next_u32() as u64;
    let lo = narrow.next_u32() as u64;
    assert_eq!(wide.next_u64(), (hi << 32) | lo);
}

/// Adjacent seeds must not produce adjacent streams. This is what the
/// seeding dance in `Pcg32::new` buys, and it is the reason the state
/// is not simply set to the seed.
#[test]
fn adjacent_seeds_give_unrelated_streams() {
    let mut a = Pcg32::from_seed(1000);
    let mut b = Pcg32::from_seed(1001);
    let first_a = a.next_u32();
    let first_b = b.next_u32();
    assert_ne!(first_a, first_b);
    // Not merely different: differing in a good spread of bits.
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
    // And they must not merely be offset from one another.
    assert!(!sequence_b.windows(4).any(|w| w == &sequence_a[0..4]));
}

// --- bounded draws ---------------------------------------------------

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

/// The debiasing must actually debias. A modulo without rejection makes
/// the low residues more likely; over a large sample with a bound that
/// divides badly into 2^32, that shows up as a measurable skew.
///
/// This is a statistical test with a fixed seed, so it is
/// deterministic: it either passes forever or fails forever.
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

/// The full `i32` span is wider than `below` works in, and takes the
/// 64-bit path. It is the boundary most likely to be wrong.
#[test]
fn range_spans_the_whole_of_i32() {
    let mut rng = Pcg32::from_seed(11);
    for _ in 0..1000 {
        let _ = rng.range(i32::MIN, i32::MAX);
    }
    // A span of exactly 2^32, which is the first value that does not
    // fit a u32 and therefore the first that takes the other branch.
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

// --- shuffle ---------------------------------------------------------

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
