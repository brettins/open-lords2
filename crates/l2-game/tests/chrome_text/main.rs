//! **The text a player looks at constantly**: the menu bar's clock and treasury,
//! and the front end's title page.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test chrome_text
//! ```
//!
//! Every assertion here is *install-gated on purpose*. The defect this file was
//! written for is invisible without the real fonts: with no install every `Pen`
//! method falls back to `l2_view::text` and a screen that never calls a `Pen` at
//! all looks exactly like one that does. `docs/agents.md` — *five defects this
//! week existed only against real assets*.

mod top_bar;
pub use top_bar::*;
mod panels;
pub use panels::*;
mod county_and_units;
pub use county_and_units::*;
mod png_part;
pub use png_part::*;

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

pub(crate) fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// Where a string drawn in `font` at `colour` sits on the canvas, by its exact
/// pattern of set pixels.
///
/// **This is the whole instrument, and it is why these tests can be about the
/// game's own fonts.** `screens.rs`'s `find_text` renders the probe in
/// `l2_view::text`, our 5 × 7 font — so it can only ever find text drawn in
/// *that* font, and a screen that switched to `Fntl2_14.pl8` would make it
/// return `None` while looking like a missing draw call. Here the probe is
/// rendered with the same [`Font`] the assertion claims drew it, so finding it
/// **is** the claim "this was drawn in this face".
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

/// The same, for a string the painter draws with a [`Style`] of its own.
///
/// **The title page needs this and it is not a nicety.** `FUN_0041EA14` sets
/// `DAT_0058FE2C` around the heading, which draws `A` … `Z` in **colour 1**
/// instead of the caller's — so *"Lords of the Realm 2"* is two colours, and a
/// probe that expects one finds nothing while the line is plainly on screen.
/// Matching the probe's own per-pixel colours is the claim *"drawn in this
/// face, in this mode"*, which is stronger than either half.
fn find_styled(canvas: &Canvas, f: &Font, s: &str, style: &Style) -> Option<(i32, i32)> {
    // Probe on 0, which no style below writes, and pad a row above and below so
    // an embossed probe has somewhere to put its shadows.
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
    // **Inclusive on both bounds.** The build stamp sits with its last row two
    // pixels off the bottom, which is exactly the offset an exclusive range
    // cannot reach — the check reported the stamp missing while a screenshot
    // showed it, which is a false negative in a test written to catch a false
    // positive.
    for oy in 0..=(canvas.height as i32 - h) {
        'next: for ox in 0..=(canvas.width as i32 - w) {
            for &(x, y, c) in &wanted {
                if canvas.at((ox + x) as usize, (oy + y) as usize) != c {
                    continue 'next;
                }
            }
            // `+ 1` undoes the padding row, so the answer is the `y` the
            // painter passed.
            return Some((ox, oy + 1));
        }
    }
    None
}

// ------------------------------------------------------- the menu bar's chrome

