//! xxHash64 against the reference implementation's own values.
//!
//! Every vector in the first section is somebody else's number. They
//! are the test vectors from `twox-hash`, which states that its
//! implementation is verified against the reference C implementation —
//! so what is being checked here is not "does our hasher agree with
//! itself" but "does our hasher agree with the rest of the world".
//! That distinction is the whole reason for vendoring rather than
//! depending: a vendored algorithm with nobody else's vectors is just
//! a private hash function with a misleading name.
//!
//! The vectors were chosen by whoever wrote them to cover the branches
//! that a transcription gets wrong, and they do:
//!
//! | input | what it exercises |
//! |---|---|
//! | empty | the short path, and the `len` fold |
//! | one byte | the single-byte tail loop |
//! | 14 bytes | the 8-byte and 4-byte tail steps and two stray bytes |
//! | 100 bytes | three full 32-byte blocks and a 4-byte tail |
//! | a non-zero seed | the accumulator initialisation |
//! | `u64::MAX - PRIME64_5` | the wrap in `seed + PRIME5` |
//!
//! A bug in the tail handling only fires on inputs of particular
//! lengths, which is exactly the kind of bug that passes a "hash the
//! same thing twice" check and then produces a checksum nobody else in
//! the world agrees with.

use l2_net::{xxhash64, XxHash64};

const OTHER_SEED: u64 = 0xae05_4331_1b70_2d91;

#[test]
fn empty_input() {
    assert_eq!(xxhash64(b"", 0), 0xef46_db37_51d8_e999);
}

#[test]
fn one_byte() {
    assert_eq!(xxhash64(&[42], 0), 0x0a9e_dece_beb0_3ae4);
}

#[test]
fn fourteen_bytes() {
    assert_eq!(xxhash64(b"Hello, world!\0", 0), 0x7b06_c531_ea43_e89f);
}

#[test]
fn a_hundred_bytes() {
    let bytes: Vec<u8> = (0..100).collect();
    assert_eq!(xxhash64(&bytes, 0), 0x6ac1_e580_3216_6597);
}

#[test]
fn empty_input_with_a_seed() {
    assert_eq!(xxhash64(b"", OTHER_SEED), 0x4b6a_04fc_df7a_4672);
}

#[test]
fn a_hundred_bytes_with_a_seed() {
    let bytes: Vec<u8> = (0..100).collect();
    assert_eq!(xxhash64(&bytes, OTHER_SEED), 0x567e_355e_0682_e1f1);
}

/// The seed is added to `PRIME64_5` on the short path. This one is
/// chosen so that the addition wraps, which a transcription using
/// checked or saturating arithmetic would get wrong — and would get
/// wrong only for seeds nobody would think to test.
#[test]
fn a_seed_that_makes_the_short_path_wrap() {
    let seed = u64::MAX - 0x27D4_EB2F_1656_67C5;
    assert_eq!(xxhash64(b"x", seed), 0xf953_d52c_12a9_f5fb);
}

// --- streaming -------------------------------------------------------

/// Streaming must equal oneshot for *every* way of splitting the input,
/// because that is what `Canonical` relies on: it writes a state field
/// by field and the resulting checksum must be the one a peer computes
/// from the same bytes delivered differently.
#[test]
fn every_split_of_an_input_hashes_the_same() {
    let bytes: Vec<u8> = (0..200u32).map(|i| (i * 7) as u8).collect();
    let expected = xxhash64(&bytes, 0);
    for split in 0..=bytes.len() {
        let mut hasher = XxHash64::new();
        hasher.write(&bytes[..split]);
        hasher.write(&bytes[split..]);
        assert_eq!(hasher.finish(), expected, "split at {split}");
    }
}

#[test]
fn one_byte_at_a_time_hashes_the_same() {
    let bytes: Vec<u8> = (0..77u8).collect();
    let mut hasher = XxHash64::new();
    for byte in &bytes {
        hasher.write(&[*byte]);
    }
    assert_eq!(hasher.finish(), xxhash64(&bytes, 0));
}

#[test]
fn empty_writes_change_nothing() {
    let mut hasher = XxHash64::new();
    hasher.write(b"");
    hasher.write(b"abc");
    hasher.write(b"");
    assert_eq!(hasher.finish(), xxhash64(b"abc", 0));
    assert_eq!(hasher.len(), 3);
}

/// `finish` takes `&self`, so a caller can take an intermediate digest
/// and carry on. If that ever mutated the state it would corrupt every
/// section digest in `Canonical`.
#[test]
fn finishing_does_not_disturb_the_hasher() {
    let mut hasher = XxHash64::new();
    hasher.write(b"first");
    let intermediate = hasher.finish();
    assert_eq!(hasher.finish(), intermediate);
    hasher.write(b"second");
    assert_eq!(hasher.finish(), xxhash64(b"firstsecond", 0));
}

#[test]
fn the_length_is_tracked() {
    let mut hasher = XxHash64::new();
    assert!(hasher.is_empty());
    hasher.write(&[0u8; 40]);
    hasher.write(&[0u8; 5]);
    assert_eq!(hasher.len(), 45);
    assert!(!hasher.is_empty());
}

// --- the properties a checksum actually needs -------------------------

/// The whole job of a state checksum is that any one-bit difference
/// changes the output. FNV-1a was rejected in `docs/netcode.md` §6
/// precisely because its avalanche on long runs of structured,
/// mostly-zero records — which is what a serialised unit array is — is
/// poor. So test on exactly that kind of input.
#[test]
fn a_single_bit_in_a_field_of_zeros_changes_everything() {
    let zeros = [0u8; 512];
    let base = xxhash64(&zeros, 0);
    for position in [0usize, 1, 31, 32, 33, 255, 400, 511] {
        for bit in 0..8 {
            let mut altered = zeros;
            altered[position] ^= 1 << bit;
            let changed = xxhash64(&altered, 0);
            assert_ne!(changed, base, "flipping bit {bit} of byte {position} changed nothing");
            let differing = (changed ^ base).count_ones();
            assert!(
                (16..=48).contains(&differing),
                "one flipped bit moved {differing} of 64 output bits — poor avalanche"
            );
        }
    }
}

/// Length must be part of the hash. Without it, a state with one extra
/// zeroed record at the end would checksum identically to one without.
#[test]
fn trailing_zeros_are_not_invisible() {
    assert_ne!(xxhash64(&[0u8; 16], 0), xxhash64(&[0u8; 17], 0));
    assert_ne!(xxhash64(&[0u8; 32], 0), xxhash64(&[0u8; 33], 0));
    assert_ne!(xxhash64(b"", 0), xxhash64(&[0u8; 1], 0));
}

/// Different seeds must give different hashes for the same input, on
/// both the short and the long path.
#[test]
fn the_seed_matters_on_both_paths() {
    assert_ne!(xxhash64(b"short", 0), xxhash64(b"short", 1));
    let long = [7u8; 200];
    assert_ne!(xxhash64(&long, 0), xxhash64(&long, 1));
}

/// No collisions across a large sweep of small, structurally similar
/// inputs — the population a state checksum actually faces, where two
/// states differ by one field of one record.
#[test]
fn no_collisions_across_a_sweep_of_similar_states() {
    let mut seen: Vec<u64> = Vec::with_capacity(20_000);
    for i in 0..20_000u32 {
        let mut record = [0u8; 64];
        record[8..12].copy_from_slice(&i.to_le_bytes());
        seen.push(xxhash64(&record, 0));
    }
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), before, "{} collisions", before - seen.len());
}
