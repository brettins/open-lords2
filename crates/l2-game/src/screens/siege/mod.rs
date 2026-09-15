//! **The siege-preparation screen** — `Screen_SiegePrep` (`0x00421F14`),
//! `g_screenId` `0x1D`, `L2.eng` group 83.
//!
//! Transcribed from `Screen_SiegePrep` (`0x00421F14`), with the coordinates
//! resolved to decimal in the trailing comment. **`FUN_0040437D(x, y, w, h, c)`
//! is a filled rectangle** — a loop of `FUN_00403A8F` horizontal lines — and
//! **`FUN_004093E0` is `Ui_DrawBoxBorder(1, …)` plus `Ui_DrawBoxInterior`**, so
//! the window is border set **1**, not 0. Neither is in the draw-call
//! extractor's primitive list
//! painter (28) is eight short of the truth.
//!
//! ```text
//! Screen_SiegePrep():                                            0x00421F14
//!   FUN_004093E0(0x10, 0x30, 0x1C, 0x19)      window, set 1, (16, 48) 448 x 400
//!   File_ReadChunk("sgeplans.pl8", spriteBuffer, 100000, 0)
//!   DAT_0058FE2C := 1                         drop capitals on for the whole screen
//!   if county.castleType != 0:
//!     Sprite_WGenSprite(castleType - 1, 0x150, 0x40)       (336, 64) the castle plan
//!   Eng_DrawString(83, 0, 0x30, 0x58, heading)   "Siege preparations."   (48, 88)
//!   Eng_DrawString(83, 4, 0x40, 0x78, body)      "Siege will take"       (64, 120)
//!   Ui_DrawCount(unit +0x19C, 0x42, 0x50, 0x88)  N Season/Seasons        (80, 136)
//!   Eng_DrawString(83, 5, pen + 0x20, 0x88)      "to make ready."   y 136, after it
//!   for row in 0..3, y = 0xC0 + 0x44 * row:                    192, 260, 328
//!     Eng_DrawString(83, 1 + row, 0x48, y)       the engine's name       x 72
//!     Ui_DrawInsetRect(0x50, y + 0x18, 0x34, 8)  the well, FOUR LINES AND NO FILL
//!     FUN_0040437D(0x51, y + 0x19, 0x32, 6, 0xF9)          the empty bar, 50 wide
//!     FUN_0040437D(0x51, y + 0x19, record[+2] / 2, 6, 0xFA)     the fill, percent/2
//!     Ui_DrawNumber(record[+2], '@', "%", 0x90, y + 0x19)  the percentage   x 144
//!     if record[+0] == 0:
//!       Eng_DrawString(83, 8, 0xF0, y + 0x10)  "- No engines to be built"  x 240
//!     else for n in 0..record[+0]:
//!       Pl8_DrawFrame(misc_cty, 0x43 + row, 0xD0 + n * step, y - 8)   step 60/60/80
//!   Ui_DrawBevelRect(0x68, 0x18C, 100, 0x1C)                       (104, 396)
//!   FUN_0040437D(0x69, 0x18D, 0x62, 0x1A, 0x18)              the button's fill
//!   Eng_DrawString(83, 6, 0x70, 0x194)          "Lift siege"        (112, 404)
//!   Ui_DrawBevelRect(0x108, 0x18C, 100, 0x1C)                      (264, 396)
//!   FUN_0040437D(0x109, 0x18D, 0x62, 0x1A, 0x18)
//!   Eng_DrawString(83, 7, 0x110, 0x194)         "Proceed"           (272, 404)
//!   DAT_0058FE2C := 0
//! ```
//!
//! Every line of it was `l2_view::text::draw` — our 5 × 7 debug font — with
//! the English typed into the source, against a painter that makes eleven
//! `Eng_DrawString` calls on `L2.eng` group 83 and three `Pl8_DrawFrame`s.
//!
//! **the window was drawn with border set 0** (the painter's `FUN_004093E0`
//! passes 1), and **the percent bar's well was filled before it was framed**,
//! which `Ui_DrawInsetRect` does not do — the exact defect `docs/decisions.md`
//! C61 records against the raise-army screen, where the fill is invisible
//! under our palette and pitch black under the game's.
//!
//! **The row order is the record order, and that is what settles which engine
//! costs what.** Row 1 is drawn from `+0x182` and labelled `L2.eng` 83/1
//! *"Catapults"*; row 2 from `+0x188`, *"Siege towers"*; row 3 from `+0x18E`,
//! *"Battering rams"*. `Army_PrepareForBattle` maps those same three records to
//! troop types 7, 8 and 9, and `g_siegeEngineWork` is indexed by the same
//! record number — so the catapult is 200 man-seasons, the tower 200 and the
//! ram 400. `[V]`
//!
//! Each engine row has **two** hotspots, and `g_siegeWidgets` (`0x004DDF10`)
//! gives all six their coordinates: one increments the order (`0x0043B681`) and
//! one decrements it (`0x0043B741`), and both end in `0x0043B7C4`, which runs
//! `Siege_RecomputeBuildTime` — so the *"Siege will take N Season(s)"* line
//! moves as you click. The increment stops at
//! [`l2_kingdom::siege::ENGINE_ORDER_CAP`] — **four catapults, four towers, two
//! rams** — which nobody had read before. Cap times cost is 800 man-seasons for
//! all three rows, which is a second, independent statement that
//! `g_siegeEngineWork` is `[200, 200, 400]` in that order.
//!
//! *"Lift siege"* calls `Siege_Break` and closes. *"Proceed"* closes, and
//! **launches the assault only if the countdown is already zero** — which it is
//! when nothing has been ordered. So a player besieging a palisade can order
//! nothing and storm it the same season, and a player besieging a stone castle
//! who does the same is refused by `Siege_LaunchAssault`'s gate with `L2.eng`
//! 281.

mod view;
pub use view::*;

use l2_kingdom::siege::{self, Engine, ENGINES, ENGINE_ORDER_CAP};
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};
use crate::widget;

/// **`L2.eng` group 83.** Verified against the *words*, not against the indices
/// existing: index 0 is *"Siege preparations."* and 1…3 are *"Catapults"*,
/// *"Siege towers"*, *"Battering rams"* in the order the three engine records
/// sit in the unit. `docs/formats/eng.md` §5 files the group here too.
pub const GROUP: usize = 83;
pub const HEADING: usize = 0;
pub const ENGINE_LABEL: [usize; 3] = [1, 2, 3];
pub const WILL_TAKE: usize = 4;
pub const TO_MAKE_READY: usize = 5;
pub const LIFT_SIEGE: usize = 6;
pub const PROCEED_LABEL: usize = 7;
pub const NO_ENGINES: usize = 8;

/// `Ui_DrawCount(seasons, 0x42, …)` — **group 8**, not group 83: index 66
/// *"Season"* and 67 *"Seasons"*. The singular/plural rule is
/// [`crate::shell::count_noun`] — `±1` takes the singular and everything else,
/// **zero included**, takes the plural.
pub const SEASON_NOUN: usize = 0x42;

pub const PLAN_SHEET: &str = "Sgeplans.pl8";
pub const PLAN_AT: (i32, i32) = (0x150, 0x40);

pub const ENGINE_FRAME0: usize = 0x43;

/// `FUN_004093E0(0x10, 0x30, 0x1C, 0x19)` — the window: origin in **pixels**,
/// size in 16-pixel **cells**
/// same one `Panel_JobDetail` uses. 448 × 400 at (16, 48).
pub const BOX_X: i32 = 0x10;
pub const BOX_Y: i32 = 0x30;
pub const BOX_COLS: i32 = 0x1C;
pub const BOX_ROWS: i32 = 0x19;
/// **Border set one.** `FUN_004093E0`'s whole body is
/// `Ui_DrawBoxBorder(1, x, y, cols, rows)` followed by `Ui_DrawBoxInterior`
/// inset one cell; passing 0 draws the other frame kit and a visibly wrong
/// window. `screens/battle.rs` carries the same constant for the same reason.
pub const BOX_SET: usize = 1;

pub const HEADING_AT: (i32, i32) = (0x30, 0x58);
pub const WILL_TAKE_AT: (i32, i32) = (0x40, 0x78);
pub const SEASONS_AT: (i32, i32) = (0x50, 0x88);

pub const ROW_Y: [i32; 3] = [0xC0, 0x104, 0x148];
pub const LABEL_X: i32 = 0x48;
pub const BAR_X: i32 = 0x50;
pub const BAR_W: i32 = 0x34;
pub const BAR_H: i32 = 8;
pub const BAR_DY: i32 = 0x18;
/// `FUN_0040437D(0x51, rowY + 0x19, 0x32, 6, 0xF9)` then the same rectangle
/// `percent / 2` wide in `0xFA` — the trough is 50 pixels inside a 52-pixel
/// well, which is what makes `percent / 2` reach exactly the far end at 100.
pub const TROUGH: (i32, i32, i32, i32) = (0x51, 0x19, 0x32, 6);
pub const TROUGH_EMPTY: u8 = 0xF9;
pub const TROUGH_FULL: u8 = 0xFA;
/// `Ui_DrawNumber(percent, '@', "%", 0x90, rowY + 0x19)` — the suffix at
/// `0x004D4404` is one byte, `0x25`, the per-cent sign.
pub const PERCENT_X: i32 = 0x90;
pub const PERCENT_DY: i32 = 0x19;
pub const NO_ENGINES_AT: (i32, i32) = (0xF0, 0x10);
pub const SPRITE_X: i32 = 0xD0;
pub const SPRITE_STEP: [i32; 3] = [0x3C, 0x3C, 0x50];
pub const SPRITE_DY: i32 = -8;
pub const BUTTON_LABEL_DX: i32 = 8;
pub const BUTTON_LABEL_Y: i32 = 0x194;
/// `FUN_0040437D(x + 1, y + 1, 0x62, 0x1A, 0x18)` — the plate inside the bevel.
pub const BUTTON_FILL: u8 = 0x18;

pub const LIFT: Rect = Rect::new(0x68, 0x18C, 100, 0x1C);
pub const PROCEED: Rect = Rect::new(0x108, 0x18C, 100, 0x1C);

/// `g_siegeWidgets` (`0x004DDF10`) holds six records: the even ones are the
/// increment buttons at **x 38, y 184 / 252 / 320** drawing button frame 21,
/// and the odd ones the decrement buttons **26 pixels below each** drawing
/// frame 23, with hotspot ids 0, 1, 2 for both. That table was read into
/// `docs/hypotheses.json` before this screen existed and is what makes the
/// layout the original's
///
/// The two handlers are `SiegePrep_OrderMore` (`0x0043B681`) and
/// `SiegePrep_OrderFewer` (`0x0043B741`), and each row's button pair sits eight
/// pixels above its `L2.eng` label at [`ROW_Y`].
///
/// **The table holds exactly six records and every caller passes six.** It was
/// decoded to ten to look for `g_sendSuppliesWidgets`' cut row: records 6…9
/// are `g_smackTestWidgets` (`0x004DDFA0`) — the minus/plus pair at (208, 232)
/// and (240, 232) and the yes/no pair at (288, 280) and (324, 284)
/// `docs/symbols.json` describes independently — so there is nothing hidden
/// behind this count.
pub const BUTTON_X: i32 = 38;
pub const BUTTON_Y: [i32; 3] = [184, 252, 320];
pub const BUTTON_STEP: i32 = 26;
/// The button sprites are **`System.pl8`** frames 21 and 23, and this module
/// used to say `Panels.pl8`. `Widget_Draw` (`0x0040CFD2`) picks the sheet from
/// the record's *size* field: below `0x18` it draws from `g_miscCtySheet` and
/// otherwise from `g_systemSheet`, and these six records carry 24. The size
/// **is** in the table, so the 24 × 24 box is the original's too.
pub const BUTTON_DIM: i32 = 24;
/// The frame each button draws, and `frame + 1` while it is held —
/// `Widget_Draw` adds one for as long as the press timer at `+0x0D` runs.
pub const WIDGET_FRAME_PLUS: usize = 21;
pub const WIDGET_FRAME_MINUS: usize = 23;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeChoice {
    None,
    Lift,
    Assault,
    Wait,
}

pub struct SiegeScreen {
    unit: usize,
    pub choice: SiegeChoice,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_rows_are_the_three_records_in_the_order_the_labels_name_them() {
        assert_eq!(ENGINES[0].name(), "Catapults");
        assert_eq!(ENGINES[1].name(), "Siege towers");
        assert_eq!(ENGINES[2].name(), "Battering rams");
        assert_eq!(ENGINE_ORDER_CAP, [4, 4, 2]);
        for (engine, cap) in ENGINES.iter().zip(ENGINE_ORDER_CAP) {
            assert_eq!(
                siege::ENGINE_WORK[engine.index()] * cap as i32,
                800,
                "{}",
                engine.name()
            );
        }
    }

    #[test]
    fn the_six_row_hotspots_are_distinct_and_clear_of_the_buttons() {
        let mut spots: Vec<Rect> = Vec::new();
        for row in 0..3 {
            spots.push(row_plus(row));
            spots.push(row_minus(row));
        }
        for (i, a) in spots.iter().enumerate() {
            for (j, b) in spots.iter().enumerate() {
                if i == j {
                    continue;
                }
                let apart = a.x + a.w <= b.x
                    || b.x + b.w <= a.x
                    || a.y + a.h <= b.y
                    || b.y + b.h <= a.y;
                assert!(apart, "hotspots {i} and {j} overlap");
            }
            assert!(a.y + a.h <= LIFT.y, "hotspot {i} runs into the buttons");
            assert!(SiegeScreen::window().contains(a.x, a.y), "hotspot {i} is off the window");
        }
    }
}

