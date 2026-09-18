
pub mod cursors;
pub mod maps;
pub mod palette;
pub mod pl8;
pub mod save;
pub mod skr;

pub use maps::{MapSet, MapSlot, Plane};
pub use save::{County, Layout, Save, SaveError};
pub use palette::Palette;
pub use skr::{Army, ScenarioText, Side, Skr, Troop};
pub use pl8::{DecodedFrame, FrameInfo, Pl8, Shape, Storage};

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated { needed: usize, have: usize },
    UnsupportedStorage(u8),
    RowOverrun { frame: usize, row: u16, got: u32, want: u16 },
    FrameSizeMismatch { frame: usize, ended: usize, expected: usize },
    ZeroLengthRun { frame: usize, row: u16 },
    UnsupportedShape { frame: usize, shape: u8 },
    BadIsoGeometry { frame: usize, width: u16, height: u16 },
    FrameOutOfRange { index: usize, count: usize },
    BadPaletteLength(usize),
    PartialMapSlot { len: usize, slot_len: usize },
    BadSkrLength { len: usize, expected: usize },
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
            Error::UnsupportedShape { frame, shape } => {
                write!(f, "frame {frame}: unsupported shape {shape}")
            }
            Error::BadIsoGeometry { frame, width, height } => {
                write!(f, "frame {frame}: {width}x{height} is not a legal iso diamond")
            }
            Error::FrameOutOfRange { index, count } => {
                write!(f, "frame {index} requested, file has {count}")
            }
            Error::BadPaletteLength(n) => write!(f, "palette is {n} bytes, expected 768"),
            Error::PartialMapSlot { len, slot_len } => {
                write!(f, "map file is {len} bytes, not a whole number of {slot_len}-byte slots")
            }
            Error::BadSkrLength { len, expected } => {
                write!(f, "skr file is {len} bytes, expected exactly {expected}")
            }
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

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
