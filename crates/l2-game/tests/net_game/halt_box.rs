//! The owner's dropped game (2026-10-11): the stopped box could not be closed and
//! nothing under it answered. A real pair, the host ends the game (C260), and the
//! joiner's screens are driven as a player drives them.

use crate::kit;
use crate::live_tcp::{pair, Peer};
use crate::seats::assets;
use l2_game::input::{Event, Key};
use l2_game::net::{continue_alone, AfterHalt};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::leave::LeaveScreen;
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

/// mp-seat-polish (3): after the host's *Save, then end*, with a tip up, the title, options
/// and load pages present in `Gateway.256` on every frame. Ablations: `SetupScreen::palette`
/// returning `None` (the campaign palette, as the one 2026-10-12 shot showed);
/// `tips.unhost()` in `leave::to_front_end` (a tip host over the title takes every click).
#[test]
fn the_front_end_after_the_hosts_end_presents_in_its_own_palette() {
    kit::big_stack(front_end_palette);
}

fn front_end_palette() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let _scope = kit::scope("palette");
    let (mut seats, _) = kit::game(&assets, 2);
    let mut m = Machine::new(ScreenId::Campaign);
    let mut frame = |seats: &mut Vec<kit::Seat>, m: &mut Machine, e: Option<Event>| {
        kit::run(seats, &assets, 1);
        let mut ctx = Ctx { game: &mut seats[0].game, assets: &assets };
        if let Some(e) = e {
            m.handle(e, &mut ctx);
        }
        m.update(&mut ctx);
        m.ids()
    };
    while !seats[0].game.tips.hosting() {
        frame(&mut seats, &mut m, None);
    }
    m.push(ScreenId::Leave);
    let end = LeaveScreen::rects(&Ctx { game: &mut seats[0].game, assets: &assets })[0];
    let mut seen = Vec::new();
    for (x, y) in [(end.x + 4, end.y + 4), (320, 103), (320, 138)] {
        frame(&mut seats, &mut m, Some(Event::Click { x, y }));
        for _ in 0..60 {
            let ids = frame(&mut seats, &mut m, None);
            let shown = (m.palette_name(), m.live_palette().is_some(), m.fade());
            assert_eq!(shown, (Some("Gateway.256"), false, None), "{ids:?}");
            seen.push(ids);
        }
    }
    assert_eq!(seen.last().unwrap(), &vec![ScreenId::Setup(SetupPage::Load)]);
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
