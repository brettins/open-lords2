#![allow(unused_imports)]
use super::*;
use super::decode::*;
use super::audio::*;
use std::fmt;


pub(super) struct Bits<'a> {
    data: &'a [u8],
    pub(super) pos: usize,
    what: &'static str,
}

impl<'a> Bits<'a> {
    pub(super) fn new(data: &'a [u8], what: &'static str) -> Bits<'a> {
        Bits { data, pos: 0, what }
    }

    #[inline]
    pub(super) fn bit(&mut self) -> Result<usize> {
        let byte = *self.data.get(self.pos >> 3).ok_or(Error::Overrun(self.what))?;
        let b = (byte >> (self.pos & 7)) & 1;
        self.pos += 1;
        Ok(b as usize)
    }

    pub(super) fn bits(&mut self, n: u32) -> Result<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= (self.bit()? as u32) << i;
        }
        Ok(v)
    }

    pub(super) fn total(&self) -> usize {
        self.data.len() * 8
    }
}


const LEAF: u32 = 0x8000_0000;
const CACHED: u32 = 0x4000_0000;

const MAX_DEPTH: usize = 512;

#[derive(Debug, Clone)]
pub(super) struct Tree8 {
    nodes: Vec<[u32; 2]>,
    root: u32,
}

impl Tree8 {
    pub(super) fn read(bits: &mut Bits) -> Result<Tree8> {
        let mut t = Tree8 { nodes: Vec::new(), root: LEAF };
        if bits.bit()? == 0 {
            return Ok(t);
        }
        t.root = t.build(bits, 0)?;
        if bits.bit()? != 0 {
            return Err(Error::BadTree("an 8-bit tree does not end on a 0 bit"));
        }
        Ok(t)
    }

    fn build(&mut self, bits: &mut Bits, depth: usize) -> Result<u32> {
        if depth > MAX_DEPTH {
            return Err(Error::BadTree("an 8-bit tree nests too deep"));
        }
        if bits.bit()? == 1 {
            let at = self.nodes.len();
            self.nodes.push([0, 0]);
            let zero = self.build(bits, depth + 1)?;
            let one = self.build(bits, depth + 1)?;
            self.nodes[at] = [zero, one];
            Ok(at as u32)
        } else {
            Ok(LEAF | bits.bits(8)?)
        }
    }

    #[inline]
    pub(super) fn decode(&self, bits: &mut Bits) -> Result<u8> {
        let mut n = self.root;
        while n & LEAF == 0 {
            n = self.nodes[n as usize][bits.bit()?];
        }
        Ok(n as u8)
    }
}

#[derive(Debug, Clone)]
pub(super) struct Tree16 {
    nodes: Vec<[u32; 2]>,
    root: u32,
}

impl Tree16 {
    pub(super) fn read(bits: &mut Bits) -> Result<Tree16> {
        let mut t = Tree16 { nodes: Vec::new(), root: LEAF };
        if bits.bit()? == 0 {
            return Ok(t);
        }
        let low = Tree8::read(bits)?;
        let high = Tree8::read(bits)?;
        let escapes = [bits.bits(16)?, bits.bits(16)?, bits.bits(16)?];
        t.root = t.build(bits, &low, &high, escapes, 0)?;
        if bits.bit()? != 0 {
            return Err(Error::BadTree("a 16-bit tree does not end on a 0 bit"));
        }
        Ok(t)
    }

    fn build(
        &mut self,
        bits: &mut Bits,
        low: &Tree8,
        high: &Tree8,
        escapes: [u32; 3],
        depth: usize,
    ) -> Result<u32> {
        if depth > MAX_DEPTH {
            return Err(Error::BadTree("a 16-bit tree nests too deep"));
        }
        if bits.bit()? == 1 {
            let at = self.nodes.len();
            self.nodes.push([0, 0]);
            let zero = self.build(bits, low, high, escapes, depth + 1)?;
            let one = self.build(bits, low, high, escapes, depth + 1)?;
            self.nodes[at] = [zero, one];
            Ok(at as u32)
        } else {
            let lo = low.decode(bits)? as u32;
            let hi = high.decode(bits)? as u32;
            let v = lo | (hi << 8);
            Ok(match escapes.iter().position(|&e| e == v) {
                Some(slot) => LEAF | CACHED | slot as u32,
                None => LEAF | v,
            })
        }
    }

    #[inline]
    pub(super) fn decode(&self, bits: &mut Bits, recent: &mut [u16; 3]) -> Result<u16> {
        let mut n = self.root;
        while n & LEAF == 0 {
            n = self.nodes[n as usize][bits.bit()?];
        }
        let v = if n & CACHED != 0 { recent[(n & 3) as usize] } else { n as u16 };
        if recent[0] != v {
            recent[2] = recent[1];
            recent[1] = recent[0];
            recent[0] = v;
        }
        Ok(v)
    }
}

