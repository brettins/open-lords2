#![allow(unused_imports)]
use super::*;
use super::start_game::*;
use super::campaign::*;
use super::heraldry::*;
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

/// **The *Army Size* option raises a garrison** — `FUN_0049BD99`'s
/// `g_startArmySize` arm (`0x0049BF9E`), rows 0 and 3 of `g_startTroops`
/// (`0x004DC110`) on England.
///
/// Row 0 is all zeroes and raises nothing; row 3 is `{0,0,0,100,100,100,0}` —
/// 300 men, in the realm's own start county, and the county keeps its people
/// because the arm pre-credits the population before `Levy_DebitPopulation`
/// takes it back.
#[test]
fn army_size_raises_the_starting_garrison() {
    let assets = assets!();
    for (row, men, troops) in [(0usize, 0, [0; 7]), (3usize, 300, [0, 0, 0, 100, 100, 100, 0])] {
        let base = l2_game::setup::SetupOptions::new().commit(1, l2_kingdom::Quirks::default());
        let settings = l2_game::setup::Settings {
            ai_lords: 1,
            garrison: l2_game::setup::START_TROOPS[row],
            ..base
        };
        let tables = Game::new(scenario::SEED).kingdom.tables;
        let mut game = scenario::new_game(&assets, ENGLAND, &settings, 1, 1, scenario::SEED, tables)
            .expect("England builds");
        settings.apply_to(&mut game);

        // The world the scenario builds already carries merchants, so this
        // counts armies only.
        let armies: Vec<usize> = game
            .kingdom
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == l2_kingdom::unit::UnitKind::Army)
            .map(|(id, _)| id)
            .collect();
        let want = if row == 0 { 0 } else { 2 };
        assert_eq!(armies.len(), want, "row {row}: one garrison per realm in play");

        for id in armies {
            let u = game.kingdom.campaign.units.get(id).expect("the slot is live").clone();
            assert_eq!(u.men, men, "row {row}: men");
            assert_eq!(u.troops, troops, "row {row}: the troop mix is the table row");
            let county = u.county as usize;
            assert_eq!(
                game.kingdom.counties[county].owner, u.owner,
                "row {row}: the garrison stands in its own realm's county",
            );
            assert_eq!(u.home_county, u.county, "row {row}: home county");
            // `County_FindFreeRoadTile` / `County_FindFreeOpenTile` search the
            // county's anchor, and `Unit_Spawn` demands `(flags & 0xFC) == 0`.
            let f = game.kingdom.campaign.map.flags_at(u.x, u.y);
            let bare = !(l2_kingdom::map::flags::ROAD | l2_kingdom::map::flags::BOUNDARY);
            assert_eq!(f & bare, 0, "row {row}: the muster tile is bare");
            // The surcharge `Army_Create` writes is undone by the next line.
            assert_eq!(game.kingdom.counties[county].levy_surcharge, 0, "row {row}: surcharge");
        }

        // The pre-credit cancels `Levy_DebitPopulation` exactly: the
        // county-status row's population survives the garrison.
        for c in game.kingdom.county_ids() {
            assert_eq!(
                game.kingdom.counties[c].population, settings.county.population,
                "row {row}: county {c} kept its people",
            );
        }
        // `Levy_ConsumeWeapons` ran against a zeroed armoury in the original, so
        // the armoury row survives the garrison intact.
        for id in 1..MAX_REALMS {
            if game.kingdom.realms[id].in_play {
                assert_eq!(game.kingdom.realms[id].weapons, settings.armoury, "row {row}: armoury");
            }
        }
    }
}

