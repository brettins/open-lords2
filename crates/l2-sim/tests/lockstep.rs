//! The battle simulation driven through the lockstep session, over real sockets.
//!
//! `l2-sim` and `l2-net` are built to be independent: the simulation knows
//! nothing about networking, and `l2-net` never depends on `l2-sim`. Nothing in
//! either crate's own tests can therefore check the one claim both exist to
//! support. This file is that seam, and it is a dev-dependency only — the
//! production `l2-sim` stays dependency-free.
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

struct Peer {
    slot: PlayerSlot,
    session: Session,
    sim: NetBattle,
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

fn connected_pair(seed: u64) -> [Peer; 2] {
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
        let sim = NetBattle::new();
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
fn run_to(peers: &mut [Peer; 2], target: Tick) {
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
    let mut peers = connected_pair(0x1025_1796);
    run_to(&mut peers, TICKS);

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
    let mut peers = connected_pair(0x00ab_cdef);
    run_to(&mut peers, TICKS);

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
    let mut peers = connected_pair(0x0000_5ca7);
    run_to(&mut peers, TICKS);

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
    let mut peers = connected_pair(0x0000_dead);
    peers[1].sim.perturb_at = Some(Tick(30));
    run_to(&mut peers, TICKS);

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
