#![allow(unused_imports)]
use super::*;
use super::game_flow_tests::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::county as county_screen;
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{
    SetupPage, SetupScreen, CUSTOM_BUTTONS, CUSTOM_BUTTON_Y, MAP_LIST_ROW, MAP_LIST_X, MAP_LIST_Y,
};
use l2_game::screens::village::VillageScreen;
use l2_game::Game;
use l2_kingdom::field::FieldType;
use l2_kingdom::tables::{Commodity, Tables};
use l2_mods::Platform;
use l2_view::village as vill;
use l2_view::{campaign, Canvas};

// ------------------------------------------------------------------- the drop

/// **One drag moves two sidebar numbers and throws a switch.**
///
/// England turn one's county 8 is the person's, cuts wood with 108 of its 435
/// and has an iron mine that `Game_SetupRealmsAndCounties` left **off** — the
/// loop takes the first of wood, iron, stone the county has and stops
/// save agrees. One icon of foresters dropped on the mine:
///
/// * **`FUN_00439CC2` switches the mine on**, so its site turns to working
///   (`1 + 1`) and its ceiling to 100,000 — *"mining started as off"*, and
///   putting men on it is how the original turns it on;
/// * **the wood row redraws** at `(108 − popBand) × 80%`
///   gone — *"industry values don't seem to update"*;
/// * **the iron row appears** under it with `popBand × 80%`.
///
/// **Ablations, both run.** Delete `self.switch_on_by_drop(county, to)` from
/// `Kingdom::move_labour`: red at the switch. Delete the `refresh_estimates` and
/// `refresh_blacksmiths` calls from its loop: red at the mine's ceiling, 0
/// against 100,000 — the first assertion that reads an estimate, and ahead of
/// the wood row, which would still draw `+86`.
#[test]
fn a_drop_on_a_switched_off_mine_switches_it_on_and_both_rows_redraw() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;
    assert!(!game.kingdom.options.advanced_farming, "the flat 80 below is Advanced Farming off");

    let county = 8u8;
    let id = county as usize;
    assert!(game.is_players(county), "England turn one seats the person in county 8");
    let (iron, wood) = (Commodity::Iron.index(), Commodity::Wood.index());
    {
        let c = &game.kingdom.counties[id];
        assert!(c.industry[iron].has_resource && !c.industry[3].has_resource, "a mine and no quarry");
        assert!(!c.industry[iron].enabled, "the file's mine starts switched off");
        assert!(c.industry[wood].enabled, "and its forest on");
    }
    let site = l2_kingdom::map::industry_site(&game.kingdom.campaign.map, county, Commodity::Iron)
        .expect("the mine has a site");
    assert_eq!(game.kingdom.campaign.map.terrain[site], SITE_BASE[iron], "an idle mine");

    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    let foresters = game.kingdom.counties[id].labour[6];
    let before = draw_stack(&mut m, &mut game, &assets);
    let file_forecast = foresters * FLAT_EFFICIENCY / 100;
    assert!(drawn_in_row(&before, &assets, file_forecast, 0, 1), "the file's +{file_forecast} on the wood row");

    let (from, to) = (cluster_of(&game, county, 6), cluster_of(&game, county, 4));
    drag_one_icon(&mut m, &mut game, &assets, county, from, to);

    let c = &game.kingdom.counties[id];
    let band = c.pop_band;
    assert_eq!(c.labour[4], band, "one icon is popBand people, and they are on the mine");
    assert_eq!(c.labour[6], foresters - band, "and out of the forest");

    // The switch, the ceiling and the picture — what the simulation uses and
    // what the map shows, which must be the same byte.
    assert!(c.industry[iron].enabled, "FUN_00439CC2: men on a site switch it on");
    assert_eq!(c.labour_useful[4], UNBOUNDED, "a switched-on mine takes as many as you like");
    assert_eq!(game.kingdom.campaign.map.terrain[site], SITE_BASE[iron] + 1, "the site is working");
    assert_eq!(county_screen::industry_rows(c), vec![6, 4], "the sidebar lists wood, then iron");

    let after = draw_stack(&mut m, &mut game, &assets);
    let wood_now = (foresters - band) * FLAT_EFFICIENCY / 100;
    let iron_now = band * FLAT_EFFICIENCY / 100;
    assert!(drawn_in_row(&after, &assets, wood_now, 0, 2), "the wood row redraws at +{wood_now}");
    assert!(
        !drawn_in_row(&after, &assets, file_forecast, 0, 2),
        "and the file's +{file_forecast} is gone from it"
    );
    assert!(drawn_in_row(&after, &assets, iron_now, 1, 2), "the iron row appears with +{iron_now}");
}

/// **The drag selection box is drawn with the debug overlay OFF**, because
/// `Village_DrawBand` (`0x00412795`) is the original's and not ours.
///
/// C173 put our outline behind Ctrl+D on the strength of not having found this
/// function
/// disappeared, it was probably a debug thing that you removed with other debug
/// boxes."* The assertion is the whole rectangle in the original's own colour —
/// `Ui_DrawRectOutline`'s literal `0x20` — read off the canvas at the four
/// edges, with the overlay left at its default.
#[test]
fn the_drag_selection_box_is_drawn_without_the_debug_overlay() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;
    assert!(!game.prefs.debug_overlay, "the default session, which is the point");

    let county = 8u8;
    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    // A press and a drag with no release: screen `0x05`, the band state.
    let top = vill::SCENE_Y;
    let (x0, y0) = (vill::SCENE_X + 20, top + 40);
    let (x1, y1) = (x0 + 60, y0 + 30);
    handle(&mut m, &mut game, &assets, Event::Click { x: x0, y: y0 });
    handle(&mut m, &mut game, &assets, Event::Pointer { x: x1, y: y1 });
    let canvas = draw_stack(&mut m, &mut game, &assets);

    // `FUN_00403cf4(x, y, w, h, 0x20)` — top, bottom, left and right.
    const BAND_INK: u8 = 0x20;
    let at = |x: i32, y: i32| canvas.at(x as usize, y as usize);
    for x in x0..=x1 {
        assert_eq!(at(x, y0), BAND_INK, "the band's top edge at x {x}");
        assert_eq!(at(x, y1), BAND_INK, "the band's bottom edge at x {x}");
    }
    for y in y0..=y1 {
        assert_eq!(at(x0, y), BAND_INK, "the band's left edge at y {y}");
        assert_eq!(at(x1, y), BAND_INK, "the band's right edge at y {y}");
    }

    // And it is the band that drew it, not the village: released, it is gone.
    handle(&mut m, &mut game, &assets, Event::Release { x: x1, y: y1 });
    let after = draw_stack(&mut m, &mut game, &assets);
    assert!(
        (x0..=x1).any(|x| after.at(x as usize, y0 as usize) != BAND_INK),
        "the outline goes with the release"
    );
}

/// **The clamp is `Village_DrawBand`'s own** — a band dragged below the
/// picture stops at `g_villageTopY + 0x178` and not at the pointer.
#[test]
fn the_drag_selection_box_stops_where_the_original_clamps_it() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;

    let county = 8u8;
    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    let top = vill::SCENE_Y;
    let (x0, y0) = (vill::SCENE_X + 20, top + 40);
    // Past the bottom of the band area, which `Village_BandStart` also refuses
    // to arm in: the pointer keeps going, the outline does not.
    let (x1, y1) = (x0 + 60, top + vill::BAND_H + 10);
    handle(&mut m, &mut game, &assets, Event::Click { x: x0, y: y0 });
    handle(&mut m, &mut game, &assets, Event::Pointer { x: x1, y: y1 });
    let canvas = draw_stack(&mut m, &mut game, &assets);

    const BAND_INK: u8 = 0x20;
    let bottom = top + vill::BAND_H - 1;
    assert_eq!(canvas.at(x0 as usize, bottom as usize), BAND_INK, "the clamped bottom edge");
    assert_ne!(
        canvas.at(x0 as usize, y1 as usize),
        BAND_INK,
        "and nothing at the pointer, ten rows past the band area"
    );
}

// ------------------------------------------------------------ across a season

/// **The split a player drags is the split the season deals him back.**
///
/// `Labour_Allocate` runs twice in every `Season_Advance` and deals the county
/// out from its eight shares; it never writes one. `Labour_Move` ends with
/// `Labour_RecomputeShares`, which does, from where people now stand — so in the
/// original the drag *is* the new split. Ours never rewrote the shares and the
/// season put everybody back: *"I have to reassign peasants to wheat each turn."*
///
/// **Staged, and why.** England turn one has **no grain in any county**, so no
/// field can be sown and the grain ceiling is zero whatever the split says — a
/// county the original would also empty. County 8 is given 2,000 sacks, and its
/// fallow fields are painted wheat through `Kingdom::paint_field`, the brush's
/// own road. Everything after that is the screens.
///
/// The share is typed from `Labour_RecomputeShares` (`FUN_00450000`): `PctOf`
/// of each farm job over the three, the remainder to the largest.
///
/// **Ablation, run:** delete `recompute_shares` (and its twin) from
/// `Kingdom::move_labour` and the shares after the drop are still the painted
/// county's, red at the first assertion below the drop.
#[test]
fn the_split_a_player_drags_is_the_split_the_season_deals_back() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;
    let county = 8u8;
    let id = county as usize;
    game.kingdom.counties[id].grain = 2000;
    let fallow: Vec<usize> = game
        .kingdom
        .field_tiles(id)
        .into_iter()
        .filter(|&(_, t)| t == FieldType::Fallow)
        .map(|(t, _)| t)
        .collect();
    assert!(!fallow.is_empty(), "county 8 has fallow fields to sow");
    for t in fallow {
        game.kingdom.paint_field(id, t, FieldType::Grain).expect("a fallow field takes wheat");
    }
    let painted = game.kingdom.counties[id].labour_share;

    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));
    let (from, to) = (cluster_of(&game, county, 1), cluster_of(&game, county, 0));
    drag_one_icon(&mut m, &mut game, &assets, county, from, to);

    let c = &game.kingdom.counties[id];
    let farm = [c.labour[0], c.labour[1], c.labour[2]];
    let total: i32 = farm.iter().sum();
    let mut expected: Vec<i32> = farm.iter().map(|&w| w * 100 / total).collect();
    let short = 100 - expected.iter().sum::<i32>();
    let largest = (0..3).max_by_key(|&j| (expected[j], std::cmp::Reverse(j))).unwrap();
    expected[largest] += short;
    assert_eq!(
        &c.labour_share[..3],
        &expected[..],
        "the drop rewrites the farm shares from the workers ({farm:?}); they were {painted:?}"
    );
    assert_ne!(&c.labour_share[..3], &painted[..3], "and the drag moved them");
    let dragged = c.labour_share;
    let grain_after_drop = c.labour[0];

    // Out of the village and End Turn, from the keyboard.
    handle(&mut m, &mut game, &assets, Event::KeyDown(Key::Escape));
    assert_eq!(m.ids(), vec![ScreenId::Campaign]);
    end_turn(&mut m, &mut game, &assets);

    let c = &game.kingdom.counties[id];
    assert_eq!(game.kingdom.turn_count, 2, "one season ran");
    assert_eq!(c.labour_share, dragged, "the season deals from the player's split and does not rewrite it");
    assert!(
        c.labour[0] >= grain_after_drop.min(c.labour_useful[0]),
        "the wheat is still staffed after the season: {} workers against {} after the drop, ceiling {}",
        c.labour[0],
        grain_after_drop,
        c.labour_useful[0]
    );
}

