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
    /// One of the five `g_sidebarButtons`, by index.
    Sidebar(usize),
    EndTurn,
}

/// **One industry's building on the campaign map,
///
/// The mine, the quarry, the forest
/// the town is, and this is the wheel `Sprite_TopIt` turns. See
/// [`MapScreen::step_industry`] for the rate and
/// [`l2_view::campaign::INDUSTRY_FRAMES`] for the four runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IndustrySite {
    /// Tile index, `y * 64 + x`. Fixed for the life of the map.
    pub(crate) tile: usize,
    pub(crate) county: u8,
    /// [`l2_kingdom::tables::Commodity::index`] — wood 0, iron 1, weapons 2,
    /// stone 3.
    pub(crate) commodity: usize,
    /// The frame the tile is drawn with right now.
    pub(crate) frame: u8,
}

/// **The end-of-turn screen fade, mid-flight.**
///
/// `FUN_004B0CB4` has exactly two call sites in the whole binary and both are
/// on the turn boundary: `Turn_Tick`'s phase 7 with `rawFlag = 1` immediately
/// after `Season_Advance`, and `FUN_0049A3E6` with `0` after reloading the
/// seasonal art. So the sequence a player sees is **units walk, season
/// advances, screen fades down, art swaps in the dark, screen fades back up** —
/// which is the order he described from memory, and it is the order these two
/// states run in.
///
/// The phase counts `0 ..= l2_view::fade::PHASES`, one per fixed tick; the
/// palette arithmetic
#[derive(Debug, Clone)]
pub(crate) struct Fading {
    pub(super) phase: u8,
/// Held
    /// written immediately because the numbers it names change *during* the
    /// dark, and announcing them early is the abruptness the fade hides.
    pub(super) status: String,
    /// Whether the turn ended the game, in which case the conquest screen comes
    /// up when the light does — as it does in the original, where
    /// `g_screenId = 0x1C` is set on the far side of the same fade.
    pub(super) over: bool,
}

/// **Move-order mode's whole state** — `g_screenId == 0x10`.
///
/// `Map_BeginMoveSelection` (`0x0043723A`) runs `Move_FloodFill` **once**, when
/// the army is picked, and leaves the result in `g_moveDistLocal`
/// (`0x00500C30`). Every frame after that, `Map_HoverUnitTarget` (`0x004A8E0B`)
/// only *descends* it — `Move_ExtractPath` — and only when the hovered tile
/// changed since the last frame (`if (DAT_005691E0 != g_hoverTileOffset)`). One
/// fill, one descent per new tile. Caching the field here is not an
/// optimisation of ours; it is the shape of the original.
///
/// This is **display state**. Nothing in it reaches the simulation: an order is
/// placed by [`MapScreen::order_march`] calling into `l2-kingdom`, and what is
/// stored here is only what the player is being shown he *would* get. That
/// matters for `docs/netcode.md` — the lockstep digest must not depend on where
/// anyone's mouse is, and a pointer position is the least deterministic input
/// there is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MoveOrder {
    /// The unit the order is for — `g_selectedUnit` (`0x0057C8C4`).
    pub(super) unit: usize,
    /// The terrain costs the fill was run over.
    pub(super) cost: l2_kingdom::map::CostMap,
    /// `g_moveDistLocal`, filled from the unit's own tile.
    pub(crate) field: l2_kingdom::movement::DistanceField,
    /// `DAT_005691E0` — the tile the last descent was run for, so the pointer
    /// moving *within* a tile costs nothing.
    pub(crate) hovered: Option<(u8, u8)>,
    /// `g_pathBuf[localPlayer]` — the route `Path_MarkPreviewTiles`
    /// (`0x004A91BA`) marks and `Map_DrawPathMarker` (`0x004081A6`) draws.
    /// Empty when the hovered tile is unreachable, which is
    /// `g_moveOrderAvailable = 0`.
    pub(crate) path: Vec<(u8, u8)>,
}

