//! The wire messages.
//!
//! Three of them, and the smallness of that list is the design. §4
//! needs a tick packet; D-12 needs a handshake that compares the
//! ruleset; a session that stops needs to say why. There is no lobby,
//! no chat, no ping, no matchmaking (§8), and no message whose meaning
//! depends on a previous one.
//!
//! Everything here is encoded with [`Canonical`], the same hand-written
//! fixed-width little-endian encoder the state checksum uses (D-10). A
//! message round-trips byte-identically, which `tests/packet.rs`
//! checks, and which matters because a packet that re-encodes
//! differently is a packet that would have hashed differently in a
//! replay.

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{Command, PlayerSlot, Tick};

/// Bumped whenever the meaning of any byte in this module changes.
///
/// Compared before anything else in [`Hello::check`], because a version
/// mismatch makes every later field's interpretation a guess.
pub const PROTOCOL_VERSION: u16 = 1;

/// The port this game uses.
///
/// Not chosen yet in any binding sense — nothing in this crate opens a
/// socket — but recorded here so that when a transport does, it is one
/// decision in one place. Deliberately **not** DirectPlay's 2300-2400
/// or 47624: colliding with a service a player may still have enabled
/// (`docs/netcode.md` §7) produces the worst kind of bug report.
///
/// 27962 sits in the range conventionally used by games. **Not
/// verified against the IANA registry** - nobody has looked it up, and
/// whoever first ships a transport should, because a collision here is
/// somebody else's service failing rather than ours.
pub const DEFAULT_PORT: u16 = 27962;

/// One tick's worth of one peer's intent.
///
/// **Sent every tick, including when there is nothing to say.** An
/// empty packet is what tells the other side it may advance; without it
/// "no input" and "player disconnected" are indistinguishable
/// (`docs/netcode.md` §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickPacket {
    /// The tick these commands execute on — *not* the tick they were
    /// issued on. The input delay is applied by the sender.
    pub tick: Tick,
    /// Which player sealed this packet.
    ///
    /// **A second deviation from `docs/netcode.md` §4**, whose
    /// `tick_packet` has no sender field. In a two-peer battle it does
    /// not need one: the connection the bytes arrived on identifies the
    /// sender. But §5's kingdom layer is a star — the host collects
    /// every player's list and broadcasts the result — so the
    /// connection identifies the *host*, not the author, and most
    /// packets are empty and therefore carry no commands to infer an
    /// author from. Without this byte, "player 3 sent nothing this
    /// turn" and "player 4 sent nothing this turn" are the same packet.
    ///
    /// One byte, and it makes [`PeerId`](crate::PeerId) and
    /// [`PlayerSlot`] genuinely independent rather
    /// than independent-until-it-matters.
    pub from: PlayerSlot,
    /// This peer's commands for that tick, in its own sequence order.
    /// The receiver still re-orders the merged set (see
    /// [`order_commands`](crate::order_commands)); a peer that sent
    /// them out of order cannot make the receiver's order differ.
    pub commands: Vec<Command>,
    /// Checksums for ticks this peer has simulated and not yet
    /// reported, oldest first.
    ///
    /// **A deviation from `docs/netcode.md` §4**, which lays the packet
    /// out as a bare `u32 ack_tick` and a bare `u64 state_hash` — one
    /// tick's checksum per packet, the newest one. Two things are wrong
    /// with that, and the second was found by a failing test rather
    /// than by reading:
    ///
    /// 1. There is no value of `u32` meaning "I have not simulated
    ///    anything yet". Tick 0 is a real tick and a hash of 0 is a
    ///    real hash, so the first packet of a session would have to
    ///    lie.
    /// 2. **A single newest-tick ack silently skips ticks.** §6 says to
    ///    compute a hash every tick and exchange it every tick, but a
    ///    peer does not always simulate exactly one tick between two
    ///    packets — at session start it runs `input_delay + 1` at once,
    ///    and it does the same after any stall. Every packet sealed in
    ///    such a burst carries the *same* newest ack, so the ticks in
    ///    between are never compared by anyone. Measured on a two-peer
    ///    battle session: the acks ran 2, 5, 8, 11, 14 and ticks 12 and
    ///    13 were never checked. A divergence landing on a skipped tick
    ///    is still caught at the next reported one, but it is
    ///    localised to a window instead of a tick, which is most of
    ///    what §6's per-tick cadence was buying.
    ///
    /// A list fixes both. In steady state it holds exactly one entry
    /// and the packet is the same size as §4's, plus a count; during a
    /// burst it holds the whole run, so no tick goes unexamined.
    pub acks: Vec<Ack>,
}

/// The most checksums one packet will carry.
///
/// A bound is needed because the list is built from "everything not yet
/// reported", and an unbounded list is an unbounded packet. Sixteen is
/// far above the real maximum: a peer can only simulate ticks it has
/// every other peer's commands for, so it cannot outrun them by more
/// than the input delay, and a burst is `input_delay + 1` ticks. If the
/// bound is ever hit, the *oldest* entries are the ones kept — a
/// divergence is localised by its first differing tick, so the early
/// ones carry the information.
pub const MAX_ACKS: usize = 16;

/// A peer's claim about its own state at a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ack {
    pub tick: Tick,
    /// The checksum of the canonical state *after* `tick` was
    /// simulated.
    pub state_hash: u64,
}

impl TickPacket {
    /// An empty packet — the "I have nothing to say, carry on" packet
    /// that most ticks are.
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

/// The session handshake, and D-12 in one struct.
///
/// D-12 says two peers must agree on the protocol version, the engine
/// version, **and the mod set and load order**, because `l2-mods`
/// exists precisely to let a mod change the simulation and a mod
/// mismatch is a guaranteed desync with a baffling symptom. `l2-mods`
/// already tracks which mod and which line set every value, so the
/// material for `ruleset_hash` exists; computing it is that crate's
/// job, and comparing it is this one's.
///
/// The seed is here for the same reason. It is not a rule, but it is
/// something the two peers must agree on before tick 0, and the cost of
/// checking it in the handshake is one `u64` against an hour of
/// wondering why two identical builds diverged immediately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub protocol: u16,
    /// A build identity — a version string, or a git description. Free
    /// text on purpose: this crate cannot know how the engine wants to
    /// version itself, and an opaque string that must match exactly is
    /// a stronger check than a structured one that invites
    /// "compatible enough" reasoning.
    pub engine: String,
    /// A hash over the *resolved* ruleset: every value, and the mod
    /// that set it, in load order.
    pub ruleset_hash: u64,
    /// The session seed, from which every [`Pcg32`](crate::Pcg32) in
    /// the simulation is derived.
    pub seed: u64,
    /// The slot the sender is claiming.
    pub slot: PlayerSlot,
}

/// One reason two peers cannot play together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    Protocol { ours: u16, theirs: u16 },
    Engine { ours: String, theirs: String },
    Ruleset { ours: u64, theirs: u64 },
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
            Mismatch::Seed { ours, theirs } => {
                write!(f, "session seed {theirs:016x} does not match ours ({ours:016x})")
            }
            Mismatch::SameSlot(slot) => write!(f, "both peers claim {slot}"),
        }
    }
}

impl Hello {
    /// Every reason this peer and that one cannot play, not just the
    /// first.
    ///
    /// All of them, because the alternative is the player fixing one
    /// thing, reconnecting, and being told about the next — and a mod
    /// list is exactly the kind of thing that is wrong in three ways at
    /// once.
    pub fn check(&self, theirs: &Hello) -> Vec<Mismatch> {
        let mut out = Vec::new();
        if self.protocol != theirs.protocol {
            out.push(Mismatch::Protocol { ours: self.protocol, theirs: theirs.protocol });
            // Nothing after this can be compared meaningfully: a
            // different protocol version may have written these fields
            // to mean something else.
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
        out.u64(self.seed);
        out.u8(self.slot.index());
    }
}

impl Decode for Hello {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let protocol = input.u16()?;
        let engine = input.str()?.to_string();
        let ruleset_hash = input.u64()?;
        let seed = input.u64()?;
        let at = input.position();
        let slot = PlayerSlot::from_wire(input.u8()?)
            .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })?;
        Ok(Hello { protocol, engine, ruleset_hash, seed, slot })
    }
}

/// Why a session stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HaltReason {
    /// The state checksums differed. Carries the tick, so both peers'
    /// dumps name the same one.
    Desync { tick: Tick },
    /// The player quit.
    Left,
    /// A peer stopped answering. Note that *this crate* never decides
    /// this — it has no clock (D-5) — so the value only ever arrives
    /// from a caller that does.
    Timeout,
    /// A malformed or impossible message.
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

/// Everything that can cross the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Hello(Hello),
    Tick(TickPacket),
    Halt(HaltReason),
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
