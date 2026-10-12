#![allow(unused_imports)]
use super::*;

use std::collections::BTreeMap;
use crate::canonical::{Canonical, Digest};
use crate::command::{commands_are_distinct, order_commands, Command, PlayerSlot, Tick};
use crate::desync::{DesyncDump, Divergence, History, HISTORY};
use crate::packet::{Ack, HaltReason, TickPacket, MAX_ACKS};
use crate::replay::Replay;

mod seats;

#[derive(Debug)]
pub struct Session {
    config: Config,
    local: PlayerSlot,
    slots: Vec<PlayerSlot>,
    seed: u64,

    tick: Tick,
    next_seal: u32,
    simulated: Option<Tick>,
    acked_through: Option<u32>,

    staged: Vec<Vec<u8>>,
    next_seq: u32,

    inbox: BTreeMap<(u32, u8), Vec<Command>>,

    claims: BTreeMap<(u32, u8), u64>,

    history: History,
    replay: Option<Replay>,
    halt: Option<HaltReason>,
    divergence: Option<Divergence>,
    divergences: u32,
    last_agreed: Option<Tick>,
    authority: PlayerSlot,
    /// C256: the tick no seat steps past until the host's state is in.
    barrier: Option<Tick>,
    /// C256: claims at or before this tick describe a world a resync replaced.
    floor: Option<Tick>,
    /// C260: a slot that sends nothing from this tick on.
    retired: BTreeMap<u8, u32>,
}

impl Session {
    pub fn new(
        config: Config,
        local: PlayerSlot,
        slots: &[PlayerSlot],
        seed: u64,
        sim: &impl Simulation,
    ) -> Session {
        let mut slots = slots.to_vec();
        slots.sort();
        slots.dedup();
        assert!(slots.contains(&local), "the local player must be one of the session's slots");
        assert!(!slots.is_empty(), "a session needs at least one player");

        let replay = if config.record { Some(Replay::start(seed, &slots, sim)) } else { None };
        let history = History::new(config.history);
        let authority = slots[0];
        Session {
            config,
            local,
            slots,
            seed,
            tick: Tick::ZERO,
            next_seal: 0,
            simulated: None,
            acked_through: None,
            staged: Vec::new(),
            next_seq: 0,
            inbox: BTreeMap::new(),
            claims: BTreeMap::new(),
            history,
            replay,
            halt: None,
            divergence: None,
            divergences: 0,
            last_agreed: None,
            authority,
            barrier: None,
            floor: None,
            retired: BTreeMap::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn local_slot(&self) -> PlayerSlot {
        self.local
    }

    pub fn slots(&self) -> &[PlayerSlot] {
        &self.slots
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn replay(&self) -> Option<&Replay> {
        self.replay.as_ref()
    }

    pub fn halt_reason(&self) -> Option<&HaltReason> {
        self.halt.as_ref()
    }

    pub fn is_halted(&self) -> bool {
        self.halt.is_some()
    }

    pub fn divergence(&self) -> Option<&Divergence> {
        self.divergence.as_ref()
    }

    /// Divergences noticed so far; each is resynced, none halts (C256).
    pub fn divergences(&self) -> u32 {
        self.divergences
    }

    pub fn authority(&self) -> PlayerSlot {
        self.authority
    }

    /// The seat whose state wins a resync: the host (C256). Defaults to the lowest slot.
    pub fn set_authority(&mut self, slot: PlayerSlot) {
        assert!(self.slots.contains(&slot), "the authority must be one of the session's slots");
        self.authority = slot;
    }

    pub fn resync_barrier(&self) -> Option<Tick> {
        self.barrier
    }

    /// The host's half of C256: the tick its state will be sent for, the last it has
    /// sealed, so no seat can already have run past it. `None` off the host, with no
    /// divergence, or with a resync already under way.
    pub fn plan_resync(&mut self) -> Option<Tick> {
        if self.local != self.authority || self.divergence.is_none() || self.barrier.is_some() {
            return None;
        }
        let at = Tick(self.next_seal.saturating_sub(1)).max(self.simulated.unwrap_or(Tick::ZERO));
        self.resync_announced(at);
        Some(at)
    }

    pub fn resync_announced(&mut self, at: Tick) {
        self.barrier = Some(at);
        self.floor = Some(self.floor.map_or(at, |f| f.max(at)));
    }

    pub fn resync_sent(&mut self) {
        if let Some(at) = self.barrier.take() {
            self.settle(at);
        }
    }

    /// C256: false when the joiner's load did not reproduce the host's digest.
    pub fn resynced(&mut self, at: Tick, hash: u64, sim: &impl Simulation) -> bool {
        let digest = state_digest(sim);
        let agrees = digest.hash == hash;
        self.history.replace(at, digest);
        if self.barrier == Some(at) {
            self.barrier = None;
        }
        self.settle(at);
        agrees
    }

    fn settle(&mut self, at: Tick) {
        self.divergence = None;
        self.claims.retain(|&(t, _), _| t > at.0);
        self.last_agreed = Some(at);
    }

    pub fn halt(&mut self, reason: HaltReason) {
        if self.halt.is_none() {
            self.halt = Some(reason);
        }
    }

    pub fn issue(&mut self, payload: Vec<u8>) {
        self.staged.push(payload);
    }

    pub fn execution_tick(&self) -> Tick {
        self.tick.plus(self.config.input_delay)
    }

    pub fn staged(&self) -> usize {
        self.staged.len()
    }

    pub fn next_packet(&mut self) -> Option<TickPacket> {
        if self.halt.is_some() {
            return None;
        }
        let horizon = self.tick.0 + self.config.input_delay;
        if self.next_seal > horizon {
            return None;
        }
        let tick = Tick(self.next_seal);
        self.next_seal += 1;

        let commands: Vec<Command> = if tick.0 == horizon {
            self.staged
                .drain(..)
                .map(|payload| {
                    let seq = self.next_seq;
                    self.next_seq += 1;
                    Command::new(self.local, seq, payload)
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut acks = Vec::new();
        if let Some(simulated) = self.simulated {
            let first = self.acked_through.map_or(0, |t| t + 1);
            for candidate in first..=simulated.0 {
                if candidate % self.config.exchange_every != 0 {
                    continue;
                }
                let Some(state_hash) = self.history.hash_at(Tick(candidate)) else {
                    continue;
                };
                acks.push(Ack { tick: Tick(candidate), state_hash });
                if acks.len() == MAX_ACKS {
                    break;
                }
            }
            if let Some(last) = acks.last() {
                self.acked_through = Some(last.tick.0);
            }
        }

        self.inbox.insert((tick.0, self.local.index()), commands.clone());
        Some(TickPacket { tick, from: self.local, commands, acks })
    }

    pub fn receive(&mut self, packet: TickPacket) -> Result<(), SessionError> {
        if self.halt.is_some() {
            return Ok(());
        }
        let from = packet.from;
        if from == self.local {
            return Err(SessionError::OwnSlot(from));
        }
        if !self.slots.contains(&from) {
            return Err(SessionError::UnknownSlot(from));
        }
        if self.is_retired(from, packet.tick) {
            return Ok(());
        }
        for command in &packet.commands {
            if command.slot != from {
                return Err(SessionError::ForgedCommand {
                    packet_from: from,
                    command_from: command.slot,
                });
            }
        }
        if let Some(simulated) = self.simulated {
            if packet.tick <= simulated {
                return Err(SessionError::AlreadySimulated { tick: packet.tick });
            }
        }
        let horizon = Tick(self.tick.0 + self.config.input_delay + self.config.lead_slack);
        if packet.tick > horizon {
            return Err(SessionError::TooFarAhead { tick: packet.tick, horizon });
        }

        let key = (packet.tick.0, from.index());
        if self.inbox.contains_key(&key) {
            return Err(SessionError::Duplicate { slot: from, tick: packet.tick });
        }
        self.inbox.insert(key, packet.commands);

        for ack in packet.acks {
            self.claims.insert((ack.tick.0, from.index()), ack.state_hash);
            self.check_claims(ack.tick);
            if self.halt.is_some() {
                break;
            }
        }
        Ok(())
    }

    pub fn advance(&mut self, sim: &mut impl Simulation) -> Advance {
        if let Some(reason) = &self.halt {
            return Advance::Halted(reason.clone());
        }

        let tick = self.tick;
        if let Some(at) = self.barrier.filter(|&at| tick > at) {
            return Advance::Resyncing { at };
        }
        let missing: Vec<PlayerSlot> = self
            .slots
            .iter()
            .copied()
            .filter(|slot| !self.is_retired(*slot, tick))
            .filter(|slot| !self.inbox.contains_key(&(tick.0, slot.index())))
            .collect();
        if !missing.is_empty() {
            return Advance::Waiting { tick, missing };
        }

        let mut commands = Vec::new();
        for slot in &self.slots {
            if let Some(from_slot) = self.inbox.remove(&(tick.0, slot.index())) {
                commands.extend(from_slot);
            }
        }
        order_commands(&mut commands);
        if !commands_are_distinct(&commands) {
            self.halt(HaltReason::Protocol {
                detail: format!("two commands share a (slot, sequence) at {tick}"),
            });
            return Advance::Halted(self.halt.clone().expect("just set"));
        }

        sim.step(tick, &commands);
        let digest = state_digest(sim);
        let hash = digest.hash;
        self.history.push(tick, digest);
        if let Some(replay) = &mut self.replay {
            replay.record(tick, &commands, hash);
        }
        self.simulated = Some(tick);
        self.tick = tick.next();

        self.check_claims(tick);
        if let Some(reason) = &self.halt {
            return Advance::Halted(reason.clone());
        }
        Advance::Stepped { tick, hash }
    }

    fn check_claims(&mut self, tick: Tick) {
        let Some(ours) = self.history.hash_at(tick) else {
            return;
        };
        if self.floor.is_some_and(|f| tick <= f) {
            return;
        }
        let mut compared = 0;
        let mut agreed = true;
        for slot in self.slots.clone() {
            if slot == self.local {
                continue;
            }
            let Some(theirs) = self.claims.remove(&(tick.0, slot.index())) else {
                continue;
            };
            compared += 1;
            if theirs != ours {
                agreed = false;
                if self.divergence.is_none() {
                    self.divergences += 1;
                    self.divergence = Some(Divergence {
                        tick,
                        peer: slot,
                        ours,
                        theirs,
                        last_agreed: self.last_agreed,
                        sections: self
                            .history
                            .get(tick)
                            .map(|d| d.sections.clone())
                            .unwrap_or_default(),
                    });
                }
            }
        }
        if compared > 0 && agreed && self.last_agreed.is_none_or(|last| tick > last) {
            self.last_agreed = Some(tick);
        }
    }

    pub fn dump(&self, sim: &impl Simulation) -> Option<DesyncDump> {
        let divergence = self.divergence.clone()?;
        let replay = self.replay.clone()?;
        let snapshot = state_snapshot(sim);
        Some(DesyncDump {
            divergence,
            replay,
            hashes: self.history.hashes(),
            state: snapshot.bytes.unwrap_or_default(),
            author: self.local,
        })
    }
}

