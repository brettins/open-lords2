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
use l2_game::shell::{font, Pen};
use l2_view::Canvas;
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
/// rather than around it.
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
        // whole world now, so a new game needs no save at all — which is the
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

/// **The build stamp identifies a build, and this is the assertion that stops
/// it decaying back into a version string.**
///
/// It exists because three of the last five interface defects a player reported
/// were against a binary four merges old — two already fixed, one fixed twice —
/// and nothing on screen could have told him. The failure mode this guards is
/// not the stamp going missing; it is somebody replacing it with something that
/// *looks* like an answer. `0.1.0` has been true of every build for two months,
/// so a constant is worse than nothing: it invites the reader to stop asking.
///
/// So the shape is asserted, not the value: nine hex digits, an optional
/// `-DIRTY`, and a date — or the literal `NO GIT` when the source is not a
/// checkout, which is a fact rather than a fabrication. No gate; it needs
/// neither the game nor the fixtures.
#[test]
fn the_build_stamp_names_a_commit_rather_than_a_version() {
    let id = l2_game::build_id::ID;
    assert!(!id.is_empty(), "build.rs always emits something");

    if id == "NO GIT" {
        return; // A tarball build. Honest, and there is nothing else to check.
    }

    let (commit, date) = id.split_once(' ').unwrap_or_else(|| {
        panic!("the stamp is `<commit>[-DIRTY] <date>`, got {id:?}")
    });
    let hex = commit.strip_suffix("-DIRTY").unwrap_or(commit);
    assert_eq!(hex.len(), 9, "nine hex digits of commit, got {hex:?}");
    assert!(
        hex.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)),
        "upper-case hex, so it reads in our caps font: {hex:?}",
    );
    assert!(
        date.len() == 10 && date.split('-').count() == 3,
        "a date, because `is this old?` is the question a hash cannot answer: {date:?}",
    );

    // The point of the whole exercise: two different builds must be able to
    // disagree. A constant cannot, and this is the shape a constant has.
    assert_ne!(id, env!("CARGO_PKG_VERSION"), "a version is not an identity");
}

/// **The build stamp is actually painted, not merely computed.**
///
/// The test above asserts the shape of the string. This one asserts a player can
/// see it, which is a different claim and the one that matters: the whole point
/// is that somebody looking at a screenshot can say which binary it is.
///
/// It is separate rather than folded in because the two fail for unrelated
/// reasons — a wrong string and an unpainted one need different fixes — and
/// because this one is the fragile half. `crate::build_id::draw` is one line at
/// the end of the title page's painter, in a file three branches touched
/// tonight, and a line like that is exactly what a merge drops without anything
/// noticing.
///
/// **How it asserts, and the first attempt was wrong.** The obvious test — count
/// non-background pixels in the stamp's band — *passed with the draw line
/// deleted*, because the title page carries a full-screen `gateway.pl8` and no
/// pixel down there is background. It measured the artwork.
///
/// So: draw the page, copy it, draw the stamp again onto the copy, and require
/// the two to be **identical**. Text is an opaque blit, so drawing it a second
/// time over itself changes nothing — but only if it was there the first time.
/// Deleting the line makes the second draw *add* the stamp, and the canvases
/// differ. That is an exact test rather than a threshold, and it needs no
/// knowledge of what else is on the page.
#[test]
fn the_build_stamp_is_painted_on_the_title_page() {
    let (mut game, assets) = world!();
    let mut screen = SetupScreen::new(SetupPage::Title);

    let mut page = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut page);
    }

    let mut twice = page.clone();
    let pen = Pen {
        assets: &assets.shell,
        ink: &assets.ink,
        chrome: assets.chrome.as_ref(),
        shadow: Some(font::SHADOW_GATEWAY),
        caps: Some(1),
    };
    l2_game::build_id::draw(&mut twice, &pen);

    let differing = (0..l2_view::canvas::HEIGHT)
        .flat_map(|y| (0..l2_view::canvas::WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| page.at(x, y) != twice.at(x, y))
        .count();

    assert_eq!(
        differing, 0,
        "drawing the build stamp again changed {differing} pixels, so it was not on the page \
         to begin with. `crate::build_id::draw` is one line at the end of the title painter \
         in screens/setup.rs, and a line like that is what a merge drops silently — which is \
         the whole reason the stamp exists.",
    );
}
