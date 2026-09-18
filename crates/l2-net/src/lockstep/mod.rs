
mod session;
pub use session::*;

use std::collections::BTreeMap;

use crate::canonical::{Canonical, Digest};
use crate::command::{commands_are_distinct, order_commands, Command, PlayerSlot, Tick};
use crate::desync::{DesyncDump, Divergence, History, HISTORY};
use crate::packet::{Ack, HaltReason, TickPacket, MAX_ACKS};
use crate::replay::Replay;

pub trait Simulation {
    fn step(&mut self, tick: Tick, commands: &[Command]);

    fn encode_state(&self, out: &mut Canonical);
}

pub fn state_digest(sim: &impl Simulation) -> Digest {
    let mut encoder = Canonical::hashing();
    sim.encode_state(&mut encoder);
    encoder.finish()
}

pub fn state_hash(sim: &impl Simulation) -> u64 {
    state_digest(sim).hash
}

pub fn state_snapshot(sim: &impl Simulation) -> Digest {
    let mut encoder = Canonical::recording();
    sim.encode_state(&mut encoder);
    encoder.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub input_delay: u32,

    pub exchange_every: u32,

    pub history: usize,

    pub record: bool,

    pub lead_slack: u32,
}

impl Config {
    pub fn battle() -> Config {
        Config {
            input_delay: 2,
            exchange_every: 1,
            history: HISTORY,
            record: true,
            lead_slack: 64,
        }
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advance {
    Stepped { tick: Tick, hash: u64 },
    Waiting { tick: Tick, missing: Vec<PlayerSlot> },
    Halted(HaltReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    UnknownSlot(PlayerSlot),
    OwnSlot(PlayerSlot),
    Duplicate { slot: PlayerSlot, tick: Tick },
    ForgedCommand { packet_from: PlayerSlot, command_from: PlayerSlot },
    TooFarAhead { tick: Tick, horizon: Tick },
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

