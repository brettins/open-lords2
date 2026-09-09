//! **Pressing *Start* starts the game the screen was showing.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!     cargo test -p l2-game --test setup
//! ```
//!
//! The unit tests in `l2_game::setup` check the *tables* — that each is as long
//! as its drop-down, that the defaults are the original's, that the commit
//! arithmetic is `Setup_CommitOptions`'. This file checks the other half, which
//! is the one that was actually broken: that clicking a value in a drop-down
//! and then clicking *Start* changes the world.
//!
//! Everything is driven through [`Screen::handle`] with real pointer
//! coordinates read out of the geometry tables, so a test cannot pass by
//! calling a method the interface does not reach.

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
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so no L2_maps.dat and no L2.eng");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

/// Click the middle of a rectangle.
fn click(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x, y }, &mut ctx)
}

fn tick(screen: &mut SetupScreen, game: &mut Game, assets: &Assets) {
    let mut ctx = Ctx { game, assets };
    screen.update(&mut ctx);
}

/// Open option `i`'s drop-down and choose row `row`, by clicking, exactly as a
/// person would.
fn choose(
    screen: &mut SetupScreen,
    game: &mut Game,
    assets: &Assets,
    i: usize,
    row: usize,
) {
    let (bx, by, _) = OPTION_CELLS[i];
    click(screen, game, assets, bx + 8, by + 8);
    assert_eq!(screen.page(), SetupPage::Dropdown, "option {i} opened its list");
    // The list's own geometry, and the *Nobles* list rides up one row per item.
    let (lx, mut ly, _) = OPTION_LIST[i];
    if i == option::NOBLES {
        let rows = SetupOptions::nobles_rows_for_map(screen.player_starts());
        ly -= (rows as i32 - 1) * 16;
    }
    click(screen, game, assets, lx + 8, ly + 16 + row as i32 * 16 + 8);
    assert_eq!(screen.page(), SetupPage::Custom, "and closed again");
    assert_eq!(screen.options().get(i), row, "option {i} took row {row}");
}

/// The *Start* button on page 7 — the third caption at y = 0xC6.
fn press_start(screen: &mut SetupScreen, game: &mut Game, assets: &Assets) -> Transition {
    click(screen, game, assets, 0xF3 + 20, 0xC6)
}

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
    for id in game.kingdom.county_ids() {
        let c = &game.kingdom.counties[id];
        assert_eq!(c.herd, COUNTY_STATUS[2].herd, "county {id}");
        assert_eq!(c.population, COUNTY_STATUS[2].population, "county {id}");
        if c.owner != 0 {
            assert_eq!(c.castle_type, 5, "a royal castle in county {id}");
            assert_eq!(c.grain, COUNTY_STATUS[2].grain);
        } else {
            // The one place setup treats a neutral county differently.
            assert_eq!(c.grain, COUNTY_STATUS[2].grain + setup::UNOWNED_COUNTY_GRAIN_BONUS);
        }
    }
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
    // And the option is genuinely "none" — the extra is the difficulty's, not
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
    // A dropped realm's county goes back to nobody rather than to somebody else.
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
/// `docs/symbols.md` says `g_playerStartCount` comes out as, and the reason the
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
    let s = screen.options().commit(1);
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
