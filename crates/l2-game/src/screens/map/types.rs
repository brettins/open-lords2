#![allow(unused_imports)]
use super::*;
use super::far_box::*;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    None,
    Sidebar(usize),
    EndTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IndustrySite {
    pub(crate) tile: usize,
    pub(crate) county: u8,
    pub(crate) commodity: usize,
    pub(crate) frame: u8,
}

/// `FUN_004B0CB4` has exactly two call sites in the whole binary and both are
/// on the turn boundary: `Turn_Tick`'s phase 7 with `rawFlag = 1` immediately
/// after `Season_Advance`, and `FUN_0049A3E6` with `0` after reloading the
/// seasonal art. So the sequence a player sees is **units walk, season
/// advances, screen fades down, art swaps in the dark, screen fades back up** —
/// which is the order he described from memory, and it is the order these two
/// states run in.
#[derive(Debug, Clone)]
pub(crate) struct Fading {
    pub(super) phase: u8,
    pub(super) status: String,
    pub(super) over: bool,
}

/// `Map_BeginMoveSelection` (`0x0043723A`) runs `Move_FloodFill` **once**, when
/// the army is picked, and leaves the result in `g_moveDistLocal`
/// (`0x00500C30`). Every frame after that, `Map_HoverUnitTarget` (`0x004A8E0B`)
/// only *descends* it — `Move_ExtractPath` — and only when the hovered tile
/// changed since the last frame (`if (DAT_005691E0 != g_hoverTileOffset)`). One
/// fill, one descent per new tile. Caching the field here is not an
/// optimisation of ours; it is the shape of the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MoveOrder {
    /// The unit the order is for — `g_selectedUnit` (`0x0057C8C4`).
    pub(super) unit: usize,
    pub(super) cost: l2_kingdom::map::CostMap,
    pub(crate) field: l2_kingdom::movement::DistanceField,
    /// `DAT_005691E0` — the tile the last descent was run for, so the pointer
    /// moving *within* a tile costs nothing.
    pub(crate) hovered: Option<(u8, u8)>,
    /// `g_pathBuf[localPlayer]` — the route `Path_MarkPreviewTiles`
    /// (`0x004A91BA`) marks and `Map_DrawPathMarker` (`0x004081A6`) draws.
    pub(crate) path: Vec<(u8, u8)>,
}

