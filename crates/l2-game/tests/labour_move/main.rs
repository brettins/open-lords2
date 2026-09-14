//! **A drag in the village is `Labour_Move`
//! subtraction.** Five player reports, one mechanism and two beside it.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!   cargo test -p l2-game --test labour_move
//! ```
//!
//! > *"industry values don't seem to update, and for some reason mining started as off."*
//! > *"The labor slider seems to reset each turn so that I have to reassign peasants to
//! > wheat each turn."*
//! > *"wheat does not show the +value when it is about to be harvested, I'm noticing
//! > generally the industry numbers in the sidebar are inaccurate."*
//!
//! `Labour_Move` (`0x00439B52`) moves the workers and then switches the
//! destination industry on (`FUN_00439CC2`), re-runs the food pass and the
//! estimates twice with every blacksmith of the realm (`FUN_00448648`), and
//! rewrites the county's shares from where people now stand. Ours moved the
//! workers and stopped. So a drag changed no forecast, staffed a switched-off
//! mine that stayed switched off, and was dealt back to the old split by the
//! season's `Labour_Allocate`. `docs/decisions.md` C180.
//!
//! **Every test here drives the screens** — a band, a release and a press in the
//! village; End Turn from the keyboard; *Start* on the setup page; a press on a
//! smithy on the map — and reads the answer back where the player reads it
//! wherever a number is drawn. Where a number is typed it is typed from the
//! decompilation (the flat `80` of `Industry_EfficiencyRamp` with *Advanced
//! Farming* off, the site bases `10, 1, 7, 4` of `Industry_UpdateSiteTile`), not
//! from the constant under test; where only a save can settle it, the save is
//! read.

mod drag_and_drop_tests;
pub use drag_and_drop_tests::*;
mod game_flow_tests;
pub use game_flow_tests::*;

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::county as county_screen;
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{
    SetupPage, SetupScreen, CUSTOM_BUTTONS, CUSTOM_BUTTON_Y, MAP_LIST_ROW, MAP_LIST_X, MAP_LIST_Y,
};
use l2_game::screens::village::VillageScreen;
use l2_game::Game;
use l2_kingdom::field::FieldType;
use l2_kingdom::tables::{Commodity, Tables};
use l2_mods::Platform;
use l2_view::village as vill;
use l2_view::{campaign, Canvas};

/// `Industry_EfficiencyRamp` (`FUN_0044F248`)'s first line:
/// `if (!g_optAdvancedFarming) return 80;`. Every fixture on this machine has
/// Advanced Farming off, and each test asserts that premise before using it.
const FLAT_EFFICIENCY: i32 = 80;

/// `Industry_LabourEstimate`'s "as many as you like" for wood, iron and stone.
const UNBOUNDED: i32 = 100_000;

/// `Industry_UpdateSiteTile` (`0x0044EDC2`)'s four bases, in commodity order —
/// wood 10, iron 1, weapons 7, stone 4 — and a working site is `base + 1`.
const SITE_BASE: [u8; 4] = [10, 1, 7, 4];

/// `colourPos` at all four industry rows' `Ui_DrawDelta` call sites.
const POS: u8 = 0xFA;

/// `Ui_DrawDelta(…, 0x22C, pitch*row + 0x139, …)` and the icon's `0x133`.
const DELTA_X: i32 = 0x22C;
const ICON_DY: i32 = 0x133;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

fn assets() -> Option<Assets> {
    let dir = install()?;
    let platform = Platform::builder().base(&dir).build().expect("the install mounts");
    Some(Assets::load(&platform.vfs).expect("assets load"))
}

macro_rules! assets {
    () => {{
        let Some(a) = assets() else {
            l2_testkit::skip!("no game install, so no village grid, fonts or map to drive");
        };
        a
    }};
}

/// `DAT_0053F04C`, the industry column's pitch, typed from `CountyStrip_Draw`.
fn pitch(rows: usize) -> i32 {
    if rows < 3 {
        0x3C
    } else if rows < 4 {
        0x2D
    } else {
        0x1E
    }
}

fn draw_stack(m: &mut Machine, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    m.draw(&ctx, &mut canvas);
    canvas
}

fn handle(m: &mut Machine, game: &mut Game, assets: &Assets, e: Event) {
    let mut ctx = Ctx { game, assets };
    m.handle(e, &mut ctx);
}

/// Every place `s` is drawn in `colour` in `font`, top-left corners.
fn all_text(canvas: &Canvas, font: &l2_game::shell::font::Font, s: &str, colour: u8) -> Vec<(i32, i32)> {
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let (w, h) = (font.width(s).max(1), font.height(s).max(1));
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    let ink: Vec<(i32, i32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| probe.at(x as usize, y as usize) == 1)
        .collect();
    let mut found = Vec::new();
    if ink.is_empty() {
        return found;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            if ink.iter().all(|&(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour) {
                found.push((ox, oy));
            }
        }
    }
    found
}

/// Whether `+value ` is drawn in the industry column's row `row` of `rows`.
fn drawn_in_row(canvas: &Canvas, assets: &Assets, value: i32, row: usize, rows: usize) -> bool {
    let f = assets.shell.ten.as_ref().expect("Font_10.pl8");
    let p = pitch(rows);
    all_text(canvas, f, &format!("+{value} "), POS).into_iter().any(|(x, y)| {
        x >= DELTA_X && y >= p * row as i32 + ICON_DY && y < p * row as i32 + ICON_DY + p
    })
}

/// A band round exactly one icon of `cluster`, a release, and a press on
/// `target` — the three screen ids of the original's gesture, `0x02 → 0x05 →
/// 0x06`, as events. Neighbouring icons are 16 px apart and rows 12 px, so a
/// band nine pixels wide and one tall catches one.
fn drag_one_icon(m: &mut Machine, game: &mut Game, assets: &Assets, county: u8, cluster: usize, target: usize) {
    let c = &game.kingdom.counties[county as usize];
    let icons = VillageScreen::icons(c);
    let slot = (0..vill::ICONS_PER_CLUSTER)
        .find(|&s| icons[cluster][s] != 0 && icons[cluster][s] != vill::ICON_SHORTFALL)
        .expect("the source cluster has a person to pick up");
    let (px, py) = vill::icon_hit_point(cluster, slot, vill::SCENE_Y);
    handle(m, game, assets, Event::Click { x: px - vill::DRAG_DEAD_ZONE, y: py });
    handle(m, game, assets, Event::Pointer { x: px, y: py });
    handle(m, game, assets, Event::Release { x: px, y: py });
    // Where the drop lands is `vill_gd8.pl8`, so the target point is read out
    // of the grid
    let art = assets.village.as_ref().expect("vill_gd8.pl8");
    let (dx, dy) = (vill::SCENE_Y..vill::SCENE_Y + 320)
        .step_by(4)
        .flat_map(|y| (vill::SCENE_X..vill::SCENE_X + 363).step_by(4).map(move |x| (x, y)))
        .find(|&(x, y)| art.cluster_at(x, y, vill::SCENE_Y) == target + 1)
        .expect("the target cluster is somewhere on the drop grid");
    handle(m, game, assets, Event::Click { x: dx, y: dy });
}

fn cluster_of(game: &Game, county: u8, slot: usize) -> usize {
    let slots = VillageScreen::slots(&game.kingdom.counties[county as usize]);
    (0..vill::CLUSTER_COUNT).find(|&i| slots[i] == slot).expect("the job has a cluster")
}

pub(crate) fn end_turn(m: &mut Machine, game: &mut Game, assets: &Assets) {
    let before = game.kingdom.turn_count;
    handle(m, game, assets, Event::KeyDown(Key::Char('E')));
    let mut done_at = None;
    for n in 1..2_000u32 {
        let mut ctx = Ctx { game: &mut *game, assets };
        m.update(&mut ctx);
        if done_at.is_none() && game.kingdom.turn_count > before {
            done_at = Some(n);
        }
        if done_at.is_some_and(|t| n >= t + l2_view::fade::PHASES as u32) {
            return;
        }
    }
    panic!("the turn never came round");
}

