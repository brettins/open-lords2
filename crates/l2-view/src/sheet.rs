//! A PL8 file with its frames decoded once and kept.
//!
//! `Pl8::parse` borrows its bytes and `decode` re-decodes on every call, which
//! is right for a validator sweeping a corpus and wrong for a renderer that
//! draws the same 1,120 tiles sixty times a second. A sheet owns its bytes and
//! decodes each frame at most once.
//!
//! Decoding is lazy: a troop sprite sheet holds up to 122 frames and a battle
//! shows a handful of them, so paying for all of them up front would be worse
//! than the repeated decode it replaces.

use std::cell::RefCell;

use l2_formats::{DecodedFrame, Pl8, Result};

pub struct Sheet {
    bytes: Vec<u8>,
    count: usize,
    frames: RefCell<Vec<Option<DecodedFrame>>>,
}

impl Sheet {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        let count = Pl8::parse(&bytes)?.frames.len();
        Ok(Sheet { bytes, count, frames: RefCell::new(vec![None; count]) })
    }

    pub fn frame_count(&self) -> usize {
        self.count
    }

    /// Decode frame `index`, caching it. Returns `None` for an index past the
    /// end or a frame this decoder cannot read, so a renderer degrades to a
    /// hole rather than dying — a battle should still be watchable when one
    /// sprite is unreadable.
    pub fn frame(&self, index: usize) -> Option<DecodedFrame> {
        if index >= self.count {
            return None;
        }
        if let Some(f) = &self.frames.borrow()[index] {
            return Some(f.clone());
        }
        let decoded = Pl8::parse(&self.bytes).ok()?.decode(index).ok()?;
        self.frames.borrow_mut()[index] = Some(decoded.clone());
        Some(decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal in-memory PL8: one raw 2x2 frame. Enough to exercise the
    /// cache without touching the game install.
    fn tiny_pl8() -> Vec<u8> {
        let mut v = vec![0u8; 8 + 16];
        v[0] = 0; // storage family: raw
        v[2] = 1; // one frame
        v[8] = 2; // width
        v[10] = 2; // height
        v[12..16].copy_from_slice(&24u32.to_le_bytes()); // data offset
        v[20] = 0; // shape 0, raw rectangle
        v.extend_from_slice(&[1, 2, 3, 4]);
        v
    }

    #[test]
    fn a_frame_decodes_once_and_reads_back_the_same() {
        let s = Sheet::new(tiny_pl8()).unwrap();
        assert_eq!(s.frame_count(), 1);
        let a = s.frame(0).unwrap();
        let b = s.frame(0).unwrap();
        assert_eq!(a.indices, vec![1, 2, 3, 4]);
        assert_eq!(a.indices, b.indices);
        assert!(s.frames.borrow()[0].is_some(), "the frame should be cached");
    }

    #[test]
    fn an_index_past_the_end_is_none_rather_than_a_panic() {
        let s = Sheet::new(tiny_pl8()).unwrap();
        assert!(s.frame(1).is_none());
        assert!(s.frame(9999).is_none());
    }
}
