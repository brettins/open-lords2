
mod decoder;
pub use decoder::*;

use crate::{u16_at, u32_at, Error, Result};

pub const HEADER_LEN: usize = 8;
pub const FRAME_RECORD_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    Raw,
    Rle,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Rect,
    Diamond,
    DiamondFull,
    DiamondLeft,
    DiamondRight,
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
    pub x: i16,
    pub y: i16,
    pub shape: Shape,
    pub shape_byte: u8,
    pub overhang_rows: u8,
    pub trailing: [u8; 8],
}

#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub width: u16,
    pub height: u16,
    pub indices: Vec<u8>,
/// Per-pixel coverage, i.e. whether the game would paint this
    /// pixel. Two things make a pixel transparent: an RLE skip run,
    /// palette index of 0 - every blitter copies only non-zero bytes
    /// (verified in the original at 0x004B43B1).
    pub opaque: Vec<bool>,
}

pub struct Pl8<'a> {
    data: &'a [u8],
    pub storage: Storage,
    pub zoom: u8,
    pub frames: Vec<FrameInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(pl8.frames[0].trailing, [0xaa, 0xbb, 0, 0, 0, 0, 0, 0]);

        let f = pl8.decode(0).unwrap();
        assert_eq!(f.indices, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(f.opaque, vec![true; 6]);
        let f1 = pl8.decode(1).unwrap();
        assert_eq!(f1.indices, vec![7, 0, 9, 0]);
        assert_eq!(f1.opaque, vec![true, false, true, false]);
        pl8.validate().unwrap();
    }

    #[test]
    fn rle_skip_runs_decode_to_transparent_pixels() {
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
        assert_eq!(&f.opaque[0..2], &[false, false]);
        assert_eq!(&f.opaque[6..12], &[true; 6]);
        pl8.validate().unwrap();
    }

    #[test]
    fn a_diamond_ignores_its_overhang_row_count() {
        let data: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let bytes = build_iso(0, &[(6, 4, 1, 3, data)]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert_eq!(pl8.frames[0].overhang_rows, 3);
        assert_eq!(pl8.decode(0).unwrap().height, 4);
        pl8.validate().unwrap();
    }

    #[test]
    fn an_iso_file_may_hold_plain_rectangles() {
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
        let mut bytes = build(1, 0, &[(1, 1, &[0x01, 0x07])]);
        bytes[2] = 2;
        assert_eq!(
            Pl8::parse(&bytes).err(),
            Some(Error::Truncated { needed: HEADER_LEN + 2 * FRAME_RECORD_LEN, have: 26 })
        );
    }
}

