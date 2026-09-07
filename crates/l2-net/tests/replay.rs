//! Replays, and the CI check they exist to make possible.
//!
//! `docs/netcode.md` §6 calls this the part that pays for itself, and
//! `docs/decisions.md` D7 explains why: two implementations agreeing
//! proves only that the same author ported the same misunderstanding
//! twice, while a property of the *data* is a real check. A recorded
//! session that must replay to bit-identical checksums is a property of
//! the data, and it runs on one machine with no network and no second
//! player.
//!
//! The last test in this file is the shape a real replay corpus takes:
//! record a session, keep the bytes, and assert forever after that they
//! still reproduce.

mod common;

use common::{Order, ToySim};
use l2_net::{
    decode_all, state_hash, Advance, Canonical, Command, Config, PlayerSlot, Replay,
    ReplayMismatch, Session, Simulation, Tick,
};

/// One peer playing alone, which is enough to record a session: the
/// lockstep core does not care that the other slots are empty.
fn record_a_session(seed: u64, rounds: usize) -> (Replay, ToySim) {
    let slots = [PlayerSlot::new(0)];
    let mut sim = ToySim::new(seed, 5);
    let mut session = Session::new(Config::battle(), slots[0], &slots, seed, &sim);

    for round in 0..rounds {
        if round % 4 == 0 {
            session.issue(Order::Attack { unit: (round % 5) as u32 }.payload());
        }
        if round % 6 == 3 {
            session.issue(
                Order::MoveTo {
                    unit: 1,
                    x: l2_net::Fixed::from_int(round as i32 % 60),
                    y: l2_net::Fixed::from_int(7),
                }
                .payload(),
            );
        }
        while session.next_packet().is_some() {}
        while let Advance::Stepped { .. } = session.advance(&mut sim) {}
    }

    (session.replay().expect("recording").clone(), sim)
}

#[test]
fn a_recorded_session_replays_to_the_same_state() {
    let (replay, played) = record_a_session(0xfeed, 40);
    assert!(replay.hashes.len() > 30);
    assert!(!replay.commands.is_empty());

    let mut fresh = ToySim::new(0xfeed, 5);
    let last = replay.verify(&mut fresh).expect("a replay must reproduce");
    assert_eq!(Some(last), replay.last_tick());
    assert_eq!(state_hash(&fresh), state_hash(&played), "the final states must be identical");
}

/// The whole point: a simulation that has stopped being deterministic
/// fails this, on one machine, with no second player.
#[test]
fn a_replay_catches_a_simulation_that_has_changed() {
    let (replay, _) = record_a_session(3, 30);

    let mut altered = ToySim::new(3, 5);
    altered.bias = 1; // the same change a desyncing peer would have
    match replay.verify(&mut altered) {
        Err(ReplayMismatch::Diverged { tick, recorded, actual }) => {
            assert_ne!(recorded, actual);
            assert!(tick.0 > 0);
            // The first differing tick, not the last: a list of every
            // tick that differs afterwards carries no information.
            let first_command = replay.commands.first().map(|(t, _)| *t).unwrap();
            assert!(tick >= first_command);
        }
        other => panic!("a biased simulation must not reproduce: {other:?}"),
    }
}

#[test]
fn a_replay_notices_the_wrong_starting_state() {
    let (replay, _) = record_a_session(5, 10);
    let mut wrong_seed = ToySim::new(6, 5);
    assert!(matches!(
        replay.verify(&mut wrong_seed),
        Err(ReplayMismatch::InitialState { .. })
    ));

    let mut wrong_size = ToySim::new(5, 4);
    assert!(matches!(
        replay.verify(&mut wrong_size),
        Err(ReplayMismatch::InitialState { .. })
    ));
}

#[test]
fn verifying_twice_gives_the_same_answer() {
    let (replay, _) = record_a_session(9, 20);
    let first = replay.verify(&mut ToySim::new(9, 5)).unwrap();
    let second = replay.verify(&mut ToySim::new(9, 5)).unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_replay_round_trips_through_its_encoding() {
    let (replay, _) = record_a_session(11, 25);
    let bytes = Canonical::bytes_of(&replay);
    assert!(bytes.starts_with(&l2_net::REPLAY_MAGIC));

    let decoded: Replay = decode_all(&bytes).expect("decode");
    assert_eq!(decoded, replay);
    assert_eq!(Canonical::bytes_of(&decoded), bytes);

    // And the decoded copy is still a working replay.
    decoded.verify(&mut ToySim::new(11, 5)).expect("a decoded replay must still reproduce");
}

#[test]
fn a_replay_is_small() {
    let (replay, _) = record_a_session(13, 200);
    let bytes = Canonical::bytes_of(&replay);
    assert!(replay.hashes.len() > 190);
    // A few kilobytes for two hundred ticks. Most of it is the
    // per-tick checksums, which are what make a divergence localisable
    // rather than merely detectable.
    assert!(bytes.len() < 16 * 1024, "{} bytes for 200 ticks", bytes.len());
}

#[test]
fn a_file_that_is_not_a_replay_is_refused() {
    assert!(decode_all::<Replay>(b"not a replay at all").is_err());
    let mut wrong_version = Canonical::bytes_of(&record_a_session(1, 3).0);
    wrong_version[4] = 99; // the version word
    assert!(decode_all::<Replay>(&wrong_version).is_err());
}

#[test]
fn a_replay_can_be_queried_by_tick() {
    let (replay, _) = record_a_session(17, 20);
    let tick = replay.commands.first().map(|(t, _)| *t).unwrap();
    assert!(!replay.commands_at(tick).is_empty());
    assert!(replay.hash_at(tick).is_some());
    assert!(replay.commands_at(Tick(9_999)).is_empty());
    assert!(replay.hash_at(Tick(9_999)).is_none());
}

/// A pinned end-to-end value, which is what a replay corpus in CI is
/// for.
///
/// Recording a session and then replaying it in the same test proves
/// only that the code agrees with itself — the failure mode
/// `docs/decisions.md` D7 warns about. The constant below is what makes
/// this a regression test instead: it was produced by this crate on one
/// day and written down, so it fails if the PRNG stream moves, if
/// `Fixed`'s rounding changes, if the canonical encoding shifts a
/// field, or if the session starts ordering commands differently.
///
/// A real corpus would be a directory of recorded battles. One pinned
/// session is the same check at the smallest size that still runs on a
/// bare checkout.
#[test]
fn a_recorded_session_still_produces_the_checksum_it_did_when_written() {
    let (replay, sim) = record_a_session(0x1234, 12);

    assert_eq!(replay.last_tick(), Some(Tick(35)));
    assert_eq!(
        state_hash(&sim),
        0xa695_5b05_146d_2a2d,
        "the recorded session no longer produces the checksum it was pinned at. \
         If a primitive changed on purpose, re-pin it; if not, something has \
         stopped being deterministic."
    );
    assert_eq!(
        l2_net::xxhash64(&Canonical::bytes_of(&replay), 0),
        0xf1cb_9b9c_143d_4926,
        "the replay file's own bytes changed, so old recordings will no longer decode"
    );

    // And it still round-trips and reproduces from those bytes.
    let stored: Replay = decode_all(&Canonical::bytes_of(&replay)).unwrap();
    let mut fresh = ToySim::new(0x1234, 5);
    let final_tick = stored.verify(&mut fresh).expect("the recording must still reproduce");
    assert_eq!(stored.hash_at(final_tick), Some(state_hash(&fresh)));
    assert_eq!(stored.seed, 0x1234);
    assert_eq!(stored.slots, vec![PlayerSlot::new(0)]);
}

/// A replay records the commands *as applied*, which is already in the
/// order `order_commands` produced. Re-ordering it must be a no-op.
#[test]
fn the_recorded_command_order_is_already_canonical() {
    let (replay, _) = record_a_session(19, 30);
    for tick in replay.hashes.iter().map(|(t, _)| *t) {
        let recorded = replay.commands_at(tick);
        let mut sorted: Vec<Command> = recorded.clone();
        l2_net::order_commands(&mut sorted);
        assert_eq!(recorded, sorted, "commands at {tick} were not in canonical order");
    }
}

/// The trait is all a replay needs, so anything implementing it can be
/// replayed — including a simulation this crate has never heard of.
#[test]
fn replaying_needs_nothing_but_the_trait() {
    struct Counter(i64);
    impl Simulation for Counter {
        fn step(&mut self, tick: Tick, commands: &[Command]) {
            self.0 += tick.0 as i64 + commands.len() as i64;
        }
        fn encode_state(&self, out: &mut Canonical) {
            out.i64(self.0);
        }
    }

    let slots = [PlayerSlot::new(0)];
    let mut sim = Counter(0);
    let mut session = Session::new(Config::kingdom(), slots[0], &slots, 1, &sim);
    for _ in 0..5 {
        session.issue(b"anything".to_vec());
        while session.next_packet().is_some() {}
        while let Advance::Stepped { .. } = session.advance(&mut sim) {}
    }

    let replay = session.replay().unwrap().clone();
    let mut fresh = Counter(0);
    replay.verify(&mut fresh).expect("reproduces");
    assert_eq!(fresh.0, sim.0);
}
