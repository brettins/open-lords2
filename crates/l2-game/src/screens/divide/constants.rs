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
/// Group 8 index 72 — *"Total men"*. It was the string `"TOTAL MEN"` written
/// in our own source, which is the third kind of invention `docs/arms.json`
/// now carries: not a control we added, but a caption we wrote where the
/// original fetches one.
pub const TOTAL_MEN_NOUN: usize = 72;
/// The mercenary band's nationality — group 16, indexed by the band.
pub const GROUP_NATIONALITY: usize = 16;

/// `Ui_DrawBox(8, 0x30, 0x1C, 0x1A)`.
pub const BOX_X: i32 = 8;
pub const BOX_Y: i32 = 0x30;
pub const BOX_COLS: i32 = 0x1C;
pub const BOX_ROWS: i32 = 0x1A;

pub fn window() -> Rect {
    Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS * 16)
}

/// The rows: seven troop types, then the band, `0x20` apart from `0x80`.
pub const ROW_Y: i32 = 0x80;
pub const ROW_STEP: i32 = 0x20;
/// The mercenary band's row — index 7, at `0x80 + 7 * 0x20`.
pub const MERC_ROW: usize = 7;

pub fn row_y(row: usize) -> i32 {
    ROW_Y + row as i32 * ROW_STEP
}

/// The noun's x, and the two columns' icon and number positions.
///
/// **`0xA8` and `0x158` are where the painter draws the troop-type ICONS**, not
/// where the buttons are: `Pl8_DrawFrame(g_miscCtySheet, t + 0x2F, 0xA8, …)` and
/// the same at `0x158` are the same `Misc_cty` frames the information panel's
/// troop grid uses. See [`PARENT_BUTTON_X`] for the buttons, which are
/// somewhere else entirely.
pub const NOUN_X: i32 = 0x18;
pub const PARENT_ICON_X: i32 = 0xA8;
pub const PARENT_NUMBER_X: i32 = 0xD8;
pub const DAUGHTER_ICON_X: i32 = 0x158;
pub const DAUGHTER_NUMBER_X: i32 = 0x188;

/// `Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)` — the well the rows sit in,
/// **in cells of 16**, because that is what the primitive takes.
pub const ROWS_WELL_COLS: i32 = 0x1A;
pub const ROWS_WELL_ROWS: i32 = 0x12;

/// The same well in pixels, for the hit tests and the tests.
pub fn rows_well() -> Rect {
    Rect::new(0x18, 0x80, ROWS_WELL_COLS * 16, ROWS_WELL_ROWS * 16)
}

/// `Ui_OkButton(0x1AC, 0x1B4, 0)`.
pub const OK: Rect = Rect::new(0x1AC, 0x1B4, 24, 24);

/// The **totals** row: `0x184` when the army carries a band, `0x160` when it
/// does not.
///
/// `0x160` is *exactly* row 7's own y — `0x80 + 7 * 0x20` — so an army with no
/// mercenaries does not leave a gap where the band would have been: the totals
/// move up into its place. With a band they sit four pixels below it. That
/// four-pixel offset is the whole of the layout difference, and it is why the
/// painter carries two literals.
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
///
/// ```text
///  #    x    y  frame  handler                 kind  id
///  0  288  420     29  Army_SplitConfirm          5   1   the tick: SPLIT
///  1  336  424     31  Army_SplitConfirm          5   0   the cross: back to 0x04
///  2  256  120     27  SplitScreen_ToParent       4   0   |  eight rows,
///  3  288  120     25  SplitScreen_ToDaughter     4   0   |  0x20 apart,
///  …                                                      |  ids 0 … 7
/// 17  288  344     25  SplitScreen_ToDaughter     4   7   |
/// ```
///
/// **The buttons are at x 256 and 288 and ours were at 168 and 344**, which are
/// [`PARENT_ICON_X`] and [`DAUGHTER_ICON_X`] — the two columns of `Misc_cty`
/// troop-type *pictures*. So our hit boxes sat on the artwork and the
/// original's live controls, in the 32-pixel gutter between the parent's number
/// and the daughter's icon, had nothing over them. The y is right and always
/// was: `row_y(row) - 8` is the table's `row * 0x20 + 0x78` exactly.
///
/// That is the fifth time a hit box on this project has been placed by reading
/// a painter; `screens/map/mod.rs`'s header carries the
/// standing warning and this is now one of its examples.
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

/// `Ui_OkButton`'s picture: `System.pl8` frame 0x33, **an arrow into a hole**.
/// It is not a tick and it is not the word "OK",
/// a caption invented where the original draws artwork.
pub const OK_FRAME: usize = 0x33;

pub fn daughter_button(row: usize) -> Rect {
    Rect::new(DAUGHTER_BUTTON_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

/// **The tick, `Army_SplitConfirm` with `g_uiHotspotId == 1`** — 32 square at
/// (288, 420), `System.pl8` frame 29.
pub const SPLIT_TICK: Rect = Rect::new(288, 420, 32, 32);
/// **The cross, the same handler with `g_uiHotspotId == 0`** — frame 31 at
/// (336, 424). It is the *first* statement of `Army_SplitConfirm`:
/// `g_screenId = 0; … if (g_uiHotspotId == 0) g_screenId = 4;`, so the cross
/// goes back to the information panel and splits nothing.
pub const SPLIT_CROSS: Rect = Rect::new(336, 424, 32, 32);

/// [`widgets`]' index of the tick; the cross is the one after it.
pub const TICK_INDEX: usize = (MERC_ROW + 1) * 2;

/// **`g_splitWidgets` (`0x004DD388`) as a table, with the kind byte each record
/// carries.**
///
/// Eighteen records: two kind-**5** — the tick and the cross, so they
/// are first in the table and last here — and sixteen kind-**4** steppers, one
/// pair per row. The order below is `row * 2` for a parent button and
/// `row * 2 + 1` for a daughter one, which is the order the table itself is in
/// past its first two records.
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

/// How many men a click moves. **Ours**: `SplitScreen_ToParent` moves exactly
/// one — `slot.available -= 1; slot.chosen += 1` — which is unusable for an
/// army of 800 without the key-repeat its widget table's kind 4 supplies and
/// our event loop does not deliver here yet. Ten is
/// [`l2_kingdom::divide`]'s own round number, and the arrow keys still move
/// one. `docs/arms.json` `ours/divide-click-moves-ten`.
///
/// **The band is the exception and it is the original's**: hotspot id 7 swaps
/// the whole band, in both directions.
// arm: ours/divide-click-moves-ten left-press
pub const CLICK_MEN: i32 = 10;

