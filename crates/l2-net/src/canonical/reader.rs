#![allow(unused_imports)]
use super::*;
use super::canonical::*;
use super::impls::*;
use crate::fixed::Fixed;
use crate::hash::XxHash64;

impl<'a> Reader<'a> {
    pub fn new(input: &'a [u8]) -> Reader<'a> {
        Reader { input, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.input.len() - self.pos
    }

    pub fn at_end(&self) -> bool {
        self.pos == self.input.len()
    }

    /// Error unless every byte has been consumed.
    pub fn finish(self) -> Result<(), CodecError> {
        if self.at_end() {
            Ok(())
        } else {
            Err(CodecError::TrailingBytes { unread: self.remaining() })
        }
    }

    pub fn raw(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        if self.remaining() < n {
            return Err(CodecError::UnexpectedEnd {
                wanted: n,
                remaining: self.remaining(),
                at: self.pos,
            });
        }
        let out = &self.input[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.raw(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, CodecError> {
        let b = self.raw(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, CodecError> {
        let b = self.raw(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> Result<u64, CodecError> {
        let b = self.raw(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    pub fn i8(&mut self) -> Result<i8, CodecError> {
        Ok(self.u8()? as i8)
    }

    pub fn i16(&mut self) -> Result<i16, CodecError> {
        Ok(self.u16()? as i16)
    }

    pub fn i32(&mut self) -> Result<i32, CodecError> {
        Ok(self.u32()? as i32)
    }

    pub fn i64(&mut self) -> Result<i64, CodecError> {
        Ok(self.u64()? as i64)
    }

    /// A `bool` written by [`Canonical::bool`]. Anything other than 0
/// or 1 is an error: a byte that was
    /// never written by our encoder means the schemas differ, and
    /// guessing at that point buries the evidence.
    pub fn bool(&mut self) -> Result<bool, CodecError> {
        let at = self.pos;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(CodecError::BadTag { tag, expected: "bool", at }),
        }
    }

    pub fn fixed(&mut self) -> Result<Fixed, CodecError> {
        Ok(Fixed::from_raw(self.i32()?))
    }

/// A `u32` length, checked against what is left.
    ///
    /// The check is the point. A declared length of four billion would
    /// otherwise become a four-billion-element `Vec::with_capacity`,
/// and a peer who sends one should get an error
    /// allocator.
    pub fn len32(&mut self) -> Result<usize, CodecError> {
        let at = self.pos;
        let declared = self.u32()? as usize;
        if declared > self.remaining() {
            return Err(CodecError::LengthOverrun {
                declared,
                remaining: self.remaining(),
                at,
            });
        }
        Ok(declared)
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], CodecError> {
        let n = self.len32()?;
        self.raw(n)
    }

    pub fn str(&mut self) -> Result<&'a str, CodecError> {
        let at = self.pos;
        let bytes = self.bytes()?;
        core::str::from_utf8(bytes).map_err(|_| CodecError::NotUtf8 { at })
    }

    /// A length-prefixed sequence.
    ///
    /// The `Vec` is *not* preallocated from the declared length even
    /// though [`Reader::len32`] has already bounded it, because an
    /// element may be one byte and a bound of "no more than the
    /// remaining input" is still a large allocation for a small packet.
    /// Growing costs a few reallocations on a path that runs ten times
    /// a second.
    pub fn seq<T>(
        &mut self,
        mut each: impl FnMut(&mut Reader<'a>) -> Result<T, CodecError>,
    ) -> Result<Vec<T>, CodecError> {
        let n = self.len32()?;
        let mut out = Vec::new();
        for _ in 0..n {
            out.push(each(self)?);
        }
        Ok(out)
    }

    pub fn option<T>(
        &mut self,
        each: impl FnOnce(&mut Reader<'a>) -> Result<T, CodecError>,
    ) -> Result<Option<T>, CodecError> {
        let at = self.pos;
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(each(self)?)),
            tag => Err(CodecError::BadTag { tag, expected: "option tag", at }),
        }
    }

    pub fn decode<T: Decode>(&mut self) -> Result<T, CodecError> {
        T::decode(self)
    }
}

