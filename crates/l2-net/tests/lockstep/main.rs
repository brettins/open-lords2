
mod session_tests;
mod flow_tests;
mod divergence_tests;
mod seat_tests;

#[path = "../common/mod.rs"]
mod common;

use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, FrameReader, Loopback,
    Message, PeerId, PlayerSlot, Session, SessionError, Tick, Transport,
};

struct Peer {
    slot: PlayerSlot,
    id: PeerId,
    endpoint: Endpoint,
    reader: FrameReader,
    session: Session,
    sim: ToySim,
    hashes: Vec<(Tick, u64)>,
    errors: Vec<SessionError>,
    waited_for: Vec<PlayerSlot>,
    sealed: Vec<Tick>,
    issued_through: Option<Tick>,
    pending: Option<(Tick, u64)>,
}

struct Table {
    net: Loopback,
    peers: Vec<Peer>,
    /// C256 in the loop: off, a divergence stays recorded for the dump tests.
    resync: bool,
    /// C256's state, cloned: `Resync` carries no bytes here.
    snapshot: Option<ToySim>,
}

impl Table {
    pub(crate) fn new(config: Config, players: u8, seed: u64) -> Table {
        let slots: Vec<PlayerSlot> = (0..players).map(PlayerSlot::new).collect();
        let ids: Vec<PeerId> = (0..players as u32).map(PeerId).collect();
        let net = Loopback::with_peers(&ids);
        let peers = (0..players as usize)
            .map(|i| {
                let sim = ToySim::new(seed, 6);
                Peer {
                    slot: slots[i],
                    id: ids[i],
                    endpoint: net.endpoint(ids[i]),
                    reader: FrameReader::new(),
                    session: Session::new(config.clone(), slots[i], &slots, seed, &sim),
                    sim,
                    hashes: Vec::new(),
                    errors: Vec::new(),
                    waited_for: Vec::new(),
                    sealed: Vec::new(),
                    issued_through: None,
                    pending: None,
                }
            })
            .collect();
        Table { net, peers, resync: false, snapshot: None }
    }

    fn send_all(&mut self, from: usize, message: &Message) {
        let bytes = frame(&Canonical::bytes_of(message)).unwrap();
        for j in 0..self.peers.len() {
            if j != from {
                let target = self.peers[j].id;
                self.peers[from].endpoint.send(target, &bytes).unwrap();
            }
        }
    }

    fn pump(&mut self) {
        let count = self.peers.len();

        for i in 0..count {
            if self.resync {
                if let Some(at) = self.peers[i].session.plan_resync() {
                    self.send_all(i, &Message::ResyncAt(at));
                }
            }
            let mut sealed = Vec::new();
            while let Some(packet) = self.peers[i].session.next_packet() {
                sealed.push(packet);
            }
            for packet in sealed {
                self.peers[i].sealed.push(packet.tick);
                let bytes = frame(&Canonical::bytes_of(&Message::Tick(packet))).unwrap();
                for j in 0..count {
                    if i != j {
                        let target = self.peers[j].id;
                        self.peers[i].endpoint.send(target, &bytes).unwrap();
                    }
                }
            }
        }

        for peer in &mut self.peers {
            while let Some((_from, bytes)) = peer.endpoint.poll() {
                peer.reader.feed(&bytes);
            }
            while let Some(message) = peer.reader.next_message().unwrap() {
                match decode_all::<Message>(&message).expect("a well-formed message") {
                    Message::Tick(packet) => {
                        if let Err(error) = peer.session.receive(packet) {
                            peer.errors.push(error);
                        }
                    }
                    Message::ResyncAt(at) => peer.session.resync_announced(at),
                    Message::Resync(r) => peer.pending = Some((r.tick, r.hash)),
                    other => panic!("unexpected {other:?}"),
                }
            }
        }

        for i in 0..count {
            loop {
                let peer = &mut self.peers[i];
                match peer.session.advance(&mut peer.sim) {
                    Advance::Stepped { tick, hash } => peer.hashes.push((tick, hash)),
                    Advance::Waiting { missing, .. } => {
                        peer.waited_for.extend(missing);
                        break;
                    }
                    Advance::Resyncing { at } if peer.slot == peer.session.authority() => {
                        let hash = l2_net::state_hash(&peer.sim);
                        self.snapshot = Some(peer.sim.clone());
                        peer.session.resync_sent();
                        let r = l2_net::Resync { tick: at, hash, state: Vec::new() };
                        self.send_all(i, &Message::Resync(r));
                    }
                    Advance::Resyncing { at } => match peer.pending.take_if(|p| p.0 == at) {
                        Some((_, hash)) => {
                            peer.sim = self.snapshot.clone().expect("the authority's world");
                            assert!(peer.session.resynced(at, hash, &peer.sim));
                        }
                        None => break,
                    },
                    Advance::Halted(_) => break,
                }
            }
        }
    }

    fn run(&mut self, rounds: usize) {
        for _ in 0..rounds {
            self.pump();
        }
    }

    fn run_scheduled(&mut self, rounds: usize, schedule: impl Fn(u8, Tick) -> Option<Order>) {
        for _ in 0..rounds {
            for peer in &mut self.peers {
                let at = peer.session.execution_tick();
                if peer.issued_through.is_none_or(|last| at > last) {
                    peer.issued_through = Some(at);
                    if let Some(order) = schedule(peer.slot.index(), at) {
                        peer.session.issue(order.payload());
                    }
                }
            }
            self.pump();
        }
    }

    fn all_agree(&self) -> bool {
        let first = l2_net::state_hash(&self.peers[0].sim);
        self.peers.iter().all(|p| l2_net::state_hash(&p.sim) == first)
    }
}

