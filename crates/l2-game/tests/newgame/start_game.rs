#![allow(unused_imports)]
use super::*;
use super::campaign::*;
use super::heraldry::*;
use super::army_size::*;
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

/// **`Game_NewGame`'s own `Save_RotateAndWrite()` (`0x00497E2B`)** — the second
/// of that function's only two call sites, the other being the turn boundary.
///
/// It is why a played install's `lastturn.sav` reads turn 1, Winter 1268 after
/// a new game and before any End Turn, and why the player's first End Turn
/// leaves a `old_turn.sav` to go back to. Raised, not performed:
/// [`l2_game::saves::run_pending`] is the only thing that writes a file, and
/// nothing in the simulation calls it.
///
/// Ablations: delete the `self.autosave = true` in `SetupScreen::new_game` and
/// nothing asks; move it above the `Err` arm and a *Start* that could not build
/// a world asks anyway.
#[test]
fn starting_a_game_asks_for_an_autosave_and_a_start_that_failed_does_not() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let mut screen = open(&assets, &mut game);
    assert!(!l2_game::Screen::take_autosave(&mut screen), "the page itself does not autosave");

    let t = press_start(&mut screen, &mut game, &assets);
    assert_eq!(t, Transition::Push(ScreenId::Campaign), "Start did nothing");
    assert!(
        l2_game::Screen::take_autosave(&mut screen),
        "Game_NewGame ends in Save_RotateAndWrite"
    );
    assert!(!l2_game::Screen::take_autosave(&mut screen), "and it is taken once");
}

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

    // `g_scenarioIndex`
    // read to know which artwork to draw.
    assert_eq!(game.map_slot, IRELAND);

    // The world is Ireland's, and the county count is the check that says so:
    // Ireland and England have different ones, so a game that had quietly
    // started England would fail here.
    let ireland = counties_in(&assets, IRELAND);
    let england = counties_in(&assets, ENGLAND);
    assert_ne!(ireland, england, "the two maps must differ or this proves nothing");
    assert_eq!(game.kingdom.county_count, ireland, "the world is not Ireland's");

    // …and the tiles are Ireland's.
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

/// **Two different maps are two different worlds**
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
/// offer.
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
            l2_game::setup::SetupOptions::new().commit(1, l2_kingdom::Quirks::default());
        // Fewer lords than the map seats, so the seat guard never fires; the
        // screen's own drop-down does this for a person.
        let lords = seats.min(5).max(1);
        let settings = l2_game::setup::Settings { ai_lords: lords as i32 - 1, ..settings };
        // A different colour on each map, so that every one of §7a's five rows
        // is built at least eight times over the forty-four.
        let shield = (slot % 5 + 1) as u8;
        let tables = game.kingdom.tables;
        let built = scenario::new_game(&assets, slot, &settings, 1, shield, scenario::SEED, tables)
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

// ------------------------------------------------------- the campaign's map

