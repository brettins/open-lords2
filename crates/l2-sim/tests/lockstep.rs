
use l2_net::{
    decode_all, frame, Advance, Canonical, Command, Config, FrameReader, Message, PeerId,
    PlayerSlot, Session, Simulation, TcpTransport, Tick, Transport,
};
use l2_sim::{Battle, Troop, SIDE_A, SIDE_B};

const LINEUP: [Troop; 8] = [
    Troop::Swordsmen,
    Troop::Pikemen,
    Troop::Knights,
    Troop::Archers,
    Troop::Macemen,
    Troop::Crossbowmen,
    Troop::Catapults,
    Troop::Peasants,
];

fn lineup() -> Battle {
    let mut battle = Battle::new();
    for (i, troop) in LINEUP.into_iter().enumerate() {
        let side = if i % 2 == 0 { SIDE_A } else { SIDE_B };
        battle.add(troop, side, 6 + i as u16).unwrap();
    }
    battle
}

struct NetBattle {
    battle: Battle,
    perturb_at: Option<Tick>,
}

impl NetBattle {
    fn new() -> Self {
        NetBattle { battle: lineup(), perturb_at: None }
    }
}

const OP_ENGAGE: u8 = 1;

fn engage_order(a: u8, b: u8) -> Vec<u8> {
    vec![OP_ENGAGE, a, b]
}

impl Simulation for NetBattle {
    fn step(&mut self, tick: Tick, commands: &[Command]) {
        if self.perturb_at == Some(tick) {
            self.battle.figures[0].hits += 1;
        }
        for cmd in commands {
            if let [OP_ENGAGE, a, b] = cmd.payload[..] {
                let (a, b) = (a as usize, b as usize);
                if a != b && a < self.battle.figures.len() && b < self.battle.figures.len() {
                    self.battle.engage(a, b);
                }
            }
        }
        self.battle.step();
    }

    fn encode_state(&self, out: &mut Canonical) {
        out.section("battle");
        out.u32(self.battle.tick);
        out.len32(self.battle.figures.len());
        for f in &self.battle.figures {
            out.u8(f.troop as u8);
            out.u8(f.side);
            out.u16(f.men);
            out.u16(f.hits);
            out.i32(f.recovery_counter);
            out.u8(f.state as u8);
            out.u8(f.role as u8);
            out.option(f.opponent.as_ref(), |c, i| c.len32(*i));
            out.i32(f.exchange);
            out.bool(f.blow_used);
        }
        out.end_section();
    }
}

struct Peer<S: Simulation> {
    slot: PlayerSlot,
    session: Session,
    sim: S,
    endpoint: TcpTransport,
    reader: FrameReader,
    other: PeerId,
    hashes: Vec<(Tick, u64)>,
    issued_through: Option<Tick>,
}

fn schedule(slot: u8, at: Tick) -> Option<Vec<u8>> {
    match (slot, at.0) {
        (0, 2) => Some(engage_order(0, 1)),
        (1, 4) => Some(engage_order(2, 3)),
        (0, 7) => Some(engage_order(4, 5)),
        (1, 9) => Some(engage_order(6, 7)),
        _ => None,
    }
}

const HOST_ID: PeerId = PeerId(0);

fn connected_pair<S: Simulation>(seed: u64, build: &dyn Fn() -> S) -> [Peer<S>; 2] {
    let mut host = TcpTransport::listen("127.0.0.1:0").expect("binding a loopback port");
    let addr = host.local_addr().expect("a bound address");
    let client = TcpTransport::connect(addr, HOST_ID).expect("connecting");

    let mut joined = Vec::new();
    for _ in 0..10_000 {
        while host.poll().is_some() {}
        joined = host.take_joined();
        if !joined.is_empty() {
            break;
        }
    }
    assert_eq!(joined.len(), 1, "the host never accepted the client");

    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let make = |slot: PlayerSlot, endpoint: TcpTransport, other: PeerId| {
        let sim = build();
        let session = Session::new(Config::battle(), slot, &slots, seed, &sim);
        Peer {
            slot,
            session,
            sim,
            endpoint,
            reader: FrameReader::new(),
            other,
            hashes: Vec::new(),
            issued_through: None,
        }
    };
    [
        make(slots[0], host, joined[0]),
        make(slots[1], client, HOST_ID),
    ]
}

fn run_to<S: Simulation>(
    peers: &mut [Peer<S>; 2],
    target: Tick,
    schedule: fn(u8, Tick) -> Option<Vec<u8>>,
) {
    for _ in 0..100_000 {
        if peers.iter().all(|p| p.session.tick() >= target) {
            return;
        }
        if peers.iter().any(|p| p.session.is_halted()) {
            return;
        }
        for peer in peers.iter_mut() {
            let at = peer.session.execution_tick();
            if peer.issued_through.is_none_or(|last| at > last) {
                peer.issued_through = Some(at);
                if let Some(order) = schedule(peer.slot.index(), at) {
                    peer.session.issue(order);
                }
            }
            let mut sealed = Vec::new();
            while let Some(packet) = peer.session.next_packet() {
                sealed.push(frame(&Canonical::bytes_of(&Message::Tick(packet))).unwrap());
            }
            for bytes in sealed {
                peer.endpoint.send(peer.other, &bytes).unwrap();
            }
        }
        for peer in peers.iter_mut() {
            while let Some((_from, bytes)) = peer.endpoint.poll() {
                peer.reader.feed(&bytes);
            }
            while let Some(message) = peer.reader.next_message().unwrap() {
                match decode_all::<Message>(&message).unwrap() {
                    Message::Tick(packet) => peer.session.receive(packet).unwrap(),
                    other => panic!("unexpected message {other:?}"),
                }
            }
            while let Advance::Stepped { tick, hash } = peer.session.advance(&mut peer.sim) {
                peer.hashes.push((tick, hash));
            }
        }
    }
    panic!(
        "the session stalled at {:?} and {:?}, short of {target:?}",
        peers[0].session.tick(),
        peers[1].session.tick()
    );
}

const TICKS: Tick = Tick(120);

#[test]
fn two_peers_simulate_the_same_battle_over_a_real_socket() {
    let mut peers = connected_pair(0x1025_1796, &NetBattle::new);
    run_to(&mut peers, TICKS, schedule);

    let common = peers[0].hashes.len().min(peers[1].hashes.len());
    assert!(
        common >= TICKS.0 as usize,
        "only {common} ticks were executed on both peers"
    );
    assert_eq!(
        peers[0].hashes[..common],
        peers[1].hashes[..common],
        "the two peers diverged"
    );

    assert_eq!(
        peers[0].sim.battle, peers[1].sim.battle,
        "state differs despite matching checksums"
    );
    for peer in &peers {
        assert!(!peer.session.is_halted(), "{:?}", peer.session.halt_reason());
        assert!(peer.session.divergence().is_none());
    }
}

#[test]
fn the_commands_actually_reached_the_simulation() {
    let mut peers = connected_pair(0x00ab_cdef, &NetBattle::new);
    run_to(&mut peers, TICKS, schedule);

    let fresh = lineup();
    assert_ne!(
        peers[0].sim.battle, fresh,
        "no order took effect - this test would pass on two idle sims"
    );
    let hurt = peers[0]
        .sim
        .battle
        .figures
        .iter()
        .zip(&fresh.figures)
        .any(|(now, before)| now.hits > 0 || now.men < before.men);
    assert!(hurt, "orders crossed the wire but nobody fought");
}

#[test]
fn a_rule_refusal_is_identical_on_both_peers() {
    let mut peers = connected_pair(0x0000_5ca7, &NetBattle::new);
    run_to(&mut peers, TICKS, schedule);

    for peer in &peers {
        let catapult = peer.sim.battle.figures[6];
        assert_eq!(catapult.troop, Troop::Catapults);
        assert_eq!(catapult.opponent, None, "the catapult was drawn into melee");
        assert_eq!(catapult.hits, 0, "the catapult was fought");
    }
    assert_eq!(peers[0].sim.battle, peers[1].sim.battle);
}

#[test]
fn a_peer_running_different_rules_is_caught() {
    let mut peers = connected_pair(0x0000_dead, &NetBattle::new);
    peers[1].sim.perturb_at = Some(Tick(30));
    run_to(&mut peers, TICKS, schedule);

    assert!(
        peers.iter().any(|p| p.session.is_halted()),
        "a divergence ran to completion undetected"
    );
    let caught = peers
        .iter()
        .filter_map(|p| p.session.divergence())
        .next()
        .expect("halted without recording a divergence");
    assert!(
        caught.tick >= Tick(30),
        "reported {:?}, before the perturbation could have had an effect",
        caught.tick
    );

    let split = peers[0]
        .hashes
        .iter()
        .zip(&peers[1].hashes)
        .position(|(a, b)| a != b)
        .expect("no differing tick in the recorded history");
    assert_eq!(
        peers[0].hashes[split].0,
        Tick(30),
        "the first differing tick should be the perturbed one"
    );
}


use l2_sim::ai::{self, Ai, AiField};
use l2_sim::unit::{Units, CATEGORY_OF_TROOP};
use l2_sim::{Figure, State};

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
    fn new() -> Self {
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


use l2_sim::runner::{Army, BattleRunner};

const OP_ORDER: u8 = 2;

struct RunnerNetBattle {
    runner: BattleRunner,
    nudge_at: Option<Tick>,
}

fn close_field() -> l2_sim::Battlefield {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[30 * l2_sim::terrain::DIM + 40] = 0x04;
    layer[50 * l2_sim::terrain::DIM + 40] = 0x0F;
    l2_sim::terrain::build(&layer, 1)
}

const RUNNER_SEED: u64 = 0x0B47_71E5;

impl RunnerNetBattle {
    fn new() -> Self {
        let ai = [(Troop::Peasants, 12)];
        let player = [(Troop::Pikemen, 4)];
        RunnerNetBattle {
            runner: BattleRunner::deploy_armies(
                close_field(),
                RUNNER_SEED,
                Army { troops: &ai, owner: 1, human: false },
                Army { troops: &player, owner: 2, human: true },
            ),
            nudge_at: None,
        }
    }
}

impl Simulation for RunnerNetBattle {
    fn step(&mut self, tick: Tick, commands: &[Command]) {
        if self.nudge_at == Some(tick) {
            self.runner.fighters[0].x = self.runner.fighters[0].x.wrapping_add(1);
        }
        for cmd in commands {
            if let [OP_ORDER, unit, x, y] = cmd.payload[..] {
                self.runner.order_unit(unit as usize, x, y);
            }
        }
        self.runner.step();
    }

    fn encode_state(&self, out: &mut Canonical) {
        out.section("runner");
        out.u32(self.runner.tick);
        out.len32(self.runner.fighters.len());
        for f in &self.runner.fighters {
            out.u8(f.x);
            out.u8(f.y);
            out.u8(f.target.0);
            out.u8(f.target.1);
            out.u8(f.facing);
            out.u8(f.anim as u8);
            out.u8(f.phase);
            out.len32(f.path.len());
            out.u8(f.barred);
            out.u8(f.hold);
            out.u16(f.reroutes);
            out.u32(f.progress.tick_counter);
            out.u32(f.progress.substep);
            out.bool(f.progress.free);
            out.u32(f.moat_cell.unwrap_or(u32::MAX));
            out.u8(f.moat_load);
            out.u8(f.polar);
            // **How long the body has lain here** — `+0x173`, stepped by the
            // corpse state's own tick. Simulation state, not a picture:
            //
            // `BattleRunner::corpse_gone` is the slot's end, and in the
            // original a pot of oil still inside its eighty frames is a spent
            // pot that cannot pour. Two peers that disagreed would clear
            // different bodies. Added at merge — C209's branch left it out and
            // this check named it.
            out.u16(f.corpse);
            // **The frames a swapped man stands still** — `+0x172`, written by
            // `BattleMan_Step`'s `BattleMen_SwapPlaces` arm (`0x0049005F`) as
            // `(other & 1) + 1`. Two peers that disagreed would disagree about
            // his cell a frame later.
            out.u8(f.delay);
        }
        out.end_section();

        out.section("fire");
        out.bool(self.runner.wood_fire);
        out.u8(self.runner.battle_size_class());
        out.end_section();

        out.section("siege");
        let s = &self.runner.siege;
        out.u8(s.is_siege as u8);
        out.u8(s.castle_level);
        out.u32(s.rampart_hits);
        out.u32(s.gate_hits);
        out.u32(s.ramparts_breached);
        out.u8(s.gate_breached as u8);
        out.u8(s.broke_in as u8);
        out.u8(s.drawbridge_down as u8);
        out.u16(s.moat_filled);
        out.u16(s.wall_damage);
        let mut fold: u64 = 0xcbf2_9ce4_8422_2325;
        for c in &self.runner.field.cells {
            for b in [c.terrain, c.flags, c.gfx, c.elevation, c.surface] {
                fold = (fold ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        out.u64(fold);
        out.end_section();

        out.section("figures");
        out.len32(self.runner.sim.figures.len());
        for f in &self.runner.sim.figures {
            out.u16(f.men);
            out.u16(f.hits);
            out.u8(f.state as u8);
            out.u16(f.unit);
            out.u8(f.targeted);
            out.option(f.opponent.as_ref(), |c, i| c.len32(*i));
            out.u16(f.reload_counter);
        }
        out.end_section();

        // **The arrows.** A missile in flight is simulation state: it carries
        // damage, it is a tick away from killing somebody, and two peers that
        // disagree about where one is disagree about the battle. The original
        // agrees — its own sync digest copies all hundred records, `0x1DB0`
        // bytes, one `Sync_RecordDigest` per slot.
        //
        // **Every slot, not every live one.** A record that has just been freed
        // must hash differently from one, and a walk over
        // only the live ones is the second half of `docs/decisions.md` C39: a
        // whole *record* can go missing from the sweep and no amount of field
        // checking sees it.
        out.section("missiles");
        for slot in 1..=l2_sim::missile::MAX_MISSILES {
            let m = self.runner.missiles.get(slot);
            out.u8(m.owner);
            out.u8(m.class);
            out.u16(m.shooter);
            out.i32(m.x as i32);
            out.i32(m.y as i32);
            out.i32(m.target_x as i32);
            out.i32(m.target_y as i32);
            out.i32(m.cell_x as i32);
            out.i32(m.cell_y as i32);
            out.i32(m.dx);
            out.i32(m.dy);
            out.i32(m.err);
            out.u8(m.major_axis);
            out.u8(m.dir);
            out.u8(m.launch_elevation);
            out.u8(m.blocked_ticks);
            out.i32(m.sub_steps as i32);
            out.i32(m.ticks_flown as i32);
            out.i32(m.range_ticks as i32);
            out.bool(m.blocked);
            out.i32(m.ttl as i32);
            out.u16(m.power);
            out.u8(m.saved_surface);
            out.i32(m.fire_arrow);
        }
        out.end_section();

        out.section("units");
        for i in 1..=l2_sim::MAX_UNITS {
            let u = self.runner.units.get(i);
            out.u8(u.owner);
            out.u8(u.figures);
            out.u8(u.category);
            out.i32(u.orders as i32);
            out.i32(u.think as i32);
            out.i32(u.x as i32);
            out.i32(u.y as i32);
            out.i32(u.target_x as i32);
            out.i32(u.target_y as i32);
            out.i32(u.target_cell);
            out.bool(u.halted);
        }
        out.end_section();

        out.section("ai");
        let (state, increment) = self.runner.ai.rng.parts();
        out.u64(state);
        out.u64(increment);
        out.i32(self.runner.ai.strength_advantage);
        out.i32(self.runner.ai.advantage_timer);
        out.i32(self.runner.ai.approach_score);
        out.i32(self.runner.ai.breach_score);
        out.i32(self.runner.ai.ramparts_breached);
        for post in self.runner.ai_field.defence_posts {
            out.u32(post as u32);
        }
        out.end_section();
    }
}

const RUNNER_TICKS: Tick = Tick(700);

fn player_order(slot: u8, at: Tick) -> Option<Vec<u8>> {
    match (slot, at.0) {
        (1, 5) => Some(vec![OP_ORDER, 2, 40, 38]),
        _ => None,
    }
}

#[test]
fn two_peers_running_a_whole_battle_stay_bit_identical() {
    let mut peers = connected_pair(0x0B47_0001, &RunnerNetBattle::new);
    run_to(&mut peers, RUNNER_TICKS, player_order);

    let common = peers[0].hashes.len().min(peers[1].hashes.len());
    assert!(common >= RUNNER_TICKS.0 as usize, "only {common} ticks executed on both");
    assert_eq!(peers[0].hashes[..common], peers[1].hashes[..common], "the peers diverged");
    for peer in &peers {
        assert!(!peer.session.is_halted(), "{:?}", peer.session.halt_reason());
        assert!(peer.session.divergence().is_none());
    }
}

#[test]
fn both_the_players_order_and_the_ais_decision_reached_the_men() {
    let mut peers = connected_pair(0x0B47_0002, &RunnerNetBattle::new);
    run_to(&mut peers, RUNNER_TICKS, player_order);
    let sim = &peers[0].sim;
    let fresh = RunnerNetBattle::new();

    let ai_unit = sim.runner.units.live().next().unwrap();
    let player_unit = sim.runner.units.live().nth(1).unwrap();
    assert!(!sim.runner.units.get(ai_unit).human);
    assert!(sim.runner.units.get(player_unit).human);

    assert_eq!(sim.runner.units.get(ai_unit).orders, 3, "three thinks in 700 frames");
    assert_eq!(sim.runner.units.get(player_unit).orders, 0, "a player's unit never thinks");
    assert_eq!(fresh.runner.units.get(ai_unit).target_y, 50, "seeded on its own end");
    assert_eq!(
        sim.runner.units.get(ai_unit).target_y, 30,
        "the AI never marched on the enemy's deployment marker"
    );

    let moved = |r: &BattleRunner, side: l2_sim::Side| -> bool {
        r.fighters
            .iter()
            .zip(fresh.runner.fighters.iter())
            .any(|(a, b)| a.side == side && (a.x, a.y) != (b.x, b.y))
    };
    assert!(moved(&sim.runner, SIDE_B), "the AI's men never advanced");
    assert!(moved(&sim.runner, SIDE_A), "the ordered pikemen never advanced");
    assert_ne!(sim.runner.ai.rng, fresh.runner.ai.rng, "the jitter was never drawn");
}

#[test]
fn a_peer_whose_figure_stepped_wrong_is_caught() {
    let mut peers = connected_pair(0x0B47_0003, &RunnerNetBattle::new);
    peers[1].sim.nudge_at = Some(Tick(300));
    run_to(&mut peers, RUNNER_TICKS, player_order);

    assert!(
        peers.iter().any(|p| p.session.is_halted()),
        "a figure standing somewhere else ran to completion undetected"
    );
    let split = peers[0]
        .hashes
        .iter()
        .zip(&peers[1].hashes)
        .position(|(a, b)| a != b)
        .expect("no differing tick in the recorded history");
    assert_eq!(peers[0].hashes[split].0, Tick(300));
}

/// A peer that had one arrow where the other had none, and agreed on every
/// checksum, would kill a different man three ticks later and diverge
/// invisibly. The original agrees that this is state: its own sync digest
/// copies all hundred records, `0x1DB0` bytes, one `Sync_RecordDigest` a slot.
#[test]
fn an_arrow_in_flight_changes_the_state_hash() {
    let mut sim = RunnerNetBattle {
        runner: BattleRunner::deploy_armies(
            close_field(),
            RUNNER_SEED,
            l2_sim::runner::Army { troops: &[(Troop::Peasants, 12)], owner: 2, human: false },
            l2_sim::runner::Army { troops: &[(Troop::Archers, 4)], owner: 1, human: true },
        ),
        nudge_at: None,
    };
    let mut loosed = None;
    for t in 0..4_000u32 {
        sim.step(Tick(t), &[]);
        if sim.runner.missiles.live() > 0 {
            loosed = Some(Canonical::hash_of(&Hashable(&sim)));
            break;
        }
    }
    let hash = loosed.expect("no missile was ever loosed");

    let mut emptied = sim.runner.clone();
    for slot in 1..=l2_sim::missile::MAX_MISSILES {
        emptied.missiles.free(slot);
    }
    let stripped = RunnerNetBattle { runner: emptied, nudge_at: None };
    assert_ne!(
        hash,
        Canonical::hash_of(&Hashable(&stripped)),
        "the missile array is outside the checksum: two peers could disagree about \
         every arrow in the air and every checksum would report agreement"
    );
}

struct Hashable<'a>(&'a RunnerNetBattle);

impl l2_net::canonical::Encode for Hashable<'_> {
    fn encode(&self, out: &mut Canonical) {
        self.0.encode_state(out);
    }
}

/// `docs/decisions.md` C39: *completeness must be derived, not remembered.* The
/// encoder above is a hand-written list, and a hand-written list cannot fail for
/// a field nobody put in it — which is how eleven `Realm` fields, the whole
/// diplomatic matrix among them, once sat outside the lockstep checksum with
/// every test green.
///
/// The exemption table has the polarity C39 insists on: inclusion is the
/// default, a line is a claim with a reason attached, and a line naming a field
/// that no longer exists fails too — because a stale exemption looks like a
/// decision and covers nothing.
#[test]
fn every_field_of_a_missile_and_a_fighter_reaches_the_bytes() {
    const NOT_HASHED: &[(&str, &str, &str)] = &[
        ("Fighter", "path", "recomputed from hashed state; its length is hashed"),
        ("Fighter", "sim", "the index of the figure this Fighter is, and both walks are by index"),
        ("Fighter", "troop", "fixed at deployment and never written again"),
        ("Fighter", "side", "fixed at deployment and never written again"),
        ("Fighter", "facing_drawn", "drawn facing (+0x0D); presentation, read only by the renderer"),
        ("Fighter", "fidget", "Anim_StandA2's shuffle counter (+0x0B); presentation"),
        ("Fighter", "fidget_period", "seeded from index and x (+0x0C); presentation"),
    ];

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(root.join("tests/lockstep.rs")).expect("this file");
    // **`encode_state`'s body, not the whole file.** Scanning the file let any
    // `.name` anywhere in it stand for a hashed field: dropping
    // `Progress::free` from the encoder stayed green because
    // `emptied.missiles.free(slot)`, in an unrelated test below, matches
    // `.free`. A check a deliberate ablation cannot turn red is not a check.
    let head = "fn encode_state(&self, out: &mut Canonical) {\n        out.section(\"runner\");";
    let at = src.find(head).expect("the encoder");
    let rest = &src[at..];
    let end = rest.find("\n    }\n").expect("the encoder's closing brace");
    let encoder = rest[..end].to_string();
    let mut all: Vec<(String, String)> = Vec::new();
    for (file, want, least) in [
        ("src/missile/types.rs", "Missile", 5),
        ("src/runner/mod.rs", "Fighter", 5),
        ("src/siege/mod.rs", "SiegeState", 5),
        // **Nested structs were outside the walk.** `Fighter::progress` is one
        // name in the census and three fields in the digest, so `free` could
        // have been added and hashed by nobody with this test green — which is
        // exactly C39's failure, one level down.
        ("src/movement.rs", "Progress", 3),
    ] {
        let src = std::fs::read_to_string(root.join(file)).expect(file);
        let fields = fields_of(&src, want);
        assert!(fields.len() >= least, "{want} parsed as {} fields - the parser broke", fields.len());
        all.extend(fields.into_iter().map(|f| (want.to_string(), f)));
    }

    for (s, f, _) in NOT_HASHED {
        assert!(
            all.iter().any(|(a, b)| a == s && b == f),
            "NOT_HASHED names {s}::{f}, which no longer exists"
        );
    }

    let missing: Vec<String> = all
        .iter()
        .filter(|(s, f)| !NOT_HASHED.iter().any(|(a, b, _)| a == s && b == f))
        .filter(|(_, f)| !mentions(&encoder, f))
        .map(|(s, f)| format!("  {s}::{f}"))
        .collect();
    assert!(
        missing.is_empty(),
        "{} field(s) of the battle state are outside the lockstep checksum:\n{}\n\n\
         Add each to `RunnerNetBattle::encode_state`. A field that genuinely is not \
         simulation state goes in NOT_HASHED with the reason it is not.",
        missing.len(),
        missing.join("\n")
    );
    println!("{} fields of Missile and Fighter, all hashed", all.len());
}

fn fields_of(src: &str, want: &str) -> Vec<String> {
    let header = format!("struct {want} {{");
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        if !line.ends_with(&header) {
            continue;
        }
        let mut out = Vec::new();
        for body in lines.by_ref() {
            if body == "}" {
                return out;
            }
            let Some(rest) = body.strip_prefix("    ") else { continue };
            if rest.starts_with(' ') || rest.starts_with("//") || rest.starts_with('#') {
                continue;
            }
            let rest = rest.strip_prefix("pub(crate) ").or_else(|| rest.strip_prefix("pub ")).unwrap_or(rest);
            let Some((name, _)) = rest.split_once(": ") else { continue };
            if !name.is_empty()
                && name.chars().all(|c| c.is_lowercase() || c.is_numeric() || c == '_')
            {
                out.push(name.to_string());
            }
        }
        return out;
    }
    Vec::new()
}

fn mentions(src: &str, field: &str) -> bool {
    let needle = format!(".{field}");
    let mut from = 0;
    while let Some(at) = src[from..].find(&needle) {
        let end = from + at + needle.len();
        if !src[end..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            return true;
        }
        from = end;
    }
    false
}
