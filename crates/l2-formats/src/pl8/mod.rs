//! PL8 sprite containers.
//!
//! Layout (see `docs/formats/pl8.md` and `docs/formats/pl8-mode2.md`):
//!
//! ```text
//! 0x00  u8   storage family: 0 = raw, 1 = RLE, 2 = isometric map tiles
//! 0x01  u8   map zoom level: 0 -> 58x30, 1 -> 26x14, 2 -> 10x6
//! 0x02  u16  frame count
//! 0x04  u16  unknown
//! 0x06  u8   unknown
//! 0x07  u8   unknown (0..15)
//! then `frame count` records of 16 bytes:
//!   0x00 u16 width
//!   0x02 u16 height
//!   0x04 u32 absolute file offset of pixel data
//!   0x08 i16 canvas placement X
//!   0x0A i16 canvas placement Y
//!   0x0C u8  shape - see `Shape`
//!   0x0D u8  overhang row count
//!   0x0E u16 padding
//! ```
//!
//! The family byte alone does not determine the encoding. An isometric file
//! carries plain raw frames alongside diamonds; the per-frame `shape` byte is
//! what decides. Missing that is why "storage mode 2" resisted analysis for so
//! long -

mod decoder;
pub use decoder::*;

use crate::{u16_at, u32_at, Error, Result};

pub const HEADER_LEN: usize = 8;
pub const FRAME_RECORD_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    /// `width * height` bytes, row-major. Palette index 0 is transparent.
    Raw,
    /// Per-row runs: `0x00 n` skips n transparent pixels, `n` copies n literals.
    Rle,
    /// Isometric map tiles. Per-frame encoding comes from the `shape` byte.
    Isometric,
    Unknown(u8),
}

impl Storage {
    fn from_byte(b: u8) -> Self {
        match b {
            0 => Storage::Raw,
            1 => Storage::Rle,
            2 => Storage::Isometric,
            other => Storage::Unknown(other),
        }
    }

    pub fn is_supported(self) -> bool {
        !matches!(self, Storage::Unknown(_))
    }
}

/// Per-frame encoding, from record byte `0x0C`. Only meaningful for
/// [`Storage::Isometric`] files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Plain raw rectangle, `width * height` bytes, even inside an iso file.
    Rect,
    /// Diamond only: `height^2` bytes. Ignores the overhang row count.
    Diamond,
    /// Diamond plus full-width chevron overhang: `height^2 + rows * width`.
    DiamondFull,
    /// Diamond plus left-half chevron overhang: `height^2 + rows * height`.
    DiamondLeft,
    /// Diamond plus right-half chevron overhang: `height^2 + rows * height`.
    DiamondRight,
    /// Not artwork: a mouse hit-test region map at 1/8 resolution, one byte per
/// 8x8 screen block, holding region ids. The
/// engine only ever reads these from region code.
    ///
    /// Detected structurally, not by filename: the frame's declared byte span
    /// is exactly `(width/8) * (height/8)`.
    RegionMap,
    Unknown(u8),
}

impl Shape {
    fn from_byte(b: u8) -> Self {
        match b {
            0 => Shape::Rect,
            1 => Shape::Diamond,
            2 => Shape::DiamondFull,
            3 => Shape::DiamondLeft,
            4 => Shape::DiamondRight,
            other => Shape::Unknown(other),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub width: u16,
    pub height: u16,
    pub offset: u32,
    /// Where the frame sits on the 640x480 canvas.
    pub x: i16,
    pub y: i16,
    /// Per-frame encoding (record byte `0x0C`), possibly reclassified after
    /// parsing - see [`Shape::RegionMap`].
    pub shape: Shape,
    /// The raw value of record byte `0x0C`, before any reclassification.
    pub shape_byte: u8,
    /// Chevron overhang rows (record byte `0x0D`). Ignored by [`Shape::Diamond`].
    pub overhang_rows: u8,
    /// The raw 8 bytes from `0x08`, kept for diagnostics.
    pub trailing: [u8; 8],
}

/// A frame's pixels on its own canvas.
///
/// **The canvas already includes the overhang rows, always.** For a shape-0
/// rectangle declaring `overhang_rows`, `height` is the record's height plus
/// that count and the rectangle sits at canvas row `overhang_rows`; the rows
/// above it are either the frame's stored artwork or transparent. That holds
/// whether or not the file bothered to store them,
/// add `overhang_rows` again — doing so drew the whole `overhang = 3` half of
/// `Fntl2_14.pl8` three pixels below the baseline, which a player caught by
/// comparing our text with the original's.
///
/// What the canvas does not decide is where its *anchor* is,
/// consumers differ: `Glyph_Draw` puts the canvas top at the line top, while
/// the map blitter anchors the tile and lets the chevrons extrude above it
/// (`l2_view::campaign` subtracts `height - tile_h`). Both read the same
/// canvas; only the caller knows which edge it is placing.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub width: u16,
    /// Record height **plus** any reserved overhang rows - see above.
    pub height: u16,
    /// Palette indices, row-major, `width * height` entries.
    pub indices: Vec<u8>,
/// Per-pixel coverage, i.e. whether the game would paint this
    /// pixel. Two things make a pixel transparent: an RLE skip run,
    /// palette index of 0 - every blitter copies only non-zero bytes
    /// (verified in the original at 0x004B43B1).
    pub opaque: Vec<bool>,
}

/// A parsed PL8 file. Borrows the caller's bytes.
pub struct Pl8<'a> {
    data: &'a [u8],
    pub storage: Storage,
    /// Header byte 1. For isometric files this is the map zoom level:
    /// 0 -> 58x30 tiles, 1 -> 26x14, 2 -> 10x6.
    pub zoom: u8,
    pub frames: Vec<FrameInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assembles a PL8 in memory: header, frame table with offsets filled in to
/// match, then the payloads laid end to end. Synthesised
    /// in as a fixture because no game data may live in this repository - and
    /// because the corpus test skips entirely without `LORDS2_DIR`, so these are
    /// the only PL8 tests that run on a bare checkout.
    fn build(storage: u8, sub: u8, frames: &[(u16, u16, &[u8])]) -> Vec<u8> {
        let mut out = vec![storage, sub, 0, 0, 0, 0, 0, 0];
        out[2..4].copy_from_slice(&(frames.len() as u16).to_le_bytes());
        let mut off = HEADER_LEN + frames.len() * FRAME_RECORD_LEN;
        for (w, h, data) in frames {
            out.extend_from_slice(&w.to_le_bytes());
            out.extend_from_slice(&h.to_le_bytes());
            out.extend_from_slice(&(off as u32).to_le_bytes());
            out.extend_from_slice(&[0xaa, 0xbb, 0, 0, 0, 0, 0, 0]);
            off += data.len();
        }
        for (_, _, data) in frames {
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn raw_frames_are_row_major_with_index_0_transparent() {
        let bytes = build(0, 0, &[(3, 2, &[1, 2, 3, 4, 5, 6]), (2, 2, &[7, 0, 9, 0])]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(pl8.storage, Storage::Raw);
        assert_eq!(pl8.frames.len(), 2);
        assert_eq!(pl8.frames[0].offset as usize, HEADER_LEN + 2 * FRAME_RECORD_LEN);
        // The trailing bytes are carried through, not silently dropped.
        assert_eq!(pl8.frames[0].trailing, [0xaa, 0xbb, 0, 0, 0, 0, 0, 0]);

        let f = pl8.decode(0).unwrap();
        assert_eq!(f.indices, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(f.opaque, vec![true; 6]);
        // Index 0 inside a raw frame is a hole, not a black pixel.
        let f1 = pl8.decode(1).unwrap();
        assert_eq!(f1.indices, vec![7, 0, 9, 0]);
        assert_eq!(f1.opaque, vec![true, false, true, false]);
        pl8.validate().unwrap();
    }

    #[test]
    fn rle_skip_runs_decode_to_transparent_pixels() {
        // Frame 0, 4x2:
        //   row 0: skip 1, then literals 0a 0b 0c
        //   row 1: literals 0d 0e, then skip 2
        // Frame 1, 2x1: a single skip covering the whole row - fully transparent.
        let f0: &[u8] = &[0x00, 0x01, 0x03, 0x0a, 0x0b, 0x0c, 0x02, 0x0d, 0x0e, 0x00, 0x02];
        let f1: &[u8] = &[0x00, 0x02];
        let bytes = build(1, 0, &[(4, 2, f0), (2, 1, f1)]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(pl8.storage, Storage::Rle);

        let f = pl8.decode(0).unwrap();
        assert_eq!(f.indices, vec![0, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0, 0]);
        assert_eq!(
            f.opaque,
            vec![false, true, true, true, true, true, false, false]
        );

        let f = pl8.decode(1).unwrap();
        assert_eq!(f.indices, vec![0, 0]);
        assert_eq!(f.opaque, vec![false, false]);

        pl8.validate().unwrap();
    }

    #[test]
    fn zero_length_skip_run_is_rejected_rather_than_hanging() {
        // "00 00" asks to skip zero pixels: no progress,
        // spins forever. Must be an error instead.
        //
        // The payload is deliberately 5 bytes for a 4x1 frame. At exactly 4 it
        // would span w*h,
        // real hazard this test caught, since that would quietly rescue a frame
        // that ought to fail.
        let bytes = build(1, 0, &[(4, 1, &[0x00, 0x00, 0x00, 0x04, 0x00])]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert!(matches!(
            pl8.decode(0),
            Err(crate::Error::ZeroLengthRun { frame: 0, row: 0 })
        ));
    }

    #[test]
    fn frame_must_end_exactly_where_the_next_begins() {
        let f0: &[u8] = &[0x00, 0x01, 0x03, 0x0a, 0x0b, 0x0c, 0x02, 0x0d, 0x0e, 0x00, 0x02];
        let mut bytes = build(1, 0, &[(4, 2, f0), (2, 1, &[0x00, 0x02])]);
// Push frame 1 one byte later than frame 0 ends. This is the
        // invariant the whole corpus check rests on, so it must really bite.
        let rec = HEADER_LEN + FRAME_RECORD_LEN;
        let moved = u32::from_le_bytes([bytes[rec + 4], bytes[rec + 5], bytes[rec + 6], bytes[rec + 7]]) + 1;
        bytes[rec + 4..rec + 8].copy_from_slice(&moved.to_le_bytes());

        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(
            pl8.validate().unwrap_err(),
            Error::FrameSizeMismatch { frame: 0, ended: 51, expected: 52 }
        );
    }

    #[test]
    fn rle_row_may_not_consume_more_than_width_pixels() {
        let bytes = build(1, 0, &[(2, 1, &[0x03, 1, 2, 3])]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(
            pl8.decode(0).unwrap_err(),
            Error::RowOverrun { frame: 0, row: 0, got: 3, want: 2 }
        );
    }

    #[test]
    fn unknown_storage_is_refused_rather_than_guessed() {
        let bytes = build(7, 0, &[(2, 2, &[1, 2, 3, 4])]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert!(!pl8.is_supported());
        assert_eq!(pl8.decode(0).unwrap_err(), Error::UnsupportedStorage(7));
    }

    /// As `build`, but sets each frame's shape and overhang-row bytes.
    fn build_iso(zoom: u8, frames: &[(u16, u16, u8, u8, &[u8])]) -> Vec<u8> {
        let mut out = vec![2, zoom, 0, 0, 0, 0, 0, 0];
        out[2..4].copy_from_slice(&(frames.len() as u16).to_le_bytes());
        let mut off = HEADER_LEN + frames.len() * FRAME_RECORD_LEN;
        for (w, h, shape, rows, data) in frames {
            out.extend_from_slice(&w.to_le_bytes());
            out.extend_from_slice(&h.to_le_bytes());
            out.extend_from_slice(&(off as u32).to_le_bytes());
            out.extend_from_slice(&[0, 0, 0, 0, *shape, *rows, 0, 0]);
            off += data.len();
        }
        for (_, _, _, _, data) in frames {
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn isometric_diamond_expands_to_centred_rows() {
        // 6x4 diamond: row widths 2, 6, 6, 2 - total 16 = height^2.
        let data: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let bytes = build_iso(0, &[(6, 4, 1, 0, data)]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(pl8.storage, Storage::Isometric);
        assert_eq!(pl8.frames[0].shape, Shape::Diamond);

        let f = pl8.decode(0).unwrap();
        assert_eq!((f.width, f.height), (6, 4));
        assert_eq!(
            f.indices,
            vec![
                0, 0, 1, 2, 0, 0, //
                3, 4, 5, 6, 7, 8, //
                9, 10, 11, 12, 13, 14, //
                0, 0, 15, 16, 0, 0,
            ]
        );
//
        assert_eq!(&f.opaque[0..2], &[false, false]);
        assert_eq!(&f.opaque[6..12], &[true; 6]);
        pl8.validate().unwrap();
    }

    #[test]
    fn a_diamond_ignores_its_overhang_row_count() {
        // 32 frames in the real corpus declare rows on shape 1 and still hold
        // exactly h^2 bytes. Honouring the count there desynchronises the file.
        let data: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let bytes = build_iso(0, &[(6, 4, 1, 3, data)]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(pl8.frames[0].overhang_rows, 3);
        assert_eq!(pl8.decode(0).unwrap().height, 4);
        pl8.validate().unwrap();
    }

    #[test]
    fn an_iso_file_may_hold_plain_rectangles() {
        // This is why "storage mode 2" resisted analysis: the family byte says
        // isometric, but the per-frame shape byte says raw rectangle.
        let bytes = build_iso(0, &[(3, 2, 0, 0, &[1, 2, 3, 4, 5, 6])]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(pl8.frames[0].shape, Shape::Rect);
        assert_eq!(pl8.decode(0).unwrap().indices, vec![1, 2, 3, 4, 5, 6]);
        pl8.validate().unwrap();
    }

    #[test]
    fn iso_geometry_must_be_a_legal_diamond() {
        let bytes = build_iso(0, &[(5, 4, 1, 0, &[0; 16])]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(
            pl8.decode(0).unwrap_err(),
            Error::BadIsoGeometry { frame: 0, width: 5, height: 4 }
        );
    }

    #[test]
    fn short_buffers_error_instead_of_panicking() {
        assert_eq!(
            Pl8::parse(&[1, 0, 1]).err(),
            Some(Error::Truncated { needed: HEADER_LEN, have: 3 })
        );
        // Header claims two frames but only one record follows.
        let mut bytes = build(1, 0, &[(1, 1, &[0x01, 0x07])]);
        bytes[2] = 2;
        assert_eq!(
            Pl8::parse(&bytes).err(),
            Some(Error::Truncated { needed: HEADER_LEN + 2 * FRAME_RECORD_LEN, have: 26 })
        );
    }
}

