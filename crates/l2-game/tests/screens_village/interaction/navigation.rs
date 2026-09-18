#![allow(unused_imports)]
use super::*;
use super::peasant_drag::*;
use super::job_popups::*;
use super::*;
use super::render::*;
use super::animation::*;
use common::*;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::map;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_view::chrome;
use l2_view::village;
use l2_view::Canvas;

/// The player, from that game: *"things that open a dialog will open
/// it and when you close that dialogue it will close town square and that
/// dialogue, probably something to fix so it only closes the dialog you
/// opened."* `docs/bugs.md` B63 records why it happens — `g_screenId` is one
/// byte and 57 of `Screen_FrameInput`'s 100 writes to it are the literal `0` —
/// and what a switch would cost. **We reproduce it on purpose**, so it needs a
/// test that fails if somebody quietly improves it. C59.
#[test]
fn a_screen_opened_over_the_village_takes_the_village_with_it_when_it_closes() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    // A sidebar button, clicked through the village. `FUN_00432967` is
    // `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` and it is the second
    // Of the six guards. COURT, as a shell's right button
    // closes it and this test needs to watch it close.
    let button = map::SIDEBAR_BUTTONS[1].rect();
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: button.x + 4, y: button.y + 4 }, &mut c);
    }
    assert_eq!(
        m.ids(),
        vec![ScreenId::Campaign, ScreenId::Court],
        "the sidebar's screen replaced the village rather than stacking on it"
    );

    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::RightClick { x: 320, y: 240 }, &mut c);
    }
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "a panel's exit is a constant 0, not a memory");
}

