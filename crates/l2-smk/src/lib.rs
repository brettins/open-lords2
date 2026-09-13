//! **Smacker video, decoded from the published format description.**
//!
//! Lords of the Realm II ships 45 `.smk` films — the intro, the Impressions
//! logo, the credits, a Lords of Magic trailer, and the stingers the game plays
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
//! and the DPCM audio — which is a set of facts about a file format, and facts
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

// --------------------------------------------------------------- bits

/// **Least significant bit first**, within bytes taken in order — the order
/// every Smacker bitstream is written in.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    what: &'static str,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8], what: &'static str) -> Bits<'a> {
        Bits { data, pos: 0, what }
    }

    #[inline]
    fn bit(&mut self) -> Result<usize> {
        let byte = *self.data.get(self.pos >> 3).ok_or(Error::Overrun(self.what))?;
        let b = (byte >> (self.pos & 7)) & 1;
        self.pos += 1;
        Ok(b as usize)
    }

    fn bits(&mut self, n: u32) -> Result<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= (self.bit()? as u32) << i;
        }
        Ok(v)
    }

    fn total(&self) -> usize {
        self.data.len() * 8
    }
}

// ------------------------------------------------------------- trees

/// A tree reference: an index into `nodes`, or a leaf.
const LEAF: u32 = 0x8000_0000;
/// On a big tree's leaf: *"use recency slot n"*, n in the low two bits.
const CACHED: u32 = 0x4000_0000;

/// How deep a tree may nest before the file is called malformed. A 16-bit
/// tree over a real frame is a few dozen levels; this is a guard against
/// recursion on a corrupt file, not a property of the format.
const MAX_DEPTH: usize = 512;

/// **An 8-bit tree.** The low- and high-byte trees inside every big tree, and
/// every audio delta tree.
///
/// On disk: one bit saying whether the tree is there; then the tree
/// depth-first, `1` for a branch (its `0` child first) and `0` followed by an
/// 8-bit value for a leaf; then a `0` bit closing it. An absent tree decodes
/// every symbol as zero and costs no bits.
#[derive(Debug, Clone)]
struct Tree8 {
    nodes: Vec<[u32; 2]>,
    root: u32,
}

impl Tree8 {
    fn read(bits: &mut Bits) -> Result<Tree8> {
        let mut t = Tree8 { nodes: Vec::new(), root: LEAF };
        if bits.bit()? == 0 {
            return Ok(t);
        }
        t.root = t.build(bits, 0)?;
        if bits.bit()? != 0 {
            return Err(Error::BadTree("an 8-bit tree does not end on a 0 bit"));
        }
        Ok(t)
    }

    fn build(&mut self, bits: &mut Bits, depth: usize) -> Result<u32> {
        if depth > MAX_DEPTH {
            return Err(Error::BadTree("an 8-bit tree nests too deep"));
        }
        if bits.bit()? == 1 {
            let at = self.nodes.len();
            self.nodes.push([0, 0]);
            let zero = self.build(bits, depth + 1)?;
            let one = self.build(bits, depth + 1)?;
            self.nodes[at] = [zero, one];
            Ok(at as u32)
        } else {
            Ok(LEAF | bits.bits(8)?)
        }
    }

    #[inline]
    fn decode(&self, bits: &mut Bits) -> Result<u8> {
        let mut n = self.root;
        while n & LEAF == 0 {
            n = self.nodes[n as usize][bits.bit()?];
        }
        Ok(n as u8)
    }
}

/// **A 16-bit tree** — MMap, MClr, Full or Type — with its three-value cache.
///
/// On disk: a presence bit; the low-byte and high-byte 8-bit trees; three
/// 16-bit **escape** values; the tree itself, depth-first as [`Tree8`] but with
/// every leaf's value spelled as a low-tree code followed by a high-tree code;
/// a closing `0` bit.
///
/// A leaf whose value equals escape *n* is not that value: it means *"the n-th
/// most recent value this tree produced"*. After every decode, a value that is
/// not already the most recent is pushed onto the front of the three and the
/// oldest falls off. The three reset to zero at the start of every frame.
#[derive(Debug, Clone)]
struct Tree16 {
    nodes: Vec<[u32; 2]>,
    root: u32,
}

impl Tree16 {
    fn read(bits: &mut Bits) -> Result<Tree16> {
        let mut t = Tree16 { nodes: Vec::new(), root: LEAF };
        if bits.bit()? == 0 {
            return Ok(t);
        }
        let low = Tree8::read(bits)?;
        let high = Tree8::read(bits)?;
        let escapes = [bits.bits(16)?, bits.bits(16)?, bits.bits(16)?];
        t.root = t.build(bits, &low, &high, escapes, 0)?;
        if bits.bit()? != 0 {
            return Err(Error::BadTree("a 16-bit tree does not end on a 0 bit"));
        }
        Ok(t)
    }

    fn build(
        &mut self,
        bits: &mut Bits,
        low: &Tree8,
        high: &Tree8,
        escapes: [u32; 3],
        depth: usize,
    ) -> Result<u32> {
        if depth > MAX_DEPTH {
            return Err(Error::BadTree("a 16-bit tree nests too deep"));
        }
        if bits.bit()? == 1 {
            let at = self.nodes.len();
            self.nodes.push([0, 0]);
            let zero = self.build(bits, low, high, escapes, depth + 1)?;
            let one = self.build(bits, low, high, escapes, depth + 1)?;
            self.nodes[at] = [zero, one];
            Ok(at as u32)
        } else {
            let lo = low.decode(bits)? as u32;
            let hi = high.decode(bits)? as u32;
            let v = lo | (hi << 8);
            Ok(match escapes.iter().position(|&e| e == v) {
                Some(slot) => LEAF | CACHED | slot as u32,
                None => LEAF | v,
            })
        }
    }

    #[inline]
    fn decode(&self, bits: &mut Bits, recent: &mut [u16; 3]) -> Result<u16> {
        let mut n = self.root;
        while n & LEAF == 0 {
            n = self.nodes[n as usize][bits.bit()?];
        }
        let v = if n & CACHED != 0 { recent[(n & 3) as usize] } else { n as u16 };
        if recent[0] != v {
            recent[2] = recent[1];
            recent[1] = recent[0];
            recent[0] = v;
        }
        Ok(v)
    }
}

const MMAP: usize = 0;
const MCLR: usize = 1;
const FULL: usize = 2;
const TYPE: usize = 3;

/// A block type's run length, indexed by bits 2–7 of the type value: one
/// through fifty-nine, then five powers of two.
const RUN: [u32; 64] = {
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
            // One byte of length in units of four, and the byte counts itself.
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
    /// nothing: every call that needs the film is handed it, so a player can
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

struct Chunks<'a> {
    palette: Option<&'a [u8]>,
    audio: [Option<&'a [u8]>; 7],
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

    fn decode(&mut self, smk: &Smk, i: usize) -> Result<()> {
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
    /// Copying reads a snapshot, so a copy whose source overlaps what this
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
    /// order, and the Type tree gives runs of them: the low two bits a block
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

// ------------------------------------------------------------- audio

/// One frame's audio, decoded, with the two numbers that say whether it was
/// decoded right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioChunk {
    /// Interleaved unsigned 8-bit samples.
    pub pcm: Vec<u8>,
    /// What the chunk's own header said the output would be.
    pub unpacked: usize,
    /// Bits read, and bits in the chunk after its length word.
    pub bits: (usize, usize),
}

/// **One packed audio chunk.**
///
/// A 32-bit unpacked length; then, as bits, a *data present* flag, a stereo
/// flag and a 16-bit flag (which must agree with the track's descriptor); one
/// 8-bit delta tree per channel; the first sample of each channel as eight raw
/// bits, **right before left**; and then one tree code per sample, a signed
/// delta added to that channel's last value, channels interleaved from the
/// left.
fn decode_audio(raw: &[u8], desc: Track) -> Result<AudioChunk> {
    if !desc.packed {
        return Ok(AudioChunk { pcm: raw.to_vec(), unpacked: raw.len(), bits: (0, 0) });
    }
    let unpacked = u32_at(raw, 0).ok_or(Error::Truncated("an audio chunk's length"))? as usize;
    let mut bits = Bits::new(&raw[4..], "an audio bitstream");
    let mut pcm = Vec::with_capacity(unpacked);
    if bits.bit()? == 0 {
        return Ok(AudioChunk { pcm, unpacked, bits: (bits.pos, bits.total()) });
    }
    let stereo = bits.bit()? == 1;
    let bits16 = bits.bit()? == 1;
    if stereo != desc.stereo || bits16 != desc.bits16 {
        return Err(Error::BadTree("an audio chunk disagrees with its track's descriptor"));
    }
    if bits16 {
        return Err(Error::Unsupported("16-bit audio"));
    }
    let channels = if stereo { 2 } else { 1 };
    let trees: Vec<Tree8> =
        (0..channels).map(|_| Tree8::read(&mut bits)).collect::<Result<Vec<_>>>()?;
    let mut last = [0u8; 2];
    for c in (0..channels).rev() {
        last[c] = bits.bits(8)? as u8;
    }
    for &v in last.iter().take(channels) {
        if pcm.len() < unpacked {
            pcm.push(v);
        }
    }
    while pcm.len() < unpacked {
        let c = pcm.len() % channels;
        let delta = trees[c].decode(&mut bits)?;
        last[c] = last[c].wrapping_add(delta);
        pcm.push(last[c]);
    }
    Ok(AudioChunk { pcm, unpacked, bits: (bits.pos, bits.total()) })
}

#[cfg(test)]
mod tests;
