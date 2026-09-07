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
    /// `width * height` bytes, row-major, fully opaque.
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
    /// Per-pixel coverage. Raw frames are fully opaque; RLE frames carry
    /// transparency via skip runs.
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
                opaque.fill(true);
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
                            x += next(&mut p)? as u32;
                        } else {
                            for k in 0..op as usize {
                                let v = next(&mut p)?;
                                let xi = x as usize + k;
                                if xi < w {
                                    indices[y * w + xi] = v;
                                    opaque[y * w + xi] = true;
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
