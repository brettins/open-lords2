

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_game::shell::font::{self, Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::Canvas;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there are no fonts to draw with");
        };
        let platform = Platform::builder()
            .base(&dir)
            .build()
            .expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

mod top_bar;
pub use top_bar::*;
mod panels;
pub use panels::*;
mod county_and_units;
pub use county_and_units::*;
mod png_part;
pub use png_part::*;

pub(crate) fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

fn find_in(canvas: &Canvas, f: &Font, s: &str, colour: u8) -> Option<(i32, i32)> {
    find_styled(
        canvas,
        f,
        s,
        &Style {
            colour,
            shadow: None,
            caps: None,
        },
    )
}

/// **The title page needs this and it is not a nicety.** `FUN_0041EA14` sets
/// `DAT_0058FE2C` around the heading, which draws `A` … `Z` in **colour 1**
/// instead of the caller's — so *"Lords of the Realm 2"* is two colours, and a
/// probe that expects one finds nothing while the line is plainly on screen.
fn find_styled(canvas: &Canvas, f: &Font, s: &str, style: &Style) -> Option<(i32, i32)> {
    let w = f.width(s).max(1);
    let h = f.height(s).max(1) + 2;
    let mut probe = Canvas::new(w as usize, h as usize);
    f.draw(&mut probe, 0, 1, s, style);
    let wanted: Vec<(i32, i32, u8)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| (x, y, probe.at(x as usize, y as usize)))
        .filter(|&(_, _, c)| c != 0)
        .collect();
    if wanted.is_empty() {
        return None;
    }
    for oy in 0..=(canvas.height as i32 - h) {
        'next: for ox in 0..=(canvas.width as i32 - w) {
            for &(x, y, c) in &wanted {
                if canvas.at((ox + x) as usize, (oy + y) as usize) != c {
                    continue 'next;
                }
            }
            return Some((ox, oy + 1));
        }
    }
    None
}


