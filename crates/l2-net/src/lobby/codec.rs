#![allow(unused_imports)]
use super::*;
use super::lobby::*;
use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{PlayerSlot, MAX_PLAYERS};
use crate::packet::{Hello, Mismatch};
use crate::transport::PeerId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    pub hello: Hello,
    pub name: String,
    pub shield: u8,
}

impl Encode for Join {
    fn encode(&self, out: &mut Canonical) {
        self.hello.encode(out);
        out.str(&self.name);
        out.u8(self.shield);
    }
}

impl Decode for Join {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Join {
            hello: Hello::decode(input)?,
            name: input.str()?.to_string(),
            shield: input.u8()?,
        })
    }
}

impl Encode for Player {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.slot.index());
        out.str(&self.name);
        out.bool(self.ready);
        out.bool(self.is_host);
        out.u8(self.shield);
    }
}

impl Decode for Player {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        let slot = PlayerSlot::from_wire(input.u8()?)
            .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })?;
        Ok(Player {
            slot,
            name: input.str()?.to_string(),
            ready: input.bool()?,
            is_host: input.bool()?,
            shield: input.u8()?,
        })
    }
}

impl Encode for Roster {
    fn encode(&self, out: &mut Canonical) {
        out.seq(&self.players, |c, p| p.encode(c));
    }
}

impl Decode for Roster {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let players = input.seq(Player::decode)?;
        if players.windows(2).any(|w| w[0].slot.index() >= w[1].slot.index()) {
            return Err(CodecError::BadTag {
                tag: 0,
                expected: "roster sorted by slot, without duplicates",
                at: 0,
            });
        }
        Ok(Roster { players })
    }
}

impl Encode for Start {
    fn encode(&self, out: &mut Canonical) {
        out.u64(self.seed);
        self.roster.encode(out);
        out.i32(self.unseated_gold);
    }
}

impl Decode for Start {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Start {
            seed: input.u64()?,
            roster: Roster::decode(input)?,
            unseated_gold: input.i32()?,
        })
    }
}

