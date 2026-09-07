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
//! apply them identically everywhere" is precisely what this session
//! does — with an input delay of zero, because a turn's commands
//! execute on that same turn rather than two ticks later. The host's
//! relaying is a *transport topology*, not a second protocol: it is why
//! [`PeerId`](crate::PeerId) and [`PlayerSlot`] are
//! different types, and it is invisible from here.
//!
//! One mechanism buys what §1 says it buys: one determinism contract,
//! one desync detector, one replay format, one test harness.
//!
//! # No clock
//!
//! There is no `Instant`, no `SystemTime` and no timeout anywhere in
//! this file. D-5 forbids the simulation from seeing wall-clock time,
//! and the cleanest way to obey a rule like that is to make the value
//! unavailable rather than to be careful with it.
//!
//! The consequence is deliberate and worth understanding: **this crate
//! will wait forever.** [`Advance::Waiting`] names the slots it is
//! waiting for, and the caller — which has a render loop and therefore
//! a clock — decides when that becomes "Waiting for player…" on screen
//! and when it becomes a timeout. §4 wants both of those behaviours;
//! neither belongs here.

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
///   there is no `dt`, no frame time and no wall clock, because
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
    /// soldiers rather than fighting-game frames, input delay is the
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
    /// the input delay being zero rather than needing a second code
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
/// for it forever. They are returned rather than acted on because
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
    /// forge a command could simply set the whole packet's `from`. It
    /// is an *integrity* check: a packet whose contents contradict its
    /// header is far more likely to be a relaying bug in our own host
    /// code than an attack, and it should be caught where it happens
    /// rather than three ticks later as a desync.
    ForgedCommand { packet_from: PlayerSlot, command_from: PlayerSlot },
    /// A packet too far ahead of our own simulation
    /// ([`Config::lead_slack`]).
    TooFarAhead { tick: Tick, horizon: Tick },
    /// A packet for a tick already simulated. Harmless — a duplicate
    /// delivery — and reported rather than silently dropped so that a
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

/// One peer's lockstep session.
///
/// The loop it is built for:
///
/// ```no_run
/// # use l2_net::*;
/// # fn go<S: Simulation, T: Transport>(
/// #     session: &mut Session, sim: &mut S, net: &mut T, peer: PeerId,
/// # ) {
/// // 1. Local intent, gathered however the UI likes.
/// session.issue(b"move 7 to 34,12".to_vec());
///
/// // 2. Seal and send. One packet per tick, empty ones included.
/// while let Some(packet) = session.next_packet() {
///     let bytes = Canonical::bytes_of(&Message::Tick(packet));
///     net.send(peer, &frame(&bytes).unwrap()).unwrap();
/// }
///
/// // 3. Drain the network at one fixed point (D-5).
/// while let Some((_from, bytes)) = net.poll() {
///     if let Ok(Message::Tick(packet)) = decode_all::<Message>(&bytes) {
///         let _ = session.receive(packet);
///     }
/// }
///
/// // 4. Advance ONE tick. See the note below before looping here.
/// match session.advance(sim) {
///     Advance::Stepped { .. } => {}
///     Advance::Waiting { .. } => { /* show "waiting for player..." */ }
///     Advance::Halted(_) => { /* the session is over */ }
/// }
/// # }
/// ```
///
/// # Pacing is the caller's job, and looping on `advance` is a decision
///
/// [`Session::advance`] steps at most one tick per call, and it stops
/// when it runs out of commands rather than when it runs out of time -
/// it has no clock (D-5). A caller that loops until [`Advance::Waiting`]
/// therefore runs the simulation **as fast as the machine allows**, up
/// to the input delay ahead of its peers. That is exactly right for a
/// replay, for catching up after a stall, and for the tests in this
/// crate. It is wrong for live play, where the tick rate is a real
/// 10 Hz and that loop belongs inside "has 100 ms elapsed?".
///
/// The distinction is not academic: run unpaced, a battle designed
/// around a 200 ms input delay resolves in whatever time the CPU takes.
#[derive(Debug)]
pub struct Session {
    config: Config,
    local: PlayerSlot,
    slots: Vec<PlayerSlot>,
    seed: u64,

    /// The next tick to simulate.
    tick: Tick,
    /// The next tick we owe a packet for.
    next_seal: u32,
    /// The last tick actually simulated.
    simulated: Option<Tick>,
    /// The newest tick whose checksum we have already put in a packet.
    acked_through: Option<u32>,

    /// Local commands issued but not yet sealed into a packet.
    staged: Vec<Vec<u8>>,
    /// Session-wide command counter for this player.
    next_seq: u32,

    /// Sealed packets, ours included, keyed by `(tick, slot)`.
    ///
    /// A `BTreeMap`, not a `HashMap`. D-4 forbids hash-ordered
    /// iteration in simulation code, and while this map is *not*
    /// simulation state, the commands that come out of it are fed
    /// straight into `step()` — so the rule may as well hold here too,
    /// and then nobody has to work out where the boundary is.
    inbox: BTreeMap<(u32, u8), Vec<Command>>,

    /// Peers' state-hash claims we have not been able to check yet,
    /// because we have not simulated that tick ourselves.
    claims: BTreeMap<(u32, u8), u64>,

    history: History,
    replay: Option<Replay>,
    halt: Option<HaltReason>,
    divergence: Option<Divergence>,
    /// The most recent tick every peer that has spoken agreed on.
    last_agreed: Option<Tick>,
}

impl Session {
    /// Start a session.
    ///
    /// `slots` must contain `local` and is sorted here — the ordering
    /// of players is a property of the session, not of the order the
    /// caller happened to list them (D-7).
    ///
    /// Takes the simulation because a recording session must capture
    /// the state *before* tick 0, and there is exactly one moment at
    /// which that state exists.
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
            last_agreed: None,
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

    /// The next tick to be simulated.
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

    /// Stop the session for a reason the caller has decided on — the
    /// player quitting, or a timeout, which this crate cannot detect
    /// because it has no clock.
    ///
    /// The first reason sticks. A session that halted on a desync and
    /// then had `Left` written over it as the UI tore down would lose
    /// the only interesting fact about it.
    pub fn halt(&mut self, reason: HaltReason) {
        if self.halt.is_none() {
            self.halt = Some(reason);
        }
    }

    /// Queue a local command. It will execute on the current tick plus
    /// [`Config::input_delay`].
    ///
    /// The payload is the simulation's own canonical encoding of the
    /// order (D-10); this crate never looks inside it.
    pub fn issue(&mut self, payload: Vec<u8>) {
        self.staged.push(payload);
    }

    /// The tick a command issued now would execute on.
    pub fn execution_tick(&self) -> Tick {
        self.tick.plus(self.config.input_delay)
    }

    /// Commands staged but not yet sealed.
    pub fn staged(&self) -> usize {
        self.staged.len()
    }

    /// Seal the next outgoing packet, or `None` when this peer is up to
    /// date.
    ///
    /// Call it in a loop. At the start of a session with an input delay
    /// of two it produces the packets for ticks 0 and 1 — both
    /// necessarily empty, since nothing can execute before the delay
    /// has elapsed — and thereafter exactly one packet per tick, which
    /// is what §4 requires: **every peer sends one packet per tick,
    /// including when it has no commands.**
    ///
    /// The sealed packet is also placed in our own inbox. The local
    /// player is not a special case anywhere below this line; our
    /// commands go through the same ordering as everyone's, which is
    /// the only way to be sure the merge is symmetric.
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

        // Staged commands attach only to the *latest* legal tick.
        // Anything earlier is a priming packet, and a command placed in
        // one would execute sooner than the input delay promises.
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

        // Every tick simulated since the last packet, not just the
        // newest. See `TickPacket::acks` for why: the newest-only form
        // skips whole ticks whenever a peer simulates more than one
        // between packets, which it does at session start and after
        // every stall.
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

    /// Accept a packet from a peer.
    ///
    /// Checking the checksum claim it carries happens here, so a
    /// divergence is noticed as soon as the evidence arrives rather
    /// than at the next step.
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
                // A divergence found in this packet. Stop reading the
                // rest of it: the first differing tick is the one
                // worth reporting, and later ones are consequences.
                break;
            }
        }
        Ok(())
    }

    /// Simulate one tick, if every peer's commands for it have arrived.
    pub fn advance(&mut self, sim: &mut impl Simulation) -> Advance {
        if let Some(reason) = &self.halt {
            return Advance::Halted(reason.clone());
        }

        let tick = self.tick;
        let missing: Vec<PlayerSlot> = self
            .slots
            .iter()
            .copied()
            .filter(|slot| !self.inbox.contains_key(&(tick.0, slot.index())))
            .collect();
        if !missing.is_empty() {
            return Advance::Waiting { tick, missing };
        }

        // Gather in slot order, then sort. The gather order is already
        // the sort order — `slots` is sorted and the inbox is keyed by
        // slot — so the sort is a no-op today. It stays because the
        // *rule* is that commands are ordered by (slot, seq) before
        // `step` sees them (§4), and a rule that is only satisfied by
        // accident is one refactor from being violated.
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

        // A claim for this tick may have arrived several ticks ago.
        self.check_claims(tick);
        if let Some(reason) = &self.halt {
            return Advance::Halted(reason.clone());
        }
        Advance::Stepped { tick, hash }
    }

    /// Compare every stored claim about `tick` with our own hash.
    fn check_claims(&mut self, tick: Tick) {
        let Some(ours) = self.history.hash_at(tick) else {
            // We have not simulated it yet, or it has fallen out of the
            // ring. Either way the claim waits.
            return;
        };
        // `compared` matters: a tick nobody has yet made a claim about
        // is *unconfirmed*, not agreed. Treating it as agreed would set
        // `last_agreed` to the diverged tick itself — the claim for a
        // tick usually arrives a tick or two after we simulate it — and
        // a divergence report saying "last agreed at the tick where we
        // disagreed" is worse than no report at all.
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
                    // §6: halt. Do not resynchronise, do not let one
                    // peer catch up from the other's state.
                    self.halt(HaltReason::Desync { tick });
                }
            }
        }
        if compared > 0 && agreed && self.last_agreed.is_none_or(|last| tick > last) {
            self.last_agreed = Some(tick);
        }
    }

    /// Everything this peer knows about the divergence, ready to be
    /// written to a file by something that is allowed to do I/O.
    ///
    /// `None` unless the session halted on a desync *and* was
    /// recording — without the command stream a dump cannot be
    /// replayed, which is most of what a dump is for.
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
