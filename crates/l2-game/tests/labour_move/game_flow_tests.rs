#![allow(unused_imports)]
use super::*;
use super::drag_and_drop_tests::*;
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

// --------------------------------------------------------------- a new game

/// **A new England opens with its start counties staffed, forecasting, and
/// with the mines the original leaves off, off.**
///
/// Through the setup page: England, *Start*. Three claims, each against the
/// original.
///
/// 1. **The switches are `Game_SetupRealmsAndCounties`' loop** (`0x0049BD99`):
///    `for (i = 0; i < 4; i++) if (i != 2 && hasResource[i]) { enabled[i] = 1;
///    break; }` — one industry per start county, the first of wood, iron and
///    stone, never the smithy and never a mine beside a forest. The map shows
///    the same byte: every site is `base + enabled`.
/// 2. **The person's county is `england-turn1.sav`'s person's county**, job for
///    job, forecast for forecast. That save is the original's own turn one. Its
///    seat is rolled, so the counties differ; the start row does not, and every
///    number below came out equal. Before this change ours had **no foresters,
///    155 idle, and all four forecasts zero**, because nothing had computed an
///    industry ceiling before the opening season's `Labour_AllocateAll` —
/// `Industry_ProduceAll`'s estimates were not ported
///    herd went into `Herd_SeasonTick` unminded, because
///    `Game_SetupRealmsAndCounties`' two allocation rounds were not either.
/// 3. **The wood row draws it** on the first frame of the campaign.
///
/// 4. **Every county's herd is the save's** — owned *and* unowned, as the
///    sorted multiset `(cattle ceiling, pasture, crowding, herd, people)`,
///    because the seats are rolled. `FUN_0049BD99`'s first loop closes on
///    `County_RecountFields(c); Herd_UpdateCrowding(c)` (`0x00469B8D`,
/// `0x0044D913`) and ours ran neither,
///    opening crowding 10 into the opening `Herd_SeasonTick` and bred at the
///    low-crowding rate: the person's came out 109 against the save's 101.
///    Only the AI's were right, and by accident — `Ai_ManageFarmsAll` calls
///    `Herd_UpdateCrowding` for its own counties.
///
/// **The unowned counties used to come out at 44 head against the save's 67**,
/// because they entered the tick with no cattle labour at all. What staffs them
/// is `County_Reset` (`0x00451150`), whose per-county tail —
/// `County_RecountFields; Herd_UpdateCrowding; Herd_LabourEstimate;
/// Field_ReclaimEstimate; Labour_Allocate; Ration_Apply` — runs on **every**
/// county before the seating and was not reproduced;
/// `Game_SetupRealmsAndCounties` then allocates the *start* counties only, and
/// `AI_ManageFields(0)` (`0x0049DFC6`) does not reach the rest until turn phase
/// 1, after the opening season. [`Kingdom::reset_county_for_new_game`].
///
/// **The last allocation's split was out too** — the nine unowned counties on
/// 342 milkmaids and 114 idle against the save's 323 and 133 — because
/// `Pass::LabourAllocateAgain` dealt against a cattle ceiling one pass out of
/// date. `Army_RecountCountyTroops` (`0x004AD6C0`) is the missing pass: its tail
/// is `Ration_Apply(c, g_seasonNext); Grain_LabourEstimate; Herd_LabourEstimate`
/// over every county, which re-prices the season against the herd that survived
/// `Herd_SeasonTick` (95 head in, 67 out, 13 eaten) and drops the ceiling from
/// 386 to the save's 323. [`Pass::ArmyRecountTroops`].
///
/// **Ablations, four run.** Delete the `settle_start_county` loop from
/// `Settings::apply_to`: red at claim 2 (280 cattle, 47 idle). Empty the
/// estimates in `Kingdom::industry`: red at claim 2 (no foresters). Delete the
/// `herd_update_crowding` call from `Settings::apply_to`: red at claim 4, our
/// person's county 109 against the save's 101. Delete the
/// `reset_county_for_new_game` loop from `Settings::apply_to`: red at claim 4,
/// the nine unowned counties on 44 head against 67.
#[test]
fn a_new_england_opens_staffed_and_forecasting_as_the_originals_turn_one() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let oracle = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    let mut game = Game::new(scenario::SEED);
    let mut setup = SetupScreen::new(SetupPage::Custom);
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        setup.update(&mut ctx);
    }
    // England is row 0 of the map list.
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        setup.handle(Event::Click { x: MAP_LIST_X + 20, y: MAP_LIST_Y + MAP_LIST_ROW / 2 }, &mut ctx);
        assert_eq!(setup.map(), 0, "England");
        setup.handle(Event::Click { x: CUSTOM_BUTTONS[1].0 + 20, y: CUSTOM_BUTTON_Y }, &mut ctx);
    }
    game.prefs.tip_screens = false;
    assert_eq!(game.kingdom.turn_count, 1, "Start ran the opening season");
    assert_eq!(
        game.kingdom.options.advanced_farming, oracle.kingdom.options.advanced_farming,
        "the setup page's defaults are the fixture's"
    );
    assert!(!game.kingdom.options.advanced_farming);

    // 1 — the switches, the rule and the picture.
    let mut owned = 0;
    for id in game.kingdom.county_ids() {
        let c = &game.kingdom.counties[id];
        if c.owner == 0 {
            continue;
        }
        owned += 1;
        let first = [0usize, 1, 3].into_iter().find(|&i| c.industry[i].has_resource);
        for i in 0..4 {
            assert_eq!(c.industry[i].enabled, Some(i) == first, "county {id} industry {i}: the start switch");
            if let Some(site) = l2_kingdom::map::industry_site(&game.kingdom.campaign.map, id as u8, Commodity::ALL[i]) {
                assert_eq!(
                    game.kingdom.campaign.map.terrain[site],
                    SITE_BASE[i] + u8::from(c.industry[i].enabled),
                    "county {id} industry {i}: the map shows the switch the simulation reads"
                );
            }
        }
    }
    assert_eq!(owned, 5);

    // 2 — the person's county, against the original's person's county.
    let theirs = oracle
        .kingdom
        .county_ids()
        .find(|&id| oracle.kingdom.counties[id].owner == oracle.player)
        .expect("the save seats the person");
    let ours = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .expect("the new game seats the person");
    let (t, o) = (&oracle.kingdom.counties[theirs], &game.kingdom.counties[ours]);
    assert_eq!(o.labour, t.labour, "the person's jobs: ours county {ours}, the original's county {theirs}");
    assert_eq!(o.industry_share, t.industry_share, "the farm/industry split");
    assert_eq!(o.labour_useful, t.labour_useful, "the ceilings");
    let next = |c: &l2_kingdom::county::County| c.industry.iter().map(|i| i.next_season).collect::<Vec<_>>();
    assert_eq!(next(o), next(t), "the four industry forecasts");
    assert_eq!(o.herd_change_expected, t.herd_change_expected, "the cattle forecast");
    assert_eq!(o.grain_change_expected, t.grain_change_expected, "the grain forecast");
    assert_eq!(o.herd, t.herd, "the person's herd after the opening season");

    // 4 — the cattle of **every** county, owned or not, as a sorted multiset
    // because the seats are rolled.
    let cattle = |k: &l2_kingdom::Kingdom| {
        let mut rows: Vec<(i32, i32, i32, i32, i32)> = k
            .county_ids()
            .map(|id| {
                let c = &k.counties[id];
                let job = k.tables.job.cattle_farming;
                (c.labour_useful[job], c.fields_cattle, c.herd_crowding, c.herd, c.population)
            })
            .collect();
        rows.sort_unstable();
        rows
    };
    let mine = cattle(&game.kingdom);
    assert_eq!(mine.len(), oracle.kingdom.county_ids().count(), "England's counties, owned and not");
    assert_eq!(mine, cattle(&oracle.kingdom), "(cattle ceiling, pasture, crowding, herd, people)");

    // and the milkmaids and the idle with them, for **every** county.
    let staffed = |k: &l2_kingdom::Kingdom| {
        let mut rows: Vec<(i32, i32)> = k
            .county_ids()
            .map(|id| {
                let c = &k.counties[id];
                (c.labour[k.tables.job.cattle_farming], c.labour[l2_kingdom::tables::JOB_IDLE_TOWNSFOLK])
            })
            .collect();
        rows.sort_unstable();
        rows
    };
    assert_eq!(staffed(&game.kingdom), staffed(&oracle.kingdom), "(milkmaids, idle) every county");

    // 3 — drawn.
    let wood = t.industry[Commodity::Wood.index()].next_season;
    assert!(wood > 0, "the original's person forecasts wood on turn one");
    game.select(ours as u8);
    let mut m = Machine::new(ScreenId::Campaign);
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert!(drawn_in_row(&canvas, &assets, wood, 0, 1), "the wood row draws the original's +{wood}");
}

// ------------------------------------------------------------ the switch

/// **Switching a smithy on redraws every other smithy of the realm.**
///
/// `Industry_ToggleFromMap` (`0x0043D309`) ends with `FUN_00448648(owner)`,
/// `Industry_LabourEstimate(c, weapons)` over every county the realm holds —
/// the Readme's *"turning a blacksmith on will reduce the resources available
/// to other blacksmiths"*, visible the moment you click. Ours refreshed the
/// clicked county only, so the other smithy kept a ceiling and a forecast
/// computed from a stockpile it no longer had to itself.
///
/// `siege-lastturn.sav`: the person holds counties 1, 2 and 3 with every smithy
/// off and 50 iron. Press county 2's smithy on the map, then county 1's.
/// The claim is **idempotence**: after the second press, county 2's weapons
/// figures are already what a fresh `Industry_LabourEstimate` gives — and they
/// are not what they were after the first, so the claim is not vacuous.
///
/// **What moves on this save is the ceiling, not the forecast**, measured:
/// county 2's useful blacksmiths halve from 180 to 90 and its forecast is 4
/// either side, because four is what its own staffing makes before either
/// quota binds. The ceiling is what `FUN_004106C4` draws the row's ringed
/// frame from (`useful < workers`), and what the season's allocation obeys. The
/// forecast is still checked on the canvas.
///
/// **Ablation, run:** delete `self.refresh_blacksmiths(owner)` from
/// `Kingdom::toggle_industry` — red at the idempotence assertion, `(90, 4)`
/// fresh against `(180, 4)` stale.
#[test]
fn switching_one_smithy_on_redraws_the_realms_other_smithy() {
    let assets = assets!();
    let save = l2_testkit::fixture!("siege-lastturn.sav");
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;
    let player = game.player;
    for id in [1usize, 2, 3] {
        assert_eq!(game.kingdom.counties[id].owner, player, "county {id} is the person's");
        assert!(!game.kingdom.counties[id].industry[2].enabled, "county {id}'s smithy starts off");
    }

    let press_smithy = |game: &mut Game, county: u8| {
        let tile = l2_kingdom::map::industry_site(&game.kingdom.campaign.map, county, Commodity::Weapons)
            .expect("every county has a smithy");
        let (tx, ty) = l2_kingdom::map::coords(tile);
        let mut screen = MapScreen::new();
        screen.centre_on_tile(tx as usize, ty as usize);
        let mut canvas = Canvas::screen();
        {
            let ctx = Ctx { game: &mut *game, assets: &assets };
            screen.draw(&ctx, &mut canvas);
        }
        let (row, col) = campaign::tile_to_cell(tx as usize, ty as usize);
        let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
        let z = screen.zoom();
        let (x, y) = (sx + z.tile_w / 2, sy + z.tile_h / 2);
        let mut ctx = Ctx { game: &mut *game, assets: &assets };
        screen.handle(Event::Click { x, y }, &mut ctx);
    };

    press_smithy(&mut game, 2);
    assert!(game.kingdom.counties[2].industry[2].enabled, "the press switched county 2's smithy on");
    let first = (game.kingdom.counties[2].labour_useful[7], game.kingdom.counties[2].industry[2].next_season);

    press_smithy(&mut game, 1);
    assert!(game.kingdom.counties[1].industry[2].enabled, "and county 1's");
    let c2 = &game.kingdom.counties[2];
    let now = (c2.labour_useful[7], c2.industry[2].next_season);

    let mut fresh = c2.clone();
    let k = &game.kingdom;
    let share = l2_kingdom::industry::weapon_shares(&k.tables, &k.counties, k.county_count, player);
    l2_kingdom::industry::refresh(
        &k.tables,
        &mut fresh,
        Commodity::Weapons,
        &k.realms[player as usize],
        share,
        k.options.advanced_farming,
    );
    assert_eq!(
        (fresh.labour_useful[7], fresh.industry[2].next_season),
        now,
        "county 2's smithy is not current after county 1's was switched on"
    );
    assert_ne!(now, first, "county 1's smithy took nothing from county 2's, so this proves nothing");

    // Drawn, on county 2's sidebar.
    game.select(2);
    let rows = county_screen::industry_rows(&game.kingdom.counties[2]);
    let row = rows.iter().position(|&s| s == 7).expect("the smithy row is listed");
    let mut m = Machine::new(ScreenId::Campaign);
    let canvas = draw_stack(&mut m, &mut game, &assets);
    if now.1 > 0 {
        assert!(drawn_in_row(&canvas, &assets, now.1, row, rows.len()), "the smithy row draws +{}", now.1);
    }
    if first.1 > 0 && first.1 != now.1 {
        assert!(!drawn_in_row(&canvas, &assets, first.1, row, rows.len()), "and not the stale +{}", first.1);
    }
}

