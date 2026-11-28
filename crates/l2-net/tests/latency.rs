//! How one-way latency sets the game's speed: `Live::pump` (`l2-game/src/net/live.rs:149`)
//! replayed over a simulated star network. `cargo test -p l2-net --test latency -- --ignored --nocapture`.

mod common;

use common::{Order, ToySim};
use l2_net::{Advance, Config, PlayerSlot, Session, TickPacket};
use std::collections::VecDeque;

/// `TICK_MS`, `l2-game/src/lib.rs:8`.
const FRAME_US: u64 = 16_000;
/// `CATCH_UP`, `l2-game/src/net/live.rs:26`.
const CATCH_UP: u32 = 4;

struct Peer {
    session: Session,
    sim: ToySim,
    mailbox: VecDeque<(u64, TickPacket)>,
    last_ready: u64,
    phase: u64,
    waited_frames: u32,
}

struct Net {
    peers: Vec<Peer>,
    one_way_us: u64,
    jitter_us: u64,
    rng: u64,
}

impl Net {
    fn new(seats: usize, one_way_us: u64, jitter_us: u64) -> Net {
        let slots: Vec<PlayerSlot> = (0..seats as u8).map(PlayerSlot::new).collect();
        let peers = slots
            .iter()
            .map(|&me| {
                let sim = ToySim::new(7, 4);
                Peer {
                    session: Session::new(Config::battle(), me, &slots, 7, &sim),
                    sim,
                    mailbox: VecDeque::new(),
                    last_ready: 0,
                    phase: (me.index() as u64 * 5_300) % FRAME_US,
                    waited_frames: 0,
                }
            })
            .collect();
        Net { peers, one_way_us, jitter_us, rng: 0x9E37_79B9_7F4A_7C15 }
    }

    fn delay(&mut self) -> u64 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = if self.jitter_us == 0 { 0 } else { (self.rng >> 33) % (self.jitter_us + 1) };
        self.one_way_us + j
    }

    /// TCP keeps order: a message is never ready before the one sent ahead of it.
    fn post(&mut self, to: usize, now: u64, packet: TickPacket) {
        let at = (now + self.delay()).max(self.peers[to].last_ready);
        self.peers[to].last_ready = at;
        self.peers[to].mailbox.push_back((at, packet));
    }

    /// `send_all`: the host to every joiner, a joiner to the host only (C260 star).
    fn send_all(&mut self, from: usize, now: u64, packet: TickPacket) {
        if from == 0 {
            for to in 1..self.peers.len() {
                self.post(to, now, packet.clone());
            }
        } else {
            self.post(0, now, packet);
        }
    }

    /// `Live::exchange`, `live.rs:200`; the host forwards with `send_but`, `live.rs:231`.
    fn exchange(&mut self, me: usize, now: u64) {
        while let Some(p) = self.peers[me].session.next_packet() {
            self.send_all(me, now, p);
        }
        while self.peers[me].mailbox.front().is_some_and(|(at, _)| *at <= now) {
            let (_, packet) = self.peers[me].mailbox.pop_front().unwrap();
            let sender = packet.from.index() as usize;
            self.peers[me].session.receive(packet.clone()).expect("an honest packet");
            if me == 0 {
                for to in 1..self.peers.len() {
                    if to != sender {
                        self.post(to, now, packet.clone());
                    }
                }
            }
        }
    }

    /// `Live::pump`, `live.rs:149`: exchange, up to `CATCH_UP` ticks, exchange.
    fn pump(&mut self, me: usize, now: u64, click: bool) {
        if click {
            self.peers[me].session.issue(Order::Attack { unit: 1 }.payload());
        }
        let mut stepped = false;
        for _ in 0..CATCH_UP {
            self.exchange(me, now);
            let p = &mut self.peers[me];
            match p.session.advance(&mut p.sim) {
                Advance::Stepped { .. } => stepped = true,
                Advance::Waiting { .. } => break,
                other => panic!("{other:?}"),
            }
        }
        self.exchange(me, now);
        if !stepped {
            self.peers[me].waited_frames += 1;
        }
    }
}

struct Run {
    speed: f64,
    waited: f64,
}

fn run(seats: usize, one_way_ms: u64, jitter_ms: u64, seconds: u64) -> Run {
    let mut net = Net::new(seats, one_way_ms * 1000, jitter_ms * 1000);
    let end = seconds * 1_000_000;
    let frames = end / FRAME_US;
    let mut events: Vec<(u64, usize)> = (0..frames)
        .flat_map(|k| (0..seats).map(move |p| (k, p)))
        .map(|(k, p)| (k * FRAME_US + net.peers[p].phase, p))
        .collect();
    events.sort();
    for (n, &(now, p)) in events.iter().enumerate() {
        net.pump(p, now, p == seats - 1 && n % 40 == 0);
    }
    let slowest = net.peers.iter().map(|p| p.session.tick().0).min().unwrap() as f64;
    let waited = net.peers.iter().map(|p| p.waited_frames).max().unwrap() as f64 / frames as f64;
    Run { speed: slowest / frames as f64, waited }
}

#[test]
#[ignore = "a measurement: prints the table"]
fn speed_against_one_way_latency() {
    println!("seats  one-way  jitter   speed  frames waiting");
    for seats in [2, 5] {
        for one_way in [0, 8, 16, 24, 32, 40, 50, 75, 100, 150] {
            for jitter in [0, 10] {
                let r = run(seats, one_way, jitter, 60);
                println!(
                    "{seats:>5}  {one_way:>4} ms  {jitter:>3} ms  {:>5.0}%  {:>9.0}%",
                    r.speed * 100.0,
                    r.waited * 100.0
                );
            }
        }
    }
}
