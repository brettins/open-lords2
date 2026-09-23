#![allow(unused_imports)]
use super::*;

use std::collections::BTreeMap;
use super::super::*;

#[test]
fn bits_are_read_least_significant_first() {
    let data = [0b1010_0110u8, 0x01];
    let mut b = Bits::new(&data, "test");
    let got: Vec<usize> = (0..8).map(|_| b.bit().unwrap()).collect();
    assert_eq!(got, [0, 1, 1, 0, 0, 1, 0, 1]);
    assert_eq!(b.bits(8).unwrap(), 1);
    assert_eq!(b.bit(), Err(Error::Overrun("test")));
}

#[test]
fn the_run_table_is_one_to_fifty_nine_then_powers_of_two() {
    assert_eq!(RUN[0], 1);
    assert_eq!(RUN[58], 59);
    assert_eq!(&RUN[59..], &[128, 256, 512, 1024, 2048]);
}

#[test]
fn a_six_bit_component_is_widened_by_replication() {
    assert_eq!(expand6(0), 0);
    assert_eq!(expand6(63), 255);
    assert_eq!(expand6(16), 0x41, "not 0x40, which is what * 255 / 63 gives");
}

#[test]
fn the_header_says_what_was_written() {
    let smk = Smk::parse(synthetic()).unwrap();
    let h = smk.header();
    assert_eq!((h.width, h.height, h.frames), (8, 8, 2));
    assert_eq!(h.period_10us(), 8333);
    assert_eq!(h.y_scale(), 1);
    assert_eq!(h.track(0), Some(Track { rate: 11025, packed: true, bits16: false, stereo: false }));
    assert_eq!(h.track(1), None);
    assert_eq!(smk.slack(), 0);
}

#[test]
fn every_block_type_draws_what_the_description_says() {
    let smk = Smk::parse(synthetic()).unwrap();
    let mut d = smk.decoder();
    assert!(d.next_frame(&smk).unwrap());
    let px = d.pixels().to_vec();
    let at = |x: usize, y: usize| px[y * 8 + x];
    assert!((0..4).all(|y| (0..4).all(|x| at(x, y) == 5)));
    assert_eq!(at(4, 0), 9, "map bit 0 set: the high colour");
    assert_eq!(at(5, 0), 7, "map bit 1 clear: the low colour");
    assert_eq!(at(7, 3), 9, "map bit 15 set");
    assert_eq!(at(6, 3), 7);
    for y in 4..8 {
        assert_eq!([at(0, y), at(1, y), at(2, y), at(3, y)], [0x0C, 0x0D, 0x0A, 0x0B]);
    }
    assert!((4..8).all(|y| (4..8).all(|x| at(x, y) == 0)));
    let (read, total) = d.video_bits();
    assert!(total - read < 8, "{read} of {total} bits read");
}

/// **`_SmackToBuffer@28` (`0x403AF0`): flag `0x02` writes the even rows and
/// leaves the odd ones as the cleared buffer; `0x04` writes each row twice.**
#[test]
fn a_doubled_frame_has_black_odd_rows_and_a_written_one_has_none() {
    let film = |flags: u8| {
        let mut f = synthetic();
        f[0x14] = flags;
        Smk::parse(f).unwrap()
    };
    let frame0 = |smk: &Smk| {
        let mut d = smk.decoder();
        d.next_frame(smk).unwrap();
        d
    };

    let smk = film(flag::Y_SCALE_1 as u8);
    assert_eq!(smk.header().display_height(), 16);
    let d = frame0(&smk);
    let out = d.display();
    assert_eq!(out.len(), 8 * 16);
    for row in 0..8 {
        assert_eq!(&out[row * 16..][..8], &d.pixels()[row * 8..][..8], "row {row} is written");
        assert!(out[row * 16 + 8..][..8].iter().all(|&p| p == 0), "row {row}'s twin is black");
    }

    let smk = film(flag::Y_SCALE_2 as u8);
    let d = frame0(&smk);
    let out = d.display();
    for row in 0..8 {
        assert_eq!(&out[row * 16..][..8], &out[row * 16 + 8..][..8], "row {row} written twice");
    }
    assert!(out.iter().any(|&p| p != 0), "and it is not a black frame either way");

    let smk = film(0);
    assert_eq!(frame0(&smk).display().len(), 8 * 8, "no flag, no scaling");
}

#[test]
fn the_palette_copies_from_the_palette_before_this_frame() {
    let smk = Smk::parse(synthetic()).unwrap();
    let mut d = smk.decoder();
    d.next_frame(&smk).unwrap();
    assert!(d.palette_changed());
    let p = *d.palette();
    assert_eq!(p[0], [255, 0, 0]);
    assert_eq!(p[1], [0, 0x41, 0]);
    assert_eq!(p[4], [0, 0, 0], "copied from the old entry 0, which was black");
    assert_eq!(p[5], [expand6(1), expand6(2), expand6(3)]);
    d.next_frame(&smk).unwrap();
    let p = *d.palette();
    assert_eq!(p[0], [0, 0, 255]);
    assert_eq!(p[1], [255, 0, 0], "the snapshot, not what this frame wrote");
}

#[test]
fn a_skip_keeps_last_frame_and_an_escape_is_a_recent_value() {
    let smk = Smk::parse(synthetic()).unwrap();
    let mut d = smk.decoder();
    d.next_frame(&smk).unwrap();
    let before = d.pixels().to_vec();
    assert!(d.next_frame(&smk).unwrap());
    let after = d.pixels().to_vec();
    let block = |px: &[u8], bx: usize, by: usize| -> Vec<u8> {
        (0..16).map(|i| px[(by * 4 + i / 4) * 8 + bx * 4 + i % 4]).collect()
    };
    assert_eq!(block(&after, 0, 0), block(&before, 0, 0), "block 0 skipped");
    assert_eq!(block(&after, 1, 0), block(&before, 1, 0), "block 1 skipped through recent[0]");
    assert_eq!(block(&after, 0, 1), vec![5; 16], "block 2 solid over the old full block");
    assert_eq!(block(&after, 1, 1), block(&before, 1, 1), "block 3 skipped through recent[1]");
    assert!(!d.next_frame(&smk).unwrap(), "two frames and no more");
    assert_eq!(d.frame(), Some(1));
}

#[test]
fn audio_is_a_first_sample_and_then_deltas() {
    let smk = Smk::parse(synthetic()).unwrap();
    let c = smk.audio_chunk(0, 0).unwrap().unwrap();
    assert_eq!(c.pcm, [0x80, 0x81, 0x82, 0x81]);
    assert_eq!(c.unpacked, 4);
    assert!(c.bits.1 - c.bits.0 < 8);
    assert_eq!(smk.audio_chunk(1, 0).unwrap(), None, "frame 1 carries no audio");
    assert_eq!(smk.audio(0).unwrap(), [0x80, 0x81, 0x82, 0x81]);
}

#[test]
fn stereo_reads_the_right_channel_first_and_interleaves_from_the_left() {
    let mut a = BitWriter::default();
    a.bit(1);
    a.bit(1); // stereo
    a.bit(0);
    tree8(&mut a, &[0x02]);
    tree8(&mut a, &[0xFE]);
    a.bits(0x10, 8); // right
    a.bits(0x20, 8); // left
    let mut raw = 6u32.to_le_bytes().to_vec();
    raw.extend_from_slice(&a.bytes);
    let desc = Track { rate: 11025, packed: true, bits16: false, stereo: true };
    let c = decode_audio(&raw, desc).unwrap();
    assert_eq!(c.pcm, [0x20, 0x10, 0x22, 0x0E, 0x24, 0x0C]);
}

#[test]
fn a_chunk_that_contradicts_its_track_is_refused() {
    let mut a = BitWriter::default();
    a.bit(1);
    a.bit(1); // stereo, in a mono track
    a.bit(0);
    let mut raw = 2u32.to_le_bytes().to_vec();
    raw.extend_from_slice(&a.bytes);
    let desc = Track { rate: 11025, packed: true, bits16: false, stereo: false };
    assert!(matches!(decode_audio(&raw, desc), Err(Error::BadTree(_))));
}

#[test]
fn a_truncated_file_says_where_it_ended() {
    let mut f = synthetic();
    f.truncate(HEADER_LEN + 4);
    assert!(matches!(Smk::parse(f), Err(Error::Truncated(_))));
    assert!(matches!(Smk::parse(b"RIFF".repeat(30)), Err(Error::Signature(_))));
}

