//! **Pick Ireland, and play Ireland.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test newgame
//! ```
//!
//! The setup screen has been able to *name* a map since its list was drawn.
//! What it could not do was start one: whatever the list said, the world came
//! out of `lastturn.sav` and it was England. This file is the check on the
//! other half — that the slot the list highlights is the world the campaign
//! screen opens on, and that a turn runs in it.
//!
//! **Nothing here loads a save.** Every game starts from `Game::new`, which is
//! an empty world, so a test that passed by inheriting the fixture's England
//! would have nothing to inherit.
//!
//! Everything is driven through [`Screen::handle`] with real pointer
//! coordinates read out of the geometry tables, for the reason
//! `tests/setup.rs` gives: a test that calls a method the interface does not
//! reach proves nothing about the interface.

use std::path::PathBuf;

use l2_formats::maps::MapSet;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::setup::{
    SetupPage, SetupScreen, CUSTOM_BUTTONS, CUSTOM_BUTTON_Y, MAP_LIST_ROW, MAP_LIST_ROWS,
    MAP_LIST_X, MAP_LIST_Y,
};
use l2_game::{turn, Game};
use l2_kingdom::realm::MAX_REALMS;

/// `L2.eng` group 101's first five names, which are the first five slots.
/// Ireland is slot 2, and it is on the list's first page, so choosing it is one
/// click.
const IRELAND: usize = 2;
const SCOTLAND: usize = 1;
const ENGLAND: usize = 0;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! assets {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so no L2_maps.dat and no L2.eng");
        };
        let platform =
            l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        Assets::load(&platform.vfs).expect("assets load")
    }};
}

fn click(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x, y }, &mut ctx)
}

/// Click row `row` of the map list, at the coordinates `FUN_00433905`'s hit
/// test uses.
fn pick_map(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, row: usize) {
    assert!(row < MAP_LIST_ROWS);
    let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW + MAP_LIST_ROW / 2;
    click(screen, game, assets, MAP_LIST_X + 20, y);
}

/// *Start* — the second of page 7's three captions.
fn press_start(screen: &mut SetupScreen, game: &mut Game, assets: &Assets) -> Transition {
    click(screen, game, assets, CUSTOM_BUTTONS[1].0 + 20, CUSTOM_BUTTON_Y)
}

/// Open the custom page and let its first tick read the map, as the machine
/// does.
fn open(assets: &Assets, game: &mut Game) -> SetupScreen {
    let mut screen = SetupScreen::new(SetupPage::Custom);
    let mut ctx = Ctx { game, assets };
    screen.update(&mut ctx);
    screen
}

/// How many counties a slot has, straight out of the file — a second reading
/// of the number the world builder produces.
fn counties_in(assets: &Assets, slot: usize) -> usize {
    assets.slot(slot).expect("the slot").county_count()
}

// ---------------------------------------------------------------- the headline

/// **A person picks Ireland and gets Ireland.**
#[test]
fn choosing_ireland_starts_ireland() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let mut screen = open(&assets, &mut game);

    pick_map(&mut screen, &mut game, &assets, IRELAND);
    assert_eq!(screen.map(), IRELAND, "the list did not take the click");

    let t = press_start(&mut screen, &mut game, &assets);
    assert_eq!(t, Transition::Push(ScreenId::Campaign));

    // `g_scenarioIndex`, which is what the campaign painter and the minimap
    // read to know which artwork to draw.
    assert_eq!(game.map_slot, IRELAND);

    // The world is Ireland's, and the county count is the check that says so:
    // Ireland and England have different ones, so a game that had quietly
    // started England would fail here rather than pass silently.
    let ireland = counties_in(&assets, IRELAND);
    let england = counties_in(&assets, ENGLAND);
    assert_ne!(ireland, england, "the two maps must differ or this proves nothing");
    assert_eq!(game.kingdom.county_count, ireland, "the world is not Ireland's");

    // …and the tiles are Ireland's, not merely the right number of counties.
    // The county plane is copied verbatim by the loader, so it is exact.
    let bytes = l2_testkit::read_install("L2_maps.dat").expect("the install has it");
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let slot = set.slot(IRELAND).unwrap();
    for y in 0..64usize {
        for x in 0..64usize {
            assert_eq!(
                game.kingdom.campaign.map.county[y * 64 + x],
                slot.county_at(x, y),
                "tile ({x}, {y})"
            );
        }
    }
}

/// **And it is a game, not a diorama.** A turn runs on the world the map
/// built: the season advances, every realm takes its turn, and nothing panics
/// on a world nothing has ever loaded from a save.
#[test]
fn a_turn_runs_on_a_world_built_from_a_map_file() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let mut screen = open(&assets, &mut game);
    pick_map(&mut screen, &mut game, &assets, IRELAND);
    press_start(&mut screen, &mut game, &assets);

    // `Game_NewGame` ends with one `Season_Advance`, so the game a person is
    // handed is already Winter 1268 — the original's own first screen.
    assert_eq!(game.kingdom.season, 4, "Winter");
    assert_eq!(game.kingdom.year, 1268);
    assert_eq!(game.kingdom.turn_count, 1);

    // The person holds exactly one county and is standing on it.
    let player = game.player as usize;
    let mine: Vec<usize> = game
        .kingdom
        .county_ids()
        .filter(|&id| game.kingdom.counties[id].owner == player as u8)
        .collect();
    assert_eq!(mine.len(), 1, "one county each at the start");
    assert_eq!(game.selected as usize, mine[0], "the game opens on it");
    assert!(game.kingdom.realms[player].is_human);
    assert!(game.kingdom.realms[player].gold > 0, "a treasury");

    // Somebody to play against.
    let realms = (1..MAX_REALMS).filter(|&i| game.kingdom.realms[i].in_play).count();
    assert_eq!(realms, 5, "five nobles is the default");

    let year_before = game.kingdom.year;
    let season_before = game.kingdom.season;
    let outcome = turn::end_turn(&mut game).expect("the turn finished");
    let _ = outcome;
    assert_eq!(game.kingdom.turn_count, 2, "the turn counter moved");
    assert!(
        game.kingdom.season != season_before || game.kingdom.year != year_before,
        "the clock did not move"
    );
    // The economy ran: a county that farms has fields and people in them.
    let c = &game.kingdom.counties[mine[0]];
    assert!(c.field_tiles.iter().any(|&t| t != 0), "the county has no fields");
    assert!(
        c.labour.iter().sum::<i32>() > 0,
        "nobody is working: Labour_Allocate never reached this world"
    );
}

/// **Two different maps are two different worlds**, and the second one does not
/// inherit the first.
///
/// The failure this catches is the one the feature replaced: a *Start* that
/// changed the settings and left the world alone would give both games the same
/// map and pass every assertion about the settings.
#[test]
fn two_maps_started_in_one_session_are_two_worlds() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);

    let mut screen = open(&assets, &mut game);
    pick_map(&mut screen, &mut game, &assets, SCOTLAND);
    press_start(&mut screen, &mut game, &assets);
    let scotland = (game.map_slot, game.kingdom.county_count, game.kingdom.campaign.map.clone());

    // Back to the front end and start again, in the same process, on a
    // different map.
    let mut screen = open(&assets, &mut game);
    pick_map(&mut screen, &mut game, &assets, IRELAND);
    press_start(&mut screen, &mut game, &assets);
    let ireland = (game.map_slot, game.kingdom.county_count, game.kingdom.campaign.map.clone());

    assert_ne!(scotland.0, ireland.0, "the slot");
    assert_ne!(scotland.2, ireland.2, "the tiles");
    assert_eq!(scotland.1, counties_in(&assets, SCOTLAND));
    assert_eq!(ireland.1, counties_in(&assets, IRELAND));
}

/// **Every shipped map starts and survives a turn.**
///
/// `docs/plan.md` C26 again: England is one input of forty-four. A map that
/// builds a world nothing can take a turn in is a map the list should not
/// offer, and there is no other way to find out which those are.
#[test]
fn every_shipped_map_starts_and_takes_a_turn() {
    let assets = assets!();
    let bytes = l2_testkit::read_install("L2_maps.dat").expect("the install has it");
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let mut started = 0;
    for slot in set.used_slots() {
        // The list only shows the sixty named slots, so that is what a person
        // can reach.
        if slot >= 60 {
            continue;
        }
        let seats = set.slot(slot).unwrap().player_start_count();
        let mut game = Game::new(scenario::SEED);
        let settings =
            l2_game::setup::SetupOptions::new().commit(1);
        // Fewer lords than the map seats, so the seat guard never fires; the
        // screen's own drop-down does this for a person.
        let lords = seats.min(5).max(1);
        let settings = l2_game::setup::Settings { ai_lords: lords as i32 - 1, ..settings };
        let built = scenario::new_game(&assets, slot, &settings, 1, scenario::SEED, game.kingdom.tables)
            .unwrap_or_else(|e| panic!("slot {slot}: {e}"));
        game = built;
        settings.apply_to(&mut game);
        game.kingdom.start_new_game();
        assert_eq!(game.kingdom.county_count, counties_in(&assets, slot), "slot {slot}");
        let owned = game
            .kingdom
            .county_ids()
            .filter(|&id| game.kingdom.counties[id].owner != 0)
            .count();
        assert_eq!(owned, lords, "slot {slot}: {owned} owned counties for {lords} lords");
        turn::end_turn(&mut game).unwrap_or_else(|| panic!("slot {slot}: the turn stopped to ask"));
        assert_eq!(game.kingdom.turn_count, 2, "slot {slot}");
        started += 1;
    }
    eprintln!("{started} maps started and took a turn");
    assert_eq!(started, 44);
}
