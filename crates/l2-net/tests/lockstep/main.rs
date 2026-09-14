//! The lockstep core, exercised as a whole session.
//!
//! This is the file `docs/netcode.md` §6 is really asking for: **two
//! simulations in one process, fed identical commands, must produce
//! identical checksums, and a deliberately perturbed one must be
//! caught.** Everything here runs with no network, no game install, no
//! clock and no threads, which is the only reason it will be run often
//! enough to be worth having.
//!
//! The harness below is a complete session — sealing, framing,
//! transport, reassembly, ordering, stepping, checksum exchange — with
//! the loopback standing in for sockets. Nothing is stubbed out except
//! the sockets themselves.

mod session_tests;
pub use session_tests::*;
mod flow_tests;
pub use flow_tests::*;
mod divergence_tests;
pub use divergence_tests::*;

#[path = "../common/mod.rs"]
mod common;

use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, FrameReader, HaltReason, Loopback,
    Message, PeerId, PlayerSlot, Fixed, Session, SessionError, Tick, Transport,
};

struct Peer {
    slot: PlayerSlot,
    id: PeerId,
    endpoint: Endpoint,
    reader: FrameReader,
    session: Session,
    sim: ToySim,
    /// Every checksum this peer computed, so two peers can be compared
    /// tick by tick
    hashes: Vec<(Tick, u64)>,
    errors: Vec<SessionError>,
    waited_for: Vec<PlayerSlot>,
    /// Every tick this peer sealed a packet for.
    sealed: Vec<Tick>,
    /// The last execution tick a scheduled command was issued for, so
    /// a schedule fires once per tick however many rounds that tick
    /// takes.
    issued_through: Option<Tick>,
}

struct Table {
    net: Loopback,
    peers: Vec<Peer>,
}

impl Table {
    /// `players` peers, all in one session, each with its own
    /// simulation seeded identically.
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
                }
            })
            .collect();
        Table { net, peers }
    }

    /// One turn of the loop every peer runs: seal, send, drain,
    /// advance.
    fn pump(&mut self) {
        let count = self.peers.len();

        for i in 0..count {
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
                    other => panic!("unexpected {other:?}"),
                }
            }
        }

        for peer in &mut self.peers {
            loop {
                match peer.session.advance(&mut peer.sim) {
                    Advance::Stepped { tick, hash } => peer.hashes.push((tick, hash)),
                    Advance::Waiting { missing, .. } => {
                        peer.waited_for.extend(missing);
                        break;
                    }
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

    /// Issue commands according to a schedule keyed on the **execution
    /// tick**, not on how many rounds of the loop have gone by.
    ///
    /// The distinction is the whole reason this method exists, and
    /// finding it out cost a failing test: a player types when they
    /// type, so under latency the same keystroke lands on a later tick
    /// — which changes the game, correctly and by design. A test that
    /// issued per round would therefore compare two runs that were
    /// given genuinely different input and call the difference a
    /// desync. Keying the schedule to the tick a command will execute
    /// on is what makes "the network's timing changed nothing" a
    /// statement about the lockstep core
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

