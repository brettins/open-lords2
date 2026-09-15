#![allow(unused_imports)]
use super::*;
use super::layout::*;
use super::save::*;
use types::*;

pub(super) struct Pe<'a> {
    bytes: &'a [u8],
    sections: Vec<(u32, u32, u32, u32)>, // vaddr, vsize, rawptr, rawsize
}

impl<'a> Pe<'a> {
    pub(super) fn parse(bytes: &'a [u8]) -> Result<Pe<'a>, SaveError> {
        let pe_off = read_u32(bytes, 0x3C).ok_or(SaveError::NotPe)? as usize;
        if read_u32(bytes, pe_off) != Some(0x0000_4550) {
            return Err(SaveError::NotPe);
        }
        let n = read_u16(bytes, pe_off + 6).ok_or(SaveError::NotPe)? as usize;
        let opt = read_u16(bytes, pe_off + 20).ok_or(SaveError::NotPe)? as usize;
        let table = pe_off + 24 + opt;
        let mut sections = Vec::with_capacity(n);
        for i in 0..n {
            let o = table + i * 40;
            let g = |k: usize| read_u32(bytes, o + k).ok_or(SaveError::NotPe);
            sections.push((g(12)?, g(8)?, g(20)?, g(16)?));
        }
        Ok(Pe { bytes, sections })
    }

    fn offset(&self, va: u32) -> Option<usize> {
        let rva = va.checked_sub(IMAGE_BASE)?;
        for &(vaddr, vsize, rawptr, rawsize) in &self.sections {
            if rva >= vaddr && rva < vaddr + vsize.max(rawsize) {
                return Some((rawptr + (rva - vaddr)) as usize);
            }
        }
        None
    }

    pub(super) fn u32_at(&self, va: u32) -> Option<u32> {
        read_u32(self.bytes, self.offset(va)?)
    }
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let s = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    let s = bytes.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

