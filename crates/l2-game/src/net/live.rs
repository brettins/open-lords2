//! The lockstep session under a played game: the app loop's half, run between
//! frames (D-5). `docs/netcode.md` §4 (input delay, stalling), §6 (dump, resync,
//! C256), §8 *A seat that goes* (C260, which replaced C240's halt).

mod seats;

use super::replay::{self, Origin, Recording};
use super::sim::{encode_net, step_game};
use crate::command::GameCommand;
use crate::game::Game;
use l2_net::{
    decode_all, state_hash, Advance, Canonical, Command, Config, DesyncDump, HaltReason, Message,
    PeerId, PlayerSlot, Resync, Session, Simulation, TcpTransport, Tick, Transport,
};
pub use seats::name_of;
use std::path::PathBuf;

/// Pumps waiting before the stall is shown. `[I]` ours: §4's "short grace period".
pub const STALL_GRACE: u32 = 30;
/// Pumps a seat with an open connection stays silent before the waiting box names it:
/// five seconds. `[D]` ours, C262: past TCP's first retransmits and a window drag.
pub const QUIET: u32 = (5_000_000_000 / crate::clock::TICK_NS) as u32;
/// Pumps waiting before a silent peer is dropped: sixty seconds of ticks. `[I]` ours.
pub const TIMEOUT: u32 = (60_000_000_000 / crate::clock::TICK_NS) as u32;
/// Ticks one pump may run to catch a peer up. `[I]` ours.
pub const CATCH_UP: u32 = 4;
/// Pumps a resync notice stays up: three seconds. `[I]` ours, C256.
pub const NOTICE: u32 = (3_000_000_000 / crate::clock::TICK_NS) as u32;

/// The world as the session steps it: the game and the dropped-payload count.
pub struct Stepper<'a> {
    pub game: &'a mut Game,
    pub malformed: &'a mut u32,
}

impl Simulation for Stepper<'_> {
    fn step(&mut self, _tick: Tick, commands: &[Command]) {
        step_game(self.game, self.malformed, commands);
        seat_battle(self.game);
    }
    fn encode_state(&self, out: &mut Canonical) {
        encode_net(self.game, *self.malformed, out);
    }
}

pub struct Live {
    pub session: Session,
    transport: TcpTransport,
    peers: Vec<(PeerId, PlayerSlot)>,
    malformed: u32,
    stalled: u32,
    reported: bool,
    /// Where a desync dump goes; `None` is the data dir.
    pub dump_dir: Option<PathBuf>,
    pub dump: Option<(PathBuf, DesyncDump)>,
    dumped: u32,
    /// C256: the host's state for a barrier this joiner has not reached.
    pending: Option<Resync>,
    /// Resyncs this seat took part in, and the tick of the last (C256).
    pub resyncs: u32,
    pub resynced_at: Option<Tick>,
    origin: Option<(Origin, PlayerSlot, u64)>,
    /// Set by [`Live::record_to`]; the replay is written there on halt and drop.
    replay_dir: Option<PathBuf>,
    pub replay_path: Option<PathBuf>,
    /// C260: the join-code service, kept so a dropped player can come back by code.
    relay: Option<super::relay::Host>,
    /// C260: connections opened after `Start` that have not been seated.
    strangers: Vec<PeerId>,
    /// C260: a dropped player back on a new connection, until the host can seat it.
    rejoining: Vec<(PeerId, PlayerSlot)>,
    /// C260: dropped seats the game waits for.
    waiting: Vec<PlayerSlot>,
    /// C262: seats silent past [`QUIET`] whose connection is still open.
    quiet: Vec<PlayerSlot>,
}

impl Live {
    /// Every seat in `game.seats` plays; this peer is `game.net.me`.
    pub fn new(transport: TcpTransport, game: &mut Game, seed: u64) -> Option<Live> {
        let me = game.net.me?;
        let slots: Vec<PlayerSlot> = game.seats.iter().map(|&(s, _)| s).collect();
        game.net.lockstep = true;
        let tables = Canonical::hash_of(&game.kingdom.tables);
        let origin = game.net.origin.as_deref().map(|o| (o.clone(), me, tables));
        let mut malformed = 0;
        let host = game.net.roster.players.iter().find(|p| p.is_host).map(|p| p.slot);
        let host = host.or(game.net.hosting.then_some(me));
        let mut session = {
            let sim = Stepper { game, malformed: &mut malformed };
            Session::new(Config::battle(), me, &slots, seed, &sim)
        };
        if let Some(h) = host.filter(|h| slots.contains(h)) {
            session.set_authority(h);
        }
        Some(Live::with(session, transport, malformed, origin))
    }

    fn with(session: Session, mut transport: TcpTransport, malformed: u32, origin: Option<(Origin, PlayerSlot, u64)>) -> Live {
        let _ = transport.take_joined();
        Live {
            session,
            transport,
            peers: Vec::new(),
            malformed,
            stalled: 0,
            reported: false,
            dump_dir: None,
            dump: None,
            dumped: 0,
            pending: None,
            resyncs: 0,
            resynced_at: None,
            origin,
            replay_dir: None,
            replay_path: None,
            relay: None,
            strangers: Vec::new(),
            rejoining: Vec::new(),
            waiting: Vec::new(),
            quiet: Vec::new(),
        }
    }

    /// Every network game the app plays is recorded (net-replay-hardening).
    pub fn record_to(&mut self, dir: PathBuf) {
        self.replay_dir = Some(dir);
    }

    /// `None` for a world not built from `Start`, which no replay can rebuild.
    pub fn record(&self) -> Option<Recording> {
        let (origin, me, tables) = self.origin.clone()?;
        let replay = self.session.replay()?.clone();
        Some(Recording { origin, me, tables, replay })
    }

    pub fn write_replay(&mut self) -> Option<std::io::Result<PathBuf>> {
        let dir = self.replay_dir.clone()?;
        let written = replay::write(&dir, &self.record()?);
        if let Ok(p) = &written {
            self.replay_path = Some(p.clone());
        }
        Some(written)
    }

    /// One pass: orders out, packets both ways, up to [`CATCH_UP`] ticks run.
    pub fn pump(&mut self, game: &mut Game) {
        self.control(game);
        if !self.session.is_halted() {
            self.stage(game);
            for _ in 0..CATCH_UP {
                self.exchange(game);
                if !self.advance(game) {
                    break;
                }
            }
            self.exchange(game);
            self.diverged(game);
            self.admit(game);
        }
        game.net.waiting = self.waiting.iter().map(|&s| name_of(game, s)).collect();
        game.net.quiet = self.quiet.iter().map(|&s| name_of(game, s)).collect();
        if let Some((_, left)) = &mut game.net.notice {
            *left = left.saturating_sub(1);
            if *left == 0 {
                game.net.notice = None;
            }
        }
        game.net.stalled = self.stalled >= STALL_GRACE && self.waiting.is_empty() && self.quiet.is_empty();
        game.net.beat.frame(self.session.tick().0);
        if let Some(reason) = self.session.halt_reason().cloned() {
            self.report(game, reason);
        }
    }

    fn stage(&mut self, game: &mut Game) {
        let mut orders = std::mem::take(&mut game.net.outbox);
        if let Some(b) = game.battle.as_deref_mut() {
            let ops = b.outbox.drain(..);
            orders.extend(ops.map(|op| GameCommand::from(crate::command::BattleOrder { op })));
        }
        for o in &orders {
            self.session.issue(Canonical::bytes_of(o));
        }
    }

    /// A new divergence is written out at once; the host then names the tick its
    /// state will be sent for, before it seals another packet (C256).
    fn diverged(&mut self, game: &mut Game) {
        if self.session.divergences() > self.dumped {
            self.dumped = self.session.divergences();
            self.evidence(game);
        }
        if let Some(at) = self.session.plan_resync() {
            self.send_all(&Message::ResyncAt(at));
        }
    }

    fn exchange(&mut self, game: &mut Game) {
        self.diverged(game);
        while let Some(packet) = self.session.next_packet() {
            self.send_all(&Message::Tick(packet));
        }
        loop {
            self.note_strangers();
            let Some((peer, bytes)) = self.transport.poll() else { break };
            let Ok(message) = decode_all::<Message>(&bytes) else { continue };
            if self.is_host() {
                self.from_joiner(game, peer, message, &bytes);
            } else {
                self.from_host(game, peer, message);
            }
        }
        let gone = self.transport.take_disconnected();
        if !gone.is_empty() {
            self.dropped(&gone);
        }
    }

    /// C260: the host forwards each joiner's packets to the others (a star, not a mesh).
    fn from_joiner(&mut self, game: &mut Game, peer: PeerId, message: Message, bytes: &[u8]) {
        match message {
            Message::Tick(packet) if self.is_seated(peer) => {
                if !self.peers.iter().any(|&(p, _)| p == peer) {
                    self.peers.push((peer, packet.from));
                }
                let retired = self.session.is_retired(packet.from, packet.tick);
                match self.session.receive(packet) {
                    Ok(()) if !retired => self.send_but(peer, bytes),
                    Ok(()) => {}
                    Err(e) => self.session.halt(HaltReason::Protocol { detail: e.to_string() }),
                }
            }
            Message::Halt(reason @ HaltReason::Protocol { .. }) => self.session.halt(reason),
            Message::Join(j) => self.knock(game, peer, &j),
            Message::Leaving => self.leaving(game, peer),
            _ => {}
        }
    }

    fn from_host(&mut self, game: &mut Game, peer: PeerId, message: Message) {
        match message {
            Message::Tick(packet) => {
                if self.peers.is_empty() {
                    self.peers.push((peer, self.session.authority()));
                }
                if let Err(e) = self.session.receive(packet) {
                    self.session.halt(HaltReason::Protocol { detail: e.to_string() });
                }
            }
            Message::Halt(reason) => self.session.halt(reason),
            Message::ResyncAt(at) => self.session.resync_announced(at),
            Message::Resync(r) => self.pending = Some(r),
            Message::Retire { slot, from } => {
                self.session.retire(slot, from);
                self.waiting.retain(|&s| s != slot);
                self.quiet.retain(|&s| s != slot);
                game.net.notice = Some((format!("The computer now plays {}'s realm.", name_of(game, slot)), NOTICE));
            }
            Message::Away(slot) if !self.waiting.contains(&slot) => self.waiting.push(slot),
            Message::Back(slot) => {
                self.waiting.retain(|&s| s != slot);
                game.net.notice = Some((format!("{} is back.", name_of(game, slot)), NOTICE));
            }
            _ => {}
        }
    }

    fn advance(&mut self, game: &mut Game) -> bool {
        let mut sim = Stepper { game: &mut *game, malformed: &mut self.malformed };
        match self.session.advance(&mut sim) {
            Advance::Stepped { .. } => {
                self.stalled = 0;
                self.quiet.clear();
                true
            }
            Advance::Waiting { missing, .. } => {
                self.silent(&missing);
                false
            }
            Advance::Resyncing { at } => self.resync(game, at),
            Advance::Halted(_) => false,
        }
    }

    /// `Sync_Rollback` (`0x0043F5C5`) / `Sync_ResendState` (`0x0043F939`) are the
    /// original's; it rolled back first, we do not (`docs/netcode.md` §6).
    fn resync(&mut self, game: &mut Game, at: Tick) -> bool {
        if self.is_host() {
            let state = super::resync::encode(game, self.malformed);
            let hash = state_hash(&Stepper { game: &mut *game, malformed: &mut self.malformed });
            self.send_all(&Message::Resync(Resync { tick: at, hash, state }));
            self.session.resync_sent();
            self.resynced(game, at, "Resynced the other players.");
            return true;
        }
        let Some(r) = self.pending.take_if(|r| r.tick == at) else {
            self.stalled += 1;
            if self.stalled >= TIMEOUT {
                self.session.halt(HaltReason::Timeout);
            }
            return false;
        };
        if let Err(e) = super::resync::adopt(game, &mut self.malformed, &r.state) {
            self.session.halt(HaltReason::Protocol { detail: format!("the host's state: {e}") });
            return false;
        }
        seat_battle(game);
        let sim = Stepper { game: &mut *game, malformed: &mut self.malformed };
        let _ = self.session.resynced(at, r.hash, &sim);
        self.stalled = 0;
        self.resynced(game, at, "Resynced with the host.");
        true
    }

    fn resynced(&mut self, game: &mut Game, at: Tick, text: &str) {
        self.resyncs += 1;
        self.resynced_at = Some(at);
        game.net.notice = Some((text.to_string(), NOTICE));
    }

    /// C256: the replay and this seat's half of the dump, before the resync replaces
    /// the world they describe.
    fn evidence(&mut self, game: &mut Game) {
        let _ = self.write_replay();
        let sim = Stepper { game: &mut *game, malformed: &mut self.malformed };
        let Some(dump) = self.session.dump(&sim) else { return };
        match self.dump_dir.clone().or_else(dump_home).map(|d| write_dump(&d, &dump)) {
            Some(Ok(path)) => self.dump = Some((path, dump)),
            Some(Err(e)) => eprintln!("desync dump not written: {e}"),
            None => eprintln!("desync dump not written: no data directory"),
        }
    }

    /// A halt: the host gone or ended (C260), or a protocol error. Tell the others, keep the replay.
    fn report(&mut self, game: &mut Game, reason: HaltReason) {
        if std::mem::replace(&mut self.reported, true) {
            return;
        }
        self.send_all(&Message::Halt(reason.clone()));
        let _ = self.write_replay();
        let host = name_of(game, self.session.authority());
        let text = match reason {
            HaltReason::Left | HaltReason::Timeout => {
                hand_to_ai(game);
                format!("The connection to {host}'s game was lost. To play on together, leave and join it again with the same code or address.")
            }
            HaltReason::Ended => {
                hand_to_ai(game);
                format!("{host} ended the game.")
            }
            HaltReason::Protocol { detail } => format!("The connection to the other players failed ({detail})."),
        };
        eprintln!("network game stopped at tick {}: {text}", self.session.tick().0);
        game.net.halted = Some(text);
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.write_replay();
    }
}

/// C240: a seat that lost its host gives every other seat's realm to the AI
/// (`ai::steps` skips `is_human`), so its world goes on single-player.
fn hand_to_ai(game: &mut Game) {
    let me = game.local_realm();
    let realms: Vec<u8> = game.seats.iter().map(|&(_, r)| r).filter(|&r| r != me).collect();
    for &r in &realms {
        game.kingdom.realms[r as usize].is_human = false;
        for (_, u) in game.kingdom.campaign.units.iter_mut().filter(|(_, u)| u.owner == r) {
            u.owner_is_human = false;
        }
    }
}

/// Clicks on this peer queue; the seat's own side is the one it draws.
fn seat_battle(game: &mut Game) {
    let me = game.player;
    let sides = crate::command::battle_sides(game);
    if let Some(b) = game.battle.as_deref_mut() {
        b.deferred = true;
        if sides.is_some_and(|(a, d)| a == me || d == me) {
            b.owner = me;
        }
    }
}

/// Beside the saves, never the install.
fn dump_home() -> Option<PathBuf> {
    Some(crate::saves::dir()?.parent()?.join("desync"))
}

fn write_dump(dir: &std::path::Path, dump: &DesyncDump) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let name = format!("desync-t{}-slot{}.l2dd", dump.divergence.tick.0, dump.author.index());
    let path = dir.join(name);
    std::fs::write(&path, Canonical::bytes_of(dump))?;
    Ok(path)
}
