//! 1. **The colours.** `Res_LoadStatic` (`0x00499859`) preloads record 2 of
//!    `g_preloadTable` (`0x004D9F48`), `t32_bat1.256`, into `0x00568EE0`, and
//!    `Screen_DrawBattlefield` (`0x004233F7`) ends a field battle's repaint with
//!    `Palette_Set(0x568EE0)`. Ours named that file and never loaded it, so the
//!    presenter fell back to `base01.256`.
//!
//! 2. **The snap.** `BattleMan_Step` (`0x0048F1DD`) enters the next cell
//!    *first* — `FUN_00491B1F` rewrites `mapX`/`mapY` — and then counts `walking`
//!    (`+0x32`) up 1, 3 … 15 while `BattleFigure_Draw` (`0x004BDC31`) trails the
//!    man behind the cell he is already in. Our simulation counts first and
//! enters last, and the picture applied the trailing offset to the cell he
//!    was *leaving*: up to 28 pixels behind his own square, then a jump.
//!
//! 3. **The ghosts.** `BattleFigure_Draw` clips every man to
//!    `Clip_Horizontal(0, 480)` / `Clip_Vertical(24, 472)`, the viewport
//!    `FUN_004BC020` stores. Ours blitted unclipped, into the menu bar, the
//! right column and the bottom strip, which nothing on this screen repaints.
//!
//! `docs/agents.md`, *compute the probe from the constant you are ablating*.

mod render;
pub use render::*;
mod motion;
pub use motion::*;
mod column;

mod band;
pub use band::*;

mod plates;
pub use column::*;
mod panel;
pub use panel::*;
mod entities;
pub use entities::*;
mod ui;
pub use ui::*;

use l2_formats::{DecodedFrame, Palette};
use l2_game::battlefield::{self as bf, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::menubar;
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::terrain::DIM;
use l2_sim::{Motion, Troop, SIDE_A, SIDE_B};
use l2_view::sheet::Sheet;
use l2_view::Canvas;


/// `FUN_004BC020(…, 0x50, 0x50, 8, 0, 0x18, 0xF, 0xE, 0x20)` stores
/// `DAT_004E6564 = 0`, `DAT_004E5D54 = 15 × 32 + 0`, `DAT_004E5D48 = 0x18` and
/// `DAT_004E5D6C = 14 × 32 + 0x18`: the clip every battle sprite is drawn
/// through. Written out, not read from `l2_view::scene`.
const FIELD_X0: i32 = 0;
const FIELD_Y0: i32 = 24;
const FIELD_X1: i32 = 480;
const FIELD_Y1: i32 = 472;


fn field_at(ai: (usize, usize)) -> l2_sim::Battlefield {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[36 * DIM + 40] = 0x04;
    layer[ai.1 * DIM + ai.0] = 0x0F;
    l2_sim::terrain::build(&layer, 1)
}

fn field(ai_row: usize) -> l2_sim::Battlefield {
    field_at((40, ai_row))
}

fn staged(
    ai_row: usize,
    human: &[(Troop, u16)],
    ai: &[(Troop, u16)],
    cam_of: impl Fn((u8, u8)) -> (i32, i32),
) -> (Game, Machine) {
    staged_on(field(ai_row), human, ai, cam_of)
}

fn staged_on(
    field: l2_sim::Battlefield,
    human: &[(Troop, u16)],
    ai: &[(Troop, u16)],
    cam_of: impl Fn((u8, u8)) -> (i32, i32),
) -> (Game, Machine) {
    let runner = BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    let first = runner.fighters.iter().find(|f| f.side == SIDE_A).expect("a human figure");
    let cam = cam_of((first.x, first.y));
    let mut live = LiveBattle::new(runner, 0, 0, 0, None, 1, 1);
    live.paused = false;
    live.cam = cam;
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.prefs.tool_tips = false;
    g.player = 1;
    g.battle = Some(Box::new(live));
    (g, Machine::new(ScreenId::Battlefield))
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn frame(m: &mut Machine, g: &mut Game, a: &Assets, canvas: &mut Canvas) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
    m.draw(&ctx, canvas);
}

pub(crate) fn paint(m: &mut Machine, g: &mut Game, a: &Assets, canvas: &mut Canvas) {
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, canvas);
}

fn live(g: &Game) -> &LiveBattle {
    g.battle.as_deref().expect("a live battle")
}

fn pixel(live: &LiveBattle, cell: (u8, u8)) -> (i32, i32) {
    let (cx, cy) = (cell.0 as i32 - live.cam.0, cell.1 as i32 - live.cam.1);
    assert!(
        (0..bf::VIEW_COLS).contains(&cx) && (0..bf::VIEW_ROWS).contains(&cy),
        "cell {cell:?} is off screen with the camera at {:?}",
        live.cam
    );
    (bf::VIEW.x + cx * bf::TILE + bf::TILE / 2, bf::VIEW.y + cy * bf::TILE + bf::TILE / 2)
}

fn click_at(m: &mut Machine, g: &mut Game, a: &Assets, (x, y): (i32, i32)) {
    send(m, g, a, Event::Pointer { x, y });
    send(m, g, a, Event::Click { x, y });
    send(m, g, a, Event::Release { x, y });
}

fn send_man(m: &mut Machine, g: &mut Game, a: &Assets, man: usize, to: (u8, u8)) {
    let f = &live(g).runner.fighters[man];
    let at = pixel(live(g), (f.x, f.y));
    click_at(m, g, a, at);
    assert_eq!(live(g).runner.selected_fighters(1), vec![man], "the click did not pick him");
    let (ox, oy) = (bf::OVERVIEW.x + 2 * to.0 as i32, bf::OVERVIEW.y + 2 * to.1 as i32);
    click_at(m, g, a, (ox, oy));
    // A right release on the field drops the selection (`FUN_0043C55C`) and
    // keeps the order, so no selection marker is painted over the man.
    send(m, g, a, Event::Pointer { x: 200, y: 200 });
    send(m, g, a, Event::RightClick { x: 200, y: 200 });
    assert!(live(g).runner.selected_fighters(1).is_empty(), "the right click kept him picked");
}

fn human_figure(g: &Game) -> usize {
    live(g).runner.fighters.iter().position(|f| f.side == SIDE_A).expect("a human figure")
}

fn install() -> Option<(Assets, l2_mods::Platform)> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    Some((assets, platform))
}

