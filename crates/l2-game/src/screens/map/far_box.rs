#![allow(unused_imports)]
use super::*;
use super::types::*;
use sidebar::*;
use helpers::*;
use l2_kingdom::field::FieldType;
use l2_kingdom::industry;
use l2_formats::maps::Plane;
use l2_view::campaign::{self, Dir, Lattice, Viewport, Zoom, FAR, NEAR, PANEL_W, PANEL_X};
use l2_view::chrome::{self, Minimap, MinimapMode, MinimapTint};
use l2_view::village;
use l2_view::{text, Canvas, Clip, Ink, Tags};
use crate::input::{Event, Key, Rect};
use crate::press::Press;
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::battlefield::{
    BOX_SET, CONFIRM_BOX, CONFIRM_COLS, CONFIRM_NO_FRAME, CONFIRM_ROWS, CONFIRM_WIDGETS,
    CONFIRM_YES_FRAME, GROUP_CONFIRM,
};
use crate::screens::county;
use crate::screens::menubar;
use crate::screens::saveload::Mode as SaveLoadMode;
use crate::turn;
use crate::shell::{font, Pen};
use crate::widget;
use paint::*;

/// `L2.eng` group 101 — the sixty map names `g_scenarioIndex` indexes, the same
/// group `ScenarioList_Draw` and `Screen_DrawConquest` read.
pub(super) const FAR_BOX_MAP_GROUP: usize = 101;
/// Group 34: index 0 *"Year"*, index 1 *"Click on the county you wish to view."*
/// `Screen_DrawCampaign` is its only consumer — so it is this box's vocabulary,
/// not a naming lead. `CLAUDE.md` rule 6.
const FAR_BOX_YEAR_GROUP: usize = 34;
pub(super) const FAR_BOX_YEAR_LABEL: usize = 0;
pub(super) const FAR_BOX_ADVICE: usize = 1;
pub(crate) const FAR_BOX_Y: i32 = 0x1A8;
pub(crate) const FAR_BOX_NAME_X: i32 = 0x40;
pub(crate) const FAR_BOX_YEAR_LABEL_X: i32 = 0x50;
pub(super) const FAR_BOX_YEAR_X: i32 = 0x60;
pub(super) const FAR_BOX_ADVICE_X: i32 = 0x50;
pub(crate) const FAR_BOX_ADVICE_Y: i32 = 0x1C6;

/// One of group 34's two strings, **as the player's own file spells them**, with
/// our transcription for an install that has no `L2.eng`. `CLAUDE.md` rule 6:
/// the fallback is the fallback, not the source.
pub(crate) fn far_box_text(ctx: &Ctx, index: usize) -> String {
    let s = ctx.assets.shell.text(FAR_BOX_YEAR_GROUP, index);
    if !s.is_empty() {
        return s.to_string();
    }
    match index {
        FAR_BOX_YEAR_LABEL => "Year".to_string(),
        _ => "Click on the county you wish to view.".to_string(),
    }
}


