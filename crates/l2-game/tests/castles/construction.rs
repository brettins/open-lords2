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

/// **The whole verb by mouse**: open the chooser, pick a castle out of the row
/// of five, press the tick — and the county is building one.
///
/// `castle_degraded` had no writer a player could reach before this. The
/// assertion that matters is not that the field moved but that **nothing in
/// this test wrote it**.
#[test]
fn picking_a_castle_and_pressing_ok_starts_the_work() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    let (wood, stone) = (g.kingdom.realms[1].wood, g.kingdom.realms[1].stone);

    click(&mut m, &mut g, &a, on(castle_button()));
    // The third strip: a Norman keep. The five are the five castle pictures,
    // side by side, and their rectangles are the original's widget table.
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
    // 200 wood and 1,000 stone, paid out of a store that had both.
    assert_eq!(g.kingdom.realms[1].wood, wood - 200);
    assert_eq!(g.kingdom.realms[1].stone, stone - 1_000);
    assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (0, 0));

    // And the map knows: the plot's terrain is a castle now.
    assert_eq!(
        g.kingdom.campaign.map.terrain_at(here.0, here.1),
        terrain::CASTLE_PLOT + 3,
        "the block is stamped as a Norman keep",
    );
}

/// The OK button's two refusals, driven through the screen.
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

/// **The order's own labour share is what gets a castle built**, and it is the
/// original's, not this fixture's shape.
///
/// `Castle_Order` (`0x00436D02`) ends with
/// `Labour_ToggleIndustryShare(county, 3, 1)`, so the castle job leaves the
/// order holding a share of the five-member industry group. Ours did not call
/// it, the share stayed 0, and this test used to assert the consequence — a
/// county with its mines running putting nobody on the walls *for ever*. With
/// the share ported a palisade's 200 man-seasons are done in four, every
/// industry still running. `docs/bugs.md` B68's *"for ever"* was the missing
/// toggle; what survives it is the ceiling asymmetry the entry describes.
///
/// **The click is not driven here and it is worth saying why.** The industry
/// toggle goes through `MapScreen::county_at`, which reads the *painted* county
/// plane — and the plane comes from the map **file**, which `Assets::placeholder`
/// supplies as 4,096 zero bytes. So no click on any tile of these synthetic
/// worlds ever finds a county. `tests/screens_map.rs`'s
/// `every_painted_pixel_of_a_mine_reaches_the_industry_toggle` drives that half
/// against the real sheet and the real map.
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

/// **A castle ordered from the map is finished by ending turns**, and it comes
/// with its own garrison of archers — `Castle_BuildTick`'s completion branch,
/// which nothing had ever run.
#[test]
fn ending_turns_finishes_the_castle_and_it_arrives_with_a_garrison() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    // A county with nothing to mine, so the industry half of its people has
    // nowhere to go but the castle. See the test above for what a county with
    // mines does instead.
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

    // `Castle_RaiseFreeGarrison`: a palisade is worth 50 archers, mustered and
    // marched straight inside.
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

// ---------------------------------------------------------------------------
// 2. Manning one
// ---------------------------------------------------------------------------

