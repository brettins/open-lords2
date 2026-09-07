//! Replays: the check that pays for the whole design.
//!
//! Because `step()` is a pure function of state and commands (D-11), a
//! session is *completely* described by `(initial state, seed, ordered
//! command stream)`. That triple is small — a battle's worth of
//! commands is a few kilobytes — and replaying it must reproduce every
//! state checksum the original session recorded.
//!
//! `docs/netcode.md` §6 calls this the part that pays for itself, and
//! the reason is in `docs/decisions.md` D7: two implementations
//! agreeing proves only that the same author ported the same
//! misunderstanding twice, while a property of the *data* is a real
//! check. A recorded session that must replay to a bit-identical
//! checksum is a property of the data. It also runs on one machine,
//! with no network and no second player, which is the only reason it
//! will be run often enough to help.
//!
//! # What a replay catches that a unit test does not
//!
//! Every violation of the determinism contract that nobody thought to
//! grep for. A `HashMap` iteration that happens to be stable on the
//! machine that wrote the test; a `sort_unstable_by` on a key with
//! ties; a `SystemTime` that crept into a damage calculation. None of
//! those fail a test that runs the simulation once. All of them fail a
//! test that runs it twice and compares.
//!
//! # What it does not catch
//!
//! Anything that is deterministic *per machine* but differs between
//! machines: `usize` width, a `DefaultHasher` whose algorithm changed
//! with the Rust version, an `f32` that the compiler contracted into an
//! FMA on one target and not another. A replay corpus in CI on one
//! runner will pass all of those happily. That is why D-1 through D-6
//! are structural rules rather than things the test suite is expected
//! to find, and it is worth being explicit that this file is not a
//! substitute for them.

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{Command, PlayerSlot, Tick};
use crate::lockstep::Simulation;

/// Identifies a replay file at a glance, and stops a truncated or
/// unrelated file from being decoded as a plausible one.
pub const REPLAY_MAGIC: [u8; 4] = *b"L2RP";

/// Bumped when this layout changes. Separate from
/// [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION): a replay file
/// outlives a session, and the two version numbers move for different
/// reasons.
pub const REPLAY_VERSION: u16 = 1;

/// A complete recording of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    /// The seed every generator in the simulation derives from.
    pub seed: u64,
    /// The canonical encoding of the state before tick 0.
    ///
    /// The bytes, not a description of how to build them. A replay that
    /// said "battle 7 on map 3" would depend on the rule files being
    /// what they were on the day, and rule files are exactly what mods
    /// change (D-12).
    pub initial: Vec<u8>,
    /// The players, in slot order.
    pub slots: Vec<PlayerSlot>,
    /// Every command applied, tagged with the tick it executed on, in
    /// the order it was applied.
    pub commands: Vec<(Tick, Command)>,
    /// The checksum after each simulated tick.
    ///
    /// Every tick, not just the last. A replay that only pinned the
    /// final hash would say "these two runs ended differently" when
    /// what is wanted is "these two runs first differed at tick 412",
    /// and the difference between those two sentences is an afternoon.
    pub hashes: Vec<(Tick, u64)>,
}

impl Replay {
    /// Start a recording from a simulation's current state.
    pub fn start(seed: u64, slots: &[PlayerSlot], sim: &impl Simulation) -> Replay {
        let mut encoder = Canonical::recording();
        sim.encode_state(&mut encoder);
        let initial = encoder.finish().bytes.expect("recording encoder keeps its bytes");
        Replay {
            seed,
            initial,
            slots: slots.to_vec(),
            commands: Vec::new(),
            hashes: Vec::new(),
        }
    }

    /// Note that `tick` was simulated with `commands` and produced
    /// `hash`.
    pub fn record(&mut self, tick: Tick, commands: &[Command], hash: u64) {
        for command in commands {
            self.commands.push((tick, command.clone()));
        }
        self.hashes.push((tick, hash));
    }

    /// The last tick recorded, if any.
    pub fn last_tick(&self) -> Option<Tick> {
        self.hashes.last().map(|(tick, _)| *tick)
    }

    /// The recorded checksum for a tick.
    pub fn hash_at(&self, tick: Tick) -> Option<u64> {
        self.hashes.iter().find(|(t, _)| *t == tick).map(|(_, h)| *h)
    }

    /// The commands recorded for one tick, in applied order.
    pub fn commands_at(&self, tick: Tick) -> Vec<Command> {
        self.commands
            .iter()
            .filter(|(t, _)| *t == tick)
            .map(|(_, c)| c.clone())
            .collect()
    }

    /// Re-run the recording against a simulation and check every
    /// checksum.
    ///
    /// `sim` must already hold the state the recording started from;
    /// this crate cannot build a simulation, only drive one. The first
    /// thing checked is that it does, because a mismatch there means
    /// the *setup* diverged and every later difference would be noise.
    ///
    /// Stops at the first difference. Continuing would produce a list
    /// of ticks that all differ for the same reason, and the only one
    /// that carries information is the first.
    pub fn verify(&self, sim: &mut impl Simulation) -> Result<Tick, ReplayMismatch> {
        let mut encoder = Canonical::recording();
        sim.encode_state(&mut encoder);
        let start = encoder.finish();
        if start.bytes.as_deref() != Some(self.initial.as_slice()) {
            return Err(ReplayMismatch::InitialState {
                recorded_len: self.initial.len(),
                actual_len: start.len as usize,
            });
        }

        // A cursor rather than `commands_at` per tick: the command
        // stream is in tick order, so one pass is enough, and a
        // thirty-minute battle has enough ticks that the quadratic
        // version is noticeable in CI.
        let mut cursor = 0usize;
        let mut last = Tick::ZERO;
        for &(tick, expected) in &self.hashes {
            while cursor < self.commands.len() && self.commands[cursor].0 < tick {
                cursor += 1;
            }
            let start = cursor;
            while cursor < self.commands.len() && self.commands[cursor].0 == tick {
                cursor += 1;
            }
            let mut commands: Vec<Command> =
                self.commands[start..cursor].iter().map(|(_, c)| c.clone()).collect();
            crate::command::order_commands(&mut commands);
            sim.step(tick, &commands);
            let actual = crate::lockstep::state_hash(sim);
            if actual != expected {
                return Err(ReplayMismatch::Diverged { tick, recorded: expected, actual });
            }
            last = tick;
        }
        Ok(last)
    }
}

/// A replay that did not reproduce.
///
/// Always a bug, and always a determinism bug: the same inputs applied
/// to the same state produced a different result. Either the
/// simulation changed (in which case old replays are expected to fail
/// and the corpus needs re-recording, deliberately and visibly), or
/// something in it is not deterministic (in which case this is the only
/// warning that will be given).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayMismatch {
    InitialState { recorded_len: usize, actual_len: usize },
    Diverged { tick: Tick, recorded: u64, actual: u64 },
}

impl core::fmt::Display for ReplayMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReplayMismatch::InitialState { recorded_len, actual_len } => write!(
                f,
                "the simulation's starting state ({actual_len} bytes) is not the one recorded \
                 ({recorded_len} bytes)"
            ),
            ReplayMismatch::Diverged { tick, recorded, actual } => write!(
                f,
                "replay diverged at {tick}: recorded {recorded:016x}, got {actual:016x}"
            ),
        }
    }
}

impl std::error::Error for ReplayMismatch {}

impl Encode for Replay {
    fn encode(&self, out: &mut Canonical) {
        out.raw(&REPLAY_MAGIC);
        out.u16(REPLAY_VERSION);
        out.u64(self.seed);
        out.seq(&self.slots, |c, slot| c.u8(slot.index()));
        out.bytes(&self.initial);
        out.seq(&self.commands, |c, (tick, command)| {
            c.u32(tick.0);
            command.encode(c);
        });
        out.seq(&self.hashes, |c, (tick, hash)| {
            c.u32(tick.0);
            c.u64(*hash);
        });
    }
}

impl Decode for Replay {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        let magic = input.raw(4)?;
        if magic != REPLAY_MAGIC {
            return Err(CodecError::BadTag { tag: magic[0], expected: "replay magic", at });
        }
        let at = input.position();
        let version = input.u16()?;
        if version != REPLAY_VERSION {
            return Err(CodecError::BadTag {
                tag: version as u8,
                expected: "replay version 1",
                at,
            });
        }
        let seed = input.u64()?;
        let slots = input.seq(|r| {
            let at = r.position();
            PlayerSlot::from_wire(r.u8()?)
                .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })
        })?;
        let initial = input.bytes()?.to_vec();
        let commands = input.seq(|r| {
            let tick = Tick(r.u32()?);
            Ok((tick, Command::decode(r)?))
        })?;
        let hashes = input.seq(|r| Ok((Tick(r.u32()?), r.u64()?)))?;
        Ok(Replay { seed, initial, slots, commands, hashes })
    }
}
