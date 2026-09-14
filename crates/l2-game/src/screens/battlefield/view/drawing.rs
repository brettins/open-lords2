#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::helpers::*;
use super::*;
use l2_sim::runner::Formation;
use l2_sim::terrain::DIM;
use l2_view::{text, Canvas};
use crate::battlefield::{
    self, BannerLayout, Button, Cursor, LiveBattle, Mode, OVERVIEW, TILE, VIEW, VIEW_COLS,
    VIEW_ROWS,
};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::menubar;
use crate::shell::{font, Pen};
use crate::turn::{self, TurnStep};

/// **Ours**, and it looks it: flat cells and a block per man, for an install
/// with no `T32_bat1.pl8`. `docs/decisions.md` C21 — a stub that is visibly ours
/// beats one that looks finished.
///
/// The blocks stand where the artwork's men would —
/// `l2_view::scene::figure_origin`, `BattleMan_Step`'s cell and trail — so the
/// placeholder walks the way the picture does.
pub(super) fn draw_placeholder_field(canvas: &mut Canvas, live: &LiveBattle, ink: &l2_view::Ink) {
    let cam = l2_view::scene::Camera::clamped(live.cam.0, live.cam.1);
    canvas.fill_rect(VIEW.x, VIEW.y, VIEW.w, VIEW.h, ink.background);
    for row in 0..VIEW_ROWS {
        for col in 0..VIEW_COLS {
            let cx = live.cam.0 + col;
            let cy = live.cam.1 + row;
            let cell = live.runner.field.at(cx as usize, cy as usize);
            let shade = if cell.impassable() { ink.border } else { ink.dim };
            canvas.fill_rect(VIEW.x + col * TILE, VIEW.y + row * TILE, TILE - 1, TILE - 1, shade);
        }
    }
    for i in 0..live.runner.fighters.len() {
        if !live.runner.is_alive(i) {
            continue;
        }
        let f = &live.runner.fighters[i];
        let (sx, sy) = l2_view::scene::figure_origin(f, cam);
        if !VIEW.contains(sx, sy) {
            continue;
        }
        let c = if f.side == l2_sim::SIDE_A { ink.highlight } else { ink.text };
        canvas.fill_rect(sx + 8, sy + 8, TILE - 16, TILE - 16, c);
    }
}

/// The 80 × 80 field at two pixels a cell — the raster `BattleMap_Click` hit
/// tests, painted by `FUN_004BC51A` (`0x004BC51A`) and scheduled by
/// [`Overview`].
///
///
/// `FUN_004BC51A` draws two things and neither is a rectangle: a terrain tile
/// per cell and a man over it. **[V]** That nothing *else* writes inside
/// `(0x1E0, 0x18)`–`(0x280, 0xB8)` is **[I]**: `Screen_DrawBattlefield`'s three
/// `Misc_bat.pl8` blits all start at `y 0xB8` or below, and the earliest banner
/// in `DAT_004D31F4` is at `y 185`.
///
/// `have_sheets` is false on an install that does not ship `T2_bat1.pl8` beside
/// the executable — the older DOS tree does not — and on
/// [`crate::game::Assets::placeholder`]. Then the panel is a flat fill and a dot
/// a side, which is ours and is marked as ours.
pub(super) fn draw_overview(
    canvas: &mut Canvas,
    panel: &Overview,
    live: &LiveBattle,
    ink: &l2_view::Ink,
    have_sheets: bool,
) {
    if have_sheets {
        canvas.blit_raster(
            &panel.raster.pixels,
            l2_view::scene::OVERVIEW_SIDE,
            OVERVIEW.x,
            OVERVIEW.y,
            1,
        );
        return;
    }
    canvas.fill_rect(OVERVIEW.x, OVERVIEW.y, OVERVIEW.w, OVERVIEW.h, ink.background);
    for i in 0..live.runner.fighters.len() {
        if !live.runner.is_alive(i) {
            continue;
        }
        let f = &live.runner.fighters[i];
        let c = if f.side == l2_sim::SIDE_A { ink.highlight } else { ink.text };
        canvas.fill_rect(OVERVIEW.x + f.x as i32 * 2, OVERVIEW.y + f.y as i32 * 2, 2, 2, c);
    }
}

/// One banner per figure the player holds, in the layout the count picks.
pub(super) fn draw_banners(canvas: &mut Canvas, live: &LiveBattle, ink: &l2_view::Ink) {
    let picked = live.runner.selected_fighters(live.owner);
    let layout = BannerLayout::for_count(picked.len());
    for (slot, &f) in picked.iter().enumerate() {
        if slot >= layout.slots {
            break;
        }
        let r = layout.rect(slot);
        canvas.fill_rect(r.x, r.y, r.w, r.h, ink.dim);
        crate::widget::frame(canvas, r, ink.border);
        let troop = live.runner.fighters[f].troop;
        let stem = l2_view::figures::stem(troop).unwrap_or("eng");
        text::draw(canvas, r.x + 2, r.y + 2, &stem.to_uppercase(), ink.text);
    }
}

