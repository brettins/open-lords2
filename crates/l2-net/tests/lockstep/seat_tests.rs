//! C260: a seat retired at one tick, and a dropped seat taken back from the host's world.

use super::common::{Order, ToySim};
use l2_net::{state_hash, Advance, Config, PlayerSlot, Rejoin, Roster, Session, Tick, TickPacket};

struct Seat {
    session: Session,
    sim: ToySim,
    hashes: Vec<(Tick, u64)>,
}

fn table(n: u8) -> Vec<Seat> {
    let slots: Vec<PlayerSlot> = (0..n).map(PlayerSlot::new).collect();
    let make = |i: usize| {
        let sim = ToySim::new(0x260, 6);
        Seat { session: Session::new(Config::battle(), slots[i], &slots, 0x260, &sim), sim, hashes: Vec::new() }
    };
    (0..n as usize).map(make).collect()
}

/// One round among the seats whose index is in `up`: seal, deliver, advance.
fn round(seats: &mut [Seat], up: &[usize], tick: u32) {
    let mut sealed: Vec<(usize, TickPacket)> = Vec::new();
    for &i in up {
        if tick % 9 == i as u32 {
            seats[i].session.issue(Order::Attack { unit: i as u32 }.payload());
        }
        while let Some(p) = seats[i].session.next_packet() {
            sealed.push((i, p));
        }
    }
    for (from, p) in sealed {
        for &j in up.iter().filter(|&&j| j != from) {
            seats[j].session.receive(p.clone()).unwrap();
        }
    }
    for &i in up {
        let s = &mut seats[i];
        while let Advance::Stepped { tick, hash } = s.session.advance(&mut s.sim) {
            s.hashes.push((tick, hash));
        }
    }
}

/// Ablation: the `is_retired` filter in `Session::advance` (the seats wait for slot 2).
#[test]
fn a_retired_slot_is_not_waited_for_from_its_tick_and_the_rest_agree() {
    let mut seats = table(3);
    for t in 0..20 {
        round(&mut seats, &[0, 1, 2], t);
    }
    let from = seats[0].session.first_missing(PlayerSlot::new(2));
    for t in 20..25 {
        round(&mut seats, &[0, 1], t);
    }
    let mut probe = seats[0].sim.clone();
    assert!(matches!(seats[0].session.advance(&mut probe), Advance::Waiting { .. }));
    for s in &mut seats[..2] {
        s.session.retire(PlayerSlot::new(2), from);
    }
    for t in 25..60 {
        round(&mut seats, &[0, 1], t);
    }
    assert!(seats[0].session.tick().0 > from.0 + 20, "stuck at {:?}", seats[0].session.tick());
    assert_eq!(seats[0].hashes, seats[1].hashes);
}

/// Ablation: the `unsimulated` packets fed to the rejoined seat (it waits for tick
/// `next` forever); `s.tick = r.next` in `Session::rejoin` (it re-runs old ticks).
#[test]
fn a_rejoined_seat_runs_from_the_hosts_tick_and_agrees() {
    let mut seats = table(2);
    for t in 0..20 {
        round(&mut seats, &[0, 1], t);
    }
    for t in 20..25 {
        round(&mut seats, &[0], t);
    }
    let host = &seats[0];
    let next = host.session.tick();
    let r = Rejoin {
        slot: PlayerSlot::new(1),
        authority: PlayerSlot::new(0),
        slots: host.session.slots().to_vec(),
        retired: host.session.retired(),
        seed: host.session.seed(),
        next,
        hash: state_hash(&host.sim),
        roster: Roster::default(),
        state: Vec::new(),
    };
    let sim = host.sim.clone();
    let mut back = Seat { session: Session::rejoin(Config::battle(), &r, &sim), sim, hashes: Vec::new() };
    for p in seats[0].session.unsimulated(PlayerSlot::new(1)) {
        back.session.receive(p).unwrap();
    }
    seats[1] = back;
    for t in 25..60 {
        round(&mut seats, &[0, 1], t);
    }
    let later: Vec<_> = seats[0].hashes.iter().filter(|(t, _)| *t >= next).cloned().collect();
    assert!(later.len() > 20, "the host never ran on");
    assert_eq!(seats[1].hashes, later);
}
