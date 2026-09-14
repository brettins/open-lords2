#![allow(unused_imports)]
use super::*;
use super::encoding_tests::*;
use l2_net::{decode_all, Canonical, CodecError, Fixed, Pcg32, Reader, CHECKSUM_SEED};

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

