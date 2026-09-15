//! `crates/l2-game/src/screens/map/mod.rs`'s header carried the line *"**not
//! reproduced:** the menu bar's three titles — `Menu_OpenDropdown`
//! (`0x0040DECA`)"*, which is nineteen input arms in one row of a table.
//!
//! `g_menuBarItems` (`0x004DC428`) is **three 16-byte records**:
//!
//! ```text
//!   0x4DC428   x=10  measuredX=0  y=6  group=1   items=0x004DC360  count=4
//!   0x4DC438   x=10  measuredX=0  y=6  group=2   items=0x004DC390  count=5
//!   0x4DC448   x=10  measuredX=0  y=6  group=3   items=0x004DC3D0  count=7
//! ```
//!
//! `Ui_DrawMenuTitles` (`0x0040C5B0`) is the only reason record 0's `x` is 10
//! and every other `x` is 10 as well:
//!
//! So each title's hit box is **exactly the width of its own word** in the
//! 14-pixel body font, with a 32-pixel gap after it, and `Menu_HitTitle`
//! (`0x0040E00A`) tests `x … measuredX` against a **fixed 12-pixel height** from
//! `y = 6`. A player who clicks the gap between *File* and *Options* hits
//! nothing. That is why [`titles`] takes the loaded font: the geometry is
//! `L2.eng`'s words through the game's own font, not a table of rectangles.
//!
//! The battlefield's half is `docs/arms.json`'s `0x0040DECA/battle-menu-bar` and
//! belongs to the battlefield group; this module is the campaign map's.
//!
//! And on the campaign map it sits **inside** the turn-ended guard: the arm
//! tests `Map_EdgeScroll`, `FUN_00439079` and `Sidebar_ButtonClicked` first and
//! *outside* it, then `if (!syncWait && (!turnEnded || debugOverride))` and only
//! then the menu bar. So the five sidebar buttons keep working after End Turn
//! and the menu bar does not.
//!
//! `Menu_OpenDropdown` saves `g_screenId` into `g_menuPrevScreen`, writes
//! **`0x32`**, and calls `Menu_SaveBackdrop` (`0x0040C8D1`), which copies the
//! **400 × 180 band at (0, 24)** so the painter can put it back. `Screen_Draw`'s
//! `0x32` arm is `Menu_RestoreBackdrop` (`0x0040C928`), which makes **zero**
//! draw calls: it sets `g_drawX`/`g_drawY`/`g_spriteWidth`/`g_spriteHeight` and
//! calls `FUN_004B3EC0`, the restore twin of the save. It is the *erase*, and
//! nothing else.
//!
//! Not in `Screen_Draw` and not in `Screen_DrawWidgets` — which has no `0x32`
//! arm either. `FUN_0040C725` (`0x0040C725`) is the painter, and its only
//! caller is the application's own frame loop at `0x004B99C0`, two lines after
//! `Screen_DrawMenuBar()`:
//!
//! ```c
//! FUN_00472A31();
//! Screen_DrawMenuBar();
//! FUN_0040C725();            /* <- the open drop-down */
//! FUN_0041423F();
//! ```
//!
//! `if (g_screenId == 0x32 && DAT_00522CB4 != 0)`. **That is a fourth place
//! drawing hides**, alongside the painter, `Screen_DrawWidgets` and the
//! variable widget count — and it is the only one of the four where a screen's
//! whole appearance lives in a function no dispatch table mentions.
//!
//! (`0x004B99C0` is named `Battle_Frame` in `docs/symbols.json`
//! battle function: it calls `Screen_Draw`, `Screen_DrawWidgets`,
//! `Screen_DrawMenuBar`, `CountyStrip_Draw`, `Smk_PlayLoop`, `Msg_Pump` and
//! `Cursor_Set`. It is *the* frame. Reported, not renamed here.)
//!
//! ```text
//! FUN_0040C725():                                               0x0040C725
//!   if (g_screenId != 0x32 || DAT_00522CB4 == 0) return;
//!   rec   = DAT_00522CAC                the 16-byte menu-bar record that is down
//!   items = rec[+4]                     the 12-byte item table
//!   x     = rec[+0]        the measured x Ui_DrawMenuTitles wrote back
//!   y     = rec[+4 as short 2]  == 6
//!   group = rec[+6]                     1, 2 or 3
//!   count = rec[+0C]                    4, 5 or 7
//!   g_spriteWidth  = 0x0C
//!   g_spriteHeight = (count * 0x15) / 16 + 2                    cells
//!   FUN_00409429(x, y + 0x12, 0x0C, g_spriteHeight)
//!       = Ui_DrawBoxBorder(2, x, y + 18, 12, h)          192 px wide, set TWO
//!         Ui_DrawBoxInterior(x + 16, y + 18, 10, h - 1)
//!   for i in 1 ..= count:
//!     iy = item.y + y + 0x20
//!     if (i == DAT_00522CB0) {
//!       g_spriteWidth = 0x0B; g_spriteHeight = 0x10;
//!       FUN_004B414A(x + 8, item.y + y + 0x1E, 0x3F);     176 x 16 plate
//!       Eng_DrawString(group, item.index, x + 0x10, iy, body, 0x18);
//!     } else
//!       Eng_DrawString(group, item.index, x + 0x10, iy, body, 0x3F);
//!   Gfx_MarkSpriteDirty(x, y + 0x12, 0x0D, 0x0C, 2)
//! ```
//!
//! * **there is a plate.** This header used to say *"the original saves the
//!   screen band and draws the captions straight onto it with no plate at
//!   all"*. It draws a twelve-cell `Ui_DrawBoxBorder(2, …)` — border set
//!   **two**, which no other screen in this crate uses — at `(x, 24)`;
//! * **the captions are at `x + 16`**, not two pixels in, and their baseline is
//!   `title.y + item.y + 32`, one pixel below the hit box's top;
//! * **`g_spriteWidth` counts sixteen-pixel units.** `FUN_004B414A` writes
//!   `g_spriteWidth` iterations of four dwords per row, so `0x0B` is a
//!   **176**-pixel highlight and not an eleven-pixel one. The same unit bit
//!   `screens/saveload.rs`, where a 96-pixel selection bar had been drawn six
//!   pixels wide.
//!
//! The box is 192 wide, the highlight 176 and `FUN_0040E099`'s hit box 144.
//!
//! ```c
//! if (FUN_0040DD92(&g_menuBarItems, 3) == 0 && g_mouseRightReleased) FUN_0040DF62();
//! ```
//!
//! and `FUN_0040DD92` splits on the left button:
//!
//! * **not pressed** — `Menu_HitTitle` again: sliding onto a *different* title
//!   with the button up **switches the open menu**, and then `FUN_0040E099`
//!   recomputes which item the pointer is on. Hover, in other words, and it is
//!   the only hover arm in the management interface.
//!
//! * **pressed** — an item under the pointer runs its handler *after* restoring
//!   `g_screenId = g_menuPrevScreen`; no item under the pointer is
//!   `FUN_0040DF62`, which closes.
//!
//! `FUN_0040E099` (`0x0040E099`) is the item hit test: `x … x + 0x90` — **144
//! pixels wide whatever the caption says** — and row `i` occupies
//! `y + item.y + 0x1F … + 0x0F`, so the first row starts 31 pixels below the
//! title's baseline and each is 15 tall in a 20-pixel pitch. The five-pixel gap
//! between rows is dead.
//!
//! * **Nothing about the plate any more.** It was ours; it is the original's
//!   `FUN_00409429(x, y + 0x12, 0x0C, h)` now, border set 2 and all —
//!   `l2_view::chrome::Chrome::draw_box` draws the open-topped shape, and
//!   `crates/l2-game/tests/chrome_text/main.rs`'s
//!   `the_drop_down_plate_has_no_top_rail` holds it there.
//!
//! * **`Menu_NewGame` and `Menu_Quit` reach the confirmation box**, prompts 1
//!   and 0 of `L2.eng` group 10 on screen `0x1E` —
//!   [`crate::screens::confirm`]. Neither item acts itself; the box's yes does.
//!
//! * **The five help topics go on the message ring.** `Menu_HelpHowDoI` and its four
//!   siblings are `Msg_Enqueue(…, 0x123 … 0x127, …)`, five consecutive message
//!   ids, category 0x13, and they go on the ring; the window is
//!   `g_helpWindowGeom`'s (`0x004D6EB8`) —
//!   [`crate::screens::message`]'s category-0x13 arm.

mod items;
pub use items::*;
mod dropdown;
pub use dropdown::*;
mod render;
pub use render::*;
mod tests_part;
pub use tests_part::*;

use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::options::Page;
use crate::screens::saveload::Mode;
use crate::shell::font;

const BAR_X: i32 = 10;
const BAR_Y: i32 = 6;
const TITLE_H: i32 = 12;
const TITLE_GAP: i32 = 32;

/// `FUN_0040E099`: the item row's width, its height, and the offset from the
/// title's `y` to the first row.
const ITEM_W: i32 = 0x90;
const ITEM_H: i32 = 0x0F;
const ITEM_TOP: i32 = 0x1F;
pub const ITEM_PITCH: i32 = 20;

/// **`FUN_004B414A`'s width unit** — it writes four dwords, sixteen bytes, per
/// iteration of `g_spriteWidth`. `[V]` at `0x004B414A`.
const PLATE_CELL: i32 = 16;

/// `FUN_00409429(x, y + 0x12, 0x0C, h)` — the drop-down's plate. Twelve cells
/// is 192 pixels; the offset from the title's `y` is 18.
const PLATE_COLS: i32 = 0x0C;
const PLATE_DY: i32 = 0x12;
/// **Border set two.** `FUN_00409429` is `Ui_DrawBoxBorder(2, …)`, and this is
/// the only screen in the crate that asks for it; `Pen::window` models sets 0
/// and 1 and draws set 1's artwork for this.
const PLATE_SET: usize = 2;
fn plate_rows(count: usize) -> i32 {
    (count as i32 * 0x15) / PLATE_CELL + 2
}

const CAPTION_DX: i32 = 0x10;
const CAPTION_DY: i32 = 0x20;
/// `g_spriteWidth = 0x0B; g_spriteHeight = 0x10; FUN_004B414A(x + 8,
/// item.y + y + 0x1E, 0x3F)` — a **176 × 16** plate under the picked caption,
/// eight pixels in from the box's left edge and two above the hit box.
const HIGHLIGHT_W: i32 = 0x0B * PLATE_CELL;
const HIGHLIGHT_H: i32 = 0x10;
const HIGHLIGHT_DX: i32 = 8;
const HIGHLIGHT_DY: i32 = 0x1E;
const PICKED_INK: u8 = 0x18;
const TITLE_PLATE_H: i32 = 0x12;
const TITLE_PLATE_DX: i32 = -2;
const TITLE_PLATE_DY: i32 = -3;

