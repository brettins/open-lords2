#![allow(unused_imports)]
use super::*;

use crate::{u16_at, u32_at, Error, Result};

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
        for i in 0..frames.len() {
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

        let rows = info.overhang_rows as usize;
        let iso_overhang = matches!(
            info.shape,
            Shape::DiamondFull | Shape::DiamondLeft | Shape::DiamondRight
        );
        // Reserving unconditionally is also what the engine does: `Glyph_Draw`
        // (0x00402A14) adds byte 0x0D to `y` before it clips, for every frame,
        // without looking at how many bytes the frame occupies.
        let reserves_overhang = info.shape == Shape::Rect && self.storage != Storage::Rle;
        let stored_rect_overhang = reserves_overhang && rows > 0 && start + w * h != boundary;

        let overhang = if iso_overhang || reserves_overhang { rows } else { 0 };
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
// shape byte for every family.
        let end = if self.storage == Storage::Rle {
            self.decode_rle(index, info, &mut indices, &mut opaque)?
        } else {
            match info.shape {
                Shape::Rect => {
                    let after = self.decode_rect(start, w, h, overhang, &mut indices, &mut opaque)?;
                    if stored_rect_overhang {
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

    pub fn is_supported(&self) -> bool {
        self.storage.is_supported()
    }

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

