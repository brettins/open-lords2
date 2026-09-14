#![allow(unused_imports)]
use super::*;
use super::behavior::*;
use super::battle::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::options::{self, Setting};
use l2_game::shell::Pen;
use l2_game::tooltip::{self, Shown, Sidebar};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_view::Canvas;

// ------------------------------------------------------------------ the box

pub(crate) fn draw(m: &mut Machine, g: &mut Game, a: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut canvas);
    canvas
}

fn region(c: &Canvas, r: Rect) -> Vec<u8> {
    let mut out = Vec::new();
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            out.push(c.at(x as usize, y as usize));
        }
    }
    out
}

/// **The words, in their own box** — compared region for region with a box
/// the test draws itself from the literal string, not with a canvas.
///
/// With the placeholder font a glyph advances 6 pixels, so *"End your turn"*
/// ends 78 pixels on plus `Ui_DrawText`'s trailing 4: `g_penAdvance` 82, and
/// `0xC - (0xB0 - 82) / 16` is 7 units — a 112 × 22 box at (340, 440), one line
/// tall.
///
/// Ablations: delete the second pass's text and the region loses its words;
/// delete the fill and the first pass shows through on the map; swap `FILL`
/// for `INK` — each goes red.
#[test]
fn the_tip_is_its_string_in_a_filled_outlined_box() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    ticks(&mut m, &mut g, &a, 64);
    let s = shown(&m).expect("a tip is up");
    let (rect, lines) = {
        let ctx = Ctx { game: &mut g, assets: &a };
        tooltip::layout(&ctx, s)
    };
    assert_eq!(rect, Rect::new(340, 440, 112, 22));
    assert_eq!(lines, vec!["End your turn".to_string()]);

    let got = region(&draw(&mut m, &mut g, &a), rect);

    let mut want = Canvas::screen();
    want.fill_rect(340, 440, 112, 22, 0x20);
    let pen = Pen { assets: &a.shell, ink: &a.ink, chrome: a.chrome.as_ref(), shadow: None, caps: None };
    pen.body(&mut want, 344, 444, "End your turn", 0x3F);
    want.fill_rect(340, 440, 112, 1, 0x3F);
    want.fill_rect(340, 461, 112, 1, 0x3F);
    want.fill_rect(340, 440, 1, 22, 0x3F);
    want.fill_rect(451, 440, 1, 22, 0x3F);
    assert_eq!(got, region(&want, rect), "the box is not End Turn's words in a 0x20 box outlined 0x3F");
}

/// **The pure geometry, from literals** — `FUN_00477131`'s two offsets,
/// `FUN_00477249`'s clamp, and `FUN_00476E95`'s size rule.
///
/// Ablations: remove the `y > 0x1B7` clamp and the bottom-edge row goes red;
/// change `0x141` to `0x140` and the middle row does.
#[test]
fn the_box_is_placed_and_sized_the_way_the_original_places_it() {
    for (pointer, want) in [
        ((100, 100), (130, 130)), // right of and below the pointer
        ((320, 240), (350, 270)), // the last column and row that are
        ((321, 241), (101, 211)), // the first that flip
        ((620, 478), (400, 440)), // bottom right: 448 clamped to 440
        ((639, 479), (419, 440)),
        ((0, 0), (30, 30)),
    ] {
        assert_eq!(tooltip::place(pointer.0, pointer.1), want, "pointer {pointer:?}");
    }
    // One line is 22 tall and two are 40; a line of 176 is 12 units, and C
    // division rounds (176 - 178) / 16 to zero, not to minus one.
    assert_eq!(tooltip::box_size(1, 82), (112, 22));
    assert_eq!(tooltip::box_size(2, 178), (192, 40));
    assert_eq!(tooltip::box_size(3, 176), (192, 40));
    // (176 - 20) / 16 is 9, so 3 units.
    assert_eq!(tooltip::box_size(1, 20), (48, 22));
}

/// **A two-line tip in the bottom-right corner stays on the screen** — the
/// battlefield's Autocalc button, `FUN_004777AA`'s id 30.
///
/// In the placeholder font *"Autocalculate battle or siege results"* breaks
/// after *"siege"* at both wraps — 142 + 39 is 181 — so the box is 40 tall;
/// `FUN_00477249` pulls it up from 448 to 440, and it ends on the screen's last
/// row. Every pixel the box writes must be inside the canvas: *"is it drawn"*
/// and *"can it be seen"* are different claims (`docs/agents.md`).
///
/// Ablation: remove the `y > 0x1B7` clamp and the box ends at 488, off the
/// screen; the containment assertion goes red.
#[test]
fn a_two_line_tip_at_the_bottom_right_corner_stays_on_the_screen() {
    let (mut g, a, mut m) = battlefield();
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the field is up");
    point(&mut m, &mut g, &a, 620, 478);
    ticks(&mut m, &mut g, &a, 64);
    let s = shown(&m).expect("the battlefield's ladder answers");
    assert_eq!(s.id, 30);
    assert_eq!(tooltip::words(&a.shell, 30), "Autocalculate battle or siege results");
    let (rect, lines) = {
        let ctx = Ctx { game: &mut g, assets: &a };
        tooltip::layout(&ctx, s)
    };
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(rect, Rect::new(400, 440, 192, 40));
    assert!(
        rect.x >= 0 && rect.y >= 0 && rect.x + rect.w <= 640 && rect.y + rect.h <= 480,
        "the box {rect:?} leaves the 640 x 480 screen"
    );
    // And the outline's bottom row really is on it.
    let c = draw(&mut m, &mut g, &a);
    assert_eq!(c.at(400, 479), tooltip::INK, "the box's last row is drawn");
}

// ------------------------------------------------------------ which screens

/// **`FUN_00477320`, position by position**, against literals out of the
/// decompilation.
///
/// Ablations: swap the `0x223` and `0x23B` splits and the heart rows go red;
/// read the farm list's `1` as wheat and the cattle row does.
#[test]
fn the_campaign_ladder_names_every_part_of_the_sidebar() {
    let mine = Sidebar { minimap_mode: 0, owned: true, farm: vec![1, 0, 2], industry: vec![6, 4, 5, 7, 3] };
    for ((x, y), want) in [
        ((477, 300), 0),  // left of the sidebar
        ((560, 23), 0),   // the menu bar
        ((500, 50), 1),   // the minimap itself
        ((620, 50), 2),   // mode buttons, owner mode: labour
        ((620, 80), 3),   // food
        ((620, 110), 4),  // happiness
        ((620, 140), 5),  // the overview button
        ((500, 200), 6),  // population
        ((560, 200), 34), // the heart between
        ((600, 200), 7),  // happiness report
        ((500, 230), 8),  // tax
        ((600, 230), 9),  // rations
        ((500, 280), 10), // the labour slider
        ((500, 304), 15), // farm row 0: cattle (pitch 45, three rows)
        ((500, 349), 16), // row 1: wheat
        ((500, 394), 17), // row 2: reclaiming
        ((600, 304), 18), // industry row 0: wood (pitch 30, five rows)
        ((600, 334), 20), // row 1 is slot 4: iron
        ((600, 364), 19), // row 2 is slot 5: stone
        ((600, 394), 21), // weapons
        ((600, 424), 22), // the castle
        ((500, 440), 11), // army
        ((520, 440), 12), // treasury
        ((560, 440), 13), // supplies
        ((600, 440), 32), // castle
        ((620, 440), 33), // diplomacy
        ((560, 470), 14), // End Turn
    ] {
        assert_eq!(tooltip::campaign_tip(&mine, x, y), want, "({x}, {y})");
    }
    // An overlay up: the three statistic buttons name the mode that is on, and
    // the fourth returns to the owners' map.
    let food = Sidebar { minimap_mode: 2, ..mine.clone() };
    assert_eq!(tooltip::campaign_tip(&food, 620, 50), 3);
    assert_eq!(tooltip::campaign_tip(&food, 620, 120), 3);
    assert_eq!(tooltip::campaign_tip(&food, 620, 140), 31);
    // Somebody else's county: the strip, the slider and the rows say nothing.
    let theirs = Sidebar { owned: false, ..mine.clone() };
    for (x, y) in [(500, 200), (500, 280), (500, 304), (600, 424)] {
        assert_eq!(tooltip::campaign_tip(&theirs, x, y), 0, "({x}, {y})");
    }
    // A row past the list is nothing, and so is a list value with no tip.
    let short = Sidebar { farm: vec![1], ..mine };
    assert_eq!(tooltip::campaign_tip(&short, 500, 380), 0);
}

/// **`FUN_004777AA`** — the battlefield's eight.
#[test]
fn the_battle_ladder_names_the_overview_the_troops_and_the_five_buttons() {
    for ((x, y), want) in [
        ((477, 100), 0),
        ((500, 23), 0),
        ((500, 100), 23),
        ((500, 300), 24),
        ((500, 430), 25),
        ((500, 460), 26),
        ((520, 460), 27),
        ((560, 460), 28),
        ((600, 460), 29),
        ((630, 460), 30),
    ] {
        assert_eq!(tooltip::battle_tip(x, y), want, "({x}, {y})");
    }
}

// ------------------------------------------------------------ the install

/// **`DAT_004D6FB8`, byte for byte, out of the player's own `Lords2.exe`.**
///
/// The only check of [`tooltip::SCREENS`] that cannot be typed into agreement:
/// every other test here reads the constant.
#[test]
fn the_screen_table_is_the_one_in_the_players_executable() {
    let Some(exe) = l2_testkit::executable() else {
        l2_testkit::skip!("no Lords2.exe");
    };
    let table = l2_testkit::pe::Table::at(&exe, 0x004D_6FB8);
    let read: Vec<u8> = (0..tooltip::SCREENS.len()).map(|i| table.u8_at(i)).collect();
    assert_eq!(read, tooltip::SCREENS.to_vec());
}

/// **Group 220 is our transcription, string for string**, and in the player's
/// own body font the corner tip really is two lines — so the clamp above is a
/// case a player meets and not one only the placeholder font makes.
#[test]
fn group_220_is_the_players_own_words_and_autocalc_wraps_in_the_real_font() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no L2.eng");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    for (i, want) in tooltip::TEXT.iter().enumerate() {
        assert_eq!(assets.shell.text(tooltip::GROUP, i), *want, "group 220 index {i}");
    }
    let mut g = world();
    let s = Shown { id: 30, x: 400, y: 440 };
    let ctx = Ctx { game: &mut g, assets: &assets };
    let (rect, lines) = tooltip::layout(&ctx, s);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(rect.h, 40);
}

