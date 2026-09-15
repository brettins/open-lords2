#![allow(unused_imports)]
use super::*;
use super::start_game::*;
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

/// Not slot 0, which is where the map list sits when nobody has touched it, and
/// not whatever `lastturn.sav` happens to hold. `Campaign_LoadEntry`
/// (`0x00499E5D`) reads column `+0x00` of row `g_campaignMap` of
/// `g_campaignTableA` (`0x004D8E18`), and that row is 17. Two independent
/// readings agree: the table in the executable, and an autosave the *original*
/// wrote eight turns into a campaign, which carries `g_scenarioIndex` 17 and
/// four counties.
pub(super) const QUAINTVILLE: usize = 17;
const AUSTRALIA: usize = 52;

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
    let r = item_rect(0);
    click(&mut screen, game, assets, r.x + 20, r.y + r.h / 2);
    assert_eq!(screen.page(), SetupPage::Options, "Single player opens page 2");

    let r = item_rect(0);
    click(&mut screen, game, assets, r.x + 20, r.y + r.h / 2);
    assert_eq!(screen.page(), SetupPage::Campaign, "Play Now! opens page 5");

    let i = usize::from(right_hand_campaign);
    click(&mut screen, game, assets, PAIR_X[i] + PAIR_W / 2, PAIR_Y + ITEM_H / 2);
    assert_eq!(screen.page(), SetupPage::Shield, "choosing a campaign opens page 4");

    let (x, y, _) = SHIELD_BUTTONS[1];
    let t = click(&mut screen, game, assets, x + 20, y + ITEM_H / 2);
    (screen, t)
}

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

    let quaintville = counties_in(&assets, QUAINTVILLE);
    assert_eq!(quaintville, 4, "Quaintville is a four-county map");
    assert_ne!(quaintville, counties_in(&assets, ENGLAND), "or this proves nothing");
    assert_eq!(game.kingdom.county_count, quaintville, "the world is not Quaintville's");

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

    assert_eq!(game.campaign.track, l2_game::victory::Track::First);
    assert_eq!(game.campaign.map, 0, "the first campaign starts at row 0");
}

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

/// `FUN_00433155`'s hotspot-2 arm is a branch, and its `else` limb walks on to
/// the page that chooses a game. Reading the whole
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
    let r = item_rect(1);
    click(&mut screen, &mut game, &assets, r.x + 20, r.y + r.h / 2);
    assert_eq!(screen.page(), SetupPage::Shield);

    let (x, y, _) = SHIELD_BUTTONS[1];
    let t = click(&mut screen, &mut game, &assets, x + 20, y + ITEM_H / 2);
    assert_eq!(t, Transition::Stay, "Continue started a game it should not have");
    assert_eq!(screen.page(), SetupPage::Custom, "it should open the custom page");
    assert_eq!(game.map_slot, before, "and it should not have built a world");
}


