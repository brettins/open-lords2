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
//! # The drop-down is screen `0x32`
//!
//! `Menu_OpenDropdown` saves `g_screenId` into `g_menuPrevScreen`, writes
//! **`0x32`**, and calls `Menu_SaveBackdrop` (`0x0040C8D1`), which copies the
//! **400 × 180 band at (0, 24)** so the painter can put it back. `Screen_Draw`'s
//! `0x32` arm is `Menu_RestoreBackdrop` and nothing else — the drop-down is a
//! screen that owns a rectangle, which is `docs/screens-county.md` §3.1's
//! pattern for the third time.
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
//! * **The panel behind the items.** The original saves the screen band and
//!   draws the captions straight onto it with no plate at all; we draw a recess
//!   so the words are readable over the map. Marked where it is drawn.
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
const ITEM_PITCH: i32 = 20;

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
    let mut out = [Rect::new(0, 0, 0, 0); 3];
    let mut pen = BAR_X;
    for (i, slot) in out.iter_mut().enumerate() {
        let caption = title_text(ctx, i);
        let w = match ctx.assets.shell.body.as_ref() {
            Some(f) => f.width(&caption),
            None => text::width(&caption),
        };
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

/// The whole band the open drop-down covers, for the panel we draw behind it.
/// **Ours** — the original saves and restores the screen instead.
fn panel_rect(titles: &[Rect; 3], menu: usize) -> Rect {
    let n = MENUS[menu].items.len() as i32;
    Rect::new(titles[menu].x - 4, BAR_Y + ITEM_TOP - 4, ITEM_W + 8, (n - 1) * ITEM_PITCH + ITEM_H + 8)
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
            Item::About => Transition::Replace(ScreenId::Shell(0x25)),
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

        // OURS: the original saves the 400 x 180 band at (0, 24) and paints the
        // captions straight onto whatever was there. We draw a recess, because
        // a caption over the campaign map at our scale is unreadable.
        let panel = panel_rect(&t, self.menu);
        crate::widget::panel(canvas, ink, panel);

        for i in 0..MENUS[self.menu].items.len() {
            let r = item_rect(&t, self.menu, i);
            let picked = self.hover == Some(i);
            if picked {
                canvas.fill_rect(r.x, r.y, r.w, r.h, ink.highlight);
            }
            let caption = item_text(ctx, self.menu, i);
            let colour = if picked { ink.background } else { ink.text };
            match ctx.assets.shell.body.as_ref() {
                Some(f) => {
                    f.draw(canvas, r.x + 2, r.y, &caption, &font::Style::new(colour));
                }
                None => {
                    text::draw(canvas, r.x + 2, r.y + 4, &caption, colour);
                }
            }
        }

        if !self.status.is_empty() {
            text::draw(canvas, panel.x, panel.y + panel.h + 4, &self.status, ink.bad);
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
            // `FUN_004B414A(x - 2, y - 3, 0x3F)` — the plate the open title
            // stands on, two left and three up, in the strip's own black.
            canvas.fill_rect(r.x - 2, r.y - 3, r.w + 4, 18, font::TEXT);
        }
        let colour = if lit { 0x18 } else { font::TEXT };
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
