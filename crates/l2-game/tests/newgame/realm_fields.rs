#![allow(unused_imports)]
use super::*;
use l2_game::scenario::SEED;
use l2_game::setup::{Settings, SetupOptions};

fn settings(ai_lords: i32) -> Settings {
    let base = SetupOptions::new().commit(1, l2_kingdom::Quirks::default());
    Settings { ai_lords, ..base }
}

/// One press of *Start*: `scenario::new_game` then `Settings::apply_to`, the
/// pair `SetupScreen::start` runs (`render.rs:604`, `render.rs:617`).
fn start(assets: &Assets, game: &mut Game, ai_lords: i32) -> Settings {
    let s = settings(ai_lords);
    let tables = game.kingdom.tables;
    *game = scenario::new_game(assets, ENGLAND, &s, 1, 1, SEED, tables).expect("England builds");
    s.apply_to(game);
    s
}

fn owns_a_county(game: &Game, realm: usize) -> bool {
    game.kingdom.county_ids().any(|id| game.kingdom.counties[id].owner as usize == realm)
}

/// **A new game writes `+0x29` and `+0x2A` for every realm 1 … 5, seated or
/// not, and clears the whole realm array first.**
///
/// `fought-old_turn.sav` is a two-realm Quaintville game whose realm 3 is out
/// of play and still carries `countyCount 1` — which reads as a field a new
/// game forgot to clear, and is not. Three writers, in the order
/// `Game_NewGame` reaches them:
///
/// * `FUN_0049C995` (`00490000.c:5714-5720`), at lobby entry — `FUN_0046F21F`
///   zeroes all seven `0x160`-byte realm records, then `shieldIndex = i` and
///   `gold = g_startingGoldChosen` for 1 … 5. Nothing survives a second game.
/// * `Realms_AssignLords` (`0x0049CAAA`) — `lord = 0` (`:5807`) and, for a
///   realm past `g_aiLordCount`, `strength = 0` (`:5842`); then a tail loop
///   over 1 … 5 that does not test seating: `+0x2A = 1; countyCount = +0x2A;`
///   (`:5800-5801`).
/// * `Game_SetupRealmsAndCounties` (`0x0049BD99`) — everything else, and only
///   where `isHuman != 0 || lord != 0` (`:5491`), including `countyCount = 1`
///   and `+0x2A = 1` again (`:5581-5582`).
///
/// `Realm_UpdateTotals` (`0x0049D1E0`), the only recount, is called for in-play
/// realms alone, so the 1 stands for the rest of the game. The fixture's realms
/// 3 … 5 read `strength 0, countyCount 1, peakCounties 1`. [V]
#[test]
fn a_second_game_rebuilds_every_realm_and_leaves_an_unseated_one_holding_one_county() {
    let assets = assets!();

    // Game one seats all five, so realm 3 ends it owning a county.
    let mut game = Game::new(SEED);
    let full = start(&assets, &mut game, 4);
    assert!(game.kingdom.realms[3].in_play, "five lords, so realm 3 is seated");
    assert!(owns_a_county(&game, 3), "a seated realm holds its start county");
    assert_eq!(game.kingdom.realms[3].gold, full.gold, "and its treasury");

    // Game two, same session, two realms — the fixture's shape.
    start(&assets, &mut game, 1);
    let r3 = &game.kingdom.realms[3];
    assert!(!owns_a_county(&game, 3), "realm 3 is out of play and holds nothing");

    // What `Realms_AssignLords` writes for an unbudgeted realm.
    assert_eq!(r3.strength, 0, "+0x04");
    assert!(!r3.in_play);
    assert_eq!(r3.lord, 0, "+0x07");
    // What its tail loop writes for every realm 1 … 5.
    assert_eq!(r3.county_count, 1, "+0x29 — the fixture's realm 3, and ours zeroed it");
    assert_eq!(r3.peak_counties, 1, "+0x2A");
    assert_eq!(r3.shield_index, 3, "+0x0A — FUN_0049C995's seed, kept");
    // And what the seated pair gets.
    for id in 1..=2 {
        assert!(game.kingdom.realms[id].in_play, "realm {id} is seated");
        assert_eq!(game.kingdom.realms[id].county_count, 1);
        assert_eq!(game.kingdom.realms[id].peak_counties, 1);
    }

    // Nothing of game one is left: `FUN_0046F21F` zeroed the array, so the
    // two-realm game is the same whether or not a five-realm game came first.
    let mut fresh = Game::new(SEED);
    start(&assets, &mut fresh, 1);
    for id in 0..MAX_REALMS {
        assert_eq!(
            game.kingdom.realms[id], fresh.kingdom.realms[id],
            "realm {id} carried something across the two games",
        );
    }
}

/// **The starting castle is drawn on the plot.**
///
/// ```c
/// g_counties[county].castleType = g_startCastle;
/// if (g_counties[county].castleType != 0) {
///   Castle_StampTile(county, g_counties[county].castleType - 1);
/// }
/// ```
///
/// `Game_SetupRealmsAndCounties` (`0x0049BD99`, `00490000.c:5714-5716`), and
/// the `if` was missing from [`Settings::apply_to`]. Every lord began with a
/// castle in the record and bare `0x14` ground on the map, so
/// `Ai_FindCastleBuildingTile` (`0x004A65A3`) found no castle tile in the AI's
/// own county and `Unit_ReachCastleBuilding` (`0x004686A0`) had nothing to fire
/// on: the garrisons AI step 7 raised for it walked at the county town and
/// stood there for the rest of the game. Ablate the `stamp_castle_terrain` call
/// in `crates/l2-game/src/setup/settings.rs` and this goes red.
#[test]
fn a_starting_castle_is_drawn_on_its_plot() {
    let assets = assets!();
    let mut game = Game::new(SEED);
    let s = start(&assets, &mut game, 4);
    assert_ne!(s.castle_type, 0, "the castle row is 'none', so this proves nothing");
    let mut seen = 0;
    for id in game.kingdom.county_ids() {
        if game.kingdom.counties[id].owner == 0 {
            continue;
        }
        seen += 1;
        let drawn = (0..l2_kingdom::map::MAP_TILES)
            .filter(|&i| {
                game.kingdom.campaign.map.county[i] as usize == id
                    && game.kingdom.campaign.map.flags[i]
                        & l2_kingdom::map::flags::SETTLEMENT
                        != 0
                    && (l2_kingdom::map::terrain::CASTLE_FROM
                        ..=l2_kingdom::map::terrain::CASTLE_TO)
                        .contains(&game.kingdom.campaign.map.terrain[i])
            })
            .count();
        assert_eq!(
            drawn, 4,
            "county {id} has castleType {} and {drawn} castle tiles — `Map_StampBlock` writes a 2×2",
            game.kingdom.counties[id].castle_type
        );
    }
    assert_eq!(seen, 5, "five lords, five start counties");
}
