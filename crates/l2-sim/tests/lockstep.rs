//! The battle simulation driven through the lockstep session, over real sockets.
//!
//! `l2-sim` knows nothing about networking, and `l2-net` never depends on
//! `l2-sim`. Nothing in either crate's own tests can therefore check the one
//! claim both exist to support. This file is that seam.
//!
//! It said "a dev-dependency only" until the battle AI needed `Pcg32` for its
//! strength-advantage jitter, which made `l2-net` a real dependency of
//! `l2-sim` — for that one frozen generator and nothing else, on the same
//! argument `l2-kingdom` already makes. The load-bearing rule is unchanged and
//! narrower than "no dependency": **nothing in `l2-sim` may reach for the
//! session, the transport or the sockets.** A simulation that knows about the
//! network is one that can branch on it.
//!
//! What it proves: two peers exchanging commands over a genuine TCP connection
//! produce **bit-identical battle state on every tick**. If a determinism rule
//! were broken — a float, an iteration in hash order, a branch on an address —
//! this is where it would surface.

use l2_net::{
    decode_all, frame, Advance, Canonical, Command, Config, FrameReader, Message, PeerId,
    PlayerSlot, Session, Simulation, TcpTransport, Tick, Transport,
};
use l2_sim::{Battle, Troop, SIDE_A, SIDE_B};

/// The lineup both peers start from. Mixed deliberately: different recovery
/// rates, and one siege engine that must never be drawn into melee.
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

/// A battle wearing the `Simulation` trait.
///
/// Kept in the test rather than in `l2-sim` so the simulation crate keeps no
/// dependency on the network crate. The real wiring belongs in the application,
/// which is the only thing that legitimately knows about both.
struct NetBattle {
    battle: Battle,
    /// Stands in for the thing lockstep actually fears: one peer running
    /// subtly different rules. See `a_peer_running_different_rules_is_caught`.
    perturb_at: Option<Tick>,
}

impl NetBattle {
    fn new() -> Self {
        NetBattle { battle: lineup(), perturb_at: None }
    }
}

/// An order is an opcode and two figure indices to pair into a duel.
/// Deliberately tiny: what is under test is the transport and the determinism,
/// not a command language.
const OP_ENGAGE: u8 = 1;

fn engage_order(a: u8, b: u8) -> Vec<u8> {
    vec![OP_ENGAGE, a, b]
}

impl Simulation for NetBattle {
    fn step(&mut self, tick: Tick, commands: &[Command]) {
        // A single man's worth of damage on one peer only - the smallest
        // divergence the encoding can express, which is the one worth testing.
        if self.perturb_at == Some(tick) {
            self.battle.figures[0].hits += 1;
        }
        // Commands arrive already in a total order, identical on every peer.
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

/// One order per peer, a few ticks apart, so that commands genuinely cross the
/// wire in both directions rather than the test proving only that two idle
/// simulations agree.
fn schedule(slot: u8, at: Tick) -> Option<Vec<u8>> {
    match (slot, at.0) {
        (0, 2) => Some(engage_order(0, 1)),
        (1, 4) => Some(engage_order(2, 3)),
        (0, 7) => Some(engage_order(4, 5)),
        // Figure 6 is the catapult. The simulation must refuse this, identically
        // on both peers.
        (1, 9) => Some(engage_order(6, 7)),
        _ => None,
    }
}

/// The id each side uses to name the other. The host learns the client's from
/// `take_joined`; the client names the host when it connects.
const HOST_ID: PeerId = PeerId(0);

fn connected_pair<S: Simulation>(seed: u64, build: &dyn Fn() -> S) -> [Peer<S>; 2] {
    let mut host = TcpTransport::listen("127.0.0.1:0").expect("binding a loopback port");
    let addr = host.local_addr().expect("a bound address");
    let client = TcpTransport::connect(addr, HOST_ID).expect("connecting");

    // The listener is non-blocking, so the accept happens on a poll.
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

/// Drive both peers until each has executed `target` ticks.
///
/// The loop is the same shape a real frame loop has — issue, seal, send, poll,
/// receive, advance — so what passes here is what the engine will do.
fn run_to<S: Simulation>(
    peers: &mut [Peer<S>; 2],
    target: Tick,
    schedule: fn(u8, Tick) -> Option<Vec<u8>>,
) {
    for _ in 0..100_000 {
        if peers.iter().all(|p| p.session.tick() >= target) {
            return;
        }
        // A halted session never steps again, so waiting for the tick count
        // would spin to the iteration cap. Returning here is not a pass: every
        // caller asserts on what it expected to happen.
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

    // Not merely equal checksums - equal simulations.
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

    // A test that passes on two idle simulations proves nothing, so check that
    // the orders had an effect: someone must be fighting, and someone hurt.
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

    // The order at tick 9 tries to pair figure 6 (a catapult) with figure 7.
    // Siege engines are never drawn into melee, so both peers must refuse it the
    // same way; a rule applied on one side only is exactly how lockstep breaks.
    for peer in &peers {
        let catapult = peer.sim.battle.figures[6];
        assert_eq!(catapult.troop, Troop::Catapults);
        assert_eq!(catapult.opponent, None, "the catapult was drawn into melee");
        assert_eq!(catapult.hits, 0, "the catapult was fought");
    }
    assert_eq!(peers[0].sim.battle, peers[1].sim.battle);
}

/// The test that makes the other three mean something.
///
/// Three green tests prove nothing unless this seam can go red, so give one
/// peer a rule the other does not have and confirm it is caught. One extra hit
/// on one figure on one tick is the smallest divergence expressible here — if
/// the checksum catches that, it catches anything coarser.
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

    // And the hashes agree right up to the moment they must not.
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

// ---------------------------------------------------------------------------
// The same seam, with the battle AI running
// ---------------------------------------------------------------------------
//
// The AI is the only part of the battle model that draws a random number: the
// strength advantage carries a -10..+21 jitter, re-rolled every 101 frames, and
// with the aggression threshold at 5 that jitter alone decides whether the field
// AI attacks in most battles. So it is exactly the kind of thing that desyncs
// two peers if it is ever taken from a system source, and exactly the kind of
// thing a checksum over positions alone would notice only long after the fact.
//
// Everything below runs the real dispatch - `ai::update_all_units`, the three
// tables, the seventeen handlers - on both peers over the same socket the tests
// above use.

use l2_sim::ai::{self, Ai, AiField};
use l2_sim::unit::{Units, CATEGORY_OF_TROOP};
use l2_sim::{Figure, State};

/// A battle with units, positions and an AI, wearing the `Simulation` trait.
///
/// The mover is deliberately crude - one cell per nine ticks, straight at the
/// unit's destination - because what is under test is the AI's decisions and
/// the determinism of its draw, not the mover. `l2-view` owns the real one.
struct AiNetBattle {
    units: Units,
    figures: Vec<Figure>,
    positions: Vec<(u8, u8)>,
    field: AiField,
    ai: Ai,
    /// Set on one peer only, to prove the seam can still go red with the AI in
    /// the loop.
    slip_at: Option<Tick>,
}

/// Two units a side, twenty cells apart, chosen so that every non-stub entry of
/// the field table gets exercised: archers (category 1), peasants (2) and
/// swordsmen (3).
const AI_LINEUP: [(Troop, u8, u8, u8); 4] = [
    // troop, owner, side, y
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

    /// One cell toward the unit's destination, every ninth tick.
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

    /// Melee where two enemy figures share a cell. Enough that men die, so the
    /// strength advantage genuinely moves over the run.
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
        // The smallest divergence the AI can express: one peer's generator one
        // step out. If the jitter ever escaped the lockstep state, this is what
        // it would look like.
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
        // The generator itself, so a divergence in the draw is caught on the
        // tick it happens rather than whenever it next changes a decision.
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

/// Nothing to issue: the whole point is that the AI, not a player, is driving.
fn no_orders(_slot: u8, _at: Tick) -> Option<Vec<u8>> {
    None
}

/// Long enough for two thinks (200 frames each) and four recomputations of the
/// strength advantage (101 frames each).
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

/// A green determinism test proves nothing unless the AI was actually running,
/// so check that it drew, decided and moved somebody.
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

/// The seam can still go red with the AI in it. One peer's generator is
/// advanced one extra step on tick 210 - the smallest divergence the jitter can
/// express - and the checksum must catch it on that tick.
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

// ---------------------------------------------------------------------------
// The battle a player would actually watch
// ---------------------------------------------------------------------------
//
// `NetBattle` synchronises the melee model and `AiNetBattle` synchronises the
// order handlers, but both drive their own toy mover on a bare grid.
// `docs/plan-review.md` hole 6 is precisely that: *"the netcode has never
// synchronised the simulation a player would watch."* This is that simulation —
// `l2_sim::runner::BattleRunner`, the whole of it: a `.skr`-shaped battlefield,
// units raised into deployment slots, the seventeen order handlers on their
// 200-frame cadence, formation reforms, pathfinding and melee.

use l2_sim::runner::{Army, BattleRunner};

/// A player's click, as a command: send unit `u` to `(x, y)`.
const OP_ORDER: u8 = 2;

struct RunnerNetBattle {
    runner: BattleRunner,
    /// Set on one peer only, to prove the seam still goes red on the real
    /// simulation and not only on the toys.
    nudge_at: Option<Tick>,
}

/// Markers twenty cells apart rather than forty, so the whole opening of a
/// battle - deployment, the player order, the AI's first three thinks and the
/// march that follows - fits inside seven hundred frames. Everything else is
/// the blank template.
fn close_field() -> l2_sim::Battlefield {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[30 * l2_sim::terrain::DIM + 40] = 0x04;
    layer[50 * l2_sim::terrain::DIM + 40] = 0x0F;
    l2_sim::terrain::build(&layer, 1)
}

const RUNNER_SEED: u64 = 0x0B47_71E5;

impl RunnerNetBattle {
    fn new() -> Self {
        // Peasants are category 2, whose silent opening is two thinks rather
        // than the melee handler's four, so the AI issues its first march
        // order on its third think, at frame 600. Twelve of them against four
        // pikemen is a weighted 48 against 32, so the strength advantage is
        // +50 before the jitter and the aggressive branch is not a coin toss.
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
        // One cell of movement on one peer: the smallest divergence a position
        // can express.
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
        }
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
            out.bool(u.halted);
        }
        out.end_section();

        out.section("ai");
        let (state, increment) = self.runner.ai.rng.parts();
        out.u64(state);
        out.u64(increment);
        out.i32(self.runner.ai.strength_advantage);
        out.i32(self.runner.ai.advantage_timer);
        out.end_section();
    }
}

/// Long enough for the AI's first three thinks — the two silent ones and the
/// march that follows — plus six recomputations of the strength advantage.
const RUNNER_TICKS: Tick = Tick(700);

/// The player's one order, crossing the wire from the peer that owns side 0.
///
/// Unit 2 is the pikemen: side 4 is raised first, so unit 1 is the AI's.
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

/// A green determinism test proves nothing unless something happened. Both
/// halves must be visible: the player's order arrived over the socket and moved
/// his pikemen, and the AI decided for itself and moved its peasants.
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

    // The AI thought on its own cadence and then wrote a destination. A human
    // unit never thinks, so its `orders` must still be zero — that asymmetry is
    // `Battle_UpdateAllUnits`'s human guard, and it is what makes this a check
    // on the AI rather than on the order machinery.
    assert_eq!(sim.runner.units.get(ai_unit).orders, 3, "three thinks in 700 frames");
    assert_eq!(sim.runner.units.get(player_unit).orders, 0, "a player's unit never thinks");
    // Its third think is `BattleUnit_OrderToEnemyEnd`, so its destination is a
    // slot of the *enemy's* marker at y = 30 - twenty cells from where it
    // deployed and from where an un-ordered unit's destination is seeded.
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
    // And the generator really is part of the synchronised state.
    assert_ne!(sim.runner.ai.rng, fresh.runner.ai.rng, "the jitter was never drawn");
}

/// One cell out of place on one peer, on tick 300, must be caught on tick 300.
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
