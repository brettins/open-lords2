//! The tile panel's castle and field arms draw `L2.eng` groups 71 and 77 out
//! of a county — `TileInfo_DrawCastle` shares
//! `Castle_DrawStatusBlock` with `FUN_00414220` — so they are measured with
//! these helpers.
//!
//! C164 carried the figures these painters draw and left the painters as stubs.
//!
//! Every assertion below is **one figure or one word, in its own box, at the
//! painter's own coordinates** — never a whole-canvas diff, which can pass when a
//! panel's height changes too. Every coordinate that starts a line is a literal
//! out of the decompilation; a piece chained after it is placed by the font's
//! own measure and `Ui_DrawText`'s two rules (C155): a character with no glyph
//! advances four, and every call adds a four-pixel trailer. Nothing here is
//! computed from a constant in `screens/job.rs` or `screens/message.rs`.


use l2_game::game::Assets;
use l2_game::message::category;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::job::JobScreen;
use l2_game::shell::font::{Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::event::{EventKind, RealmPurse};
use l2_kingdom::field::FieldType;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::tables::{
    Commodity, Season, Tables, Weather, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING,
    JOB_FIELD_RECLAMATION, JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING,
    JOB_WOOD_CUTTING,
};
use l2_mods::Platform;
use l2_view::Canvas;


macro_rules! install_assets {
    () => {{
        let Some(dir) = l2_testkit::install_dir() else {
            l2_testkit::skip!("no game install, so there are no fonts and no L2.eng");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        Assets::load(&platform.vfs).expect("assets load")
    }};
}

macro_rules! england_world {
    () => {{
        let assets = install_assets!();
        let save = l2_testkit::england!();
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        game.prefs.tip_screens = false;
        (game, assets)
    }};
}

macro_rules! fixture_world {
    ($name:expr) => {{
        let assets = install_assets!();
        let save = l2_testkit::fixture!($name);
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        game.prefs.tip_screens = false;
        (game, assets)
    }};
}

mod job_popups;
pub use job_popups::*;
mod events_and_letters;
pub use events_and_letters::*;
mod tile_panel;
pub use tile_panel::*;

fn draw_job(game: &mut Game, assets: &Assets, county: usize, job: usize) -> Canvas {
    let mut screen = JobScreen::new(county as u8, job);
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

const INK: u8 = 0x3F;
const NEG: u8 = 0xF9;

fn body(a: &Assets) -> &Font {
    a.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install")
}

fn heading(a: &Assets) -> &Font {
    a.shell.heading.as_ref().expect("Fntl2_22.pl8 is in the install")
}

/// One `L2.eng` string out of the install, which must be there.
#[track_caller]
fn eng(a: &Assets, group: usize, index: usize) -> String {
    let s = a.shell.text(group, index).to_string();
    assert!(!s.is_empty(), "setup: L2.eng {group}/{index} is empty");
    s
}

fn is_at(canvas: &Canvas, f: &Font, s: &str, colour: u8, x: i32, y: i32) -> bool {
    let (w, h) = (f.width(s).max(1), f.height(s).max(1));
    let mut probe = Canvas::new(w as usize, h as usize);
    f.draw(&mut probe, 0, 0, s, &Style { colour: 1, shadow: None, caps: None });
    let mut any = false;
    for py in 0..h {
        for px in 0..w {
            if probe.at(px as usize, py as usize) != 1 {
                continue;
            }
            any = true;
            let (cx, cy) = (x + px, y + py);
            if cx < 0 || cy < 0 || cx >= canvas.width as i32 || cy >= canvas.height as i32 {
                return false;
            }
            if canvas.at(cx as usize, cy as usize) != colour {
                return false;
            }
        }
    }
    any
}

fn xs_on_row(canvas: &Canvas, f: &Font, s: &str, colour: u8, y: i32) -> Vec<i32> {
    (0..canvas.width as i32).filter(|&x| is_at(canvas, f, s, colour, x, y)).collect()
}

#[track_caller]
fn expect_at(canvas: &Canvas, f: &Font, s: &str, colour: u8, x: i32, y: i32) {
    assert!(
        is_at(canvas, f, s, colour, x, y),
        "{s:?} in {colour:#04x} is not at ({x:#x}, {y:#x}); on that row it is at {:?}",
        xs_on_row(canvas, f, s, colour, y)
    );
}

/// `Ui_DrawText`'s advance over `s`: four for a character with no glyph, the
/// measure for the rest, and the call's four-pixel trailer. C155.
fn advance(f: &Font, s: &str) -> i32 {
    s.chars()
        .map(|ch| match f.width(&ch.to_string()) {
            0 => 4,
            w => w,
        })
        .sum::<i32>()
        + 4
}

#[track_caller]
fn word_at(canvas: &Canvas, a: &Assets, group: usize, index: usize, x: i32, y: i32) -> i32 {
    let s = eng(a, group, index);
    expect_at(canvas, body(a), &s, INK, x, y);
    x + advance(body(a), &s)
}

/// `Ui_DrawCount(value, noun, x, y)`: the digits one blank sign column right of
/// `x`, and group 8's singular or plural one trailer after them. Returns where
/// the next piece starts.
#[track_caller]
fn count_at(canvas: &Canvas, a: &Assets, value: i32, noun: usize, x: i32, y: i32) -> i32 {
    let f = body(a);
    let digits = value.to_string();
    expect_at(canvas, f, &digits, INK, x + 4, y);
    let noun = eng(a, 8, if value.abs() == 1 { noun } else { noun + 1 });
    let noun_x = x + 4 + advance(f, &digits);
    expect_at(canvas, f, &noun, INK, noun_x, y);
    noun_x + advance(f, &noun)
}

#[track_caller]
fn signed_at(canvas: &Canvas, a: &Assets, value: i32, y: i32) {
    let f = body(a);
    match value {
        0 => expect_at(canvas, f, "0", INK, 0x130 + 4, y),
        v if v < 0 => expect_at(canvas, f, &format!("-{}", -v), NEG, 0x130, y),
        v => expect_at(canvas, f, &format!("+{v}"), INK, 0x130, y),
    }
}

#[track_caller]
fn county_where(k: &Kingdom, what: &str, pick: impl Fn(&l2_kingdom::county::County) -> bool) -> usize {
    (1..=k.county_count)
        .find(|&id| pick(&k.counties[id]))
        .unwrap_or_else(|| panic!("setup: no county in this fixture is {what}"))
}

fn paint_all_fallow_to_grain(k: &mut Kingdom, county: usize) -> i32 {
    let tiles: Vec<usize> = k
        .field_tiles(county)
        .into_iter()
        .filter(|&(_, kind)| kind == FieldType::Fallow)
        .map(|(tile, _)| tile)
        .collect();
    for &tile in &tiles {
        k.paint_field(county, tile, FieldType::Grain).expect("a fallow field takes the grain brush");
    }
    tiles.len() as i32
}

