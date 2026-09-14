#![allow(unused_imports)]
use super::*;
use super::start_game::*;
use super::campaign::*;
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
        // And the copy the *map* painter reads
        // looking at when he said it had not been honoured.
        assert_eq!(game.realm_colour[me], shield, "shield {shield}: the drawn colour");
        // Five realms, five different colours, none of them left at zero.
        let mut flown: Vec<u8> =
            (1..MAX_REALMS).map(|r| game.kingdom.realms[r].shield_index).collect();
        flown.sort_unstable();
        assert_eq!(flown, vec![1, 2, 3, 4, 5], "shield {shield}: two realms share a colour");
    }
}

/// **The counter-intuitive half
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

    // …and the name the interface prints for that realm
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
/// and the custom-page tests above would not.
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

// ------------------------------------------------------ the starting garrison

