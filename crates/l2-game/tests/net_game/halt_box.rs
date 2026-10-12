//! The owner's dropped game (2026-10-11): the stopped box could not be closed and
//! nothing under it answered. A real pair, the host ends the game (C260), and the
//! joiner's screens are driven as a player drives them.

use crate::live_tcp::{pair, Peer};
use crate::seats::assets;
use l2_game::input::{Event, Key};
use l2_game::net::{continue_alone, AfterHalt};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::net_halt::NetHaltScreen;
use l2_game::screens::setup::SetupPage;
use l2_view::Canvas;

fn halted() -> Option<Peer> {
    let mut peers = pair()?;
    for _ in 0..60 {
        peers.iter_mut().for_each(|p| p.live.pump(&mut p.game));
    }
    peers[0].live.leave();
    let [_, mut a] = peers;
    for _ in 0..2_000 {
        a.live.pump(&mut a.game);
        if a.game.net.halted.is_some() {
            return Some(a);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the survivor never halted");
}

/// Ablations: the `NetHalt` push in `Machine::update`; `Transition::Stay` for a
/// click outside the buttons; `continue_alone`'s `Link::default()`.
#[test]
fn the_stopped_box_takes_every_click_and_continue_alone_keeps_the_game_running() {
    let Some(mut a) = halted() else { l2_testkit::skip!("no install") };
    let assets = assets().unwrap();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ctx = Ctx { game: &mut a.game, assets: &assets };
    m.update(&mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::NetHalt), "the halt opens the box");
    let said = ctx.game.net.halted.clone().unwrap();
    assert_eq!(said, "Richard ended the game.");

    let mut canvas = Canvas::screen();
    m.draw(&ctx, &mut canvas);
    let [alone, leave] = NetHaltScreen::buttons(&ctx);
    assert!(alone.y + alone.h <= 480 && alone.x + alone.w < leave.x, "{alone:?} {leave:?}");

    let stack = m.ids();
    for e in [Event::Click { x: 30, y: 8 }, Event::Click { x: 600, y: 300 }, Event::KeyDown(Key::Escape)] {
        m.handle(e, &mut ctx);
        m.update(&mut ctx);
        assert_eq!(m.ids(), stack, "{e:?} reached under the box");
    }

    m.handle(Event::Click { x: alone.x + 4, y: alone.y + 4 }, &mut ctx);
    assert_eq!(ctx.game.net.after_halt, Some(AfterHalt::Alone));
    assert!(!m.ids().contains(&ScreenId::NetHalt));
    continue_alone(ctx.game);
    assert!(!ctx.game.net.lockstep && ctx.game.net.halted.is_none() && !ctx.game.multiplayer);

    // The screen the stopped world had opened under the box (a battle prompt here) now
    // answers as in any single-player game.
    let top = m.top_id();
    for _ in 0..200 {
        m.update(&mut ctx);
    }
    assert!(!m.ids().contains(&ScreenId::NetHalt), "the box came back");
    assert_eq!(m.top_id(), top, "something replaced the screen the player was answering");
}

/// Ablation: `Transition::Replace(ScreenId::Setup(SetupPage::Title))` in `answer`.
#[test]
fn leave_game_goes_to_the_title_page() {
    let Some(mut a) = halted() else { l2_testkit::skip!("no install") };
    let assets = assets().unwrap();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ctx = Ctx { game: &mut a.game, assets: &assets };
    m.update(&mut ctx);
    let [_, leave] = NetHaltScreen::buttons(&ctx);
    m.handle(Event::Click { x: leave.x + 4, y: leave.y + 4 }, &mut ctx);
    assert_eq!(ctx.game.net.after_halt, Some(AfterHalt::Leave));
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Title)));
}
