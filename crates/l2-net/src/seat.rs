//! C260: a seat leaving a game under way, waited for, and taken back. Ours: the
//! original eliminated a departing realm (`Net_OnSessionEvent` `0x0043E6A2`).

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{PlayerSlot, Tick};
use crate::lobby::Roster;
use crate::packet::Message;

/// C260: the host's answer to a dropped player's `Join`; C256's payload for `next - 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejoin {
    pub slot: PlayerSlot,
    pub authority: PlayerSlot,
    pub slots: Vec<PlayerSlot>,
    pub retired: Vec<(PlayerSlot, Tick)>,
    pub seed: u64,
    pub next: Tick,
    pub hash: u64,
    pub roster: Roster,
    pub state: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSeat {
    /// C260: no seat in this game was this install's.
    Started,
    /// C260: the host let the computer play this install's realm.
    Retired,
}

pub(crate) const TAG_LEAVING: u8 = 11;
const TAG_RETIRE: u8 = 12;
const TAG_AWAY: u8 = 13;
const TAG_BACK: u8 = 14;
const TAG_REJOIN: u8 = 15;
pub(crate) const TAG_NO_SEAT: u8 = 16;

fn slot(input: &mut Reader<'_>) -> Result<PlayerSlot, CodecError> {
    let at = input.position();
    PlayerSlot::from_wire(input.u8()?).ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })
}

pub(crate) fn encode(m: &Message, out: &mut Canonical) {
    match m {
        Message::Leaving => out.u8(TAG_LEAVING),
        Message::Retire { slot, from } => {
            out.u8(TAG_RETIRE);
            out.u8(slot.index());
            out.u32(from.0);
        }
        Message::Away(s) => {
            out.u8(TAG_AWAY);
            out.u8(s.index());
        }
        Message::Back(s) => {
            out.u8(TAG_BACK);
            out.u8(s.index());
        }
        Message::Rejoin(r) => {
            out.u8(TAG_REJOIN);
            r.encode(out);
        }
        Message::NoSeat(why) => {
            out.u8(TAG_NO_SEAT);
            out.u8(*why as u8);
        }
        _ => unreachable!("packet.rs encodes the rest"),
    }
}

pub(crate) fn decode(tag: u8, input: &mut Reader<'_>) -> Result<Message, CodecError> {
    let at = input.position();
    Ok(match tag {
        TAG_LEAVING => Message::Leaving,
        TAG_RETIRE => Message::Retire { slot: slot(input)?, from: Tick(input.u32()?) },
        TAG_AWAY => Message::Away(slot(input)?),
        TAG_BACK => Message::Back(slot(input)?),
        TAG_REJOIN => Message::Rejoin(Box::new(Rejoin::decode(input)?)),
        _ => match input.u8()? {
            0 => Message::NoSeat(NoSeat::Started),
            1 => Message::NoSeat(NoSeat::Retired),
            tag => return Err(CodecError::BadTag { tag, expected: "no-seat reason", at }),
        },
    })
}

impl Encode for Rejoin {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.slot.index());
        out.u8(self.authority.index());
        out.seq(&self.slots, |c, s| c.u8(s.index()));
        out.seq(&self.retired, |c, (s, t)| {
            c.u8(s.index());
            c.u32(t.0);
        });
        out.u64(self.seed);
        out.u32(self.next.0);
        out.u64(self.hash);
        self.roster.encode(out);
        out.bytes(&self.state);
    }
}

impl Decode for Rejoin {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Rejoin {
            slot: slot(input)?,
            authority: slot(input)?,
            slots: input.seq(slot)?,
            retired: input.seq(|r| Ok((slot(r)?, Tick(r.u32()?))))?,
            seed: input.u64()?,
            next: Tick(input.u32()?),
            hash: input.u64()?,
            roster: Roster::decode(input)?,
            state: input.bytes()?.to_vec(),
        })
    }
}
