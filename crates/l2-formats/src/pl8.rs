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
//! long - there was never one mode-2 codec to find.

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
    /// 8x8 screen block, holding region ids rather than palette indices. The
    /// engine only ever reads these from region code, never from a blitter.
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

#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub width: u16,
    pub height: u16,
    /// Palette indices, row-major, `width * height` entries.
    pub indices: Vec<u8>,
    /// Per-pixel coverage, i.e. whether the game would actually paint this
    /// pixel. Two things make a pixel transparent: an RLE skip run, and a
    /// palette index of 0 - every blitter copies only non-zero bytes
    /// (verified in the original at 0x004B43B1).
    pub opaque: Vec<bool>,
}

/// A parsed PL8 file. Borrows the caller's bytes rather than copying them.
pub struct Pl8<'a> {
    data: &'a [u8],
    pub storage: Storage,
    /// Header byte 1. For isometric files this is the map zoom level:
    /// 0 -> 58x30 tiles, 1 -> 26x14, 2 -> 10x6.
    pub zoom: u8,
    pub frames: Vec<FrameInfo>,
}

impl<'a> Pl8<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() < HEADER_LEN {
            return Err(Error::Truncated { needed: HEADER_LEN, have: data.len() });
        }
        let storage = Storage::from_byte(data[0]);
        let zoom = data[1];
        let count = u16_at(data, 2)? as usize;

        let mut frames = Vec::with_capacity(count);
        for i in 0..count {
            let rec = HEADER_LEN + i * FRAME_RECORD_LEN;
            let end = rec + FRAME_RECORD_LEN;
            if end > data.len() {
                return Err(Error::Truncated { needed: end, have: data.len() });
            }
            let mut trailing = [0u8; 8];
            trailing.copy_from_slice(&data[rec + 8..end]);
            frames.push(FrameInfo {
                width: u16_at(data, rec)?,
                height: u16_at(data, rec + 2)?,
                offset: u32_at(data, rec + 4)?,
                x: u16_at(data, rec + 8)? as i16,
                y: u16_at(data, rec + 10)? as i16,
                shape: Shape::from_byte(data[rec + 12]),
                shape_byte: data[rec + 12],
                overhang_rows: data[rec + 13],
                trailing,
            });
        }
        // Reclassify hit-test region maps. Their shape byte says "rectangle",
        // but they hold one byte per 8x8 block, so a rectangle read runs off the
        // end of the file. Decided on the byte span rather than the filename.
        for i in 0..frames.len() {
            // RLE frames have variable-length data and no meaningful shape byte,
            // so a coincidental span must not reclassify one.
            if storage == Storage::Rle || frames[i].shape != Shape::Rect {
                continue;
            }
            let (w, h) = (frames[i].width as usize, frames[i].height as usize);
            let start = frames[i].offset as usize;
            let next = frames
                .get(i + 1)
                .map(|f| f.offset as usize)
                .unwrap_or(data.len());
            if w >= 8 && h >= 8 && next > start && next - start == (w / 8) * (h / 8) {
                frames[i].shape = Shape::RegionMap;
            }
        }

        // A file that declares RLE but whose every frame spans exactly w*h is
        // stored raw: the header byte is simply wrong. `Font_c2.pl8` is the same
        // font as `Fntl2_9.pl8`, exported twice, and shares 103 of its 108 frame
        // records. Nothing in the engine reads the family byte, so a wrong one is
        // invisible to the game.
        //
        // Decided over the *whole file*. Deciding per frame would let a single
        // coincidental span reinterpret one frame of an otherwise valid RLE file
        // - including a genuinely corrupt frame that should have raised an error.
        let storage = if storage == Storage::Rle
            && !frames.is_empty()
            && frames.iter().enumerate().all(|(i, f)| {
                let next = frames
                    .get(i + 1)
                    .map(|n| n.offset as usize)
                    .unwrap_or(data.len());
                next.checked_sub(f.offset as usize)
                    == Some(f.width as usize * f.height as usize)
            }) {
            Storage::Raw
        } else {
            storage
        };

        Ok(Pl8 { data, storage, zoom, frames })
    }

    /// Where frame `i`'s pixel data must end: the next frame's offset, or EOF
    /// for the last frame. This is what makes the format self-verifying.
    pub fn frame_boundary(&self, i: usize) -> usize {
        self.frames
            .get(i + 1)
            .map(|f| f.offset as usize)
            .unwrap_or(self.data.len())
    }

    pub fn decode(&self, index: usize) -> Result<DecodedFrame> {
        self.decode_inner(index).map(|(frame, _end)| frame)
    }

    fn decode_inner(&self, index: usize) -> Result<(DecodedFrame, usize)> {
        let info = self.frames.get(index).ok_or(Error::FrameOutOfRange {
            index,
            count: self.frames.len(),
        })?;

        let (w, h) = (info.width as usize, info.height as usize);
        let start = info.offset as usize;

        let boundary = self.frame_boundary(index);

        // Two things extrude a frame *upward*, and both are decided per frame
        // rather than by the file header:
        //
        //   * an isometric diamond of shape 2-4 appends chevron records
        //   * a plain rectangle may store extra RLE rows above itself
        //
        // Shape::Diamond ignores the overhang count even when non-zero - 24
        // frames in the corpus declare rows and still occupy exactly h^2.
        let rows = info.overhang_rows as usize;
        let iso_overhang = matches!(
            info.shape,
            Shape::DiamondFull | Shape::DiamondLeft | Shape::DiamondRight
        );
        // A rectangle only carries stored overhang when the bytes are really
        // there; some frames declare rows and store nothing. Decide structurally
        // - does the bare rectangle land exactly on the next frame's offset?
        let rect_overhang = info.shape == Shape::Rect
            && rows > 0
            && self.storage != Storage::Rle
            && start + w * h != boundary;

        let overhang = if iso_overhang || rect_overhang { rows } else { 0 };
        // A region map is stored at 1/8 resolution, so its canvas is not the
        // record's width and height.
        let (canvas_w, canvas_h) = if info.shape == Shape::RegionMap {
            (w / 8, h / 8)
        } else {
            (w, h + overhang)
        };
        let mut indices = vec![0u8; canvas_w * canvas_h];
        let mut opaque = vec![false; canvas_w * canvas_h];

        if info.shape == Shape::RegionMap {
            let n = canvas_w * canvas_h;
            let end = start + n;
            if end > self.data.len() {
                return Err(Error::Truncated { needed: end, have: self.data.len() });
            }
            indices.copy_from_slice(&self.data[start..end]);
            // Deliberately left fully transparent: these bytes are region ids,
            // not palette indices, and the engine never blits them.
            return Ok((
                DecodedFrame {
                    width: canvas_w as u16,
                    height: canvas_h as u16,
                    indices,
                    opaque,
                },
                end,
            ));
        }

        if let Storage::Unknown(m) = self.storage {
            return Err(Error::UnsupportedStorage(m));
        }

        // The header family byte is very nearly vestigial: `Pl8_DrawFrame`
        // (0x0040A21A) indexes straight to `buf + frame*0x10 + 8` and never
        // reads bytes 0 or 1, so the engine cannot see them. Base2a and Base2b
        // differ in exactly one byte of their 2,248-byte header and frame table
        // - byte 0 - with identical frame records. So dispatch on the per-frame
        // shape byte for every family, not just for isometric files.
        let end = if self.storage == Storage::Rle {
            self.decode_rle(index, info, &mut indices, &mut opaque)?
        } else {
            match info.shape {
                Shape::Rect => {
                    let after = self.decode_rect(start, w, h, overhang, &mut indices, &mut opaque)?;
                    if overhang > 0 {
                        self.decode_rle_rows(index, after, w, overhang, &mut indices, &mut opaque)?
                    } else {
                        after
                    }
                }
                Shape::Diamond
                | Shape::DiamondFull
                | Shape::DiamondLeft
                | Shape::DiamondRight => {
                    self.decode_iso(index, info, overhang, &mut indices, &mut opaque)?
                }
                // Region maps take the early return above, so this arm exists
                // only to keep the match exhaustive.
                Shape::RegionMap | Shape::Unknown(_) => {
                    return Err(Error::UnsupportedShape {
                        frame: index,
                        shape: info.shape_byte,
                    })
                }
            }
        };

        Ok((
            DecodedFrame {
                width: info.width,
                height: canvas_h as u16,
                indices,
                opaque,
            },
            end,
        ))
    }

    /// A plain `width * height` rectangle of palette indices, written starting
    /// at canvas row `dest_row` — non-zero when overhang rows sit above it.
    fn decode_rect(
        &self,
        start: usize,
        w: usize,
        h: usize,
        dest_row: usize,
        indices: &mut [u8],
        opaque: &mut [bool],
    ) -> Result<usize> {
        let n = w * h;
        let end = start + n;
        if end > self.data.len() {
            return Err(Error::Truncated { needed: end, have: self.data.len() });
        }
        let at = dest_row * w;
        indices[at..at + n].copy_from_slice(&self.data[start..end]);
        for i in 0..n {
            opaque[at + i] = indices[at + i] != 0;
        }
        Ok(end)
    }

    /// `rows` RLE-encoded rows stored *after* a rectangle but drawn *above* it.
    ///
    /// Real artwork, contiguous with the rectangle — the accent on a glyph, the
    /// sloped top edge of a hill tile. `FUN_00402A14` shifts the destination
    /// down by this row count before clipping, reserving exactly these rows.
    fn decode_rle_rows(
        &self,
        index: usize,
        mut p: usize,
        w: usize,
        rows: usize,
        indices: &mut [u8],
        opaque: &mut [bool],
    ) -> Result<usize> {
        let next = |p: &mut usize| -> Result<u8> {
            let b = *self.data.get(*p).ok_or(Error::Truncated {
                needed: *p + 1,
                have: self.data.len(),
            })?;
            *p += 1;
            Ok(b)
        };
        for y in 0..rows {
            let mut x: u32 = 0;
            while x < w as u32 {
                let op = next(&mut p)?;
                if op == 0 {
                    let skip = next(&mut p)?;
                    if skip == 0 {
                        return Err(Error::ZeroLengthRun { frame: index, row: y as u16 });
                    }
                    x += skip as u32;
                } else {
                    for k in 0..op as usize {
                        let v = next(&mut p)?;
                        let xi = x as usize + k;
                        if xi < w {
                            indices[y * w + xi] = v;
                            opaque[y * w + xi] = v != 0;
                        }
                    }
                    x += op as u32;
                }
            }
            if x != w as u32 {
                return Err(Error::RowOverrun {
                    frame: index,
                    row: y as u16,
                    got: x,
                    want: w as u16,
                });
            }
        }
        Ok(p)
    }


    fn decode_rle(
        &self,
        index: usize,
        info: &FrameInfo,
        indices: &mut [u8],
        opaque: &mut [bool],
    ) -> Result<usize> {
        let (w, h) = (info.width as usize, info.height as usize);
        let mut p = info.offset as usize;
        let next = |p: &mut usize| -> Result<u8> {
            let b = *self.data.get(*p).ok_or(Error::Truncated {
                needed: *p + 1,
                have: self.data.len(),
            })?;
            *p += 1;
            Ok(b)
        };
        for y in 0..h {
            let mut x: u32 = 0;
            while x < info.width as u32 {
                let op = next(&mut p)?;
                if op == 0 {
                    let skip = next(&mut p)?;
                    if skip == 0 {
                        return Err(Error::ZeroLengthRun { frame: index, row: y as u16 });
                    }
                    x += skip as u32;
                } else {
                    for k in 0..op as usize {
                        let v = next(&mut p)?;
                        let xi = x as usize + k;
                        if xi < w {
                            indices[y * w + xi] = v;
                            opaque[y * w + xi] = v != 0;
                        }
                    }
                    x += op as u32;
                }
            }
            if x != info.width as u32 {
                return Err(Error::RowOverrun {
                    frame: index,
                    row: y as u16,
                    got: x,
                    want: info.width,
                });
            }
        }
        Ok(p)
    }

    /// Isometric diamond, optionally with a chevron overhang above it.
    ///
    /// Row `r` of the diamond holds `2 + 4*r` pixels in the top half and
    /// `2 + 4*(h-1-r)` in the bottom, centred - so widths run
    /// `2, 6, 10, ..., w, w, ..., 6, 2` and total exactly `h^2`. Only the
    /// pixels inside the diamond are stored; there are no control bytes.
    ///
    /// Each overhang record is a *chevron* tracing the diamond's own upper
    /// silhouette rather than a horizontal row, and record `i` paints one
    /// screen row higher than the last. That is how the engine extrudes
    /// mountains and cliffs upward without storing a bounding rectangle.
    fn decode_iso(
        &self,
        index: usize,
        info: &FrameInfo,
        overhang: usize,
        indices: &mut [u8],
        opaque: &mut [bool],
    ) -> Result<usize> {
        let (w, h) = (info.width as usize, info.height as usize);
        if h == 0 || h % 2 != 0 || w + 2 != 2 * h {
            return Err(Error::BadIsoGeometry {
                frame: index,
                width: info.width,
                height: info.height,
            });
        }
        let hh = h / 2;
        let canvas_h = h + overhang;
        let mut p = info.offset as usize;

        for r in 0..h {
            let rw = if r < hh { 2 + 4 * r } else { 2 + 4 * (h - 1 - r) };
            let x0 = (w - rw) / 2;
            for i in 0..rw {
                let v = *self.data.get(p).ok_or(Error::Truncated {
                    needed: p + 1,
                    have: self.data.len(),
                })?;
                p += 1;
                let at = (overhang + r) * w + x0 + i;
                indices[at] = v;
                opaque[at] = v != 0;
            }
        }

        if overhang > 0 {
            // Pair `m` sits at column `2*m`, on silhouette row |hh-1-m|.
            let (m_first, m_count) = match info.shape {
                Shape::DiamondFull => (0usize, w / 2),
                Shape::DiamondLeft => (0usize, hh),
                Shape::DiamondRight => (hh - 1, hh),
                _ => (0usize, 0usize),
            };
            for i in 0..overhang {
                for k in 0..m_count {
                    let m = m_first + k;
                    let row = (hh as isize - 1 - m as isize).unsigned_abs();
                    let cy = overhang + row - (i + 1);
                    for d in 0..2 {
                        let v = *self.data.get(p).ok_or(Error::Truncated {
                            needed: p + 1,
                            have: self.data.len(),
                        })?;
                        p += 1;
                        let x = 2 * m + d;
                        if x < w && cy < canvas_h {
                            let at = cy * w + x;
                            indices[at] = v;
                            opaque[at] = v != 0;
                        }
                    }
                }
            }
        }
        Ok(p)
    }

    /// Whether this file can be decoded.
    /// Currently equivalent to the storage check; kept as a file-level hook for
    /// when sub-mode turns out to matter.
    pub fn is_supported(&self) -> bool {
        self.storage.is_supported()
    }

    /// Decode every frame and assert the format's self-verifying invariants:
    /// each frame must end exactly where the next begins, and every RLE row
    /// must consume exactly `width` pixels.
    pub fn validate(&self) -> Result<()> {
        for i in 0..self.frames.len() {
            let (_, end) = self.decode_inner(i)?;
            let expected = self.frame_boundary(i);
            if end != expected {
                return Err(Error::FrameSizeMismatch { frame: i, ended: end, expected });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assembles a PL8 in memory: header, frame table with offsets filled in to
    /// match, then the payloads laid end to end. Synthesised rather than checked
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
        // "00 00" asks to skip zero pixels: no progress, so a naive decoder
        // spins forever. Must be an error instead.
        //
        // The payload is deliberately 5 bytes for a 4x1 frame. At exactly 4 it
        // would span w*h, and the whole file would be reclassified as raw - a
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
        // Push frame 1 one byte later than frame 0 actually ends. This is the
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
        // Outside the diamond is not merely index 0 - it was never written.
        assert_eq!(&f.opaque[0..2], &[false, false]);
        assert_eq!(&f.opaque[6..12], &[true; 6]);
        pl8.validate().unwrap();
    }

    #[test]
    fn a_diamond_ignores_its_overhang_row_count() {
        // 24 frames in the real corpus declare rows on shape 1 and still hold
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
