//! PL8 sprite containers.
//!
//! Layout (see `docs/formats/pl8.md` for how this was established):
//!
//! ```text
//! 0x00  u8   storage mode: 0 = raw, 1 = RLE, 2 = undecoded
//! 0x01  u8   sub-mode / flags
//! 0x02  u16  frame count
//! 0x04  u16  unknown
//! 0x06  u8   unknown
//! 0x07  u8   unknown (0..15)
//! then `frame count` records of 16 bytes:
//!   0x00 u16 width
//!   0x02 u16 height
//!   0x04 u32 absolute file offset of pixel data
//!   0x08 u8[8] undecoded; not padding
//! ```

use crate::{u16_at, u32_at, Error, Result};

pub const HEADER_LEN: usize = 8;
pub const FRAME_RECORD_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    /// `width * height` bytes, row-major. Palette index 0 is transparent.
    Raw,
    /// Per-row runs: `0x00 n` skips n transparent pixels, `n` copies n literals.
    Rle,
    /// Header byte 0 == 2. Heterogeneous and not yet decoded.
    Unknown(u8),
}

impl Storage {
    fn from_byte(b: u8) -> Self {
        match b {
            0 => Storage::Raw,
            1 => Storage::Rle,
            other => Storage::Unknown(other),
        }
    }

    pub fn is_supported(self) -> bool {
        matches!(self, Storage::Raw | Storage::Rle)
    }
}

#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub width: u16,
    pub height: u16,
    pub offset: u32,
    /// Not padding - frequently non-zero. The u16 at `[2]` runs in arithmetic
    /// sequences, so it reads as a sheet position or ordering key. Undecoded.
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
    pub sub_mode: u8,
    pub frames: Vec<FrameInfo>,
}

impl<'a> Pl8<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() < HEADER_LEN {
            return Err(Error::Truncated { needed: HEADER_LEN, have: data.len() });
        }
        let storage = Storage::from_byte(data[0]);
        let sub_mode = data[1];
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
                trailing,
            });
        }
        Ok(Pl8 { data, storage, sub_mode, frames })
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
        let pixels = w * h;
        let mut indices = vec![0u8; pixels];
        let mut opaque = vec![false; pixels];

        let end = match self.storage {
            Storage::Raw => {
                let end = start + pixels;
                if end > self.data.len() {
                    return Err(Error::Truncated { needed: end, have: self.data.len() });
                }
                indices.copy_from_slice(&self.data[start..end]);
                for (o, &i) in opaque.iter_mut().zip(indices.iter()) {
                    *o = i != 0;
                }
                end
            }
            Storage::Rle => {
                let mut p = start;
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
                                return Err(Error::ZeroLengthRun {
                                    frame: index,
                                    row: y as u16,
                                });
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
                p
            }
            Storage::Unknown(m) => return Err(Error::UnsupportedStorage(m)),
        };

        Ok((
            DecodedFrame { width: info.width, height: info.height, indices, opaque },
            end,
        ))
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
        let bytes = build(1, 0, &[(4, 1, &[0x00, 0x00, 0x00, 0x04])]);
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
        let bytes = build(2, 0, &[(2, 2, &[1, 2, 3, 4])]);
        let pl8 = Pl8::parse(&bytes).unwrap();
        assert!(!pl8.is_supported());
        assert_eq!(pl8.decode(0).unwrap_err(), Error::UnsupportedStorage(2));
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
