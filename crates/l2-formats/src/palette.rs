//! `.256` palette files: 256 RGB triples in 6-bit VGA range.

use crate::{Error, Result};

/// Palette entries, already scaled from 6-bit VGA to full 8-bit range.
#[derive(Debug, Clone)]
pub struct Palette {
    entries: [[u8; 3]; 256],
}

impl Palette {
    pub const FILE_LEN: usize = 768;

    /// Parse a `.256` file. Values are 0..=63 on disk and are scaled here, so
    /// callers never have to remember the conversion.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::FILE_LEN {
            return Err(Error::BadPaletteLength(bytes.len()));
        }
        let mut entries = [[0u8; 3]; 256];
        for (i, entry) in entries.iter_mut().enumerate() {
            for (c, out) in entry.iter_mut().enumerate() {
                // 6-bit (0..63) -> 8-bit (0..255), rounded rather than shifted
                // so that 63 maps to a true 255 instead of 252.
                *out = ((bytes[i * 3 + c] as u16 * 255) / 63) as u8;
            }
        }
        Ok(Palette { entries })
    }

    /// A palette from triples that are **already** 8-bit, for a caller that
    /// derives one palette from another rather than reading a file.
    ///
    /// The end-of-turn fade is the only such caller and it lives in `l2-view`,
    /// where a fade belongs; this pair of accessors is what lets it stay there
    /// instead of putting a display effect in the format crate.
    pub fn from_entries(entries: [[u8; 3]; 256]) -> Palette {
        Palette { entries }
    }

    /// All 256 triples, 8-bit. See [`Palette::from_entries`].
    #[inline]
    pub fn entries(&self) -> &[[u8; 3]; 256] {
        &self.entries
    }

    #[inline]
    pub fn rgb(&self, index: u8) -> [u8; 3] {
        self.entries[index as usize]
    }
}
