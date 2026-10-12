#![allow(unused_imports)]
use super::*;
use super::dropdown::*;
use super::render::*;
use super::tests_part::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::options::Page;
use crate::screens::saveload::Mode;
use crate::shell::font;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    NewGame,
    Load,
    Save,
    Quit,
    Options(Page),
    Slider(crate::screens::slider::Spin),
    Help(u32),
    About,
    /// C257: a joiner's Quit.
    LeaveGame,
    /// C257: the host's Quit.
    EndGame,
}

/// C257: an item with no `L2.eng` string; [`ours`] says it.
pub const OURS: usize = usize::MAX;

/// C257: a network game's File menu. No New Game and no Load (C245: a load would
/// change one seat's world); Save is the host's alone; Quit is *Leave game* for a
/// joiner and *End game for everyone* for the host. `[D]` ours: the original kept
/// all four on every seat (`Menu_SaveGame` `0x00433F49` globs `.sva`/`.svb` by
/// `g_netIsMaster`; `SaveLoad_Tick` `0x004AD9F0` sends net command `0x17`/`0x18`).
pub const HOST_FILE: &[(i32, usize, Item)] = &[(0, 3, Item::Save), (20, OURS, Item::EndGame)];
pub const JOINER_FILE: &[(i32, usize, Item)] = &[(0, OURS, Item::LeaveGame)];

/// The items menu `menu` shows in this game.
pub fn rows(game: &crate::Game, menu: usize) -> &'static [(i32, usize, Item)] {
    match (menu, game.net.lockstep, game.net.hosting) {
        (0, true, true) => HOST_FILE,
        (0, true, false) => JOINER_FILE,
        _ => MENUS[menu].items,
    }
}

/// C257's captions; title case as `L2.eng` group 1 is.
pub fn ours(item: Item) -> &'static str {
    match item {
        Item::LeaveGame => "Leave game",
        // [D] the plate is the original's 192 px; the box it opens says "for everyone".
        Item::EndGame => "End game for all",
        _ => "",
    }
}

/// One drop-down: its `L2.eng` group and its items, `(y, stringIndex, handler)`.
pub struct Menu {
    pub group: usize,
    pub items: &'static [(i32, usize, Item)],
    /// Ours: the caption to fall back on with no `L2.eng` loaded.
    pub fallback: &'static str,
    pub item_fallbacks: &'static [&'static str],
}

/// **`g_menuBarItems` (`0x004DC428`) and its three item tables**, verbatim from
/// `node tools/oracle/widgets.js menu 4dc428 3`.
///
/// Four items, five items, seven items; `L2.eng` groups 1, 2 and 3 hold exactly
/// four, five and seven strings after their label, the `y` column is 0, 20,
/// 40, … with no gaps and the string indices are 1, 2, 3, … with no gaps.
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
            (60, 4, Item::Slider(crate::screens::slider::Spin::GameSpeed)),
            (80, 5, Item::Slider(crate::screens::slider::Spin::ScrollSpeed)),
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
pub fn title_text(ctx: &Ctx, menu: usize) -> String {
    let m = &MENUS[menu];
    let s = ctx.assets.shell.text(m.group, 0);
    if s.is_empty() {
        m.fallback.to_string()
    } else {
        s.to_string()
    }
}

pub fn item_text(ctx: &Ctx, menu: usize, item: usize) -> String {
    let m = &MENUS[menu];
    let (_, index, what) = rows(ctx.game, menu)[item];
    if index == OURS {
        return ours(what).to_string();
    }
    let s = ctx.assets.shell.text(m.group, index);
    if s.is_empty() {
        let n = m.items.iter().position(|r| r.1 == index).unwrap_or(0);
        m.item_fallbacks[n].to_string()
    } else {
        s.to_string()
    }
}

pub fn titles(ctx: &Ctx) -> [Rect; 3] {
    let captions = [title_text(ctx, 0), title_text(ctx, 1), title_text(ctx, 2)];
    title_boxes(&captions, |s| match ctx.assets.shell.body.as_ref() {
        Some(f) => f.width(s),
        None => text::width(s),
    })
}

pub fn title_boxes(captions: &[String; 3], measure: impl Fn(&str) -> i32) -> [Rect; 3] {
    let mut out = [Rect::new(0, 0, 0, 0); 3];
    let mut pen = BAR_X;
    for (slot, caption) in out.iter_mut().zip(captions) {
        // Ui_DrawText 0x00402637 00400000.c:1191 `g_penAdvance += 4` after the last glyph;
        // Ui_DrawMenuTitles 0x0040C5B0 stores that pen as measuredX (`items[1]`) [V].
        let w = measure(caption) + font::SPACE_ADVANCE;
        *slot = Rect::new(pen, BAR_Y, w, TITLE_H);
        pen += w + TITLE_GAP;
    }
    out
}

/// `Menu_HitTitle` (`0x0040E00A`) — the 1-based index, or `None`.
pub fn title_at(ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
    titles(ctx).iter().position(|r| r.contains(x, y))
}

/// The rectangle one item of an open drop-down occupies — `FUN_0040E099`.
pub fn item_rect(titles: &[Rect; 3], menu: usize, item: usize) -> Rect {
    row_rect(titles, menu, MENUS[menu].items[item].0)
}

/// [`item_rect`] for a row at `y` of whichever table is shown.
pub fn row_rect(titles: &[Rect; 3], menu: usize, y: i32) -> Rect {
    Rect::new(titles[menu].x, BAR_Y + y + ITEM_TOP, ITEM_W, ITEM_H)
}

/// **The plate `FUN_00409429(x, y + 0x12, 0x0C, h)` draws**, in pixels:
pub fn plate_rect(titles: &[Rect; 3], menu: usize) -> Rect {
    plate_rect_of(titles, menu, MENUS[menu].items.len())
}

/// `FUN_00409429`'s plate for `count` items: `(count * 0x15) / 16 + 2` cells.
pub fn plate_rect_of(titles: &[Rect; 3], menu: usize, count: usize) -> Rect {
    let rows = plate_rows(count);
    Rect::new(
        titles[menu].x,
        BAR_Y + PLATE_DY,
        PLATE_COLS * PLATE_CELL,
        rows * PLATE_CELL,
    )
}

