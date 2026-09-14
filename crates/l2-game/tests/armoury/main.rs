//! **The armoury against the real artwork**, headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" \
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-game --test armoury
//! ```
//!
//! `crates/l2-game/tests/military/main.rs` walks the whole verb on
//! `Assets::placeholder`, picture agree in. This file is the other half: **the hit map and the sprites
//! picture agree in. This file is the other half: **the hit map and the sprites
//! it is supposed to sit on, out of the install.** `docs/decisions.md` C58 —
//! every campaign-map test on this project ran on the placeholder once, and a
//! near-miss reached a player three times.
//!
//! The strongest assertion here is the last one. It moves **one field of the
//! world** — a realm's stock of one weapon — and requires the *same pixels* to
//! appear and disappear, which is the shape the flag and minimap tests were
//! rewritten into after a diff-in-a-box passed a wrong sprite.

mod hit_map;
pub use hit_map::*;
mod rack;
pub use rack::*;
mod animation;
pub use animation::*;
mod screenshots;
pub use screenshots::*;

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::armoury;
use l2_game::Game;
use l2_kingdom::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_view::Canvas;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no armoury to walk into");
        };
        let platform =
            l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

/// A county the local player holds. The England fixture's realm→county
/// assignment is **rolled per game** (`docs/environment.md`), so this is found
///
fn own_county(g: &Game) -> u8 {
    (1..=g.kingdom.county_count as u8)
        .find(|&id| g.is_players(id))
        .expect("the local player holds a county")
}

fn frame(m: &mut Machine, g: &mut Game, a: &Assets) -> Canvas {
    let mut c = Canvas::screen();
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut c);
    c
}

/// The bounding box of one weapon's cells in `arm_grid.pl8`.
fn grid_box(a: &Assets, weapon: u8) -> Option<Rect> {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in (0..480).step_by(8) {
        for x in (0..640).step_by(8) {
            if a.shell.armoury_grid(x, y) == Some(weapon) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 8);
                y1 = y1.max(y + 8);
            }
        }
    }
    (x1 > x0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

