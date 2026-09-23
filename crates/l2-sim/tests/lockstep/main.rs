

mod basic;
mod ai_battle;
mod runner_battle;
pub(crate) use runner_battle::*;
mod runner_tests;
mod hashing;


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
    pub(crate) fn new() -> Self {
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

