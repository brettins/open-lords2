#![allow(unused_imports)]
use super::*;
use super::title_tests::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::setup::{
    SetupPage, SetupScreen, MAP_LIST_ROW, MAP_LIST_ROWS, MAP_LIST_X, MAP_LIST_Y, OPTION_CELLS,
    OPTION_LIST,
};
use l2_game::setup::{self, option, SetupOptions, COUNTY_STATUS, STARTING_GOLD, START_ARMOURY};
use l2_game::Game;
use l2_game::shell::{font, Pen};
use l2_view::Canvas;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;

#[test]
fn start_carries_all_twelve_settings_into_the_game() {
    let (mut game, assets) = world!();
    let mut screen = SetupScreen::new(SetupPage::Custom);
    tick(&mut screen, &mut game, &assets);

    // Deliberately not the defaults, and every one of them different from what
    // the England fixture carries: difficulty 0, no advanced farming, armies
    // do not eat.
    choose(&mut screen, &mut game, &assets, option::ADVANCED_FARMING, 1); // on
    choose(&mut screen, &mut game, &assets, option::EXPLORATION, 1); // on
    choose(&mut screen, &mut game, &assets, option::ARMIES_EAT, 1); // yes
    choose(&mut screen, &mut game, &assets, option::DIFFICULTY, 2); // hard
    choose(&mut screen, &mut game, &assets, option::WEAPONS, 3); // many
    choose(&mut screen, &mut game, &assets, option::CROWNS, 4); // 5000
    choose(&mut screen, &mut game, &assets, option::COUNTY_STATUS, 2); // strong
    choose(&mut screen, &mut game, &assets, option::TIME_LIMIT, 3); // 4 mins
    choose(&mut screen, &mut game, &assets, option::STARTING_CASTLE, 5); // royal
    choose(&mut screen, &mut game, &assets, option::FIGHT, 0); // humans

    // Nothing has reached the world yet: the screen holds the selections and
    // the game is still the one the save described.
    assert!(!game.kingdom.options.advanced_farming, "not until Start");

    let t = press_start(&mut screen, &mut game, &assets);
    assert_eq!(t, Transition::Push(ScreenId::Campaign));

    // The six that are rules.
    let o = game.kingdom.options;
    assert!(o.advanced_farming);
    assert!(o.exploration);
    assert!(o.armies_eat);
    assert_eq!(o.difficulty, 2);
    assert_eq!(o.fight_humans_only_byte, 0, "index 0 is \"humans\", and the byte is inverted");
    assert_eq!(o.time_limit, 240, "\"4 mins\" is 240 seconds, not the index 3");

    // The six that are starting conditions.
    let player = game.player as usize;
    assert_eq!(game.kingdom.realms[player].gold, STARTING_GOLD[4]);
    assert_eq!(game.kingdom.realms[player].weapons, START_ARMOURY[3]);
    assert_eq!(game.kingdom.realms[player].iron, setup::STARTING_MATERIALS);
    // **The stores are read one season later than they used to be**, because
    // *Start* now does what `Game_NewGame` does and runs the first
    // `Season_Advance` before handing the world over — a new game begins in
    // Winter 1268, not in the Autumn 1267 the county-status row is written
    // into. So the row is checked where the season leaves it visible:
    // `pop_last` is the population the pass started from, which is the row's.
    for id in game.kingdom.county_ids() {
        let c = &game.kingdom.counties[id];
        assert_eq!(c.pop_last, COUNTY_STATUS[2].population, "county {id}");
        if c.owner != 0 {
            assert_eq!(c.castle_type, 5, "a royal castle in county {id}");
        }
    }
    assert_eq!(game.kingdom.season, 4, "Winter");
    assert_eq!(game.kingdom.year, 1268);
    assert_eq!(game.kingdom.turn_count, 1);
}

/// **The *County Status* row reaches the land**, measured through the season
///
///
/// The three rows differ by a factor of eight in the herd and seven in the
/// population, and one season of eating does not close that — so the ordering
/// survives, and asserting the ordering asserts the setting arrived without
/// asserting a number the season is entitled to move.
#[test]
fn the_county_status_row_still_orders_the_counties_after_the_first_season() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install, so no L2_maps.dat and no L2.eng");
    };
    let platform = Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut totals = Vec::new();
    for row in 0..3usize {
        // **From an empty game, not from the fixture.** *Start* replaces the
        // whole world now
        // property this whole commit is about, asserted in passing.
        let mut game = Game::new(scenario::SEED);
        let mut screen = SetupScreen::new(SetupPage::Custom);
        tick(&mut screen, &mut game, &assets);
        choose(&mut screen, &mut game, &assets, option::COUNTY_STATUS, row);
        let t = press_start(&mut screen, &mut game, &assets);
        assert_eq!(t, Transition::Push(ScreenId::Campaign), "row {row} did not start");
        let herd: i32 = game.kingdom.county_ids().map(|id| game.kingdom.counties[id].herd).sum();
        let pop: i32 = game.kingdom.county_ids().map(|id| game.kingdom.counties[id].pop_last).sum();
        totals.push((herd, pop));
    }
    let [weak, medium, strong] = <[(i32, i32); 3]>::try_from(totals).unwrap();
    assert!(weak.0 < medium.0 && medium.0 < strong.0, "the herd: {weak:?} {medium:?} {strong:?}");
    assert!(weak.1 < medium.1 && medium.1 < strong.1, "the population");
    // …and the population really is the row's, fourteen counties of it.
    assert_eq!(strong.1, 14 * COUNTY_STATUS[2].population);
}

#[test]
fn difficulty_reaches_the_ai_armoury_and_not_the_persons() {
    let (mut game, assets) = world!();
    let mut screen = SetupScreen::new(SetupPage::Custom);
    tick(&mut screen, &mut game, &assets);
    choose(&mut screen, &mut game, &assets, option::WEAPONS, 0); // none at all
    choose(&mut screen, &mut game, &assets, option::DIFFICULTY, 3); // impossible
    press_start(&mut screen, &mut game, &assets);

    let extra = 3 * setup::AI_EXTRA_MAIL_PER_DIFFICULTY;
    let slot = setup::AI_EXTRA_WEAPON_SLOT;
    let mut ai = 0;
    for id in 1..MAX_REALMS {
        let r = &game.kingdom.realms[id];
        if !r.in_play {
            continue;
        }
        if r.is_human {
            assert_eq!(r.weapons[slot], 0, "realm {id} is a person and gets nothing");
        } else {
            assert_eq!(r.weapons[slot], extra, "realm {id}");
            ai += 1;
        }
    }
    assert!(ai > 0, "somebody has to be the AI or this proves nothing");
// And the option is "none" — the extra is the difficulty's
    // the table's.
    assert_eq!(START_ARMOURY[0][slot], 0);
}

#[test]
fn fewer_lords_than_the_map_seats_leaves_the_rest_of_the_map_neutral() {
    let (mut game, assets) = world!();
    let mut screen = SetupScreen::new(SetupPage::Custom);
    tick(&mut screen, &mut game, &assets);
    // The England fixture seats five and starts with five realms holding one
    // county each.
    let before = (1..MAX_REALMS).filter(|&i| game.kingdom.realms[i].in_play).count();
    assert_eq!(before, 5, "the fixture has five realms in play");

    choose(&mut screen, &mut game, &assets, option::NOBLES, 0); // two
    assert_eq!(screen.options().lords(), 2);
    press_start(&mut screen, &mut game, &assets);

    let after = (1..MAX_REALMS).filter(|&i| game.kingdom.realms[i].in_play).count();
    assert_eq!(after, 2, "one person and one lord");
    assert!(game.kingdom.realms[game.player as usize].in_play, "the person is not dropped");
// A dropped realm's county goes back to nobody.
    for id in game.kingdom.county_ids() {
        let owner = game.kingdom.counties[id].owner as usize;
        assert!(
            owner == 0 || game.kingdom.realms[owner].in_play,
            "county {id} is owned by a realm that is not in the game",
        );
    }
}

#[test]
fn the_map_list_sets_the_lord_count_from_the_map() {
    let (mut game, assets) = world!();
    let mut screen = SetupScreen::new(SetupPage::Custom);
    tick(&mut screen, &mut game, &assets);
    // Slot 0 is England, and it seats five.
    assert_eq!(screen.map(), 0);
    assert_eq!(screen.player_starts(), 5, "England seats five");
    assert_eq!(screen.options().lords(), 5);

    // Walk the visible rows of the list; whatever each one seats, the lord
    // count follows it, and the drop-down is never longer than the seats.
    for row in 0..MAP_LIST_ROWS {
        let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW + 4;
        click(&mut screen, &mut game, &assets, MAP_LIST_X + 8, y);
        let seats = screen.player_starts();
        assert!((2..=5).contains(&seats), "map {} seats {seats}", screen.map());
        assert_eq!(screen.options().lords(), seats, "map {}", screen.map());
        assert!(
            SetupOptions::nobles_rows_for_map(seats) < seats,
            "map {} seats {seats} but offers {} rows",
            screen.map(),
            SetupOptions::nobles_rows_for_map(seats),
        );
    }
}

/// **Every map in `L2_maps.dat` seats 5, 4 or 2**, which is what
/// `docs/symbols.md` says `g_playerStartCount` comes out as
/// *Nobles* drop-down needs shortening at all.
#[test]
fn the_shipped_maps_seat_five_four_or_two() {
    let (_game, assets) = world!();
    let mut counts = std::collections::BTreeMap::new();
    let mut slots = 0;
    for slot in 0..60 {
        let Some(map) = assets.slot(slot) else { continue };
        if map.is_empty() {
            continue;
        }
        slots += 1;
        let seats = map.player_start_count();
        *counts.entry(seats).or_insert(0usize) += 1;
        assert!(
            (2..=5).contains(&seats),
            "map slot {slot} seats {seats}, which is outside the five-realm array",
        );
    }
    assert!(slots >= 40, "only {slots} used map slots, which is too few to have read the file");
    let kinds: Vec<usize> = counts.keys().copied().collect();
    assert_eq!(kinds, vec![2, 4, 5], "seat counts across the shipped maps: {counts:?}");
}

#[test]
fn the_defaults_button_restores_the_originals_defaults() {
    let (mut game, assets) = world!();
    let mut screen = SetupScreen::new(SetupPage::Custom);
    tick(&mut screen, &mut game, &assets);
    choose(&mut screen, &mut game, &assets, option::CROWNS, 0); // 100
    choose(&mut screen, &mut game, &assets, option::STARTING_CASTLE, 0); // none
    // *Defaults* is the third caption on the row.
    click(&mut screen, &mut game, &assets, 0x141 + 20, 0xC6);
    assert_eq!(screen.options(), &SetupOptions::new());
    let s = screen.options().commit(1, l2_kingdom::Quirks::FAITHFUL);
    assert_eq!(s.gold, 1000, "the default purse, not 100 and not zero");
    assert_eq!(s.castle_type, 3, "a keep");
}

#[test]
fn nothing_is_started_by_looking_at_the_page() {
    let (mut game, assets) = world!();
    let before = game.kingdom.clone();
    let mut screen = SetupScreen::new(SetupPage::Custom);
    tick(&mut screen, &mut game, &assets);
    choose(&mut screen, &mut game, &assets, option::DIFFICULTY, 3);
    choose(&mut screen, &mut game, &assets, option::CROWNS, 0);
    // *Cancel*, not *Start*.
    click(&mut screen, &mut game, &assets, 0xA5 + 20, 0xC6);
    assert_eq!(game.kingdom, before, "only Start may touch the world");
}

