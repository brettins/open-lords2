
use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Tick(pub u32);

impl Tick {
    pub const ZERO: Tick = Tick(0);

    pub fn next(self) -> Tick {
        Tick(self.0 + 1)
    }

    pub fn plus(self, n: u32) -> Tick {
        Tick(self.0 + n)
    }

    pub fn minus(self, n: u32) -> Tick {
        Tick(self.0.saturating_sub(n))
    }
}

impl core::fmt::Display for Tick {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "tick {}", self.0)
    }
}

pub const MAX_PLAYERS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerSlot(u8);

impl PlayerSlot {
    pub fn new(index: u8) -> PlayerSlot {
        assert!(
            (index as usize) < MAX_PLAYERS,
            "player slot {index} is beyond the {MAX_PLAYERS}-player limit"
        );
        PlayerSlot(index)
    }

    pub fn from_wire(index: u8) -> Option<PlayerSlot> {
        if (index as usize) < MAX_PLAYERS {
            Some(PlayerSlot(index))
        } else {
            None
        }
    }

    pub fn index(self) -> u8 {
        self.0
    }
}

impl core::fmt::Display for PlayerSlot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "player {}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub slot: PlayerSlot,
    pub seq: u32,
    pub payload: Vec<u8>,
}

impl Command {
    pub fn new(slot: PlayerSlot, seq: u32, payload: Vec<u8>) -> Command {
        Command { slot, seq, payload }
    }
}

impl Encode for Command {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.slot.index());
        out.u32(self.seq);
        out.bytes(&self.payload);
    }
}

impl Decode for Command {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        let slot = PlayerSlot::from_wire(input.u8()?)
            .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })?;
        let seq = input.u32()?;
        let payload = input.bytes()?.to_vec();
        Ok(Command { slot, seq, payload })
    }
}

pub fn order_commands(commands: &mut [Command]) {
    commands.sort_by(|a, b| a.slot.cmp(&b.slot).then(a.seq.cmp(&b.seq)));
}

pub fn commands_are_distinct(commands: &[Command]) -> bool {
    commands
        .windows(2)
        .all(|pair| pair[0].slot != pair[1].slot || pair[0].seq != pair[1].seq)
}
