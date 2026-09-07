//! Canonical serialisation: the one byte stream everything agrees on.
//!
//! Three things need to turn simulation state into bytes, and
//! `docs/netcode.md` requires that they all produce the *same* bytes:
//!
//! * the state checksum exchanged every tick (§6),
//! * the once-per-turn snapshot used for late join and crash recovery
//!   (§5),
//! * the desync dump (§6).
//!
//! If the checksum hashed one encoding and the snapshot wrote another,
//! then a desync dump would be a picture of a state that was never
//! checksummed, and the first hour of every investigation would go on
//! establishing that the two encoders agree. So there is one encoder,
//! [`Canonical`], and hashing is something it does *while* writing
//! rather than a separate pass over a buffer. A checksum and a snapshot
//! taken from the same state cannot disagree, because the same code
//! produced both.
//!
//! # What "canonical" rules out
//!
//! §6 is emphatic that we never hash the in-memory image, and the
//! reason is that a memory image is full of things that legitimately
//! differ between two peers running identical simulations: struct
//! padding, `Vec` spare capacity, allocator layout, `usize` width. Hash
//! any of those and the detector reports divergence on two peers that
//! agree perfectly — and a desync detector that cries wolf gets turned
//! off, which is worse than not having one.
//!
//! So every method here writes a **fixed width in little-endian**,
//! chosen once and never derived from the host. There is no `usize`
//! method: a `usize` is 8 bytes on this machine and 4 on a 32-bit one,
//! and a length written as `usize` is a desync between a 32-bit and a
//! 64-bit player. Lengths go out as `u32` through [`Canonical::len32`],
//! which panics rather than truncating.
//!
//! Little-endian to match the file formats we already read
//! (`docs/netcode.md` D-10), not because of the host's byte order —
//! `to_le_bytes` is a byte shuffle on a big-endian machine, not a
//! no-op, and that is the point.
//!
//! # Self-delimiting
//!
//! Every variable-length field is length-prefixed. That is not for the
//! decoder's benefit — a decoder that knows the schema could get by
//! without it — but for the *hash*: without a prefix, the two states
//! `("ab", "c")` and `("a", "bc")` produce identical byte streams and
//! therefore identical checksums, and a desync detector that cannot
//! see the difference between them has a blind spot exactly where
//! field boundaries move.
//!
//! # Sections
//!
//! [`Canonical::section`] starts a named region whose bytes are hashed
//! separately as well as into the whole. That is §6's "hash subsystems
//! separately" — units, terrain, PRNG state, command queue — which
//! turns "the game desynced" into "the unit array diverged at tick
//! 4,112" without anyone opening a debugger.
//!
//! **Deviation from the design, stated deliberately:** §6 says to do
//! this "in debug builds". This crate does it in every build. The
//! reason is that desyncs are reported by players, players run release
//! builds, and a dump from a release build that omits the one field
//! that localises the fault is a dump that costs a day. The price is
//! one extra `XxHash64` update per byte written — the same bytes,
//! hashed twice — which at 10 Hz over a state of this size is not
//! measurable. What is *not* done in every build is exchanging the
//! section hashes over the wire; the tick packet carries one `u64`, as
//! §4 specifies, and the section vector is local until a divergence
//! makes it worth writing down.

use crate::fixed::Fixed;
use crate::hash::XxHash64;

/// The seed for every state checksum in this project.
///
/// Zero, so that `xxh64sum` or any other off-the-shelf XXH64 tool
/// reproduces our number from a dumped byte stream. A random-looking
/// constant would have added nothing — there is no adversary here
/// (§8) — and would have made an external cross-check a small research
/// project at exactly the moment nobody has patience for one.
pub const CHECKSUM_SEED: u64 = 0;

/// A named region of the byte stream, hashed on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionDigest {
    pub name: &'static str,
    pub hash: u64,
    /// Bytes written inside the section. A length difference is a
    /// structural divergence — a different number of units — rather
    /// than a numeric one, and that distinction is worth a lot when
    /// reading a dump.
    pub len: u64,
}

/// The result of encoding a state: the checksum, the per-section
/// checksums, and optionally the bytes themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    pub hash: u64,
    pub len: u64,
    pub sections: Vec<SectionDigest>,
    /// Present only when the encoder was built with
    /// [`Canonical::recording`].
    pub bytes: Option<Vec<u8>>,
}

impl Digest {
    /// The section whose hash differs from `other`'s, if exactly one
    /// does.
    ///
    /// Returns `None` both when the states agree and when several
    /// sections differ, because those two cases lead to different
    /// questions: nothing to explain, versus a divergence that has
    /// already spread. §6's advice applies — if the PRNG section is the
    /// only one that differs, someone drew a random number outside the
    /// simulation; if it differs along with everything else, it is a
    /// consequence rather than a cause.
    pub fn sole_difference(&self, other: &Digest) -> Option<&'static str> {
        if self.sections.len() != other.sections.len() {
            return None;
        }
        let mut found = None;
        for (a, b) in self.sections.iter().zip(&other.sections) {
            if a.name != b.name {
                return None;
            }
            if a.hash != b.hash || a.len != b.len {
                if found.is_some() {
                    return None;
                }
                found = Some(a.name);
            }
        }
        found
    }

    /// Every section that differs, in encode order.
    pub fn differences<'a>(&'a self, other: &'a Digest) -> Vec<&'static str> {
        let mut out = Vec::new();
        for a in &self.sections {
            match other.sections.iter().find(|b| b.name == a.name) {
                Some(b) if b.hash == a.hash && b.len == a.len => {}
                _ => out.push(a.name),
            }
        }
        out
    }
}

/// A write-only, hash-as-you-go canonical encoder.
///
/// ```
/// # use l2_net::{Canonical, Fixed};
/// let mut c = Canonical::hashing();
/// c.section("units");
/// c.u16(3);
/// c.fixed(Fixed::from_int(40));
/// let digest = c.finish();
/// assert_eq!(digest.len, 6);
/// assert_eq!(digest.sections[0].name, "units");
/// ```
#[derive(Debug)]
pub struct Canonical {
    root: XxHash64,
    open: Option<(&'static str, XxHash64)>,
    sections: Vec<SectionDigest>,
    bytes: Option<Vec<u8>>,
}

impl Canonical {
    /// Hash without keeping the bytes. The per-tick checksum path — no
    /// allocation at all once the section vector has grown.
    pub fn hashing() -> Canonical {
        Canonical {
            root: XxHash64::with_seed(CHECKSUM_SEED),
            open: None,
            sections: Vec::new(),
            bytes: None,
        }
    }

    /// Hash *and* keep the bytes. The snapshot and desync-dump path.
    pub fn recording() -> Canonical {
        let mut c = Canonical::hashing();
        c.bytes = Some(Vec::new());
        c
    }

    /// Total bytes written so far.
    pub fn len(&self) -> u64 {
        self.root.len()
    }

    pub fn is_empty(&self) -> bool {
        self.root.len() == 0
    }

    /// Begin a named section, ending any section already open.
    ///
    /// Sections do not nest. Flat is what §6's localisation wants — a
    /// list of subsystems, one of which is the culprit — and nesting
    /// would raise the question of whether an inner section's bytes
    /// belong to the outer one, which is a question with no useful
    /// answer.
    pub fn section(&mut self, name: &'static str) {
        self.end_section();
        self.open = Some((name, XxHash64::with_seed(CHECKSUM_SEED)));
    }

    /// End the current section, if any. Called automatically by
    /// [`Canonical::section`] and [`Canonical::finish`].
    pub fn end_section(&mut self) {
        if let Some((name, hasher)) = self.open.take() {
            self.sections.push(SectionDigest {
                name,
                hash: hasher.finish(),
                len: hasher.len(),
            });
        }
    }

    /// Raw bytes, with **no** length prefix.
    ///
    /// The escape hatch, and the only method here that can make an
    /// encoding ambiguous. Use it for fixed-size arrays whose length is
    /// part of the schema (an 80×80 terrain grid); use
    /// [`Canonical::bytes`] for anything whose length can vary.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.root.write(bytes);
        if let Some((_, hasher)) = &mut self.open {
            hasher.write(bytes);
        }
        if let Some(buf) = &mut self.bytes {
            buf.extend_from_slice(bytes);
        }
    }

    pub fn u8(&mut self, v: u8) {
        self.raw(&[v]);
    }

    pub fn u16(&mut self, v: u16) {
        self.raw(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.raw(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.raw(&v.to_le_bytes());
    }

    pub fn i8(&mut self, v: i8) {
        self.raw(&v.to_le_bytes());
    }

    pub fn i16(&mut self, v: i16) {
        self.raw(&v.to_le_bytes());
    }

    pub fn i32(&mut self, v: i32) {
        self.raw(&v.to_le_bytes());
    }

    pub fn i64(&mut self, v: i64) {
        self.raw(&v.to_le_bytes());
    }

    /// One byte, `0` or `1`. Never the host's `bool` representation,
    /// which is not a stability promise anyone has made.
    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }

    /// A [`Fixed`] as its raw `i32`. The value, not a rendering of it.
    pub fn fixed(&mut self, v: Fixed) {
        self.i32(v.raw());
    }

    /// A count, as `u32`.
    ///
    /// Panics above `u32::MAX`. Truncating silently would be the
    /// classic serialisation bug — a length that no longer matches its
    /// data — and this game has no collection that can approach 4
    /// billion entries, so a panic here means memory corruption or a
    /// wildly wrong call, both of which want to stop immediately.
    pub fn len32(&mut self, n: usize) {
        assert!(n <= u32::MAX as usize, "canonical length {n} exceeds u32");
        self.u32(n as u32);
    }

    /// Length-prefixed bytes.
    pub fn bytes(&mut self, v: &[u8]) {
        self.len32(v.len());
        self.raw(v);
    }

    /// Length-prefixed UTF-8.
    ///
    /// Strings are rare in simulation state and common in handshakes.
    /// The length is in *bytes*, not characters, because that is the
    /// only definition two implementations cannot disagree about.
    pub fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }

    /// A length-prefixed sequence, written by a closure per item.
    ///
    /// Takes a slice rather than an iterator on purpose: an iterator
    /// over a `HashMap` would compile, and D-4 exists to make sure that
    /// never happens. A slice has one order and it is the one in
    /// memory.
    pub fn seq<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Canonical, &T)) {
        self.len32(items.len());
        for item in items {
            each(self, item);
        }
    }

    /// A value that may be absent: a tag byte, then the value.
    pub fn option<T>(&mut self, value: Option<&T>, each: impl FnOnce(&mut Canonical, &T)) {
        match value {
            None => self.u8(0),
            Some(v) => {
                self.u8(1);
                each(self, v);
            }
        }
    }

    pub fn encode<T: Encode>(&mut self, value: &T) {
        value.encode(self);
    }

    /// Close the encoder and produce the digest.
    pub fn finish(mut self) -> Digest {
        self.end_section();
        Digest {
            hash: self.root.finish(),
            len: self.root.len(),
            sections: self.sections,
            bytes: self.bytes,
        }
    }

    /// The checksum of one encodable value, with nothing kept.
    pub fn hash_of<T: Encode>(value: &T) -> u64 {
        let mut c = Canonical::hashing();
        value.encode(&mut c);
        c.finish().hash
    }

    /// The canonical bytes of one encodable value.
    pub fn bytes_of<T: Encode>(value: &T) -> Vec<u8> {
        let mut c = Canonical::recording();
        value.encode(&mut c);
        c.finish().bytes.expect("recording encoder keeps its bytes")
    }
}

/// Something with a canonical byte form.
///
/// Hand-written, per D-10. There is no derive here and there will not
/// be one: a derive is a layout decided by a macro, and the whole point
/// of this module is that the layout is decided by a person and does
/// not move when a dependency does.
pub trait Encode {
    fn encode(&self, out: &mut Canonical);
}

/// The inverse of [`Encode`]. Round-tripping is a test obligation, not
/// a promise the trait can make: `tests/canonical.rs` checks it for
/// every type in this crate.
pub trait Decode: Sized {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Ran off the end. Carries what it wanted and what was left,
    /// because "unexpected end of input" alone has never once been
    /// enough to find the cause.
    UnexpectedEnd { wanted: usize, remaining: usize, at: usize },
    /// Decoded successfully but did not consume everything. Always an
    /// error here: a trailing byte means the two sides disagree about
    /// the schema, which is precisely the disagreement this crate
    /// exists to catch early.
    TrailingBytes { unread: usize },
    /// A length prefix larger than the data that follows it.
    LengthOverrun { declared: usize, remaining: usize, at: usize },
    NotUtf8 { at: usize },
    /// A tag byte outside the set the schema defines.
    BadTag { tag: u8, expected: &'static str, at: usize },
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CodecError::UnexpectedEnd { wanted, remaining, at } => {
                write!(f, "wanted {wanted} bytes at offset {at}, {remaining} remain")
            }
            CodecError::TrailingBytes { unread } => {
                write!(f, "{unread} unread bytes after decoding")
            }
            CodecError::LengthOverrun { declared, remaining, at } => {
                write!(f, "length {declared} at offset {at} exceeds the {remaining} bytes left")
            }
            CodecError::NotUtf8 { at } => write!(f, "not valid UTF-8, at offset {at}"),
            CodecError::BadTag { tag, expected, at } => {
                write!(f, "tag {tag} at offset {at} is not a {expected}")
            }
        }
    }
}

impl std::error::Error for CodecError {}

/// The decoding counterpart to [`Canonical`].
///
/// Every method is bounds-checked and returns a `Result`. This decodes
/// bytes that arrived over a network from a peer who may be running a
/// different version, a different mod set, or a deliberately corrupted
/// build; there is no input here that may be trusted enough to index
/// without checking, and §8's "no anti-cheat" does not extend to
/// "panics on a malformed packet".
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(input: &'a [u8]) -> Reader<'a> {
        Reader { input, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.input.len() - self.pos
    }

    pub fn at_end(&self) -> bool {
        self.pos == self.input.len()
    }

    /// Error unless every byte has been consumed.
    pub fn finish(self) -> Result<(), CodecError> {
        if self.at_end() {
            Ok(())
        } else {
            Err(CodecError::TrailingBytes { unread: self.remaining() })
        }
    }

    pub fn raw(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        if self.remaining() < n {
            return Err(CodecError::UnexpectedEnd {
                wanted: n,
                remaining: self.remaining(),
                at: self.pos,
            });
        }
        let out = &self.input[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.raw(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, CodecError> {
        let b = self.raw(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, CodecError> {
        let b = self.raw(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> Result<u64, CodecError> {
        let b = self.raw(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    pub fn i8(&mut self) -> Result<i8, CodecError> {
        Ok(self.u8()? as i8)
    }

    pub fn i16(&mut self) -> Result<i16, CodecError> {
        Ok(self.u16()? as i16)
    }

    pub fn i32(&mut self) -> Result<i32, CodecError> {
        Ok(self.u32()? as i32)
    }

    pub fn i64(&mut self) -> Result<i64, CodecError> {
        Ok(self.u64()? as i64)
    }

    /// A `bool` written by [`Canonical::bool`]. Anything other than 0
    /// or 1 is an error rather than "nonzero is true": a byte that was
    /// never written by our encoder means the schemas differ, and
    /// guessing at that point buries the evidence.
    pub fn bool(&mut self) -> Result<bool, CodecError> {
        let at = self.pos;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(CodecError::BadTag { tag, expected: "bool", at }),
        }
    }

    pub fn fixed(&mut self) -> Result<Fixed, CodecError> {
        Ok(Fixed::from_raw(self.i32()?))
    }

    /// A `u32` length, checked against what is actually left.
    ///
    /// The check is the point. A declared length of four billion would
    /// otherwise become a four-billion-element `Vec::with_capacity`,
    /// and a peer who sends one should get an error rather than our
    /// allocator.
    pub fn len32(&mut self) -> Result<usize, CodecError> {
        let at = self.pos;
        let declared = self.u32()? as usize;
        if declared > self.remaining() {
            return Err(CodecError::LengthOverrun {
                declared,
                remaining: self.remaining(),
                at,
            });
        }
        Ok(declared)
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], CodecError> {
        let n = self.len32()?;
        self.raw(n)
    }

    pub fn str(&mut self) -> Result<&'a str, CodecError> {
        let at = self.pos;
        let bytes = self.bytes()?;
        core::str::from_utf8(bytes).map_err(|_| CodecError::NotUtf8 { at })
    }

    /// A length-prefixed sequence.
    ///
    /// The `Vec` is *not* preallocated from the declared length even
    /// though [`Reader::len32`] has already bounded it, because an
    /// element may be one byte and a bound of "no more than the
    /// remaining input" is still a large allocation for a small packet.
    /// Growing costs a few reallocations on a path that runs ten times
    /// a second.
    pub fn seq<T>(
        &mut self,
        mut each: impl FnMut(&mut Reader<'a>) -> Result<T, CodecError>,
    ) -> Result<Vec<T>, CodecError> {
        let n = self.len32()?;
        let mut out = Vec::new();
        for _ in 0..n {
            out.push(each(self)?);
        }
        Ok(out)
    }

    pub fn option<T>(
        &mut self,
        each: impl FnOnce(&mut Reader<'a>) -> Result<T, CodecError>,
    ) -> Result<Option<T>, CodecError> {
        let at = self.pos;
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(each(self)?)),
            tag => Err(CodecError::BadTag { tag, expected: "option tag", at }),
        }
    }

    pub fn decode<T: Decode>(&mut self) -> Result<T, CodecError> {
        T::decode(self)
    }
}

/// Decode a complete buffer, requiring that nothing is left over.
pub fn decode_all<T: Decode>(input: &[u8]) -> Result<T, CodecError> {
    let mut reader = Reader::new(input);
    let value = T::decode(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

impl Encode for crate::rng::Pcg32 {
    /// Sixteen bytes: state then increment.
    ///
    /// The generator belongs *in* the simulation state (D-3) and is
    /// therefore part of the checksum. That is deliberate and it is
    /// what makes §6's advice about the PRNG section work: a peer that
    /// has drawn a different number of random values shows a divergence
    /// in this field on the very next tick, before the consequence has
    /// had time to spread through the rest of the state.
    fn encode(&self, out: &mut Canonical) {
        let (state, increment) = self.parts();
        out.u64(state);
        out.u64(increment);
    }
}

impl Decode for crate::rng::Pcg32 {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let state = input.u64()?;
        let increment = input.u64()?;
        Ok(crate::rng::Pcg32::from_parts(state, increment))
    }
}

impl Encode for Fixed {
    fn encode(&self, out: &mut Canonical) {
        out.fixed(*self);
    }
}

impl Decode for Fixed {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        input.fixed()
    }
}
