
use crate::canonical::{Canonical, Digest, SectionDigest};
use crate::command::{PlayerSlot, Tick};
use crate::replay::Replay;

pub const HISTORY: usize = 256;

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

    pub fn earliest(&self) -> Option<Tick> {
        self.entries.front().map(|(t, _)| *t)
    }

    pub fn hashes(&self) -> Vec<(Tick, u64)> {
        self.entries.iter().map(|(t, d)| (*t, d.hash)).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub tick: Tick,
    pub peer: PlayerSlot,
    pub ours: u64,
    pub theirs: u64,
    pub last_agreed: Option<Tick>,
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

#[derive(Debug, Clone)]
pub struct DesyncDump {
    pub divergence: Divergence,
    pub replay: Replay,
    pub hashes: Vec<(Tick, u64)>,
    pub state: Vec<u8>,
    pub author: PlayerSlot,
}

impl DesyncDump {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpComparison {
    pub first_difference: Option<Tick>,
    pub sole_subsystem: Option<&'static str>,
    pub subsystems: Vec<&'static str>,
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
