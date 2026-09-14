//! **A film written by hand, bit by bit, from the format description.**
//!
//! These run everywhere, CI included, which the corpus tests cannot. Read them
//! for what they are: an *encoder* written from the same description as the
//! decoder, so agreement between the two proves the description was applied
//! consistently and nothing about whether it was read right. That second
//! question belongs to `tests/corpus.rs`, where the shipped films and an
//! independent decoder's numbers are the other side of the comparison.

use std::collections::BTreeMap;

use super::*;

/// The bit order every Smacker stream uses: least significant first.
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    len: usize,
}

impl BitWriter {
    pub(super) fn bit(&mut self, b: u32) {
        if self.len % 8 == 0 {
            self.bytes.push(0);
        }
        if b & 1 != 0 {
            *self.bytes.last_mut().unwrap() |= 1 << (self.len % 8);
        }
        self.len += 1;
    }
    pub(super) fn bits(&mut self, v: u32, n: u32) {
        for i in 0..n {
            self.bit((v >> i) & 1);
        }
    }
    fn path(&mut self, p: &[u8]) {
        for &b in p {
            self.bit(b as u32);
        }
    }
}

type Codes = BTreeMap<u32, Vec<u8>>;

/// A balanced tree over `leaves`, written depth-first; returns each leaf's
/// path. `leaf` writes a leaf's value.
fn emit(w: &mut BitWriter, leaves: &[u32], prefix: Vec<u8>, codes: &mut Codes, leaf: &dyn Fn(&mut BitWriter, u32)) {
    if leaves.len() == 1 {
        w.bit(0);
        leaf(w, leaves[0]);
        codes.insert(leaves[0], prefix);
        return;
    }
    w.bit(1);
    let (a, b) = leaves.split_at(leaves.len() / 2);
    let mut zero = prefix.clone();
    zero.push(0);
    emit(w, a, zero, codes, leaf);
    let mut one = prefix;
    one.push(1);
    emit(w, b, one, codes, leaf);
}

fn tree8(w: &mut BitWriter, leaves: &[u32]) -> Codes {
    let mut codes = Codes::new();
    w.bit(1);
    emit(w, leaves, Vec::new(), &mut codes, &|w, v| w.bits(v, 8));
    w.bit(0);
    codes
}

fn distinct(v: impl Iterator<Item = u32>) -> Vec<u32> {
    let mut s: Vec<u32> = v.collect();
    s.sort();
    s.dedup();
    s
}

/// A 16-bit tree over `leaves` with three escapes; returns the codes by
/// 16-bit value.
fn tree16(w: &mut BitWriter, leaves: &[u32], escapes: [u32; 3]) -> Codes {
    w.bit(1);
    let lows = tree8(w, &distinct(leaves.iter().map(|v| v & 0xFF)));
    let highs = tree8(w, &distinct(leaves.iter().map(|v| v >> 8)));
    for e in escapes {
        w.bits(e, 16);
    }
    let mut codes = Codes::new();
    emit(w, leaves, Vec::new(), &mut codes, &|w, v| {
        w.path(&lows[&(v & 0xFF)]);
        w.path(&highs[&(v >> 8)]);
    });
    w.bit(0);
    codes
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

const ESC: [u32; 3] = [0xFFF0, 0xFFF1, 0xFFF2];

/// An 8 × 8 film, two frames, one mono audio track: every block type, every
/// palette opcode, the recency cache and the snapshot the palette copies from.
fn synthetic() -> Vec<u8> {
    // The four trees.
    let mut t = BitWriter::default();
    let mmap = tree16(&mut t, &[0x8001], ESC);
    let mclr = tree16(&mut t, &[0x0907], ESC);
    let full = tree16(&mut t, &[0x0B0A, 0x0D0C], ESC);
    let types = tree16(&mut t, &[0x0000, 0x0001, 0x0002, 0x0503, 0xFFF0, 0xFFF1, 0xFFF2], ESC);
    let trees = t.bytes;

    // Frame 0: palette, audio, then solid / mono / full / skip.
    let mut f0 = Vec::new();
    let pal0: Vec<u8> = vec![
        0x3F, 0x00, 0x00, // entry 0 = (63, 0, 0)
        0x00, 0x10, 0x00, // entry 1 = (0, 16, 0)
        0x81, //             skip entries 2 and 3
        0x40, 0x00, //       entry 4 = old entry 0, which is still black
        0x01, 0x02, 0x03, // entry 5
        0x80, 0x80, 0x80, // three one-entry skips: padding the chunk to 16
    ];
    f0.push(((pal0.len() + 1) / 4) as u8);
    f0.extend_from_slice(&pal0);
    let mut a = BitWriter::default();
    a.bit(1); // data present
    a.bit(0); // mono
    a.bit(0); // 8-bit
    let deltas = tree8(&mut a, &[0x01, 0xFF]);
    a.bits(0x80, 8);
    for d in [0x01, 0x01, 0xFF] {
        a.path(&deltas[&d]);
    }
    let mut audio = 4u32.to_le_bytes().to_vec();
    audio.extend_from_slice(&a.bytes);
    f0.extend_from_slice(&((audio.len() + 4) as u32).to_le_bytes());
    f0.extend_from_slice(&audio);
    let mut v = BitWriter::default();
    v.path(&types[&0x0503]);
    v.path(&types[&0x0000]);
    v.path(&mclr[&0x0907]);
    v.path(&mmap[&0x8001]);
    v.path(&types[&0x0001]);
    for _ in 0..4 {
        v.path(&full[&0x0B0A]);
        v.path(&full[&0x0D0C]);
    }
    v.path(&types[&0x0002]);
    f0.extend_from_slice(&v.bytes);
    pad4(&mut f0);

    // Frame 1: a palette that copies over itself, then skip / recent[0] /
    // solid / recent[1].
    let mut f1 = Vec::new();
    let pal1: Vec<u8> = vec![
        0x00, 0x00, 0x3F, // entry 0 = blue
        0x40, 0x00, //       entry 1 = OLD entry 0, which is red, not blue
        0x80, 0x80, //       padding
    ];
    f1.push(((pal1.len() + 1) / 4) as u8);
    f1.extend_from_slice(&pal1);
    let mut v = BitWriter::default();
    v.path(&types[&0x0002]); // skip; recent = [2, 0, 0]
    v.path(&types[&ESC[0]]); // recent[0] = skip
    v.path(&types[&0x0503]); // solid 5; recent = [0x503, 2, 0]
    v.path(&types[&ESC[1]]); // recent[1] = skip
    f1.extend_from_slice(&v.bytes);
    pad4(&mut f1);

    let mut file = Vec::new();
    file.extend_from_slice(b"SMK2");
    for x in [8u32, 8, 2] {
        file.extend_from_slice(&x.to_le_bytes());
    }
    file.extend_from_slice(&(-8333i32).to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    file.extend_from_slice(&4u32.to_le_bytes());
    file.extend_from_slice(&[0u8; 24]);
    file.extend_from_slice(&(trees.len() as u32).to_le_bytes());
    file.extend_from_slice(&[0u8; 16]);
    file.extend_from_slice(&0x8000_2B11u32.to_le_bytes());
    file.extend_from_slice(&[0u8; 24]);
    file.extend_from_slice(&[0u8; 4]);
    assert_eq!(file.len(), HEADER_LEN);
    file.extend_from_slice(&(f0.len() as u32).to_le_bytes());
    file.extend_from_slice(&(f1.len() as u32).to_le_bytes());
    file.extend_from_slice(&[3, 1]);
    file.extend_from_slice(&trees);
    file.extend_from_slice(&f0);
    file.extend_from_slice(&f1);
    file
}

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
    // Block 0, solid colour 5.
    assert!((0..4).all(|y| (0..4).all(|x| at(x, y) == 5)));
    // Block 1, mono: bit 0 is the top-left pixel and bit 15 the bottom-right.
    assert_eq!(at(4, 0), 9, "map bit 0 set: the high colour");
    assert_eq!(at(5, 0), 7, "map bit 1 clear: the low colour");
    assert_eq!(at(7, 3), 9, "map bit 15 set");
    assert_eq!(at(6, 3), 7);
    // Block 2, full: the first code of a row is columns 2 and 3.
    for y in 4..8 {
        assert_eq!([at(0, y), at(1, y), at(2, y), at(3, y)], [0x0C, 0x0D, 0x0A, 0x0B]);
    }
    // Block 3, skip, on the first frame: still zero.
    assert!((4..8).all(|y| (4..8).all(|x| at(x, y) == 0)));
    let (read, total) = d.video_bits();
    assert!(total - read < 8, "{read} of {total} bits read");
}

/// **`_SmackToBuffer@28` (`0x403AF0`): flag `0x02` writes the even rows and
/// leaves the odd ones as the cleared buffer; `0x04` writes each row twice.**
/// Ablation: copy the row for `Interlace` and the black rows go.
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
    // A tree with one leaf is a code of no bits at all.
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
