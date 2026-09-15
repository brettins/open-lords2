//! The original has one display palette and only a **painter** writes it —
//! `Screen_Armoury` ends with `Palette_Set(armoury.256)`,
//! `Screen_DrawBattlefield` (`0x004233F7`) sets `T32_bat1.256`. Nothing drawn
//! over a page touches it: `Tip_Show`
//! (`0x00476DA9`) saves `g_screenId`, writes `0x27` and posts; `FUN_00476E21`
//! puts the byte back; `Msg_DrawWindow` (`0x0047309E`) calls no `Palette_Set`.
//!
//! so the
//! backdrop behind a tip is the backdrop, in its own colours. `[V]`


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

mod overlay_palette;
pub use overlay_palette::*;

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

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

const PROBES: [(i32, i32); 8] =
    [(8, 8), (40, 60), (600, 40), (624, 200), (40, 300), (600, 300), (20, 470), (560, 470)];

fn outside(r: (i32, i32, i32, i32), (x, y): (i32, i32)) -> bool {
    !(x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3)
}

