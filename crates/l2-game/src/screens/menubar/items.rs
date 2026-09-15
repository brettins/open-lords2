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
    Slider(&'static str),
    Help(u32),
    About,
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
    let s = ctx.assets.shell.text(m.group, m.items[item].1);
    if s.is_empty() {
        m.item_fallbacks[item].to_string()
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
        let w = measure(caption);
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
    let row = MENUS[menu].items[item].0;
    Rect::new(titles[menu].x, BAR_Y + row + ITEM_TOP, ITEM_W, ITEM_H)
}

/// **The plate `FUN_00409429(x, y + 0x12, 0x0C, h)` draws**, in pixels:
pub fn plate_rect(titles: &[Rect; 3], menu: usize) -> Rect {
    let rows = plate_rows(MENUS[menu].items.len());
    Rect::new(
        titles[menu].x,
        BAR_Y + PLATE_DY,
        PLATE_COLS * PLATE_CELL,
        rows * PLATE_CELL,
    )
}

