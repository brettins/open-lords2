//! C260: seats coming and going under a game. A joiner that leaves, or that the
//! host gives up waiting for, is handed to the computer at one tick on every seat; a
//! dropped joiner is waited for and taken back on the same install. `docs/netcode.md` §8.

use super::*;
use crate::command::ToComputer;
use l2_net::{Join, NoSeat, Rejoin, PROTOCOL_VERSION};

/// Polls a leaving seat gives its goodbye to be written: about 200 ms. `[I]` ours.
const GOODBYE: u32 = 200;

/// C260: a seat's player name: the room's, else the realm's lord.
pub fn name_of(game: &Game, slot: PlayerSlot) -> String {
    let roster = game.net.roster.get(slot).map(|p| p.name.clone());
    let realm = game.realm_of(slot).map(|r| game.player_names[r as usize].as_str().to_string());
    roster.or(realm).unwrap_or_else(|| "A player".into())
}

impl Live {
    /// C260: a seat taken back into a game under way; `game` is `resync::rebuild`'s.
    pub fn rejoin(transport: TcpTransport, game: &mut Game, r: &Rejoin) -> Result<Live, String> {
        let mut malformed = 0;
        super::super::resync::adopt(game, &mut malformed, &r.state)?;
        seat_battle(game);
        game.net.lockstep = true;
        let sim = Stepper { game: &mut *game, malformed: &mut malformed };
        if state_hash(&sim) != r.hash {
            return Err("the host's world did not load the same here".into());
        }
        let session = Session::rejoin(Config::battle(), r, &sim);
        Ok(Live::with(session, transport, malformed, None))
    }

    /// C260: the join-code service stays open so a dropped player can come back by code.
    pub fn keep_relay(&mut self, relay: Option<super::super::relay::Host>) {
        self.relay = relay;
    }

    /// C260: a host's goodbye is `Halt(Ended)`, a joiner's `Leaving`; then every connection closes.
    pub fn leave(&mut self) {
        let bye = if self.is_host() { Message::Halt(HaltReason::Ended) } else { Message::Leaving };
        self.send_all(&bye);
        for _ in 0..GOODBYE {
            let _ = self.transport.flush();
            while self.transport.poll().is_some() {}
            if self.transport.peers().iter().all(|&p| self.transport.pending_out(p) == 0) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        for peer in self.transport.peers() {
            self.transport.disconnect(peer);
        }
    }

    pub(super) fn is_host(&self) -> bool {
        self.session.local_slot() == self.session.authority()
    }

    pub(super) fn is_seated(&self, peer: PeerId) -> bool {
        !self.strangers.contains(&peer) && !self.rejoining.iter().any(|&(p, _)| p == peer)
    }

    pub(super) fn note_strangers(&mut self) {
        if let Some(relay) = self.relay.as_mut() {
            for stream in relay.pump() {
                let _ = self.transport.add(stream);
            }
        }
        let open = self.transport.peers();
        for peer in self.transport.take_joined() {
            if self.is_seated(peer) && open.contains(&peer) {
                self.strangers.push(peer);
            }
        }
    }

    /// C260: to every seated connection; a stranger hears nothing until it is seated.
    pub(super) fn send_all(&mut self, m: &Message) {
        let bytes = Canonical::bytes_of(m);
        self.send_but(PeerId(u32::MAX), &bytes);
    }

    pub(super) fn send_but(&mut self, except: PeerId, bytes: &[u8]) {
        for peer in self.transport.peers() {
            if peer != except && self.is_seated(peer) {
                let _ = self.transport.send(peer, bytes);
            }
        }
    }

    fn send(&mut self, peer: PeerId, m: &Message) {
        let _ = self.transport.send(peer, &Canonical::bytes_of(m));
    }

    /// C260: the waiting box's *Let the computer play*, drained here.
    pub(super) fn control(&mut self, game: &mut Game) {
        if std::mem::take(&mut game.net.computer_plays) && self.is_host() {
            for slot in self.waiting.clone() {
                self.to_computer(game, slot);
            }
        }
    }

    /// C260: a closed connection. The host waits for that seat; a joiner lost its host.
    pub(super) fn dropped(&mut self, gone: &[PeerId]) {
        self.strangers.retain(|p| !gone.contains(p));
        self.rejoining.retain(|(p, _)| !gone.contains(p));
        if !self.is_host() {
            self.session.halt(HaltReason::Left);
            return;
        }
        let slots: Vec<PlayerSlot> = self.peers.iter().filter(|(p, _)| gone.contains(p)).map(|&(_, s)| s).collect();
        self.peers.retain(|(p, _)| !gone.contains(p));
        for slot in slots {
            self.away(slot);
        }
    }

    fn away(&mut self, slot: PlayerSlot) {
        let tick = self.session.tick();
        if self.waiting.contains(&slot) || self.session.is_retired(slot, tick) {
            return;
        }
        self.waiting.push(slot);
        self.send_all(&Message::Away(slot));
    }

    /// Silent past [`TIMEOUT`]: the host waits for that seat as for a drop; a joiner
    /// stops only when its host's packet is the one missing (C240).
    pub(super) fn silent(&mut self, missing: &[PlayerSlot]) {
        if missing.iter().all(|s| self.waiting.contains(s)) {
            self.stalled = 0;
            return;
        }
        self.stalled += 1;
        if self.stalled < TIMEOUT {
            return;
        }
        if !self.is_host() {
            if missing.contains(&self.session.authority()) {
                self.session.halt(HaltReason::Timeout);
            }
            return;
        }
        for &slot in missing {
            for (peer, _) in self.peers.iter().filter(|&&(_, s)| s == slot) {
                self.transport.disconnect(*peer);
            }
            self.peers.retain(|&(_, s)| s != slot);
            self.away(slot);
        }
        self.stalled = 0;
    }

    pub(super) fn leaving(&mut self, game: &mut Game, peer: PeerId) {
        let Some(&(_, slot)) = self.peers.iter().find(|&&(p, _)| p == peer) else { return };
        self.to_computer(game, slot);
        let who = name_of(game, slot);
        game.net.notice = Some((format!("{who} left. The computer now plays their realm."), NOTICE));
    }

    /// C260: the host stops expecting `slot` after what it has, tells the others, and
    /// orders the realm to the computer, which every seat runs at one tick.
    fn to_computer(&mut self, game: &mut Game, slot: PlayerSlot) {
        let from = self.session.first_missing(slot);
        self.session.retire(slot, from);
        self.send_all(&Message::Retire { slot, from });
        self.waiting.retain(|&s| s != slot);
        if let Some(realm) = game.realm_of(slot) {
            game.issue(ToComputer { realm });
            game.net.notice = Some((format!("The computer now plays {}'s realm.", name_of(game, slot)), NOTICE));
        }
        for (peer, _) in self.peers.iter().filter(|&&(_, s)| s == slot) {
            self.transport.disconnect(*peer);
        }
        self.peers.retain(|&(_, s)| s != slot);
    }

    /// C260: a `Join` after `Start`. A dropped seat's install is taken back; one the
    /// computer took is told so; anyone else is told the game has started.
    pub(super) fn knock(&mut self, game: &Game, peer: PeerId, join: &Join) {
        self.strangers.retain(|&p| p != peer);
        let h = &join.hello;
        let same = h.protocol == PROTOCOL_VERSION
            && h.ruleset_hash == Canonical::hash_of(&game.kingdom.tables)
            && h.quirks == game.kingdom.options.quirks.bits();
        let mine = |i: &super::super::InstallId| !i.is_nil() && i.0 == join.install;
        let slot = game.net.installs.iter().find(|(_, i)| mine(i)).map(|&(s, _)| s);
        let retired = |s: PlayerSlot| self.session.retired().iter().any(|&(r, _)| r == s);
        let answer = match slot {
            _ if !same => Message::Refused(Vec::new()),
            Some(s) if self.waiting.contains(&s) => {
                self.rejoining.push((peer, s));
                return;
            }
            Some(s) if retired(s) => Message::NoSeat(NoSeat::Retired),
            _ => Message::NoSeat(NoSeat::Started),
        };
        self.send(peer, &answer);
        let _ = self.transport.flush();
        self.transport.disconnect(peer);
    }

    /// C260: seats a rejoining player once the host has run all that seat sent: C256's
    /// payload for the world after `next - 1`, then every packet in hand from `next`.
    pub(super) fn admit(&mut self, game: &mut Game) {
        if self.session.resync_barrier().is_some() {
            return;
        }
        let now = self.session.tick();
        let ready: Vec<(PeerId, PlayerSlot)> =
            self.rejoining.iter().copied().filter(|&(_, s)| self.session.first_missing(s) == now).collect();
        for (peer, slot) in ready {
            let state = super::super::resync::encode(game, self.malformed);
            let hash = state_hash(&Stepper { game: &mut *game, malformed: &mut self.malformed });
            let r = Rejoin {
                slot,
                authority: self.session.authority(),
                slots: self.session.slots().to_vec(),
                retired: self.session.retired(),
                seed: self.session.seed(),
                next: now,
                hash,
                roster: game.net.roster.clone(),
                state,
            };
            self.send(peer, &Message::Rejoin(Box::new(r)));
            for packet in self.session.unsimulated(slot) {
                self.send(peer, &Message::Tick(packet));
            }
            for w in self.waiting.clone().into_iter().filter(|&w| w != slot) {
                self.send(peer, &Message::Away(w));
            }
            self.rejoining.retain(|&(p, _)| p != peer);
            self.waiting.retain(|&s| s != slot);
            self.send_but(peer, &Canonical::bytes_of(&Message::Back(slot)));
            self.peers.push((peer, slot));
            game.net.notice = Some((format!("{} is back.", name_of(game, slot)), NOTICE));
        }
    }
}
