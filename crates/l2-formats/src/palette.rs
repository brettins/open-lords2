
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct Palette {
    entries: [[u8; 3]; 256],
}

impl Palette {
    pub const FILE_LEN: usize = 768;

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::FILE_LEN {
            return Err(Error::BadPaletteLength(bytes.len()));
        }
        let mut entries = [[0u8; 3]; 256];
        for (i, entry) in entries.iter_mut().enumerate() {
            for (c, out) in entry.iter_mut().enumerate() {
                *out = ((bytes[i * 3 + c] as u16 * 255) / 63) as u8;
            }
        }
        Ok(Palette { entries })
    }

    ///.
    pub fn from_entries(entries: [[u8; 3]; 256]) -> Palette {
        Palette { entries }
    }

    #[inline]
    pub fn entries(&self) -> &[[u8; 3]; 256] {
        &self.entries
    }

    #[inline]
    pub fn rgb(&self, index: u8) -> [u8; 3] {
        self.entries[index as usize]
    }
}
