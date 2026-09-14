//! The lockstep session: tick queue, command collection, and the point
//! at which a divergence stops the game.
//!
//! # One mechanism, two layers
//!
//! `docs/netcode.md` §1 describes two games stacked on each other — a
//! turn-based kingdom layer with up to five players and a real-time
//! battle layer with two — and says they get the *same* mechanism with
//! very different parameters. This is that mechanism, and the two
//! layers really are the same code: [`Config::battle`] and
//! [`Config::kingdom`] differ only in numbers.
//!
//! That is worth stating because §5 reads as if the kingdom layer were
//! a different protocol: each player plans a turn, sends a command list
//! to the host, the host concatenates in slot order and broadcasts. But
//! "collect every player's commands for step N, order them by slot,
//! apply them identically everywhere" is what this session
//! does — with an input delay of zero, because a turn's commands
//! execute on that same turn. The host's
//! relaying is a *transport topology*, not a second protocol: it is why
//! [`PeerId`](crate::PeerId) and [`PlayerSlot`] are
//! different types, and it is invisible from here.
//!
//! One mechanism buys what §1 says it buys: one determinism contract,
//! one desync detector, one replay format, one test harness.
//!
//! # No clock
//!
//! no `SystemTime` and no timeout anywhere in
//! this file. D-5 forbids the simulation from seeing wall-clock time,
//! and the cleanest way to obey a rule like that is to make the value
//! unavailable.
//!
//! The consequence is deliberate and worth understanding: **this crate
//! will wait forever.** [`Advance::Waiting`] names the slots it is
//! waiting for, and the caller — which has a render loop and therefore
//! a clock — decides when that becomes "Waiting for player…" on screen
//! and when it becomes a timeout. §4 wants both of those behaviours;
//! neither belongs here.

mod session;
pub use session::*;

use std::collections::BTreeMap;

use crate::canonical::{Canonical, Digest};
use crate::command::{commands_are_distinct, order_commands, Command, PlayerSlot, Tick};
use crate::desync::{DesyncDump, Divergence, History, HISTORY};
use crate::packet::{Ack, HaltReason, TickPacket, MAX_ACKS};
use crate::replay::Replay;

/// What the session needs from a simulation. Nothing more.
///
/// Two methods, and the shape of them is the determinism contract in
/// miniature:
///
/// * `step` is D-11's pure function of state and commands. Everything
///   the tick may read is reachable from `self` and `commands`. No file
///   I/O, no globals, no clock — and note what is *not* passed in:
/// no frame time and no wall clock, because
///   [`Tick`] is the only time the simulation has (D-5).
/// * `encode_state` writes the canonical byte stream §6 requires,
///   never the in-memory image.
///
/// `l2-net` deliberately knows nothing else about the simulation. It
/// cannot construct one, cannot inspect one, and cannot interpret a
/// command's payload. That is what keeps the dependency arrow pointing
/// the right way: `l2-sim` depends on this crate for [`Pcg32`] and
/// [`Fixed`], and this crate never depends on `l2-sim`.
///
/// [`Pcg32`]: crate::Pcg32
/// [`Fixed`]: crate::Fixed
pub trait Simulation {
    /// Apply one tick's commands, already ordered by
    /// [`order_commands`].
    fn step(&mut self, tick: Tick, commands: &[Command]);

    /// Write the canonical state, ideally in named
    /// [`sections`](Canonical::section) so a divergence can be
    /// localised.
    fn encode_state(&self, out: &mut Canonical);
}

/// The checksum and section digests of a simulation's current state.
pub fn state_digest(sim: &impl Simulation) -> Digest {
    let mut encoder = Canonical::hashing();
    sim.encode_state(&mut encoder);
    encoder.finish()
}

/// The checksum of a simulation's current state.
pub fn state_hash(sim: &impl Simulation) -> u64 {
    state_digest(sim).hash
}

/// The canonical state, kept.
pub fn state_snapshot(sim: &impl Simulation) -> Digest {
    let mut encoder = Canonical::recording();
    sim.encode_state(&mut encoder);
    encoder.finish()
}

/// Session parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// A command issued during tick `N` executes on tick `N +
    /// input_delay`.
    ///
    /// Two for the battle layer: at 10 Hz that is the ~200 ms §4 spends
    /// to guarantee that every peer has every peer's commands before it
    /// needs them, so no peer ever has to un-run anything. Zero for the
    /// kingdom layer, where a turn's commands execute on that turn and
    /// the round trip is invisible against a turn that takes a human
    /// minutes.
    ///
    /// The alternative to a delay is rollback, and §4 is explicit that
    /// **we do not build rollback**. It buys responsiveness at the cost
    /// of making every piece of simulation state rewindable, which is a
    /// tax on every future feature; for units that are formations of
/// soldiers, input delay is the
    /// right trade.
    pub input_delay: u32,

    /// Include a state checksum in every `n`th packet.
    ///
    /// One — every tick — for both layers. §6 sizes it: the checksum is
    /// 8 bytes in a packet we are sending anyway. The larger value is
    /// §6's stated fallback if hashing ever costs too much: keep
    /// hashing every tick, exchange less often, and let the history
    /// ring localise the divergence to a window.
    pub exchange_every: u32,

    /// How many ticks of checksums to keep. §6 says 256.
    pub history: usize,

    /// Record the session as a [`Replay`].
    ///
    /// On by default. §6's argument is that a lockstep session is
    /// *already* `(initial state, seed, command stream)`, so recording
    /// costs a few kilobytes and buys the CI check that catches
    /// nondeterminism on one machine with no second player. Turning it
    /// off is for the memory-constrained case that has not appeared.
    pub record: bool,

    /// How far ahead of our own simulation a peer's packets may be
    /// before we refuse them.
    ///
    /// Without a bound, a peer that runs fast — or one that is
    /// deliberately misbehaving — grows our inbox without limit while
    /// we wait for someone else. The bound is generous: a peer can
    /// legitimately be `input_delay` ahead plus whatever jitter the
    /// network adds, and this allows sixty-four ticks of that.
    pub lead_slack: u32,
}

impl Config {
    /// The battle layer: 10 Hz, two peers, ~200 ms of input delay.
    pub fn battle() -> Config {
        Config {
            input_delay: 2,
            exchange_every: 1,
            history: HISTORY,
            record: true,
            lead_slack: 64,
        }
    }

    /// The kingdom layer: one step per turn, no delay.
    ///
    /// With `input_delay = 0`, [`Session::next_packet`] *is* the "end
    /// turn" button: staged commands attach to the packet for the
    /// current turn, and nothing is sent while the player is still
    /// planning. That matches §5 exactly - "nothing is sent while
    /// planning, and nothing local is applied" - and it falls out of
/// the input delay being zero
    /// path.
    pub fn kingdom() -> Config {
        Config {
            input_delay: 0,
            exchange_every: 1,
            history: HISTORY,
            record: true,
            lead_slack: 8,
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config::battle()
    }
}

/// What happened when the session was asked to advance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advance {
    /// One tick was simulated.
    Stepped { tick: Tick, hash: u64 },
    /// Not everyone's commands for `tick` have arrived.
    ///
    /// §4: the simulation blocks. It does not extrapolate and it does
    /// not proceed with a guess, because proceeding is a desync and a
    /// desync is worse than a pause. The caller shows "Waiting for
    /// player…" after a grace period of its own choosing.
    Waiting { tick: Tick, missing: Vec<PlayerSlot> },
    /// The session is over and will not step again.
    Halted(HaltReason),
}

/// A packet this session refuses.
///
/// Every one of these is fatal to the session in practice: a rejected
/// packet is a tick that will never arrive, and the session will wait
/// for it forever. They are returned because
/// *how* to fail is a policy decision — a dialog, a log line, a
/// [`HaltReason::Protocol`] sent to the peer — and this crate does not
/// make policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// A slot that is not in this session.
    UnknownSlot(PlayerSlot),
    /// A packet claiming to be from us.
    OwnSlot(PlayerSlot),
    /// A second packet for a tick that already has one.
    Duplicate { slot: PlayerSlot, tick: Tick },
    /// A packet containing a command attributed to a different player.
    ///
    /// Not an anti-cheat measure — §8 is clear that lockstep offers no
    /// protection against a modified client, and a peer that wanted to
/// forge a command could set the whole packet's `from`. It
    /// is an *integrity* check: a packet whose contents contradict its
    /// header is far more likely to be a relaying bug in our own host
    /// code than an attack, and it should be caught where it happens
    ///
    ForgedCommand { packet_from: PlayerSlot, command_from: PlayerSlot },
    /// A packet too far ahead of our own simulation
    /// ([`Config::lead_slack`]).
    TooFarAhead { tick: Tick, horizon: Tick },
    /// A packet for a tick already simulated. Harmless — a duplicate
/// delivery — and reported so that a
    /// transport that is duplicating packets is visible.
    AlreadySimulated { tick: Tick },
}

impl core::fmt::Display for SessionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SessionError::UnknownSlot(slot) => write!(f, "{slot} is not in this session"),
            SessionError::OwnSlot(slot) => write!(f, "a packet claiming to be from us ({slot})"),
            SessionError::Duplicate { slot, tick } => {
                write!(f, "a second packet from {slot} for {tick}")
            }
            SessionError::ForgedCommand { packet_from, command_from } => write!(
                f,
                "a packet from {packet_from} carried a command from {command_from}"
            ),
            SessionError::TooFarAhead { tick, horizon } => {
                write!(f, "{tick} is beyond the horizon ({horizon})")
            }
            SessionError::AlreadySimulated { tick } => write!(f, "{tick} has already been run"),
        }
    }
}

impl std::error::Error for SessionError {}

