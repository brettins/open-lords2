#![allow(unused_imports)]
use super::*;
use super::draw::*;
use super::strip::*;
use l2_kingdom::tables::{
    HEALTH_BAND_NAMES, JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT, RATION_NAMES,
};
use l2_view::chrome::{self, misc_cty, system};
use l2_view::{text, Canvas};
use crate::game::{MAX_RATION_SPLIT, MAX_TAX_RATE};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen, TRAILING};
use crate::widget;

// ----------------------------------------------------------------- geometry
//
// Every rectangle below is `Ui_DrawBox(x, y, cols, rows)` out of the panel's
// own painter, in cells of 16 pixels. `docs/screens-county.md` §5.

/// `Panel_Population` (`0x004110B1`): `Ui_DrawBox(0x10, 0x30, 0x1C, 0x17)`.
const POPULATION_BOX: (i32, i32, i32, i32) = (16, 48, 28, 23);
/// `Panel_Happiness` (`0x004116FB`): `Ui_DrawBox(0x10, 0x30, 0x1C, 0x18)`.
const HAPPINESS_BOX: (i32, i32, i32, i32) = (16, 48, 28, 24);
/// `Panel_Tax` (`0x0041152F`): `Ui_DrawBox(0x50, 0x90, 0x14, 0x09)`.
const TAX_BOX: (i32, i32, i32, i32) = (80, 144, 20, 9);
/// `Panel_Ration` (`0x00411B72`): `Ui_DrawBox(0x80, 0x60, 0x12, 0x0F)`. The
/// height is 17 cells with *Armies Eat* on; we always draw the short one,
/// because we never draw the foraging line it makes room for.
const RATION_BOX: (i32, i32, i32, i32) = (128, 96, 18, 15);

/// `Ui_HistoryGraph(0x20, 0x54, mode)` opens with
/// `Ui_DrawInsetRect(x, y, 0x192, 0x9B)` — 402 × 155 at (32, 84) — and both
/// graph panels call it with the same origin.
const GRAPH: Rect = Rect::new(32, 84, 402, 155);

/// The county strip's own rectangle: `Misc_cty` frame 55 is 162 × 94 and
/// `Screen_DrawCampaign` draws it at (478, 156).
pub const STRIP: Rect = Rect::new(478, chrome::PANEL_MIDDLE_Y, 162, 94);

/// `CountyStrip_Click`'s outer guard, made half-open: the original tests
/// `mx > 487 && mx < 630` and `my > 181 && my < 241`.
const HOT_X0: i32 = 488;
const HOT_X1: i32 = 630;
const HOT_Y0: i32 = 182;
const HOT_Y1: i32 = 241;
/// Its two splits. `548 … 567` is a dead band 19 pixels wide, and the health
/// thermometer — 14 wide, drawn at x = 552 — sits inside it with two pixels
/// spare on each side. The gap exists for the bar.
const HOT_X_LEFT_END: i32 = 548;
const HOT_X_RIGHT_START: i32 = 568;
const HOT_Y_SPLIT: i32 = 212;

/// `Pl8_DrawFrameHere(g_miscCtySheet, healthBand + 0x46, 0x228, 0xB5)`.
const THERMOMETER: (i32, i32) = (552, 181);
const THERMOMETER_FRAME: usize = 0x46;
const THERMOMETER_W: i32 = 14;

/// The county strip's three farming rows, by labour slot, as
/// (plain, below the floor, **above the ceiling — the blue ring**).
///
/// **`[V]`.** Every drawer in `CountyStrip_Draw`'s left-hand row loop asks the
/// same question first: `if (county.labour[slot].useful <
/// county.labour[slot].workers)` — *more people on this job than it can use* —
/// and draws the ringed frame two pixels up and left of where the plain one
/// goes. That is the signal the player described as *"a blue outline for idle
/// peasants (eg too many on dairy)"*, and dairy is the second row.
///
/// A `None` in the middle column is a job whose drawer has no shortfall frame
/// at all: `FUN_004103C5` tests only the ceiling. The frame numbers themselves
/// are [`misc_cty::RINGED_PAIRS`](l2_view::chrome::misc_cty::RINGED_PAIRS).
///
/// | slot | drawer | job |
/// |---|---|---|
/// | 0 | `FUN_0041023A` | grain farming |
/// | 1 | `FUN_004100AF` | cattle farming — the dairy |
/// | 2 | `FUN_004103C5` | field reclamation |
const PRODUCE_ICONS: [(usize, Option<usize>, usize); 3] = [
    (misc_cty::RINGED_PAIRS[0].0, Some(misc_cty::SHORTFALL_GRAIN), misc_cty::RINGED_PAIRS[0].1),
    (misc_cty::RINGED_PAIRS[1].0, Some(misc_cty::SHORTFALL_CATTLE), misc_cty::RINGED_PAIRS[1].1),
    (misc_cty::RINGED_PAIRS[2].0, None, misc_cty::RINGED_PAIRS[2].1),
];

/// Both graph panels put their labels at x = 48 (`0x30`) and **start** their
/// values at x = 336 (`0x150`).
///
/// **Not right-anchored.** `Ui_DrawNumber` and `Ui_DrawDelta` both end in a
/// plain `Ui_DrawText(buffer, x, …)` and nothing measures the string; the `'@'`
/// lead is a zero-width glyph that reserves the sign column so a `+7` and a `7`
/// line up, which is what makes the column *look* right-anchored in a
/// screenshot. `docs/screens-county.md` §5.1 says *"values right-anchored from
/// x = 336"* and it is wrong.
const LABEL_X: i32 = 48;
const VALUE_LEFT: i32 = 336;

/// `Panel_Ration`'s three food columns: `Ui_DrawNumberRight(…, x, y, 0x40, …)`,
/// which **centres** the number in those 64 pixels. Grain, cattle and the third
/// source, under the icon row at (224, 256), (284, 256) and (344, 256).
const FOOD_COL_X: [i32; 3] = [0xD0, 0x10A, 0x144];
const FOOD_COL_W: i32 = 0x40;

/// `CountyStrip_JobClick`'s hit box: `x 0x1DE … 0x27F, y 0x12E … 0x1AD`.
/// column split at `0x230`.
const JOBS_Y0: i32 = 0x12E;
const JOBS_Y1: i32 = 0x1AE;
const JOBS_SPLIT_X: i32 = 0x230;

/// **`FUN_0040FEC1`'s left-hand list**, verbatim and in its own order: cattle,
/// then grain, then reclamation — each entry the **labour slot** the row is
/// about, which is also what `CountyStrip_JobClick` writes into `g_jobPanelJob`
/// plus one.
pub fn farm_rows(c: &l2_kingdom::county::County) -> Vec<usize> {
    let mut rows = Vec::with_capacity(3);
    if c.fields_cattle != 0 || c.herd != 0 {
        rows.push(1);
    }
    if c.fields_grain != 0 || c.grain != 0 {
        rows.push(0);
    }
    if c.fields_reclaiming != 0 {
        rows.push(2);
    }
    rows
}

/// **`FUN_0040FEC1`'s right-hand list**, and its order is not the industry
/// array's: wood, iron, stone, weapons, then the castle.
///
/// Each of the four is gated on `industry[n].enabled` — county `+0x297 + n*0x18`,
/// the byte `Industry_ToggleFromMap` (`0x0043D309`) XORs when you click the
/// building on the campaign map — and the castle row on `castleDegraded`.
pub fn industry_rows(c: &l2_kingdom::county::County) -> Vec<usize> {
    let mut rows = Vec::with_capacity(5);
    for (industry, slot) in [(0usize, 6usize), (1, 4), (3, 5), (2, 7)] {
        if c.industry[industry].enabled {
            rows.push(slot);
        }
    }
    if c.castle_degraded != 0 {
        rows.push(3);
    }
    rows
}

/// `DAT_0053E970` and `DAT_0053F04C`, the two row pitches, which
/// `CountyStrip_Draw` picks from the two counts.
///
/// **They are not the same rule.** The farm column has two cases.
/// industry column three, because the industry column can hold five rows:
///
/// ```c
/// DAT_0053E970 = farmRows     < 3 ? 0x3C : 0x2D;
/// DAT_0053F04C = industryRows < 3 ? 0x3C : industryRows < 4 ? 0x2D : 0x1E;
/// ```
pub fn farm_pitch(rows: usize) -> i32 {
    if rows < 3 {
        0x3C
    } else {
        0x2D
    }
}

pub fn industry_pitch(rows: usize) -> i32 {
    if rows < 3 {
        0x3C
    } else if rows < 4 {
        0x2D
    } else {
        0x1E
    }
}

/// **`CountyStrip_JobClick` (`0x00438E3B`)** — which labour slot a press in the
/// sidebar's lower plate opens the job popup for, or `None`.
///
/// The caller supplies the ownership gate, which is the function's own first
/// line: `if (g_counties[g_selectedCounty].owner == g_localPlayer)`. Everything
/// else is here, including the two ways it refuses — a row index past the end of
/// its list returns 0.
pub fn job_row_at(c: &l2_kingdom::county::County, x: i32, y: i32) -> Option<usize> {
    if !(478..640).contains(&x) || !(JOBS_Y0..JOBS_Y1).contains(&y) {
        return None;
    }
    let (rows, pitch) = if x < JOBS_SPLIT_X {
        let rows = farm_rows(c);
        let pitch = farm_pitch(rows.len());
        (rows, pitch)
    } else {
        let rows = industry_rows(c);
        let pitch = industry_pitch(rows.len());
        (rows, pitch)
    };
    let n = ((y - JOBS_Y0) / pitch) as usize;
    rows.get(n).copied()
}

/// `CountyStrip_Click` (`0x00438CEB`) itself: which panel a pixel opens, or
/// `None` for the outer guard and for the thermometer's dead band.
///
/// **This is the whole navigation into the four panels** — §2.3 — so it lives
/// here as one function,
/// and the campaign map and the county screen both call it.
pub fn panel_at(x: i32, y: i32) -> Option<Panel> {
    PANELS.into_iter().find(|p| p.strip_hotspot().contains(x, y))
}

// -------------------------------------------------- the ration split slider
//
// `Panel_RationSlider` (`0x00411FDE`) draws it and `Ration_SliderClick`
// (`0x0043A379`) hit-tests it. The caps are drawn at 200 and 324 and are 24
// wide, the track runs 224 … 323.

/// **Two y's, not one, and this file used to have one.** `Panel_RationSlider`
/// draws the knob at `0xD8` = 216 and both caps at `0xDC` = 220, and
/// `Ration_SliderClick` hit-tests all three boxes at `0xDC` —
/// constant of 216 put every control four pixels above where the game has it,
/// drawn *and* clickable.
const SLIDER_KNOB_Y: i32 = 216;
const SLIDER_CTRL_Y: i32 = 220;
const SLIDER_CAP_LEFT_X: i32 = 200;
const SLIDER_CAP_RIGHT_X: i32 = 324;
const SLIDER_TRACK_X: i32 = 224;
const SLIDER_TRACK_W: i32 = 100;
const SLIDER_KNOB_ORIGIN: i32 = 220;
/// The three track colours, which are the original's own literal arguments:
/// `FUN_00403A8F(…, 0x10)` for the top line, `FUN_0040437D(…, 0x3F)` for the
/// 100 × 4 fill, `FUN_00403A8F(…, 0x1F)` for the bottom. They used to be
/// [`Ink`](l2_view::Ink) values of ours.
const TRACK_TOP: u8 = 0x10;
const TRACK_FILL: u8 = 0x3F;
const TRACK_BOTTOM: u8 = 0x1F;

/// `(200, 0xDC, 0x18, 0x18)` — steps the split down by one.
pub fn split_down_button() -> Rect {
    Rect::new(SLIDER_CAP_LEFT_X, SLIDER_CTRL_Y, system::SLIDER_CAP, system::SLIDER_CAP)
}

/// `(0x145, 0xDC, 0x18, 0x18)` — up by one. 0x145 is 325, one pixel right of
/// where the cap is drawn; that off-by-one is the original's.
pub fn split_up_button() -> Rect {
    Rect::new(SLIDER_CAP_RIGHT_X + 1, SLIDER_CTRL_Y, system::SLIDER_CAP, system::SLIDER_CAP)
}

/// `(0xE0, 0xDC, 0x66, 0x18)` — a click here jumps the split to `mouseX - 224`.
pub fn split_track() -> Rect {
    Rect::new(SLIDER_TRACK_X, SLIDER_CTRL_Y, 102, system::SLIDER_CAP)
}

