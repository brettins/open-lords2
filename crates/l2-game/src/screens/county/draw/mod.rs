#![allow(unused_imports)]

mod panel;
pub use panel::*;
mod screen;
pub use screen::*;
mod helpers;
pub use helpers::*;

use super::*;
use super::layout::*;
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

/// **`Panel_Ration`'s number arguments, which are not the same as everybody
/// else's.** Every `Ui_DrawNumberRight` and `Ui_DrawNumber` call in this
/// painter passes `' '` as the lead and a pointer into the run of zero bytes at
/// `0x004D3E00` … `0x004D3E1B` as the suffix, so **the suffix is the empty
/// string** — six call sites, six distinct addresses, every one of them a NUL.
///
/// It matters because the suffix is inside what gets **measured**: the centring
/// tail `FUN_004025D7` is `x + max(0, (width − FUN_004014F0(buffer)) / 2)` and
/// `FUN_004014F0` charges four pixels for a space at any position. A trailing
/// space we invent therefore moves the digits two pixels left, on all five
/// columns at once. `docs/decisions.md` C140. **[V]**
const RATION_LEAD: char = ' ';
const FRAME_FACE: usize = 0x17;
const FRAME_FED_MARK: usize = 0x18;
const FRAME_GRAIN: usize = 0x21;
const FRAME_CATTLE: usize = 0x26;
/// `Panel_Ration`: the third food column's icon, 23 × 19. `Misc_cty` frame
/// `0x2A` has not been decoded to a picture here; `docs/screens-county.md` §5.4
/// calls it sheep, which is **[D]** and not checked.
const FRAME_THIRD_FOOD: usize = 0x2A;
const FRAME_TAX_VIGNETTE: usize = 0x3E;
const FACE_W: i32 = 0x14;


