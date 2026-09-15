#![allow(unused_imports)]
use super::*;
use super::garrison_and_siege::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, map};
use l2_game::Game;
use l2_kingdom::map::{flags, terrain, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

/// **The door.** The sidebar's CASTLE button opens screen `0x1B` for the
/// selected county, and refuses a county that is not yours — `Castle_OpenScreen`
/// (`0x00436A88`), whose else-branch is message `0x70`.
#[test]
fn the_sidebar_castle_button_opens_the_chooser_for_your_own_county() {
    let (mut g, a, mut m) = on_the_map();
    click(&mut m, &mut g, &a, on(castle_button()));
    assert_eq!(m.top_id(), Some(ScreenId::Castle(1)), "the chooser opened on county 1");

    send(&mut m, &mut g, &a, Event::KeyDown(Key::Escape));
    g.selected = 2;
    click(&mut m, &mut g, &a, on(castle_button()));
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "county 2 is not yours");
}

#[test]
fn picking_a_castle_and_pressing_ok_starts_the_work() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    let (wood, stone) = (g.kingdom.realms[1].wood, g.kingdom.realms[1].stone);

    click(&mut m, &mut g, &a, on(castle_button()));
    click(&mut m, &mut g, &a, on(castle::type_rect(2)));
    press_and_wait(&mut m, &mut g, &a, on(castle::OK));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the screen closes on the order");
    let c = &g.kingdom.counties[1];
    assert_eq!(c.castle_type, 3, "castleType moves the moment the work is ordered");
    assert_eq!(c.castle_building, 0, "and nothing stood here before it");
    assert_eq!(c.castle_degraded, 1, "**the field nothing used to write**");
    assert!(c.castle_switch, "and the castle job is switched on");
    assert_eq!(c.castle_work_left, 800, "a Norman keep is 800 man-seasons");
    assert_eq!(c.castle_percent, 0);
    assert_eq!(g.kingdom.realms[1].wood, wood - 200);
    assert_eq!(g.kingdom.realms[1].stone, stone - 1_000);
    assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (0, 0));

    assert_eq!(
        g.kingdom.campaign.map.terrain_at(here.0, here.1),
        terrain::CASTLE_PLOT + 3,
        "the block is stamped as a Norman keep",
    );
}

#[test]
fn the_ok_button_refuses_the_castle_you_have_and_anything_smaller() {
    for (standing, pick, why) in [
        (3u8, 2usize, "already"),
        (3, 1, "smaller"),
        (3, 0, "smaller"),
    ] {
        let (mut g, a, mut m) = on_the_map();
        let here = visible_in(&g, 1);
        plot(&mut g, 1, here);
        g.kingdom.counties[1].castle_type = standing;
        let before = g.kingdom.realms[1].stone;

        click(&mut m, &mut g, &a, on(castle_button()));
        click(&mut m, &mut g, &a, on(castle::type_rect(pick)));
        press_and_wait(&mut m, &mut g, &a, on(castle::OK));

        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "{why}: the screen closes either way");
        assert_eq!(g.kingdom.counties[1].castle_degraded, 0, "{why}: no work was started");
        assert_eq!(g.kingdom.counties[1].castle_type, standing, "{why}: and nothing changed");
        assert_eq!(g.kingdom.realms[1].stone, before, "{why}: nothing was spent");
    }
}

/// `Castle_Order` (`0x00436D02`) ends with
/// `Labour_ToggleIndustryShare(county, 3, 1)`, so the castle job leaves the
/// order holding a share of the five-member industry group. Ours did not call
/// it, the share stayed 0, and this test used to assert the consequence — a
/// county with its mines running putting nobody on the walls *for ever*. With
/// the share ported a palisade's 200 man-seasons are done in four, every
/// industry still running. `docs/bugs.md` B68's *"for ever"* was the missing
/// toggle; what survives it is the ceiling asymmetry the entry describes.
#[test]
fn the_orders_labour_share_is_what_gets_a_castle_built() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);

    click(&mut m, &mut g, &a, on(castle_button()));
    click(&mut m, &mut g, &a, on(castle::type_rect(0)));
    press_and_wait(&mut m, &mut g, &a, on(castle::OK));

    assert!(
        g.kingdom.counties[1].labour_share[3] > 0,
        "the order staffs the castle job — Labour_ToggleIndustryShare(county, 3, 1)",
    );

    for _ in 0..4 {
        end_turn(&mut m, &mut g, &a);
    }
    let c = &g.kingdom.counties[1];
    assert_eq!(c.castle_work_left, 0, "four seasons finish a palisade's 200");
    assert_eq!(c.castle_degraded, 0, "and the castle is standing");
    assert_eq!(c.castle_type, 1, "a wooden palisade");
    assert_eq!(c.labour_share[3], 0, "the builders go back to the fields");
    assert!(c.labour[6] > 0, "who are in the forest, which never fills up");
}

#[test]
fn ending_turns_finishes_the_castle_and_it_arrives_with_a_garrison() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    for slot in 0..4 {
        g.kingdom.counties[1].industry[slot].has_resource = false;
    }

    click(&mut m, &mut g, &a, on(castle_button()));
    click(&mut m, &mut g, &a, on(castle::type_rect(0))); // a wooden palisade, 200 man-seasons
    press_and_wait(&mut m, &mut g, &a, on(castle::OK));
    assert_eq!(g.kingdom.counties[1].castle_degraded, 1);
    assert_eq!(g.kingdom.campaign.units.len(), 0, "nothing on the map yet");

    let mut seasons = 0;
    while g.kingdom.counties[1].castle_degraded != 0 && seasons < 20 {
        end_turn(&mut m, &mut g, &a);
        seasons += 1;
    }
    assert_eq!(
        g.kingdom.counties[1].castle_degraded, 0,
        "the palisade never topped out in {seasons} seasons — percent {}, work left {}",
        g.kingdom.counties[1].castle_percent, g.kingdom.counties[1].castle_work_left,
    );
    assert_eq!(g.kingdom.counties[1].castle_percent, 100);
    assert_eq!(g.kingdom.counties[1].castle_type, 1);
    assert!(!g.kingdom.counties[1].castle_switch, "the builders go back to the fields");

    let garrison = g.kingdom.counties[1].garrison_unit;
    assert_ne!(garrison, 0, "a new castle comes with a garrison");
    let unit = g.kingdom.campaign.units.get(garrison).expect("the garrison");
    assert_eq!(unit.men, 50, "50 archers for a wooden palisade");
    assert_eq!(unit.troops[TroopType::Archer.index()], 50, "and they carry bows");
    assert_eq!(unit.garrison_county, 1, "the unit knows which castle it is in");
    assert_eq!(
        (unit.x, unit.y),
        here,
        "and it was teleported onto the castle block, not left outside",
    );
}


