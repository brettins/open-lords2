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

    #[inline]
    pub fn rgb(&self, index: u8) -> [u8; 3] {
        self.entries[index as usize]
    }
}
