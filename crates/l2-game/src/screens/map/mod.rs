//! The campaign screen: the original's layout, at the original's two zooms.
//!
//! `docs/screens.md` is what this is built from. The layout it establishes:
//!
//! ```text
//!  0                                            478        640
//!  +--------------------------------------------+----------+  0
//!  |  menu bar, 24 px                            banners    |
//!  +--------------------------------------------+----------+  24
//!  |  map viewport                              || minimap  |
//!  |  x [0, 478)  y [24, 474) near               | 128x128  |
//!  |              y [24, 408) far               ||  ...     |
//!  |                                            || End turn |
//!  +--------------------------------------------+----------+  480
//! ```
//!
//! Every one of those numbers is decompiled, not designed: `Map_SetZoom`'s
//! `pitch * cols + viewX` comes out 480 at both zooms, the `Misc_cty.pl8`
//! right-column frames are 162 wide and tile `y` 24 … 480 with no gap, and
//! `Map_DrawPathMarker` clips the map to `x < 478`, `y < 474`.
//!
//! # The screen is two screens, and every arm of both is named here
//!
//! The campaign map is `g_screenId == 0` **and** `g_screenId == 0x10`, and
//! forgetting the second one cost two player-reported defects in a single
//! evening. `Screen_FrameInput` (`0x0042FF10`) dispatches on the id, so while an
//! army is picked *none of the arms below the first heading run at all*.
//!
//! Rule 5 in `CLAUDE.md` says a behaviour must name the function it reproduces.
//! This module's arms. A behaviour must name the function it reproduces.
//! rediscover it:
//!
//! ## Screen `0` — the map
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
//! arms** between them — the menu bar alone is sixteen items over three
//! drop-downs — which is what a one-line table row can hide.
//! `docs/arms.json` groups `menu-bar` and `right-column`.)*
//!
//! `Map_Click` itself is six branches, in this order.
//! — **it tests the picked *unit* before any tile flag**, so an army standing on
//! your own farmland is an army and not a field:
//!
//! 1. your army, not besieging → move-order mode (`Panel_MoveButton` `0x004371CE`)
//! 2. your army, besieging → the siege screen `0x1D`
//! 3. a merchant in your county → the merchant `0x08`; elsewhere, `Msg_Enqueue` 0x70
//! 4. flag `0x80`, your county → select, recentre, `Industry_ToggleFromMap` `0x0043D309`
//! 5. flag `0x40`, your county → select, recentre, the village `0x02`
//! 6. flag `0x20`, your county → the field brush `0x04`
//!
//! ## Screen `0x10` — the map, taking an order
//!
//! Four clauses in `Screen_FrameInput`, plus one per-frame call that lives, of
//! all places, in the *draw* dispatcher:
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
//! # A miss does nothing, and it took four bugs to get here
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
//! So the hit tests are still worth getting right, and now a mistake in one is
//! quiet. `a_click_on_plain_ground_changes_nothing_at_all` is
//! the assertion that could not exist while the arm did; it covers the whole
//! kingdom, not the screen id, because *nothing happened* is the claim.
//!
//! # What is the original's, and what is ours
//!
//! **The original's:** the viewport and both zooms,
//! eight scroll directions, the `−4 / −12` centring, the tile artwork, the
//! menu-bar background tiled from `Panels.pl8`, the `Misc_cty.pl8` right
//! column, the `MAPnn.PL8` minimap and its realm colour ramp out of
//! `Lords2.exe`,
//!
//! **Ours, and it should look it:** every word of text (our 5 × 7 font, not the
//! game's `Fntl2_*.pl8`), the county marker squares, the keys that scroll, and
//! the layout of the numbers inside the right panel's frames. The original also
//! *measures* the selected county — `Map_DrawFrame` tallies how much of the
//! viewport each county fills and takes the maximum — and we do not; here the
//! selection only changes when the player clicks.
//!
//! **The yellow outline round the selected county was ours and is gone.** It
//! used to be on that list, and being on the list is what let it stay: the
//! county-selection arm and the outline were one invention with two halves.
//! arm was removed
//! four merges later — *"still a weird yellow outline around the county that is
//! selected on the map."* This was recorded in the input inventory.
//! **The removal was recorded in the input inventory.
//! was in the painter, where the input inventory does not look.**
//! `docs/decisions.md` C132.
//!
//! `the_selection_is_not_drawn_on_the_map` compared the first two counties the
//! player does not own — 1 and 2 —
//! and 9. Putting the outline back turned nothing red. It sweeps the counties
//! the pick plane says are *on screen* now.
//! set is non-empty is the part that stops it happening again.
//! `docs/decisions.md` C138.
//!
//! # Picking
//!
//! A click on the map reads the tag plane, which is exact by construction: it
//! answers with the county whose tile is visible at that pixel. The
//! original inverts the projection instead (`Map_PickTile`), which is a
//! different algorithm reaching the same answer for the same reason.
//!
//! A click on the minimap goes through `MAPnn.PL8`'s own county raster, which
//! *is* what the original does, and then centres the map on that county.

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
use crate::screens::menubar;
use crate::screens::saveload::Mode as SaveLoadMode;
use crate::turn;
use crate::shell::{font, Pen};
use crate::widget;

/// The menu bar: `Screen_DrawMenuBar`'s 640 × 24 strip at y 0.
pub const TOP_BAR: i32 = campaign::TOP_BAR_H;

/// The screen the original ran at,
pub const CANVAS_W: i32 = l2_view::canvas::WIDTH as i32;
pub const CANVAS_H: i32 = l2_view::canvas::HEIGHT as i32;

/// The right column: `Misc_cty` frames 54 … 59 at x 478, 162 wide.
pub const PANEL: Rect = Rect::new(PANEL_X, TOP_BAR, PANEL_W, 480 - TOP_BAR);

/// The End Turn strip — `Misc_cty` frame 59 at (478, 460), with `L2.eng` group
/// 4 centred in it.
///
/// **The hit box is 161 × 19 and not 162 × 20**, and that is `g_sidebarButtons`
/// record 5: `(0, 30) … (161, 49)` at the table's
/// (`0x1DE`, `0x1AE`) offset, with `Hotspot_Test` half-open on **both** axes —
/// `my < y0 + off || y1 + off <= my` rejects. So the strip's own last column
/// (x 639) and last row (y 479) are dead in the original, exactly like the
/// one-pixel dead columns between the five icons above it, and ours had them
/// live. Read out of the player's `Lords2.exe` in
/// `crates/l2-game/tests/right_column/main.rs`; the artwork is 162 × 20
/// hotspot is not.
pub const END_TURN_BUTTON: Rect =
    Rect::new(PANEL_X, chrome::PANEL_END_TURN_Y, PANEL_W - 1, 19);

/// **`g_sidebarButtons` (`0x004DC680`) — the five buttons in `Misc_cty` frame
/// 57, the 162 × 30 strip at (478, 430).** `docs/screens-county.md` §2.4.
///
/// This strip used to be one button of ours labelled COUNTY PANEL, with a
/// status line written across it. Both were drawn straight over the original's
/// own five icons, which is what the player was looking at when he reported
/// that the icons in the bottom right did nothing and had text over them: they
/// were live artwork under a dead rectangle.
///
/// The table's five records start at x offsets 0, 34, 66, 98 and 130 and end at
/// 33, 65, 97, 129 and 161. `Hotspot_Test` (`0x0040E3EE`) is **half-open** —
/// `x0 + off <= mx < x1 + off` — so the widths are 33, 31, 31, 31, 31 and there
/// is a one-pixel dead column between each pair. That gap is the original's.
///
/// All five destinations are read: `Sidebar_Button` (`0x0043AE30`) is a
/// five-way `if` on the hotspot id, and every arm sets a `g_screenId` we
/// already have a [`shell`](crate::screens::shells) for.
/// **The table itself is the arm**.
/// more: `Sidebar_ButtonClicked` (`0x00432967`) is one
/// `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` call and nothing else.
/// The sixth record is **End Turn**, `(0, 30) … (161, 49)` at that offset, whose
/// handler is `Turn_End`; it is [`END_TURN_BUTTON`]
/// here and its own arm.
// arm: 0x00432967/sidebar-hit-test left-press
pub const SIDEBAR_BUTTONS: [SidebarButton; 5] = [
    // `Levy_SetPercent(sel, g_levyPercent); FUN_004AA90A(sel, g_levyMen);
    // g_screenId = 0x17`.
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

/// One entry of that table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarButton {
    /// Offset from [`PANEL_X`], as the table stores it.
    pub x: i32,
    pub w: i32,
    pub action: SidebarAction,
    /// **Ours**, and only used for the hover caption: the original's buttons
    /// are pictures and carry no words at all.
    pub name: &'static str,
}

/// **29, not 30**.
///
/// Every one of the five records is `(x, 0) … (x, 29)` at the table's `0x1AE`
/// offset and `Hotspot_Test` is half-open, so the strip is y 430 … 458 and
/// **y 459 belongs to nothing** — the same one-pixel gutter the table leaves
/// between each pair of icons horizontally, once, horizontally across the whole
/// strip. This used to be `PANEL_END_TURN_Y − PANEL_STATUS_Y`, which is the
/// distance between two *plates* and not the height of a *hotspot*, and it made
/// the dead row live. See `crates/l2-game/tests/right_column/main.rs`, which reads
/// the table out of the player's own copy.
pub const SIDEBAR_H: i32 = 29;

/// **Which of our screens a sidebar button opens.**
///
/// The table stores a `g_screenId` because that is what `Sidebar_Button`
/// writes. **All five now resolve to a screen** — the shell table is empty —
/// and two need the county, because in the original the whole strip is *about*
/// `g_selectedCounty`: `Sidebar_Button`'s own arm is
/// `Levy_SetPercent(g_selectedCounty, g_levyPercent); FUN_004AA90A(g_selectedCounty, g_levyMen)`.
///
/// **This is the function to change when a screen graduates**.
/// below makes forgetting it a failure:
/// button: every id in the table must resolve to a screen this function names.
///
/// The five ids are `Sidebar_Button`'s five hotspots — 1 the levy, 2 the court,
/// 3 send supplies, 4 the castle, 5 diplomacy — and **only 1 and 3 are gated**
/// on the county belonging to the local player. 2 and 5 have no test at all,
/// which `screens/index.rs` used to say the opposite of.
pub fn sidebar_destination(id: u8, county: u8) -> ScreenId {
    match id {
        0x17 => ScreenId::RaiseArmy(county),
        // `Castle_OpenScreen` (`0x00436A88`) is the same shape: it refuses a
        // county that is not the local player's with message 0x70 — the gate
        // already in `handle` — and otherwise sets `g_screenId = 0x1B` for
        // `g_selectedCounty`.
        0x1B => ScreenId::Castle(county),
        // Hotspot 2, `Sidebar_Button`'s own arm, **ungated**.
        0x09 => ScreenId::Court,
        // Hotspot 3. The destination opens equal to the source
        // is the only thing that moves it.
        0x18 => ScreenId::Supplies(county),
        // `FUN_0043611B` — the LORDS button — is a bare `g_screenId = 0x0B`
        // with no county in it at all, because the diplomacy screen is about
// realms. `Diplo_DrawScreen` picks its own target
        // through `Diplo_DefaultTarget`.
        // It does not touch `g_diploTarget`; the painter's prologue heals a
        // stale one.
        0x0B => ScreenId::Diplomacy,
        //
// The sixth hotspot is End Turn; the fall-through is unreachable from
        // the strip. It went to a shell until the table was emptied; the map
        // is the honest destination for an id nothing claims.
        _ => ScreenId::Campaign,
    }
}

/// **`FUN_00439122` — the farm/industry labour split slider**, on the 162 × 52
/// plate at (478, 250) that `CountyStrip_Draw` paints. Its hit rectangle is the
/// whole width of the sidebar, `x 478 … 639, y 257 … 296`.
///
/// This is the loaf-and-hammer bar with an arrow between them, and it is the
/// one control on the campaign screen that moves peasants in bulk: it sets
/// [`County::industry_share`](l2_kingdom::county::County::industry_share), the
/// percentage of the county's people that goes to the mines
/// A player reported that he
/// could not assign peasants.
///
/// # It is a **drag**.
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
/// `DAT_004E65CC` is the button's **level**, `DAT_004EAFB4` and `DAT_004E65D8`
/// its two edges, and `DAT_004EA4B0` is set whenever the pointer moved or a
/// button changed this frame. So the slider runs on *held **and** moved*, every
/// frame, and does nothing at all on the release. **Press and track** — no
/// second click, no dead zone, and no extra screen ids: `g_screenId` is
/// untouched by the whole function.
///
/// It is tested on the campaign map *and* on the village, in that order — the
/// two arms in `Screen_FrameInput` — so it keeps working with the village open.
pub const SPLIT_SLIDER: Rect = Rect::new(PANEL_X, 257, PANEL_W, 296 - 257 + 1);

/// Where a click means "that county on the map". Half-open, and it stops at
/// 478 because `Clip_Horizontal` stops there.
pub const MAP_AREA: Rect = Rect::new(0, TOP_BAR, PANEL_X, NEAR.bottom() - TOP_BAR);

/// Half-width of a county's marker square. **Ours** — the original draws a
/// county flag from `Flags1a.pl8` over the castle tile instead, which we do not
/// yet place.
const MARKER: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
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
struct IndustrySite {
    /// Tile index, `y * 64 + x`. Fixed for the life of the map.
    tile: usize,
    county: u8,
    /// [`l2_kingdom::tables::Commodity::index`] — wood 0, iron 1, weapons 2,
    /// stone 3.
    commodity: usize,
    /// The frame the tile is drawn with right now.
    frame: u8,
}

pub struct MapScreen {
    zoom: Zoom,
    view: Viewport,
    /// The near view's scroll origin while the far view is up, so zooming back
    /// in returns where it left — `Map_ZoomOut` saves it and `Map_ZoomIn`
    /// restores it.
    saved: Viewport,
    /// The painted tiles, and what county each pixel came from. Rebuilt when
    /// the slot, the zoom or the scroll origin changes — which is what makes
/// scrolling costs one repaint per frame.
    base: Canvas,
    tags: Tags,
    /// `(map slot, zoom, viewport, turn, season, field digest, castle digest)`
    /// — everything the painted base plane depends on. **Two branches added a
    /// component to this tuple on the same day and neither knew about the
/// other**. It is a tuple, not a
    /// hand-written comparison: a missed component is a stale picture, and a
    /// stale picture is the hardest defect on this screen to attribute.
    /// The last `u64` is the fog — [`MapScreen::fog_key`].
    built: Option<(usize, u8, Viewport, u32, u8, u64, u64, u64, u64)>,
    /// **Every industry site on the map,
    /// `Sprite_TopIt` arm 5b's `g_tiles[].frame`, which the original steps in
    /// the tile record itself.
    ///
    /// It is here and not in `Kingdom` because it is **display state driven by
    /// a clock**, and `docs/netcode.md` D-12 says nothing below this crate may
    /// read one. The original has no such constraint — its tile array is both
    /// the simulation's map
    /// animation departs from it, in storage and not in behaviour.
    ///
    /// Built once per map slot by [`MapScreen::rebuild_industry_sites`]: a
    /// site's *tile* never moves, only its terrain and its frame change.
    industry_sites: Vec<IndustrySite>,
    industry_slot: Option<usize>,
    /// The **gate** counter the four rungs are derived from — one step per
    /// `Tick_Pulses` gate, not per fixed tick. See
    /// [`MapScreen::step_industry`].
    industry_tick: u32,
    /// Milliseconds since the last gate, zeroed on each — `Tick_Pulses`
    /// (`0x004BBC80`) sets `stamp = now`. See [`l2_view::village::GATE_MS`].
    industry_gate_ms: u32,
    /// The two 128 × 128 rasters for this slot, decoded once.
    minimap: Option<Minimap>,
    minimap_slot: Option<usize>,
    focus: Focus,
    /// One line of feedback about the last thing that happened. **Ours.**
    status: String,
    /// Where the pointer last was, in canvas pixels. **Edge scrolling needs a
    /// position that outlives the event that carried it**: the player holds the
    /// cursor still against the edge of the window
    /// moving, so the scroll happens in [`Screen::update`] and reads this.
    pointer: (i32, i32),
    /// Whether the pointer is over the window at all. `WindowEvent::CursorLeft`
    /// clears it,
    /// from wherever it was last seen.
    pointer_in: bool,
    /// Set when `update` moved the map, so [`Machine`](crate::screen::Machine)
    /// knows to repaint without an event having arrived.
    scrolled: bool,
    /// Whether the opening centre-on-the-player's-county has been done.
    ///
/// for why the map does not
    /// stay where `Map_InitMode` put it.
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
    /// eight.
    ///
    /// `FUN_004CFB08` steps both counters behind one 16 ms `GetTickCount` gate,
    /// so a herd holds each frame for 16 × 16 ms
    /// seconds. **This is not `Tick_Pulses`** (`0x004BBC80`), the 20 ms
    /// `timeGetTime` divider chain the village animates off — the campaign map
    /// has its own clock.
    /// because the natural guess is that everything on screen shares one.
    ///
    /// It is stepped from [`Screen::update`] for the same reason `flag_tick`
    /// is: nothing below this crate may read a clock, and this must never
    /// reach the simulation (`docs/netcode.md` D-12).
    herd_tick: u8,
    herd_phase: u8,
    /// **`g_selectedUnit`.
    ///
    /// `Map_Click`'s army branch is three lines: a picked unit of type 1 that is
    /// the local player's either opens the siege screen (`+0x199` set, after
    /// `Siege_ValidateLink` has had a chance to clear it) or goes to
    /// `Panel_MoveButton`, which is `g_screenId = 0x10` — the campaign map
    /// *in move-order mode* — with `g_selectedUnit` set and a flood fill run
    /// from the unit. The next click on the map is
    /// `Map_ConfirmMoveOrder`, which places the order.
    ///
    /// So the original does not have a separate move screen; it has the map
    /// with a selection. This is that selection, and it is why a click on a
    /// tile means *march there* while it is `Some`.
    selected_unit: Option<usize>,
    /// **What `Map_BeginMoveSelection` sets up and `Map_HoverUnitTarget`
    /// reads every frame** — the flood fill,
    /// pointer is over right now. `None` unless [`MapScreen::selected_unit`] is
    /// `Some`; the two are set and cleared together by
    /// [`MapScreen::begin_move_selection`] and
    /// [`MapScreen::cancel_move_selection`].
    move_order: Option<MoveOrder>,
    /// **The farm/industry slider is held.** `FUN_00439122` acts on the button
    /// being *down*, not on it having been clicked, so the value tracks the
    /// pointer for as long as it is held — see [`SPLIT_SLIDER`].
    slider_held: bool,
    /// **`g_minimapMode` (`0x0057A0C4`)** — what the minimap is coloured by.
    /// Set by [`MINIMAP_MODE_BUTTONS`] through `Minimap_ModeButton`.
    minimap_mode: MinimapMode,
    /// **The end-of-turn screen fade, while it is running.** See [`Fading`].
    fading: Option<Fading>,
    /// **A `Save_RotateAndWrite` is owed** — `FUN_0049A3E6`'s last statement,
    /// raised at the bottom of the fade and drained by the machine. See
    /// [`crate::screen::Screen::take_autosave`].
    autosave: bool,
    /// The player's gold when End Turn was pressed, so the *"you gained N"*
    /// line still compares against the right number however many frames later
    /// the turn finishes. See [`MapScreen::end_turn`].
    gold_at_turn_start: i32,
    /// **`g_optScrollSpeed` (`0x0053F234`).** 0 … 100 in steps of ten, shown on
    /// the options slider as 0 … 10; the shipped default is
    /// [`DEFAULT_SCROLL_SPEED`].
    ///
/// It lives on the screen, not in `Options`;
    /// rule: nothing in the simulation reads it, and two lockstep peers may
    /// disagree about it the way they may disagree about window size.
    /// `Menu_ScrollSpeed` (`0x00434CEE`) is the control that sets it, the
    /// options menu is not drawn yet, and this is the seam it will attach to.
    /// See [`MapScreen::scroll_interval_ticks`].
    scroll_speed: i32,
    /// Ticks still to wait before the next edge-scroll step.
    scroll_wait: u32,
    /// `g_confirmWidgets`' press timer — the twenty frames the gauntlet is held
    /// down before the answer, the same as the battlefield's box.
    press: Press,
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
struct Fading {
    pub(super) phase: u8,
/// Held
    /// written immediately because the numbers it names change *during* the
    /// dark, and announcing them early is the abruptness the fade hides.
    status: String,
    /// Whether the turn ended the game, in which case the conquest screen comes
    /// up when the light does — as it does in the original, where
    /// `g_screenId = 0x1C` is set on the far side of the same fade.
    over: bool,
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
struct MoveOrder {
    /// The unit the order is for — `g_selectedUnit` (`0x0057C8C4`).
    unit: usize,
    /// The terrain costs the fill was run over.
    cost: l2_kingdom::map::CostMap,
    /// `g_moveDistLocal`, filled from the unit's own tile.
    field: l2_kingdom::movement::DistanceField,
    /// `DAT_005691E0` — the tile the last descent was run for, so the pointer
    /// moving *within* a tile costs nothing.
    hovered: Option<(u8, u8)>,
    /// `g_pathBuf[localPlayer]` — the route `Path_MarkPreviewTiles`
    /// (`0x004A91BA`) marks and `Map_DrawPathMarker` (`0x004081A6`) draws.
    /// Empty when the hovered tile is unreachable, which is
    /// `g_moveOrderAvailable = 0`.
    path: Vec<(u8, u8)>,
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

/// `L2.eng` group 101 — the sixty map names `g_scenarioIndex` indexes, the same
/// group `ScenarioList_Draw` and `Screen_DrawConquest` read.
const FAR_BOX_MAP_GROUP: usize = 101;
/// Group 34: index 0 *"Year"*, index 1 *"Click on the county you wish to view."*
/// `Screen_DrawCampaign` is its only consumer — so it is this box's vocabulary,
/// not a naming lead. `CLAUDE.md` rule 6.
const FAR_BOX_YEAR_GROUP: usize = 34;
const FAR_BOX_YEAR_LABEL: usize = 0;
const FAR_BOX_ADVICE: usize = 1;
const FAR_BOX_Y: i32 = 0x1A8;
const FAR_BOX_NAME_X: i32 = 0x40;
const FAR_BOX_YEAR_LABEL_X: i32 = 0x50;
const FAR_BOX_YEAR_X: i32 = 0x60;
const FAR_BOX_ADVICE_X: i32 = 0x50;
const FAR_BOX_ADVICE_Y: i32 = 0x1C6;

/// One of group 34's two strings, **as the player's own file spells them**, with
/// our transcription for an install that has no `L2.eng`. `CLAUDE.md` rule 6:
/// the fallback is the fallback, not the source.
fn far_box_text(ctx: &Ctx, index: usize) -> String {
    let s = ctx.assets.shell.text(FAR_BOX_YEAR_GROUP, index);
    if !s.is_empty() {
        return s.to_string();
    }
    match index {
        FAR_BOX_YEAR_LABEL => "Year".to_string(),
        _ => "Click on the county you wish to view.".to_string(),
    }
}

