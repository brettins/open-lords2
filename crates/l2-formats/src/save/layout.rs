#![allow(unused_imports)]
use super::*;
use super::pe::*;
use super::save::*;
use types::*;

impl Layout {
    /// Walk `Save_Write`'s table. Stops at the first zero length.
    /// game's loop does.
    pub fn from_executable(exe: &[u8]) -> Result<Layout, SaveError> {
        let pe = Pe::parse(exe)?;
        let mut blocks = Vec::new();
        let mut cum = 0usize;
        for i in 0..MAX_ENTRIES {
            let at = SAVE_TABLE_VA + (i as u32) * 8;
            let va = pe.u32_at(at).ok_or(SaveError::BadBlockTable)?;
            let len = pe.u32_at(at + 4).ok_or(SaveError::BadBlockTable)?;
            if len == 0 {
                break;
            }
            blocks.push(Block { va, len, offset: cum });
            cum += len as usize;
        }
        if blocks.is_empty() {
            return Err(SaveError::BadBlockTable);
        }
        Ok(Layout { blocks, total: cum })
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The size a save written from this layout must have.
    pub fn expected_len(&self) -> usize {
        self.total + CASTLE_BLOCKS * CASTLE_BLOCK
    }

    /// Where a runtime address landed in the file.
    pub fn offset_of(&self, va: u32) -> Option<usize> {
        self.blocks
            .iter()
            .find(|b| va >= b.va && va < b.va + b.len)
            .map(|b| b.offset + (va - b.va) as usize)
    }
}

