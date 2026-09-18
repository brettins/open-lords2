//! `App_WndProc` (`0x004B29BE`) `VK_ESCAPE` is `if (g_appPhase == 3)
//! Menu_Quit(); else g_quitRequest = 1;` — inside a game the key is the menu
//! item, and `Menu_Quit` (`0x004343F8`) is `Ui_OpenConfirm(0, 0xA0, 0xA0,
//! FUN_0043441C)`. Nothing quits until the box is answered.
//!
//! No install needed: `Assets::placeholder` has no `L2.eng` and no chrome, so
//! the prompt falls back to our transcription and the gauntlets to plain
//! buttons. Neither is what is asserted here — the transitions are.

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::battlefield::{CONFIRM_NO, CONFIRM_YES};
use l2_game::screens::confirm::Ask;
use l2_game::Game;

fn world() -> (Game, Assets) {
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(3);
    l2_testkit::chain_neighbours!(g.kingdom);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[2].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[3].owner = 2;
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn send(m: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let mut ctx = Ctx { game, assets };
    m.handle(event, &mut ctx);
}

fn answer(m: &mut Machine, game: &mut Game, assets: &Assets, at: l2_game::input::Rect) {
    send(m, game, assets, Event::Click { x: at.x + 4, y: at.y + 4 });
    for _ in 0..press::DELAYED_FRAMES {
        let mut ctx = Ctx { game, assets };
        m.update(&mut ctx);
    }
}

/// **Ablated** by putting `Transition::Pop` back on the map's Escape arm: the
/// stack is `[Campaign]` after the key and the first assertion goes red.
#[test]
fn escape_on_the_campaign_map_opens_the_exit_box_and_does_not_leave() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Escape));
    assert_eq!(
        m.ids(),
        vec![ScreenId::Campaign, ScreenId::Confirm(Ask::Quit)],
        "the box is on top of the map, and the map is still there"
    );
    assert!(!m.should_quit(), "the key asks, it does not quit");
}

/// `FUN_0043441C`'s else: `g_screenId = g_screenIdSaved` — back to whatever
/// asked, and nothing else happens.
#[test]
fn no_returns_to_the_map_with_the_game_still_running() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Escape));
    answer(&mut m, &mut game, &assets, CONFIRM_NO);
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "the box closed, the map is back");
    assert!(!m.should_quit());
}

/// `FUN_0043441C`'s yes, single player: `g_quitRequest = 1`. (The `lom.256`
/// send-off on screen `0x45` that the original shows first is not built.)
#[test]
fn yes_leaves_the_game() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Escape));
    assert!(!m.should_quit(), "not before the answer");
    answer(&mut m, &mut game, &assets, CONFIRM_YES);
    assert!(m.should_quit(), "the yes is the quit");
}

#[test]
fn the_gauntlet_is_held_for_twenty_frames_before_the_answer() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Escape));
    send(&mut m, &mut game, &assets, Event::Click { x: CONFIRM_YES.x + 4, y: CONFIRM_YES.y + 4 });
    for _ in 1..press::DELAYED_FRAMES {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert!(!m.should_quit(), "nineteen frames in, the gauntlet is still down");
    assert_eq!(m.ids().last().copied(), Some(ScreenId::Confirm(Ask::Quit)));
}

/// `Menu_NewGame` (`0x00433DBD`) — `Ui_OpenConfirm(1, …)`, group 10 index 1.
///
/// Its yes is `FUN_00433DEB`, which tears the game down; ours lands on the
/// front end's title page.
#[test]
fn the_new_game_box_answers_yes_by_leaving_for_the_front_end() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Confirm(Ask::NewGame));
    answer(&mut m, &mut game, &assets, CONFIRM_YES);
    assert_eq!(
        m.ids().last().copied(),
        Some(ScreenId::Setup(l2_game::screens::setup::SetupPage::Title)),
        "the front end, not a quit"
    );
    assert!(!m.should_quit());
}
