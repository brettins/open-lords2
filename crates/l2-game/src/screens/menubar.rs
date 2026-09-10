//! **The menu bar — three titles, sixteen items, and a screen id of its own.**
//!
//! `docs/screens-county.md` §10.1. The bar itself is painted every frame by
//! `Screen_DrawMenuBar`, and until now it was painted here too and did nothing:
//! `crates/l2-game/src/screens/map.rs`'s header carried the line *"**not
//! reproduced:** the menu bar's three titles — `Menu_OpenDropdown`
//! (`0x0040DECA`)"*, which is nineteen input arms in one row of a table.
//!
//! # The interface is data, and every number below came out of the table
//!
//! `g_menuBarItems` (`0x004DC428`) is **three 16-byte records**:
//!
//! ```text
//!   0x4DC428   x=10  measuredX=0  y=6  group=1   items=0x004DC360  count=4
//!   0x4DC438   x=10  measuredX=0  y=6  group=2   items=0x004DC390  count=5
//!   0x4DC448   x=10  measuredX=0  y=6  group=3   items=0x004DC3D0  count=7
//! ```
//!
//! and each item is 12 bytes — `{short y; short stringIndex; void (*handler)();
//! int 0}`. `tools/oracle/widgets.js menu 4dc428 3` prints all sixteen with
//! their handlers named, and [`MENUS`] is that print-out.
//!
//! # The titles are **measured**, not placed
//!
//! `Ui_DrawMenuTitles` (`0x0040C5B0`) is the only reason record 0's `x` is 10
//! and every other `x` is 10 as well:
//!
//! ```c
//! g_penAdvance = items[0].x;                       /* 10 */
//! for (i = 1; i <= count; i++) {
//!     items[i].x = g_penAdvance;                   /* written back  */
//!     Eng_DrawString(items[i].group, 0, x, y, &g_fontBody, colour);
//!     items[i].measuredX = g_penAdvance;           /* the pen after the string */
//!     g_penAdvance += 0x20;                        /* 32 px between titles */
//! }
//! ```
//!
//! So each title's hit box is **exactly the width of its own word** in the
//! 14-pixel body font, with a 32-pixel gap after it, and `Menu_HitTitle`
//! (`0x0040E00A`) tests `x … measuredX` against a **fixed 12-pixel height** from
//! `y = 6`. A player who clicks the gap between *File* and *Options* hits
//! nothing. That is why [`titles`] takes the loaded font: the geometry is
//! `L2.eng`'s words through the game's own font, not a table of rectangles.
//!
//! # Where it is live, which is two screens and not every screen
//!
//! `Menu_OpenDropdown` is called from exactly two of `Screen_FrameInput`'s
//! forty-nine arms: `g_screenId == 0` (the campaign map) and `g_screenId ==
//! 0x29` (the battlefield). **The bar is drawn over the village and the four
//! county panels and is dead there** — it is not tested in any of their arms.
//! The battlefield's half is `docs/arms.json`'s `0x0040DECA/battle-menu-bar` and
//! belongs to the battlefield group; this module is the campaign map's.
//!
//! And on the campaign map it sits **inside** the turn-ended guard: the arm
//! tests `Map_EdgeScroll`, `FUN_00439079` and `Sidebar_ButtonClicked` first and
//! *outside* it, then `if (!syncWait && (!turnEnded || debugOverride))` and only
//! then the menu bar. So the five sidebar buttons keep working after End Turn
//! and the menu bar does not.
//!
//! # The drop-down is screen `0x32`, and **its painter draws nothing**
//!
//! `Menu_OpenDropdown` saves `g_screenId` into `g_menuPrevScreen`, writes
//! **`0x32`**, and calls `Menu_SaveBackdrop` (`0x0040C8D1`), which copies the
//! **400 × 180 band at (0, 24)** so the painter can put it back. `Screen_Draw`'s
//! `0x32` arm is `Menu_RestoreBackdrop` (`0x0040C928`), which makes **zero**
//! draw calls: it sets `g_drawX`/`g_drawY`/`g_spriteWidth`/`g_spriteHeight` and
//! calls `FUN_004B3EC0`, the restore twin of the save. It is the *erase*, and
//! nothing else.
//!
//! # So where is the drop-down drawn? **In the frame loop.**
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
//! It runs every frame on every screen and guards *itself*:
//! `if (g_screenId == 0x32 && DAT_00522CB4 != 0)`. **That is a fourth place
//! drawing hides**, alongside the painter, `Screen_DrawWidgets` and the
//! variable widget count — and it is the only one of the four where a screen's
//! whole appearance lives in a function no dispatch table mentions.
//!
//! (`0x004B99C0` is named `Battle_Frame` in `docs/symbols.json` and is not a
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
//! **Four draw-call sites, two of them a border pair.** The three numbers this
//! module had wrong before that listing was written:
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
//! Three different widths for the same row, all read out of the binary.
//!
//! Its own arm, `Screen_FrameInput`'s `0x32`, is two lines:
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
//! # What is ours
//!
//! * **Nothing about the plate any more.** It was ours; it is the original's
//!   `FUN_00409429(x, y + 0x12, 0x0C, h)` now. What remains ours is that our
//!   `Pen::window` only models two of the original's three border sets, so set
//!   2 draws with set 1's artwork. Recorded rather than faked.
//! * **`Menu_NewGame` and `Menu_Quit` reach no confirmation box.** Both open
//!   `Ui_OpenConfirm` in the original — prompts 1 and 0 of `L2.eng` group 10 —
//!   and screen `0x1E` is not built. New Game is refused with a status line and
//!   Quit leaves at once; both say so below.
//! * **The five help topics enqueue nothing.** `Menu_HelpHowDoI` and its four
//!   siblings are `Msg_Enqueue(…, 0x123 … 0x127, …)`, five consecutive message
//!   ids, and we have no message scroll. They are recorded and refused.

use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::options::Page;
use crate::screens::saveload::Mode;
use crate::shell::font;

/// `g_menuBarItems[0].x` — the pen the first title starts at.
const BAR_X: i32 = 10;
/// `g_menuBarItems[*].y`, and `Menu_HitTitle`'s fixed height.
const BAR_Y: i32 = 6;
const TITLE_H: i32 = 12;
/// `g_penAdvance += 0x20` between one title and the next.
const TITLE_GAP: i32 = 32;

/// `FUN_0040E099`: the item row's width, its height, and the offset from the
/// title's `y` to the first row.
const ITEM_W: i32 = 0x90;
const ITEM_H: i32 = 0x0F;
const ITEM_TOP: i32 = 0x1F;
/// The item table's own `y` column: 0, 20, 40, … with no gaps.
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
/// `g_spriteHeight = (count * 0x15) / 16 + 2`, in cells.
fn plate_rows(count: usize) -> i32 {
    (count as i32 * 0x15) / PLATE_CELL + 2
}

/// `Eng_DrawString(group, index, x + 0x10, item.y + y + 0x20, body, colour)` —
/// the caption is **sixteen pixels** into the plate, and its baseline is one
/// pixel below the hit box's top edge.
const CAPTION_DX: i32 = 0x10;
const CAPTION_DY: i32 = 0x20;
/// `g_spriteWidth = 0x0B; g_spriteHeight = 0x10; FUN_004B414A(x + 8,
/// item.y + y + 0x1E, 0x3F)` — a **176 × 16** plate under the picked caption,
/// eight pixels in from the box's left edge and two above the hit box.
const HIGHLIGHT_W: i32 = 0x0B * PLATE_CELL;
const HIGHLIGHT_H: i32 = 0x10;
const HIGHLIGHT_DX: i32 = 8;
const HIGHLIGHT_DY: i32 = 0x1E;
/// The picked caption's colour. `Ui_DrawMenuTitles` uses the same `0x18` for the
/// open *title*, which is how the two halves of the bar agree.
const PICKED_INK: u8 = 0x18;
/// `g_spriteHeight = 0x12` and `g_spriteWidth = (textWidth + 4) / 16 + 2` —
/// the plate `Ui_DrawMenuTitles` puts under the **open title**, two pixels left
/// and three up.
const TITLE_PLATE_H: i32 = 0x12;
const TITLE_PLATE_DX: i32 = -2;
const TITLE_PLATE_DY: i32 = -3;

/// What one drop-down item does when it is picked.
///
/// Every variant names the original's handler out of `g_menuBarItems`' item
/// tables, and there are exactly sixteen because the three tables hold four,
/// five and seven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// `Menu_NewGame` — `Ui_OpenConfirm(1, …)`, *"Start a new game?"*.
    NewGame,
    /// `Menu_LoadGame` — globs `*.sav` and sets `g_screenId = 0x35`.
    Load,
    /// `Menu_SaveGame` — `g_screenId = 0x36`.
    Save,
    /// `Menu_Quit` — `Ui_OpenConfirm(0, …)`, *"Exit the game?"*.
    Quit,
    /// `Menu_AdvancedOptions` / `Menu_SoundOptions` / `Menu_DisplayOptions` /
    /// `Menu_GameHelp`: `g_screenId = 0x39 / 0x42 / 0x43 / 0x31`.
    Options(Page),
    /// `Menu_GameSpeed` and `Menu_ScrollSpeed` — `Ui_OpenSlider` on
    /// `g_optGameSpeed` / `g_optScrollSpeed`, which is screen `0x21`, the value
    /// spinner that prints *"Click Right to Exit"*.
    Slider(&'static str),
    /// The five help topics: `Msg_Enqueue(…, id, …)` with `id` 0x123 … 0x127.
    Help(u32),
    /// `Menu_About` — `g_screenId = 0x25`.
    About,
}

/// One drop-down: its `L2.eng` group and its items, `(y, stringIndex, handler)`.
pub struct Menu {
    /// `items[3]` of the 16-byte record. Index 0 of the group is the **title**;
    /// 1 … n are the item captions, with no gaps.
    pub group: usize,
    pub items: &'static [(i32, usize, Item)],
    /// Ours: the caption to fall back on with no `L2.eng` loaded.
    pub fallback: &'static str,
    /// Ours: the same, per item, in the table's own order.
    pub item_fallbacks: &'static [&'static str],
}

/// **`g_menuBarItems` (`0x004DC428`) and its three item tables**, verbatim from
/// `node tools/oracle/widgets.js menu 4dc428 3`.
///
/// Four items, five items, seven items; `L2.eng` groups 1, 2 and 3 hold exactly
/// four, five and seven strings after their label, the `y` column is 0, 20,
/// 40, … with no gaps and the string indices are 1, 2, 3, … with no gaps.
/// Nothing here was chosen by us. `docs/screens-county.md` §10.1.
pub const MENUS: [Menu; 3] = [
    Menu {
        group: 1,
        items: &[
            (0, 1, Item::NewGame),
            (20, 2, Item::Load),
            (40, 3, Item::Save),
            (60, 4, Item::Quit),
        ],
        fallback: "FILE",
        item_fallbacks: &["NEW GAME", "LOAD", "SAVE", "QUIT"],
    },
    Menu {
        group: 2,
        items: &[
            (0, 1, Item::Options(Page::Advanced)),
            (20, 2, Item::Options(Page::Sound)),
            (40, 3, Item::Options(Page::Display)),
            // `Menu_GameSpeed` passes `&g_optGameSpeed`; `Menu_ScrollSpeed`
            // passes `&g_optScrollSpeed`, a global that had been named for
            // unrelated reasons and whose caption here is "Scroll Speed". That
            // is a check that could have failed and did not.
            (60, 4, Item::Slider("GAME SPEED")),
            (80, 5, Item::Slider("SCROLL SPEED")),
        ],
        fallback: "OPTIONS",
        item_fallbacks: &["ADVANCED", "SOUNDS", "DISPLAY", "GAME SPEED", "SCROLL SPEED"],
    },
    Menu {
        group: 3,
        items: &[
            (0, 1, Item::Options(Page::Help)),
            (20, 2, Item::Help(0x123)),
            (40, 3, Item::Help(0x124)),
            (60, 4, Item::Help(0x125)),
            (80, 5, Item::Help(0x126)),
            (100, 6, Item::Help(0x127)),
            (120, 7, Item::About),
        ],
        fallback: "HELP",
        item_fallbacks: &[
            "GAME HELP",
            "HOW DO I...",
            "GROW GRAIN?",
            "BUILD A CASTLE?",
            "MAKE WEAPONS?",
            "MANAGE EACH TURN?",
            "ABOUT",
        ],
    },
];

/// The title caption, from `L2.eng` group `n` index 0 or from the fallback.
///
/// **Index 0 of a group is the label the game wrote about itself**, which is
/// what makes the bar readable without a single string of ours on a machine
/// that has the game — `docs/formats/eng.md` §5.
pub fn title_text(ctx: &Ctx, menu: usize) -> String {
    let m = &MENUS[menu];
    let s = ctx.assets.shell.text(m.group, 0);
    if s.is_empty() {
        m.fallback.to_string()
    } else {
        s.to_string()
    }
}

/// One item's caption.
pub fn item_text(ctx: &Ctx, menu: usize, item: usize) -> String {
    let m = &MENUS[menu];
    let s = ctx.assets.shell.text(m.group, m.items[item].1);
    if s.is_empty() {
        m.item_fallbacks[item].to_string()
    } else {
        s.to_string()
    }
}

/// **The three title rectangles, measured the way `Ui_DrawMenuTitles` measures
/// them** — each caption's own width in the body font, then a 32-pixel gap.
///
/// The font is the one the install has; with none loaded this falls back to our
/// 5 × 7 metrics, so the *rule* is the original's on every machine and only the
/// pixel widths are ours on a machine with no game.
pub fn titles(ctx: &Ctx) -> [Rect; 3] {
    let captions = [title_text(ctx, 0), title_text(ctx, 1), title_text(ctx, 2)];
    title_boxes(&captions, |s| match ctx.assets.shell.body.as_ref() {
        Some(f) => f.width(s),
        None => text::width(s),
    })
}

/// **`Ui_DrawMenuTitles`' layout rule, on its own**, so that a test can drive
/// it with the install's real `Fntl2_14.pl8` without building a whole [`Ctx`].
///
/// Start the pen at the first record's `x`; each title's box is its own drawn
/// width at that pen; then advance by the width plus `g_penAdvance += 0x20`.
pub fn title_boxes(captions: &[String; 3], measure: impl Fn(&str) -> i32) -> [Rect; 3] {
    let mut out = [Rect::new(0, 0, 0, 0); 3];
    let mut pen = BAR_X;
    for (slot, caption) in out.iter_mut().zip(captions) {
        let w = measure(caption);
        *slot = Rect::new(pen, BAR_Y, w, TITLE_H);
        pen += w + TITLE_GAP;
    }
    out
}

/// `Menu_HitTitle` (`0x0040E00A`) — the 1-based index, or `None`.
///
/// Half-open in x (`items[0] <= mx < items[1]`) and in y
/// (`items[2] <= my < items[2] + 12`), which is what [`Rect::contains`] is.
pub fn title_at(ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
    titles(ctx).iter().position(|r| r.contains(x, y))
}

/// The rectangle one item of an open drop-down occupies — `FUN_0040E099`.
pub fn item_rect(titles: &[Rect; 3], menu: usize, item: usize) -> Rect {
    let row = MENUS[menu].items[item].0;
    Rect::new(titles[menu].x, BAR_Y + row + ITEM_TOP, ITEM_W, ITEM_H)
}

/// **The plate `FUN_00409429(x, y + 0x12, 0x0C, h)` draws**, in pixels:
/// twelve cells wide from the open title's measured `x`, eighteen pixels below
/// the bar's `y`, and `(count * 21) / 16 + 2` cells tall.
pub fn plate_rect(titles: &[Rect; 3], menu: usize) -> Rect {
    let rows = plate_rows(MENUS[menu].items.len());
    Rect::new(
        titles[menu].x,
        BAR_Y + PLATE_DY,
        PLATE_COLS * PLATE_CELL,
        rows * PLATE_CELL,
    )
}

/// **Screen `0x32` — a drop-down is open.**
pub struct DropdownScreen {
    /// `DAT_00522CB4`, but zero-based: which of the three titles is down.
    menu: usize,
    /// `DAT_00522CB0`, zero-based: the item under the pointer, if any.
    hover: Option<usize>,
    /// Ours: what a refused item said, drawn under the bar so a player is told
    /// rather than left wondering.
    status: String,
}

impl DropdownScreen {
    pub fn new(menu: usize) -> DropdownScreen {
        DropdownScreen { menu: menu.min(MENUS.len() - 1), hover: None, status: String::new() }
    }

    pub fn menu(&self) -> usize {
        self.menu
    }

    pub fn hover(&self) -> Option<usize> {
        self.hover
    }

    /// One item's handler.
    ///
    /// The original restores `g_screenId = g_menuPrevScreen` **before** calling
    /// it, so a handler that sets a screen id wins and one that does not leaves
    /// the player where he was. `Transition::Pop` first is that ordering; a
    /// `Push` on top of the pop is a handler that moved.
    fn run(&mut self, _ctx: &mut Ctx, item: Item) -> Transition {
        match item {
            // `Menu_NewGame`: `Ui_OpenConfirm(1, …)` — group 10 index 1. Screen
            // 0x1E is not built, so this is refused rather than guessed at.
            // arm: 0x0040DECA/file-new-game
            Item::NewGame => {
                self.status = "NEW GAME NEEDS THE CONFIRM BOX (SCREEN 0x1E)".into();
                Transition::Stay
            }
            // arm: 0x0040DECA/file-load
            Item::Load => Transition::Replace(ScreenId::SaveLoad(Mode::Load)),
            // arm: 0x0040DECA/file-save
            Item::Save => Transition::Replace(ScreenId::SaveLoad(Mode::Save)),
            // `Menu_Quit`: `Ui_OpenConfirm(0, …)`, *"Exit the game?"*. With no
            // confirm box we quit outright, which is the answer the box would
            // have carried and not the box.
            // arm: 0x0040DECA/file-quit
            Item::Quit => Transition::Quit,
            // arm: 0x0040DECA/options-and-help-pages
            Item::Options(page) => Transition::Replace(ScreenId::Options(page)),
            // `Ui_OpenSlider` on one option — screen `0x21`, the value spinner.
            // We have the two values on the options pages instead, and no
            // spinner; the item says so.
            // arm: 0x0040DECA/options-sliders
            Item::Slider(what) => {
                self.status = format!("{what} IS ON THE ADVANCED PAGE - NO SPINNER (SCREEN 0x21)");
                Transition::Stay
            }
            // Five consecutive message ids, 0x123 … 0x127, through `Msg_Enqueue`.
            // arm: 0x0040DECA/help-topics
            Item::Help(id) => {
                self.status = format!("HELP MESSAGE 0x{id:03X} NEEDS THE MESSAGE SCROLL");
                Transition::Stay
            }
            // arm: 0x0040DECA/help-about
            Item::About => Transition::Replace(ScreenId::About),
        }
    }

    /// `FUN_0040E099` against the pointer, for the menu that is open.
    fn item_at(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        let t = titles(ctx);
        (0..MENUS[self.menu].items.len()).find(|&i| item_rect(&t, self.menu, i).contains(x, y))
    }
}

impl Screen for DropdownScreen {
    fn id(&self) -> ScreenId {
        ScreenId::MenuBar(self.menu)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("Menu {} - screen 0x32", MENUS[self.menu].fallback)
    }

    /// `Screen_Draw`'s `0x32` arm is `Menu_RestoreBackdrop` and nothing else:
    /// the band under the bar is put back and whatever was around it is still
    /// on screen from the last frame.
    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // **`FUN_0040DD92`'s button-up half, and it is the only hover arm in
            // the management interface.** Sliding onto a different title with
            // the button up switches the open menu; then the item under the
            // pointer is recomputed. Both are the same call, in that order.
            Event::Pointer { x, y } => {
                // arm: 0x0040DD92/slide-between-titles
                if let Some(t) = title_at(&*ctx, x, y) {
                    if t != self.menu {
                        self.menu = t;
                        self.status.clear();
                    }
                }
                // arm: 0x0040DD92/hover-item
                self.hover = self.item_at(&*ctx, x, y);
            }
            // The button-down half: an item under the pointer runs, anything
            // else closes. `FUN_0040DF62` is the close and it is unconditional
            // — a press on the title that opened the menu closes it too, which
            // is what makes the bar feel like a menu bar.
            Event::Click { x, y } => {
                self.hover = self.item_at(&*ctx, x, y);
                match self.hover {
                    // arm: 0x0040DD92/pick-item
                    Some(i) => {
                        let item = MENUS[self.menu].items[i].2;
                        let t = self.run(ctx, item);
                        return match t {
                            Transition::Stay => Transition::Stay,
                            // The handler ran with `g_screenId` already back to
                            // `g_menuPrevScreen`, so it lands *there* and not on
                            // top of the drop-down.
                            other => other,
                        };
                    }
                    // arm: 0x0040DF62/close-on-miss
                    None => return Transition::Pop,
                }
            }
            // `if (FUN_0040DD92(...) == 0 && g_mouseRightReleased) FUN_0040DF62();`
            // arm: 0x0042FF10/dropdown-right-close
            Event::RightClick { .. } => return Transition::Pop,
            // **Ours.** The original has no key here: the window procedure's
            // Escape arm is `Menu_Quit` and never reaches screen 0x32.
            // arm: ours/dropdown-escape-closes
            Event::KeyDown(Key::Escape) => return Transition::Pop,
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let t = titles(ctx);
        let pen = crate::shell::Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            // `FUN_0040C725` touches neither `DAT_005AEA40` nor `DAT_0058FE2C`
            // around the captions, so this is the ordinary embossed body pen.
            shadow: Some(font::SHADOW),
            caps: None,
        };

        // `FUN_00409429(x, y + 0x12, 0x0C, (count * 0x15) / 16 + 2)` — the
        // original's own plate. It used to be a `widget::panel` of ours, on the
        // belief that the original drew the captions onto the bare map.
        let plate = plate_rect(&t, self.menu);
        pen.window(
            canvas,
            plate.x,
            plate.y,
            PLATE_COLS,
            plate.h / PLATE_CELL,
            PLATE_SET,
        );

        for i in 0..MENUS[self.menu].items.len() {
            let picked = self.hover == Some(i);
            if picked {
                // 176 x 16 at (x + 8, item.y + y + 0x1E) — **not** the width of
                // the hit box, which is 144, and not the width of the plate,
                // which is 192.
                canvas.fill_rect(
                    plate.x + HIGHLIGHT_DX,
                    BAR_Y + MENUS[self.menu].items[i].0 + HIGHLIGHT_DY,
                    HIGHLIGHT_W,
                    HIGHLIGHT_H,
                    font::TEXT,
                );
            }
            let caption = item_text(ctx, self.menu, i);
            // `Eng_DrawString(group, index, x + 0x10, item.y + y + 0x20, body,
            // picked ? 0x18 : 0x3F)`.
            let colour = if picked { PICKED_INK } else { font::TEXT };
            pen.body(
                canvas,
                plate.x + CAPTION_DX,
                BAR_Y + MENUS[self.menu].items[i].0 + CAPTION_DY,
                &caption,
                colour,
            );
        }

        // **Ours**, and in our own 5 x 7 font so a screenshot cannot mistake it
        // for the game's wording: what a refused item could not do.
        if !self.status.is_empty() {
            text::draw(canvas, plate.x, plate.y + plate.h + 4, &self.status, ink.bad);
        }
    }
}

/// **The bar's three captions, drawn.** A free function because the *campaign
/// map* draws it, exactly as `Screen_DrawMenuBar` does: the drop-down is a
/// separate screen and the titles are not its.
///
/// `Ui_DrawMenuTitles` draws the open title inverted — colour `0x18` on a
/// `0x3F` plate two pixels out — and the rest in `0x3F`. `open` is which, if
/// any, so the map can pass what the stack knows.
pub fn draw_titles(ctx: &Ctx, canvas: &mut Canvas, open: Option<usize>) {
    let ink = &ctx.assets.ink;
    let t = titles(ctx);
    for (i, r) in t.iter().enumerate() {
        let caption = title_text(ctx, i);
        let lit = open == Some(i);
        if lit {
            // `g_spriteWidth = (textWidth + 4) / 16 + 2; g_spriteHeight = 0x12;
            // FUN_004B414A(x - 2, y - 3, 0x3F)` — the plate the open title
            // stands on, two left and three up, in the strip's own black.
            //
            // **`g_spriteWidth` is in sixteen-pixel units**, so the plate is
            // rounded out to a whole number of cells and is always at least 32
            // pixels wider than the word rather than four. This drew `w + 4`
            // until `FUN_004B414A`'s body was read.
            let cells = (r.w + 4) / PLATE_CELL + 2;
            canvas.fill_rect(
                r.x + TITLE_PLATE_DX,
                r.y + TITLE_PLATE_DY,
                cells * PLATE_CELL,
                TITLE_PLATE_H,
                font::TEXT,
            );
        }
        let colour = if lit { PICKED_INK } else { font::TEXT };
        match ctx.assets.shell.body.as_ref() {
            Some(f) => {
                f.draw(canvas, r.x, r.y, &caption, &font::Style::new(colour));
            }
            None => {
                let c = if lit { ink.background } else { ink.text };
                text::draw(canvas, r.x, r.y + 2, &caption, c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four items, five items, seven items — and `L2.eng` groups 1, 2 and 3
    /// hold exactly that many strings after their label. The `y` column is the
    /// table's own and has no gaps.
    #[test]
    fn the_three_tables_are_the_shapes_the_binary_has() {
        assert_eq!(MENUS[0].items.len(), 4);
        assert_eq!(MENUS[1].items.len(), 5);
        assert_eq!(MENUS[2].items.len(), 7);
        for m in &MENUS {
            assert_eq!(m.items.len(), m.item_fallbacks.len(), "group {}", m.group);
            for (n, &(y, index, _)) in m.items.iter().enumerate() {
                assert_eq!(y, n as i32 * ITEM_PITCH, "group {}: the y column has a gap", m.group);
                assert_eq!(index, n + 1, "group {}: the string indices have a gap", m.group);
            }
        }
        // Sixteen items over three menus, which is what the row count of
        // docs/screens-county.md 10.1 has to be.
        assert_eq!(MENUS.iter().map(|m| m.items.len()).sum::<usize>(), 16);
    }

    /// The five help topics are five **consecutive** message ids. That is the
    /// property that says the table was read and not assembled.
    #[test]
    fn the_help_topics_are_consecutive_message_ids() {
        let ids: Vec<u32> = MENUS[2]
            .items
            .iter()
            .filter_map(|&(_, _, it)| match it {
                Item::Help(id) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec![0x123, 0x124, 0x125, 0x126, 0x127]);
    }

    /// **The plate is the original's, and its height is the item count.**
    /// `g_spriteHeight = (count * 0x15) / 16 + 2`, in cells: 4 items → 7,
    /// 5 → 8, 7 → 11. Pinned as literals from the decompilation rather than
    /// recomputed from `plate_rows`, which would test nothing.
    #[test]
    fn the_dropdown_plate_is_twelve_cells_wide_and_grows_with_the_item_count() {
        assert_eq!(plate_rows(4), 7);
        assert_eq!(plate_rows(5), 8);
        assert_eq!(plate_rows(7), 11);
        let t = [Rect::new(10, 6, 30, 12), Rect::new(72, 6, 50, 12), Rect::new(154, 6, 30, 12)];
        let p = plate_rect(&t, 2);
        assert_eq!((p.x, p.y, p.w, p.h), (154, 6 + 0x12, 192, 11 * 16));
        // Three widths for one row, and they really are three.
        assert_eq!(HIGHLIGHT_W, 176);
        assert_eq!(ITEM_W, 144);
        assert!(HIGHLIGHT_W < p.w && ITEM_W < HIGHLIGHT_W);
        // Every caption sits inside the plate it is drawn on.
        for i in 0..MENUS[2].items.len() {
            let x = p.x + CAPTION_DX;
            let y = BAR_Y + MENUS[2].items[i].0 + CAPTION_DY;
            assert!(x > p.x && x < p.x + p.w, "caption {i} starts outside the plate");
            assert!(y > p.y && y < p.y + p.h, "caption {i} at y {y} is outside the plate");
        }
    }

    /// **The menu bar's words are `L2.eng`'s, on a machine that has the game.**
    ///
    /// `docs/agents.md` records *"our own labels drawn where the menu bar's
    /// words are"* as one of five defects that existed **only** against real
    /// assets, so this is asserted against the player's own `L2.eng` and not
    /// against `MENUS`' fallbacks. Every string here is pinned as a literal.
    #[test]
    fn the_bar_draws_the_games_own_words_and_not_ours() {
        let Some(dir) = l2_testkit::install_dir() else {
            eprintln!("skipping: no game install");
            return;
        };
        let bytes = std::fs::read(dir.join("L2.eng")).expect("L2.eng");
        let eng = crate::shell::Eng::parse(bytes).expect("L2.eng parses");

        // The three titles, index 0 of groups 1, 2 and 3.
        assert_eq!(eng.get(1, 0), Some("File"));
        assert_eq!(eng.get(2, 0), Some("Options"));
        assert_eq!(eng.get(3, 0), Some("Help"));

        // All sixteen items, in the tables' own order.
        let expected: [&[&str]; 3] = [
            &["New Game", "Load", "Save", "Quit"],
            &["Advanced", "Sounds", "Display", "Game Speed", "Scroll Speed"],
            &[
                "Game Help",
                "How do I...",
                "Grow grain?",
                "Build a castle?",
                "Make Weapons?",
                "Manage each turn.?",
                "About",
            ],
        ];
        for (m, want) in MENUS.iter().zip(expected) {
            assert_eq!(
                m.items.len(),
                want.len(),
                "group {} has {} items in the table",
                m.group,
                want.len()
            );
            // **The group holds the title plus exactly those items and no
            // more.** A seventeenth string would mean a row we do not draw.
            assert_eq!(
                eng.group(m.group).len(),
                want.len() + 1,
                "group {} is one title and {} items",
                m.group,
                want.len()
            );
            for (n, &(_, index, _)) in m.items.iter().enumerate() {
                assert_eq!(eng.get(m.group, index), Some(want[n]), "group {}", m.group);
            }
        }

        // And none of the fallbacks is ever what a player with the game sees.
        for (m, want) in MENUS.iter().zip(expected) {
            assert_ne!(eng.get(m.group, 0), Some(m.fallback));
            for (n, f) in m.item_fallbacks.iter().enumerate() {
                assert_ne!(*f, want[n], "fallback {f:?} is the game's own word");
            }
        }
    }

    /// **The titles are measured in the game's own font**, which is what makes
    /// the hit boxes right — `Ui_DrawMenuTitles` starts the pen at 10 and adds
    /// the drawn width plus 32 after each. With `Fntl2_14.pl8` loaded the three
    /// boxes must be three different widths, must not overlap, and must leave
    /// exactly 32 pixels between one and the next.
    #[test]
    fn the_title_boxes_are_measured_with_fntl2_14() {
        let Some(dir) = l2_testkit::install_dir() else {
            eprintln!("skipping: no game install");
            return;
        };
        let bytes = std::fs::read(dir.join(font::BODY)).expect("Fntl2_14.pl8");
        let font = crate::shell::Font::new(bytes, 16).expect("Fntl2_14.pl8 decodes");
        // The module's own rule, driven with the install's own font. The
        // captions and every number asserted are literals out of `L2.eng` and
        // `Ui_DrawMenuTitles`, so ablating `BAR_X`, `BAR_Y`, `TITLE_H` or
        // `TITLE_GAP` turns this red.
        let captions =
            ["File".to_string(), "Options".to_string(), "Help".to_string()];
        let boxes = title_boxes(&captions, |s| font.width(s));

        assert_eq!(boxes[0].x, 10, "the first title starts at the record's own x");
        for (i, b) in boxes.iter().enumerate() {
            assert!(b.w > 0, "title {i} measured zero: the font did not load");
            assert_eq!(b.y, 6, "every record's y is 6");
            assert_eq!(b.h, 12, "Menu_HitTitle's height is a fixed 12");
        }
        for pair in boxes.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert_eq!(b.x - (a.x + a.w), 32, "g_penAdvance += 0x20 between titles");
        }
        // "Options" is the longest of the three in any reasonable typeface, and
        // if all three came out equal the font is not being consulted at all.
        assert!(boxes[1].w > boxes[2].w, "Options must be wider than Help");
        // And the fallback metrics really are different metrics, so a machine
        // with no game is not silently getting the same answer.
        let fallback = title_boxes(&captions, l2_view::text::width);
        assert_ne!(fallback[1].w, boxes[1].w, "the 5 x 7 font must not measure Fntl2_14's widths");
    }

    /// Every item row is 144 wide and 15 tall in a 20-pixel pitch, and the first
    /// is 31 pixels below the title's baseline. Read off `FUN_0040E099`.
    #[test]
    fn the_item_rows_are_the_hit_tests_geometry() {
        let t = [Rect::new(10, 6, 30, 12), Rect::new(72, 6, 50, 12), Rect::new(154, 6, 30, 12)];
        let first = item_rect(&t, 1, 0);
        assert_eq!((first.x, first.y, first.w, first.h), (72, 6 + 0x1F, 0x90, 0x0F));
        let second = item_rect(&t, 1, 1);
        assert_eq!(second.y - first.y, ITEM_PITCH, "20-pixel pitch");
        assert!(second.y > first.y + first.h, "the five pixels between rows are dead");
    }
}
