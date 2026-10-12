//! C260: a joiner's *Leave game* hands its realm to the computer at one tick on every
//! remaining seat, and they play on in step. Real TCP on 127.0.0.1, the real lobby.

use crate::kit::{self, Seat};
use crate::seats::assets;
use l2_game::net::{replay, Stepper};
use l2_game::screens::leave::Leaving;
use l2_kingdom::tables::Tables;
use l2_net::Tick;

fn leave(seat: &mut Seat, assets: &l2_game::game::Assets) {
    seat.game.leaving = Some(Leaving::Leave);
    seat.pump(assets);
    assert!(seat.live.is_none() && !seat.game.net.lockstep, "the leaver is out of the game");
}

/// Ablations: the `Message::Leaving` arm in `Live::from_joiner` (the host waits for a
/// player who left); `game.issue(ToComputer)` in `Live::to_computer` (realm 2 stays
/// human); `Leaving` in `Live::leave` back to a bare close (the host waits).
#[test]
fn a_joiner_that_leaves_hands_its_realm_to_the_computer_and_the_host_plays_on() {
    kit::big_stack(two_seats);
}

fn two_seats() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let _scope = kit::scope("leave2");
    let (mut seats, _) = kit::game(&assets, 2);
    let mut leaver = seats.pop().unwrap();
    leave(&mut leaver, &assets);
    let at = seats[0].tick();
    kit::until(&mut seats, &assets, "the host runs on", |s| s[0].tick() > at + 300);
    let host = &seats[0];
    assert!(host.game.net.waiting.is_empty(), "the host waits for a player who left");
    assert!(host.game.net.halted.is_none(), "{:?}", host.game.net.halted);
    assert!(!host.game.kingdom.realms[2].is_human, "the computer plays the leaver's realm");
    assert_eq!(host.game.seats.len(), 1);
    // The handover is in the command stream: a fresh world fed it agrees on every hash.
    let record = host.live().record().expect("recorded from Start");
    let mut world = replay::rebuild(&assets, &record, Tables::DEFAULT).expect("rebuilt");
    let mut malformed = 0;
    let mut sim = Stepper { game: &mut world, malformed: &mut malformed };
    let last = record.replay.verify(&mut sim).expect("the host's replay verifies");
    assert!(last.0 > at + 250, "verified only to {last:?}");
    assert!(!world.kingdom.realms[2].is_human);
}

/// Three seats; John leaves; Richard and Matilda play on and agree.
/// Ablations: the host's forward (`send_but` in `Live::from_joiner`: the joiners never
/// hear each other); `self.session.retire` in `Live::from_host` (Matilda waits for John).
#[test]
fn three_seats_one_leaves_and_the_other_two_play_on_in_step() {
    kit::big_stack(three_seats);
}

fn three_seats() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let _scope = kit::scope("leave3");
    let (mut seats, _) = kit::game(&assets, 3);
    let mut john = seats.pop().unwrap();
    let realm = john.game.player;
    leave(&mut john, &assets);
    let at = seats[0].tick();
    kit::until(&mut seats, &assets, "both run on", |s| s.iter().all(|x| x.tick() > at + 300));
    for s in &seats {
        assert!(s.game.net.halted.is_none() && s.game.net.waiting.is_empty(), "{:?}", s.game.net.halted);
        assert!(!s.game.kingdom.realms[realm as usize].is_human, "the computer plays John's realm");
        assert_eq!(s.game.seats.len(), 2);
    }
    let t = kit::common_tick(&seats);
    for t in [Tick(t.0 - 100), t] {
        assert_eq!(seats[0].hash_at(t), seats[1].hash_at(t), "the two disagree at {t:?}");
        assert!(seats[0].hash_at(t).is_some());
    }
    assert_eq!(seats.iter().map(|s| s.live().session.divergences()).sum::<u32>(), 0);
}
