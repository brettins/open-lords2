#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::common::*;
use super::blacksmith::*;
use super::bodies::*;
use l2_kingdom::county::County;
use l2_kingdom::tables::{
    Commodity, Tables, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_NAMES, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
};
use l2_view::chrome::system;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{count_noun, font, Face, Pen};

/// **`Castle_DrawStatusBlock(county, x, y, row)` (`0x0041DEDB`)**, 13 call
/// sites, at a caller-chosen origin. The job popup passes `(-0x20, 0x40, 0)`,
/// so its column is `x + 0x60 = 0x40` and its first line `y + 0x68 = 0xA8`.
///
/// ```text
/// 71/0x10 at (x+0x60, r+0x68) + Ui_DrawNumber(DAT_004D8A24[type], ' ', " %", pen + x+0x60)
/// 71/0xB  at (x+0x60, r+0x78) + Ui_DrawNumber(DAT_004D8A0C[type], ' ', " ",  pen + x+0x60) + 71/0xC
/// Ui_DrawCount(+0x1D0, 0xE,  x+0x60, r+0x90) + 71/6
/// Ui_DrawCount(+0x1D4, 0x10, x+0x60, r+0xA0) + 71/7
/// +0x1A6 == 0 ? 71/0x11 at (x+0x60, r+0xB0)
///             : Ui_DrawCount(labour[3], 0x26, x+0x60, r+0xB0) + 71/8 + Ui_DrawCount(+0x1A6, 0x42, pen + x+0x60)
/// ```
///
/// where `r = row * 0x10 + y`. `+0x1A6` is `Castle_BuildEstimate`'s byte, which
/// [`l2_kingdom::industry::castle_seasons_left`] recomputes (C164); the byte's
/// width is kept, so an estimate past 255 wraps as the original's store does.
#[allow(clippy::too_many_arguments)]
pub fn castle_status_block(
    pen: &Pen,
    ctx: &Ctx,
    canvas: &mut Canvas,
    c: &County,
    x: i32,
    y: i32,
    row: i32,
) {
    let t = &ctx.game.kingdom.tables;
    let left = x + 0x60;
    let top = row * 0x10 + y;

    let at = say(pen, ctx, canvas, CASTLE_GROUP, 0x10, left, top + 0x68);
    let bonus = castle_word(t, CASTLE_TAX_BONUS_BASE + usize::from(c.castle_type));
    pen.number_in(Face::Body, canvas, at, top + 0x68, bonus, ' ', " %", BODY_INK);

    let at = say(pen, ctx, canvas, CASTLE_GROUP, 0x0B, left, top + 0x78);
    let barracks = castle_word(t, CASTLE_BARRACKS_BASE + usize::from(c.castle_type));
    let at = pen.number_in(Face::Body, canvas, at, top + 0x78, barracks, ' ', " ", BODY_INK);
    say(pen, ctx, canvas, CASTLE_GROUP, 0x0C, at, top + 0x78);

    let at = count(pen, ctx, canvas, c.castle_stone_owed, NOUN_STONE, left, top + 0x90);
    say(pen, ctx, canvas, CASTLE_GROUP, 6, at, top + 0x90);
    let at = count(pen, ctx, canvas, c.castle_wood_owed, NOUN_WOOD, left, top + 0xA0);
    say(pen, ctx, canvas, CASTLE_GROUP, 7, at, top + 0xA0);

    let seasons = l2_kingdom::industry::castle_seasons_left(t, c) as u8;
    if seasons == 0 {
        say(pen, ctx, canvas, CASTLE_GROUP, 0x11, left, top + 0xB0);
    } else {
        let builders = c.labour[JOB_CASTLE_BUILDING];
        let at = count(pen, ctx, canvas, builders, NOUN_BUILDER, left, top + 0xB0);
        let at = say(pen, ctx, canvas, CASTLE_GROUP, 8, at, top + 0xB0);
        count(pen, ctx, canvas, i32::from(seasons), NOUN_SEASON, at, top + 0xB0);
    }
}

/// **The castle tables as the original addresses them: one run of 28 words
/// from `0x004D89E8`.** `CASTLE_WORKFORCE` is ten of them (to `0x004D8A10`),
/// then `g_castleGarrisonCap` six, the tax bonuses six (`0x004D8A28`) and the
/// free archers six (`0x004D8A40`).
///
/// `Castle_DrawStatusBlock` indexes two of them by `castleType` from a base one
/// word low — `&DAT_004D8A0C + type * 4` and `&DAT_004D8A24 + type * 4` — which
/// is right for types 1…5 and **reads the neighbouring table at type 0**: the
/// barracks line then says `CASTLE_WORKFORCE[4].1`, **2500**, and the tax line
/// the garrison table's trailing zero. Both words read out of the shipped exe
/// at those addresses (`c4 09 00 00`, `00 00 00 00`). `[V]` on the bytes; that a
/// player building a first castle sees *"Barracks for 2500 troops."* is `[I]`
/// — the reading of the painter, not observed.
pub(crate) fn castle_word(t: &Tables, index: usize) -> i32 {
    let c = &t.castle;
    let mut run = [0i32; 28];
    for (i, &(a, b)) in c.workforce.iter().enumerate() {
        run[i * 2] = a;
        run[i * 2 + 1] = b;
    }
    run[10..16].copy_from_slice(&c.garrison_cap);
    run[16..22].copy_from_slice(&c.tax_bonus_pct);
    run[22..28].copy_from_slice(&c.free_archers);
    run.get(index).copied().unwrap_or(0)
}

/// `&DAT_004D8A0C`, as a word of [`castle_word`]'s run.
pub(crate) const CASTLE_BARRACKS_BASE: usize = (0x004D_8A0C - 0x004D_89E8) / 4;
/// `&DAT_004D8A24`, the same.
pub(crate) const CASTLE_TAX_BONUS_BASE: usize = (0x004D_8A24 - 0x004D_89E8) / 4;

