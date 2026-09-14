//! Canonical serialisation: the one byte stream everything agrees on.
//!
//! Three things need to turn simulation state into bytes,
//! `docs/netcode.md` requires that they all produce the *same* bytes:
//!
//! * the state checksum exchanged every tick (§6),
//! * the once-per-turn snapshot used for late join and crash recovery
//!   (§5),
//! * the desync dump (§6).
//!
//! If the checksum hashed one encoding and the snapshot wrote another,
//! then a desync dump would be a picture of a state that
//! checksummed,
//! establishing that the two encoders agree. So there is one encoder,
//! [`Canonical`], and hashing is something it does *while* writing
//! A checksum and a snapshot
//! taken from the same state cannot disagree, because the same code
//! produced both.
//!
//! # What "canonical" rules out
//!
//! §6 is emphatic that we never hash the in-memory image,
//! reason is that a memory image is full of things that legitimately
//! differ between two peers running identical simulations: struct
//! padding, `Vec` spare capacity, allocator layout, `usize` width. Hash
//! any of those and the detector reports divergence on two peers that
//! agree perfectly — and a desync detector that cries wolf gets turned
//! off, which is worse than not having one.
//!
//! So every method here writes a **fixed width in little-endian**,
//! chosen once and never derived from the host.
//! method: a `usize` is 8 bytes on this machine and 4 on a 32-bit one,
//! and a length written as `usize` is a desync between a 32-bit and a
//! 64-bit player. Lengths go out as `u32` through [`Canonical::len32`],
//! which panics.
//!
//! Little-endian to match the file formats we already read
//! (`docs/netcode.md` D-10), —
//! `to_le_bytes` is a byte shuffle on a big-endian machine, not a
//! no-op, and that is the point.
//!
//! # Self-delimiting
//!
//! Every variable-length field is length-prefixed. That is not for the
//! decoder's benefit — a decoder that knows the schema could get by
//! without it — but for the *hash*: without a prefix,
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
//! §4 specifies,
//! makes it worth writing down.

mod canonical;
pub use canonical::*;
mod reader;
pub use reader::*;
mod impls;
pub use impls::*;

use crate::fixed::Fixed;
use crate::hash::XxHash64;

/// The seed for every state checksum in this project.
///
/// Zero, so that `xxh64sum` or any other off-the-shelf XXH64 tool
/// reproduces our number from a dumped byte stream. A random-looking
/// constant would have added nothing —
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

/// Something with a canonical byte form.
///
/// Hand-written, per D-10.
/// be one: a derive is a layout decided by a macro,
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
/// the schema, which is the disagreement this crate
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
/// build;
/// without checking, and §8's "no anti-cheat" does not extend to
/// "panics on a malformed packet".
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    input: &'a [u8],
    pos: usize,
}

/// Decode a complete buffer, requiring that nothing is left over.
pub fn decode_all<T: Decode>(input: &[u8]) -> Result<T, CodecError> {
    let mut reader = Reader::new(input);
    let value = T::decode(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

