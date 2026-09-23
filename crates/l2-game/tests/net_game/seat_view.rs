//! `docs/netcode.md` §5: what a seat shows of a world event.

use l2_game::message::{category, Record};
use l2_game::net::NetGame;
use l2_game::turn_clock::{seat_saw_expiry, EXPIRED};
use l2_net::{Simulation, Tick};

/// The clock runs out in the world (`net::frame`); the seat's letter closes.
/// Ablation: `crate::message::dismiss` in `seat_saw_expiry`.
#[test]
fn the_clocks_expiry_closes_the_seats_open_letter() {
    let save = l2_testkit::england!();
    let mut game = l2_game::scenario::from_save(&save, l2_kingdom::tables::Tables::DEFAULT).unwrap();
    game.kingdom.options.time_limit = 30;
    let me = game.player;
    game.messages.enqueue(Record { group: 1, category: category::NOTICE, ..Record::default() }, me);
    assert!(game.messages.pull() && game.messages.is_open());
    let mut sim = NetGame::new(game);
    let mut seen = None;
    for t in 0..3_000 {
        sim.step(Tick(t), &[]);
        if seat_saw_expiry(&mut sim.game, &mut seen) {
            assert_eq!(sim.game.turn_clock.remaining(), EXPIRED);
            assert!(!sim.game.messages.is_open(), "the letter stayed up");
            return;
        }
        assert!(sim.game.messages.is_open(), "closed at tick {t}, before the clock ran out");
    }
    panic!("the clock never ran out");
}
