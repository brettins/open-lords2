//! Divergence: noticing it, localising it, and writing down enough to
//! reproduce it offline.
//!
//! `docs/netcode.md` §6 puts it plainly: in a lockstep design this is
//! the part that decides whether the whole approach was a good idea,
//! because one divergence anywhere breaks the game and the symptom
//! appears arbitrarily far from the cause. A detector added later is
//! added after the first unexplained multi-hour debugging session.
//!
//! # The policy, and it is not negotiable
//!
//! **On divergence, halt.** Do not resynchronise. Do not let one peer
//! catch up from the other's state. Once two simulations differ,
//! everything either of them says about the game is suspect, and
//! continuing converts a reproducible bug into an unreproducible one —
//! which is the difference between a bug that gets fixed and a bug that
//! gets a forum thread.
//!
//! [`Session::advance`](crate::Session::advance) implements exactly
//! that and offers no way to override it.

use crate::canonical::{Canonical, Digest, SectionDigest};
use crate::command::{PlayerSlot, Tick};
use crate::replay::Replay;

/// How many ticks of checksums each peer keeps.
///
/// §6's number. At 10 Hz that is the last 25 seconds, which is far more
/// than the few ticks of skew a session can accumulate — the ring is
/// not a queue waiting to be consumed, it is the window a dump can
/// localise a divergence *within*, and 256 entries of a `u64` plus a
/// handful of section digests is a few tens of kilobytes.
pub const HISTORY: usize = 256;

/// A peer's own recent state checksums.
#[derive(Debug, Clone)]
pub struct History {
    capacity: usize,
    entries: std::collections::VecDeque<(Tick, Digest)>,
}

impl History {
    pub fn new(capacity: usize) -> History {
        assert!(capacity > 0, "a history of zero ticks cannot detect anything");
        History { capacity, entries: std::collections::VecDeque::with_capacity(capacity) }
    }

    pub fn push(&mut self, tick: Tick, digest: Digest) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back((tick, digest));
    }

    pub fn get(&self, tick: Tick) -> Option<&Digest> {
        self.entries.iter().find(|(t, _)| *t == tick).map(|(_, d)| d)
    }

    pub fn hash_at(&self, tick: Tick) -> Option<u64> {
        self.get(tick).map(|d| d.hash)
    }

    pub fn last(&self) -> Option<(Tick, &Digest)> {
        self.entries.back().map(|(t, d)| (*t, d))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The oldest tick still remembered. A claim about a tick older
    /// than this cannot be checked — which is a real limitation and one
    /// [`Session`](crate::Session) reports rather than hides.
    pub fn earliest(&self) -> Option<Tick> {
        self.entries.front().map(|(t, _)| *t)
    }

    /// `(tick, hash)` pairs, oldest first — the form a dump wants.
    pub fn hashes(&self) -> Vec<(Tick, u64)> {
        self.entries.iter().map(|(t, d)| (*t, d.hash)).collect()
    }
}

/// Two peers disagreed about the state at a tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub tick: Tick,
    /// The peer whose claim differs from ours. With more than two
    /// players this names the first disagreement found, in slot order —
    /// which is *not* an accusation. Lockstep cannot tell which peer is
    /// wrong, only that two of them differ, and a majority vote among
    /// three peers would be a guess dressed up as evidence.
    pub peer: PlayerSlot,
    pub ours: u64,
    pub theirs: u64,
    /// The last tick every peer that has spoken agreed on. The
    /// divergence is somewhere in `(last_agreed, tick]`, and with a
    /// per-tick exchange that interval is a single tick.
    pub last_agreed: Option<Tick>,
    /// Our per-subsystem digests at the diverged tick.
    ///
    /// §6's localisation: the first tick where exactly one subsystem
    /// hash differs names the subsystem. We only ever have our own —
    /// the wire carries one `u64` per tick, not a vector — so
    /// localisation happens when the two dumps are compared, which is
    /// the point at which somebody is looking anyway.
    pub sections: Vec<SectionDigest>,
}

impl core::fmt::Display for Divergence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "desync at {}: we have {:016x}, {} has {:016x}",
            self.tick, self.ours, self.peer, self.theirs
        )?;
        if let Some(agreed) = self.last_agreed {
            write!(f, " (last agreed at {agreed})")?;
        }
        Ok(())
    }
}

/// Everything one peer knows about a divergence, in a form that can be
/// written to a file and sent to whoever is debugging it.
///
/// Two of these plus nothing else are enough to reproduce the
/// divergence offline, on one machine, with no network.
///
/// # Deviation from `docs/netcode.md` §6
///
/// The design asks a dump to contain "the canonical state at the last
/// agreed tick **and** at the diverged tick". This carries the state at
/// the diverged tick and not at the last agreed one, because keeping a
/// rolling full-state snapshot for every one of the last 256 ticks is
/// precisely the per-tick whole-world cost that §2 chose lockstep to
/// avoid — for a battle it would be the largest allocation in the
/// engine, and it would be paid on every tick of every game to serve
/// the sessions that desync.
///
/// It is also unnecessary. The dump carries the initial state, the
/// seed and the whole command stream, so the state at the last agreed
/// tick is a replay away — under a second of CPU, on the machine of
/// the person who is already sitting down to debug it. What cannot be
/// recovered by replaying is the *diverged* state, since replaying
/// reproduces the correct one; so that is the one worth carrying.
#[derive(Debug, Clone)]
pub struct DesyncDump {
    pub divergence: Divergence,
    /// Initial state, seed and the full command stream.
    pub replay: Replay,
    /// Our last 256 checksums.
    pub hashes: Vec<(Tick, u64)>,
    /// Our canonical state at the diverged tick.
    pub state: Vec<u8>,
    /// Which peer wrote this dump.
    pub author: PlayerSlot,
}

impl DesyncDump {
    /// Compare two dumps of the same divergence.
    ///
    /// Returns the subsystems whose digests differ, and the first tick
    /// at which the two checksum histories part company — which is the
    /// sentence anyone reading a desync report actually wants: "the
    /// unit array diverged at tick 4,112".
    pub fn compare(&self, other: &DesyncDump) -> DumpComparison {
        let first_difference = self
            .hashes
            .iter()
            .find(|(tick, hash)| {
                other
                    .hashes
                    .iter()
                    .any(|(other_tick, other_hash)| other_tick == tick && other_hash != hash)
            })
            .map(|(tick, _)| *tick);

        let ours = Digest {
            hash: self.divergence.ours,
            len: 0,
            sections: self.divergence.sections.clone(),
            bytes: None,
        };
        let theirs = Digest {
            hash: other.divergence.ours,
            len: 0,
            sections: other.divergence.sections.clone(),
            bytes: None,
        };

        DumpComparison {
            first_difference,
            sole_subsystem: ours.sole_difference(&theirs),
            subsystems: ours.differences(&theirs),
            same_command_stream: self.replay.commands == other.replay.commands,
            same_initial_state: self.replay.initial == other.replay.initial,
            same_seed: self.replay.seed == other.replay.seed,
        }
    }
}

/// What two dumps say when put side by side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpComparison {
    /// The earliest tick both peers recorded and disagreed about.
    pub first_difference: Option<Tick>,
    /// Set when exactly one subsystem differs — §6's "the first tick
    /// where exactly one subsystem hash differs names the subsystem".
    pub sole_subsystem: Option<&'static str>,
    /// Every subsystem that differs.
    pub subsystems: Vec<&'static str>,
    /// False means the peers applied different commands, which makes
    /// this a *networking* bug and not a simulation one — and that is a
    /// completely different investigation.
    pub same_command_stream: bool,
    pub same_initial_state: bool,
    pub same_seed: bool,
}

impl core::fmt::Display for DumpComparison {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.same_seed {
            return writeln!(f, "the two peers used different seeds; nothing else matters");
        }
        if !self.same_initial_state {
            return writeln!(f, "the two peers started from different states");
        }
        if !self.same_command_stream {
            writeln!(f, "the two peers applied different commands: a networking fault")?;
        }
        match self.first_difference {
            Some(tick) => writeln!(f, "first disagreement at {tick}")?,
            None => writeln!(f, "no disagreement in the retained history")?,
        }
        match self.sole_subsystem {
            Some(name) => writeln!(f, "exactly one subsystem differs: {name}")?,
            None if self.subsystems.is_empty() => writeln!(f, "no subsystem digests differ")?,
            None => writeln!(f, "subsystems differing: {}", self.subsystems.join(", "))?,
        }
        Ok(())
    }
}

/// Encode a dump for writing to a file.
///
/// This crate never touches the filesystem — see the "What is not
/// here" section of `lib.rs` — so a dump becomes bytes here and becomes
/// a file somewhere that is allowed to do I/O.
///
/// **There is deliberately no `Decode`.** A [`SectionDigest`]'s name is
/// a `&'static str` because it is a compile-time constant written by
/// the simulation, and decoding one would mean either owning the string
/// (an allocation per section per tick, on the per-tick checksum path)
/// or leaking it. That asymmetry is affordable because of how §6 says
/// dumps are used: *both* peers write their own dump locally and the
/// two files are compared afterwards by a person. Nothing sends a dump
/// over the wire, and [`DesyncDump::compare`] — which is what a
/// comparison actually needs — works on the values, not on the bytes.
/// A tool that wants to read dump files can decode this format without
/// this crate's type.
impl crate::canonical::Encode for DesyncDump {
    fn encode(&self, out: &mut Canonical) {
        out.raw(b"L2DD");
        out.u16(1);
        out.u8(self.author.index());
        out.u32(self.divergence.tick.0);
        out.u8(self.divergence.peer.index());
        out.u64(self.divergence.ours);
        out.u64(self.divergence.theirs);
        out.option(self.divergence.last_agreed.as_ref(), |c, tick| c.u32(tick.0));
        out.seq(&self.divergence.sections, |c, section| {
            c.str(section.name);
            c.u64(section.hash);
            c.u64(section.len);
        });
        out.seq(&self.hashes, |c, (tick, hash)| {
            c.u32(tick.0);
            c.u64(*hash);
        });
        out.bytes(&self.state);
        self.replay.encode(out);
    }
}
