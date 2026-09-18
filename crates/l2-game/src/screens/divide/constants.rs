#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::tests_part::*;
use l2_kingdom::divide::{SplitBasket, SplitInto, SplitRefusal};
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_view::{text, Canvas};
use crate::press::{Press, Widget};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};
use crate::widget;

/// `L2.eng` group 17 — *"Army Division."* and *"Split the army?"*.
pub const GROUP: usize = 17;
/// `L2.eng` group 8's troop nouns begin at index `0x34`, two apiece: singular
/// then plural. `Ui_DrawUnitNoun(count, 0x34 + t*2)` picks between them.
pub const NOUN_BASE: usize = 0x34;
/// `Ui_DrawUnitNoun` reads its plural out of **group 8**, not this screen's
/// group 17. The heading and the question are 17; every troop name is 8.
pub const NOUN_GROUP: usize = 8;
pub const TOTAL_MEN_NOUN: usize = 72;
/// The mercenary band's nationality — group 16, indexed by the band.
pub const GROUP_NATIONALITY: usize = 16;

pub const BOX_X: i32 = 8;
pub const BOX_Y: i32 = 0x30;
pub const BOX_COLS: i32 = 0x1C;
pub const BOX_ROWS: i32 = 0x1A;

pub fn window() -> Rect {
    Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS * 16)
}

pub const ROW_Y: i32 = 0x80;
pub const ROW_STEP: i32 = 0x20;
pub const MERC_ROW: usize = 7;

pub fn row_y(row: usize) -> i32 {
    ROW_Y + row as i32 * ROW_STEP
}

pub const NOUN_X: i32 = 0x18;
pub const PARENT_ICON_X: i32 = 0xA8;
pub const PARENT_NUMBER_X: i32 = 0xD8;
pub const DAUGHTER_ICON_X: i32 = 0x158;
pub const DAUGHTER_NUMBER_X: i32 = 0x188;

pub const ROWS_WELL_COLS: i32 = 0x1A;
pub const ROWS_WELL_ROWS: i32 = 0x12;

pub fn rows_well() -> Rect {
    Rect::new(0x18, 0x80, ROWS_WELL_COLS * 16, ROWS_WELL_ROWS * 16)
}

pub const OK: Rect = Rect::new(0x1AC, 0x1B4, 24, 24);

pub fn totals_y(band: bool) -> i32 {
    if band {
        0x184
    } else {
        0x160
    }
}

/// **`g_splitWidgets` (`0x004DD388`) — this screen's real controls, and they
/// were not on this screen at all.**
///
/// `Screen_FrameInput`'s `0x11` arm is three exits and no verb; every button
/// here belongs to `Screen_HandleInput`, which is the second of the three
/// places input hides (`docs/arms.json`'s `_note`). The table is
/// `Widget_Test(0, 0, &DAT_004DD388, DAT_0055321C)` with the count
/// `Panel_SplitButton` sets: **16 records with no mercenary band, 18 with one**,
/// so the band's pair exists only when the army has a band.
pub const BUTTON_DIM: i32 = 24;
pub const PARENT_BUTTON_X: i32 = 256;
pub const DAUGHTER_BUTTON_X: i32 = 288;

pub fn parent_button(row: usize) -> Rect {
    Rect::new(PARENT_BUTTON_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

/// The button-sheet frames records 2…17 carry. Frame 27 is the record whose
/// handler is `SplitScreen_ToParent` (`0x00437D65`) and 25 is
/// `SplitScreen_ToDaughter` (`0x00437E9E`) — the names come from the
/// **handlers**, not from the pictures, which is the direction that cannot be
/// got backwards.
pub const TO_PARENT_FRAME: usize = 27;
pub const TO_DAUGHTER_FRAME: usize = 25;

pub const OK_FRAME: usize = 0x33;

pub fn daughter_button(row: usize) -> Rect {
    Rect::new(DAUGHTER_BUTTON_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

pub const SPLIT_TICK: Rect = Rect::new(288, 420, 32, 32);
pub const SPLIT_CROSS: Rect = Rect::new(336, 424, 32, 32);

pub const TICK_INDEX: usize = (MERC_ROW + 1) * 2;

/// **`g_splitWidgets` (`0x004DD388`) as a table, with the kind byte each record
/// carries.**
///
/// Each `arm!` is its arm's marker and the kind it is answered with, in one
/// token: **`SplitScreen_ToParent` (`0x00437D65`) and `SplitScreen_ToDaughter`
/// (`0x00437E9E`)**, sixteen widgets in eight rows with `g_uiHotspotId`
/// carrying the slot, and **`Army_SplitConfirm` (`0x00437AFB`)** behind both
/// the tick and the cross.
pub fn widgets() -> Vec<Widget> {
    let mut out = Vec::with_capacity(18);
    for row in 0..=MERC_ROW {
        out.push(Widget::new(
            parent_button(row),
            crate::arm!("0x00437D65/divide-to-parent", Repeat),
        ));
        out.push(Widget::new(
            daughter_button(row),
            crate::arm!("0x00437E9E/divide-to-daughter", Repeat),
        ));
    }
    out.push(Widget::new(SPLIT_TICK, crate::arm!("0x00437AFB/divide-confirm", Delayed)));
    out.push(Widget::new(SPLIT_CROSS, crate::arm!("0x00437AFB/divide-cancel", Delayed)));
    out
}

// arm: ours/divide-click-moves-ten left-press
pub const CLICK_MEN: i32 = 10;

