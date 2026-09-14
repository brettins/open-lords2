#![allow(unused_imports)]

mod pacing_tests;
pub use pacing_tests::*;

use super::*;

use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_kingdom::tables::Tables;
use l2_kingdom::UnitKind;

/// Where every merchant stood at the end of each **frame** of a turn started by
/// pressing End Turn on the campaign map, plus how many frames it took.
///
/// The machine is driven: one
/// [`Machine::update`] a frame, and no other door into the simulation.
fn watch_a_turn(game: &mut l2_game::Game, machine: &mut Machine) -> Vec<Vec<(u8, u8)>> {
    let assets = Assets::placeholder();
    let slots = merchant_slots(game);
    let before = game.kingdom.turn_count;

    {
        let mut ctx = Ctx { game, assets: &assets };
        machine.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    }
    assert!(l2_game::turn::turn_in_flight(game), "End Turn started a turn");

    let mut trail: Vec<Vec<(u8, u8)>> = vec![Vec::new(); slots.len()];
    for _ in 0..GIVE_UP {
        {
            let mut ctx = Ctx { game, assets: &assets };
            machine.update(&mut ctx);
        }
        assert_eq!(
            machine.top_id(),
            Some(ScreenId::Campaign),
            "nobody is at war on turn one, so nothing may come up over the map",
        );
        for (i, &slot) in slots.iter().enumerate() {
            let u = game.kingdom.campaign.units.get(slot).expect("a merchant is never lost");
            trail[i].push(u.tile());
        }
        if game.kingdom.turn_count > before {
            return trail;
        }
    }
    panic!("the turn never came round in {GIVE_UP} frames");
}

/// The frames on which this unit entered a tile.
fn entries(path: &[(u8, u8)]) -> Vec<u32> {
    path.windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] != w[1])
        .map(|(i, _)| i as u32 + 1)
        .collect()
}

