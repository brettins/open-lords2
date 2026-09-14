#![allow(unused_imports)]
use super::*;
use super::bitstream::*;
use super::audio::*;
use std::fmt;

impl Header {
    pub fn parse(b: &[u8]) -> Result<Header> {
        if b.len() < HEADER_LEN {
            return Err(Error::Truncated("the header"));
        }
        let signature = [b[0], b[1], b[2], b[3]];
        if &signature != b"SMK2" && &signature != b"SMK4" {
            return Err(Error::Signature(signature));
        }
        let at = |o: usize| u32_at(b, o).expect("header length checked");
        let mut audio_max = [0u32; 7];
        let mut audio = [0u32; 7];
        for t in 0..7 {
            audio_max[t] = at(0x18 + t * 4);
            audio[t] = at(0x48 + t * 4);
        }
        Ok(Header {
            signature,
            width: at(0x04),
            height: at(0x08),
            frames: at(0x0C),
            interval: at(0x10) as i32,
            flags: at(0x14),
            audio_max,
            trees_size: at(0x34),
            tree_alloc: [at(0x38), at(0x3C), at(0x40), at(0x44)],
            audio,
        })
    }

    pub fn is_smk4(&self) -> bool {
        &self.signature == b"SMK4"
    }

    pub fn has_ring_frame(&self) -> bool {
        self.flags & flag::RING != 0
    }

    /// **How many display rows each stored row becomes: 1 or 2.**
    pub fn y_scale(&self) -> u32 {
        match self.y_scale_mode() {
            YScale::One => 1,
            _ => 2,
        }
    }

    /// **What the two scaling bits do**, from the DLL. See [`YScale`].
    pub fn y_scale_mode(&self) -> YScale {
        match self.flags & (flag::Y_SCALE_1 | flag::Y_SCALE_2) {
            flag::Y_SCALE_1 => YScale::Interlace,
            flag::Y_SCALE_2 => YScale::Double,
            _ => YScale::One,
        }
    }

    pub fn display_height(&self) -> u32 {
        self.height * self.y_scale()
    }

    /// One frame's duration in tens of microseconds, the unit a negative
    /// interval is already written in. `-8333` is 83.33 ms, twelve frames a
    /// second; `-7100` is the two films at 14.08.
    pub fn period_10us(&self) -> u32 {
        match self.interval {
            i if i > 0 => (i as u32).saturating_mul(100),
            i if i < 0 => i.unsigned_abs(),
            _ => 10_000,
        }
    }

    pub fn track(&self, t: usize) -> Option<Track> {
        let d = *self.audio.get(t)?;
        if d == 0 {
            return None;
        }
        Some(Track {
            rate: d & 0x00FF_FFFF,
            packed: d & (1 << 31) != 0,
            bits16: d & (1 << 29) != 0,
            stereo: d & (1 << 28) != 0,
        })
    }
}

const MMAP: usize = 0;
const MCLR: usize = 1;
const FULL: usize = 2;
const TYPE: usize = 3;

/// A block type's run length, indexed by bits 2–7 of the type value: one
/// through fifty-nine, then five powers of two.
pub(super) const RUN: [u32; 64] = {
    let mut t = [0u32; 64];
    let mut i = 0;
    while i < 59 {
        t[i] = i as u32 + 1;
        i += 1;
    }
    t[59] = 128;
    t[60] = 256;
    t[61] = 512;
    t[62] = 1024;
    t[63] = 2048;
    t
};

/// A 6-bit palette component to 8 bits, by replicating its top two bits into
/// the bottom — so 63 is 255 and 16 is 0x41.
///
/// **Not** the `.256` files' `v * 255 / 63`, which differs by one in the
/// middle of the range. `docs/formats/smk.md` §Palette.
#[inline]
pub fn expand6(v: u8) -> u8 {
    let v = v & 0x3F;
    (v << 2) | (v >> 4)
}

pub(super) struct Chunks<'a> {
    palette: Option<&'a [u8]>,
    pub(super) audio: [Option<&'a [u8]>; 7],
    video: &'a [u8],
}

// ------------------------------------------------------------- video

/// Plays a film one frame at a time into its own buffer.
///
/// **The buffer persists between frames** and is the whole of how Smacker
/// compresses time: a skip block leaves last frame's pixels where they are.
/// It starts at zero, as does the palette.
#[derive(Debug, Clone)]
pub struct Decoder {
    width: usize,
    height: usize,
    y: YScale,
    next: usize,
    pixels: Vec<u8>,
    palette: [[u8; 3]; 256],
    palette_changed: bool,
    recent: [[u16; 3]; 4],
    video_bits: (usize, usize),
}

impl Decoder {
    pub fn new(smk: &Smk) -> Decoder {
        let (w, h) = (smk.header.width as usize, smk.header.height as usize);
        Decoder {
            width: w,
            height: h,
            y: smk.header.y_scale_mode(),
            next: 0,
            pixels: vec![0; w * h],
            palette: [[0; 3]; 256],
            palette_changed: false,
            recent: [[0; 3]; 4],
            video_bits: (0, 0),
        }
    }

    /// The stored width and height of [`Decoder::pixels`].
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// The index of the frame now in [`Decoder::pixels`], or `None` before the
    /// first.
    pub fn frame(&self) -> Option<usize> {
        self.next.checked_sub(1)
    }

    /// Stored width × stored height palette indices, row by row.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// **The frame as `_SmackToBuffer@28` (`0x403AF0`) leaves a cleared
    /// destination**: stored width × [`Header::display_height`], borrowed when
    /// [`YScale::Interlace`] writes the even rows and
    /// leaves the odd ones black; [`YScale::Double`] writes each row twice.
    pub fn display(&self) -> std::borrow::Cow<'_, [u8]> {
        if self.y == YScale::One {
            return std::borrow::Cow::Borrowed(&self.pixels);
        }
        let mut out = vec![0u8; self.width * self.height * 2];
        for row in 0..self.height {
            let src = &self.pixels[row * self.width..(row + 1) * self.width];
            out[2 * row * self.width..][..self.width].copy_from_slice(src);
            if self.y == YScale::Double {
                out[(2 * row + 1) * self.width..][..self.width].copy_from_slice(src);
            }
        }
        std::borrow::Cow::Owned(out)
    }

    /// The palette as 8-bit triples.
    pub fn palette(&self) -> &[[u8; 3]; 256] {
        &self.palette
    }

    /// Whether the frame just decoded changed the palette.
    pub fn palette_changed(&self) -> bool {
        self.palette_changed
    }

    /// **(bits read, bits available)** in the last frame's video bitstream.
    /// A correct decode leaves fewer than 32 unread: the chunk's padding.
    pub fn video_bits(&self) -> (usize, usize) {
        self.video_bits
    }

    /// Decode `smk`'s next frame. `Ok(false)` once every frame has been.
    ///
    /// `smk` must be the film this decoder was made for; a different one of
    /// the same size decodes garbage.
    pub fn next_frame(&mut self, smk: &Smk) -> Result<bool> {
        let i = self.next;
        if i >= smk.frames() {
            return Ok(false);
        }
        self.decode(smk, i).map_err(|e| Error::Frame(i, Box::new(e)))?;
        self.next += 1;
        Ok(true)
    }

    pub(super) fn decode(&mut self, smk: &Smk, i: usize) -> Result<()> {
        let chunks = smk.chunks(i)?;
        self.palette_changed = false;
        if let Some(p) = chunks.palette {
            self.apply_palette(p)?;
            self.palette_changed = true;
        }
        self.decode_video(smk, chunks.video)
    }

    /// **The palette delta.** Three opcodes against the palette as it stood
    /// before this frame:
    ///
    /// * `1nnnnnnn` — leave the next `n + 1` entries alone;
    /// * `01nnnnnn s` — copy `n + 1` entries from the *old* palette starting at
    ///   `s`;
    /// * `00rrrrrr gg bb` — one entry, three 6-bit components.
    ///
    /// Copying reads a snapshot
    /// frame has already written reads the old values. `Pill_brn.smk` frame
    /// 104 depends on exactly that.
    fn apply_palette(&mut self, chunk: &[u8]) -> Result<()> {
        let old = self.palette;
        let mut idx = 0usize;
        let mut p = 0usize;
        while idx < 256 && p < chunk.len() {
            let b = chunk[p];
            p += 1;
            if b & 0x80 != 0 {
                idx += (b & 0x7F) as usize + 1;
            } else if b & 0x40 != 0 {
                let count = (b & 0x3F) as usize + 1;
                // A chunk is padded out to a multiple of four bytes, so an
                // opcode cut short by the end is padding, not a truncation.
                let Some(&src) = chunk.get(p) else { break };
                let src = src as usize;
                p += 1;
                for k in 0..count {
                    if idx >= 256 || src + k >= 256 {
                        break;
                    }
                    self.palette[idx] = old[src + k];
                    idx += 1;
                }
            } else {
                let Some(rest) = chunk.get(p..p + 2) else { break };
                self.palette[idx] = [expand6(b), expand6(rest[0]), expand6(rest[1])];
                p += 2;
                idx += 1;
            }
        }
        Ok(())
    }

    #[inline]
    fn put(&mut self, x: usize, y: usize, v: u8) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = v;
        }
    }

    /// **The block pass.** The picture is cut into 4 × 4 blocks in reading
    /// order
    /// type, the next six an index into [`RUN`], the high byte a colour.
    ///
    /// | type | name | per block |
    /// |---|---|---|
    /// | 0 | mono | a MClr code (high byte and low byte are two colours) and a MMap code (sixteen bits, row by row from the top, least significant first: set picks the high colour) |
    /// | 1 | full | per row, two Full codes: the first fills columns 2 and 3, the second columns 0 and 1, low byte then high |
    /// | 2 | skip | nothing — last frame's pixels stand |
    /// | 3 | solid | nothing — sixteen pixels of the run's colour |
    fn decode_video(&mut self, smk: &Smk, video: &[u8]) -> Result<()> {
        self.recent = [[0; 3]; 4];
        let (w, h) = (self.width, self.height);
        let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
        let blocks = bw * bh;
        let mut bits = Bits::new(video, "the video bitstream");
        let mut blk = 0usize;
        while blk < blocks {
            let t = smk.trees[TYPE].decode(&mut bits, &mut self.recent[TYPE])?;
            let kind = t & 3;
            let run = RUN[((t >> 2) & 0x3F) as usize] as usize;
            let colour = (t >> 8) as u8;
            if kind == 1 && smk.header.is_smk4() {
                return Err(Error::Unsupported("SMK4 full blocks"));
            }
            for _ in 0..run {
                if blk >= blocks {
                    break;
                }
                let (x0, y0) = ((blk % bw) * 4, (blk / bw) * 4);
                match kind {
                    0 => {
                        let clr = smk.trees[MCLR].decode(&mut bits, &mut self.recent[MCLR])?;
                        let mut map = smk.trees[MMAP].decode(&mut bits, &mut self.recent[MMAP])?;
                        let (hi, lo) = ((clr >> 8) as u8, clr as u8);
                        for dy in 0..4 {
                            for dx in 0..4 {
                                self.put(x0 + dx, y0 + dy, if map & 1 != 0 { hi } else { lo });
                                map >>= 1;
                            }
                        }
                    }
                    1 => {
                        for dy in 0..4 {
                            let a = smk.trees[FULL].decode(&mut bits, &mut self.recent[FULL])?;
                            self.put(x0 + 2, y0 + dy, a as u8);
                            self.put(x0 + 3, y0 + dy, (a >> 8) as u8);
                            let b = smk.trees[FULL].decode(&mut bits, &mut self.recent[FULL])?;
                            self.put(x0, y0 + dy, b as u8);
                            self.put(x0 + 1, y0 + dy, (b >> 8) as u8);
                        }
                    }
                    2 => {}
                    _ => {
                        for dy in 0..4 {
                            for dx in 0..4 {
                                self.put(x0 + dx, y0 + dy, colour);
                            }
                        }
                    }
                }
                blk += 1;
            }
        }
        self.video_bits = (bits.pos, bits.total());
        Ok(())
    }
}

