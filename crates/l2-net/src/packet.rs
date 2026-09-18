
use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{Command, PlayerSlot, Tick};

pub const PROTOCOL_VERSION: u16 = 2;

pub const DEFAULT_PORT: u16 = 27962;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickPacket {
    pub tick: Tick,
    pub from: PlayerSlot,
    pub commands: Vec<Command>,
    pub acks: Vec<Ack>,
}

pub const MAX_ACKS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ack {
    pub tick: Tick,
    pub state_hash: u64,
}

impl TickPacket {
    pub fn empty(tick: Tick, from: PlayerSlot) -> TickPacket {
        TickPacket { tick, from, commands: Vec::new(), acks: Vec::new() }
    }
}

impl Encode for TickPacket {
    fn encode(&self, out: &mut Canonical) {
        out.u32(self.tick.0);
        out.u8(self.from.index());
        out.seq(&self.commands, |c, command| command.encode(c));
        out.seq(&self.acks, |c, ack| {
            c.u32(ack.tick.0);
            c.u64(ack.state_hash);
        });
    }
}

impl Decode for TickPacket {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let tick = Tick(input.u32()?);
        let at = input.position();
        let from = PlayerSlot::from_wire(input.u8()?)
            .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })?;
        let commands = input.seq(Command::decode)?;
        let acks = input.seq(|r| {
            let tick = Tick(r.u32()?);
            let state_hash = r.u64()?;
            Ok(Ack { tick, state_hash })
        })?;
        Ok(TickPacket { tick, from, commands, acks })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub protocol: u16,
    pub engine: String,
    pub ruleset_hash: u64,
    pub quirks: u64,
    pub seed: u64,
    pub slot: PlayerSlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    Protocol { ours: u16, theirs: u16 },
    Engine { ours: String, theirs: String },
    Ruleset { ours: u64, theirs: u64 },
    Quirks { ours: u64, theirs: u64 },
    Seed { ours: u64, theirs: u64 },
    SameSlot(PlayerSlot),
}

impl core::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Mismatch::Protocol { ours, theirs } => {
                write!(f, "protocol version {theirs} does not match ours ({ours})")
            }
            Mismatch::Engine { ours, theirs } => {
                write!(f, "engine build '{theirs}' does not match ours ('{ours}')")
            }
            Mismatch::Ruleset { ours, theirs } => write!(
                f,
                "the mod set differs: their rules hash to {theirs:016x}, ours to {ours:016x}"
            ),
            Mismatch::Quirks { ours, theirs } => write!(
                f,
                "you reproduce a different set of the original's bugs: theirs {theirs:016x}, \n                 ours {ours:016x} (zero is faithful - every bug reproduced)"
            ),
            Mismatch::Seed { ours, theirs } => {
                write!(f, "session seed {theirs:016x} does not match ours ({ours:016x})")
            }
            Mismatch::SameSlot(slot) => write!(f, "both peers claim {slot}"),
        }
    }
}

impl Hello {
    pub fn check(&self, theirs: &Hello) -> Vec<Mismatch> {
        let mut out = Vec::new();
        if self.protocol != theirs.protocol {
            out.push(Mismatch::Protocol { ours: self.protocol, theirs: theirs.protocol });
            return out;
        }
        if self.engine != theirs.engine {
            out.push(Mismatch::Engine {
                ours: self.engine.clone(),
                theirs: theirs.engine.clone(),
            });
        }
        if self.ruleset_hash != theirs.ruleset_hash {
            out.push(Mismatch::Ruleset { ours: self.ruleset_hash, theirs: theirs.ruleset_hash });
        }
        if self.quirks != theirs.quirks {
            out.push(Mismatch::Quirks { ours: self.quirks, theirs: theirs.quirks });
        }
        if self.seed != theirs.seed {
            out.push(Mismatch::Seed { ours: self.seed, theirs: theirs.seed });
        }
        if self.slot == theirs.slot {
            out.push(Mismatch::SameSlot(self.slot));
        }
        out
    }
}

impl Encode for Hello {
    fn encode(&self, out: &mut Canonical) {
        out.u16(self.protocol);
        out.str(&self.engine);
        out.u64(self.ruleset_hash);
        out.u64(self.quirks);
        out.u64(self.seed);
        out.u8(self.slot.index());
    }
}

impl Decode for Hello {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let protocol = input.u16()?;
        let engine = input.str()?.to_string();
        let ruleset_hash = input.u64()?;
        let quirks = input.u64()?;
        let seed = input.u64()?;
        let at = input.position();
        let slot = PlayerSlot::from_wire(input.u8()?)
            .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })?;
        Ok(Hello { protocol, engine, ruleset_hash, quirks, seed, slot })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HaltReason {
    Desync { tick: Tick },
    Left,
    Timeout,
    Protocol { detail: String },
}

impl core::fmt::Display for HaltReason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HaltReason::Desync { tick } => write!(f, "state checksums diverged at {tick}"),
            HaltReason::Left => write!(f, "the player left"),
            HaltReason::Timeout => write!(f, "a peer stopped responding"),
            HaltReason::Protocol { detail } => write!(f, "protocol error: {detail}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Hello(Hello),
    Tick(TickPacket),
    Halt(HaltReason),
    Join(crate::lobby::Join),
    Ready(bool),
    Roster(crate::lobby::Roster),
    Start(crate::lobby::Start),
    Refused(Vec<Mismatch>),
}

impl Encode for Message {
    fn encode(&self, out: &mut Canonical) {
        match self {
            Message::Hello(h) => {
                out.u8(TAG_HELLO);
                h.encode(out);
            }
            Message::Tick(t) => {
                out.u8(TAG_TICK);
                t.encode(out);
            }
            Message::Join(j) => {
                out.u8(TAG_JOIN);
                j.encode(out);
            }
            Message::Ready(r) => {
                out.u8(TAG_READY);
                out.bool(*r);
            }
            Message::Roster(r) => {
                out.u8(TAG_ROSTER);
                r.encode(out);
            }
            Message::Start(s) => {
                out.u8(TAG_START);
                s.encode(out);
            }
            Message::Refused(reasons) => {
                out.u8(TAG_REFUSED);
                out.seq(reasons, |c, m| m.encode(c));
            }
            Message::Halt(reason) => {
                out.u8(TAG_HALT);
                match reason {
                    HaltReason::Desync { tick } => {
                        out.u8(0);
                        out.u32(tick.0);
                    }
                    HaltReason::Left => out.u8(1),
                    HaltReason::Timeout => out.u8(2),
                    HaltReason::Protocol { detail } => {
                        out.u8(3);
                        out.str(detail);
                    }
                }
            }
        }
    }
}

impl Decode for Message {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        match input.u8()? {
            TAG_HELLO => Ok(Message::Hello(Hello::decode(input)?)),
            TAG_TICK => Ok(Message::Tick(TickPacket::decode(input)?)),
            TAG_JOIN => Ok(Message::Join(crate::lobby::Join::decode(input)?)),
            TAG_READY => Ok(Message::Ready(input.bool()?)),
            TAG_ROSTER => Ok(Message::Roster(crate::lobby::Roster::decode(input)?)),
            TAG_START => Ok(Message::Start(crate::lobby::Start::decode(input)?)),
            TAG_REFUSED => Ok(Message::Refused(input.seq(Mismatch::decode)?)),
            TAG_HALT => {
                let at = input.position();
                let reason = match input.u8()? {
                    0 => HaltReason::Desync { tick: Tick(input.u32()?) },
                    1 => HaltReason::Left,
                    2 => HaltReason::Timeout,
                    3 => HaltReason::Protocol { detail: input.str()?.to_string() },
                    tag => {
                        return Err(CodecError::BadTag { tag, expected: "halt reason", at });
                    }
                };
                Ok(Message::Halt(reason))
            }
            tag => Err(CodecError::BadTag { tag, expected: "message kind", at }),
        }
    }
}

const TAG_HELLO: u8 = 1;
const TAG_TICK: u8 = 2;
const TAG_HALT: u8 = 3;
const TAG_JOIN: u8 = 4;
const TAG_READY: u8 = 5;
const TAG_ROSTER: u8 = 6;
const TAG_START: u8 = 7;
const TAG_REFUSED: u8 = 8;

impl Encode for Mismatch {
    fn encode(&self, out: &mut Canonical) {
        match self {
            Mismatch::Protocol { ours, theirs } => {
                out.u8(0);
                out.u16(*ours);
                out.u16(*theirs);
            }
            Mismatch::Engine { ours, theirs } => {
                out.u8(1);
                out.str(ours);
                out.str(theirs);
            }
            Mismatch::Ruleset { ours, theirs } => {
                out.u8(2);
                out.u64(*ours);
                out.u64(*theirs);
            }
            Mismatch::Quirks { ours, theirs } => {
                out.u8(5);
                out.u64(*ours);
                out.u64(*theirs);
            }
            Mismatch::Seed { ours, theirs } => {
                out.u8(3);
                out.u64(*ours);
                out.u64(*theirs);
            }
            Mismatch::SameSlot(slot) => {
                out.u8(4);
                out.u8(slot.index());
            }
        }
    }
}

impl Decode for Mismatch {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        match input.u8()? {
            0 => Ok(Mismatch::Protocol { ours: input.u16()?, theirs: input.u16()? }),
            1 => Ok(Mismatch::Engine {
                ours: input.str()?.to_string(),
                theirs: input.str()?.to_string(),
            }),
            2 => Ok(Mismatch::Ruleset { ours: input.u64()?, theirs: input.u64()? }),
            3 => Ok(Mismatch::Seed { ours: input.u64()?, theirs: input.u64()? }),
            4 => {
                let slot = PlayerSlot::from_wire(input.u8()?)
                    .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })?;
                Ok(Mismatch::SameSlot(slot))
            }
            5 => Ok(Mismatch::Quirks { ours: input.u64()?, theirs: input.u64()? }),
            tag => Err(CodecError::BadTag { tag, expected: "mismatch kind", at }),
        }
    }
}
