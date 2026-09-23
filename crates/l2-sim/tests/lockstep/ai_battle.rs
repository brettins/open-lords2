#![allow(unused_imports)]
use super::*;
use super::basic::*;
use super::runner_battle::*;
use super::runner_tests::*;
use super::hashing::*;
use l2_net::{
    decode_all, frame, Advance, Canonical, Command, Config, FrameReader, Message, PeerId,
    PlayerSlot, Session, Simulation, TcpTransport, Tick, Transport,
};
use l2_sim::{Battle, Troop, SIDE_A, SIDE_B};
use l2_sim::ai::{self, Ai, AiField};
use l2_sim::unit::{Units, CATEGORY_OF_TROOP};
use l2_sim::{Figure, State};
use l2_sim::runner::{Army, BattleRunner};


struct AiNetBattle {
    units: Units,
    figures: Vec<Figure>,
    positions: Vec<(u8, u8)>,
    field: AiField,
    ai: Ai,
    slip_at: Option<Tick>,
}

const AI_LINEUP: [(Troop, u8, u8, u8); 4] = [
    (Troop::Archers, 1, SIDE_B, 52),
    (Troop::Swordsmen, 1, SIDE_B, 50),
    (Troop::Peasants, 2, SIDE_A, 30),
    (Troop::Swordsmen, 2, SIDE_A, 28),
];

impl AiNetBattle {
    pub(crate) fn new() -> Self {
        let mut units = Units::new();
        let mut figures = Vec::new();
        let mut positions = Vec::new();
        for (troop, owner, side, y) in AI_LINEUP {
            let unit = units
                .create(owner, false, side, CATEGORY_OF_TROOP[troop.index()])
                .expect("a free unit slot");
            for i in 0..4u8 {
                let mut f = Figure::new(troop, side, 8);
                f.unit = unit as u16;
                f.owner = owner;
                figures.push(f);
                positions.push((38 + i, y));
            }
        }
        let mut sim = AiNetBattle {
            units,
            figures,
            positions,
            field: AiField::field((40, 20), (40, 60)),
            ai: Ai::new(AI_SEED),
            slip_at: None,
        };
        sim.units.rebuild_from_figures(&mut sim.figures);
        sim
    }

    fn move_everyone(&mut self) {
        for i in 0..self.figures.len() {
            if !self.figures[i].is_alive() || self.figures[i].state == State::Melee {
                continue;
            }
            let u = self.units.get(self.figures[i].unit as usize);
            let (tx, ty) = (u.target_x, u.target_y);
            let (x, y) = self.positions[i];
            let step = |from: u8, to: i16| -> u8 {
                match to.cmp(&(from as i16)) {
                    core::cmp::Ordering::Less => from.saturating_sub(1),
                    core::cmp::Ordering::Greater => (from + 1).min(79),
                    core::cmp::Ordering::Equal => from,
                }
            };
            self.positions[i] = (step(x, tx), step(y, ty));
        }
    }

    fn engage_touching(&mut self) {
        for a in 0..self.figures.len() {
            if !self.figures[a].is_alive() || self.figures[a].state == State::Melee {
                continue;
            }
            for b in (a + 1)..self.figures.len() {
                if !self.figures[b].is_alive()
                    || self.figures[b].state == State::Melee
                    || self.figures[b].owner == self.figures[a].owner
                    || self.positions[a] != self.positions[b]
                {
                    continue;
                }
                l2_sim::melee::engage(&mut self.figures, a, b);
                break;
            }
        }
    }
}

const AI_SEED: u64 = 0x0B0A_71E5;

impl Simulation for AiNetBattle {
    fn step(&mut self, tick: Tick, commands: &[Command]) {
        if self.slip_at == Some(tick) {
            self.ai.rng.next_u32();
        }
        for cmd in commands {
            if let [OP_ENGAGE, a, b] = cmd.payload[..] {
                let (a, b) = (a as usize, b as usize);
                if a != b && a < self.figures.len() && b < self.figures.len() {
                    l2_sim::melee::engage(&mut self.figures, a, b);
                }
            }
        }
        self.units.rebuild_from_figures(&mut self.figures);
        ai::update_all_units(
            &mut self.units,
            &mut self.figures,
            &self.positions,
            &self.field,
            &mut self.ai,
        );
        if tick.0.is_multiple_of(9) {
            self.move_everyone();
        }
        self.engage_touching();
        for i in 0..self.figures.len() {
            if self.figures[i].state == State::Melee {
                l2_sim::melee::tick(&mut self.figures, i);
            }
        }
    }

    fn encode_state(&self, out: &mut Canonical) {
        out.section("ai");
        let (state, increment) = self.ai.rng.parts();
        out.u64(state);
        out.u64(increment);
        out.i32(self.ai.strength_advantage);
        out.i32(self.ai.advantage_timer);
        out.i32(self.ai.commit_counter);
        out.i32(self.ai.engagement_count);
        out.bool(self.ai.rally_request);
        out.i32(self.ai.rally_x);
        out.i32(self.ai.rally_y);
        out.end_section();

        out.section("units");
        for i in 1..=l2_sim::MAX_UNITS {
            let u = self.units.get(i);
            out.u8(u.owner);
            out.u8(u.figures);
            out.u8(u.category);
            out.u16(u.last_attacker);
            out.u8(u.hit_memory);
            out.u8(u.times_hit);
            out.i32(u.orders as i32);
            out.i32(u.think as i32);
            out.i32(u.x as i32);
            out.i32(u.y as i32);
            out.i32(u.target_x as i32);
            out.i32(u.target_y as i32);
            out.i32(u.target_cell);
            out.bool(u.halted);
            out.u8(u.withdrawals);
        }
        out.end_section();

        out.section("figures");
        out.len32(self.figures.len());
        for (i, f) in self.figures.iter().enumerate() {
            out.u8(f.troop as u8);
            out.u16(f.men);
            out.u16(f.hits);
            out.u8(f.state as u8);
            out.u16(f.unit);
            out.u8(self.positions[i].0);
            out.u8(self.positions[i].1);
        }
        out.end_section();
    }
}

fn no_orders(_slot: u8, _at: Tick) -> Option<Vec<u8>> {
    None
}

const AI_TICKS: Tick = Tick(420);

#[test]
fn two_peers_running_the_battle_ai_stay_bit_identical() {
    let mut peers = connected_pair(0x0A17_0001, &AiNetBattle::new);
    run_to(&mut peers, AI_TICKS, no_orders);

    let common = peers[0].hashes.len().min(peers[1].hashes.len());
    assert!(common >= AI_TICKS.0 as usize, "only {common} ticks executed on both");
    assert_eq!(peers[0].hashes[..common], peers[1].hashes[..common], "the peers diverged");
    for peer in &peers {
        assert!(!peer.session.is_halted(), "{:?}", peer.session.halt_reason());
        assert!(peer.session.divergence().is_none());
    }
}

#[test]
fn the_ai_really_ran_and_the_jitter_really_moved() {
    let mut peers = connected_pair(0x0A17_0002, &AiNetBattle::new);
    run_to(&mut peers, AI_TICKS, no_orders);
    let sim = &peers[0].sim;

    assert_ne!(
        sim.ai.rng,
        Ai::new(AI_SEED).rng,
        "the generator never advanced - the jitter was not in play"
    );
    assert!(
        sim.units.live().all(|u| sim.units.get(u).orders >= 2),
        "every unit should have thought twice in 420 frames"
    );
    let fresh = AiNetBattle::new();
    assert_ne!(sim.positions, fresh.positions, "nobody moved");
    assert!(
        sim.units
            .live()
            .any(|u| sim.units.get(u).target_y != fresh.units.get(u).target_y),
        "no handler ever wrote a destination"
    );
}

#[test]
fn one_peer_whose_generator_slipped_is_caught() {
    let mut peers = connected_pair(0x0A17_0003, &AiNetBattle::new);
    peers[1].sim.slip_at = Some(Tick(210));
    run_to(&mut peers, AI_TICKS, no_orders);

    assert!(
        peers.iter().any(|p| p.session.is_halted()),
        "a divergent generator ran to completion undetected"
    );
    let split = peers[0]
        .hashes
        .iter()
        .zip(&peers[1].hashes)
        .position(|(a, b)| a != b)
        .expect("no differing tick in the recorded history");
    assert_eq!(peers[0].hashes[split].0, Tick(210));
}


