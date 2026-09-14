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
//! so the
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
/// windows it up, and says how many survived.
const PROBES: [(i32, i32); 8] =
    [(8, 8), (40, 60), (600, 40), (624, 200), (40, 300), (600, 300), (20, 470), (560, 470)];

fn outside(r: (i32, i32, i32, i32), (x, y): (i32, i32)) -> bool {
    !(x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3)
}

