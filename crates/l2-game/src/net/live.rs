//! The lockstep session under a played game: the app loop's half, run between
//! frames (D-5). `docs/netcode.md` §4 (input delay, stalling), §6 (halt, dump).

use super::sim::{encode_net, step_game};
use crate::command::GameCommand;
use crate::game::Game;
use l2_net::{
    decode_all, Advance, Canonical, Command, Config, DesyncDump, HaltReason, Message, PeerId,
    PlayerSlot, Session, Simulation, TcpTransport, Tick, Transport,
};
use std::path::PathBuf;

/// Pumps waiting before the stall is shown. `[I]` ours: §4's "short grace period".
pub const STALL_GRACE: u32 = 30;
/// Ticks one pump may run to catch a peer up. `[I]` ours.
pub const CATCH_UP: u32 = 4;

/// The world as the session steps it: the game and the dropped-payload count.
pub struct Stepper<'a> {
    pub game: &'a mut Game,
    pub malformed: &'a mut u32,
}

impl Simulation for Stepper<'_> {
    fn step(&mut self, _tick: Tick, commands: &[Command]) {
        step_game(self.game, self.malformed, commands);
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
}

impl Live {
    /// Every seat in `game.seats` plays; this peer is `game.net.me`.
    pub fn new(transport: TcpTransport, game: &mut Game, seed: u64) -> Option<Live> {
        let me = game.net.me?;
        let slots: Vec<PlayerSlot> = game.seats.iter().map(|&(s, _)| s).collect();
        game.net.lockstep = true;
        let mut malformed = 0;
        let session = {
            let sim = Stepper { game, malformed: &mut malformed };
            Session::new(Config::battle(), me, &slots, seed, &sim)
        };
        Some(Live {
            session,
            transport,
            peers: Vec::new(),
            malformed,
            stalled: 0,
            reported: false,
            dump_dir: None,
            dump: None,
        })
    }

    /// One pass: orders out, packets both ways, up to [`CATCH_UP`] ticks run.
    pub fn pump(&mut self, game: &mut Game) {
        if !self.session.is_halted() {
            self.stage(game);
            for _ in 0..CATCH_UP {
                self.exchange();
                if !self.advance(game) {
                    break;
                }
            }
            self.exchange();
        }
        game.net.stalled = self.stalled >= STALL_GRACE;
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

    fn exchange(&mut self) {
        while let Some(packet) = self.session.next_packet() {
            let _ = self.transport.broadcast(&Canonical::bytes_of(&Message::Tick(packet)));
        }
        while let Some((peer, bytes)) = self.transport.poll() {
            match decode_all::<Message>(&bytes) {
                Ok(Message::Tick(packet)) => {
                    if !self.peers.iter().any(|&(p, _)| p == peer) {
                        self.peers.push((peer, packet.from));
                    }
                    if let Err(e) = self.session.receive(packet) {
                        self.session.halt(HaltReason::Protocol { detail: e.to_string() });
                    }
                }
                Ok(Message::Halt(reason)) => {
                    let from = self.peers.iter().find(|&&(p, _)| p == peer).map(|&(_, s)| s);
                    let local = self.session.local_slot();
                    let from = from.or_else(|| self.session.slots().iter().copied().find(|&s| s != local));
                    self.session.halted_by_peer(from.unwrap_or(local), reason);
                }
                _ => {}
            }
        }
        if !self.transport.take_disconnected().is_empty() {
            self.session.halt(HaltReason::Left);
        }
    }

    fn advance(&mut self, game: &mut Game) -> bool {
        let mut sim = Stepper { game: &mut *game, malformed: &mut self.malformed };
        match self.session.advance(&mut sim) {
            Advance::Stepped { .. } => {
                self.stalled = 0;
                seat_battle(game);
                true
            }
            Advance::Waiting { .. } => {
                self.stalled += 1;
                false
            }
            Advance::Halted(_) => false,
        }
    }

    /// §6 *When it diverges*: halt the others, then write our half of the dump.
    fn report(&mut self, game: &mut Game, reason: HaltReason) {
        if std::mem::replace(&mut self.reported, true) {
            return;
        }
        let _ = self.transport.broadcast(&Canonical::bytes_of(&Message::Halt(reason.clone())));
        let sim = Stepper { game: &mut *game, malformed: &mut self.malformed };
        let dump = self.session.dump(&sim);
        let mut text = reason.to_string();
        if let Some(dump) = dump {
            let dir = self.dump_dir.clone().or_else(dump_home);
            match dir.map(|d| write_dump(&d, &dump)) {
                Some(Ok(path)) => {
                    text = format!("{text}; dump written to {}", path.display());
                    self.dump = Some((path, dump));
                }
                Some(Err(e)) => text = format!("{text}; the dump could not be written: {e}"),
                None => text = format!("{text}; no data directory for the dump"),
            }
        }
        game.net.halted = Some(text);
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
