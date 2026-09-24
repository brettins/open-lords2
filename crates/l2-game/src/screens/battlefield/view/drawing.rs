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
/// `FUN_004BC51A` draws two things: a terrain tile
/// per cell and a man over it. **[V]** That nothing *else* writes inside
/// `(0x1E0, 0x18)`–`(0x280, 0xB8)` is **[I]**: `Screen_DrawBattlefield`'s three
/// `Misc_bat.pl8` blits all start at `y 0xB8` or below
/// in `DAT_004D31F4` is at `y 185`.
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

/// `Screen_DrawBattlefield` (`0x004233F7`) lays the column down in one run,
/// and every one of the five frames is drawn at the position in its own
/// `Pl8` header:
///
/// ```c
/// Pl8_DrawFrameHere(g_miscCtySheet, 0, 0x1e0, 0xb8);                   /* 160 x 228 */
/// Pl8_DrawFrameHere(g_miscCtySheet, 1, 0x1e0, 0x1c0);                  /* 160 x  32 */
/// Pl8_DrawFrameHere(g_miscCtySheet, 2, 0x1e0, 0x19c);                  /* 160 x  36 */
/// Pl8_DrawFrameHere(g_miscCtySheet, DAT_00568934 + 6, 0x1e2, 0x19d);   /*  28 x  35 */
/// Pl8_DrawFrameHere(g_miscCtySheet, DAT_00568938 + 6, 0x230, 0x19d);
/// ```
///
/// `FUN_00423530` (`0x00423530`), the once-a-frame refresh, repaints frames 1
/// and 2 and both shields and adds the two lit buttons: frame 5 at
/// `(0x1E1, 0x1C1)` while the pause word `DAT_0053F238` is set, frame 6 at
/// `(0x201, 0x1C1)` while `DAT_00568964 == 1` — input armed, which
/// `Battle_Start` does before the screen is raised and nothing clears while it
/// is up, so the second is unconditional here. **[V]**
///
/// **`DAT_00568934` is side 4's shield and `DAT_00568938` is side 0's**, which
/// is the way round nothing about the names suggests: both are
/// `g_units[army] + 0x02`, and `g_battleArmyB` — the one on the *right*, at
/// `0x230` — is the side-0 army, `l2_view::scene`'s `BattleBanner_Draw` note.
///
/// So the left plate is [`l2_sim::SIDE_B`]. **[V]**
///
/// `FUN_00423530` ends with the two living-men counts on frame 2's plate:
///
/// ```c
/// Ui_DrawNumberRight(g_battleMenA,' ',&DAT_004d4448,0x1fa,0x1a6,0x38,&g_fontBody,0x20);
/// Ui_DrawNumberRight(g_battleMenB,' ',&DAT_004d444c,0x24a,0x1a6,0x38,&g_fontBody,0x20);
/// ```
///
/// `Ui_DrawNumberRight` **centres** in the width (C119), so each count sits in
/// the middle of a 0x38 box. **[V]**. That the left box is the left shield's
/// side is **[I]** from the geometry: `g_battleMenB` is drawn at `0x24A`,
/// inside the right shield's plate at `0x230`, and `g_battleArmyB` is the
/// side-0 army, so `men.1` here is [`l2_sim::SIDE_A`] as `shields.1` is.
pub(super) fn draw_column_chrome(
    canvas: &mut Canvas,
    p: &Pen,
    chrome: Option<&l2_view::chrome::Chrome>,
    shields: (u8, u8),
    men: (u32, u32),
    paused: bool,
) -> bool {
    use l2_view::chrome::misc_bat as mb;
    let Some(c) = chrome.filter(|c| c.misc_bat().is_some()) else { return false };
    c.draw_misc_bat(canvas, mb::COLUMN, 0x1E0, 0xB8);
    c.draw_misc_bat(canvas, mb::BUTTONS, 0x1E0, 0x1C0);
    c.draw_misc_bat(canvas, mb::COUNTS, 0x1E0, 0x19C);
    c.draw_misc_bat(canvas, mb::SHIELD + shields.0 as usize, 0x1E2, 0x19D);
    c.draw_misc_bat(canvas, mb::SHIELD + shields.1 as usize, 0x230, 0x19D);
    if paused {
        c.draw_misc_bat(canvas, mb::PAUSE_LIT, 0x1E1, 0x1C1);
    }
    c.draw_misc_bat(canvas, mb::RETREAT_LIT, 0x201, 0x1C1);
    p.body_centred(canvas, 0x1FA, 0x1A6, 0x38, &men.0.to_string(), 0x20);
    p.body_centred(canvas, 0x24A, 0x1A6, 0x38, &men.1.to_string(), 0x20);
    true
}

/// **The banner plates, `FUN_004238B8` (`0x004238B8`), and their counts,
/// `FUN_004239D5` (`0x004239D5`)** — the right column's other tenant of
/// `Misc_bat.PL8`, laid over frame 0 by `FUN_00423739` whenever the held count
/// changes bands.
///
/// ```c
/// Pl8_DrawFrameHere(g_miscCtySheet, rec.frame + man.troopType, rec.x, rec.y);
/// /* then, only while DAT_00553220 < 0x1e: */
/// Ui_DrawNumber(man.men,' ',…, rec.x + (man.men < 0x65 ? 0x14 : 0xc), rec.y + 2, &g_fontBody, 0x3f);
/// ```
///
/// Two rules out of that, both **[V]**: a figure with **no men is skipped and
/// takes no slot**, and **the fifty-slot layout carries no numbers at all** —
/// `DAT_00553220` *is* 0x1E there, so `FUN_004239D5` returns at its first
/// test. The `0x14` / `0xC` step left is room for a third digit.
///
/// `Ui_DrawRectOutline` (`0x00403CF4`) has fourteen callers; the only one on a
/// battle screen is `Battlefield_DrawBand` (`0x0041298A`), the rubber band,
/// which draws the *drag* box in colour `0x20` and is gated on
/// `g_screenId == 0x2A`. Being held shows in this column and nowhere else.
///
/// **[V]** on the caller set.
pub(super) fn draw_banner_plates(
    canvas: &mut Canvas,
    p: &Pen,
    chrome: Option<&l2_view::chrome::Chrome>,
    live: &LiveBattle,
    ink: &l2_view::Ink,
) -> bool {
    let Some(c) = chrome.filter(|c| c.misc_bat().is_some()) else {
        draw_banners(canvas, live, ink);
        return false;
    };
    let picked = live.runner.selected_fighters(live.owner);
    let layout = BannerLayout::for_count(picked.len());
    for (slot, &f) in picked.iter().enumerate() {
        if slot >= layout.slots {
            break;
        }
        let r = layout.rect(slot);
        let fighter = &live.runner.fighters[f];
        let men = live.runner.sim.figures[fighter.sim].men;
        if men == 0 {
            continue;
        }
        c.draw_misc_bat(canvas, layout.frame + fighter.troop.index(), r.x, r.y);
        if layout.slots < crate::battlefield::BANNERS_MANY.slots {
            let dx = if men < 0x65 { 0x14 } else { 0xC };
            p.body(canvas, r.x + dx, r.y + 2, &men.to_string(), font::TEXT);
        }
    }
    true
}

/// `Battlefield_DrawBand` (`0x0041298A`): `Ui_DrawRectOutline(…, 0x20)`. [V]
pub(super) const BAND_INK: u8 = 0x20;

/// `FUN_004BE070`'s shift by `g_battleSizeClass`: `DAT_004D9680` (sign, 1 = left)
/// and `DAT_004D96A8` (count), read from `Lords2.exe`. [V]
const MARKER_LEFT: [bool; 10] = [true, true, true, false, false, false, false, false, false, false];
const MARKER_SHIFT: [u32; 10] = [2, 1, 0, 1, 2, 3, 4, 5, 6, 0];

/// `FUN_004BE070`: `Misc_bat.pl8` frame `men <</>> shift + 0x2E` at the figure's
/// origin `+ 8`, `- 0xC` for troop < 7 or oil, else `- 0x20`, ram `+ 0x10`. [V]
pub fn selection_marker(size_class: u8, men: u16, troop: usize) -> (usize, i32, i32) {
    let c = usize::from(size_class).min(9);
    let men = u32::from(men);
    let bar = if MARKER_LEFT[c] { men << MARKER_SHIFT[c] } else { men >> MARKER_SHIFT[c] };
    let dy = match troop {
        0..=6 | 10 => -0xC,
        9 => -0x20 + 0x10,
        _ => -0x20,
    };
    (bar as usize + 0x2E, 8, dy)
}

/// `FUN_004BD233` walks the viewport's cells and marks each occupant whose
/// `selectedSeen` holds the local realm's bit; ours walks `selected_fighters`. [I]
pub(super) fn draw_selection_markers(
    canvas: &mut Canvas,
    chrome: Option<&l2_view::chrome::Chrome>,
    live: &LiveBattle,
    ink: &l2_view::Ink,
) {
    let cam = l2_view::scene::Camera::clamped(live.cam.0, live.cam.1);
    let class = live.runner.size_class();
    for f in live.runner.selected_fighters(live.owner) {
        let fig = &live.runner.fighters[f];
        let (sx, sy) = l2_view::scene::figure_origin(fig, cam);
        if !VIEW.contains(sx, sy) {
            continue;
        }
        let men = live.runner.sim.figures[fig.sim].men;
        let (frame, dx, dy) = selection_marker(class, men, fig.troop.index());
        let drawn = chrome.is_some_and(|c| c.draw_misc_bat(canvas, frame, sx + dx, sy + dy));
        // [D] FUN_004BE070 0x004BE070 draws nothing when the frame is out of range; ours
        // boxes the man only when Misc_bat.pl8 is missing from the install.
        if !drawn && chrome.is_none() {
            crate::widget::frame(canvas, Rect::new(sx, sy, TILE, TILE), ink.highlight);
        }
    }
}

/// FUN_004BE33F 0x004BE33F, last in FUN_004BD233 while `0 < DAT_00553F84`: misc_bat
/// frame 0x3F at `(cellX - camX) * 32 + viewX + 2`, `(cellY - camY) * 32 + viewY + 9`. [V]
pub(super) fn draw_order_mark(canvas: &mut Canvas, chrome: Option<&l2_view::chrome::Chrome>, live: &LiveBattle) {
    if live.order_mark_ticks == 0 {
        return;
    }
    let cam = l2_view::scene::Camera::clamped(live.cam.0, live.cam.1);
    let x = (live.order_mark.0 as i32 - cam.x as i32) * TILE + VIEW.x + 2;
    let y = (live.order_mark.1 as i32 - cam.y as i32) * TILE + VIEW.y + 9;
    // [I] Clip_Horizontal/Clip_Vertical to the field view leave an off-view mark undrawn.
    if let Some(c) = chrome.filter(|_| VIEW.contains(x, y)) {
        c.draw_misc_bat(canvas, ORDER_MARK_FRAME, x, y);
    }
}

pub(crate) const ORDER_MARK_FRAME: usize = 0x3F;

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

