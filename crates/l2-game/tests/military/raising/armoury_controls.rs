#![allow(unused_imports)]
use super::*;
use super::raising_flow::*;
use super::*;
use super::battle_part::*;
use super::marching::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

#[test]
fn the_plus_and_minus_on_a_rack_move_exactly_one_man() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    let bows = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 5).expect("a bow rack");
    click(&mut m, &mut g, &a, ((bows.0 + bows.2) / 2, (bows.1 + bows.3) / 2));

    let archer = TroopType::Archer.index();
    for expected in 1..=3 {
        click(&mut m, &mut g, &a, on(armoury::button_box(0)));
        assert_eq!(g.levy.basket.troops()[archer], expected, "+ moves one man");
    }
    click(&mut m, &mut g, &a, on(armoury::button_box(1)));
    assert_eq!(g.levy.basket.troops()[archer], 2, "- moves one back");
    click(&mut m, &mut g, &a, on(armoury::button_box(2)));
    assert_eq!(g.levy.basket.troops()[archer], 0, "NONE empties the rack");
    assert_eq!(g.levy.basket.unequipped(), 300);
}

#[test]
fn create_reaches_through_an_open_rack_and_the_other_two_buttons_do_not() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    let swords = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 3).expect("a sword rack");
    let sword_click = ((swords.0 + swords.2) / 2, (swords.1 + swords.3) / 2);

    click(&mut m, &mut g, &a, sword_click);
    for dead in [armoury::CHANGE_BOX, armoury::CANCEL_BOX] {
        click(&mut m, &mut g, &a, on(dead));
        assert_eq!(m.top_id(), Some(ScreenId::Rack(1, 3)), "{dead:?} acted on 0x0D");
        assert_eq!(m.depth(), 3, "and it did not disturb the stack either");
    }

    click(&mut m, &mut g, &a, on(armoury::button_box(3)));
    click(&mut m, &mut g, &a, on(armoury::CREATE_BOX));
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "Create is live, and it closed both");
    assert_eq!(m.depth(), 1, "the rack did not survive the armoury it was opened from");
    assert_eq!(g.kingdom.campaign.units.len(), 1);
    let (_, unit) = g.kingdom.campaign.units.iter().next().expect("the army");
    assert_eq!(unit.troops[TroopType::Swordsman.index()], 200);
}

/// `docs/decisions.md` C58: three wrong-screen bugs have reached this player
/// through a near-miss, so the boxes are walked.
#[test]
fn no_pixel_of_the_armoury_opens_the_wrong_thing() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));

    for &(x0, y0, x1, y1, troop) in &armoury::RACK_HOTSPOTS {
        for (x, y) in [(x0, y0), (x1 - 1, y0), (x0, y1 - 1), (x1 - 1, y1 - 1)] {
            click(&mut m, &mut g, &a, (x, y));
            assert_eq!(
                m.top_id(),
                Some(ScreenId::Rack(1, troop)),
                "({x}, {y}) is rack {troop}'s corner and opened something else",
            );
            click(&mut m, &mut g, &a, on(armoury::RACK_OK));
        }
    }
    click(&mut m, &mut g, &a, on(armoury::CHANGE_BOX));
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));
}


