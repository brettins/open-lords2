
mod decode;
pub use decode::*;
mod bitstream;
pub use bitstream::*;
mod audio;
pub use audio::*;

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated(&'static str),
    Signature([u8; 4]),
    Unsupported(&'static str),
    BadTree(&'static str),
    Overrun(&'static str),
    Frame(usize, Box<Error>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated(what) => write!(f, "file ends inside {what}"),
            Error::Signature(s) => write!(f, "not a Smacker file (signature {s:02x?})"),
            Error::Unsupported(what) => write!(f, "unsupported: {what}"),
            Error::BadTree(what) => write!(f, "malformed Huffman tree: {what}"),
            Error::Overrun(what) => write!(f, "bitstream overrun in {what}"),
            Error::Frame(i, e) => write!(f, "frame {i}: {e}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub const HEADER_LEN: usize = 104;

pub mod flag {
    pub const RING: u32 = 0x01;
    pub const Y_SCALE_1: u32 = 0x02;
    pub const Y_SCALE_2: u32 = 0x04;
}

/// **What the header's two scaling bits do to the picture.** `[V]` from
/// `Smackw32.dll` (base `0x400000`): `_SmackOpen@12` (`0x404EF0`) maps
/// `flags & 6 == 2` to `struct+0x392 |= 0x10` and `== 4` to `|= 0x20`, and
/// doubles the height either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YScale {
    One,
    /// **`0x02`** — bit `0x10`. `_SmackToBuffer@28` (`0x403AF0`) sets the row
    /// step to `pitch * 2` and keeps the ordinary block writers at
    /// `0x0040CCFC`, which write one row each, so **every odd display row is
    /// never written** and stays as the destination was cleared: black. The
    /// three wide films are these.
    Interlace,
    /// **`0x04`** — bit `0x20`, the doubling writers at `0x0040B24C`, which
    /// write each value twice. No shipped file sets it.
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub signature: [u8; 4],
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub interval: i32,
    pub flags: u32,
    pub audio_max: [u32; 7],
    pub trees_size: u32,
    pub tree_alloc: [u32; 4],
    pub audio: [u32; 7],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Track {
    pub rate: u32,
    pub packed: bool,
    pub bits16: bool,
    pub stereo: bool,
}

impl Track {
    pub fn channels(&self) -> u16 {
        if self.stereo {
            2
        } else {
            1
        }
    }
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}


#[derive(Debug, Clone)]
pub struct Smk {
    data: Vec<u8>,
    header: Header,
    sizes: Vec<u32>,
    flags: Vec<u8>,
    offsets: Vec<usize>,
    trees: [Tree16; 4],
    end: usize,
}

impl Smk {
    pub fn parse(data: Vec<u8>) -> Result<Smk> {
        let header = Header::parse(&data)?;
        let n = header.frames as usize + header.has_ring_frame() as usize;
        let sizes_at = HEADER_LEN;
        let flags_at = sizes_at + n * 4;
        let trees_at = flags_at + n;
        let frames_at = trees_at + header.trees_size as usize;
        if data.len() < frames_at {
            return Err(Error::Truncated("the frame table or the tree block"));
        }
        let mut sizes = Vec::with_capacity(n);
        let mut flags = Vec::with_capacity(n);
        let mut offsets = Vec::with_capacity(n);
        let mut at = frames_at;
        for i in 0..n {
            let size = u32_at(&data, sizes_at + i * 4).expect("checked above") & !3;
            sizes.push(size);
            flags.push(data[flags_at + i]);
            offsets.push(at);
            at += size as usize;
        }
        if data.len() < at {
            return Err(Error::Truncated("the last frame"));
        }
        let mut bits = Bits::new(&data[trees_at..frames_at], "the tree block");
        let trees = [
            Tree16::read(&mut bits)?,
            Tree16::read(&mut bits)?,
            Tree16::read(&mut bits)?,
            Tree16::read(&mut bits)?,
        ];
        Ok(Smk { header, sizes, flags, offsets, trees, end: at, data })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn frames(&self) -> usize {
        self.header.frames as usize
    }

    pub fn slack(&self) -> i64 {
        self.data.len() as i64 - self.end as i64
    }

    pub fn frame_flags(&self, frame: usize) -> u8 {
        self.flags.get(frame).copied().unwrap_or(0)
    }

    fn frame_bytes(&self, frame: usize) -> &[u8] {
        let at = self.offsets[frame];
        &self.data[at..at + self.sizes[frame] as usize]
    }

    fn chunks(&self, frame: usize) -> Result<Chunks<'_>> {
        let data = self.frame_bytes(frame);
        let flags = self.flags[frame];
        let mut pos = 0usize;
        let mut palette = None;
        if flags & 1 != 0 {
            let len = *data.first().ok_or(Error::Truncated("a palette chunk"))? as usize * 4;
            if len == 0 || len > data.len() {
                return Err(Error::Truncated("a palette chunk"));
            }
            palette = Some(&data[1..len]);
            pos = len;
        }
        let mut audio: [Option<&[u8]>; 7] = [None; 7];
        for (t, slot) in audio.iter_mut().enumerate() {
            if flags & (2 << t) == 0 {
                continue;
            }
            let len = u32_at(data, pos).ok_or(Error::Truncated("an audio chunk"))? as usize;
            if len < 4 || pos + len > data.len() {
                return Err(Error::Truncated("an audio chunk"));
            }
            *slot = Some(&data[pos + 4..pos + len]);
            pos += len;
        }
        Ok(Chunks { palette, audio, video: &data[pos..] })
    }

    pub fn decoder(&self) -> Decoder {
        Decoder::new(self)
    }

    pub fn audio_chunk(&self, frame: usize, track: usize) -> Result<Option<AudioChunk>> {
        let desc = self.header.track(track).ok_or(Error::Unsupported("a track that is absent"))?;
        let chunks = self.chunks(frame).map_err(|e| Error::Frame(frame, Box::new(e)))?;
        let Some(raw) = chunks.audio.get(track).copied().flatten() else { return Ok(None) };
        decode_audio(raw, desc).map(Some).map_err(|e| Error::Frame(frame, Box::new(e)))
    }

    pub fn audio(&self, track: usize) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        for f in 0..self.frames() {
            if let Some(c) = self.audio_chunk(f, track)? {
                out.extend_from_slice(&c.pcm);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests;

