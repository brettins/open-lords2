#![allow(unused_imports)]

mod render_helpers;
pub use render_helpers::*;
pub(crate) mod draw;
pub use draw::*;

use super::*;
use super::layout::*;
use super::draw::*;
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

// -------------------------------------------------------- drawing the strip
//
// A free function, not a method, because **the campaign map draws it too**.
// `Screen_DrawCampaign` puts `CountyStrip_Draw` in the sidebar of the map
// itself; until now our map screen drew a box of our own numbers over the jobs
// plate below it and left this plate empty, which is the thing a player looks
// at every turn.

/// `Ui_DrawDelta`'s `colourPos`, the eighth argument at all seven produce-row
/// call sites.
const DELTA_POS: u8 = 0xFA;

/// `Ui_DrawDelta`'s `colourNeg`, the ninth. It is the same index
/// [`font::HIGHLIGHT`](crate::shell::font::HIGHLIGHT) carries and they are kept
/// apart on purpose: that one is *"this is the thing you are looking at"* and
/// this one is *"this number is negative"*, and a rename of either must not
/// drag the other.
const DELTA_NEG: u8 = 0xF9;

/// The county's name: `L2.eng` group 100, index `scenarioIndex * 20 + countyId`
/// — and `g_scenarioIndex` *is* the map slot ([`crate::game::Game::map_slot`]).
///
/// Falls back to `COUNTY n` for an install with no `L2.eng`, which is also what
/// the tests run against.
pub fn county_name(ctx: &Ctx, id: u8) -> String {
    let index = ctx.game.map_slot * 20 + id as usize;
    let name = ctx.assets.shell.text(100, index);
    if name.is_empty() {
        format!("COUNTY {id}")
    } else {
        name.to_string()
    }
}

/// **One `L2.eng` string with a fallback — the panels' vocabulary,
/// strip's two.**
///
/// All four county panels hard-coded their words — `"RATION"`, `"WANTED:"`,
/// `"PEOPLE PAY"` — where the original draws `Eng_DrawString(group, index)`.
///
/// **The ration panel is wired through here. The other three are not, and this
/// is the list**, so that "recorded" does not become "left":
///
/// | panel | `g_screenId` | group | painter | state |
/// |---|---|---|---|---|
/// | ration | `0x19` | 87 | `Panel_Ration` (`0x00411B72`) | wired |
/// | tax | `0x15` | 86 | `Panel_Tax` (`0x0041152F`) | **hard-coded** |
/// | population | `0x14` | 73 | | **hard-coded** |
/// | happiness | `0x16` | 85 | | **hard-coded** |
///
/// The ration panel's own numbers say what the other three are likely to cost:
/// wiring it went from **12 of `Panel_Ration`'s 26 content draws to 18**, and
/// six of the six added were *labels* — the frames that turn three unlabelled
/// numbers into a Fed row. `CLAUDE.md` rule 6.
///
/// The honest account of how that happened, because it is a habit and not an
/// oversight: **we read these panels' numbers out of the binary and wrote their
/// words ourselves**, treating the numbers as the mechanism and the text as a
/// skin over it. `Panel_Ration` is the *only* consumer of group 87 in the whole
/// binary and draws seven of its twelve strings, so the group **is** the
/// panel's specification. A screen's strings are part of what it does.
///
/// No case handling here: [`l2_view::text::glyph`] upper-cases, so a lower-case
/// string out of `L2.eng` draws the same as our shouted fallback.
pub(super) fn eng(ctx: &Ctx, group: usize, index: usize, fallback: &str) -> String {
    let s = ctx.assets.shell.text(group, index);
    if s.is_empty() {
        fallback.to_string()
    } else {
        s.to_string()
    }
}

/// The ration level's name — `L2.eng` group 21, which is what `CountyStrip_Draw`
/// indexes with county `+0x15D`.
pub(super) fn ration_label(ctx: &Ctx, level: i32) -> String {
    let level = level.clamp(0, RATION_LEVEL_COUNT as i32 - 1);
    eng(ctx, GROUP_RATION_LEVELS, level as usize, ration_name(level))
}

/// The fallback words for the three flat industry rows, in commodity order —
/// ours, and reached only by an install with no `Misc_cty.pl8`.
const INDUSTRY_LABEL: [&str; 4] = ["WOOD", "IRON", "WEAPONS", "STONE"];

