#![allow(unused_imports)]
use super::*;
use super::persistence::*;
use super::rendering::*;
use l2_game::game::{Assets, Game};
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::setup::{SetupPage, SetupScreen, NAME_PLATE_X, NAME_PLATE_Y, NAME_X};
use l2_game::text::{FontMetrics, Kind, PlayerName, TextField, NAME_MAX_TYPED, PLAYER_NAME_LEN};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_view::Canvas;


#[test]
fn typing_a_name_reaches_the_field() {
    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);
    assert_eq!(field_of(&m), "Player1", "the field opens seeded — Edit_Begin(&g_options, …)");

    type_into(&mut m, &mut game, &assets, "Richard");
    assert_eq!(field_of(&m), "Richard", "seven characters over seven");
}

#[test]
fn a_short_name_leaves_the_tail_of_the_old_one_until_delete_or_insert() {
    let (mut game, assets) = bare();

    let mut m = name_page(&mut game, &assets);
    type_into(&mut m, &mut game, &assets, "Ed");
    assert_eq!(field_of(&m), "Edayer1", "the original's behaviour, not a defect of ours");
    for _ in 0..5 {
        press(&mut m, &mut game, &assets, Key::Delete);
    }
    assert_eq!(field_of(&m), "Ed", "VK_DELETE clears the tail");

    let mut m = name_page(&mut game, &assets);
    press(&mut m, &mut game, &assets, Key::Insert);
    type_into(&mut m, &mut game, &assets, "Ed");
    assert_eq!(field_of(&m), "EdPlayer1");
}

#[test]
fn the_field_takes_the_keys_the_menu_would_otherwise_spend() {
    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);

    let ts = type_into(&mut m, &mut game, &assets, "Ivo Rex");
    assert_eq!(field_of(&m), "Ivo Rex", "the I typed and the space typed");
    assert!(!pushed(&ts), "and the I did not also open the screen index");
    assert_eq!(m.page(), SetupPage::Shield, "and the space did not also press a button");

    let (mut game, assets) = bare();
    let mut title = SetupScreen::new(SetupPage::Title);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let t = title.handle(Event::KeyDown(Key::Char('I')), &mut ctx);
    assert!(matches!(t, Transition::Push(_)), "the title page still has its keyboard");
}


/// Typing into a field that nothing reads is the same defect as
/// `castle_degraded`, which was written by nothing. *Start* runs
/// `Player_SetHuman` (`0x0049BAE9`), and this asserts on `Game::player_names`
/// after driving the whole page — the name is never assigned by the test.
///
/// **Page 4 is reached through the campaign chooser, and that is not
/// incidental.** `FUN_00433155`'s *Continue* arm only starts a game when
/// `DAT_0057D320` says a campaign was chosen; every other route out of page 4
/// walks on to the page that picks a game. This test used to arrive from the
/// title menu's *Multiple players*, which is one of the arms that **clears**
/// that flag, so after that arm was reproduced it was pressing a button that
/// correctly does not start anything. The name still has to survive the trip:
#[test]
fn start_puts_the_typed_name_into_the_realm() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so Start has no map to build");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut game = Game::new(1);

    let mut screen = SetupScreen::new(SetupPage::Title);
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
        assert_eq!(screen.page(), SetupPage::Options);
        screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
        assert_eq!(screen.page(), SetupPage::Campaign);
        screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
        screen.update(&mut ctx);
    }
    assert_eq!(screen.page(), SetupPage::Shield);

    for c in "Aethelred".chars() {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::Text(c), &mut ctx);
    }
    assert_eq!(screen.name(), "Aethelred");

    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::Click { x: 0x150 + 40, y: 0xD7 + 8 }, &mut ctx);
    }

    let player = game.player as usize;
    assert_eq!(
        game.player_names[player].as_str(),
        "Aethelred",
        "g_playerNames[g_localPlayer] is what Player_SetHuman writes, and nothing \
         in this test assigned it"
    );

    // **And the other five slots are the AI lords', from `L2.eng` group 7.**
    let others: Vec<String> = (0..MAX_REALMS)
        .filter(|r| *r != player)
        .map(|r| game.player_names[r].as_str())
        .collect();
    assert!(
        others.iter().all(|n| !n.is_empty()),
        "every realm is named, not only the human's: {others:?}"
    );
    for (r, n) in (0..MAX_REALMS).filter(|r| *r != player).zip(&others) {
        let lord = game.kingdom.realms[r].lord as usize;
        let title = assets.shell.text(7, lord.min(4));
        assert_eq!(n, &title.chars().take(0x10).collect::<String>(), "realm {r} is lord {lord}");
    }
}


