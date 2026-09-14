#![allow(unused_imports)]
use super::*;

use l2_game::battlefield::LiveBattle;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::message;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{army, message as scroll};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::Troop;
use l2_view::Canvas;

/// **Tip 209 over the raise-army screen.** `Tip_Update`'s `0x17` arm, twenty
/// frames after the screen opens, the first time in a game.
#[test]
fn the_raise_army_screen_keeps_the_armourys_colours_behind_its_first_tip() {
    let a = assets!();
    let armoury = a.shell.palette("Armoury.256").expect("armoury.256 is in the install").clone();
    let campaign = a.palette.clone();

    let mut g = Game::new(11);
    g.prefs.tip_screens = true;
    g.kingdom.set_county_count(3);
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[1].population = 1_000;
    g.kingdom.counties[1].happiness = 90;
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].weapons = [20; 6];
    g.player = 1;
    g.selected = 1;

    let mut m = Machine::new(ScreenId::Campaign);
    send(&mut m, &mut g, &a, Event::KeyDown(Key::letter('r')));
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));
    let (_, before) = frame(&mut m, &mut g, &a);

    let mut posted = false;
    for _ in 0..60 {
        tick(&mut m, &mut g, &a);
        if g.messages.open().map(|r| r.group) == Some(209) {
            posted = true;
            break;
        }
    }
    assert!(posted, "Tip_Update shows 209, \"The Armoury:\", on screen 0x17");
    assert_eq!(
        m.ids(),
        vec![ScreenId::Campaign, ScreenId::RaiseArmy(1), ScreenId::Tip, ScreenId::Message],
        "0x27 and the scroll over the levy window"
    );

    let (c, during) = frame(&mut m, &mut g, &a);
    let tip = {
        let r = *g.messages.open().expect("the tip is open");
        let ctx = Ctx { game: &mut g, assets: &a };
        scroll::window_frame(&ctx, &r).expect("a tip window has a frame")
    };
    let levy = army::window(false);
    let behind: Vec<(i32, i32)> = PROBES
        .iter()
        .copied()
        .filter(|&p| outside((tip.x, tip.y, tip.w, tip.h), p))
        .filter(|&p| outside((levy.x, levy.y, levy.w, levy.h), p))
        .collect();
    assert!(behind.len() >= 4, "probes behind both windows: {behind:?}");
    let telling = behind
        .iter()
        .filter(|&&p| armoury.rgb(index(&c, p)) != campaign.rgb(index(&c, p)))
        .count();
    assert!(telling >= 1, "no probe that the two palettes colour differently, so nothing is proved");

    for &p in &behind {
        let i = index(&c, p);
        assert_eq!(
            shown(&during, p),
            armoury.rgb(i),
            "{p:?}, index {i}, behind the tip: armoury.256's colour, not the campaign palette's"
        );
        assert_eq!(shown(&during, p), shown(&before, p), "{p:?}: and not dimmed by the tip");
    }

    // `Msg_HandleInput`'s right release: `Msg_Dismiss`, then `FUN_00476E21`.
    send(&mut m, &mut g, &a, Event::RightClick { x: 5, y: 5 });
    tick(&mut m, &mut g, &a);
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::RaiseArmy(1)]);
    let (_, after) = frame(&mut m, &mut g, &a);
    for &p in &behind {
        assert_eq!(shown(&after, p), shown(&during, p), "{p:?}: the tip changed no colour going away");
    }
}

/// A live battle between a human (realm 1) and an AI (realm 2), paused, with
/// both deployment markers on one screen.
fn battle() -> LiveBattle {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[36 * l2_sim::terrain::DIM + 40] = 0x04;
    layer[44 * l2_sim::terrain::DIM + 40] = 0x0F;
    let field = l2_sim::terrain::build(&layer, 1);
    let ai: &[(Troop, u16)] = &[(Troop::Knights, 2)];
    let human: &[(Troop, u16)] = &[(Troop::Pikemen, 3)];
    let runner = BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    let mut live = LiveBattle::new(runner, 0, 0, 0, None, 1, 1);
    live.paused = true;
    live.cam = (33, 33);
    live
}

/// **A message that opens over the battlefield asks for the battlefield's
/// palette.** The battlefield is one of `Msg_Pump`'s screens, so a notice still
/// in the ring when a battle begins opens straight over it, and the scroll
/// names no palette of its own.
///
/// **This one stops at the name, and why is a second defect, not this one.**
/// `T32_bat1.256` is read into `Assets::battle` (`l2_view::scene::BattleAssets`)
/// and **not** into the shell's palette map, which is the only place
/// `Machine::present` looks — so the name resolves to nothing and the presenter
/// falls back to the campaign palette for *every* battlefield frame, window or
/// no window. That is the player's *"blue grainy madness"*, it predates this
/// change, and it is left to the battlefield's own work. When it is fixed, this
/// test should present and compare colours the way the one above does.
#[test]
fn a_message_that_opens_over_the_battlefield_asks_for_the_battlefields_palette() {
    let a = Assets::placeholder();
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.player = 1;
    g.battle = Some(Box::new(battle()));
    let mut m = Machine::new(ScreenId::Battlefield);
    assert_eq!(m.palette_name(), Some(l2_view::scene::TILE_PALETTE));

    let mut rec = message::Record::default();
    rec.group = 130;
    rec.category = message::category::COUNTY_NOTICE;
    rec.to = g.player;
    assert!(g.messages.enqueue(rec, g.player), "the notice is in the ring");
    tick(&mut m, &mut g, &a);
    assert_eq!(m.ids(), vec![ScreenId::Battlefield, ScreenId::Message], "Msg_Pump opened it");
    assert_eq!(
        m.palette_name(),
        Some(l2_view::scene::TILE_PALETTE),
        "the scroll names no palette, so the page under it does"
    );
}

/// **A screen change between drawing and presenting would flash one frame**,
/// and the shell's guard is the only thing that stops it.
///
/// `main.rs` draws when `Machine::take_dirty` says something changed, asks the
/// window to redraw, and presents when `RedrawRequested` comes back. Anything
/// `winit` delivers in between — a click, a key — reaches `Machine::handle` and
/// can change the page; `Machine::present` then reads the palette off the
/// **live** stack while the canvas still holds the page before it. That is this
/// file's defect in reverse: the old page's indices under the new page's
/// colours.
///
/// `present` cannot be made to remember instead. `Screen::fade` and a film's
/// `live_palette` both change **with no redraw at all** — the end-of-turn fade
/// is entirely a palette effect (`FUN_004B0CB4`) — so a presenter that used the
/// palette of the last draw would freeze both. The order is the fix, and it
/// belongs where the order is.
///
/// Two halves, because the shell is a binary:
///
/// * the flash is **measured** here, on `Machine::present`;
/// * the guard is read out of `main.rs`, the one artefact the application and
///   this test share — as `tests/movies.rs` reads the start-up call.
///
/// **Ablation:** delete the `take_dirty` guard from `App::present` and the
/// second half goes red; make `Machine::present` ignore the live stack and the
/// first half does.
#[test]
fn a_page_change_between_the_draw_and_the_present_would_flash() {
    let a = assets!();
    let campaign = a.palette.clone();
    let armoury = a.shell.palette("Armoury.256").expect("armoury.256 is in the install").clone();
    assert!(
        (0..=255u8).any(|i| campaign.rgb(i) != armoury.rgb(i)),
        "the two palettes differ, or nothing here means anything"
    );

    let mut g = Game::new(11);
    g.kingdom.set_county_count(3);
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[1].population = 1_000;
    g.kingdom.counties[1].happiness = 90;
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].weapons = [20; 6];
    g.player = 1;
    g.selected = 1;

    let mut m = Machine::new(ScreenId::Campaign);
    // The frame the shell drew: the campaign map, in the campaign palette.
    let mut canvas = Canvas::screen();
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        m.draw(&ctx, &mut canvas);
    }
    let mut drawn = vec![0u8; 640 * 480 * 4];
    m.present(&a, &canvas, &mut drawn);

    // R between `request_redraw` and `RedrawRequested`: the levy window, which
    // reads `Armoury.256`.
    send(&mut m, &mut g, &a, Event::KeyDown(Key::letter('r')));
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)), "the page changed, the canvas did not");
    assert!(m.take_dirty(), "and the change marked the machine dirty - what the guard keys on");

    // Presenting the same canvas now is the flash.
    let mut flashed = vec![0u8; 640 * 480 * 4];
    m.present(&a, &canvas, &mut flashed);
    let differing = PROBES.iter().filter(|&&p| shown(&drawn, p) != shown(&flashed, p)).count();
    assert!(
        differing > 0,
        "the same plane of indices presented two ways is the same colour at all 8 probes, \
         so this test cannot see a flash"
    );

    // And the guard is in the shell, where the order is.
    let main_rs = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main/mod.rs"),
    )
    .expect("main.rs is beside this test");
    let present = main_rs.split("fn present(&mut self)").nth(1).expect("App::present");
    let body = &present[..present.find("fn redraw").unwrap_or(present.len())];
    assert!(
        body.contains("if self.machine.take_dirty()") && body.contains("self.redraw()"),
        "App::present redraws what went dirty before it presents"
    );
}

