//! Decoders for Lords of the Realm II asset formats.
//!
//! Deliberately dependency-free: this crate turns bytes into pixels and
//! nothing else. No I/O, no rendering, no game logic.

pub mod palette;
pub mod pl8;

pub use palette::Palette;
pub use pl8::{DecodedFrame, FrameInfo, Pl8, Storage};

use std::fmt;

/// Everything that can go wrong reading an asset file.
///
/// Variants carry the numbers needed to diagnose a bad file, because
/// "parse error" is useless when validating a corpus of 291 files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Read past the end of the buffer.
    Truncated { needed: usize, have: usize },
    /// Header byte 0 held a storage mode we cannot decode yet (mode 2).
    UnsupportedStorage(u8),
    /// An RLE row consumed more pixels than the frame is wide.
    RowOverrun { frame: usize, row: u16, got: u32, want: u16 },
    /// A frame's data did not end exactly where the next frame begins.
    /// This is the format's self-verifying property; see docs/formats/pl8.md.
    FrameSizeMismatch { frame: usize, ended: usize, expected: usize },
    /// An RLE skip run of length zero, which would advance no pixels and
    /// loop forever. No shipped file contains one; a malformed file could.
    ZeroLengthRun { frame: usize, row: u16 },
    /// Frame index past the end of the frame table.
    FrameOutOfRange { index: usize, count: usize },
    /// A palette file was not exactly 768 bytes.
    BadPaletteLength(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated { needed, have } => {
                write!(f, "truncated: needed {needed} bytes, have {have}")
            }
            Error::UnsupportedStorage(m) => write!(f, "unsupported storage mode {m}"),
            Error::RowOverrun { frame, row, got, want } => {
                write!(f, "frame {frame} row {row}: consumed {got} pixels, width is {want}")
            }
            Error::FrameSizeMismatch { frame, ended, expected } => {
                write!(f, "frame {frame} ended at {ended:#x}, expected {expected:#x}")
            }
            Error::ZeroLengthRun { frame, row } => {
                write!(f, "frame {frame} row {row}: zero-length skip run")
            }
            Error::FrameOutOfRange { index, count } => {
                write!(f, "frame {index} requested, file has {count}")
            }
            Error::BadPaletteLength(n) => write!(f, "palette is {n} bytes, expected 768"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Bounds-checked little-endian reads. Every parse goes through these so a
/// malformed file returns an error instead of panicking.
pub(crate) fn u16_at(b: &[u8], off: usize) -> Result<u16> {
    let end = off + 2;
    if end > b.len() {
        return Err(Error::Truncated { needed: end, have: b.len() });
    }
    Ok(u16::from_le_bytes([b[off], b[off + 1]]))
}

pub(crate) fn u32_at(b: &[u8], off: usize) -> Result<u32> {
    let end = off + 4;
    if end > b.len() {
        return Err(Error::Truncated { needed: end, have: b.len() });
    }
    Ok(u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]))
}
