//! The campaign map is `g_screenId == 0` **and** `g_screenId == 0x10`, and
//! forgetting the second one cost two player-reported defects in a single
//! evening. `Screen_FrameInput` (`0x0042FF10`) dispatches on the id, so while an
//! army is picked *none of the arms below the first heading run at all*.
//!
//! | arm | the original |
//! |---|---|
//! | edge of the screen scrolls | `Map_EdgeScroll` `0x00432221` |
//! | left release on a tile | `Map_Click` `0x0043CE1A` |
//! | right release on a tile | `g_screenId = 4; FUN_0043CAF4` (`0x0043CAF4`) |
//! | minimap raster | `Minimap_Click` `0x0043253A` |
//! | the four mode icons | `Minimap_ModeButtonClicked` `0x0043292D` |
//! | the six sidebar buttons | `Sidebar_ButtonClicked` `0x00432967` |
//! | the strip's 2 × 2 | `CountyStrip_Click` `0x00438CEB` |
//! | the farm/industry slider | `Labour_SplitSliderDrag` `0x00439122` |
//! | the menu bar's three titles | `Menu_OpenDropdown` `0x0040DECA` — [`crate::screens::menubar`] |
//! | right release clears the minimap mode | `FUN_00439079` `0x00439079` |
//! | the sidebar's job rows | `CountyStrip_JobClick` `0x00438E3B` |
//! | **not reproduced:** a left press at zoom 2 zooms in on the tile | `Map_ZoomInAtTile` |
//! | **not reproduced:** a right release dismisses the message scroll | `Msg_Dismiss` |
//!
//! *(The last three rows used to read* **not reproduced** *for the menu bar,
//! `FUN_00439079` and the job rows.
//!
//! 1. your army, not besieging → move-order mode (`Panel_MoveButton` `0x004371CE`)
//! 2. your army, besieging → the siege screen `0x1D`
//! 3. a merchant in your county → the merchant `0x08`; elsewhere, `Msg_Enqueue` 0x70
//! 4. flag `0x80`, your county → select, recentre, `Industry_ToggleFromMap` `0x0043D309`
//! 5. flag `0x40`, your county → select, recentre, the village `0x02`
//! 6. flag `0x20`, your county → the field brush `0x04`
//!
//! | arm | the original |
//! |---|---|
//! | entering it | `Map_BeginMoveSelection` `0x0043723A` — one `Move_FloodFill`, and that is all |
//! | the route under the cursor, every frame | `Map_HoverUnitTarget` `0x004A8E0B`, called from `Screen_DrawWidgets` `0x004BA26E` |
//! | marking and drawing it | `Path_MarkPreviewTiles` `0x004A91BA`, `Map_DrawPathMarker` `0x004081A6` |
//! | left press → leave the mode, then order | `Map_ConfirmMoveOrder` `0x004A9252` |
//! | right release → leave the mode | inline, `g_screenId = 0` |
//! | edge scroll | `Map_EdgeScroll`, again |
//! | **absent by model, not omission:** 40 dead frames | `g_moveOrderClickGuard` `0x00553ECC` |
//!
//! **`Map_Click` has no county-selection arm.** It ends in the ladder above;
//! the tail is `else { DAT_0056D64C = 0; }`, a scroll latch. Its three writes to
//! `g_selectedCounty` are all inside branches 3, 4 and 5 — **selecting a county
//! is a side effect of arriving somewhere, never a verb of its own.** A click on
//! plain ground, on sea, on a foreign county, or on your own county away from
//! its town, fields and buildings changes nothing at all.
//!
//! We had an arm here, and a second click on the selected county opened its tax
//! panel — one click fewer than reaching for the strip. Both are gone
//! (`docs/decisions.md` C61). The history is worth keeping, because **that arm
//! was the multiplier under three separate defects that reached a player in one
//! evening**: a hit test smaller than the thing drawn (C58, a 9 × 9 box under a
//! 40 × 32 figure); one that stopped at the diamond while the sprite stood over
//! the tiles behind it (C57, the mine); and one whose arithmetic was
//! wrong (C60, `pick_tile` dividing by `tile_w / 2` where `Map_PickTile` divides
//! by the half pitch — 56 dead pixels around every tile centre). Three different
//! mistakes. **All three became *the wrong screen opening*
//! happening**, because a miss had somewhere to fall through to —
//! reported the arm itself in the end: *"if you click anywhere on grass it opens
//! up the tax window too."*
//!
//! `docs/decisions.md` C132.
//!
//! `docs/decisions.md` C138.

mod types;
pub use types::*;
mod far_box;
pub use far_box::*;

mod sidebar;
pub use sidebar::*;
mod helpers;
pub use helpers::*;

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
use crate::screens::confirm;
use crate::screens::menubar;
use crate::screens::saveload::Mode as SaveLoadMode;
use crate::turn;
use crate::shell::{font, Pen};
use crate::widget;

pub const TOP_BAR: i32 = campaign::TOP_BAR_H;

pub const CANVAS_W: i32 = l2_view::canvas::WIDTH as i32;
pub const CANVAS_H: i32 = l2_view::canvas::HEIGHT as i32;

pub const PANEL: Rect = Rect::new(PANEL_X, TOP_BAR, PANEL_W, 480 - TOP_BAR);

/// The End Turn strip — `Misc_cty` frame 59 at (478, 460), with `L2.eng` group
/// 4 centred in it.
pub const END_TURN_BUTTON: Rect =
    Rect::new(PANEL_X, chrome::PANEL_END_TURN_Y, PANEL_W - 1, 19);

/// **`g_sidebarButtons` (`0x004DC680`) — the five buttons in `Misc_cty` frame
/// 57, the 162 × 30 strip at (478, 430).** `docs/screens-county.md` §2.4.
///
/// The table's five records start at x offsets 0, 34, 66, 98 and 130 and end at
/// 33, 65, 97, 129 and 161. `Hotspot_Test` (`0x0040E3EE`) is **half-open** —
/// `x0 + off <= mx < x1 + off` — so the widths are 33, 31, 31, 31, 31 and there
/// is a one-pixel dead column between each pair. That gap is the original's.
///
/// All five destinations are read: `Sidebar_Button` (`0x0043AE30`) is a
/// five-way `if` on the hotspot id, and every arm sets a `g_screenId` we
/// already have a [`shell`](crate::screens::shells) for.
///
/// more: `Sidebar_ButtonClicked` (`0x00432967`) is one
/// `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` call and nothing else.
///
// arm: 0x00432967/sidebar-hit-test left-press
pub const SIDEBAR_BUTTONS: [SidebarButton; 5] = [
    // `Levy_SetPercent(sel, g_levyPercent); FUN_004AA90A(sel, g_levyMen);
    // g_screenId = 0x17`.
    //
    // the county has an offer. `L2.eng` group 69 index 0x10 is "Raising an
    // army in", so 0x17 is the **raise-army** screen
    // an optional half of it. The shell table called the whole screen "Hire
    // mercenaries" — the smaller half naming the larger — and this button and
    // that name were corrected in the same afternoon by two agents who had not
    // spoken. `docs/decisions.md` C45; the screen is `crate::screens::army` and
// this goes through
    // [`sidebar_destination`].
    SidebarButton { x: 0, w: 33, action: SidebarAction::Screen(0x17), name: "ARMY" },
    SidebarButton { x: 34, w: 31, action: SidebarAction::Screen(0x09), name: "COURT" },
    SidebarButton { x: 66, w: 31, action: SidebarAction::Screen(0x18), name: "SUPPLY" },
    // `FUN_00436A88`: `g_screenId = 0x1B`, castle building.
    SidebarButton { x: 98, w: 31, action: SidebarAction::Screen(0x1B), name: "CASTLE" },
    // `FUN_0043611B`: `g_screenId = 0x0B`, the other lords.
    SidebarButton { x: 130, w: 31, action: SidebarAction::Screen(0x0B), name: "LORDS" },
];

pub const MINIMAP_MODE_BUTTONS: [Rect; 4] = [
    Rect::new(610, 32, 27, 31),
    Rect::new(610, 64, 27, 34),
    Rect::new(610, 96, 27, 31),
    Rect::new(610, 128, 27, 31),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarButton {
    pub x: i32,
    pub w: i32,
    pub action: SidebarAction,
    pub name: &'static str,
}

pub const SIDEBAR_H: i32 = 29;

/// The table stores a `g_screenId` because that is what `Sidebar_Button`
/// writes. **All five now resolve to a screen** — the shell table is empty —
/// and two need the county, because in the original the whole strip is *about*
/// `g_selectedCounty`: `Sidebar_Button`'s own arm is
/// `Levy_SetPercent(g_selectedCounty, g_levyPercent); FUN_004AA90A(g_selectedCounty, g_levyMen)`.
pub fn sidebar_destination(id: u8, county: u8) -> ScreenId {
    match id {
        0x17 => ScreenId::RaiseArmy(county),
        // `Castle_OpenScreen` (`0x00436A88`) is the same shape: it refuses a
        // county that is not the local player's with message 0x70 — the gate
        // already in `handle` — and otherwise sets `g_screenId = 0x1B` for
        // `g_selectedCounty`.
        0x1B => ScreenId::Castle(county),
        0x09 => ScreenId::Court,
        0x18 => ScreenId::Supplies(county),
        // `FUN_0043611B` — the LORDS button — is a bare `g_screenId = 0x0B`
        // with no county in it at all, because the diplomacy screen is about
// realms. `Diplo_DrawScreen` picks its own target
        // through `Diplo_DefaultTarget`.
        0x0B => ScreenId::Diplomacy,
        _ => ScreenId::Campaign,
    }
}

/// **`FUN_00439122` — the farm/industry labour split slider**, on the 162 × 52
/// plate at (478, 250) that `CountyStrip_Draw` paints. Its hit rectangle is the
/// whole width of the sidebar, `x 478 … 639, y 257 … 296`.
///
/// The same player then reported that it was not draggable, and it is. The
/// question was whether it uses the village's three-screen gesture — press,
/// nine pixels, release, second press (`docs/screens-county.md` §6.4.1) — or
/// simple press-and-track, and `FUN_00439122`'s guard settles it outright:
///
/// ```c
/// if (g_mouseLeftReleased == 0) {          // DAT_004E65D8: the up edge
///     if (g_mouseLeftDown == 0)      return 0;   // DAT_004E65CC: the level
///     else if (g_mouseMoved == 0)    return 0;   // DAT_004EA4B0
///     else                           ...set the value...
/// } else return 1;                          // a release is eaten, not acted on
/// ```
///
/// Those three globals are named by the frame poll at `0x004B2D5A`, which
/// derives them from the window procedure's `WM_LBUTTONDOWN` / `WM_LBUTTONUP`:
///
/// `DAT_004E65CC` is the button's **level**, `DAT_004EAFB4` and `DAT_004E65D8`
/// its two edges, and `DAT_004EA4B0` is set whenever the pointer moved or a
/// button changed this frame. So the slider runs on *held **and** moved*, every
/// frame, and does nothing at all on the release. **Press and track** — no
/// second click, no dead zone, and no extra screen ids: `g_screenId` is
/// untouched by the whole function.
pub const SPLIT_SLIDER: Rect = Rect::new(PANEL_X, 257, PANEL_W, 296 - 257 + 1);

pub const MAP_AREA: Rect = Rect::new(0, TOP_BAR, PANEL_X, NEAR.bottom() - TOP_BAR);

const MARKER: i32 = 2;

pub struct MapScreen {
    zoom: Zoom,
    view: Viewport,
    saved: Viewport,
    base: Canvas,
    tags: Tags,
    built: Option<(usize, u8, Viewport, u32, u8, u64, u64, u64, u64)>,
    industry_sites: Vec<IndustrySite>,
    industry_slot: Option<usize>,
    industry_tick: u32,
    /// Milliseconds since the last gate, zeroed on each — `Tick_Pulses`
    /// (`0x004BBC80`) sets `stamp = now`. See [`l2_view::village::GATE_MS`].
    industry_gate_ms: u32,
    minimap: Option<Minimap>,
    minimap_slot: Option<usize>,
    focus: Focus,
    status: String,
    pointer: (i32, i32),
    pointer_in: bool,
    scrolled: bool,
    opened: bool,
    /// `DAT_0057D378`, the map's animation tick, and `DAT_0057D390`, the flag's
    /// wave phase, which is that counter mod `0x80` shifted right by four.
    ///
    /// `FUN_004CFB08` advances the counter once per **16 ms** of `GetTickCount`
    /// and then draws a frame,
    /// 16 × 16 ms
    /// (`main::TICK`) and nothing below this crate reads a clock, so the counter
    /// is stepped by [`Screen::update`]
    flag_tick: u8,
    flag_phase: u8,
    /// `DAT_0057D388`, the map's **second** animation counter, and
    /// `_DAT_0057D38C`, the herd's grazing phase, which is that counter mod
/// **six** phases, not eight.
    ///
    /// `FUN_004CFB08` steps both counters behind one 16 ms `GetTickCount` gate,
    /// so a herd holds each frame for 16 × 16 ms
    /// seconds. **This is not `Tick_Pulses`** (`0x004BBC80`), the 20 ms
    /// `timeGetTime` divider chain the village animates off — the campaign map
    /// has its own clock.
    herd_tick: u8,
    herd_phase: u8,
    /// `Map_Click`'s army branch is three lines: a picked unit of type 1 that is
    /// the local player's either opens the siege screen (`+0x199` set, after
    /// `Siege_ValidateLink` has had a chance to clear it) or goes to
    /// `Panel_MoveButton`, which is `g_screenId = 0x10` — the campaign map
    /// *in move-order mode* — with `g_selectedUnit` set and a flood fill run
    /// from the unit. The next click on the map is
    /// `Map_ConfirmMoveOrder`, which places the order.
    selected_unit: Option<usize>,
    move_order: Option<MoveOrder>,
    /// **The farm/industry slider is held.** `FUN_00439122` acts on the button
    /// being *down*, not on it having been clicked, so the value tracks the
    /// pointer for as long as it is held — see [`SPLIT_SLIDER`].
    slider_held: bool,
    /// **`g_minimapMode` (`0x0057A0C4`)** — what the minimap is coloured by.
    minimap_mode: MinimapMode,
    fading: Option<Fading>,
    /// **A `Save_RotateAndWrite` is owed** — `FUN_0049A3E6`'s last statement,
    /// raised at the bottom of the fade and drained by the machine. See
    /// [`crate::screen::Screen::take_autosave`].
    autosave: bool,
    gold_at_turn_start: i32,
    /// **`g_optScrollSpeed` (`0x0053F234`).** 0 … 100 in steps of ten, shown on
    /// the options slider as 0 … 10; the shipped default is
    /// [`DEFAULT_SCROLL_SPEED`].
    ///
    /// `Menu_ScrollSpeed` (`0x00434CEE`) is the control that sets it, the
    /// options menu is not drawn yet, and this is the seam it will attach to.
    scroll_speed: i32,
    scroll_wait: u32,
    press: Press,
}

/// `Ui_OpenConfirm(5, …)` — `L2.eng` group 10 index 5, *"Combine armies?"*,
/// which `Map_ConfirmMoveOrder` (`0x004A9252`) raises off `g_hoverMergeUnit`.
pub const COMBINE_PROMPT: usize = 5;

/// **Rule 6**: the group's own words, with our transcription only when the
/// install's `L2.eng` has nothing at that index.
fn ours_or(from_eng: &str, ours: &str) -> String {
    if from_eng.is_empty() {
        ours.into()
    } else {
        from_eng.to_uppercase()
    }
}

mod camera;
mod events;
mod paint;
mod panel;
mod terrain;
mod units;

#[cfg(test)]
mod tests;

pub use paint::*;

impl Default for MapScreen {
    fn default() -> Self {
        MapScreen::new()
    }
}

