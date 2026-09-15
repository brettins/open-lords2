#![allow(unused_imports)]
use super::*;
use super::decoding_tests::*;
use l2_net::{decode_all, Canonical, CodecError, Fixed, Pcg32, Reader, CHECKSUM_SEED};

#[test]
fn every_scalar_is_fixed_width_little_endian() {
    let mut c = Canonical::recording();
    c.u8(0x11);
    c.u16(0x2233);
    c.u32(0x4455_6677);
    c.u64(0x8899_aabb_ccdd_eeff);
    c.i8(-1);
    c.i16(-2);
    c.i32(-3);
    c.i64(-4);
    c.bool(true);
    c.bool(false);
    c.fixed(Fixed::ONE);
    let bytes = c.finish().bytes.unwrap();
    assert_eq!(
        bytes,
        vec![
            0x11, //
            0x33, 0x22, //
            0x77, 0x66, 0x55, 0x44, //
            0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, //
            0xff, //
            0xfe, 0xff, //
            0xfd, 0xff, 0xff, 0xff, //
            0xfc, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
            0x01, 0x00, //
            0x00, 0x00, 0x01, 0x00, //
        ]
    );
}

#[test]
fn the_length_reported_is_the_length_written() {
    let mut c = Canonical::hashing();
    assert!(c.is_empty());
    c.u32(1);
    c.str("abcd");
    assert_eq!(c.len(), 4 + 4 + 4);
    let digest = c.finish();
    assert_eq!(digest.len, 12);
}

#[test]
fn adjacent_strings_cannot_be_confused_with_each_other() {
    let mut a = Canonical::recording();
    a.str("ab");
    a.str("c");
    let mut b = Canonical::recording();
    b.str("a");
    b.str("bc");
    let a = a.finish();
    let b = b.finish();
    assert_ne!(a.bytes, b.bytes);
    assert_ne!(a.hash, b.hash);
}

#[test]
fn raw_bytes_are_ambiguous_by_design() {
    let mut a = Canonical::recording();
    a.raw(b"ab");
    a.raw(b"c");
    let mut b = Canonical::recording();
    b.raw(b"abc");
    assert_eq!(a.finish().bytes, b.finish().bytes);
}

#[test]
fn the_checksum_is_plain_xxhash64_of_the_bytes() {
    let mut c = Canonical::recording();
    c.u32(0xdead_beef);
    c.str("units");
    let digest = c.finish();
    assert_eq!(digest.hash, l2_net::xxhash64(digest.bytes.as_ref().unwrap(), CHECKSUM_SEED));
}

#[test]
fn hashing_and_recording_agree() {
    let build = |c: &mut Canonical| {
        c.section("a");
        c.u64(7);
        c.section("b");
        c.seq(&[1u8, 2, 3], |c, v| c.u8(*v));
    };
    let mut hashing = Canonical::hashing();
    build(&mut hashing);
    let mut recording = Canonical::recording();
    build(&mut recording);
    let hashing = hashing.finish();
    let recording = recording.finish();
    assert_eq!(hashing.hash, recording.hash);
    assert_eq!(hashing.sections, recording.sections);
    assert!(hashing.bytes.is_none());
    assert!(recording.bytes.is_some());
}

#[test]
#[should_panic(expected = "exceeds u32")]
fn a_length_beyond_u32_panics_rather_than_truncating() {
    let mut c = Canonical::hashing();
    c.len32(u32::MAX as usize + 1);
}


#[test]
fn sections_are_hashed_separately_and_in_order() {
    let mut c = Canonical::hashing();
    c.u8(0); // before any section: counted in the total only
    c.section("units");
    c.u32(1);
    c.section("terrain");
    c.u32(2);
    let digest = c.finish();

    assert_eq!(digest.len, 9);
    assert_eq!(digest.sections.len(), 2);
    assert_eq!(digest.sections[0].name, "units");
    assert_eq!(digest.sections[0].len, 4);
    assert_eq!(digest.sections[1].name, "terrain");
    assert_eq!(digest.sections[0].hash, l2_net::xxhash64(&1u32.to_le_bytes(), CHECKSUM_SEED));
}

#[test]
fn a_section_with_no_bytes_still_appears() {
    let mut c = Canonical::hashing();
    c.section("empty");
    c.section("also_empty");
    let digest = c.finish();
    assert_eq!(digest.sections.len(), 2);
    assert_eq!(digest.sections[0].len, 0);
}

#[test]
fn a_single_differing_section_is_named() {
    let build = |units: u32, terrain: u32| {
        let mut c = Canonical::hashing();
        c.section("units");
        c.u32(units);
        c.section("terrain");
        c.u32(terrain);
        c.finish()
    };
    let base = build(1, 2);
    let units_moved = build(9, 2);
    let both_moved = build(9, 9);

    assert_eq!(base.sole_difference(&units_moved), Some("units"));
    assert_eq!(base.differences(&units_moved), vec!["units"]);
    assert_eq!(base.sole_difference(&base), None, "agreement is not a difference");
    assert_eq!(base.sole_difference(&both_moved), None, "two differences localise nothing");
    assert_eq!(base.differences(&both_moved), vec!["units", "terrain"]);
}

#[test]
fn digests_with_different_section_layouts_do_not_pretend_to_localise() {
    let mut a = Canonical::hashing();
    a.section("units");
    a.u32(1);
    let mut b = Canonical::hashing();
    b.section("units");
    b.u32(1);
    b.section("extra");
    b.u32(1);
    assert_eq!(a.finish().sole_difference(&b.finish()), None);
}

