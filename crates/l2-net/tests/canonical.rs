//! The canonical encoding: fixed widths, self-delimiting fields, and
//! one byte stream shared by the checksum, the snapshot and the dump.

use l2_net::{decode_all, Canonical, CodecError, Fixed, Pcg32, Reader, CHECKSUM_SEED};

// --- the stream is what it says it is ---------------------------------

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

/// The reason variable-length fields are prefixed. Without it these two
/// states produce identical bytes and therefore identical checksums,
/// and the detector has a blind spot exactly where field boundaries
/// move.
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

/// `raw` is the escape hatch and it really does have this hazard, which
/// is why it is documented as being for fixed-size arrays only.
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
    // Reproducible with any off-the-shelf XXH64 implementation, which
    // is why CHECKSUM_SEED is zero.
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
    // Not reachable with a real collection; the guard is what matters.
    let mut c = Canonical::hashing();
    c.len32(u32::MAX as usize + 1);
}

// --- sections ---------------------------------------------------------

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

/// §6's localisation: the first tick where exactly one subsystem hash
/// differs names the subsystem.
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

// --- round trips ------------------------------------------------------

#[test]
fn scalars_round_trip() {
    let mut c = Canonical::recording();
    c.u8(1);
    c.u16(2);
    c.u32(3);
    c.u64(4);
    c.i8(-1);
    c.i16(-2);
    c.i32(-3);
    c.i64(-4);
    c.bool(true);
    c.fixed(Fixed::from_ratio(1, 3));
    c.str("hello");
    c.bytes(&[9, 8, 7]);
    let bytes = c.finish().bytes.unwrap();

    let mut r = Reader::new(&bytes);
    assert_eq!(r.u8().unwrap(), 1);
    assert_eq!(r.u16().unwrap(), 2);
    assert_eq!(r.u32().unwrap(), 3);
    assert_eq!(r.u64().unwrap(), 4);
    assert_eq!(r.i8().unwrap(), -1);
    assert_eq!(r.i16().unwrap(), -2);
    assert_eq!(r.i32().unwrap(), -3);
    assert_eq!(r.i64().unwrap(), -4);
    assert!(r.bool().unwrap());
    assert_eq!(r.fixed().unwrap(), Fixed::from_ratio(1, 3));
    assert_eq!(r.str().unwrap(), "hello");
    assert_eq!(r.bytes().unwrap(), &[9, 8, 7]);
    r.finish().unwrap();
}

#[test]
fn sequences_and_options_round_trip() {
    let mut c = Canonical::recording();
    c.seq(&[10u32, 20, 30], |c, v| c.u32(*v));
    c.option(Some(&7u8), |c, v| c.u8(*v));
    c.option(None::<&u8>, |c, v| c.u8(*v));
    let bytes = c.finish().bytes.unwrap();

    let mut r = Reader::new(&bytes);
    assert_eq!(r.seq(|r| r.u32()).unwrap(), vec![10, 20, 30]);
    assert_eq!(r.option(|r| r.u8()).unwrap(), Some(7));
    assert_eq!(r.option(|r| r.u8()).unwrap(), None);
    r.finish().unwrap();
}

// --- decoding hostile input --------------------------------------------

#[test]
fn running_off_the_end_is_an_error_not_a_panic() {
    let mut r = Reader::new(&[1, 2]);
    assert_eq!(
        r.u32(),
        Err(CodecError::UnexpectedEnd { wanted: 4, remaining: 2, at: 0 })
    );
}

/// A four-billion-byte length prefix must be an error, not a
/// four-billion-byte allocation.
#[test]
fn an_absurd_length_prefix_is_refused() {
    let mut bytes = u32::MAX.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"short");
    let mut r = Reader::new(&bytes);
    assert!(matches!(r.bytes(), Err(CodecError::LengthOverrun { .. })));
}

#[test]
fn trailing_bytes_are_an_error() {
    let mut bytes = Canonical::bytes_of(&Fixed::ONE);
    bytes.push(0);
    assert_eq!(
        decode_all::<Fixed>(&bytes),
        Err(CodecError::TrailingBytes { unread: 1 })
    );
}

#[test]
fn a_bool_that_is_neither_zero_nor_one_is_an_error() {
    let mut r = Reader::new(&[2]);
    assert!(matches!(r.bool(), Err(CodecError::BadTag { tag: 2, .. })));
}

#[test]
fn invalid_utf8_is_an_error() {
    let mut bytes = 2u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[0xff, 0xfe]);
    let mut r = Reader::new(&bytes);
    assert!(matches!(r.str(), Err(CodecError::NotUtf8 { .. })));
}

#[test]
fn a_truncated_sequence_reports_the_shortfall() {
    let mut bytes = 3u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[1, 2]); // three u8 items promised, two given
    let mut r = Reader::new(&bytes);
    assert!(r.seq(|r| r.u8()).is_err());
}

#[test]
fn the_reader_tracks_where_it_is() {
    let bytes = [1u8, 2, 3, 4, 5];
    let mut r = Reader::new(&bytes);
    assert_eq!(r.remaining(), 5);
    r.u32().unwrap();
    assert_eq!(r.position(), 4);
    assert_eq!(r.remaining(), 1);
    assert!(!r.at_end());
    r.u8().unwrap();
    assert!(r.at_end());
}

#[test]
fn errors_say_something_useful() {
    let error = CodecError::UnexpectedEnd { wanted: 8, remaining: 3, at: 12 };
    assert_eq!(error.to_string(), "wanted 8 bytes at offset 12, 3 remain");
}

// --- the generator is part of the state ---------------------------------

#[test]
fn the_generator_is_inside_the_checksum() {
    let mut a = Pcg32::from_seed(1);
    let b = Pcg32::from_seed(1);
    assert_eq!(Canonical::hash_of(&a), Canonical::hash_of(&b));
    a.next_u32();
    assert_ne!(
        Canonical::hash_of(&a),
        Canonical::hash_of(&b),
        "a peer that drew one extra value must show it immediately"
    );
}
