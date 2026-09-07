//! Ticks, player slots, and the commands that cross the wire.
//!
//! # Why there is no `enum Command` here
//!
//! A command's *payload* is an opaque `Vec<u8>` as far as this crate is
//! concerned, and that is a deliberate limit on how much this crate
//! knows.
//!
//! The set of commands is the game's, not the network's: "move unit 7
//! to (34, 12)", "buy 40 grain", "end turn". It belongs to `l2-sim` and
//! to the kingdom layer, and both of those will grow commands for years
//! after the lockstep core stops changing. If the command set lived
//! here, every new order in the battle simulation would be a change to
//! the networking crate, and `l2-net` would end up depending on
//! `l2-sim` — which is backwards, because `l2-sim` needs [`Pcg32`] and
//! [`Fixed`] from here.
//!
//! What this crate does own is the part that must be identical on both
//! peers regardless of what the payload means: the **envelope**
//! ([`Command`]), the **ordering rule** ([`order_commands`]), and the
//! encoding primitives ([`Canonical`]) that D-10 requires the payload
//! be written with. The simulation supplies bytes; this crate promises
//! that both peers see the same bytes in the same order.
//!
//! [`Pcg32`]: crate::Pcg32
//! [`Fixed`]: crate::Fixed
//! [`Canonical`]: crate::Canonical

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};

/// The simulation clock, and the only clock the simulation has (D-5).
///
/// A `u32` at 10 Hz wraps after thirteen years of continuous play. The
/// kingdom layer counts turns in the same type and will not reach four
/// digits.
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

    /// Saturating, so that "the tick two before the start" is the start
    /// rather than a panic or a wrap to four billion.
    pub fn minus(self, n: u32) -> Tick {
        Tick(self.0.saturating_sub(n))
    }
}

impl core::fmt::Display for Tick {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "tick {}", self.0)
    }
}

/// The most players the original supports, and therefore the most we
/// do.
///
/// Verified: `g_playerStartCount` is 5, 4 or 2 across the shipped maps,
/// and the original opens its DirectPlay session with `dwMaxPlayers =
/// 5` (`docs/netcode.md` §1 and Appendix A).
pub const MAX_PLAYERS: usize = 5;

/// Which player a command came from.
///
/// A slot, not a connection: slots are the *game's* identity for a
/// player and they survive a reconnect, a host migration, and the
/// difference between "player 2" on my screen and on yours. The
/// ordering rule in [`order_commands`] is defined on slots for exactly
/// that reason — a rule defined on connections would order commands
/// differently on each peer, which is a desync on the first tick where
/// two players act at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerSlot(u8);

impl PlayerSlot {
    /// Panics above [`MAX_PLAYERS`]. A slot out of range is a
    /// programming error at every call site in this crate, and a packet
    /// carrying one is rejected by [`PlayerSlot::from_wire`] instead.
    pub fn new(index: u8) -> PlayerSlot {
        assert!(
            (index as usize) < MAX_PLAYERS,
            "player slot {index} is beyond the {MAX_PLAYERS}-player limit"
        );
        PlayerSlot(index)
    }

    /// The fallible constructor for bytes that came from a peer.
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

/// One command, in the form both peers see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub slot: PlayerSlot,
    /// Per-player, monotonic across the whole session.
    ///
    /// Not per tick. A session-wide counter makes a gap visible — if
    /// this peer has seen sequence 40 and then 42, a command was lost,
    /// which a per-tick counter starting from zero each tick could not
    /// tell us. It is also the tiebreak that makes [`order_commands`] a
    /// total order (D-7).
    pub seq: u32,
    /// The simulation's own canonical encoding of the order (D-10).
    /// Opaque here.
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

/// Put a tick's commands into the order every peer will use.
///
/// By slot, then by sequence number. **Never arrival order** — arrival
/// order is the network's opinion and it differs per peer
/// (`docs/netcode.md` §4).
///
/// `sort_by` and not `sort_unstable_by`: D-7 warns that an unstable
/// sort on a key with ties is a desync waiting for a different
/// allocation pattern. The key here *is* total — two commands cannot
/// share a slot and a sequence number — so `sort_unstable_by` would in
/// fact be correct today. It is not used anyway, because the cost is
/// nothing at this scale and because the next person to add a field to
/// the key is not guaranteed to keep it total. This is the one place in
/// the crate where the paranoid choice is free.
pub fn order_commands(commands: &mut [Command]) {
    commands.sort_by(|a, b| a.slot.cmp(&b.slot).then(a.seq.cmp(&b.seq)));
}

/// True when no two commands share a `(slot, seq)`.
///
/// **Assumes the slice has been through [`order_commands`]** — it
/// compares neighbours rather than every pair, which is linear instead
/// of quadratic and is exactly as strong once the list is sorted.
///
/// Called by the session immediately after ordering and before
/// stepping. A duplicate means a packet was applied twice, which would
/// otherwise show up as a desync several ticks later with no trace of
/// the cause.
pub fn commands_are_distinct(commands: &[Command]) -> bool {
    commands
        .windows(2)
        .all(|pair| pair[0].slot != pair[1].slot || pair[0].seq != pair[1].seq)
}
