//! The three screens, drawn and driven against a real install — headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game
//! ```
//!
//! **No window is opened.** Every assertion is on the canvas's `Vec<u8>` of
//! palette indices, which is the same shape the eventual pixel diff against
//! `Lords2.exe`'s framebuffer will take. A screen that had to be looked at to
//! be checked would be a screen that stops being checked.
//!
//! Several tests below read text back off the canvas with [`find_text`], which
//! renders the string it is looking for and searches for that exact pattern of
//! ink. It is paired every time with a near-miss that must *not* be found, so
//! "the panel shows 435" cannot pass by finding some other number.

#![allow(dead_code)]

pub use std::path::PathBuf;
pub use l2_game::game::Assets;
pub use l2_game::input::Event;
pub use l2_game::screen::Ctx;
pub use l2_game::screen::Machine;
pub use l2_game::screen::Screen;
pub use l2_game::screen::ScreenId;
pub use l2_game::screen::Transition;
pub use l2_game::screens::map::MapScreen;
pub use l2_game::Game;
pub use l2_view::text;
pub use l2_view::Canvas;

pub fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = $crate::common::install() else {
            l2_testkit::skip!("no game install, so there are no assets to draw with");
        };
        let platform =
            ::l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = ::l2_game::game::Assets::load(&platform.vfs).expect("assets load");
        // The **assets** come from the install and the **position** comes from
        // the named fixture. They used to come from the same place, and every
        // number below - fourteen counties, the treasury, the selected county -
        // is the England turn-one position's.
        let save = l2_testkit::england!();
        let mut game = ::l2_game::scenario::from_save(&save, ::l2_kingdom::tables::Tables::DEFAULT)
            .expect("the fixture loads");
        // **Tip screens: No.** A new game's tips come up over the campaign map
        // and hold its input on screen `0x27`; that is `tests/tips.rs`'s
        // subject, and every screen here is drawn and driven without them.
        game.prefs.tip_screens = false;
        (game, assets)
    }};
}

pub fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

pub fn send<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets, e: Event) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(e, &mut ctx)
}

/// One event into a whole [`Machine`], which is the only way to exercise an arm
/// that a screen answers with [`Transition::Pass`].
///
/// **Half the county sidebar is one of those.** `Screen_FrameInput`'s arms for
/// the village and for the four county panels open with six guards belonging to
/// the campaign map, so an assertion made against a bare `CountyScreen` cannot
/// see them at all — which is how the panels came to swallow the entire
/// right-hand column with a green suite.
pub fn send_stack(m: &mut Machine, game: &mut Game, assets: &Assets, e: Event) {
    let mut ctx = Ctx { game, assets };
    m.handle(e, &mut ctx);
}

pub fn draw_stack(m: &mut Machine, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    m.draw(&ctx, &mut canvas);
    canvas
}

/// The campaign map with one screen opened over it, which is what every county
/// panel, the village and the job popup are.
pub fn over_the_map(over: ScreenId) -> Machine {
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(over);
    m
}

/// Find a string drawn in `colour`, returning its top-left. Only the glyphs'
/// *set* pixels are matched; what is behind the letters is the panel's
/// business.
pub fn find_text(canvas: &Canvas, s: &str, colour: u8) -> Option<(i32, i32)> {
    let w = text::width(s);
    let h = text::GLYPH_H;
    let mut wanted: Vec<(i32, i32)> = Vec::new();
    let mut probe = Canvas::new(w.max(1) as usize, h as usize);
    text::draw(&mut probe, 0, 0, s, 1);
    for y in 0..h {
        for x in 0..w {
            if probe.at(x as usize, y as usize) == 1 {
                wanted.push((x, y));
            }
        }
    }
    if wanted.is_empty() {
        return None;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            // Cheap rejection on the first ink pixel before the full compare.
            let (fx, fy) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != colour {
                continue;
            }
            if wanted.iter().all(|(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour)
            {
                return Some((ox, oy));
            }
        }
    }
    None
}

/// The same search, but for a string drawn in one of the **original's** fonts.
///
/// The county strip is drawn in `Fntl2_9.pl8` — the only place in the game that
/// font is used — so a search cannot see
/// it. The probe is the same idea: render the string, keep the set pixels,
/// scan for that pattern.
pub fn find_font_text(
    canvas: &Canvas,
    font: &l2_game::shell::font::Font,
    s: &str,
    colour: u8,
) -> Option<(i32, i32)> {
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let w = font.width(s).max(1);
    let h = font.height(s).max(1);
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    let mut wanted: Vec<(i32, i32)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if probe.at(x as usize, y as usize) == 1 {
                wanted.push((x, y));
            }
        }
    }
    if wanted.is_empty() {
        return None;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            let (fx, fy) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != colour {
                continue;
            }
            if wanted.iter().all(|(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour)
            {
                return Some((ox, oy));
            }
        }
    }
    None
}

/// One line of the county strip, found in whichever font drew it: the
/// original's 9-pixel one where the install has it, ours where it does not.
/// **What the county strip's text is drawn in**: the literal `0x3F`
/// every `Ui_DrawText` call in `CountyStrip_Draw` passes, which is `rgb(0,0,0)`
/// in `Base01.256`. These assertions used to look for `ink.text` — white — and
/// a player reported the strip as white-on-parchment before anyone read the
/// argument.
pub const STRIP_INK: u8 = l2_game::shell::font::TEXT;

pub fn find_strip(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.small.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

/// One line of the strip drawn in the **body** font — the county's name, the
/// three "sovereign land of …" lines on a county you do not hold, and every
/// body line of the four county panels since they graduated to [`Pen`].
pub fn find_body(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.body.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

/// The same in the **heading** font, `Fntl2_22.pl8` — which the county panels'
/// titles and their three plain-number rows are drawn in
/// (`Eng_DrawString(…, &g_fontHeading, …)`).
pub fn find_heading(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.heading.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

// ---------------------------------------------------------------------------
// The field brush
// ---------------------------------------------------------------------------

/// Centre the map on a tile and give back its screen position.
pub fn on_screen(screen: &mut MapScreen, tile: usize) -> (i32, i32) {
    let (x, y) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(x as usize, y as usize);
    l2_view::campaign::tile_centre(screen.viewport(), screen.zoom(), x as usize, y as usize)
        .expect("a tile the viewport was just centred on is in the viewport")
}

/// How wide a string draws in whichever font [`find_body`] would have found it
/// in — the install's `Fntl2_14.pl8` where there is one, our own 5 × 7 fallback
/// where there is not. Both `Pen::body_centred` arms measure this way, so an
/// expectation built on it is the same expectation in both worlds.
pub fn body_width(assets: &Assets, s: &str) -> i32 {
    match assets.shell.body.as_ref() {
        Some(f) => f.width(s),
        None => text::width(s),
    }
}

/// A rectangle of `canvas`, lifted out so a search cannot match something
/// elsewhere on the screen. A one-digit column is otherwise unfindable: `"0"`
/// appears in half a dozen places on a county panel.
pub fn crop(canvas: &Canvas, x: i32, y: i32, w: i32, h: i32) -> Canvas {
    let mut out = Canvas::new(w as usize, h as usize);
    for row in 0..h {
        for col in 0..w {
            let (sx, sy) = (x + col, y + row);
            if sx < 0 || sy < 0 || sx >= canvas.width as i32 || sy >= canvas.height as i32 {
                continue;
            }
            out.set(col as usize, row as usize, canvas.at(sx as usize, sy as usize));
        }
    }
    out
}
