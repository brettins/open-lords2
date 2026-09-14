//! **Smacker video, decoded from the published format description.**
//!
//! Lords of the Realm II ships 45 `.smk` films — the intro, the Impressions
//! logo, the credits, a Lords of Magic trailer
//! over a captured county, a finished battle, a new castle and a fallen lord.
//! `docs/formats/smk.md` measured all of them: every one is `SMK2`, 8-bit
//! palettised video with one Huffman-packed 8-bit audio track, no ring frame
//! and no keyframe.
//!
//! # Where this came from, and where it did not
//!
//! `docs/formats/smk.md` found **no permissively licensed decoder anywhere**:
//! libsmacker is LGPL-2.1, the `smk` crate is a port of it, FFmpeg's is
//! LGPL-2.1+ and ScummVM's is GPL. So this is written from the format's
//! *description* — the container layout, the four Huffman trees and their
//! three-value recency cache, the four block types, the palette delta opcodes
//! and the DPCM audio — which is a set of facts about a file format
//! are not copyrightable (`CLAUDE.md` rule 3). No line of any of those
//! implementations was read while writing this one.
//!
//! # What checks it, in rising order of strength
//!
//! 1. **The container invariant** — `104 + n·4 + n + trees + Σ sizes` is the
//!    file length exactly, for all 45 shipped films ([`Smk::slack`]).
//! 2. **Every bitstream is consumed to its padding and no further.** Chunks
//!    are padded to a four-byte boundary, and in all 7,652 video frames and
//!    7,108 audio chunks of the corpus the decoder leaves between 0 and 31 bits
//!    unread. A Huffman tree read one bit wrong desynchronises every code after
//! it and the decoder runs off the end of its chunk instead.
//!    [`Decoder::video_bits`] and [`AudioChunk`] report both numbers so the
//!    corpus test can demand it (`crates/l2-smk/tests/corpus.rs`).
//! 3. **An independent decoder agrees, pixel for pixel.** That comparison was
//!    run once, outside the tree, against a black-box build of the LGPL crate —
//!    its output compared, its source never opened — and its numbers are pinned
//!    in the corpus test. See `docs/formats/smk.md`.
//!
//! None of those is the original's `smackw32.dll` drawing a frame. The one
//! thing the data cannot settle — what happens to the second row of a doubled
//! film — was read out of that DLL instead: see [`YScale`].
//!
//! # What it does not do
//!
//! **`SMK4`'s two extra full-block modes and 16-bit audio.** Neither occurs in
//! this game's corpus, the description of both is thinner than the rest, and a
//! decoder for inputs nobody can test against is a decoder that is wrong in a
//! way nobody will notice. Both are refused with [`Error::Unsupported`] rather
//! than guessed at.

mod decode;
pub use decode::*;
mod bitstream;
pub use bitstream::*;
mod audio;
pub use audio::*;

use std::fmt;

/// Everything that can go wrong, and every variant names the thing it was
/// reading — a decoder that fails on a shipped film should say which frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file ended inside the named structure.
    Truncated(&'static str),
    /// Not `SMK2` or `SMK4`.
    Signature([u8; 4]),
    /// A legal Smacker feature this decoder deliberately refuses. See the
    /// crate docs.
    Unsupported(&'static str),
    /// A Huffman tree that does not have the shape the format requires.
    BadTree(&'static str),
    /// A bitstream asked for more bits than its chunk holds.
    Overrun(&'static str),
    /// Something inside one frame, with the frame's index.
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

/// The fixed header's length.
pub const HEADER_LEN: usize = 104;

/// The header flags word.
pub mod flag {
    /// A ring frame follows the last, for looping playback. No shipped film
    /// sets it.
    pub const RING: u32 = 0x01;
    /// The two vertical-scaling bits. The three wide films — `Intro.smk`,
    /// `LOM.SMK` and `Credits.smk` — set `0x02` and nothing else does. See
    /// [`super::YScale`] for what each does.
    pub const Y_SCALE_1: u32 = 0x02;
    pub const Y_SCALE_2: u32 = 0x04;
}

/// **What the header's two scaling bits do to the picture.** `[V]` from
/// `Smackw32.dll` (base `0x400000`): `_SmackOpen@12` (`0x404EF0`) maps
/// `flags & 6 == 2` to `struct+0x392 |= 0x10` and `== 4` to `|= 0x20`, and
/// doubles the height either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YScale {
    /// Neither bit: one display row per stored row.
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

/// The 104-byte header, field for field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub signature: [u8; 4],
    /// The stored width, in pixels.
    pub width: u32,
    /// The **stored** height. The displayed height is [`Header::display_height`].
    pub height: u32,
    /// Frames, not counting a ring frame.
    pub frames: u32,
    /// Positive: milliseconds per frame. Negative: tens of microseconds. Zero:
    /// the format's default of ten frames a second.
    pub interval: i32,
    pub flags: u32,
    /// The largest unpacked audio chunk, per track.
    pub audio_max: [u32; 7],
    /// The length of the Huffman tree block.
    pub trees_size: u32,
    /// Allocation hints for the four big trees — MMap, MClr, Full, Type. Read
    /// and reported; the decoder sizes its trees from the bitstream instead.
    pub tree_alloc: [u32; 4],
    /// The audio descriptor per track: sample rate in bits 0–23, then flags.
    pub audio: [u32; 7],
}

/// One audio track's descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Track {
    pub rate: u32,
    /// Bit 31.
    pub packed: bool,
    /// Bit 29.
    pub bits16: bool,
    /// Bit 28.
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

// -------------------------------------------------------------- file

/// A parsed film: the header, the frame table and the four trees. The frames
/// themselves are decoded on demand by a [`Decoder`].
#[derive(Debug, Clone)]
pub struct Smk {
    data: Vec<u8>,
    header: Header,
    sizes: Vec<u32>,
    flags: Vec<u8>,
    offsets: Vec<usize>,
    trees: [Tree16; 4],
    /// Where the payloads end, which the container invariant compares with
    /// the file's length.
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
            // The low two bits of a size are flags (bit 0 a keyframe), never
            // length. No shipped film sets either.
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

    /// Frames to play, not counting a ring frame.
    pub fn frames(&self) -> usize {
        self.header.frames as usize
    }

    /// **The container invariant's residue**: the file's length minus what the
    /// header, the tables, the trees and every frame account for. Zero for
    /// all 45 of this game's films; a wrong reading of any field drifts it.
    pub fn slack(&self) -> i64 {
        self.data.len() as i64 - self.end as i64
    }

    /// The frame-type byte: bit 0 a palette, bits 1–7 audio tracks 0–6.
    pub fn frame_flags(&self, frame: usize) -> u8 {
        self.flags.get(frame).copied().unwrap_or(0)
    }

    fn frame_bytes(&self, frame: usize) -> &[u8] {
        let at = self.offsets[frame];
        &self.data[at..at + self.sizes[frame] as usize]
    }

    /// Split one frame's payload into its palette chunk, its audio chunks and
    /// its video bitstream.
    fn chunks(&self, frame: usize) -> Result<Chunks<'_>> {
        let data = self.frame_bytes(frame);
        let flags = self.flags[frame];
        let mut pos = 0usize;
        let mut palette = None;
        if flags & 1 != 0 {
            // One byte of length in units of four
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
            // Four bytes of length, which count themselves.
            let len = u32_at(data, pos).ok_or(Error::Truncated("an audio chunk"))? as usize;
            if len < 4 || pos + len > data.len() {
                return Err(Error::Truncated("an audio chunk"));
            }
            *slot = Some(&data[pos + 4..pos + len]);
            pos += len;
        }
        Ok(Chunks { palette, audio, video: &data[pos..] })
    }

    /// A decoder positioned before this film's first frame. It borrows
    /// nothing: every call that needs the film is handed it
    /// own the film and its decoder side by side.
    pub fn decoder(&self) -> Decoder {
        Decoder::new(self)
    }

    /// **One frame's audio for one track**, decoded, or `None` when the frame
    /// carries none.
    pub fn audio_chunk(&self, frame: usize, track: usize) -> Result<Option<AudioChunk>> {
        let desc = self.header.track(track).ok_or(Error::Unsupported("a track that is absent"))?;
        let chunks = self.chunks(frame).map_err(|e| Error::Frame(frame, Box::new(e)))?;
        let Some(raw) = chunks.audio.get(track).copied().flatten() else { return Ok(None) };
        decode_audio(raw, desc).map(Some).map_err(|e| Error::Frame(frame, Box::new(e)))
    }

    /// **A whole track**, decoded and concatenated: interleaved unsigned 8-bit
    /// PCM at [`Track::rate`], the format a `.wav` of this game also uses.
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

