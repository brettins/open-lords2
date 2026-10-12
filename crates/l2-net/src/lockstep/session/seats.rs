//! C260: a seat retired at one tick every peer agrees on, and a dropped seat taken
//! back. The original removes a departing realm instead (`Net_OnSessionEvent`
//! `0x0043E6A2`, event 2 in play: `Realm_Eliminate`).

use super::*;
use crate::lockstep::state_digest;

impl Session {
    /// C260: what `slot` sealed before `from` stands; nothing more is expected of it.
    pub fn retire(&mut self, slot: PlayerSlot, from: Tick) {
        if slot == self.local || !self.slots.contains(&slot) {
            return;
        }
        self.retired.insert(slot.index(), from.0);
        self.inbox.retain(|&(t, s), _| s != slot.index() || t < from.0);
    }

    pub fn is_retired(&self, slot: PlayerSlot, tick: Tick) -> bool {
        self.retired.get(&slot.index()).is_some_and(|&from| tick.0 >= from)
    }

    pub fn retired(&self) -> Vec<(PlayerSlot, Tick)> {
        self.retired.iter().map(|(&s, &t)| (PlayerSlot::new(s), Tick(t))).collect()
    }

    /// C260: the first tick from the next to run with no packet from `slot` in.
    /// `[I]` TCP keeps one seat's packets in order, so all before it have arrived.
    pub fn first_missing(&self, slot: PlayerSlot) -> Tick {
        let mut t = self.tick.0;
        while self.inbox.contains_key(&(t, slot.index())) {
            t += 1;
        }
        Tick(t)
    }

    /// C260: every packet in hand for ticks not yet run, but `except`'s.
    pub fn unsimulated(&self, except: PlayerSlot) -> Vec<TickPacket> {
        self.inbox
            .iter()
            .filter(|(&(t, s), _)| s != except.index() && t >= self.tick.0)
            .map(|(&(t, s), commands)| TickPacket {
                tick: Tick(t),
                from: PlayerSlot::new(s),
                commands: commands.clone(),
                acks: Vec::new(),
            })
            .collect()
    }

    /// C260: a seat back in a game: `sim` is the host's world after `next - 1`, and
    /// this seat seals from `next`. `[D]` no replay: none can rebuild this world.
    pub fn rejoin(config: Config, r: &crate::seat::Rejoin, sim: &impl Simulation) -> Session {
        let config = Config { record: false, ..config };
        let mut s = Session::new(config, r.slot, &r.slots, r.seed, sim);
        s.set_authority(r.authority);
        for &(slot, from) in &r.retired {
            s.retire(slot, from);
        }
        s.tick = r.next;
        s.next_seal = r.next.0;
        if let Some(prev) = r.next.0.checked_sub(1).map(Tick) {
            s.simulated = Some(prev);
            s.acked_through = Some(prev.0);
            s.history.push(prev, state_digest(sim));
            s.last_agreed = Some(prev);
            s.floor = Some(prev);
        }
        s
    }

    pub fn hash_at(&self, tick: Tick) -> Option<u64> {
        self.history.hash_at(tick)
    }
}
