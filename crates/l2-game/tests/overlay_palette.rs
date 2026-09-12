//! **A window over a page is in the page's colours.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test overlay_palette
//! ```
//!
//! Two reports from one player, and one cause. Behind the first tip on the
//! raise-army screen and on castle building: *"the screen behind it is color
//! reversed. Immediately fixes after dismissing the tutorial screen and doesn't
//! return."* And on a battlefield: *"it's all reverse color ...or..something.
//! It's blue grainy madness."*
//!
//! The original has one display palette and only a **painter** writes it —
//! `Screen_Armoury` ends with `Palette_Set(armoury.256)`,
//! `Screen_DrawBattlefield` (`0x004233F7`) sets `T32_bat1.256`. Nothing drawn
//! over a page touches it: `Tip_Show`
//! (`0x00476DA9`) saves `g_screenId`, writes `0x27` and posts; `FUN_00476E21`
//! puts the byte back; `Msg_DrawWindow` (`0x0047309E`) calls no `Palette_Set`.
//! There is no dim, no shade table and no remap in any of the three, so the
//! backdrop behind a tip is the backdrop, in its own colours. `[V]`
//!
//! `Machine::palette_name` asked the **top** screen alone, and the tip host
//! (`0x27`) and the message scroll name no palette — so a page under either was
//! presented through the campaign palette. The canvas never saw it: every index
//! on it was right. The raise-army test therefore asserts **presented colour at
//! fixed pixels**, through `Machine::present`, the function `main.rs` presents
//! with. The battlefield test stops at the palette's name, and its doc comment
//! says why: the battlefield has a second, separate palette defect.
//!
//! **Ablation, run:** put `self.stack.last().and_then(|s| s.palette())` back as
//! the body of `Machine::palette_name` and both tests go red — the first on its
//! colour assertion behind the tip, the second on the name under the scroll.

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

macro_rules! assets {
    () => {{
        let Some(dir) = l2_testkit::install_dir() else {
            l2_testkit::skip!("no game install, so there is no .256 to present a page through");
        };
        let platform =
            l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        Assets::load(&platform.vfs).expect("assets load")
    }};
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

/// Draw the stack, then present it: the plane of indices, and the four bytes a
/// pixel is shown as.
fn frame(m: &mut Machine, g: &mut Game, a: &Assets) -> (Canvas, Vec<u8>) {
    let mut canvas = Canvas::screen();
    {
        let ctx = Ctx { game: g, assets: a };
        m.draw(&ctx, &mut canvas);
    }
    let mut rgba = vec![0u8; 640 * 480 * 4];
    m.present(a, &canvas, &mut rgba);
    (canvas, rgba)
}

fn shown(rgba: &[u8], (x, y): (i32, i32)) -> [u8; 3] {
    let i = (y as usize * 640 + x as usize) * 4;
    [rgba[i], rgba[i + 1], rgba[i + 2]]
}

fn index(c: &Canvas, (x, y): (i32, i32)) -> u8 {
    c.at(x as usize, y as usize)
}

/// Fixed probes, spread round the edges of the screen where neither the levy
/// window nor a tip window reaches. Each test still filters them against the
/// windows it actually has up, and says how many survived.
const PROBES: [(i32, i32); 8] =
    [(8, 8), (40, 60), (600, 40), (624, 200), (40, 300), (600, 300), (20, 470), (560, 470)];

fn outside(r: (i32, i32, i32, i32), (x, y): (i32, i32)) -> bool {
    !(x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3)
}

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
