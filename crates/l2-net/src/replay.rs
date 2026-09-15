
use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{Command, PlayerSlot, Tick};
use crate::lockstep::Simulation;

pub const REPLAY_MAGIC: [u8; 4] = *b"L2RP";

pub const REPLAY_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub seed: u64,
    pub initial: Vec<u8>,
    pub slots: Vec<PlayerSlot>,
    pub commands: Vec<(Tick, Command)>,
    pub hashes: Vec<(Tick, u64)>,
}

impl Replay {
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

    pub fn record(&mut self, tick: Tick, commands: &[Command], hash: u64) {
        for command in commands {
            self.commands.push((tick, command.clone()));
        }
        self.hashes.push((tick, hash));
    }

    pub fn last_tick(&self) -> Option<Tick> {
        self.hashes.last().map(|(tick, _)| *tick)
    }

    pub fn hash_at(&self, tick: Tick) -> Option<u64> {
        self.hashes.iter().find(|(t, _)| *t == tick).map(|(_, h)| *h)
    }

    pub fn commands_at(&self, tick: Tick) -> Vec<Command> {
        self.commands
            .iter()
            .filter(|(t, _)| *t == tick)
            .map(|(_, c)| c.clone())
            .collect()
    }

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
