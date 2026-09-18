#![allow(unused_imports)]
use super::*;
use super::canonical::*;
use super::reader::*;
use crate::fixed::Fixed;
use crate::hash::XxHash64;

impl Encode for crate::rng::Pcg32 {
    fn encode(&self, out: &mut Canonical) {
        let (state, increment) = self.parts();
        out.u64(state);
        out.u64(increment);
    }
}

impl Decode for crate::rng::Pcg32 {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let state = input.u64()?;
        let increment = input.u64()?;
        Ok(crate::rng::Pcg32::from_parts(state, increment))
    }
}

impl Encode for Fixed {
    fn encode(&self, out: &mut Canonical) {
        out.fixed(*self);
    }
}

impl Decode for Fixed {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        input.fixed()
    }
}

