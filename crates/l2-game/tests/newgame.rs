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

/// **The original campaign's first map is Quaintville, slot 17.**
///
/// Not slot 0, which is where the map list sits when nobody has touched it, and
/// not whatever `lastturn.sav` happens to hold. `Campaign_LoadEntry`
/// (`0x00499E5D`) reads column `+0x00` of row `g_campaignMap` of
/// `g_campaignTableA` (`0x004D8E18`), and that row is 17. Two independent
/// readings agree: the table in the executable, and an autosave the *original*
/// wrote eight turns into a campaign, which carries `g_scenarioIndex` 17 and
/// four counties.
const QUAINTVILLE: usize = 17;
/// The second campaign opens on Australia — `g_campaignTableB` row **2**, which
/// is where `Setup_ChooseCampaign` starts that track's counter.
const AUSTRALIA: usize = 52;

/// Walk the front end the way a person does: *Single player*, *Play Now!*, one
/// of the two campaigns, then *Continue*. Every click is a real coordinate out
/// of the geometry tables.
fn walk_to_campaign(
    assets: &Assets,
    game: &mut Game,
    right_hand_campaign: bool,
) -> (SetupScreen, Transition) {
    use l2_game::screens::setup::{item_rect, ITEM_H, PAIR_W, PAIR_X, PAIR_Y, SHIELD_BUTTONS};

    let mut screen = SetupScreen::new(SetupPage::Title);
    {
        let mut ctx = Ctx { game, assets };
        screen.update(&mut ctx);
    }
    // Page 1, item 0 — "Single player".
    let r = item_rect(0);
    click(&mut screen, game, assets, r.x + 20, r.y + r.h / 2);
    assert_eq!(screen.page(), SetupPage::Options, "Single player opens page 2");

    // Page 2, item 0 — "Play Now!", which is the campaign chooser.
    let r = item_rect(0);
    click(&mut screen, game, assets, r.x + 20, r.y + r.h / 2);
    assert_eq!(screen.page(), SetupPage::Campaign, "Play Now! opens page 5");

    // Page 5, one of the two 164x24 recesses.
    let i = usize::from(right_hand_campaign);
    click(&mut screen, game, assets, PAIR_X[i] + PAIR_W / 2, PAIR_Y + ITEM_H / 2);
    assert_eq!(screen.page(), SetupPage::Shield, "choosing a campaign opens page 4");

    // Page 4, "Continue" — the second of the two buttons.
    let (x, y, _) = SHIELD_BUTTONS[1];
    let t = click(&mut screen, game, assets, x + 20, y + ITEM_H / 2);
    (screen, t)
}

/// **The bug, as the player reported it: the original campaign puts you on its
/// first map.**
///
/// The assertion that matters is the second one, and it is written against the
/// *game's own* string table rather than against our copy of the campaign
/// table: whatever slot the campaign started, `L2.eng` group 101 must call it
/// *Quaintville*. A test that computed the expected slot from
/// `victory::TRACK_FIRST` would agree with that table however wrong it was.
#[test]
fn the_original_campaign_starts_on_quaintville() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let (_screen, t) = walk_to_campaign(&assets, &mut game, false);
    assert_eq!(t, Transition::Push(ScreenId::Campaign), "Continue did not start a game");

    assert_eq!(game.map_slot, QUAINTVILLE, "the campaign did not start on its first map");
    assert_eq!(
        assets.shell.text(l2_game::screens::setup::GROUP_MAPS, game.map_slot),
        "Quaintville",
        "L2.eng group 101 does not call the started slot Quaintville"
    );

    // …and the world really is that map, not merely the label. Quaintville has
    // four counties and England — slot 0, where the map list sits untouched and
    // where this used to land — has fourteen, so the two cannot be confused.
    let quaintville = counties_in(&assets, QUAINTVILLE);
    assert_eq!(quaintville, 4, "Quaintville is a four-county map");
    assert_ne!(quaintville, counties_in(&assets, ENGLAND), "or this proves nothing");
    assert_eq!(game.kingdom.county_count, quaintville, "the world is not Quaintville's");

    // Tile for tile, from a second reading of the file.
    let bytes = l2_testkit::read_install("L2_maps.dat").expect("the install has it");
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let slot = set.slot(QUAINTVILLE).unwrap();
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

/// **The campaign row is the settings too, not just the map.**
///
/// `Campaign_LoadEntry` writes the eight committed globals straight over
/// whatever the custom page last committed, and forces five more. Row 0 of the
/// first campaign is difficulty 0, 5,000 crowns and **one** AI lord — so a
/// four-county map opens with two realms holding one county each, which is a
/// position the custom page's defaults (five nobles) could not produce.
#[test]
fn the_campaign_row_overrides_the_custom_options() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let (_screen, _t) = walk_to_campaign(&assets, &mut game, false);

    assert_eq!(game.kingdom.options.difficulty, 0, "row 0 is the easy tier");
    assert_eq!(game.kingdom.options.time_limit, 0, "a campaign has no time limit");
    assert!(!game.kingdom.options.advanced_farming, "Campaign_LoadEntry zeroes it");
    assert!(!game.kingdom.options.exploration, "Campaign_LoadEntry zeroes it");
    assert!(!game.kingdom.options.armies_eat, "Campaign_LoadEntry zeroes it");
    assert_eq!(game.kingdom.options.fight_humans_only_byte, 1, "and forces this one to 1");

    let realms = (1..MAX_REALMS).filter(|&i| game.kingdom.realms[i].in_play).count();
    assert_eq!(realms, 2, "one person and one lord, from g_aiLordCount = 1");
    let owned =
        game.kingdom.county_ids().filter(|&id| game.kingdom.counties[id].owner != 0).count();
    assert_eq!(owned, 2, "two realms, one county each");
    assert_eq!(game.kingdom.realms[game.player as usize].gold, 5000, "row 0's purse");

    // The counter and the track went onto the *new* game, so the conquest
    // screen can step to Rose rather than back to the default track's row 0.
    assert_eq!(game.campaign.track, l2_game::victory::Track::First);
    assert_eq!(game.campaign.map, 0, "the first campaign starts at row 0");
}

/// **The second campaign is a different ladder and starts at row 2.**
///
/// `Setup_ChooseCampaign` writes `g_campaignMap = 2` for the right-hand choice,
/// and track B's rows 0 and 1 are the zero padding. Australia, not Quaintville
/// and not England.
#[test]
fn the_second_campaign_starts_on_australia() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let (_screen, t) = walk_to_campaign(&assets, &mut game, true);
    assert_eq!(t, Transition::Push(ScreenId::Campaign));
    assert_eq!(game.map_slot, AUSTRALIA);
    assert_eq!(game.campaign.track, l2_game::victory::Track::Second);
    assert_eq!(game.campaign.map, 2, "the second campaign is entered at row 2");
    assert_eq!(game.kingdom.county_count, counties_in(&assets, AUSTRALIA));
}

/// **The other way into page 4 does not start a game at all.**
///
/// `FUN_00433155`'s hotspot-2 arm is a branch, and its `else` limb walks on to
/// the page that chooses a game rather than starting one. Reading the whole
/// button as *Start* is what let the campaign limb go missing for a merge, so
/// the `else` limb is asserted too — otherwise `DAT_0057D320` could be ignored
/// and every test above would still pass by starting a campaign unconditionally.
#[test]
fn continue_without_a_campaign_opens_the_custom_page() {
    use l2_game::screens::setup::{item_rect, ITEM_H, SHIELD_BUTTONS};
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let before = game.map_slot;

    let mut screen = SetupScreen::new(SetupPage::Title);
    // Page 1, item 1 — "Multiple players", the arm that clears the flag.
    let r = item_rect(1);
    click(&mut screen, &mut game, &assets, r.x + 20, r.y + r.h / 2);
    assert_eq!(screen.page(), SetupPage::Shield);

    let (x, y, _) = SHIELD_BUTTONS[1];
    let t = click(&mut screen, &mut game, &assets, x + 20, y + ITEM_H / 2);
    assert_eq!(t, Transition::Stay, "Continue started a game it should not have");
    assert_eq!(screen.page(), SetupPage::Custom, "it should open the custom page");
    assert_eq!(game.map_slot, before, "and it should not have built a world");
}

// --------------------------------------------------- the colour on page 4

/// Walk the front end to page 4, take colour `shield` (1 … 5), press
/// *Continue*, and press *Start* on the custom page — every click a real
/// coordinate out of the geometry tables.
///
/// **This is the road the player took**, and until now nothing travelled it:
/// `tests/setup.rs` presses the custom page's *Start* from a screen constructed
/// straight onto page 7, and the three campaign tests above reach page 4 and
/// press *Continue* without ever clicking a shield. The colour picker had a
/// hotspot, a painter and no test.
fn walk_taking(assets: &Assets, game: &mut Game, shield: u8) -> (SetupScreen, Transition) {
    use l2_game::screens::setup::{
        item_rect, ITEM_H, SHIELD_BUTTONS, SHIELD_H, SHIELD_STEP, SHIELD_W, SHIELD_X, SHIELD_Y,
    };
    assert!((1..=5).contains(&shield));

    let mut screen = SetupScreen::new(SetupPage::Title);
    // Page 1, item 1 — *Multiple players*, the one arm of the title menu that
    // opens page 4 without also choosing a campaign.
    let r = item_rect(1);
    click(&mut screen, game, assets, r.x + 20, r.y + r.h / 2);
    assert_eq!(screen.page(), SetupPage::Shield, "the title menu did not open page 4");

    // The shield itself. `FUN_0041F1DD` lays the five out at
    // x = 0x70 + 0x58 i, y = 0x8C.
    let i = i32::from(shield) - 1;
    click(
        &mut screen,
        game,
        assets,
        SHIELD_X + i * SHIELD_STEP + SHIELD_W / 2,
        SHIELD_Y + SHIELD_H / 2,
    );
    assert_eq!(screen.shield(), shield, "the page did not take the click");

    // *Continue*, which with no campaign chosen walks on to the custom page.
    let (x, y, _) = SHIELD_BUTTONS[1];
    click(&mut screen, game, assets, x + 20, y + ITEM_H / 2);
    assert_eq!(screen.page(), SetupPage::Custom);
    {
        let mut ctx = Ctx { game, assets };
        screen.update(&mut ctx);
    }
    // The list is untouched, so it is on slot 0 — England, and England is
    // scenario group 0, which is the arrangement `docs/rules.md` §7a tabulates.
    assert_eq!(screen.map(), ENGLAND, "the list should still be on England");
    let t = press_start(&mut screen, game, assets);
    (screen, t)
}

/// Lord ids, which are `L2.eng` group 7's indices.
const KNIGHT: u8 = 1;
const BARON: u8 = 2;
const COUNTESS: u8 = 3;
const BISHOP: u8 = 4;

/// `docs/rules.md` §7a, as five rows of `(shield, lord)` for realms 2 … 5,
/// **typed from the document and computed from nothing.**
///
/// The row that matters is the second: the Knight does not fall back to red, he
/// becomes the **black** lord, and the **Baron** takes red.
const SEVEN_A: [[(u8, u8); 4]; 5] = [
    [(2, KNIGHT), (3, BARON), (4, BISHOP), (5, COUNTESS)], // take 1 red
    [(1, BARON), (3, KNIGHT), (4, BISHOP), (5, COUNTESS)], // take 2 yellow
    [(1, BARON), (2, KNIGHT), (4, BISHOP), (5, COUNTESS)], // take 3 black
    [(1, BARON), (2, KNIGHT), (3, COUNTESS), (5, BISHOP)], // take 4 magenta
    [(1, BARON), (2, KNIGHT), (3, COUNTESS), (4, BISHOP)], // take 5 blue
];

/// **The headline: pick blue and be blue.**
///
/// A player reported *"I picked a colour and it didn't get honoured once the
/// game opened."* Two things had to be true for that and only one of them was
/// the picker: the page took the click and stored it, and
/// `l2_scenario::newgame::assign_lords` then ignored it, because `NewGame` had
/// no shield field and the line read `let shield = realm`.
#[test]
fn the_colour_a_person_picks_is_the_colour_their_realm_flies() {
    let assets = assets!();
    for shield in 1..=5u8 {
        let mut game = Game::new(scenario::SEED);
        let (_screen, t) = walk_taking(&assets, &mut game, shield);
        assert_eq!(t, Transition::Push(ScreenId::Campaign), "shield {shield}: Start did nothing");

        let me = game.player as usize;
        assert_eq!(
            game.kingdom.realms[me].shield_index, shield,
            "the person picked shield {shield} and their realm flies {}",
            game.kingdom.realms[me].shield_index
        );
        // And the copy the *map* painter reads, which is the one the player was
        // looking at when he said it had not been honoured.
        assert_eq!(game.realm_colour[me], shield, "shield {shield}: the drawn colour");
        // Five realms, five different colours, none of them left at zero.
        let mut flown: Vec<u8> =
            (1..MAX_REALMS).map(|r| game.kingdom.realms[r].shield_index).collect();
        flown.sort_unstable();
        assert_eq!(flown, vec![1, 2, 3, 4, 5], "shield {shield}: two realms share a colour");
    }
}

/// **The counter-intuitive half, and the one a player will check: taking a
/// middle colour moves the lords, not only their colours.**
///
/// `Realms_AssignLords` (`0x0049CAAA`) hands the AIs the lowest shield nobody
/// has taken, *in realm order*, and only then reads `g_lordChoice` to pick each
/// realm's lord **from its shield**. So taking yellow does not push the Knight
/// to the next colour along — red's candidate list names the **Baron** first
/// and the walk reaches red before black, so the Baron becomes the red lord and
/// the Knight ends up black.
///
/// The expected values are typed out of `docs/rules.md` §7a. Nothing here
/// computes them, which is the difference between checking the rule and
/// checking that the code agrees with itself — `docs/agents.md`, *"compute the
/// probe from the constant you are ablating"*.
#[test]
fn taking_a_middle_colour_moves_which_lord_flies_which_shield() {
    let assets = assets!();
    for shield in 1..=5u8 {
        let mut game = Game::new(scenario::SEED);
        let (_screen, _t) = walk_taking(&assets, &mut game, shield);
        let got: Vec<(u8, u8)> = (2..MAX_REALMS)
            .map(|r| (game.kingdom.realms[r].shield_index, game.kingdom.realms[r].lord))
            .collect();
        assert_eq!(
            got,
            SEVEN_A[shield as usize - 1].to_vec(),
            "taking shield {shield} did not produce docs/rules.md 7a's row"
        );
    }

    // Stated as the player-visible sentence, because a table is easy to read
    // past. Take yellow: red is the Baron's and the Knight is on black.
    let mut game = Game::new(scenario::SEED);
    walk_taking(&assets, &mut game, 2);
    let by_shield = |s: u8| {
        (1..MAX_REALMS).find(|&r| game.kingdom.realms[r].shield_index == s).expect("a realm")
    };
    assert_eq!(game.kingdom.realms[by_shield(1)].lord, BARON, "red is the Baron's");
    assert_eq!(game.kingdom.realms[by_shield(3)].lord, KNIGHT, "and the Knight has black");
    assert!(
        !(1..MAX_REALMS).any(|r| game.kingdom.realms[r].lord == KNIGHT
            && game.kingdom.realms[r].shield_index == 2),
        "nobody but the person has yellow"
    );

    // …and the name the interface prints for that realm, which is
    // `Realms_AssignLords`' own `Eng_Seek(7, lord)` and what a player reads on
    // the diplomacy screen.
    let red = by_shield(1);
    assert_eq!(
        game.player_names[red].as_str(),
        assets.shell.text(l2_game::screens::setup::LORD_TITLE_GROUP, BARON as usize),
        "the red realm should be named The Baron"
    );
}

/// **The colour survives the campaign route too**, which is the one that does
/// *not* go through the custom page's *Start*.
///
/// `Campaign_LoadEntry` rewrites all twelve committed options from the campaign
/// row. It says nothing about the shield, and it must not: page 4 is the page a
/// campaign passes *through*. If the colour were carried on
/// `l2_game::setup::Settings` instead of beside the seed, this test would fail
/// and the custom-page tests above would not — which is why it is here.
#[test]
fn a_campaign_keeps_the_colour_page_four_chose() {
    use l2_game::screens::setup::{
        item_rect, ITEM_H, PAIR_W, PAIR_X, PAIR_Y, SHIELD_BUTTONS, SHIELD_H, SHIELD_STEP,
        SHIELD_W, SHIELD_X, SHIELD_Y,
    };
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let mut screen = SetupScreen::new(SetupPage::Title);

    // *Single player*, *Play Now!*, then the left-hand campaign.
    let r = item_rect(0);
    click(&mut screen, &mut game, &assets, r.x + 20, r.y + r.h / 2);
    let r = item_rect(0);
    click(&mut screen, &mut game, &assets, r.x + 20, r.y + r.h / 2);
    click(&mut screen, &mut game, &assets, PAIR_X[0] + PAIR_W / 2, PAIR_Y + ITEM_H / 2);
    assert_eq!(screen.page(), SetupPage::Shield);

    // Blue, the far right of the five.
    click(
        &mut screen,
        &mut game,
        &assets,
        SHIELD_X + 4 * SHIELD_STEP + SHIELD_W / 2,
        SHIELD_Y + SHIELD_H / 2,
    );
    let (x, y, _) = SHIELD_BUTTONS[1];
    let t = click(&mut screen, &mut game, &assets, x + 20, y + ITEM_H / 2);
    assert_eq!(t, Transition::Push(ScreenId::Campaign), "Continue did not start the campaign");
    assert_eq!(game.map_slot, QUAINTVILLE, "and it should still be the campaign's first map");
    assert_eq!(game.kingdom.realms[game.player as usize].shield_index, 5, "blue");
    // Quaintville is slot 17, so scenario group 1, and the row gives one AI
    // lord — so the walk hands out exactly one shield: the lowest free one,
    // which is red.
    assert_eq!(game.kingdom.realms[2].shield_index, 1, "the one AI takes the lowest free colour");
}

/// **Clicking the colour you already hold does nothing**, which is
/// `FUN_00432FAB`'s `if (claim[hotspot] == 0)` guard seen from single player:
/// the only occupied entry in the claim table is your own.
#[test]
fn re_picking_the_same_shield_changes_nothing() {
    use l2_game::screens::setup::{item_rect, SHIELD_H, SHIELD_STEP, SHIELD_W, SHIELD_X, SHIELD_Y};
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let mut screen = SetupScreen::new(SetupPage::Title);
    let r = item_rect(1);
    click(&mut screen, &mut game, &assets, r.x + 20, r.y + r.h / 2);

    assert_eq!(screen.shield(), 1, "page 4 opens on red");
    let (x, y) = (SHIELD_X + 2 * SHIELD_STEP + SHIELD_W / 2, SHIELD_Y + SHIELD_H / 2);
    click(&mut screen, &mut game, &assets, x, y);
    assert_eq!(screen.shield(), 3, "black");
    click(&mut screen, &mut game, &assets, x, y);
    assert_eq!(screen.shield(), 3, "and clicking it again is not a toggle");
}
