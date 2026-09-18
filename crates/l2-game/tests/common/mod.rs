
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
        let save = l2_testkit::england!();
        let mut game = ::l2_game::scenario::from_save(&save, ::l2_kingdom::tables::Tables::DEFAULT)
            .expect("the fixture loads");
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

pub fn over_the_map(over: ScreenId) -> Machine {
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(over);
    m
}

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

pub const STRIP_INK: u8 = l2_game::shell::font::TEXT;

pub fn find_strip(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.small.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

pub fn find_body(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.body.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

pub fn find_heading(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.heading.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}


pub fn on_screen(screen: &mut MapScreen, tile: usize) -> (i32, i32) {
    let (x, y) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(x as usize, y as usize);
    l2_view::campaign::tile_centre(screen.viewport(), screen.zoom(), x as usize, y as usize)
        .expect("a tile the viewport was just centred on is in the viewport")
}

pub fn body_width(assets: &Assets, s: &str) -> i32 {
    match assets.shell.body.as_ref() {
        Some(f) => f.width(s),
        None => text::width(s),
    }
}

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
