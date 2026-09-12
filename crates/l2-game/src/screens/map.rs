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
//! This module's arms, so that the next person can check the list rather than
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
//! `FUN_00439079` and the job rows. Those three lines were **twenty-one input
//! arms** between them — the menu bar alone is sixteen items over three
//! drop-downs — which is what a one-line table row can hide.
//! `docs/arms.json` groups `menu-bar` and `right-column`.)*
//!
//! `Map_Click` itself is six branches, in this order, and the order is the rule
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
//! the tiles behind it (C57, the mine); and one whose arithmetic was simply
//! wrong (C60, `pick_tile` dividing by `tile_w / 2` where `Map_PickTile` divides
//! by the half pitch — 56 dead pixels around every tile centre). Three different
//! mistakes. **All three became *the wrong screen opening* rather than nothing
//! happening**, because a miss had somewhere to fall through to — and the player
//! reported the arm itself in the end: *"if you click anywhere on grass it opens
//! up the tax window too."*
//!
//! So the hit tests are still worth getting right, and now a mistake in one is
//! quiet rather than loud. `a_click_on_plain_ground_changes_nothing_at_all` is
//! the assertion that could not exist while the arm did; it covers the whole
//! kingdom, not the screen id, because *nothing happened* is the claim.
//!
//! # What is the original's, and what is ours
//!
//! **The original's:** the viewport and both zooms, the scroll clamp and the
//! eight scroll directions, the `−4 / −12` centring, the tile artwork, the
//! menu-bar background tiled from `Panels.pl8`, the `Misc_cty.pl8` right
//! column, the `MAPnn.PL8` minimap and its realm colour ramp out of
//! `Lords2.exe`, and the End Turn strip's rectangle.
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
//! county-selection arm and the outline were one invention with two halves, the
//! arm was removed and the paint was not, and a player reported the leftover
//! four merges later — *"still a weird yellow outline around the county that is
//! selected on the real map."* The lesson is not that somebody was careless.
//! **The removal was recorded in the input inventory, and the surviving half
//! was in the painter, where the input inventory does not look.**
//! `docs/decisions.md` C132.
//!
//! **And the assertion written to keep it gone was itself vacuous for a while.**
//! `the_selection_is_not_drawn_on_the_map` compared the first two counties the
//! player does not own — 1 and 2 — and the viewport this map opens on shows 8
//! and 9. Putting the outline back turned nothing red. It sweeps the counties
//! the pick plane says are *on screen* now, and the `assert!` that the visible
//! set is non-empty is the part that stops it happening again.
//! `docs/decisions.md` C138.
//!
//! # Picking
//!
//! A click on the map reads the tag plane, which is exact by construction: it
//! answers with the county whose tile is actually visible at that pixel. The
//! original inverts the projection instead (`Map_PickTile`), which is a
//! different algorithm reaching the same answer for the same reason.
//!
//! A click on the minimap goes through `MAPnn.PL8`'s own county raster, which
//! *is* what the original does, and then centres the map on that county.

use l2_kingdom::field::FieldType;
use l2_kingdom::industry;
use l2_formats::maps::Plane;
use l2_view::campaign::{self, Dir, Lattice, Viewport, Zoom, FAR, NEAR, PANEL_W, PANEL_X};
use l2_view::chrome::{self, Minimap, MinimapMode, MinimapTint};
use l2_view::{text, Canvas, Clip, Ink, Tags};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::county;
use crate::screens::menubar;
use crate::screens::saveload::Mode as SaveLoadMode;
use crate::turn;
use crate::shell::font;
use crate::widget;

/// The menu bar: `Screen_DrawMenuBar`'s 640 × 24 strip at y 0.
pub const TOP_BAR: i32 = campaign::TOP_BAR_H;

/// The screen the original ran at, and the only size a canvas ever is.
pub const CANVAS_W: i32 = l2_view::canvas::WIDTH as i32;
pub const CANVAS_H: i32 = l2_view::canvas::HEIGHT as i32;

/// The right column: `Misc_cty` frames 54 … 59 at x 478, 162 wide.
pub const PANEL: Rect = Rect::new(PANEL_X, TOP_BAR, PANEL_W, 480 - TOP_BAR);

/// The End Turn strip — `Misc_cty` frame 59 at (478, 460), with `L2.eng` group
/// 4 centred in it.
///
/// **The hit box is 161 × 19 and not 162 × 20**, and that is `g_sidebarButtons`
/// record 5 rather than the plate: `(0, 30) … (161, 49)` at the table's
/// (`0x1DE`, `0x1AE`) offset, with `Hotspot_Test` half-open on **both** axes —
/// `my < y0 + off || y1 + off <= my` rejects. So the strip's own last column
/// (x 639) and last row (y 479) are dead in the original, exactly like the
/// one-pixel dead columns between the five icons above it, and ours had them
/// live. Read out of the player's `Lords2.exe` in
/// `crates/l2-game/tests/right_column.rs`; the artwork is 162 × 20 and the
/// hotspot is not, which is why deriving this from the plate was wrong.
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
/// **The table itself is the arm**, and the six things it dispatches to are six
/// more: `Sidebar_ButtonClicked` (`0x00432967`) is one
/// `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` call and nothing else.
/// The sixth record is **End Turn**, `(0, 30) … (161, 49)` at that offset, whose
/// handler is `Turn_End` rather than `Sidebar_Button`; it is [`END_TURN_BUTTON`]
/// here and its own arm.
// arm: 0x00432967/sidebar-hit-test left-press
pub const SIDEBAR_BUTTONS: [SidebarButton; 5] = [
    // `Levy_SetPercent(sel, g_levyPercent); FUN_004AA90A(sel, g_levyMen);
    // g_screenId = 0x17`, and the mercenary band is loaded on top of it when
    // the county has an offer. `L2.eng` group 69 index 0x10 is "Raising an
    // army in", so 0x17 is the **raise-army** screen and the mercenary offer is
    // an optional half of it. The shell table called the whole screen "Hire
    // mercenaries" — the smaller half naming the larger — and this button and
    // that name were corrected in the same afternoon by two agents who had not
    // spoken. `docs/decisions.md` C45; the screen is `crate::screens::army` and
    // is not a shell any more, which is why this goes through
    // [`sidebar_destination`].
    SidebarButton { x: 0, w: 33, action: SidebarAction::Screen(0x17), name: "ARMY" },
    SidebarButton { x: 34, w: 31, action: SidebarAction::Screen(0x09), name: "COURT" },
    SidebarButton { x: 66, w: 31, action: SidebarAction::Screen(0x18), name: "SUPPLY" },
    // `FUN_00436A88`: `g_screenId = 0x1B`, castle building.
    SidebarButton { x: 98, w: 31, action: SidebarAction::Screen(0x1B), name: "CASTLE" },
    // `FUN_0043611B`: `g_screenId = 0x0B`, the other lords.
    SidebarButton { x: 130, w: 31, action: SidebarAction::Screen(0x0B), name: "LORDS" },
];

/// **`g_minimapModeButtons` (`0x004DC620`) — the four icons in the strip beside
/// the minimap**, `Misc_cty` frame `0x5C` (29 × 123) at (611, 32).
/// `FUN_0043292D` tests them at offset (610, 32) and `Minimap_ModeButton`
/// (`0x0043AB76`) handles all four.
///
/// The top three switch `g_minimapMode` — 1 the labour rating, 2 the food
/// rating, 3 happiness — which recolours the minimap from a second ramp
/// (`g_minimapRatingRamp`, `0x004D28F8`, transcribed as
/// [`chrome::MINIMAP_RATING_RAMP`]). The fourth is **the zoom toggle** in mode
/// 0 and **the way back out of an overlay** in every other mode; it is the
/// control `docs/screens.md` §7 says we replaced with the `Z` key.
/// [`MapScreen::minimap_mode_button`] has the whole of that behaviour.
///
/// The second record's `y1` is `0x42` where the pattern wants `0x3F`, so band 2
/// is 34 pixels tall and overlaps band 3's first two rows. `Hotspot_Test`
/// returns on the first match, so y 96 and 97 select mode 2. **That is the
/// original's own data**, transcribed rather than tidied.
/// What our status line calls each overlay. **Ours** — the original labels them
/// only with the button icons and the badge.
///
/// **`L2.eng` does have words for them, and this comment said it did not.** The
/// original's tooltip layer (`FUN_00476E95`, gated on `g_optToolTips`) resolves
/// the three mode buttons through `FUN_00477320` to tip ids 2, 3 and 4 and
/// draws group **220** at those indices: *"Labour, red if needed, purple if
/// idle."*, *"Ration status"* and *"Overall happiness"* — with *"Overview map"*
/// on the fourth button and *"Return census map to empire mode"* (index 31) once
/// an overlay is up. `docs/draws-map.md` §5.1, **C86**. The status line stays
/// ours, because a status line is not a tooltip; the tips themselves are drawn
/// from the player's own group 220 by [`crate::tooltip`], which reads
/// [`MapScreen::minimap_mode`] through [`Screen::minimap_mode`].
fn minimap_mode_name(mode: MinimapMode) -> &'static str {
    match mode {
        MinimapMode::Owner => "OWNERS",
        MinimapMode::Labour => "LABOUR",
        MinimapMode::Food => "FOOD",
        MinimapMode::Happiness => "HAPPINESS",
    }
}

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

impl SidebarButton {
    pub const fn rect(&self) -> Rect {
        Rect::new(PANEL_X + self.x, chrome::PANEL_STATUS_Y, self.w, SIDEBAR_H)
    }
}

/// **29, not 30**, and the difference is a dead row.
///
/// Every one of the five records is `(x, 0) … (x, 29)` at the table's `0x1AE`
/// offset and `Hotspot_Test` is half-open, so the strip is y 430 … 458 and
/// **y 459 belongs to nothing** — the same one-pixel gutter the table leaves
/// between each pair of icons horizontally, once, horizontally across the whole
/// strip. This used to be `PANEL_END_TURN_Y − PANEL_STATUS_Y`, which is the
/// distance between two *plates* and not the height of a *hotspot*, and it made
/// the dead row live. See `crates/l2-game/tests/right_column.rs`, which reads
/// the table out of the player's own copy.
pub const SIDEBAR_H: i32 = 29;

/// What one of them does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    /// The `g_screenId` the table's handler sets. Four of the five are still
    /// [`crate::screens::shells`] entries, so the button reaches the original's
    /// own artwork and the shell says for itself what it does not yet do; the
    /// fifth is the raise-army screen, which is built. See
    /// [`sidebar_destination`].
    Screen(u8),
}

/// **Which of our screens a sidebar button opens.**
///
/// The table stores a `g_screenId` because that is what `Sidebar_Button`
/// writes. **All five now resolve to a screen** — the shell table is empty —
/// and two need the county, because in the original the whole strip is *about*
/// `g_selectedCounty`: `Sidebar_Button`'s own arm is
/// `Levy_SetPercent(g_selectedCounty, g_levyPercent); FUN_004AA90A(g_selectedCounty, g_levyMen)`.
///
/// **This is the function to change when a screen graduates**, and the test
/// below is what makes forgetting it a failure rather than a silently dead
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
        // Hotspot 3. The destination opens equal to the source and the minimap
        // is the only thing that moves it.
        0x18 => ScreenId::Supplies(county),
        // `FUN_0043611B` — the LORDS button — is a bare `g_screenId = 0x0B`
        // with no county in it at all, because the diplomacy screen is about
        // realms rather than counties. `Diplo_DrawScreen` picks its own target
        // through `Diplo_DefaultTarget`.
        // It does not touch `g_diploTarget`; the painter's prologue heals a
        // stale one.
        0x0B => ScreenId::Diplomacy,
        //
        // There is no sixth hotspot, so the fall-through is unreachable from
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
/// percentage of the county's people that goes to the mines and the smithy
/// rather than to the fields. It was not wired, and a player reported that he
/// could not assign peasants.
///
/// # It is a **drag**, and the flags say which kind
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

/// The slider's own arithmetic, verbatim: left of the track steps down by four,
/// right of it up by four, and on the track the value is
/// `((x - 531) * 2) & 0xFC` — masked, so it lands on a multiple of four.
///
/// **The three zones are half-open and the upper bound is 594, not 595.** The
/// original is `if (mx < 0x213) down; else if (mx < 0x252) track; else up;` —
/// so x = 594 steps the share **up**. This read `x > 594` and put that one
/// column on the track instead: a wrong arm rather than a missing one, and the
/// kind nothing looks broken about.
pub fn split_from_click(x: i32, current: i32) -> i32 {
    let next = if x < 531 {
        current - 4
    } else if x >= 594 {
        current + 4
    } else {
        ((x - 531) * 2) & 0xFC
    };
    next.clamp(0, 100)
}

/// Where a click means "that county on the map". Half-open, and it stops at
/// 478 rather than 480 because that is where `Clip_Horizontal` stops.
pub const MAP_AREA: Rect = Rect::new(0, TOP_BAR, PANEL_X, NEAR.bottom() - TOP_BAR);

/// Half-width of a county's marker square. **Ours** — the original draws a
/// county flag from `Flags1a.pl8` over the castle tile instead, which we do not
/// yet place.
const MARKER: i32 = 2;

/// **The field markers, which are debug overlay now.**
///
/// This module used to hold a brush *popup* of ours on the campaign map, opened
/// by a left click on a field. It is gone: in the original that click is
/// `Map_Click`'s farmland arm — `_DAT_005681CC = 3; g_screenId = 4;
/// FUN_0041B032();` — which opens **screen `0x04`, the same information panel a
/// right click opens**, and the brush is drawn and hit-tested there
/// (`FUN_0041C996`, `FUN_00438990`). See [`crate::screens::info`].
///
/// What is left is the squares we drew on the county's fields, coloured by what
/// each is used for. **The original draws nothing there** — `Sprite_TopIt`'s
/// farm arm is the pasture herd and nothing else, and the crop is the tile's own
/// artwork (`Terrain_Set`, which [`MapScreen::field_graphics`] reproduces) — so
/// they are drawn only with [`crate::game::Prefs::debug_overlay`] on.
mod brush {
    /// Half-width of a field marker. **Ours**, debug overlay only.
    pub const FIELD_MARKER: i32 = 3;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    None,
    /// One of the five `g_sidebarButtons`, by index.
    Sidebar(usize),
    EndTurn,
}

/// **Where the town's 2 × 2 block is re-stamped to, by the county's own
/// population.** `FUN_0046ac22` is called with `'/'`, `'3'` or `'7'` — 47, 51
/// and 55 — and the population pass re-runs it every season.
///
/// The thresholds are the original's literals `0x321` and `0x4b1`, tested as
/// `pop < 801` and `pop < 1201`.
const TOWN_FRAME_BASE: [(i32, u8); 3] = [(801, 47), (1201, 51), (i32::MAX, 55)];

/// The plane-1 byte a town tile carries: bank `0x0c`, `Town1a.pl8`.
const TOWN_BANK: u8 = 0x0c;

/// **One industry's building on the campaign map, and the frame it is showing.**
///
/// The mine, the quarry, the forest and the smithy are `Town1a.pl8` frames like
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
    /// scrolling cost one repaint rather than one per frame.
    base: Canvas,
    tags: Tags,
    /// `(map slot, zoom, viewport, turn, season, field digest, castle digest)`
    /// — everything the painted base plane depends on. **Two branches added a
    /// component to this tuple on the same day and neither knew about the
    /// other**, which is the argument for it being a tuple rather than a
    /// hand-written comparison: a missed component is a stale picture, and a
    /// stale picture is the hardest defect on this screen to attribute.
    /// The last `u64` is the fog — [`MapScreen::fog_key`].
    built: Option<(usize, u8, Viewport, u32, u8, u64, u64, u64, u64)>,
    /// **Every industry site on the map, and the frame its wheel is on** —
    /// `Sprite_TopIt` arm 5b's `g_tiles[].frame`, which the original steps in
    /// the tile record itself.
    ///
    /// It is here and not in `Kingdom` because it is **display state driven by
    /// a clock**, and `docs/netcode.md` D-12 says nothing below this crate may
    /// read one. The original has no such constraint — its tile array is both
    /// the simulation's map and the renderer's — so this is the one place the
    /// animation departs from it, in storage and not in behaviour.
    ///
    /// Built once per map slot by [`MapScreen::rebuild_industry_sites`]: a
    /// site's *tile* never moves, only its terrain and its frame change.
    industry_sites: Vec<IndustrySite>,
    industry_slot: Option<usize>,
    /// The fixed-tick counter the four pulses are derived from. See
    /// [`MapScreen::step_industry`].
    industry_tick: u32,
    /// The two 128 × 128 rasters for this slot, decoded once.
    minimap: Option<Minimap>,
    minimap_slot: Option<usize>,
    focus: Focus,
    /// One line of feedback about the last thing that happened. **Ours.**
    status: String,
    /// Where the pointer last was, in canvas pixels. **Edge scrolling needs a
    /// position that outlives the event that carried it**: the player holds the
    /// cursor still against the edge of the window and the map has to keep
    /// moving, so the scroll happens in [`Screen::update`] and reads this.
    pointer: (i32, i32),
    /// Whether the pointer is over the window at all. `WindowEvent::CursorLeft`
    /// clears it, so a cursor that has left the window does not go on scrolling
    /// from wherever it was last seen.
    pointer_in: bool,
    /// Set when `update` moved the map, so [`Machine`](crate::screen::Machine)
    /// knows to repaint without an event having arrived.
    scrolled: bool,
    /// Whether the opening centre-on-the-player's-county has been done.
    ///
    /// See [`MapScreen::open_on_the_player`] for why the map does not simply
    /// stay where `Map_InitMode` put it.
    opened: bool,
    /// `DAT_0057D378`, the map's animation tick, and `DAT_0057D390`, the flag's
    /// wave phase, which is that counter mod `0x80` shifted right by four.
    ///
    /// `FUN_004CFB08` advances the counter once per **16 ms** of `GetTickCount`
    /// and then draws a frame, so a flag holds each of its eight frames for
    /// 16 × 16 ms and the whole wave takes 2.05 seconds. Our fixed tick is 16 ms
    /// (`main::TICK`) and nothing below this crate reads a clock, so the counter
    /// is stepped by [`Screen::update`] and the arithmetic is the original's.
    flag_tick: u8,
    flag_phase: u8,
    /// `DAT_0057D388`, the map's **second** animation counter, and
    /// `_DAT_0057D38C`, the herd's grazing phase, which is that counter mod
    /// `0x60` shifted right by four — so **six** phases rather than the flag's
    /// eight.
    ///
    /// `FUN_004CFB08` steps both counters behind one 16 ms `GetTickCount` gate,
    /// so a herd holds each frame for 16 × 16 ms and the loop takes 1.54
    /// seconds. **This is not `Tick_Pulses`** (`0x004BBC80`), the 20 ms
    /// `timeGetTime` divider chain the village animates off — the campaign map
    /// has its own clock, and the two are different rates. Worth stating
    /// because the natural guess is that everything on screen shares one.
    ///
    /// It is stepped from [`Screen::update`] for the same reason `flag_tick`
    /// is: nothing below this crate may read a clock, and this must never
    /// reach the simulation (`docs/netcode.md` D-12).
    herd_tick: u8,
    herd_phase: u8,
    /// **`g_selectedUnit`, and the map is in move-order mode while it is set.**
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
    /// reads every frame** — the flood fill, and the route to the tile the
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
    /// It lives on the screen rather than in `Options` because it is not a
    /// rule: nothing in the simulation reads it, and two lockstep peers may
    /// disagree about it the way they may disagree about window size.
    /// `Menu_ScrollSpeed` (`0x00434CEE`) is the control that sets it, the
    /// options menu is not drawn yet, and this is the seam it will attach to.
    /// See [`MapScreen::scroll_interval_ticks`].
    scroll_speed: i32,
    /// Ticks still to wait before the next edge-scroll step.
    scroll_wait: u32,
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
/// palette arithmetic and the entry range are in [`l2_view::fade`].
#[derive(Debug, Clone)]
struct Fading {
    phase: u8,
    /// What to put in the status line once the light is back. Held rather than
    /// written immediately because the numbers it names change *during* the
    /// dark, and announcing them early is the abruptness the fade hides.
    status: String,
    /// Whether the turn ended the game, in which case the conquest screen comes
    /// up when the light does — as it does in the original, where
    /// `g_screenId = 0x1C` is set on the far side of the same fade.
    over: bool,
}

/// One fixed simulation tick in milliseconds — `main::TICK`.
///
/// **This is a constant, not a clock.** Nothing here asks how long a frame
/// actually took; the number exists so an interval the original states in
/// milliseconds can be converted to the whole ticks this crate is allowed to
/// count. `VillageScreen::CLICK_SETTLE_TICKS` makes the same conversion by
/// hand and for the same reason (`docs/netcode.md`).
const TICK_MS: u32 = 16;

/// `g_optScrollSpeed`'s shipped default, written by the options-defaults
/// routine at `0x004AE310`. **[V]**
pub const DEFAULT_SCROLL_SPEED: i32 = 60;

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

impl MapScreen {
    pub fn new() -> MapScreen {
        // `Map_InitMode`: zoom 0, scroll origin row 0x4A col 0x14.
        let view = Viewport::START.clamped(&NEAR);
        MapScreen {
            zoom: NEAR,
            view,
            saved: view,
            base: Canvas::screen(),
            tags: Tags::screen(),
            built: None,
            industry_sites: Vec::new(),
            industry_slot: None,
            industry_tick: 0,
            minimap: None,
            minimap_slot: None,
            focus: Focus::None,
            status: "CLICK A COUNTY".into(),
            pointer: (CANVAS_W / 2, CANVAS_H / 2),
            pointer_in: false,
            scrolled: false,
            opened: false,
            flag_tick: 0,
            flag_phase: 0,
            herd_tick: 0,
            herd_phase: 0,
            selected_unit: None,
            move_order: None,
            slider_held: false,
            minimap_mode: MinimapMode::Owner,
            fading: None,
            autosave: false,
            gold_at_turn_start: 0,
            scroll_speed: DEFAULT_SCROLL_SPEED,
            scroll_wait: 0,
        }
    }

    /// `Minimap_ModeButton` (`0x0043AB76`), button 0…3 of
    /// [`MINIMAP_MODE_BUTTONS`].
    ///
    /// **The original is not a set of four radio buttons**, and this is the
    /// shape of it:
    ///
    /// * in mode 0, buttons 1…3 select their mode and button 4 toggles the map
    ///   zoom;
    /// * in any other mode, **button 4 turns the overlay off** and buttons 1…3
    ///   do nothing at all.
    ///
    /// So there is no switching straight from food to happiness: the overlay
    /// has to be turned off first. The artwork agrees — `Misc_cty` frame `0x5B`,
    /// the strip drawn while a mode is up, has one button on it where frame
    /// `0x5C` has four.
    fn minimap_mode_button(&mut self, ctx: &mut Ctx, button: usize) {
        if self.minimap_mode == MinimapMode::Owner {
            match MinimapMode::from_button(button) {
                // arm: 0x0043AB76/minimap-mode-set left-press
                Some(mode) => {
                    self.minimap_mode = mode;
                    self.status = format!("MINIMAP {}", minimap_mode_name(mode));
                }
                // arm: 0x0043AB76/minimap-zoom-toggle left-press
                None => self.toggle_zoom(ctx),
            }
        } else if button == 3 {
            // arm: 0x0043AB76/minimap-mode-clear left-press
            self.minimap_mode = MinimapMode::Owner;
            self.status = "MINIMAP OWNERS".into();
        }
    }

    /// **`FUN_00439079` (`0x00439079`)** — a right release anywhere in
    /// `x >= 0x1DE, 0x18 <= y < 0x99` with a minimap overlay up **clears the
    /// overlay and swallows the click**, and does nothing at all when no overlay
    /// is up.
    ///
    /// It is guard 2 of the `g_screenId == 0` arm, ahead of the sidebar and
    /// *outside* the turn-ended gate, and it is also guard 6 of the village's
    /// and of all four county panels'. This module's header carried it as *"not
    /// reproduced"*.
    ///
    /// The rectangle is the **minimap and its button strip**, not the whole
    /// column: y stops at 152, which is four pixels above the county strip's
    /// plate at 156.
    fn clear_minimap_mode(&mut self, x: i32, y: i32) -> bool {
        if x < PANEL_X || !(0x18..0x99).contains(&y) || self.minimap_mode == MinimapMode::Owner {
            return false;
        }
        // arm: 0x00439079/right-clears-minimap-mode right-release
        self.minimap_mode = MinimapMode::Owner;
        self.status = "MINIMAP OWNERS".into();
        true
    }

    /// `CountyStrip_JobClick`'s ownership gate and its geometry, which is
    /// [`county::job_row_at`].
    fn job_row_at(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        if !ctx.game.is_players(ctx.game.selected) {
            return None;
        }
        let c = ctx.game.kingdom.counties.get(ctx.game.selected as usize)?;
        county::job_row_at(c, x, y)
    }

    /// Set `g_optScrollSpeed`. 0 … 100; 0 disables scrolling, as it does in the
    /// original. See [`MapScreen::scroll_interval_ticks`].
    pub fn set_scroll_speed(&mut self, speed: i32) {
        self.scroll_speed = speed.clamp(0, 100);
        self.scroll_wait = 0;
    }

    /// **`Map_ScrollThrottle` (`0x004BBBE3`), in ticks.**
    ///
    /// The original:
    ///
    /// ```c
    /// elapsed = timeGetTime() - g_lastScrollTick;
    /// q = (100 - g_optScrollSpeed) / 10;
    /// if (q >= 10) return 0;                       /* speed 0 never scrolls */
    /// if (g_screenId == 0x10) q += 2;
    /// if (q * 12 + 2 > elapsed) return 0;
    /// g_lastScrollTick = timeGetTime();  return 1;
    /// ```
    ///
    /// So the interval is **`((100 − speed) / 10) × 12 + 2` milliseconds**, the
    /// remainder is discarded rather than carried, and `Map_EdgeScroll` itself
    /// is called unconditionally every frame — the *detection* runs at frame
    /// rate and only the *movement* is gated. `g_optScrollSpeed` is a 0 … 100
    /// slider in steps of ten shown as 0 … 10, and the default written by the
    /// options-defaults routine at `0x004AE310` is **60**, which is 50 ms,
    /// which is **20 tiles a second**. **[V]** — decoded from the binary.
    ///
    /// Ours had no throttle at all and scrolled one tile per fixed tick, which
    /// is 62.5 a second: **three times too fast**. A player said *"mouse scroll
    /// needs to be like… half that speed, not sure if it's a game default or
    /// some cycle thing"*, and it was both — there is a game default and it is
    /// applied on a timer.
    ///
    /// # The quantisation is ours and this is it
    ///
    /// Nothing below `main.rs` may read a clock (`docs/netcode.md`), so the
    /// millisecond interval becomes a whole number of [`TICK_MS`] ticks,
    /// **rounded to nearest** and never below one. At the default that is 3
    /// ticks — 48 ms, 20.8 tiles a second against the original's 20.0. Rounding
    /// rather than flooring or ceiling is what keeps it close: 4 ticks would be
    /// 64 ms and visibly slower than the game.
    fn scroll_interval_ticks(&self) -> u32 {
        let speed = self.scroll_speed.clamp(0, 100);
        let q = (100 - speed) / 10;
        if q >= 10 {
            // Speed 0 disables scrolling outright, which is a real setting and
            // not a degenerate one: `Map_ScrollThrottle` returns 0 for ever.
            return u32::MAX;
        }
        let ms = (q * 12 + 2) as u32;
        ((ms + TICK_MS / 2) / TICK_MS).max(1)
    }

    /// One frame of the farm/industry slider: `FUN_00439122`'s body, once the
    /// button is known to be down and the pointer inside [`SPLIT_SLIDER`].
    fn drag_split(&mut self, ctx: &mut Ctx, x: i32) {
        let id = ctx.game.selected as usize;
        let Some(current) = ctx.game.kingdom.counties.get(id).map(|c| c.industry_share) else {
            return;
        };
        let next = split_from_click(x, current);
        // `if (next == share) return 1;` — the original checks and skips the
        // recompute, which matters here for the same reason: dragging along
        // one snapped step must not rerun the allocator on every pixel.
        if next != current && ctx.game.kingdom.set_industry_share(id, next) {
            self.status = format!("INDUSTRY {next}% FARM {}%", 100 - next);
        }
    }

    /// **Open the map where the player's own county is** — correction C48.
    ///
    /// `Map_InitMode` puts the scroll origin at row `0x4A`, column `0x14`, and
    /// we reproduced that faithfully and stopped there. The original does not:
    /// the last thing `Game_SetupRealmsAndCounties` (`0x0049BD99`) does, after
    /// every realm has its county and its starting garrison, is
    ///
    /// ```c
    /// FUN_00432746(g_playerStartTable[g_localPlayer * 2]);   /* 0x00432746 */
    ///     -> if (county.townTile) { Map_CentreOnTile(county.townTile);
    ///                               g_selectedCounty = county; }
    /// ```
    ///
    /// so a new game opens looking at **the player's own town**, not at row
    /// `0x4A`. Ours opened on a stretch of England the player owned nothing in:
    /// on the turn-one fixture the near view is eight lattice columns wide and
    /// county 8's town is fourteen columns outside it, so his county, his
    /// merchants and the army he raised were all off the side of the screen.
    /// That is the second half of *"I raised an army and nothing appeared"*;
    /// the first half is `l2_kingdom::levy::muster_tile`, C47.
    ///
    /// **Two departures, both deliberate.** The original does this at
    /// `Game_NewGame` time and we do it the first time the campaign screen is
    /// built, because a screen is constructed from a [`ScreenId`] with no game
    /// in hand. And it centres on the *start* county from `g_playerStartTable`,
    /// which a loaded position does not carry; we centre on the selected county
    /// when it is the player's and otherwise on his lowest-numbered one, which
    /// is the same county on turn one.
    ///
    /// The guard is the original's too: `FUN_00432746` does nothing at all when
    /// the county's town tile is zero, so a position with no town — every
    /// synthetic map in the test suite — stays exactly where `Map_InitMode`
    /// left it.
    fn open_on_the_player(&mut self, ctx: &Ctx) {
        self.opened = true;
        let g = &ctx.game;
        let county = if g.is_players(g.selected) {
            g.selected
        } else {
            match g.kingdom.county_ids().into_iter().find(|&c| g.is_players(c as u8)) {
                Some(c) => c as u8,
                None => return,
            }
        };
        // `g_counties[c].townTile`, whose zero means "no town".
        let Some(&tile) = Self::town(ctx, county).first() else { return };
        let (x, y) = l2_kingdom::map::coords(tile);
        self.centre_on_tile(x as usize, y as usize);
        self.saved = self.view;
    }

    /// Which of a set of candidate tiles a pixel is on, if any.
    ///
    /// The original inverts the isometric projection (`Map_PickTile`) and gets
    /// the tile from anywhere on the map. **Ours** hit-tests the diamonds of
    /// the tiles that could mean something — the county's fields and its
    /// settlements, a few dozen — which reaches the same answer on those and no
    /// answer elsewhere. Honest about being less than the original's picker,
    /// and enough for the two things a map click does to a county.
    fn tile_at(&self, x: i32, y: i32, candidates: impl Iterator<Item = usize>) -> Option<usize> {
        if !self.map_clip().contains(x, y) {
            return None;
        }
        let (hw, hh) = (self.zoom.tile_w as f32 / 2.0, self.zoom.tile_h as f32 / 2.0);
        for tile in candidates {
            let (tx, ty) = l2_kingdom::map::coords(tile);
            let Some((cx, cy)) =
                campaign::tile_centre(self.view, &self.zoom, tx as usize, ty as usize)
            else {
                continue;
            };
            // The diamond, not its bounding box: |dx|/halfW + |dy|/halfH <= 1.
            let (dx, dy) = ((x - cx).abs() as f32, (y - cy).abs() as f32);
            if dx / hw + dy / hh <= 1.0 {
                return Some(tile);
            }
        }
        None
    }

    /// The county's tiles carrying one plane-0 bit.
    fn tiles_with(ctx: &Ctx, county: u8, bit: u8) -> Vec<usize> {
        let map = &ctx.game.kingdom.campaign.map;
        (0..map.terrain.len())
            .filter(|&t| map.county[t] == county && map.flags[t] & bit != 0)
            .collect()
    }

    /// The county's settlement tiles — its four industry sites and its castle
    /// block. `Map_Click`'s own test: plane-0 bit `0x80`.
    fn settlements(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::tiles_with(ctx, county, l2_kingdom::map::flags::SETTLEMENT)
    }

    /// [`MapScreen::settlements`], for the tests that need to find a county's
    /// mine on the map without duplicating the flag test.
    pub fn settlements_for_test(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::settlements(ctx, county)
    }

    /// **Every industry site's tile and the frame its wheel is showing**, so a
    /// test can watch the wheel turn without reaching into private state or
    /// re-deriving the site list beside the code that derives it.
    ///
    /// `(tile, county, commodity, frame)`, in [`MapScreen::rebuild_industry_sites`]'s
    /// own order. The list is empty until the first
    /// [`MapScreen::step_industry`], which is the first `update`.
    pub fn industry_sites_for_test(&self) -> Vec<(usize, u8, usize, u8)> {
        self.industry_sites.iter().map(|s| (s.tile, s.county, s.commodity, s.frame)).collect()
    }

    /// **Which settlement tile a pixel is on — the ground first, then the
    /// building standing on it.**
    ///
    /// The diamond alone is not enough, and this is the second half of a defect
    /// a player reported as *"I can't click the iron mine on the world map"*.
    /// `Town1a.pl8` frame 30, the mine, is **58 × 47** against a 58 × 30 tile,
    /// so seventeen rows of headframe are drawn *above* the tile's diamond and
    /// a further band of it falls inside the diamond's bounding box but outside
    /// the rhombus. Swept pixel by pixel, 1,314 of the mine's pixels are
    /// painted and only 857 of them were on the tile: **the whole upper half of
    /// the building — the part anybody would aim at — was dead.** The forest is
    /// worse, at 22 rows of overhang.
    ///
    /// **This is a deliberate departure from the original and the only one on
    /// this path.** `Map_PickTile` (`0x00429ba4`) is pure geometry — it divides
    /// by the pitch and resolves the diamond with a parity test, and never
    /// looks at a pixel — so in the original the top of the mine belongs to the
    /// tile behind it, where `Map_Click` finds no flags and does nothing. Ours
    /// answers instead of doing nothing. It can only *add* hits, never move
    /// one: the diamond is tried first and wins, and the fallback tests the
    /// frame's own opaque mask, so it fires only on pixels where that building
    /// is actually painted.
    ///
    /// It reads the map file rather than [`MapScreen::town_graphics`] because
    /// the overrides plane holds towns — plane-0 bit `0x40` — and a settlement
    /// is bit `0x80`; the two sets are disjoint.
    fn settlement_at(&self, ctx: &Ctx, county: u8, x: i32, y: i32) -> Option<usize> {
        let tiles = Self::settlements(ctx, county);
        if let Some(tile) = self.tile_at(x, y, tiles.iter().copied()) {
            return Some(tile);
        }
        if !self.map_clip().contains(x, y) {
            return None;
        }
        let slot = ctx.assets.slot(ctx.game.map_slot)?;
        for tile in tiles {
            let (tx, ty) = l2_kingdom::map::coords(tile);
            let (row, col) = campaign::tile_to_cell(tx as usize, ty as usize);
            let (sx, sy) = campaign::cell_to_screen(self.view, &self.zoom, row, col);
            let bank_byte = slot.at(Plane::GfxBank, tx as usize, ty as usize);
            let frame = slot.at(Plane::GfxIndex, tx as usize, ty as usize) as usize;
            let bank = ((bank_byte & campaign::BANK_MASK) >> 2) as usize;
            let season = ctx.game.kingdom.season;
            let Some(sheet) = ctx.assets.map.bank(&self.zoom, season, bank) else { continue };
            let Some(decoded) = sheet.frame(frame) else { continue };
            let overhang = (decoded.height as i32 - self.zoom.tile_h).max(0);
            let (dx, dy) = (x - sx, y - (sy - overhang));
            if dx < 0 || dy < 0 || dx >= decoded.width as i32 || dy >= decoded.height as i32 {
                continue;
            }
            if decoded.opaque[dy as usize * decoded.width as usize + dx as usize] {
                return Some(tile);
            }
        }
        None
    }

    /// The county's **town**: the 2 × 2 block on plane-0 bit `0x40`.
    ///
    /// The constant is still spelled `CASTLE` in `l2-kingdom` and its own doc
    /// comment explains why (`docs/decisions.md` C25); the bit is the town.
    pub fn town(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::tiles_with(ctx, county, l2_kingdom::map::flags::CASTLE)
    }

    /// **One quadrant of the town block, named by the number `Sprite_TopIt`
    /// tests** — `tile.part & 0xf`, plane 3, which `l2-formats` calls
    /// [`l2_formats::maps::Plane::ObjectPart`] and documents as `dx + W * dy`
    /// from the block's north-west tile.
    ///
    /// **This exists because `town()[n]` and `part == n` are not the same
    /// thing**, and a `town.get(1)` that meant `part == 2` is what a player saw
    /// as *"I haven't seen any mercenary icons on the town square yet."* For a
    /// 2 × 2 block with `W = 2`:
    ///
    /// | `part` | offset from the origin | index order |
    /// |---:|---|---:|
    /// | 0 | `(x, y)` | 0 |
    /// | 1 | `(x + 1, y)` | 1 |
    /// | 2 | `(x, y + 1)` | **2** |
    /// | 3 | `(x + 1, y + 1)` | 3 |
    ///
    /// so `part == 2` is the **third** tile in index order, one map row south of
    /// the origin — not the second. Three independent sources agree, and the
    /// third is the one that settles it:
    ///
    /// * the `dx + W * dy` rule, `[V]` 10,971/10,971 in `l2-formats`;
    /// * every town block of all 44 shipped maps, swept: `part` runs 0, 1, 2, 3
    ///   over `(x, y)`, `(x+1, y)`, `(x, y+1)`, `(x+1, y+1)`;
    /// * **`County_FindTownTile` (`0x00467FD1`), which never reads `part` at
    ///   all.** It sweeps the grid in index order — `for y { for x { … } }`, the
    ///   same order [`MapScreen::tiles_with`] produces — counting the county's
    ///   `flags & 0x40` tiles, and sets **bank bit `0x80` on the 0th and the
    ///   2nd**. Bank `0x80` is the *only* gate on `Sprite_TopIt` being called at
    ///   all (`FUN_00405EB5`: `if (tile.bank & 0x80) Sprite_TopIt(…)`), so the
    ///   original does not even *visit* the tile we were drawing on.
    ///
    /// `None` when the county has no town block, or a block that is not four
    /// tiles — which no shipped map has, and which is a refusal rather than a
    /// guess.
    fn town_quadrant(ctx: &Ctx, county: u8, part: usize) -> Option<usize> {
        let town = Self::town(ctx, county);
        if town.len() != 4 || part > 3 {
            return None;
        }
        let origin = *town.first()?;
        let tile = origin + part % 2 + (part / 2) * l2_kingdom::map::MAP_DIM;
        // The arithmetic has to land back inside the block it came from; a town
        // that straddles the right edge of the 64-wide grid would wrap.
        town.contains(&tile).then_some(tile)
    }

    pub fn zoom(&self) -> &Zoom {
        &self.zoom
    }

    pub fn viewport(&self) -> Viewport {
        self.view
    }

    /// The rectangle the map is actually drawn in at the current zoom.
    pub fn map_clip(&self) -> Clip {
        self.zoom.clip()
    }

    /// Paint the tiles and stamp the county ids.
    ///
    /// Called from `draw` *and* from `handle`, because a click can arrive
    /// before a frame has been drawn — in a test it always does — and a pick
    /// plane that only exists after the first repaint is a pick plane that
    /// works everywhere except in the tests.
    fn ensure(&mut self, ctx: &Ctx) {
        if !self.opened {
            self.open_on_the_player(ctx);
        }
        // The turn count is in the key because [`town_graphics`] depends on
        // every county's population, which the end of a turn moves. The season
        // is in it because it repoints all five tile banks, and the field
        // digest because a brush stroke repaints one tile without ending a
        // turn — see [`MapScreen::field_graphics`].
        //
        // **And the castles are in it too**, because ordering one changes the
        // picture on the map in the middle of a turn and a cache keyed on the
        // turn alone would show the bare plot until the next one. Three of the
        // seven components were added by three different agents inside a day;
        // each is a thing that repaints the map without ending a turn.
        let key = (
            ctx.game.map_slot,
            self.zoom.id,
            self.view,
            ctx.game.kingdom.turn_count,
            ctx.game.kingdom.season,
            Self::field_digest(ctx),
            Self::castle_key(ctx),
            // **And the industry wheels**, which move without a turn ending and
            // without anything being clicked — the eighth component, and the
            // first one a *clock* writes. Without it a mine that stepped its
            // frame would keep the cached picture until something else
            // invalidated it, which is precisely the shape of *"industry map
            // things are still not animated when active."*
            self.industry_key(),
            // **And the fog**, which a march lifts without a turn ending and
            // the options page drops or raises in the middle of one. The
            // seventh painter-side input, and the only one that is a viewer's
            // rather than the world's.
            Self::fog_key(ctx),
        );
        if self.built == Some(key) {
            return;
        }
        // The seasonal art changes in the dark. See
        // [`MapScreen::holding_art_for_the_dark`].
        if self.holding_art_for_the_dark() {
            return;
        }
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else {
            return;
        };
        let lattice = Lattice::build(&slot);
        // The towns, the fields, the castles — and now the four industry
        // buildings per county, whose frame is the animation. `Overrides` was
        // written for the first three and never carried the fourth, which is
        // the whole of *"industry map things are still not animated"*: the
        // feature was enumerated and nothing drove it.
        let mut overrides = Self::tile_graphics(ctx);
        self.add_industry_graphics(&mut overrides);
        self.base.clear(ctx.assets.ink.background);
        self.tags.clear();
        // `Map_DrawTile`'s and the row walkers' `g_optExploration == 1` test,
        // with the viewer's seen bits behind it. `campaign::draw` has the
        // arithmetic; `Game::hides_tile` has the test.
        let hidden = |x: usize, y: usize| {
            ctx.game.hides_tile(l2_kingdom::map::index(x as u8, y as u8))
        };
        let fog: campaign::Fog =
            if ctx.game.kingdom.options.exploration { Some(&hidden) } else { None };
        campaign::draw(
            &mut self.base,
            &slot,
            &lattice,
            &ctx.assets.map,
            self.view,
            &self.zoom,
            &mut self.tags,
            &overrides,
            ctx.game.kingdom.season,
            fog,
        );
        self.built = Some(key);
    }

    /// The fog, folded for the cache key: zero with the option off, and
    /// otherwise a fold of the viewer's seen bits over the 4,096 tiles in index
    /// order — `docs/netcode.md` §3's rule even though this never leaves the
    /// screen, because a hash of an unordered walk is how a cache comes to
    /// disagree with itself.
    fn fog_key(ctx: &Ctx) -> u64 {
        if !ctx.game.kingdom.options.exploration {
            return 0;
        }
        let mut n: u64 = 1;
        for tile in 0..l2_kingdom::MAP_TILES {
            n = n.wrapping_mul(0x100_0000_01B3).wrapping_add(ctx.game.hides_tile(tile) as u64 + 1);
        }
        n
    }

    /// Everything the game rewrites over the map file: the towns, and the
    /// fields.
    ///
    /// Two passes over one plane rather than two planes, because they are
    /// disjoint by construction — a town tile carries plane-0 bit `0x40` and a
    /// field carries `0x20`, and `maps-layers.md` §2's census has no tile with
    /// both.
    pub fn tile_graphics(ctx: &Ctx) -> campaign::Overrides {
        let mut out = Self::town_graphics(ctx);
        Self::add_field_graphics(ctx, &mut out);
        out
    }

    /// **Give every farm tile the picture its crop state calls for.**
    ///
    /// `Terrain_Set` (`0x0046D7F4`) is the game's single writer of a tile's
    /// `content` byte and it picks the graphic at the same moment; the frame it
    /// writes is a pure function of the new terrain and the two low bits of
    /// whatever frame the tile already had, so it can be recomputed from the
    /// file rather than tracked. [`l2_view::campaign::field_graphic`] is that
    /// function and carries the derivation.
    ///
    /// Until this existed the field brush painted markers of our own and the
    /// map showed the same ploughed field in March and in August, whatever the
    /// county's crops were doing. `maps-layers.md` §5.5 read the mapping and
    /// said so: *"Not yet drawn."*
    fn add_field_graphics(ctx: &Ctx, out: &mut campaign::Overrides) {
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else { return };
        let map = &ctx.game.kingdom.campaign.map;
        for tile in 0..map.terrain.len() {
            if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
                continue;
            }
            let (x, y) = l2_kingdom::map::coords(tile);
            let stored = slot.at(Plane::GfxIndex, x as usize, y as usize);
            let (bank, frame) = campaign::field_graphic(map.terrain[tile], stored);
            out.set(x as usize, y as usize, bank, frame);
        }
    }

    /// A cheap summary of every farm tile's crop state, for the repaint key.
    ///
    /// **Order-dependent and deterministic**, which is all it has to be: it
    /// never leaves this screen, is never saved and is not the lockstep digest.
    /// It exists so that a single brush stroke repaints the map — the turn
    /// counter does not move when a player ploughs one field, and without this
    /// the new picture would not appear until the season turned.
    fn field_digest(ctx: &Ctx) -> u64 {
        let map = &ctx.game.kingdom.campaign.map;
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for tile in 0..map.terrain.len() {
            if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
                continue;
            }
            h ^= u64::from(map.terrain[tile]) ^ (tile as u64);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// **Put the towns back.** `Counties_PlaceSites` (`0x00468D4F`) rewrites
    /// every county's 2 × 2 town block at load, and the population pass rewrites
    /// it again every season; the bytes `L2_maps.dat` holds for those tiles are
    /// a placeholder the original never draws.
    ///
    /// Drawing the placeholder is what put **four quarries where a player's
    /// town should be**, and it is not a coincidence that they looked like
    /// quarries: `Town1a.pl8` frame 0 *is* the stone quarry, which is how
    /// `County_PlaceResourceSites` identifies one (frame 0 stone, 20 wood, 30
    /// iron). The stored frames 0 … 3 are four of them.
    ///
    /// `FUN_0046ac22` stamps `frame = base + quadTable[part]`, and the stored
    /// frame already *is* `quadTable[part]` — 0, 2, 1, 3 for the north-west,
    /// north-east, south-west and south-east tiles — so the rewrite is the base
    /// added to what the file holds, and no quadrant table is needed here.
    pub fn town_graphics(ctx: &Ctx) -> campaign::Overrides {
        let mut out = campaign::Overrides::new();
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else { return out };
        let k = &ctx.game.kingdom;
        for id in k.county_ids() {
            let pop = k.counties[id].population;
            let base = TOWN_FRAME_BASE
                .iter()
                .find(|(limit, _)| pop < *limit)
                .map_or(55, |(_, base)| *base);
            for tile in Self::town(ctx, id as u8) {
                let (x, y) = l2_kingdom::map::coords(tile);
                let stored = slot.at(Plane::GfxIndex, x as usize, y as usize);
                out.set(x as usize, y as usize, TOWN_BANK, base + stored);
            }
            Self::castle_graphics(ctx, id as u8, &mut out);
        }
        out
    }

    /// Stamp each site's current frame over the map file's.
    ///
    /// Every site gets an override, not only the working ones: an idle mine's
    /// picture is *also* wrong from the file the moment
    /// `Industry_UpdateSiteTile` has ever run, and a wrecked one is a different
    /// frame entirely. The bank is [`TOWN_BANK`] because the mine, the quarry,
    /// the forest and the smithy live in `Town1a.pl8` beside the town.
    fn add_industry_graphics(&self, out: &mut campaign::Overrides) {
        for site in &self.industry_sites {
            let (x, y) = l2_kingdom::map::coords(site.tile);
            out.set(x as usize, y as usize, TOWN_BANK, site.frame);
        }
    }

    /// **Find every industry building once**, when the map slot changes.
    ///
    /// The original does not look: `County_PlaceResourceSites` stores each
    /// site's tile on the record at load (`Industry.siteTile`, county `+0x298 +
    /// c*0x18`) and every reader indexes it. We derive it from the settlement
    /// bit and the terrain ladder instead —
    /// [`l2_kingdom::map::industry_site`] carries the argument for deriving
    /// rather than storing — and cache the answer here, because the *tile* is
    /// what never changes while the terrain on it does.
    fn rebuild_industry_sites(&mut self, ctx: &Ctx) {
        if self.industry_slot == Some(ctx.game.map_slot) {
            return;
        }
        self.industry_slot = Some(ctx.game.map_slot);
        self.industry_sites.clear();
        let k = &ctx.game.kingdom;
        for id in k.county_ids() {
            for c in l2_kingdom::tables::Commodity::ALL {
                let Some(tile) = l2_kingdom::map::industry_site(&k.campaign.map, id as u8, c)
                else {
                    continue;
                };
                self.industry_sites.push(IndustrySite {
                    tile,
                    county: id as u8,
                    commodity: c.index(),
                    frame: Self::industry_rest_frame(ctx, tile, c.index()),
                });
            }
        }
    }

    /// The frame a site shows when its wheel is **not** turning: the wrecked
    /// picture if an army trampled it, and otherwise the idle one
    /// `Industry_UpdateSiteTile` writes whenever the season's output was not
    /// positive.
    fn industry_rest_frame(ctx: &Ctx, tile: usize, commodity: usize) -> u8 {
        let (idle, _, _, wrecked) = campaign::INDUSTRY_FRAMES[commodity.min(3)];
        let terrain = ctx.game.kingdom.campaign.map.terrain.get(tile).copied().unwrap_or(0);
        match l2_kingdom::map::industry_state(terrain) {
            Some((_, l2_kingdom::map::SiteState::Wrecked)) => wrecked,
            _ => idle,
        }
    }

    /// **`Sprite_TopIt` arm 5b, once per fixed tick — the wheel, and its rate.**
    ///
    /// A site whose terrain says *working* steps its own frame; one that is
    /// idle or wrecked is pinned to [`MapScreen::industry_rest_frame`]. There is
    /// **no overlay for either state**: the whole of "this mine is running" is
    /// that its picture moves, which is why a feature that was fully enumerated
    /// still looked like nothing was happening — `Overrides` was written for
    /// fields, towns and castles, so every site drew the frame `L2_maps.dat`
    /// stores, and that is the idle frame in all four cases.
    ///
    /// The rate is [`campaign::industry_period_ms`], banded from the season's
    /// output. Converted here rather than there because the tick is this
    /// crate's: `main::TICK` is 16 ms, so 640 ms is 40 ticks and 80 ms is 5.
    ///
    /// **This is the only thing in the screen that a clock drives into a
    /// picture the base plane holds**, so it returns whether anything moved and
    /// the caller repaints on that rather than every frame.
    fn step_industry(&mut self, ctx: &Ctx) -> bool {
        self.rebuild_industry_sites(ctx);
        self.industry_tick = self.industry_tick.wrapping_add(1);
        let k = &ctx.game.kingdom;
        let mut moved = false;
        for site in &mut self.industry_sites {
            let terrain = k.campaign.map.terrain.get(site.tile).copied().unwrap_or(0);
            let working = matches!(
                l2_kingdom::map::industry_state(terrain),
                Some((_, l2_kingdom::map::SiteState::Working))
            );
            if !working {
                let (idle, _, _, wrecked) = campaign::INDUSTRY_FRAMES[site.commodity];
                let rest = match l2_kingdom::map::industry_state(terrain) {
                    Some((_, l2_kingdom::map::SiteState::Wrecked)) => wrecked,
                    _ => idle,
                };
                if site.frame != rest {
                    site.frame = rest;
                    moved = true;
                }
                continue;
            }
            // **Arm 5b is inside `Sprite_TopIt`, and so behind its fog test**: a
            // mine in the dark does not turn. The rest-frame pin above is
            // `Industry_UpdateSiteTile`'s and is not gated.
            if ctx.game.hides_tile(site.tile) {
                continue;
            }
            // `total - totalSnapshot`, which this crate keeps as
            // `Industry::output` and computes at exactly the same moment.
            let output = k
                .counties
                .get(site.county as usize)
                .map_or(0, |c| c.industry[site.commodity].output);
            let every = (campaign::industry_period_ms(output) / crate::TICK_MS).max(1);
            if self.industry_tick % every != 0 {
                continue;
            }
            site.frame = campaign::industry_step(site.commodity, site.frame);
            moved = true;
        }
        moved
    }

    /// The site frames, folded so the base plane's cache notices a wheel that
    /// turned. A fold over the list in build order, not a hash of anything
    /// unordered — `docs/netcode.md` §3, and it never leaves this screen.
    fn industry_key(&self) -> u64 {
        let mut n: u64 = 0;
        for site in &self.industry_sites {
            n = n.wrapping_mul(0x100_0001).wrapping_add(site.frame as u64);
        }
        n
    }

    /// Every county's castle state, folded into one number, so that the painted
    /// map is rebuilt the moment a castle is ordered or a season of work moves
    /// its picture on. Not a hash of anything iterated in an unordered way —
    /// see `docs/netcode.md` — it is a fold over `county_ids` in order.
    fn castle_key(ctx: &Ctx) -> u64 {
        let k = &ctx.game.kingdom;
        let mut n: u64 = 0;
        for id in k.county_ids() {
            let c = &k.counties[id];
            n = n
                .wrapping_mul(0x100_0001)
                .wrapping_add(c.castle_type as u64)
                .wrapping_mul(0x101)
                .wrapping_add(c.castle_degraded as u64)
                .wrapping_mul(0x101)
                .wrapping_add(c.castle_percent as u64);
        }
        n
    }

    /// **Put the castle there at all.** `Castle_StampTile` (`0x0046826C`), the
    /// half of it that is artwork.
    ///
    /// A county's castle is **not in `L2_maps.dat`**. Unlike the mine, the
    /// quarry and the forest — which the file stores as real pictures and
    /// `County_PlaceResourceSites` merely flags — the castle plot is plain
    /// ground in the base bank, and every castle you have ever seen on the
    /// original's campaign map was stamped in at run time. Ours drew the plain
    /// ground, so **no county's castle was on the map**.
    ///
    /// The frame is chosen from the *castle's state*, which is why this lives
    /// with the map screen's per-turn cache rather than in a load-time pass:
    /// three appearances per level, twenty frames apart, and a castle going up
    /// changes picture twice on its way.
    ///
    /// Bank `0x10` is `(0x10 & 0x1C) >> 2 == 4`, `Castle1a.pl8` / `Castle2a.pl8`
    /// — and the `a` there is the **season**, swapped whole by
    /// `Gfx_LoadCountyMode` with the frame indices unchanged, so these numbers
    /// are season-independent. The install ships `Castle1a … Castle1d` and
    /// `Castle2a … Castle2d`, the same four-suffix shape as `Base`, `Mtns`,
    /// `Roads` and `Town`. `[V]`
    fn castle_graphics(ctx: &Ctx, county: u8, out: &mut campaign::Overrides) {
        let c = &ctx.game.kingdom.counties[county as usize];
        let Some(stamp) =
            l2_kingdom::map::castle_stamp(c.castle_type, c.castle_degraded, c.castle_percent)
        else {
            return;
        };
        // The block's own order is index order over the 2×2, which is what
        // `Map_StampBlock` walks: north-west, north-east, south-west,
        // south-east. `castle_tiles` returns them in tile-index order, which is
        // the same walk.
        for (quadrant, tile) in
            l2_kingdom::map::castle_tiles(&ctx.game.kingdom.campaign.map, county)
                .into_iter()
                .take(4)
                .enumerate()
        {
            let (x, y) = l2_kingdom::map::coords(tile);
            out.set(x as usize, y as usize, stamp.bank, stamp.frames[quadrant]);
        }
    }

    fn ensure_minimap(&mut self, ctx: &Ctx) {
        if self.minimap_slot == Some(ctx.game.map_slot) {
            return;
        }
        self.minimap = ctx.assets.minimap(ctx.game.map_slot);
        self.minimap_slot = Some(ctx.game.map_slot);
    }

    /// **`Map_PickTile` — which map tile a pixel is in.**
    ///
    /// The original inverts its own projection; we test the pixel against every
    /// tile's diamond instead, which is 4,096 integer comparisons on a click and
    /// exact by construction: a lattice cell is a `tile_w × tile_h` rhombus
    /// centred on [`campaign::tile_centre`], the diamonds tile the plane without
    /// gaps, and `|dx| / hw + |dy| / hh <= 1` — multiplied out to stay in
    /// integers — is inside it. Ties on a shared edge go to the lower tile
    /// index, which makes the answer reproducible; `docs/netcode.md` §3.
    ///
    /// It is *not* the same algorithm as the original's and it does not have to
    /// be: nothing in the simulation depends on how a pixel became a tile, only
    /// on which tile the order named.
    pub fn pick_tile(&self, x: i32, y: i32) -> Option<(u8, u8)> {
        if !self.map_clip().contains(x, y) {
            return None;
        }
        // **The half-extents are the lattice's, not the picture's.**
        // `Map_PickTile` (`0x00429BA4`) *"divides by `g_mapTileHalfStep` and
        // `g_mapRowStep`"*, which are the half **pitch** and the row step — 30
        // and 15 near, 6 and 3 far. These were `tile_w / 2` and `tile_h / 2`,
        // which are 29 and 15: the near tile is 58 wide but the lattice pitch
        // is 60, so the diamonds were two pixels narrow and **did not tile the
        // plane** — 56 dead pixels around every tile centre.
        //
        // A `None` from here is not a refusal. The click falls through to
        // county selection, so a march order aimed at one of those pixels
        // quietly reselected a county instead of ordering anything.
        // `the_diamonds_leave_no_pixel_unpicked` is the assertion.
        let (hw, hh) = (self.zoom.half_pitch, self.zoom.row_step);
        let dim = l2_kingdom::MAP_DIM;
        for ty in 0..dim {
            for tx in 0..dim {
                let Some((cx, cy)) = campaign::tile_centre(self.view, &self.zoom, tx, ty) else {
                    continue;
                };
                if (x - cx).abs() * hh + (y - cy).abs() * hw <= hw * hh {
                    return Some((tx as u8, ty as u8));
                }
            }
        }
        None
    }

    /// **`g_pickedTileUnit` — the unit standing on the tile a pixel is in.**
    ///
    /// `Map_ResolvePick` (`0x0046D5FE`) reads it out of the tile record —
    /// `g_pickedTileUnit = g_tiles[t].unit` — so in the original a click
    /// **anywhere on a unit's tile** is that unit. This asked the unit's little
    /// *marker* instead, a box `unit_marker_half + 1` around the tile centre,
    /// which at near zoom is nine pixels across on a diamond that is 58 × 30.
    ///
    /// That is what a player reported as *"if I click a merchant while the map
    /// has a different county selected it will open up the tax window"*: the
    /// click missed the box, fell past the unit arm and past the settlement,
    /// town and field arms, and landed on our own "a second click on the
    /// selected county opens it". Same shape as the mine (`docs/decisions.md`
    /// C57) and the same cause — a hit test smaller than the thing drawn. C58.
    ///
    /// So: **the tile first, which is the original's whole answer**, and then
    /// the drawn figure, because `Map_DrawArmies` anchors a sprite on the
    /// tile's *bottom vertex* and it therefore stands up over the tiles behind
    /// it — pixels the original would resolve to a tile with no unit on it.
    /// Ours can add an answer there; it can never move one, because the tile
    /// wins whenever it has a unit.
    pub fn unit_at(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        if !self.map_clip().contains(x, y) {
            return None;
        }
        let units = &ctx.game.kingdom.campaign.units;
        if let Some((tx, ty)) = self.pick_tile(x, y) {
            // Ascending slot order, so two units sharing a tile — which the
            // original's single `tile.unit` byte cannot even express — resolve
            // the same way twice. `docs/netcode.md` §3.
            if let Some(id) = units.iter().find(|(_, u)| u.x == tx && u.y == ty).map(|(id, _)| id) {
                return Some(id);
            }
        }
        units.iter().find_map(|(id, u)| {
            if u.is_garrisoned() {
                // Drawn as a hollow marker, not a figure — there is no sprite
                // to hit-test, so the marker box is the whole of it.
                let (cx, cy) = campaign::tile_centre(self.view, &self.zoom, u.x as usize, u.y as usize)?;
                let r = unit_marker_half(&self.zoom, u) + 1;
                return ((x - cx).abs() <= r && (y - cy).abs() <= r).then_some(id);
            }
            self.unit_sprite_covers(ctx, id, u, x, y).then_some(id)
        })
    }

    /// Whether a pixel lands on a unit's drawn figure, at
    /// [`campaign::draw_unit`]'s own placement and against the frame's own
    /// opacity mask. Falls back to the marker box when the sprite sheets are
    /// missing, which is the case in every test that runs without an install.
    ///
    /// **Where it is drawn, walk offset and all**, so an army part-way across a
    /// tile is hit on its figure rather than on the tile it has not reached.
    fn unit_sprite_covers(&self, ctx: &Ctx, id: usize, u: &l2_kingdom::Unit, x: i32, y: i32) -> bool {
        let Some((cx, cy)) =
            campaign::tile_centre(self.view, &self.zoom, u.x as usize, u.y as usize)
        else {
            return false;
        };
        let sprite = unit_sprite(&self.zoom, ctx.game, id, u);
        match campaign::unit_sprite_rect(&ctx.assets.map, self.view, &self.zoom, (u.x as usize, u.y as usize), sprite) {
            Some((ox, oy, decoded)) => {
                let (dx, dy) = (x - ox, y - oy);
                dx >= 0
                    && dy >= 0
                    && dx < decoded.width as i32
                    && dy < decoded.height as i32
                    && decoded.opaque[dy as usize * decoded.width as usize + dx as usize]
            }
            None => {
                let r = unit_marker_half(&self.zoom, u) + 1;
                (x - cx).abs() <= r && (y - cy).abs() <= r
            }
        }
    }

    /// The army the map is currently giving orders to, if it is still an army.
    pub fn selected_unit(&self) -> Option<usize> {
        self.selected_unit
    }

    /// **`Map_Click`'s army branch**, verbatim:
    ///
    /// ```c
    /// if (unit.owner == g_localPlayer) {
    ///     if (unit.besiegingCounty) Siege_ValidateLink(unit);
    ///     if (unit.besiegingCounty == 0) Panel_MoveButton();
    ///     else { g_siegeScreenUnit = unit; g_screenId = 0x1D; }
    /// }
    /// ```
    ///
    /// Three things worth stating because each is a decision the original made
    /// and a reimplementation would not:
    ///
    /// * **An enemy unit does nothing at all.** There is no `else`: clicking
    ///   another lord's army is a click that falls off the end of the branch.
    /// * **The validate runs first**, so a besieger whose target garrison has
    ///   gone gets its link cleared *by the click* and lands on the move branch
    ///   in the same call. `Siege_ValidateLink` is the whole of that.
    /// * **From the map a besieging army never sees the "Lift the siege?"
    ///   prompt.** `Panel_MoveButton` raises `L2.eng` 10/13 when its unit is
    ///   besieging; the map does not reach `Panel_MoveButton` in that case at
    ///   all, it opens the siege screen instead. Two routes to one decision, and
    ///   only one of them asks.
    /// **`Map_BeginMoveSelection` (`0x0043723A`)** — the map enters move-order
    /// mode, `g_screenId = 0x10`.
    ///
    /// The whole of what it does that we can do: `g_selectedUnit = unit`, then
    /// `Move_FloodFill` from the unit's own tile. It also sets
    /// `g_moveOrderClickGuard` (`0x00553ECC`) to 40 — forty frames in which a
    /// left button that is *down* is not read as the destination, so that the
    /// press which opened the mode cannot also close it. **We do not need it
    /// and do not have it**: the original polls the button's level once a frame,
    /// where we are handed one [`Event::Click`] per press, and the press that
    /// selected the army was consumed by this call. Recorded rather than
    /// silently dropped — it is an arm of the original, and the reason it is
    /// absent is a difference in our input model, not a judgement that it is
    /// unimportant.
    fn begin_move_selection(&mut self, ctx: &Ctx, unit: usize) {
        let Some(u) = ctx.game.kingdom.campaign.units.get(unit) else { return };
        let start = u.tile();
        let cost = ctx.game.kingdom.campaign.map.cost_map();
        let field =
            l2_kingdom::movement::flood_fill(&cost, start, l2_kingdom::movement::Routing::Direct);
        self.selected_unit = Some(unit);
        self.move_order =
            Some(MoveOrder { unit, cost, field, hovered: None, path: Vec::new() });
    }

    /// Leaving move-order mode — `g_screenId = 0`.
    ///
    /// Three things do it in the original and all three are just that
    /// assignment: the right button released (`Screen_FrameInput`'s `0x10`
    /// arm), `Map_ConfirmMoveOrder` having placed the order, and the turn
    /// ceasing to be the local player's.
    fn cancel_move_selection(&mut self) {
        self.selected_unit = None;
        self.move_order = None;
    }

    /// **`Map_HoverUnitTarget` (`0x004A8E0B`) — the arm this project missed,
    /// and the one a player noticed first.**
    ///
    /// In the original the march route appears **while the pointer moves**, not
    /// after the click: you sweep the cursor over the map and the gold balls
    /// follow it, so you can see the route and its cost *before* committing.
    /// Ours drew the same artwork — `Flags1a.pl8` frames `0x38 … 0x4E`, read
    /// carefully and correctly — from the unit's **already ordered** path, which
    /// is a picture the original never shows: `Path_MarkPreviewTiles` is the
    /// only writer of tile bank bit `0x40` in the whole binary, it runs only
    /// from here, and here runs only on screen `0x10`. So the balls exist in
    /// move-order mode and nowhere else, and they show the *hovered* route.
    ///
    /// The sprite sheet was read and the behaviour was not. `docs/decisions.md`
    /// C61.
    ///
    /// **It had no marker and no record until the gesture-kind audit.** The arm
    /// was built by C61's branch and then counted by nothing: `arms.rs` checks
    /// that every record has a marker and every marker has a record, and an arm
    /// with neither is invisible to both directions of that check. It is a
    /// `hover` — the original runs it from `Screen_DrawWidgets`' `0x10` arm once
    /// a frame, where every other screen draws its widget table — so it has no
    /// kind byte and the exe-gated check cannot classify it either.
    ///
    /// // arm: 0x004A8E0B/hover-march-target hover
    ///
    /// Two economies of the original are kept because they are behaviour, not
    /// speed: the descent runs **only when the hovered tile changed**
    /// (`if (DAT_005691E0 != g_hoverTileOffset)`), and the flood fill is not
    /// re-run at all — it was done once when the army was picked, from where the
    /// army *was*.
    // arm: 0x004A8E0B/hover-unit-target hover
    fn update_hover_path(&mut self, x: i32, y: i32) {
        if self.move_order.is_none() {
            return;
        }
        // `g_hoverTileOffset >= 0x0FFF0000` — the pointer is not over a tile —
        // clears `g_moveOrderAvailable` and draws nothing.
        let tile = if self.map_clip().contains(x, y) { self.pick_tile(x, y) } else { None };
        let sel = self.move_order.as_mut().expect("checked directly above");
        if sel.hovered == tile {
            return;
        }
        sel.hovered = tile;
        sel.path = tile
            .and_then(|dest| {
                l2_kingdom::movement::extract_path(&sel.cost, &sel.field, dest)
            })
            .unwrap_or_default();
    }

    fn click_unit(&mut self, ctx: &mut Ctx, unit: usize) -> Transition {
        // **The merchant arm, and its guard is the county's owner rather than
        // the merchant's.** `Map_Click` tests `kind != 1` first, then `kind !=
        // 3`, and the merchant branch is
        //
        // ```c
        // else if (g_counties[g_pickedTileCounty].owner == g_localPlayer) {
        //     DAT_00553C64 = g_pickedTileUnit;                  /* the trading unit */
        //     if (g_counties[g_pickedTileCounty].townTile != 0) {
        //         g_selectedCounty = g_pickedTileCounty;
        //         Map_CentreOnTile(g_counties[...].townTile);
        //         g_screenId = 8;
        //     }
        // } else Msg_Enqueue(..., 0x70, ...);
        // ```
        //
        // A unit's owner byte is read **exactly once** in the whole 1,263-byte
        // function, on the `kind == 1` path, and never here. That matters,
        // because **every merchant in the game carries owner 6** — `ownerless`.
        // `Merchant_SpawnAll` passes 6 to `Unit_Spawn` unconditionally, nothing
        // rewrites it, and all six of the England fixture's merchants have it.
        // So a guard on the *merchant's* owner could never fire, and "your
        // merchant" — which `docs/screens.md` §6 and `docs/symbols.md` both said
        // — is not a thing that exists. A merchant belongs to nobody and is
        // clickable while it stands in a county you own. C50.
        if ctx.game.kingdom.campaign.units.get(unit).map(|u| u.kind)
            == Some(l2_kingdom::UnitKind::Merchant)
        {
            return self.click_merchant(ctx, unit);
        }
        if !ctx.game.is_players_unit(unit) {
            self.status = "NOT YOUR UNIT".into();
            return Transition::Stay;
        }
        let besieging = {
            let l2_kingdom::Kingdom { counties, campaign, .. } = &mut ctx.game.kingdom;
            if campaign.units.get(unit).is_some_and(|u| u.besieging_county != 0) {
                l2_kingdom::siege::validate_link(counties, &mut campaign.units, unit);
            }
            ctx.game.kingdom.campaign.units.get(unit).is_some_and(|u| u.besieging_county != 0)
        };
        // arm: 0x0043CE1A/siege-preparation left-release
        if besieging {
            self.cancel_move_selection();
            return Transition::Push(ScreenId::Siege(unit));
        }
        // `Panel_MoveButton` -> `Map_BeginMoveSelection`: the map stays up and
        // the next click is the order.
        // **Clicking the same army again used to cancel the selection. That was
        // ours and it is gone.** In the original `Map_Click` is not reachable at
        // all while move-order mode is up — `Screen_FrameInput` dispatches on
        // `g_screenId`, and `0x10` is not `0` — so a second click on the army
        // is `Map_ConfirmMoveOrder` aimed at the tile the army is standing on,
        // and `Map_HoverUnitTarget` has already cleared `g_moveOrderAvailable`
        // for that tile because the fill's distance there is its own start. It
        // does nothing. **The cancel is the right button**, and the reason this
        // convenience existed is that we had never looked for it.
        // arm: 0x0043CE1A/unit-orders left-release
        self.begin_move_selection(ctx, unit);
        let (men, left) = ctx
            .game
            .kingdom
            .campaign
            .units
            .get(unit)
            .map_or((0, 0), |u| (u.men, u.moves_left()));
        self.status = format!("{men} MEN, {left} MOVES - CLICK A TILE TO MARCH");
        Transition::Stay
    }

    /// **`Map_Click`'s merchant arm**, and where it stops.
    ///
    /// The guard, the centre-on-the-town and the `townTile != 0` refusal are
    /// the original's (quoted in [`MapScreen::click_unit`]).
    ///
    /// `DAT_00553C64`, which the original sets here, has **exactly one writer
    /// in the whole binary — this line** — and two readers, both in the
    /// merchant screen's price arithmetic. So the trading screen is reachable
    /// only by clicking a merchant on the map, and the unit id is carried into
    /// [`ScreenId::Merchant`] because the *price* depends on it: the markup is
    /// this merchant's own morale. See [`crate::screens::merchant`].
    ///
    /// // arm: 0x0043CE1A/merchant left-release
    fn click_merchant(&mut self, ctx: &mut Ctx, unit: usize) -> Transition {
        let county = ctx.game.kingdom.campaign.units.get(unit).map_or(0, |u| u.county);
        if !ctx.game.is_players(county) {
            // `Msg_Enqueue(…, 0x70, …)` — the same refusal the flag arms use
            // for somebody else's county.
            self.status = "THAT MERCHANT IS NOT IN ONE OF YOUR COUNTIES".into();
            return Transition::Stay;
        }
        let Some(&town) = Self::town(ctx, county).first() else {
            // `if (townTile != 0)`: no town, no trade, and no message either.
            self.status = "THAT COUNTY HAS NO TOWN TO TRADE IN".into();
            return Transition::Stay;
        };
        let (x, y) = l2_kingdom::map::coords(town);
        ctx.game.select(county);
        self.centre_on_tile(x as usize, y as usize);
        self.cancel_move_selection();
        // `DAT_00553C64 = g_pickedTileUnit` — and this is the call that carries
        // it, exactly as the paragraph above predicted it would have to.
        Transition::Push(ScreenId::Merchant(unit))
    }

    /// **`Map_ConfirmMoveOrder`** — the second click, the one that places the
    /// order.
    ///
    /// The original picks at most one confirmation out of the targets
    /// `Map_HoverUnitTarget` collected, in a fixed priority — slaughter
    /// villagers, destroy field, combine armies, garrison castle, besiege castle
    /// — and otherwise lets the order through. Those are `L2.eng` group 10
    /// indices 4, 10, 5, 7 and 8, and the confirm dialog is not built here; the
    /// order goes through and the consequence happens when the army arrives,
    /// which is where [`l2_kingdom::movement::try_enter`] already puts it.
    ///
    /// The refusal is the whole rule: `Unit_OrderMove` writes **nothing at all**
    /// when no path is extracted, so a refused order leaves the army exactly as
    /// it was — not half-ordered, not stopped. `docs/armies.md` §2.3.
    /// **`g_screenId = 0` and then `Map_ConfirmMoveOrder`** — the left press
    /// in move-order mode, in the order `Screen_FrameInput` does it:
    ///
    /// ```c
    /// if ((g_mouseLeftPressed != '\0') && (g_moveOrderClickGuard < 1)) {
    ///     g_screenId = 0; DAT_0056D64C = 1; Map_ConfirmMoveOrder(); }
    /// ```
    ///
    /// **The mode is left first, unconditionally, and whether an order results
    /// is a separate question.** `Map_ConfirmMoveOrder` opens with
    /// `if (g_moveOrderAvailable != 1) return;` — and `g_moveOrderAvailable` is
    /// [`MapScreen::update_hover_path`]'s output, cleared when the flood
    /// fill's raw distance at the tile is below 2. Raw 0 is *never reached*;
    /// raw 1 is the army's **own tile**, whose distance is
    /// [`START_DISTANCE`]. So a click on the army you just picked ends the
    /// selection and orders nothing — which is what we used to do by hand in
    /// `click_unit`, as a convenience, and can now stop doing.
    ///
    /// [`START_DISTANCE`]: l2_kingdom::movement::START_DISTANCE
    fn confirm_move_order(&mut self, ctx: &mut Ctx, unit: usize, dest: (u8, u8)) -> Transition {
        let available = self.move_order.as_ref().is_some_and(|sel| {
            sel.field.cost_to(dest.0, dest.1).is_some_and(|d| d > 0)
        });
        self.cancel_move_selection();
        if !available {
            // Not a refusal message in the original — it is a press that
            // reached a `return`. Ours says so, because a silent no-op on a
            // deliberate click is the shape three hit-test defects hid behind.
            self.status = "THE ARMY IS ALREADY THERE".into();
            return Transition::Stay;
        }
        self.order_march(ctx, unit, dest)
    }

    fn order_march(&mut self, ctx: &mut Ctx, unit: usize, dest: (u8, u8)) -> Transition {
        match ctx.game.order_unit_move(unit, dest) {
            // **A zero-length path is an accepted order, not a refusal.**
            // `Move_ExtractPath` returns success with nothing in the buffer
            // when the descent never reached the destination, so the order
            // stands, `moveState` becomes 2, and the army stays where it is.
            // Only a dead end in the descent returns 0. `docs/armies.md` §2.3 —
            // and saying so is the difference between a player thinking the
            // click missed and knowing the tile is out of reach.
            Some(0) => self.status = "THAT TILE CANNOT BE REACHED - THE ARMY STANDS".into(),
            Some(steps) => {
                let left = ctx
                    .game
                    .kingdom
                    .campaign
                    .units
                    .get(unit)
                    .map_or(0, |u| u.moves_left());
                self.status = format!(
                    "MARCHING TO {},{} - {steps} STEPS, {left} MOVES LEFT",
                    dest.0, dest.1
                );
            }
            None => self.status = "NO ROAD THAT WAY - NOTHING ORDERED".into(),
        }
        Transition::Stay
    }

    /// The county at a canvas pixel, or 0.
    pub fn county_at(&self, x: i32, y: i32) -> u8 {
        if !self.map_clip().contains(x, y) {
            return 0;
        }
        self.tags.at(x, y)
    }

    /// `Map_ToggleZoom`: the campaign screen has two zooms and this is the only
    /// way between them.
    fn toggle_zoom(&mut self, ctx: &mut Ctx) {
        if self.zoom.id == NEAR.id {
            self.saved = self.view;
            self.set_zoom(ctx, FAR);
            self.view = Viewport::new(0x0C, 0x0E).clamped(&FAR);
            self.status = "ZOOMED OUT".into();
        } else {
            self.set_zoom(ctx, NEAR);
            self.view = self.saved.clamped(&NEAR);
            self.status = "ZOOMED IN".into();
        }
    }

    /// **`Map_SetZoom` — the only place `self.zoom` is written after
    /// construction, and the projection into [`crate::game::Game::map_zoom_far`]
    /// rides on it.**
    ///
    /// `g_mapZoom` is a global in the original and is read from three arms that
    /// are not on this screen: `Map_EdgeScroll`'s far-zoom refusal, which is
    /// what decides whether the information panel closes on an edge hover, and
    /// the `if (g_mapZoom != 2)` at the head of `FUN_00438ACC` and
    /// `FUN_0043893C`. Our overlays cannot reach this screen, so the value has
    /// to be somewhere they can see, and a projection written at the one write
    /// site cannot drift from the thing it projects.
    fn set_zoom(&mut self, ctx: &mut Ctx, zoom: Zoom) {
        self.zoom = zoom;
        ctx.game.map_zoom_far = zoom.id == FAR.id;
    }

    /// **`Map_EdgeScroll` — the direction the pointer's position asks for.**
    ///
    /// The original reads the *desktop* cursor (`GetCursorPos` into
    /// `0x004E6594`/`0x004E6598`) and scrolls while it sits on the outermost
    /// pixel of a 640 × 480 screen. It ran full-screen, so "the edge of the
    /// screen" and "the edge of the window" were the same place.
    ///
    /// They are not the same place for us, and that is the whole of the bug a
    /// player reported as *"I can't scroll the map"*. The window is scaled by
    /// an integer factor and centred, so a window that is not an exact multiple
    /// of 640 × 480 has black borders — and a cursor pushed into the border is
    /// outside the picture. `main.rs` clamps every pointer position into the
    /// canvas ([`pixels::Pixels::clamp_pixel_pos`]), so a cursor anywhere in
    /// the border arrives here **as the edge pixel it is nearest**, and the
    /// gesture works at the edge of the *window* whatever the letterboxing is
    /// doing. Nothing here needs to know the window exists.
    ///
    /// `None` when the pointer is not at an edge, or has left the window.
    fn edge_direction(&self) -> Option<Dir> {
        if !self.pointer_in {
            return None;
        }
        let (x, y) = self.pointer;
        let west = x <= 0;
        let east = x >= CANVAS_W - 1;
        let north = y <= 0;
        let south = y >= CANVAS_H - 1;
        match (north, east, south, west) {
            (true, false, false, false) => Some(Dir::N),
            (true, true, false, false) => Some(Dir::NE),
            (false, true, false, false) => Some(Dir::E),
            (false, true, true, false) => Some(Dir::SE),
            (false, false, true, false) => Some(Dir::S),
            (false, false, true, true) => Some(Dir::SW),
            (false, false, false, true) => Some(Dir::W),
            (true, false, false, true) => Some(Dir::NW),
            _ => None,
        }
    }

    fn scroll(&mut self, dir: Dir) -> bool {
        match self.view.scrolled(dir, &self.zoom) {
            Some(v) => {
                self.view = v;
                self.opened = true;
                true
            }
            None => false,
        }
    }

    /// `Map_CentreOnTile`. Reached from a minimap click, from a click on a
    /// county town, and from `Field_SetType`'s caller — the original centres
    /// the map before it opens anything over it (`docs/decisions.md` C22).
    pub fn centre_on_tile(&mut self, x: usize, y: usize) {
        self.view = Viewport::centred_on_tile(x, y, &self.zoom);
        // Somebody has said where to look, so the opening centre
        // ([`MapScreen::open_on_the_player`]) must not override it on the first
        // paint. It is a *default* for a screen nobody has positioned, not a
        // thing that happens to every campaign screen once.
        self.opened = true;
    }

    fn centre_on_county(&mut self, anchor: (usize, usize)) {
        self.centre_on_tile(anchor.0, anchor.1);
    }

    /// **Ours.** Select the player's next unit in ascending slot order and
    /// centre the map on it, so an army the viewport is nowhere near can still
    /// be given an order. Ascending slot order rather than nearest-first,
    /// because a selection that depends on where the viewport happens to be is
    /// a selection two peers could disagree about.
    fn cycle_unit(&mut self, ctx: &mut Ctx) {
        let mine = ctx.game.player_units();
        if mine.is_empty() {
            self.cancel_move_selection();
            self.status = "YOU HAVE NOTHING ON THE MAP".into();
            return;
        }
        let next = match self.selected_unit {
            Some(cur) => mine.iter().copied().find(|&id| id > cur).unwrap_or(mine[0]),
            None => mine[0],
        };
        self.begin_move_selection(ctx, next);
        if let Some(u) = ctx.game.kingdom.campaign.units.get(next) {
            let (x, y, men, left, kind) = (u.x, u.y, u.men, u.moves_left(), u.kind);
            self.centre_on_tile(x as usize, y as usize);
            self.status = format!("{} #{next}: {men} MEN, {left} MOVES", kind.name().to_uppercase());
        }
    }

    /// **End the turn — or get as far as the first battle**, and leave for
    /// screen `0x1C` if the turn ended the game.
    ///
    /// `FUN_00476768` is the original's shape for the ending: dismissing the
    /// message that set `DAT_0053F0C4` calls `FUN_00497879` — which advances the
    /// campaign counter on a win — and sets `g_screenId = 0x1C`. There is no
    /// message window here yet, so the turn's own end is the dismissal.
    ///
    /// [`turn::begin_turn`] is the interactive door and may come back holding a
    /// question, which is `Push`ed as screen `0x12`. The turn stays suspended on
    /// the [`Game`](crate::game::Game) until the prompt and the result screen
    /// hand it back; [`Self::resume_turn`] is what picks it up again when they
    /// pop, and it is called from `update` because this screen is underneath
    /// them and gets its tick back the moment they are gone.
    fn end_turn(&mut self, ctx: &mut Ctx) -> Transition {
        if turn::turn_in_flight(ctx.game) || self.fading.is_some() {
            return Transition::Stay;
        }
        // **The gold is snapshotted here and not in `resume_turn`.** A turn
        // takes many frames now, and `resume_turn` runs on all of them; reading
        // the treasury there would compare the end of the turn against the
        // frame before it and report a change of nothing.
        self.gold_at_turn_start = ctx.game.gold();
        // The slider's drag has nowhere to land while the map is not taking
        // clicks. The unit *selection* is kept: an order placed and then
        // silently cancelled by the turn ending would be a surprise.
        self.slider_held = false;
        let step = turn::begin_turn(ctx.game);
        self.status = "ENDING THE TURN...".into();
        self.scrolled = true;
        let before = self.gold_at_turn_start;
        self.settle_turn(ctx, step, before)
    }

    /// Pick a suspended turn back up **and wind it on by one tick**. Called
    /// every tick; almost always a no-op, because almost always there is no
    /// turn in flight.
    ///
    /// This used to call [`turn::dismiss_report`], which ran the machine to the
    /// end unless a battle stopped it — so a turn with no battle in it was over
    /// before the next frame was drawn. [`turn::tick_turn`] is one
    /// `Turn_Tick` / `Units_Tick` pair, which is what the original's main loop
    /// calls once a frame, and it is what gives a turn its duration. See
    /// [`turn::TurnStep::Running`].
    fn resume_turn(&mut self, ctx: &mut Ctx) -> Transition {
        if !turn::turn_in_flight(ctx.game) {
            return Transition::Stay;
        }
        // A turn that is still asking is one whose screen has not been put up
        // yet, or has just been popped without answering; either way the prompt
        // is what has to come back.
        if turn::pending_question(ctx.game).is_some() {
            return Transition::Push(ScreenId::BattlePrompt);
        }
        if turn::pending_report(ctx.game).is_some() {
            return Transition::Push(ScreenId::BattleResult);
        }
        // **A tick of the turn is a repaint**, because the whole point of
        // spreading it over frames is that the units on it are seen to move.
        self.scrolled = true;
        let step = turn::tick_turn(ctx.game);
        let before = self.gold_at_turn_start;
        self.settle_turn(ctx, step, before)
    }

    /// What to do with whatever the turn machine came back with.
    fn settle_turn(&mut self, ctx: &mut Ctx, step: turn::TurnStep, before: i32) -> Transition {
        match step {
            turn::TurnStep::Ask(_) => return Transition::Push(ScreenId::BattlePrompt),
            turn::TurnStep::Report(_) => return Transition::Push(ScreenId::BattleResult),
            // One tick down. The next frame brings the next one.
            turn::TurnStep::Running => return Transition::Stay,
            turn::TurnStep::Stuck => {
                self.status = "THE TURN MACHINE DID NOT COME ROUND".into();
                return Transition::Stay;
            }
            turn::TurnStep::Done(outcome) => self.finish_turn(ctx, *outcome, before),
        }
    }

    fn finish_turn(
        &mut self,
        ctx: &mut Ctx,
        outcome: turn::TurnOutcome,
        before: i32,
    ) -> Transition {
        let change = ctx.game.gold() - before;
        let fought = outcome.battles.len();
        let status = format!(
            "{} {} {} - {} MSG{}",
            season_name(ctx.game.kingdom.season),
            ctx.game.kingdom.year,
            widget::signed(change),
            outcome.report.messages.len(),
            if fought == 0 { String::new() } else { format!(" - {fought} BATTLE") }
        );
        let over = outcome.outcome.is_over();
        if over {
            ctx.game.campaign.enter_conquest_screen();
        }
        // **`Turn_Tick`'s phase 7 fades the screen the instant `Season_Advance`
        // returns**, and this is that instant. `FUN_004B0CB4(_, 1, _)` down to
        // a quarter, the seasonal art reloaded in the dark, `FUN_0049A3E6`'s
        // `FUN_004B0CB4(_, 0, _)` back up. See [`l2_view::fade`] and
        // [`Fading`].
        //
        // The status line and the leave for screen `0x1C` both wait for the
        // light: the original's `g_screenId = 0x1C` is on the far side of the
        // fade too.
        self.fading = Some(Fading { phase: 0, status, over });
        self.scrolled = true;
        Transition::Stay
    }

    /// One frame of the end-of-turn fade. See [`Fading`].
    fn tick_fade(&mut self) -> Transition {
        let Some(mut f) = self.fading.take() else { return Transition::Stay };
        self.scrolled = true;
        f.phase += 1;
        // **`FUN_0049A3E6` — the rolling autosave, and this frame is where the
        // original writes it.** `[V]`:
        //
        // ```c
        // void FUN_0049a3e6(void) {
        //   if (g_screenId == '$') {                      /* 0x24, the bottom  */
        //     DAT_0057c968 = 1; Gfx_LoadCountyMode();     /* the season's art  */
        //     Screen_DrawCampaign(3); FUN_004b14ff(); Gfx_Present(1);
        //     DAT_0056d6a0 = 1; Gfx_MarkAllDirty();
        //     FUN_004b0cb4(0,0,0x5691f0);                 /* and back up       */
        //     Save_RotateAndWrite();
        //   }
        // }
        // ```
        //
        // So it is neither the button nor the far side of the light: it is the
        // one frame the screen is dark and the new season is already loaded, and
        // what it stores is therefore the **opening of the turn that just
        // began**. [`l2_view::fade::is_darkest`] already names this frame for
        // the art reload, which is the statement two lines above it.
        //
        // Raised, not performed — see [`crate::screen::Screen::take_autosave`].
        if l2_view::fade::is_darkest(f.phase) {
            self.autosave = true;
        }
        if f.phase < l2_view::fade::PHASES {
            self.fading = Some(f);
            return Transition::Stay;
        }
        self.status = f.status;
        if f.over {
            // `Replace`, not `Push`: the campaign map underneath is a map of a
            // game that is over, and the original leaves it — the OK button on
            // `0x1C` goes on to `Game_NewGame` or the front end, never back to
            // it.
            return Transition::Replace(ScreenId::Conquest);
        }
        Transition::Stay
    }

    /// Whether the base render is being held back until the fade bottoms out.
    ///
    /// **The seasonal art changes in the dark, and that is the job the fade is
    /// doing.** The original's second `FUN_004B0CB4` call site is
    /// `FUN_0049A3E6`, *after* reloading the seasonal art — so the reload
    /// happens between the two calls, inside the dark window. A player who has
    /// played it says the same thing from the other side: the fade *"hides the
    /// season change visuals just abruptly changing"*. Two unrelated sources
    /// meeting is what turns the note in [`l2_view::fade`] from inferred into
    /// confirmed.
    ///
    /// So a rebuild that falls due while the light is going down waits for the
    /// bottom. Without this the map pops to the new season one frame after the
    /// button and then fades a picture the player has already watched change,
    /// which is the abruptness the effect exists to hide.
    fn holding_art_for_the_dark(&self) -> bool {
        matches!(self.fading, Some(Fading { phase, .. }) if phase < l2_view::fade::STEPS)
    }
}

impl Default for MapScreen {
    fn default() -> Self {
        MapScreen::new()
    }
}

pub fn season_name(season: u8) -> &'static str {
    match l2_kingdom::tables::Season::from_index(season) {
        Some(s) => s.name(),
        None => "-",
    }
}

/// **The season as the game spells it** — `L2.eng` group 29, indexed by
/// `g_season` directly.
///
/// The group is five strings: `"No Season"`, `"Spring"`, `"Summer"`, `"Autumn"`,
/// `"Winter"`, and [`l2_kingdom::tables::Season`] is 1-based for exactly that
/// reason, so index 0 is reachable and means what it says. Falls back to
/// [`season_name`]'s English on an install with no `L2.eng` — the same four
/// words here, and not the same in a localised install, which is the whole
/// point of reading them out of the file.
pub fn season_text(assets: &crate::game::Assets, season: u8) -> String {
    let s = assets.shell.text(SEASON_GROUP, season as usize);
    if s.is_empty() {
        return season_name(season).to_string();
    }
    s.to_string()
}

// ------------------------------------------- the far zoom's box, and its words

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

/// `Eng_DrawString(101, g_scenarioIndex, …)` — the map's own name, from the
/// player's own file, with our slot number where there is no `L2.eng` to read.
pub fn map_name(ctx: &Ctx) -> String {
    let s = ctx.assets.shell.text(FAR_BOX_MAP_GROUP, ctx.game.map_slot);
    if s.is_empty() {
        return format!("MAP {}", ctx.game.map_slot);
    }
    s.to_string()
}

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

impl Screen for MapScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Campaign
    }

    /// **`g_screenId` `0x10` while an army is picked up.** `Panel_MoveButton`
    /// and `Map_Click` reach `Map_BeginMoveSelection`, which writes it, and the
    /// three ways out write `0` — which is [`MapScreen::move_order`] being
    /// `Some` and `None`. `Tip_Update`'s *"Army Movement:"* arm is the reader.
    fn mode_screen_id(&self) -> Option<u8> {
        self.move_order.is_some().then_some(0x10)
    }

    /// `g_minimapMode`, 0 owners … 3 happiness, for the tool tips.
    fn minimap_mode(&self) -> Option<u8> {
        Some(self.minimap_mode as u8)
    }

    fn title(&self, ctx: &Ctx) -> String {
        format!(
            "Lords of the Realm II - {} {}",
            season_name(ctx.game.kingdom.season),
            ctx.game.kingdom.year
        )
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **While a turn is being wound on the map is a spectator.** Every
        // hotspot it offers writes to state the phase machine is in the middle
        // of reading, so a click that landed mid-turn would race it. Pointer
        // motion still gets through, because a frozen cursor reads as a hang
        // rather than as a turn passing.
        if (turn::turn_in_flight(ctx.game) || self.fading.is_some())
            && !matches!(event, Event::Pointer { .. } | Event::PointerLeft | Event::Release { .. })
        {
            return Transition::Stay;
        }
        match event {
            Event::KeyDown(Key::Escape) => return Transition::Pop,
            // **Ours, and only the key is.** The original has no keyboard route
            // into a county panel at all; the strip's quadrants are it. Enter
            // opens the one the strip's bottom-left quadrant opens, and the
            // panel's own Up/Down cycle the other three.
            Event::KeyDown(Key::Enter) => {
                if ctx.game.selected != 0 {
                    return Transition::Push(ScreenId::County(
                        ctx.game.selected,
                        county::Panel::Tax,
                    ));
                }
            }
            // The original scrolls by pushing the pointer into the edge of the
            // *desktop* (`Map_EdgeScroll`), which a headless test cannot do and
            // a windowed player would find surprising today. The directions,
            // the step and the clamp are the original's; the keys are ours.
            Event::KeyDown(Key::Up) => {
                self.scroll(Dir::N);
            }
            Event::KeyDown(Key::Down) => {
                self.scroll(Dir::S);
            }
            Event::KeyDown(Key::Left) => {
                self.scroll(Dir::W);
            }
            Event::KeyDown(Key::Right) => {
                self.scroll(Dir::E);
            }
            // **A shortcut, now that the real route works.** The original opens
            // the village by clicking the county's *town*: `Map_Click` tests
            // plane-0 bit `0x40`, checks the county is the local player's,
            // centres the map on it and sets `g_screenId = 2`. That arm is
            // wired above — this key is a convenience for a county whose town
            // is off-screen, and it is ours.
            Event::KeyDown(Key::Char('V')) => {
                if ctx.game.is_players(ctx.game.selected) {
                    return Transition::Push(ScreenId::Village(ctx.game.selected));
                }
                self.status = "NOT YOUR COUNTY".into();
            }
            // **Ours, and only the key is.** The original reaches the
            // raise-army screen from the county strip's first button —
            // `Sidebar_Button` (`0x0043AE30`), hotspot 1, *"the county's
            // army"*, which sets `g_screenId = 0x17`. That strip is another
            // agent's, so the destination is the original's and the way in is
            // not. See `screens::army`.
            Event::KeyDown(Key::Char('R')) => {
                // `Game::open_levy` is `Sidebar_Button`'s own body: the county
                // check, `Levy_SetPercent` at the slider's last position, and
                // the basket seeded before the screen id moves.
                let county = ctx.game.selected;
                if ctx.game.open_levy(county) {
                    return Transition::Push(ScreenId::RaiseArmy(county));
                }
                self.status = "SELECT ONE OF YOUR COUNTIES FIRST".into();
            }
            // **Ours, and only the key is.** The original reaches screen `0x11`
            // from the *unit panel*'s split button, which is one of the three
            // hotspots `FUN_00437002` tests over an army. We have no unit
            // panel — our map click goes straight to move-order mode, which is
            // what `Panel_MoveButton` does with two of its three siblings
            // unbuilt — so this is the door to it. See `screens::divide`.
            Event::KeyDown(Key::Char('A')) => match self.selected_unit {
                Some(unit) if ctx.game.is_players_unit(unit) => {
                    return Transition::Push(ScreenId::Divide(unit))
                }
                _ => self.status = "CLICK ONE OF YOUR ARMIES FIRST".into(),
            },
            // **Ours entirely.** The original has no cycle key; it has a map
            // you can see the whole of at zoom 2. Ours is here because an army
            // three screens away is otherwise unreachable without scrolling.
            Event::KeyDown(Key::Char('N')) => self.cycle_unit(ctx),
            // Ours, and marked as such where it lands: the demo's index of
            // every screen, so the ones the game logic cannot yet open can
            // still be walked. `screens::index`.
            Event::KeyDown(Key::Char('I')) => return Transition::Push(ScreenId::Index),
            // **Ours, and only the key is.** The original reaches `0x35` and
            // `0x36` through the menu bar's Game drop-down (`Menu_LoadGame` and
            // `Menu_SaveGame`, which save `g_screenId` into `g_screenIdSaved`
            // so that `SaveLoad_Cancel` can put it back — which is what
            // `Transition::Pop` does here). The drop-down is not drawn yet, so
            // the destinations are the original's and the way in is not.
            Event::KeyDown(Key::Char('S')) => {
                return Transition::Push(ScreenId::SaveLoad(SaveLoadMode::Save))
            }
            Event::KeyDown(Key::Char('L')) => {
                return Transition::Push(ScreenId::SaveLoad(SaveLoadMode::Load))
            }
            Event::KeyDown(Key::Char('Z')) => self.toggle_zoom(ctx),
            Event::KeyDown(Key::Char('E')) | Event::KeyDown(Key::Space) => {
                return self.end_turn(ctx)
            }
            Event::Pointer { x, y } => {
                self.pointer = (x, y);
                self.pointer_in = true;
                // `Map_HoverUnitTarget`. It runs once a frame in the original,
                // out of `Screen_DrawWidgets`'s `0x10` arm — where every other
                // screen draws its widget table, this one recomputes the route
                // under the cursor — and its first act is to compare the hovered
                // tile with the last one and do nothing if it has not changed.
                // Driving it from pointer motion is that comparison, made by the
                // event loop instead of by hand.
                self.update_hover_path(x, y);
                // The slider's whole gesture: held **and** moved, tested
                // against the rectangle again every time, which is what lets
                // the pointer wander off the sidebar and come back without
                // letting go. See [`SPLIT_SLIDER`].
                if self.slider_held && SPLIT_SLIDER.contains(x, y) {
                    self.drag_split(ctx, x);
                }
                self.focus = if END_TURN_BUTTON.contains(x, y) {
                    Focus::EndTurn
                } else {
                    match SIDEBAR_BUTTONS.iter().position(|b| b.rect().contains(x, y)) {
                        Some(i) => Focus::Sidebar(i),
                        None => Focus::None,
                    }
                };
            }
            // **On the map the right button opens rather than closes.**
            // `Screen_FrameInput`'s `g_screenId == 0` arm ends with
            // `if (onATile && rightReleased) { g_screenId = 4; FUN_0043CAF4(); }`
            // — screen `0x04`, the information panel, which
            // `FUN_0043CAF4` fills by picking whatever is under the cursor:
            // `UnitPanel_Draw` for a unit, `FUN_0041BEFE` for a bare tile. The
            // shipped `Readme.txt` errata describes exactly this — *"Right
            // clicking on an army accesses an information pop-up that includes
            // the army's county of origin"* — and the line it names is
            // `Eng_DrawString(31, 9, ...)`, *"An army from"*, followed by group
            // 100 indexed `homeCounty + scenarioIndex * 20`.
            //
            // It is **not** gated on hitting a unit, or on owning anything. The
            // gate is `Map_PickTile` finding a tile at all, which is our map
            // clip. A right-click on the sidebar reaches the minimap-mode
            // clear instead, and we have no minimap modes to clear.
            // **Guard 2 of the arm, and it is tested before anything else the
            // right button does** — including the information panel below.
            // `FUN_00439079` consumes the click only when an overlay is up, so
            // with no overlay this falls through exactly as the original's
            // `return 0` does.
            Event::RightClick { x, y } if self.clear_minimap_mode(x, y) => {}
            Event::RightClick { x, y } if self.map_clip().contains(x, y) => {
                // **This is how an army is deselected, and it was missing.**
                //
                // A player found it in a minute: *"you cannot deselect an
                // army."* The information panel above is screen `0`'s arm, and
                // while an army is picked the screen is `0x10`, whose entire
                // right-button clause is
                //
                //     if (g_mouseRightReleased != 0) {
                //         g_screenId = 0; g_redrawRequest = 2; }
                //
                // — leave move-order mode, redraw, and that is all. No
                // information panel, no confirmation. Ours reached the panel
                // instead because the arm was written for one screen and the
                // mode it belongs to was never given its own.
                //
                // Note what is *not* here: a click on empty ground does **not**
                // cancel. In the original that click is `Map_ConfirmMoveOrder`
                // and it either places the order or does nothing at all. Adding
                // "click away to deselect" would be the same invention as the
                // tax-panel convenience removed above, made in the opposite
                // direction.
                // arm: 0x0042FF10/move-order-right-cancels right-release
                if self.selected_unit.is_some() {
                    self.cancel_move_selection();
                    self.status = "ORDERS CANCELLED".into();
                    return Transition::Stay;
                }
                // **`0x04` is a real screen now** and it needs to know what the
                // right click resolved to, because the original picks between
                // its two painters on `g_pickedTileUnit`: a unit under the
                // cursor gets the unit panel, anything else gets the tile
                // panel and — on the player's own farmland — the field brush.
                // See [`crate::screens::info`].
                let target = self
                    .pick_tile(x, y)
                    .map(|(tx, ty)| {
                        let tile = l2_kingdom::map::index(tx, ty);
                        match ctx.game.kingdom.campaign.units.at(tx, ty) {
                            Some(unit) => crate::screens::info::Target::Unit(unit),
                            None => crate::screens::info::Target::Tile(tile),
                        }
                    })
                    .unwrap_or(crate::screens::info::Target::Tile(0));
                // arm: 0x0042FF10/map-right-opens-info right-release
                return Transition::Push(ScreenId::Info(target));
            }
            // **Ours.** `winit` has no "the pointer left"; `main.rs` synthesises
            // this so a cursor that walked off the window stops scrolling the
            // map from wherever it was last seen.
            Event::PointerLeft => {
                self.pointer_in = false;
                self.focus = Focus::None;
            }
            // `FUN_00439122` eats the release and does nothing with it — the
            // value was already set on the way down and on every move since.
            // All the release does is end the drag.
            Event::Release { .. } => self.slider_held = false,
            Event::Click { x, y } => {
                // **`Map_Click`'s outermost guard, and it is the whole
                // function.** `Map_Click` (`0x0043CE1A`) is
                // `if (g_messageGroup == 0) { …all 1,263 bytes of it… } else
                // { Msg_DismissUnlessQuestion(); }` — so with a message scroll
                // up, a left click on the map closes it and **the map does
                // nothing else**: no tile picked, no county selected, no
                // village, no army ordered. A *question* survives, which is
                // what stops a stray click from silently declining an alliance.
                //
                // It is here rather than in the message screen because it is
                // here in the original: the scroll's own arm returned zero and
                // let the click through, and this is what the click found.
                // arm: 0x00476710/map-click-dismiss left-release
                if ctx.game.messages.is_open() {
                    ctx.game.messages.dismiss_unless_question();
                    return Transition::Stay;
                }
                // **`Sidebar_ButtonClicked` is guard 3 and it is OUTSIDE the
                // turn-ended gate**, which the menu bar below it is inside. So
                // the five icons and End Turn keep working once the turn has
                // been ended and nothing else in the column does. Verbatim, the
                // `g_screenId == 0` arm:
                //
                // ```c
                // if (Map_EdgeScroll() || FUN_00439079() || Sidebar_ButtonClicked()) goto done;
                // if (!syncWait && (!turnEnded || debugOverride)) {
                //     if (Menu_OpenDropdown(&g_menuBarItems, 3) || Minimap_ModeButtonClicked()
                //         || CountyStrip_Click() || Labour_SplitSliderDrag()
                //         || CountyStrip_JobClick()) goto done;
                //     ...the message scroll, the zoom, Map_Click, the info panel...
                // }
                // ```
                if END_TURN_BUTTON.contains(x, y) {
                    // **Ending the turn takes every open panel with it**, and
                    // that is not this arm's doing: `Turn_End` writes
                    // `DAT_0055403C`, and the *next* frame twenty-nine of
                    // `Screen_FrameInput`'s forty-nine arms open with
                    // `if (DAT_0055403C != 0 && !debugOverride)` and force-close.
                    // The observable result is the same and it has to happen
                    // here, because this button is reachable *through* a county
                    // panel and `Machine::update` ticks only the top screen — a
                    // turn started from under a panel would be a turn nothing
                    // wound on.
                    //
                    // `docs/arms.json` `0x0042FF10/force-close-on-turn-end` is
                    // the general arm and stays `missing`: this is one screen's
                    // corner of it, not the guard.
                    // arm: 0x0043AC23/end-turn left-press
                    let t = self.end_turn(ctx);
                    return if t == Transition::Stay { Transition::Reveal } else { t };
                } else if let Some(b) = SIDEBAR_BUTTONS.iter().find(|b| b.rect().contains(x, y)) {
                    // `g_sidebarButtons`. Three of the five reach a screen we
                    // can draw; the other two name the function the original
                    // dispatches to and do nothing, which is the honest state.
                    // Three of the five are gated on the county being yours,
                    // and the two that are not are the court and the lords.
                    // Each of the five is an arm of its own: `Sidebar_Button`
                    // (`0x0043AE30`) is a five-way `if` on `g_uiHotspotId`.
                    // arm: 0x0043AE30/sidebar-levy left-press
                    // arm: 0x0043AE30/sidebar-court left-press
                    // arm: 0x0043AE30/sidebar-supplies left-press
                    // arm: 0x0043AE30/sidebar-castle left-press
                    // arm: 0x0043AE30/sidebar-lords left-press
                    let SidebarAction::Screen(id) = b.action;
                    let gated = matches!(id, 0x17 | 0x18 | 0x1B);
                    if gated && !ctx.game.is_players(ctx.game.selected) {
                        self.status = "NOT YOUR COUNTY".into();
                        return Transition::Stay;
                    }
                    return Transition::Push(sidebar_destination(id, ctx.game.selected));
                } else if let Some(title) = menubar::title_at(&*ctx, x, y) {
                    // **`Menu_OpenDropdown` (`0x0040DECA`)** — the one line this
                    // module's header carried as *"not reproduced"*. It saves
                    // `g_screenId` into `g_menuPrevScreen` and writes `0x32`,
                    // which is a push here.
                    // arm: 0x0040DECA/open-dropdown left-press
                    return Transition::Push(ScreenId::MenuBar(title));
                } else if let Some(i) =
                    MINIMAP_MODE_BUTTONS.iter().position(|r| r.contains(x, y))
                {
                    // arm: 0x0043292D/minimap-mode-buttons left-press
                    self.minimap_mode_button(ctx, i);
                } else if SPLIT_SLIDER.contains(x, y)
                    && ctx.game.is_players(ctx.game.selected)
                {
                    // `FUN_00439122`, the farm/industry split. The press is the
                    // first frame of a **drag**: the button is now down, and
                    // every pointer move while it stays down moves the slider.
                    //
                    // **The ownership gate is the function's second line** —
                    // `if (g_counties[g_selectedCounty].owner == g_localPlayer)`
                    // — and this tested only `selected != 0`, so the slider
                    // moved another lord's peasants.
                    // arm: 0x00439122/split-slider drag
                    self.slider_held = true;
                    self.drag_split(ctx, x);
                } else if let Some(job) = self.job_row_at(&*ctx, x, y) {
                    // **`CountyStrip_JobClick` (`0x00438E3B`)** — the produce
                    // rows on the 162 × 128 plate at y = 302, which open the job
                    // popup for that row. Another of this module's header's
                    // three "not reproduced" lines.
                    // arm: 0x00438E3B/job-rows left-press
                    return Transition::Push(ScreenId::Job(ctx.game.selected, job));
                } else if let Some(panel) = county::panel_at(x, y) {
                    // **The county strip is a 2 x 2 hotspot and it is the whole
                    // navigation into the four county panels** — there is no
                    // other way into any of them, in the original or here
                    // (`docs/screens-county.md` §2.3). It used not to be tested
                    // on this screen at all, which is why a player could reach
                    // tax and nothing else: our own COUNTY PANEL button opened
                    // the county screen on its own default.
                    // arm: 0x00438CEB/strip-population left-press
                    // arm: 0x00438CEB/strip-happiness left-press
                    // arm: 0x00438CEB/strip-tax left-press
                    // arm: 0x00438CEB/strip-ration left-press
                    if ctx.game.selected != 0 {
                        return Transition::Push(ScreenId::County(ctx.game.selected, panel));
                    }
                } else if chrome::minimap_hit_area().contains(x, y) {
                    // `Minimap_Click`: the county raster decides, then
                    // `Map_CentreOnTile` moves the viewport onto it.
                    //
                    // **It is reached from every screen, not only this one.**
                    // `Screen_FrameInput`'s epilogue runs it on any press with
                    // `g_screenId != 0x12`, and closes the management surface on
                    // a hit — so this arm is live under a county panel, the job
                    // popup and a shell. Our stack says that with
                    // [`Transition::Reveal`]: the overlay passes the press down,
                    // this runs, and everything above the map is thrown away.
                    //
                    // `Minimap_Click` returns **1 for any press inside the
                    // raster**, county or no county, so the surface closes even
                    // where the raster is blank. And it returns 0 outright while
                    // `g_screenId` is `0x05` or `0x06` — the village's two drag
                    // screens — which is why the village keeps a peasant drag
                    // rather than losing it to a stray click on the minimap.
                    // arm: 0x0043253A/minimap-click left-press
                    self.ensure_minimap(ctx);
                    let county = self.minimap.as_ref().map_or(0, |m| m.county_at(x, y));
                    if county != 0 && ctx.game.select(county) {
                        let id = county as usize;
                        let anchor = (
                            ctx.game.anchor_x[id] as usize,
                            ctx.game.anchor_y[id] as usize,
                        );
                        self.centre_on_county(anchor);
                        self.status = format!("COUNTY {county} SELECTED");
                    }
                    // arm: 0x0042FF10/minimap-closes-the-surface left-press
                    return Transition::Reveal;
                } else if self.map_clip().contains(x, y) {
                    self.ensure(ctx);
                    // **At the far zoom the left button does not select
                    // anything — it zooms in on the tile under it.**
                    //
                    // ```c
                    // if (picked && leftPressed && g_mapZoom == 2) {
                    //     g_screenId = 0; Map_ZoomInAtTile(); return;
                    // }
                    // ```
                    //
                    // The arm **returns** rather than falling through, so
                    // `Map_Click` is unreachable at zoom 2 and every tile arm
                    // below — the village, the industry switch, the field
                    // brush, selecting a county — is dead there. Ours had no
                    // click arm at all at the far zoom, so a click did what the
                    // near zoom's click does, on a tile ten pixels wide.
                    //
                    // It sits inside the `g_screenId == 0` arm, so it is *not*
                    // live in move-order mode (`0x10`), whose own four-line arm
                    // has no zoom test: `self.selected_unit` is that screen id
                    // here, and it is why this is guarded rather than first.
                    //
                    // `Map_ZoomInAtTile` (`0x004350A1`) centres the near view on
                    // the picked tile with the same `col - 4`, `(row & ~1) - 12`
                    // arithmetic as `Map_CentreOnTile`, which is
                    // [`MapScreen::centre_on_tile`].
                    // arm: 0x0042FF10/map-zoom-in-at-tile left-press
                    if self.selected_unit.is_none() && self.zoom.id == FAR.id {
                        if let Some((tx, ty)) = self.pick_tile(x, y) {
                            self.set_zoom(ctx, NEAR);
                            self.centre_on_tile(tx as usize, ty as usize);
                            self.status = "ZOOMED IN".into();
                            return Transition::Stay;
                        }
                    }
                    // **Move-order mode is a screen, not a flag, and that is the
                    // whole reason this block is shaped the way it is.**
                    //
                    // `Screen_FrameInput` dispatches on `g_screenId`, and while
                    // an army is picked that is `0x10`, not `0`. So the map's
                    // own arm — `Map_Click`, every hotspot below, the sidebar
                    // guards, the minimap — **is not reachable at all**. The
                    // `0x10` arm is four lines long: the turn-ended latch,
                    // `Map_EdgeScroll`, a left press that is
                    // `Map_ConfirmMoveOrder`, and a right release that leaves.
                    //
                    // We had this as a flag consulted *after* the unit hit test,
                    // which is why clicking a second army re-selected it. In the
                    // original that click is a destination: the second army is
                    // `g_hoverMergeUnit` and the order asks *"Combine armies?"*.
                    if let Some(unit) = self.selected_unit {
                        if !ctx.game.is_players_unit(unit) {
                            // The turn-ended latch's civilian cousin: the
                            // selection's owner changed under it.
                            self.cancel_move_selection();
                        // arm: 0x0042FF10/move-order-confirm left-press
                        } else if let Some(dest) = self.pick_tile(x, y) {
                            return self.confirm_move_order(ctx, unit, dest);
                        } else {
                            self.cancel_move_selection();
                            return Transition::Stay;
                        }
                    }
                    // **`Map_Click` tests the picked *unit* before it tests any
                    // tile flag**, and both of its unit branches return without
                    // ever reaching the terrain dispatch below. That order is
                    // the rule: an army standing on your own farmland is an
                    // army, not a field.
                    // **A castle is a move *target*, not a unit**, and it is
                    // tested first because the garrison inside it stands on the
                    // castle tile.
                    //
                    // `Map_HoverUnitTarget` collects a plane-0 `0x80` tile with
                    // terrain above `0x14` as a target for the selected army —
                    // as `g_hoverGarrisonCounty` when the county is the mover's
                    // and as `g_hoverSiegeCounty` when it is not, raising
                    // `L2.eng` 10/7 *"Garrison castle?"* or 10/8 *"Besiege
                    // castle?"*. The original never offers the garrison itself,
                    // because **it does not draw a garrisoned unit at all** — it
                    // flies a flag over the castle instead. Ours draws a hollow
                    // marker so a player can see his men are in there, and that
                    // marker sat on top of the only route to a siege: clicking
                    // an enemy castle selected its garrison, and there was no
                    // way to order an army to besiege anything.
                    let castle_target = self.pick_tile(x, y).filter(|&(tx, ty)| {
                        let map = &ctx.game.kingdom.campaign.map;
                        map.has(tx, ty, l2_kingdom::map::flags::SETTLEMENT)
                            && map.terrain_at(tx, ty) > l2_kingdom::map::terrain::CASTLE_PLOT
                    });
                    if let (Some(unit), Some(dest)) = (self.selected_unit, castle_target) {
                        if ctx.game.is_players_unit(unit) {
                            return self.order_march(ctx, unit, dest);
                        }
                    }
                    if let Some(unit) = self.unit_at(&Ctx { game: ctx.game, assets: ctx.assets }, x, y)
                    {
                        return self.click_unit(ctx, unit);
                    }
                    let county = self.county_at(x, y);
                    // **A click on one of your own buildings or fields takes
                    // precedence over selecting the county**, and in that
                    // order, which is `Map_Click`'s own plane-0 dispatch:
                    // `0x80` is a settlement and switches its industry, `0x40`
                    // is the county town and opens the village, `0x20` is
                    // farmland and opens the field brush. All three are gated
                    // on the county being the local player's.
                    if county != 0 && ctx.game.is_players(county) {
                        // arm: 0x0043CE1A/industry-toggle left-release
                        if let Some(tile) = self.settlement_at(ctx, county, x, y) {
                            // **The industry arm selects the county first**, and
                            // it is the only one of the three flag arms that
                            // guards on the selection:
                            //
                            //     if (pickedCounty != g_selectedCounty) {
                            //         if (townTile == 0) return;
                            //         g_selectedCounty = pickedCounty;
                            //         Map_CentreOnTile(townTile);
                            //     }
                            //
                            // A county with no town refuses the toggle outright
                            // — the `return` is before `Industry_ToggleFromMap`.
                            if county != ctx.game.selected {
                                let Some(&town) = Self::town(ctx, county).first() else {
                                    self.status = "THAT COUNTY HAS NO TOWN".into();
                                    return Transition::Stay;
                                };
                                let (tx, ty) = l2_kingdom::map::coords(town);
                                ctx.game.select(county);
                                self.centre_on_tile(tx as usize, ty as usize);
                            }
                            let terrain = ctx.game.kingdom.campaign.map.terrain[tile];
                            match industry::map_toggle_for_graphic(terrain) {
                                Some(what) => {
                                    let on = ctx.game.kingdom.toggle_industry(county as usize, what);
                                    // **`Industry_ToggleFromMap`'s last
                                    // statement**, which was missing entirely:
                                    // *"there's no message saying or visually
                                    // showing mining on / mining off."*
                                    //
                                    // `Msg_Enqueue(0, g_localPlayer, group, 0,
                                    // 0x04, 0, 0, 0)` — a **floating tip**, so
                                    // the words appear by the cursor, carry no
                                    // OK button and time out on their own.
                                    // `industry::toggle_message_group` is the
                                    // id and the whole derivation.
                                    //
                                    // The guard is the original's own
                                    // `if (county.owner == g_localPlayer)` and
                                    // is already satisfied: this arm is inside
                                    // `ctx.game.is_players(county)`.
                                    let player = ctx.game.player;
                                    ctx.game.messages.enqueue(
                                        crate::message::Record {
                                            to: 0,
                                            from: player,
                                            group: industry::toggle_message_group(what, on),
                                            variant: 0,
                                            category: crate::message::category::TIP,
                                            county: 0,
                                            spare: 0,
                                            payload: 0,
                                        },
                                        player,
                                    );
                                    // OURS, and it stays: the sidebar's status
                                    // line is this interface's only running
                                    // commentary and a player reading it should
                                    // not have to catch a tip that lasts a
                                    // hundred ticks.
                                    self.status = format!(
                                        "{} {}",
                                        toggle_name(what),
                                        if on { "ON" } else { "OFF" }
                                    );
                                }
                                // Terrain 13 … 20 — the empty castle plot —
                                // is the original's own `return`.
                                None => self.status = "NOTHING TO SWITCH THERE".into(),
                            }
                            return Transition::Stay;
                        }
                        // **`0x40` — the county town — opens the village.**
                        // `Map_Click`'s second arm: `g_screenId = 2;
                        // Village_Draw(1)`, after `Map_CentreOnTile` has put
                        // the town in the middle. This arm was missing, so a
                        // click on the town fell through to "select the county"
                        // and the village had no route in but a key of ours.
                        // arm: 0x0043CE1A/village left-release
                        if let Some(tile) = self.tile_at(x, y, Self::town(ctx, county).into_iter())
                        {
                            // `if (townTile != 0) { g_selectedCounty = picked;
                            // Map_CentreOnTile(townTile); ... }` — the selection
                            // is inside the guard, `g_screenId = 2` outside it.
                            // We had the recentre and not the selection.
                            let (tx, ty) = l2_kingdom::map::coords(tile);
                            ctx.game.select(county);
                            self.centre_on_tile(tx as usize, ty as usize);
                            return Transition::Push(ScreenId::Village(county));
                        }
                        // **`0x20` — farmland — opens screen `0x04`, the panel a
                        // RIGHT click opens.** `Map_Click`'s third flag arm:
                        //
                        // ```c
                        // if ((flags & 0x20) && g_counties[pickedCounty].owner == g_localPlayer) {
                        //     _DAT_005681CC = 3; g_screenId = 4; FUN_0041B032();
                        // }
                        // ```
                        //
                        // and the right button's `FUN_0043CAF4` ends in the same
                        // `_DAT_005681CC = 3` and the same `FUN_0041B032()`, which
                        // picks the tile half on `g_pickedTileUnit == 0`. So on
                        // your own field **the two buttons open one screen**, the
                        // player's own words: *"right click and left click on
                        // fields does the same thing in the real game."* Ours
                        // opened a popup of our own here instead
                        // (`ours/brush-popup-on-the-map`, removed); a left click
                        // on somebody else's field falls out of the bottom, as the
                        // original's owner test inside the arm makes it.
                        //
                        // The tile is `Map_PickTile`'s, the same one the right
                        // button resolves, so the two gestures cannot disagree
                        // about which field was meant.
                        // arm: 0x0043CE1A/field-brush left-release
                        if let Some((tx, ty)) = self.pick_tile(x, y) {
                            let tile = l2_kingdom::map::index(tx, ty);
                            let map = &ctx.game.kingdom.campaign.map;
                            if map.flags[tile] & l2_kingdom::map::flags::FARMLAND != 0
                                && ctx.game.is_players(map.county[tile])
                            {
                                return Transition::Push(ScreenId::Info(
                                    crate::screens::info::Target::Tile(tile),
                                ));
                            }
                        }
                    }
                    // **And that is the end of `Map_Click`. There is no arm
                    // below this one.**
                    //
                    // A click that reaches here — plain ground, sea, a county
                    // that is not yours, your own county away from its town, its
                    // fields and its buildings — falls out of the bottom having
                    // changed nothing at all. The last statement in the original
                    // is `else { DAT_0056D64C = 0; }`, a scroll latch.
                    //
                    // We had a county-selection arm here, and then a second
                    // click on the selected county opened its tax panel. A
                    // player reported it: *"there's some weird thing where if
                    // you click anywhere on grass it opens up the tax window
                    // too."* Both halves are gone. The three ways into a
                    // selection are the ones the original has: the county strip,
                    // the minimap, and the three arms above — a village, an
                    // industry building or a merchant, each of which selects the
                    // county it belongs to on the way to opening something.
                    // `docs/decisions.md` C61.
                    let _ = county;
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    /// One tick of edge scrolling, and nothing else.
    ///
    /// It lives here rather than in `handle` because the gesture is *holding*
    /// the cursor against the edge: no further event arrives while it is held,
    /// so a scroller driven by events moves one step and stops. The step and
    /// the clamp are `Map_ScrollStep`'s and `Map_ClampScroll`'s; the rate is
    /// one step per fixed tick, which is ours because the original's is a frame
    /// rate and nothing below this crate may read a clock.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        // **`Panel_MoveButton`'s second statement.** The information panel
        // (`0x04`) writes `g_screenId = 0` and then calls
        // `Map_BeginMoveSelection()`; ours pops back to here and leaves the
        // request on [`crate::game::Game::begin_move_order`], because the
        // selection is this screen's state and not a global. Taken on the tick
        // after the pop, which is the same frame ordering the original has —
        // `Screen_FrameInput` runs last in a frame, so its navigation lands one
        // frame late by construction.
        if let Some(unit) = ctx.game.begin_move_order.take() {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            self.begin_move_selection(&read, unit);
        }
        // **A turn left suspended by a battle screen is picked up here.** Only
        // the top screen is given a tick, so this runs the moment `0x12` or
        // `0x13` pops and not before — which is exactly when the campaign is
        // allowed to move again.
        // `Map_DrawFrame`: `if (0x7F < tick) tick = 0; phase = tick >> 4;`
        // Only a change of phase is a repaint, so a still map with flags on it
        // costs eight frames every 2.05 seconds rather than sixty a second.
        self.flag_tick = (self.flag_tick + 1) & 0x7F;
        let phase = self.flag_tick >> 4;
        if phase != self.flag_phase {
            self.flag_phase = phase;
            self.scrolled = true;
        }
        // The same frame steps the herd's counter, off the same gate.
        // `Map_DrawFrame`: `if (0x5F < tick) tick = 0; phase = tick >> 4;` —
        // note the wrap is `0x60`, not a mask, so the six phases are 0 … 5 and
        // the counter is **not** a power of two.
        self.herd_tick += 1;
        if self.herd_tick > 0x5F {
            self.herd_tick = 0;
        }
        let phase = self.herd_tick >> 4;
        if phase != self.herd_phase {
            self.herd_phase = phase;
            self.scrolled = true;
        }
        // **And the industry wheels**, which are neither of those clocks: they
        // run off `Tick_Pulses` (`0x004BBC80`), the 20 ms `timeGetTime` divider
        // chain the *village* animates from, at whichever of its four rungs the
        // mine's output picked. Three different clocks on one screen, and the
        // natural guess — that everything on screen shares one — is wrong about
        // all three pairs.
        {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            if self.step_industry(&read) {
                // A wheel that turned changes the **base plane**, unlike a flag
                // or a cow, so this is a repaint of the map and not only of the
                // overlays. `ensure`'s key picks it up.
                self.scrolled = true;
            }
        }
        // The season has turned and the screen is dark or on its way there.
        // Nothing else may run: the fade *is* the frame.
        if self.fading.is_some() {
            return self.tick_fade();
        }
        // **The turn timer ran out.** `Turn_Tick`'s phase-4 arm called
        // `Turn_End` — this screen's End Turn handler — and the frame driver has
        // closed what the turn-ended guard closes; the request waits here because
        // only this screen can start a turn. It goes through the button's own
        // door, and like the button's frame it is one tick of the turn and no
        // more. `crate::turn_clock`, `Machine::run_turn_clock`.
        if ctx.game.turn_clock.end_turn_pending() && !turn::turn_in_flight(ctx.game) {
            ctx.game.turn_clock.take_end_turn();
            return self.end_turn(ctx);
        }
        let resumed = self.resume_turn(ctx);
        if resumed != Transition::Stay {
            return resumed;
        }
        // A turn in flight winds itself on above and runs `Units_Tick` as part
        // of doing so; the sweep below must not run a second time in the same
        // frame, or every unit would take two tiles a tick and three of the
        // seven phases would settle early.
        if !turn::turn_in_flight(ctx.game) {
            // **`Units_Tick` on an ordinary frame.** This is what makes an army
            // the player has just ordered walk away while he watches, rather
            // than standing still until End Turn. See
            // [`turn::tick_units_only`].
            if turn::tick_units_only(ctx.game) > 0 {
                self.scrolled = true;
            }
        }
        // **`Map_ScrollThrottle` (`0x004BBBE3`)** — the map does not step on
        // every frame the pointer is at the edge. See
        // [`MapScreen::scroll_interval_ticks`].
        let every = self.scroll_interval_ticks();
        self.scroll_wait = self.scroll_wait.saturating_sub(1);
        // `q >= 10` — speed 0 — is the original's own early return, and no
        // amount of waiting satisfies it.
        // arm: 0x00432221/map-edge-scroll hover-at-edge
        if self.scroll_wait == 0 && every != u32::MAX {
            if let Some(dir) = self.edge_direction() {
                self.scroll_wait = every;
                if self.scroll(dir) {
                    self.scrolled = true;
                }
            }
        }
        Transition::Stay
    }

    fn fade(&self) -> Option<u8> {
        self.fading.as_ref().map(|f| f.phase)
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::replace(&mut self.scrolled, false)
    }

    /// `FUN_0049A3E6`'s `Save_RotateAndWrite()`. Raised in
    /// [`MapScreen::tick_fade`] at [`l2_view::fade::is_darkest`].
    fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        self.ensure(ctx);
        self.ensure_minimap(ctx);
        let ink = &ctx.assets.ink;
        let game = &ctx.game;
        let k = &game.kingdom;
        let clip = self.map_clip();

        canvas.pixels.copy_from_slice(&self.base.pixels);

        // **The yellow outline used to be drawn here, and it is gone.** It was
        // the visual half of the county-selection arm this module's header
        // records removing, and it survived that removal by being in the
        // painter rather than in the input ladder — `docs/agents.md`'s second
        // place behaviour hides. The original draws no selection on the map at
        // all: its borders are in the tile data (plane-0 bit `0x02`, the
        // `roads` bank's boundary frames) and its *selection* is which county
        // the right panel describes. `the_selection_is_not_drawn_on_the_map`
        // is the assertion that could not exist while it did.

        // Ours: one marker per county in view, at its anchor tile, coloured by
        // owner — **debug overlay only**. It stood in for the owner's banner
        // before `draw_flags` placed it; `Sprite_TopIt` draws the banner on the
        // town's quadrant 0 and **nothing** at our anchor, so a player saw a
        // square on the town square the original never had.
        let debug = game.prefs.debug_overlay;
        for id in k.county_ids().filter(|_| debug) {
            let (ax, ay) = (game.anchor_x[id] as usize, game.anchor_y[id] as usize);
            // **In the dark, not at all.** This marker is ours, standing in for
            // the owner's banner `Sprite_TopIt` flies — and that banner is
            // behind the fog test, so an owner colour here would tell the
            // player what the original hides.
            if game.hides_tile(l2_kingdom::map::index(ax as u8, ay as u8)) {
                continue;
            }
            let Some((cx, cy)) = campaign::tile_centre(self.view, &self.zoom, ax, ay) else {
                continue;
            };
            let owner = k.counties[id].owner as usize;
            let colour = ink.realm.get(owner).copied().unwrap_or(ink.dim);
            fill_clipped(
                canvas,
                cx - MARKER - 1,
                cy - MARKER - 1,
                MARKER * 2 + 3,
                ink.background,
                clip,
            );
            fill_clipped(
                canvas,
                cx - MARKER,
                cy - MARKER,
                MARKER * 2 + 1,
                colour,
                clip,
            );
        }

        // **`Screen_DrawCampaign`'s far-zoom arm** (`0x0040F5FD`), whole:
        //
        // ```c
        // Ui_DrawBox(0, 0x19C, 0x1E, 4);            /* (0, 412), 480 x 64   */
        // DAT_0058FE2C = 1;
        // g_penAdvance = 0;
        // Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8, &g_fontHeading, 0x3F);
        // Eng_DrawString(0x22, 0, g_penAdvance + 0x50, 0x1A8, &g_fontHeading, 0x3F);
        // Ui_DrawYear(g_year, g_penAdvance + 0x60, 0x1A8, 1);
        // DAT_0058FE2C = 0;
        // Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F);
        // ```
        //
        // The box was here and the four draws were not: C173 gated our own
        // status line out of it and left the box **empty**, which is the hole
        // that correction filed rather than closed. Group 101 is the sixty map
        // names and group 34 is *"Year"* and *"Click on the county you wish to
        // view."* — the game saying in its own words what the far zoom is for,
        // which is why `Map_Click` does nothing at zoom 2. `docs/draws-map.md`
        // §5.4, C89, C189.
        //
        // **There is no season in this box.** C173's note and `docs/screens.md`
        // §7 both say *"the map's name, the season and the year"*; the painter
        // draws three things and a season is not one of them. The season is on
        // the menu bar, out of group 29.
        //
        // `g_penAdvance` is the width drawn **since the reset**, cumulative
        // over both strings, and every `Pen` method returns an absolute x — so
        // `g_penAdvance + 0x50` after a name drawn at `0x40` is sixteen pixels
        // past where that name ended, and `g_penAdvance + 0x60` after a label
        // drawn at `0x50` is sixteen past *that*. Written as the subtraction
        // the decompilation implies, like `draw_menu_bar` two hundred lines
        // below. `docs/decisions.md` C61 — the same confusion, seven times.
        if self.zoom.id == FAR.id {
            match &ctx.assets.chrome {
                Some(c) => c.draw_box(canvas, 0, 412, 30, 4, 0),
                None => widget::panel(canvas, ink, Rect::new(0, 412, 480, 64)),
            }
            let pen = crate::shell::Pen {
                assets: &ctx.assets.shell,
                ink,
                chrome: ctx.assets.chrome.as_ref(),
                shadow: Some(font::SHADOW),
                caps: None,
            };
            // `DAT_0058FE2C = 1` over the three heading draws.
            let caps = pen.drop_caps();
            let face = crate::shell::Face::Heading;
            let name = map_name(ctx);
            let after_name = caps.text_in(face, canvas, FAR_BOX_NAME_X, FAR_BOX_Y, &name, font::TEXT);
            let after_year_label = caps.text_in(
                face,
                canvas,
                after_name - FAR_BOX_NAME_X + FAR_BOX_YEAR_LABEL_X,
                FAR_BOX_Y,
                &far_box_text(ctx, FAR_BOX_YEAR_LABEL),
                font::TEXT,
            );
            caps.year(
                canvas,
                after_year_label - FAR_BOX_YEAR_LABEL_X + FAR_BOX_YEAR_X,
                FAR_BOX_Y,
                ctx.game.kingdom.year,
                1,
                font::TEXT,
            );
            // `DAT_0058FE2C = 0` again, and the body font for the instruction.
            pen.body(
                canvas,
                FAR_BOX_ADVICE_X,
                FAR_BOX_ADVICE_Y,
                &far_box_text(ctx, FAR_BOX_ADVICE),
                font::TEXT,
            );
            // Ours, under the original's two lines: debug overlay only.
            if debug {
                text::draw(canvas, 24, FAR_BOX_ADVICE_Y + 14, &self.status, ink.text);
            }
        }

        // Ours: the player's own county's fields, marked by what each is being
        // used for — **debug overlay only**, because the original draws nothing
        // over a field but its own artwork and a herd. See [`brush`].
        if debug && game.is_players(game.selected) {
            for (tile, kind) in k.field_tiles(game.selected as usize) {
                // Ours, and kept out of the dark with everything else a tile
                // carries. A county of the player's is seen from the moment it
                // is his, so this skips nothing a player can reach.
                if game.hides_tile(tile) {
                    continue;
                }
                let (tx, ty) = l2_kingdom::map::coords(tile);
                let Some((cx, cy)) =
                    campaign::tile_centre(self.view, &self.zoom, tx as usize, ty as usize)
                else {
                    continue;
                };
                let m = brush::FIELD_MARKER;
                fill_clipped(
                    canvas,
                    cx - m - 1,
                    cy - m - 1,
                    m * 2 + 3,
                    ink.background,
                    clip,
                );
                fill_clipped(
                    canvas,
                    cx - m,
                    cy - m,
                    m * 2 + 1,
                    field_colour(ink, kind),
                    clip,
                );
            }
        }

        // `Map_DrawFrame`'s order: the terrain, then the building/flag pass,
        // then the unit sprites — so a flag is over the town and under an army
        // walking past it.
        // **Two loops where the original has one, and it shows in one place.**
        // `FUN_004071A0` handles all four arms inside a single lattice-ordered
        // traversal, so a cattle sprite on a later tile overdraws a flag on an
        // earlier one. Ours draws every herd and then every flag, so the flag
        // wins instead. A tile is never both, so nothing is drawn twice; the
        // only visible difference is a 58 × 30 meadow overlapping the 32 × 24
        // banner of the town up and to its left. Recorded rather than papered
        // over — merging the passes means the county loop and the tile loop
        // becoming one, which is a bigger change than the defect.
        draw_herds(self, canvas, ctx, clip);
        draw_flags(self, canvas, ctx, clip);
        draw_path_preview(self, canvas, ctx, clip);
        draw_units(self, canvas, ctx, clip);

        draw_menu_bar(canvas, ctx);
        draw_right_panel(self, canvas, ctx);
        draw_unit_banner(self, canvas, ctx);
    }
}

/// Half the side of a unit's marker, in pixels.
///
/// **The three size classes are the original's** — `Army_Tick` picks sprite
/// bank `0x48`, `0x60` or `0x78` at **301 and 601 men**, which the player
/// described as one, two or three figures (`docs/armies.md` §2.4) — and the
/// marker grows with them so that the same thing is legible. **The square is
/// ours**: `Sprite1a.pl8` holds the actual figures and we do not place them.
fn unit_marker_half(zoom: &Zoom, unit: &l2_kingdom::Unit) -> i32 {
    let base = if zoom.id == FAR.id { 1 } else { 3 };
    base + unit.size_class() as i32
}

/// **What `Map_DrawArmies` (`0x00408438`) draws one unit with**: the sheet; the
/// frame its tick handler wrote on the way into the last sweep
/// ([`crate::game::UnitFrames`]); the per-kind nudge; and the walk offset for
/// its facing and `+0x149`, which is how far across its tile it is
/// ([`campaign::walk_offset`]).
///
/// One function for the painter and for the hit test, so a figure part-way
/// across a tile is clicked where it is seen.
fn unit_sprite(zoom: &Zoom, game: &crate::game::Game, id: usize, unit: &l2_kingdom::Unit) -> campaign::UnitSprite {
    campaign::UnitSprite {
        sheet: unit.sprite_sheet(),
        frame: game.unit_frame(id, unit),
        nudge: unit.sprite_nudge(),
        walk: campaign::walk_offset(zoom, unit.facing, unit.sub_tile),
    }
}

/// **`Map_DrawArmies` (`0x00408438`)** — every unit on the map, as the figure
/// the original draws.
///
/// The sheet, the frame and the placement are all the original's:
/// `Sprite1a.pl8` for armies, mobs **and merchants**, `Sprite1b.pl8` for
/// transports alone, `frame = bank + 3*((facing+1)&7) + walk` for the first two
/// and `6*((facing+1)&7) + phase` for the other two, anchored on the tile's
/// bottom vertex, then **dragged back toward the tile it is leaving** by the
/// walk tables. See [`unit_sprite`], [`l2_kingdom::Unit::sprite_frame`] and
/// [`campaign::walk_offset`]. This used to leave the walk tables out as needing
/// *"a sub-tile step counter we do not keep"*; `docs/decisions.md` C134 gave
/// the unit that counter and nothing here read it, so every army jumped from
/// tile to tile.
///
/// **What is still ours** is the *selection* and the *state marks*: the
/// original shows a selected army by flood-filling its reachable tiles, and it
/// marks a besieger with `Flags1a.pl8` frame `0x82`. A garrisoned unit is drawn
/// hollow because it is inside the castle rather than standing on the tile —
/// the original draws it not at all and flies a flag over the castle instead
/// (see [`draw_flags`]).
///
/// The square marker is the fallback for an install with no `Sprite?a.pl8`, and
/// for the placeholder assets the tests use. It says *there is something here*
/// without claiming to be the game's art. `docs/decisions.md` C21.
fn draw_units(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let ink = &ctx.assets.ink;
    for (id, unit) in ctx.game.kingdom.campaign.units.iter() {
        // **`Map_DrawArmies`' whole body is inside the fog test**:
        // `if (g_optExploration != 1 || (tile.bank & 0x20) != 0) { ...every unit
        // on the tile... }`. A unit in the dark is not drawn — not its figure,
        // not its banner — and so neither are the marks this function adds of
        // its own. Its *click* is not gated: no arm reads the bit.
        if ctx.game.hides_tile(l2_kingdom::map::index(unit.x, unit.y)) {
            continue;
        }
        let Some((cx, cy)) =
            campaign::tile_centre(screen.view, &screen.zoom, unit.x as usize, unit.y as usize)
        else {
            continue;
        };
        let h = unit_marker_half(&screen.zoom, unit);
        let debug = ctx.game.prefs.debug_overlay;
        // A garrisoned unit is inside the castle. The original does not draw it
        // on the map at all — `draw_flags` flies the garrison's banner over the
        // castle instead — so ours draws nothing either, and the hollow marker
        // that used to say "your garrison is in there" is **debug overlay
        // only**. Never the figure, which would say it was standing outside.
        let drawn = !unit.is_garrisoned()
            && campaign::draw_unit(
                canvas,
                &ctx.assets.map,
                screen.view,
                &screen.zoom,
                (unit.x as usize, unit.y as usize),
                unit_sprite(&screen.zoom, ctx.game, id, unit),
                clip,
            );
        // The square is the fallback for an install with no sprite sheet, and a
        // normal install never reaches it — except for a garrison, which is the
        // overlay's.
        if !drawn && (debug || !unit.is_garrisoned()) {
            let colour = ink.realm.get(unit.owner as usize).copied().unwrap_or(ink.dim);
            fill_clipped(canvas, cx - h - 1, cy - h - 1, h * 2 + 3, ink.background, clip);
            fill_clipped(canvas, cx - h, cy - h, h * 2 + 1, colour, clip);
            if unit.is_garrisoned() {
                fill_clipped(
                    canvas,
                    cx - h + 1,
                    cy - h + 1,
                    (h * 2 - 1).max(1),
                    ink.background,
                    clip,
                );
            }
        }
        // The selection ring: `g_selectedUnit`, and the map is taking orders
        // for it. **Ours**, debug overlay only — `Map_DrawArmies` draws the
        // figure and its banner and nothing round them; the original's answer
        // to "which army is picked" is the path under the cursor
        // (`draw_path_preview`).
        if debug && screen.selected_unit == Some(id) {
            let r = h + 3;
            widget::frame(canvas, Rect::new(cx - r, cy - r, r * 2 + 1, r * 2 + 1), ink.highlight);
        }
        // A besieger carries a second, smaller mark: it is camped rather than
        // standing, and clicking it opens the siege screen rather than ordering
        // a march. The original's is `Flags1a.pl8` frame `0x82` with the seasons
        // left printed under it (`FUN_00407F82`), over the *castle*; ours is a
        // dot over the unit, where the original draws nothing, so it is debug
        // overlay only and `FUN_00407F82` stays a missing draw.
        if debug && unit.besieging_county != 0 {
            fill_clipped(canvas, cx - 1, cy - h - 4, 3, ink.bad, clip);
        }
    }
}

/// **`FUN_004071A0`'s two flags** — the county town's owner-coloured banner and
/// the castle's garrison banner, both waving.
///
/// A player who has played the original: *"each county's town square would have
/// a coloured flag waving on it, and castles with armies in them have a flag."*
/// Both are in `FUN_004071A0` (`0x004071A0`), the pass `Map_DrawFrame` runs
/// between the terrain and the unit sprites, gated on the runtime tile record's
/// **bank bit `0x80`** — which `County_FindTownTile` and `County_FindCastleTile`
/// set on their anchor quadrants. The branch inside then splits on plane 0:
///
/// ```c
/// if      (flags & 0x40)  /* the town  */ { quadrant 0: county.shield;  quadrant 2: merc offer }
/// else if (flags & 0x80)  /* the castle*/ { if (content <= 0x14 || !county.garrisonUnit) return;
///                                           shield = units[county.garrisonUnit].shield; }
/// frame = shield * 8 - 8 + phase;
/// ```
///
/// Three things worth stating because each is a decision:
///
/// * **The colour is the frame index**, not a palette remap — `Flags1a.pl8`'s
///   first forty frames are five shields by eight wave phases, and `shield = 5,
///   phase = 7` lands on the fortieth exactly.
/// * **The castle flag carries the *garrison's* shield, not the county's.** A
///   captured castle whose garrison is still somebody else's flies the
///   garrison's colours, and the two flags of one county can disagree.
/// * **`content == 0x14` returns**: `0x14` is the bare castle plot and
///   `0x15 … 0x19` are castle types 1 … 5, so an unbuilt castle flies nothing
///   even with a garrison standing on it.
///
/// **`docs/screens.md` §5 attributed all of this to `FUN_004081A6`, which is
/// not the flag at all** — it is the gold path-preview ball, and its `bank`
/// bit `0x40` is the path mark `Path_MarkPreviewTiles` sets. C49.
fn draw_flags(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let k = &ctx.game.kingdom;
    let phase = screen.flag_phase;
    // A free function rather than a closure: the mercenary arm below needs the
    // canvas too, and a closure that captured it would hold the borrow for the
    // whole loop.
    fn flag(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip, tile: usize, frame: usize) {
        let (x, y) = l2_kingdom::map::coords(tile);
        campaign::draw_flag(
            canvas,
            &ctx.assets.map,
            screen.view,
            &screen.zoom,
            (x as usize, y as usize),
            frame,
            clip,
        );
    }
    // **`Sprite_TopIt`'s first statement is the fog test**, before it looks at
    // a single flag bit: `if (g_optExploration == 1 && (tile.bank & 0x20) == 0)
    // return 0;`. Every arm below is behind it, on the tile it would draw on,
    // so a town in the dark flies nothing, advertises no band and shows no
    // garrison — and its owner is not given away by the banner.
    let lit = |tile: &usize| !ctx.game.hides_tile(*tile);
    for id in k.county_ids() {
        let county = &k.counties[id];
        // The town's 2 x 2 block. Plane-3 quadrant 0 is its north-west tile —
        // the lowest tile index, and the top of the diamond.
        let shield = k.realms.get(county.owner as usize).map_or(0, |r| r.shield_index);
        if let (Some(nw), Some(frame)) = (
            MapScreen::town_quadrant(ctx, id as u8, 0).filter(lit),
            campaign::flag_frame(shield, phase),
        ) {
            flag(screen, canvas, ctx, clip, nw, frame);
        }
        // **`county.mercenaryOffer != 0` puts frame `0x81` on quadrant 2**, the
        // tile one map row south of the origin — a standing band, advertised on
        // the map. See [`MapScreen::town_quadrant`] for why that is `town()[2]`
        // and not `town()[1]`, which is what this drew and which is a tile the
        // original's overlay pass never runs on at all.
        //
        // **It goes through its own painter**, because its offset is not the
        // banner's: `(+0x10, −0x12)` at the near zoom against the banner's
        // `(+0x1A, −0x1C)`. Sharing [`campaign::draw_flag`] is how it came to be
        // ten pixels out.
        if county.mercenary_offer != 0 {
            if let Some(tile) = MapScreen::town_quadrant(ctx, id as u8, 2).filter(lit) {
                let (x, y) = l2_kingdom::map::coords(tile);
                campaign::draw_mercenary_marker(
                    canvas,
                    &ctx.assets.map,
                    screen.view,
                    &screen.zoom,
                    (x as usize, y as usize),
                    clip,
                );
            }
        }
        // The castle: built, and holding a garrison.
        if county.castle_type == 0 || county.garrison_unit == 0 {
            continue;
        }
        let garrison_shield =
            k.campaign.units.get(county.garrison_unit).map_or(0, |u| u.shield);
        let Some(frame) = campaign::flag_frame(garrison_shield, phase) else { continue };
        let castle = MapScreen::settlements(ctx, id as u8)
            .into_iter()
            .find(|&t| industry::map_toggle_for_graphic(k.campaign.map.terrain[t])
                == Some(industry::MapToggle::Castle));
        if let Some(tile) = castle.filter(lit) {
            flag(screen, canvas, ctx, clip, tile, frame);
        }
    }
}

/// **`FUN_004071A0`'s farm arm — the cattle in the pastures.**
///
/// A player who had the build in front of him: *"why do the pastures not have
/// cows in them?"* Because this pass had three of its four arms and not the
/// fourth. It is the same overlay pass as [`draw_flags`], on the same bank bit
/// `0x80`, off the same `Flags1a.pl8` — `Terrain_Set` sets that bit for
/// `0x0E < terrain < 0x17`, which is exactly the pasture range, so a pasture is
/// the one field state that gets a second blit at all.
///
/// **Which of three pictures is drawn is `herd ÷ fieldsCattle`**, so this is a
/// rule wearing a graphic's clothes: `l2_kingdom::land::herd_graphic` bands the
/// density at 11 and 21 and `Herd_UpdateCrowding` writes the answer onto every
/// pasture tile of the county. The terrain byte carries it, this reads it back,
/// and neither end guesses. An empty herd is terrain `0x13` and draws nothing.
///
/// The phase is [`MapScreen::herd_phase`] and is **display state**: it never
/// reaches [`l2_kingdom::Kingdom`], exactly as the flag's does not.
fn draw_herds(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let map = &ctx.game.kingdom.campaign.map;
    for tile in 0..map.terrain.len() {
        if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
            continue;
        }
        // `Sprite_TopIt`'s fog test, ahead of the farm arm like every other.
        if ctx.game.hides_tile(tile) {
            continue;
        }
        let (x, y) = l2_kingdom::map::coords(tile);
        campaign::draw_herd(
            canvas,
            &ctx.assets.map,
            screen.view,
            &screen.zoom,
            (x as usize, y as usize),
            map.terrain[tile],
            screen.herd_phase,
            clip,
        );
    }
}

/// **`Path_MarkPreviewTiles` (`0x004A91BA`) and `Map_DrawPathMarker`** — the
/// gold balls along an ordered path.
///
/// The original sets bit `0x40` of each path tile's runtime record and then
/// draws `g_flagsSheet` frame `0x38 + cost` on every tile carrying it, so the
/// frame index *is* the accumulated cost and everything past the remaining
/// budget collapses to frame `0x38`. `docs/armies.md` §2.3.
///
/// **The balls are the game's own art now.** A player who has played it:
/// *"there are colored dot images for the army walking dots"* — and they are
/// `Flags1a.pl8` frames `0x38 … 0x4D`, 23 pictures of one 15 × 15 ball in
/// rising amounts of colour, indexed by the **accumulated cost** of reaching
/// that tile. Nothing about the realm, the shield or the unit's kind selects
/// the colour; see [`campaign::path_marker_frame`], which carries the
/// measurement that settled it.
///
/// The small dot is the fallback for an install with no `Flags1a.pl8`, and for
/// the placeholder assets the tests use — bright while the army can still reach
/// the tile this season, dim beyond that, greying at exactly the step the
/// original greys at. `docs/decisions.md` C21.
///
/// **This is the feedback the player was missing.** `Unit_OrderMove` writes
/// nothing at all when no path is found, and an *unreachable* destination is an
/// accepted order with an empty path — so a refused order, a hopeless one and a
/// perfectly good one all looked the same on screen. The path is the original's
/// own answer to that, and drawing it is how a player tells our bug from his own
/// mis-click.
fn draw_path_preview(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let ink = &ctx.assets.ink;
    let Some(sel) = screen.move_order.as_ref() else { return };
    let Some(unit) = ctx.game.kingdom.campaign.units.get(sel.unit) else { return };
    let left_at_start = unit.moves_left();
    // **The route under the cursor, not the route already ordered.** This loop
    // used to walk `unit.path` — the *committed* path — and re-derive each
    // step's cost by hand from the road flag. Both were wrong in the same way:
    // the original never draws balls for an order that has been placed (the
    // only writer of the bank bit runs only on screen `0x10`), and it never
    // re-derives a cost, because the flood fill already holds one.
    //
    // `local_c = g_moveDistLocal[tile] - 1`, and `if (allowance - used <
    // local_c) local_c = 0` — a step past the army's remaining moves draws
    // frame `0x38`, the one recolouring in the run with no colour in it.
    for &(x, y) in &sel.path {
        let spent = sel.field.cost_to(x, y).unwrap_or(0);
        let in_range = spent <= left_at_start;
        // `local_14`: the gold ball on a tile a click would act on, tested
        // before the cost is looked at. See [`path_marker_is_action`].
        let action = path_marker_is_action(&ctx.game.kingdom.campaign.map, x, y);
        let drawn = campaign::draw_path_marker(
            canvas,
            &ctx.assets.map,
            screen.view,
            &screen.zoom,
            (x as usize, y as usize),
            campaign::path_marker_frame(spent, in_range, action),
            clip,
        );
        if drawn {
            continue;
        }
        let Some((cx, cy)) = campaign::tile_centre(screen.view, &screen.zoom, x as usize, y as usize)
        else {
            continue;
        };
        let colour = if in_range { ink.highlight } else { ink.dim };
        fill_clipped(canvas, cx - 1, cy - 1, 3, colour, clip);
    }
}

/// **`Map_DrawPathMarker`'s `local_14` (`0x004081A6`) — is this a tile a click
/// would act on?**
///
/// ```c
/// local_14 = flags & 0x50;                                   /* the town, or a dwelling plot */
/// if ((flags & 0x80) != 0 && content != 0x14) local_14 = 1;  /* a site or a castle, not a bare plot */
/// ```
///
/// The plane-0 bits and the terrain byte, and nothing else: not the owner, not
/// the reach, not a unit standing there. So your own town is gold, a town past
/// the budget is gold, and an enemy army on open ground is coloured by its cost
/// like any other tile — which is narrower than *"an attack"*, and is the
/// original's. `Map_HoverUnitTarget` asks the same bits separately, with an
/// owner test, to decide what a click would *do*; the ball does not.
fn path_marker_is_action(map: &l2_kingdom::map::CampaignMap, x: u8, y: u8) -> bool {
    use l2_kingdom::map::{flags, terrain};
    let tile = l2_kingdom::map::index(x, y);
    let f = map.flags[tile];
    f & (flags::CASTLE | flags::PLOT) != 0
        || (f & flags::SETTLEMENT != 0 && map.terrain[tile] != terrain::CASTLE_PLOT)
}

/// **Ours, and it is deliberately not in the right column.**
///
/// The original's army panel is `UnitPanel_Draw` (`0x0041B19D`) with `L2.eng`
/// group 31's own field labels beside the record's offsets — *Wages*, *Formed*,
/// *Morale*, *N moves left.*, the supply line and the health line. It is screen
/// `0x04`, it is a **shell**, and a right-click is how the original opens it —
/// which is a different gesture on a different screen from this one.
///
/// This is one line of the same numbers, drawn in our font at the bottom-left
/// of the viewport while an army is selected, so that a player *placing a march
/// order* can see what he is ordering without leaving move-order mode. The
/// right column belongs to the county strip. When `0x04` graduates, this stays:
/// they answer different questions.
fn draw_unit_banner(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx) {
    // Debug overlay only: the original draws nothing over the foot of the map.
    if !ctx.game.prefs.debug_overlay {
        return;
    }
    let Some(id) = screen.selected_unit else { return };
    let Some(unit) = ctx.game.kingdom.campaign.units.get(id) else { return };
    let ink = &ctx.assets.ink;
    let bar = Rect::new(0, NEAR.bottom() - 26, PANEL_X, 26);
    widget::panel(canvas, ink, bar);
    // `L2.eng` 31/22 prints `moveAllowance - movesUsed` as "moves left."
    let home = unit.home_county;
    text::draw(
        canvas,
        4,
        bar.y + 4,
        &format!(
            "#{id} {} - {} MEN, {} MOVES LEFT, MORALE {}",
            unit.kind.name().to_uppercase(),
            unit.men,
            unit.moves_left(),
            unit.morale,
        ),
        ink.text,
    );
    let where_to = if unit.besieging_county != 0 {
        format!("BESIEGING COUNTY {}", unit.besieging_county)
    } else if unit.garrison_county != 0 {
        format!("GARRISONING COUNTY {}", unit.garrison_county)
    } else if !unit.path.is_empty() {
        format!("{} STEPS TO GO", unit.path.len())
    } else {
        "IDLE - CLICK A TILE TO MARCH, A FOR ORDERS".into()
    };
    text::draw(canvas, 4, bar.y + 14, &format!("FROM COUNTY {home}. {where_to}"), ink.dim);
}

/// What a settlement click switched, in words. **Ours** — the original enqueues
/// one of `L2.eng`'s "mining stopped / started" messages instead.
fn toggle_name(what: industry::MapToggle) -> &'static str {
    match what {
        industry::MapToggle::Industry(c) => match c {
            l2_kingdom::Commodity::Wood => "WOOD CUTTING",
            l2_kingdom::Commodity::Iron => "IRON MINING",
            l2_kingdom::Commodity::Weapons => "THE SMITHY",
            l2_kingdom::Commodity::Stone => "STONE QUARRYING",
        },
        industry::MapToggle::Castle => "CASTLE BUILDING",
    }
}

/// **Ours.** One colour per field use, so a marked field says what it is
/// without a legend.
fn field_colour(ink: &Ink, kind: FieldType) -> u8 {
    match kind {
        FieldType::Grain => ink.good,
        FieldType::Pasture => ink.highlight,
        FieldType::Fallow => ink.dim,
        FieldType::Reclaiming => ink.panel,
        FieldType::Waste => ink.bad,
    }
}

/// A filled square, clipped to the map viewport so a marker near the edge
/// cannot spill into the panel.
fn fill_clipped(canvas: &mut Canvas, x: i32, y: i32, side: i32, colour: u8, clip: Clip) {
    for yy in y..y + side {
        for xx in x..x + side {
            if clip.contains(xx, yy) {
                canvas.set(xx as usize, yy as usize, colour);
            }
        }
    }
}

/// `Ui_DrawYear(g_year, 0x168, 6, 3)` — where the year starts, and the row every
/// one of the bar's three readings sits on.
const CLOCK_X: i32 = 0x168;
const CLOCK_Y: i32 = 6;
/// `Eng_DrawString(0x1D, g_season, g_penAdvance + 0x16C, …)` — the season's
/// **base**, which the year's own width is added to. Four pixels right of
/// [`CLOCK_X`], and that four is the original's, not a rounding of ours.
const SEASON_X: i32 = 0x16C;
/// `L2.eng` group 29: `"No Season"`, `"Spring"`, `"Summer"`, `"Autumn"`,
/// `"Winter"` — indexed by `g_season` with no adjustment.
pub const SEASON_GROUP: usize = 29;
/// `Ui_DrawCount(gold, 0, 500, 6, …)` — the treasury, and its group 8 noun
/// index. 0/1 is *"Crown."* / *"Crowns."*.
const GOLD_X: i32 = 500;
const GOLD_NOUN: usize = 0;

/// `Screen_DrawMenuBar`, as far as we can reproduce it.
///
/// **The original's:** the 640 × 24 background tiled from `Panels.pl8` frames
/// 196 + (c mod 8) — 25 cells from x 0 and 2 more from x 592 — and one 13 × 16
/// `Misc_cty` banner per live realm at `x = 270 + 16i, y = 4`.
///
/// **The File / Options / Help titles are drawn now**, out of `L2.eng` groups
/// 1, 2 and 3 index 0, measured the way `Ui_DrawMenuTitles` (`0x0040C5B0`)
/// measures them — see [`menubar`](crate::screens::menubar). This comment used
/// to say they were not, which was true and was nineteen input arms.
///
/// **And so are the year, the season and the treasury**, which used to be the
/// line here: *"ours: the clock and treasury are our font at the original's x
/// positions."* A player read that off the screen —
///
/// > *"still placeholder font in the top right for gold and summer"*
///
/// — and he was looking at two `l2_view::text::draw` calls in the 5 × 7 debug
/// font, on the busiest chrome in the game. Reading the tail of
/// `Screen_DrawMenuBar` back turned up three things beyond the face:
///
/// ```c
/// g_penAdvance = 0;
/// Ui_DrawYear(g_year, 0x168, 6, 3);
/// Eng_DrawString(0x1D, g_season, g_penAdvance + 0x16C, 6, &g_fontBody, 0x3F);
/// ...
/// Ui_DrawCount(g_realms[g_localPlayer].gold, 0, 500, 6, &g_fontBody, 0x3F);
/// ```
///
/// * **the year comes first and the season after it**, placed by the pen rather
///   than by a coordinate. We drew `"{season} {year}"`, in that order, at 360.
/// * **the treasury is a count, not a caption**: `Ui_DrawCount(gold, 0, …)`
///   draws the number and then `L2.eng` group 8's *"Crown."* / *"Crowns."*. We
///   drew the word `GOLD`, which is not in `L2.eng` at all.
/// * **`g_fontBody` is `Fntl2_14.pl8`**, one of the game's two *blackletter*
///   faces — so "the right font" here is the display one, not the plain one,
///   which is the opposite of where [`crate::build_id`] lands and worth stating
///   because the instinct is to reach for legibility. Verified twice:
///   `docs/symbols.md` `0x005AF8F0`, and the preload table at `0x004D9FC0`
///   gives `fntl2_14.pl8` a buffer of `0x36B0` bytes, which is exactly
///   `g_fontHeading - g_fontBody`.
fn draw_menu_bar(canvas: &mut Canvas, ctx: &Ctx) {
    let ink = &ctx.assets.ink;
    let game = &ctx.game;
    let k = &game.kingdom;

    match &ctx.assets.chrome {
        Some(c) => {
            c.draw_menu_bar_background(canvas);
            // `Screen_DrawMenuBar` (`0x00419C78`): realms 1..=5 under
            // `strength != 0 && aiStep < 999`, banner at 270 + 16 * slot.
            //
            // **The shield row is the turn clock.** A realm's banner is up
            // while it has a turn still to play and goes the moment it ends
            // one — the person's own on the click, each AI's as it finishes —
            // and all of them come back when the next turn begins.
            // `turn::realm_turn_ended` is the second clause, and says why it
            // is not a literal `ai_step < 999`.
            //
            // `slot` is `local_c`, which the original advances after every
            // `Pl8_DrawFrame` it calls — so the row closes up leftwards over a
            // realm that is out, and a frame that fails to load still holds
            // its place rather than shifting its neighbours onto it.
            let mut slot = 0;
            for id in 1..k.realms.len() {
                if !k.realms[id].in_play || turn::realm_turn_ended(game, id) {
                    continue;
                }
                let raw = game.realm_colour.get(id).copied().unwrap_or(0);
                let colour = chrome::realm_colour(raw);
                c.draw_banner(canvas, slot, colour);
                slot += 1;
            }
        }
        None => widget::panel(canvas, ink, Rect::new(0, 0, canvas.width as i32, TOP_BAR)),
    }

    // `Screen_DrawMenuBar` touches neither `DAT_005AEA40` nor `DAT_0058FE2C`
    // around these three, so it is the ordinary embossed body pen — the same
    // one `menubar::draw_titles` uses two lines below.
    let pen = crate::shell::Pen {
        assets: &ctx.assets.shell,
        ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };

    // `Ui_DrawYear(g_year, 0x168, 6, 3)` — style 3 is
    // `Ui_DrawNumber(year, ' ', &DAT_004D41F0, x, y, &g_fontBody, 0x3F)`, the
    // bare number with a leading and a trailing space and no BC/AD.
    let after_year = pen.year(canvas, CLOCK_X, CLOCK_Y, k.year, 3, font::TEXT);
    // `Eng_DrawString(0x1D, g_season, g_penAdvance + 0x16C, 6, &g_fontBody, 0x3F)`.
    //
    // **`g_penAdvance` is a width and `Pen::year` returns an absolute x** —
    // the confusion `docs/decisions.md` C61 records four agents making seven
    // times. The subtraction is written out rather than folded away so the line
    // reads the way the decompilation does.
    let advance = after_year - CLOCK_X;
    let season = season_text(ctx.assets, k.season);
    pen.body(canvas, SEASON_X + advance, CLOCK_Y, &season, font::TEXT);
    // `Ui_DrawCount(g_realms[g_localPlayer].gold, 0, 500, 6, &g_fontBody, 0x3F)`
    // — the number, then group 8 index 0 or 1, *"Crown."* or *"Crowns."*.
    // `Ui_DrawCount` opens the number with `'@'`, so the digits start at 504,
    // not at 500; they were four pixels left until `Pen::count` stopped taking
    // a lead of its caller's.
    pen.count(canvas, GOLD_X, CLOCK_Y, game.gold(), GOLD_NOUN, font::TEXT);

    // `Ui_DrawMenuTitles(&g_menuBarItems, 3)`. Nothing here is open — the
    // drop-down is its own screen and draws its own title lit.
    menubar::draw_titles(ctx, canvas, None);

    // **OURS, and it has been evicted from the menu bar.**
    //
    // The turn number and the counties-held count were at x = 6 and x = 150,
    // which is where the original draws *File*, *Options* and *Help* — so two
    // lines of ours were sitting on the three words that are the way into every
    // menu in the game, and neither was visible with the real fonts loaded.
    // There is no room for them: the bar holds three measured titles, up to five
    // 13 × 16 realm banners from x = 270, the clock at 360 and the treasury at
    // 500, and every one of those is `Screen_DrawMenuBar`'s.
    //
    // So they go **under** the bar, on the map's own top-left corner, where
    // nothing of the original's is drawn. Still ours, still marked, and now they
    // cannot hide a control.
    //
    // And now debug overlay only: the original draws nothing there at all.
    if game.prefs.debug_overlay {
        text::draw(canvas, 6, 28, &format!("TURN {}", k.turn_count), ink.dim);
        let held = format!("COUNTIES {}/{}", game.owned_by(game.player), k.county_count);
        text::draw(canvas, 6, 38, &held, ink.dim);
    }
}

/// The right column: the original's seven `Misc_cty` frames, our numbers inside
/// them.
fn draw_right_panel(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx) {
    let ink = &ctx.assets.ink;
    let game = &ctx.game;
    let k = &game.kingdom;
    let own = game.selected != 0
        && (game.selected as usize) < k.counties.len()
        && k.counties[game.selected as usize].owner == game.player;

    match &ctx.assets.chrome {
        Some(c) => {
            c.draw_right_panel(canvas, own);
        }
        None => widget::panel(canvas, ink, PANEL),
    }

    // The minimap, tinted from `Lords2.exe`'s own ramps.
    if let Some(m) = &screen.minimap {
        let owner = |county: u8| -> u8 {
            let id = county as usize;
            if id == 0 || id >= k.counties.len() {
                return 0;
            }
            // Ramp row 0 is the unowned shading — the raster's own indices,
            // unchanged — so an unowned county must reach it, and only an owned
            // one goes through the 1..=5 clamp.
            match k.counties[id].owner as usize {
                0 => 0,
                realm => chrome::realm_colour(game.realm_colour.get(realm).copied().unwrap_or(0)),
            }
        };
        // **The three statistic overlays colour the local player's counties and
        // nothing else** — `Minimap_DrawOverlay` tests `owner == g_localPlayer`
        // in each of its three branches and skips the pixel otherwise, so a
        // rival's county keeps the raster's own grey.
        let band = |county: u8| -> Option<u8> {
            let id = county as usize;
            let c = k.counties.get(id)?;
            if id == 0 || c.owner != game.player {
                return None;
            }
            let bands = c.minimap_bands();
            Some(match screen.minimap_mode {
                MinimapMode::Labour => bands.labour,
                MinimapMode::Food => bands.food,
                MinimapMode::Happiness => bands.happiness,
                MinimapMode::Owner => return None,
            })
        };
        let tint = match screen.minimap_mode {
            MinimapMode::Owner => MinimapTint::Owner(&owner),
            _ => MinimapTint::Rating(&band),
        };
        chrome::draw_minimap(canvas, m, game.selected, &tint);
        if let Some(c) = &ctx.assets.chrome {
            // `Minimap_Draw` draws the strip and then the badge, both after the
            // overlay, so both sit on top of it.
            c.draw_minimap_side(canvas, screen.minimap_mode);
            c.draw_minimap_badge(canvas, screen.minimap_mode);
        }
    } else {
        // No `MAPnn.PL8`: say so rather than leaving an unexplained hole.
        text::draw_centred(canvas, PANEL_X + PANEL_W / 2, 84, "NO MINIMAP", ink.dim);
    }

    // **The county strip, at the original's own coordinates.** `Misc_cty` frame
    // 55 is the 162 x 94 plate at (478, 156) and `CountyStrip_Draw`
    // (`0x0040F7D3`) fills it: the county's name, its population and happiness,
    // the tax rate, the achieved ration — red when it is not the wanted one —
    // and the five-level health thermometer. Every coordinate is
    // `docs/screens-county.md` §2.1 and the code is shared with the county
    // screen, which draws the same plate.
    //
    // This plate used to be left empty while a box of ours went over the jobs
    // plate below it. The player was looking at our text where the game's own
    // numbers belong, and at four blank quadrants that are in fact the menu.
    if game.selected != 0 {
        // The strip, and the split slider's thumb on the plate below it — both
        // `CountyStrip_Draw`'s, both shared with the county screen.
        county::draw_strip(ctx, canvas, game.selected, None);
    } else if game.prefs.debug_overlay {
        text::draw_centred(canvas, PANEL_X + 80, 200, "NO COUNTY SELECTED", ink.dim);
    }

    // **Ours, and it should look it.** `0x00438E3B` turns the 162 x 128 plate
    // at y = 302 into two columns of job rows — farm jobs left of x = 560,
    // industry right of it — and a click opens the job popup for that job
    // (`docs/screens-county.md` §2.4). We do not lay those rows out yet, so the
    // plate carries a dark box of ours with the county's stores in it and the
    // one status line this interface has. A stub that says so beats one that
    // looks finished.
    //
    // **Both columns are now drawn**, by `county::draw_produce_rows` and
    // `county::draw_industry_rows` with the rest of the strip. The left is
    // where the blue outline lives — the cow gains a ring the moment more
    // people are milking than the herd can use — and the right is the five
    // industry rows, which stood behind a box of ours reading
    // `INDUSTRY / NOT DRAWN` until the seventeen draw calls under it were
    // enumerated. The reason the box was there — *"three of its five rows are
    // flat icons and the other two pick their frame from bytes this project has
    // not settled"* — was checkable and wrong on both halves; see
    // `county::draw_industry_rows`.
    //
    // What is left here is the one status line this interface has, which is
    // ours and is marked so in §7's count.
    let x = PANEL_X + 8;
    if game.prefs.debug_overlay {
        text::draw(canvas, x, chrome::PANEL_OWN_C_Y + 110, &screen.status, ink.dim);
    }

    // **The five sidebar buttons.** `Misc_cty` frame 57 already drew them; all
    // this adds is which one the pointer is over — an outline and a caption of
    // ours. **Debug overlay only.** A player: *"still seeing debug outlines and
    // text for the 4 icons at the bottom right … it's not in the OG."*
    // `Sidebar_ButtonClicked` is one `Hotspot_Test`, which draws nothing, and
    // `Screen_DrawCampaign` paints the strip as one frame with nothing over it.
    // The original's words for these buttons are the tooltip layer's
    // (`FUN_00476E95`, group 220), which is not built.
    if let (Focus::Sidebar(i), true) = (screen.focus, game.prefs.debug_overlay) {
        let r = SIDEBAR_BUTTONS[i].rect();
        widget::frame(canvas, r, ink.highlight);
        text::draw_centred(canvas, r.centre_x(), r.y - 10, SIDEBAR_BUTTONS[i].name, ink.highlight);
    }

    // `Screen_DrawEndTurn` (`0x0041A734`):
    // `Ui_DrawCentred(4, 0, 0x1DE, 0x1CE, 0xA2, &g_fontSmall, 0x16)` — `L2.eng`
    // group 4, centred in 162 pixels at (478, 462), in the strip's own 9-pixel
    // font. We had it two lines lower and in words of ours.
    //
    // # The label goes away while the turn runs, and that is a *conditional
    // draw*, not a pressed frame
    //
    // A player: *"in the original, the text 'END TURN' would disappear when you
    // click it, until the new turn was ready."* He is exactly right, and the
    // original's shape is one line:
    //
    // ```c
    // Pl8_DrawFrameHere(g_miscCtySheet, 0x3B, 0x1DE, 0x1CC);       /* the strip, always */
    // if (g_realms[g_localPlayer].aiStep < 999)
    //     Ui_DrawCentred(4, 0, 0x1DE, 0x1CE, 0xA2, &g_fontSmall, 0x16);
    // ```
    //
    // The strip is opaque artwork and is blitted unconditionally, so drawing it
    // *is* the erase; the label is simply not put back. **The button does not
    // move, is not redrawn pressed, and is not removed by the sidebar's own
    // gate** — the other three explanations that fitted the report.
    //
    // **The flag is `aiStep`, and it is a per-realm turn program counter rather
    // than a boolean.** `Turn_BeginPlayersTurn` sets every living realm's to 0
    // and a dead one's to 999; `AI_RunTurnStep` walks it up and parks it at 999
    // when that realm is finished; `Turn_End` (`0x0043AC23`) sets the local
    // player's to 999 the instant this button is clicked. So **999 means "this
    // realm's turn is over"** and the label's absence is the interval between
    // the click and the next turn beginning — the player's sentence, verbatim.
    //
    // `turn::turn_in_flight` is that interval here, and the mapping is exact
    // rather than approximate: `game.turn` is `Some` from the click until the
    // turn completes, which is when `Turn_BeginPlayersTurn` would clear the
    // counter. **This is a draw that only became possible to reproduce when the
    // turn started being paced over frames** — before that there was no
    // interval to be inside.
    //
    // **Two other things read the same flag.** `Screen_DrawMenuBar`'s banner
    // loop is `strength != 0 && aiStep < 999`, so each realm's banner vanishes
    // from the menu bar as that realm finishes its turn and the bar refills as
    // the new one begins — reproduced in `draw_menu_bar`, through
    // `turn::realm_turn_ended`. And `FUN_0041A639`'s turn timer reads it too —
    // as one half of `DAT_0055403C < 1 || aiStep == 999`, which keeps the
    // timer up through the person's own turn *and* after he ends it. That one
    // is drawn by `Machine::draw`, not here, because the original calls it from
    // the frame loop rather than from this screen's painter; see
    // `crate::turn_clock`. This paragraph used to say we had no turn timer, and
    // before that that we reproduced neither of the other two, and both times it
    // sat twenty lines from the draw it described as missing.
    // `docs/draws-map.md` §5.11, `docs/decisions.md`
    // C152 and C158.
    if !turn::turn_in_flight(&ctx.game) {
        // The hover colour is ours: `Screen_DrawEndTurn` passes `0x16` whatever
        // the pointer does. Debug overlay only.
        let end = if screen.focus == Focus::EndTurn && game.prefs.debug_overlay {
            ink.highlight
        } else {
            ink.text
        };
        let label = ctx.assets.shell.text(4, 0);
        let label = if label.is_empty() { "END TURN" } else { label };
        county::strip_centred_at(ctx, canvas, PANEL_X, 462, PANEL_W, label, end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout constants, checked against each other rather than restated.
    /// The panel starts where the map's clip ends and reaches the screen edge,
    /// and the two strips at the bottom of the panel meet exactly.
    #[test]
    fn the_screen_is_partitioned_with_no_gap_and_no_overlap() {
        assert_eq!(
            MAP_AREA.x + MAP_AREA.w,
            PANEL.x,
            "the map stops where the panel starts"
        );
        assert_eq!(PANEL.x + PANEL.w, 640);
        assert_eq!(PANEL.y, TOP_BAR);
        for b in SIDEBAR_BUTTONS {
            let r = b.rect();
            // **They do not meet.** `g_sidebarButtons` records 0…4 stop at
            // y 458 and record 5 starts at 460, so y 459 is a dead row — and
            // this assertion used to require the opposite, which is how the
            // strip came to be a pixel taller than the table. The plates meet;
            // the hotspots do not, and only the hotspots decide a click.
            assert_eq!(
                r.y + r.h + 1,
                END_TURN_BUTTON.y,
                "{b:?}: the table leaves one dead row above the end-turn strip",
            );
            assert!(PANEL.contains(r.x, r.y), "{b:?} starts outside the sidebar");
            assert!(r.x + r.w <= PANEL.x + PANEL.w, "{b:?} runs past the screen edge");
        }
        // 479, not 480: record 5's `y1` is 49 at offset `0x1AE` and
        // `Hotspot_Test` is half-open, so the bottom row of the screen is dead
        // here too.
        assert_eq!(END_TURN_BUTTON.y + END_TURN_BUTTON.h, 479);
        assert_eq!(END_TURN_BUTTON.x + END_TURN_BUTTON.w, 639, "and the last column with it");
        // The map's clip is the zoom's, and it stops at 478 at both zooms.
        for z in [NEAR, FAR] {
            assert_eq!(z.clip().x1, PANEL.x);
            assert_eq!(z.clip().y0, TOP_BAR);
        }
        // The minimap lives inside the panel, and nothing outside it is
        // clickable as minimap.
        let hit = chrome::minimap_hit_area();
        assert!(PANEL.contains(hit.x0, hit.y0));
        assert!(PANEL.contains(hit.x1 - 1, hit.y1 - 1));
    }

    /// `g_sidebarButtons`' five records, checked against the arithmetic the
    /// table itself closes on: the offsets and the half-open ends give five
    /// widths that, with the one-pixel gaps, tile the 162-pixel strip exactly.
    #[test]
    fn the_five_sidebar_buttons_tile_the_strip_and_all_five_name_a_screen() {
        let ends = [33, 65, 97, 129, 161];
        for (b, end) in SIDEBAR_BUTTONS.iter().zip(ends) {
            assert_eq!(b.x + b.w, end, "{b:?} does not end where the table says");
        }
        // Half-open, so a pixel belongs to at most one of them.
        for (i, a) in SIDEBAR_BUTTONS.iter().enumerate() {
            for b in SIDEBAR_BUTTONS.iter().skip(i + 1) {
                let (ra, rb) = (a.rect(), b.rect());
                assert!(ra.x + ra.w <= rb.x || rb.x + rb.w <= ra.x, "{a:?} overlaps {b:?}");
            }
        }
        // And every destination is a screen we can actually draw.
        //
        // **This check used to be *"either a shell or a graduated screen, and
        // not both"*, and the shell table is empty now**, so the first half is
        // gone and the second is all of it: a button whose id falls through
        // `sidebar_destination`'s ladder lands on `ScreenId::Campaign` — the
        // screen it was already on — which is a button that does nothing, and
        // that is exactly what this must catch. It is the same failure the old
        // form caught (a graduation that forgot to add an arm), stated against
        // the fall-through instead of against the table.
        for b in SIDEBAR_BUTTONS {
            let SidebarAction::Screen(id) = b.action;
            assert_ne!(
                sidebar_destination(id, 1),
                ScreenId::Campaign,
                "{b:?} names screen {id:#04X}, which sidebar_destination does not build",
            );
        }
        assert_eq!(
            sidebar_destination(0x17, 4),
            ScreenId::RaiseArmy(4),
            "the ARMY button opens the raise-army screen for the selected county",
        );
    }

    /// The minimap's four mode buttons, including the record whose `y1` is
    /// `0x42` where the pattern wants `0x3F`. That is the original's data and
    /// the overlap is transcribed rather than tidied: `Hotspot_Test` returns on
    /// the first match, so y 96 and 97 belong to band 2.
    #[test]
    fn the_minimap_mode_buttons_are_the_originals_including_its_off_by_three() {
        for r in MINIMAP_MODE_BUTTONS {
            assert!(r.x >= PANEL.x && r.x + r.w <= 640, "{r:?} escapes the sidebar");
            assert!(r.y >= TOP_BAR, "{r:?} is under the menu bar");
        }
        let second = MINIMAP_MODE_BUTTONS[1];
        let third = MINIMAP_MODE_BUTTONS[2];
        assert_eq!(second.y + second.h, third.y + 2, "band 2 runs two rows into band 3");
        let first = MINIMAP_MODE_BUTTONS[0].contains(620, 96);
        assert!(!first);
        assert!(second.contains(620, 96), "and the first match wins, so y 96 is mode 2");
    }

    /// `FUN_00439122`, the farm/industry split slider, arithmetic and all.
    ///
    /// The mask is the part a reimplementation drops: on the track the value is
    /// `((x - 531) * 2) & 0xFC`, so it lands on a multiple of four and the
    /// slider is not continuous. Off the track it steps by four instead.
    #[test]
    fn the_split_slider_snaps_to_fours_on_the_track_and_steps_by_four_off_it() {
        assert_eq!(split_from_click(531, 50), 0, "the track starts at x 531");
        assert_eq!(split_from_click(581, 50), 100, "and ends at 581");
        assert_eq!(split_from_click(532, 50), 0, "((1)*2) & 0xFC");
        assert_eq!(split_from_click(533, 50), 4, "((2)*2) & 0xFC");
        assert_eq!(split_from_click(534, 50), 4, "and 6 masks back to 4");
        assert_eq!(split_from_click(530, 50), 46, "left of the track steps down");
        assert_eq!(split_from_click(600, 50), 54, "right of it steps up");
        assert_eq!(split_from_click(530, 0), 0, "and both ends clamp");
        assert_eq!(split_from_click(600, 100), 100);
    }

    /// **Edge scrolling.** The eight directions come from which edges the
    /// pointer is touching, and nothing else on the screen scrolls.
    ///
    /// The pixel positions are the *canvas's*, and that is the whole point: the
    /// window is letterboxed by integer scaling, and `main.rs` clamps a pointer
    /// in the black border to the nearest canvas pixel, so pushing the cursor
    /// into the border of a window of any size arrives here as 0 or 639.
    #[test]
    fn the_pointer_at_an_edge_asks_for_the_direction_that_edge_means() {
        let mut s = MapScreen::new();
        s.pointer_in = true;
        let cases = [
            ((320, 0), Some(Dir::N)),
            ((CANVAS_W - 1, 0), Some(Dir::NE)),
            ((CANVAS_W - 1, 240), Some(Dir::E)),
            ((CANVAS_W - 1, CANVAS_H - 1), Some(Dir::SE)),
            ((320, CANVAS_H - 1), Some(Dir::S)),
            ((0, CANVAS_H - 1), Some(Dir::SW)),
            ((0, 240), Some(Dir::W)),
            ((0, 0), Some(Dir::NW)),
            ((320, 240), None),
            ((1, 1), None),
            ((CANVAS_W - 2, CANVAS_H - 2), None),
        ];
        for (at, want) in cases {
            s.pointer = at;
            assert_eq!(s.edge_direction(), want, "pointer at {at:?}");
        }
        // A cursor that has left the window does not go on scrolling.
        s.pointer = (0, 240);
        s.pointer_in = false;
        assert_eq!(s.edge_direction(), None, "the pointer is not over the window");
    }

    /// **The gesture, driven the way the player drives it: as pointer
    /// positions.**
    ///
    /// The three cases are the three that matter, and the middle one is the bug
    /// he reported. The window is scaled by a whole number and centred, so it
    /// has black borders unless it is an exact multiple of 640 × 480 — and
    /// [`crate::input::window::to_canvas`] clamps a position in the border to
    /// the nearest canvas pixel, so *pushing the cursor into the black band
    /// arrives here as column 0 or column 639*. That is asserted there; this
    /// asserts what the map then does with it.
    #[test]
    fn a_pointer_at_the_edge_scrolls_and_a_pointer_just_inside_it_does_not() {
        use crate::input::window;
        let assets = crate::game::Assets::placeholder();
        let start = Viewport::new(40, 20);

        // The window the bug was reported from: 1898 x 1562, picture 1280 x 960
        // with 309 pixels of border on each side.
        let (ww, wh) = (1898u32, 1562u32);
        let run = |at: (f64, f64)| -> Viewport {
            let mut s = MapScreen::new();
            s.view = start;
            let (x, y) = window::to_canvas(ww, wh, at.0, at.1);
            let mut ctx = Ctx { game: &mut crate::Game::new(1), assets: &assets };
            s.handle(Event::Pointer { x, y }, &mut ctx);
            s.update(&mut ctx);
            s.viewport()
        };

        // 1. The content's own left and right edges.
        assert_eq!(run((309.0, 781.0)), Viewport::new(40, 19), "the picture's left column");
        assert_eq!(run((1588.0, 781.0)), Viewport::new(40, 21), "and its right one");

        // 2. **The border scrolls too.** This is what was broken: the player
        //    ran out of picture before he ran out of window, and the gesture
        //    died in the black band.
        assert_eq!(run((0.0, 781.0)), Viewport::new(40, 19), "the far left of the window");
        assert_eq!(run((1897.0, 781.0)), Viewport::new(40, 21), "the far right");
        // The top and bottom borders move the view by two rows, which is one
        // map tile — `Map_ScrollStep`'s own step.
        assert_eq!(run((949.0, 0.0)), Viewport::new(38, 20), "the top");
        assert_eq!(run((949.0, 1561.0)), Viewport::new(42, 20), "the bottom");
        // And a corner of the window is a diagonal, as `Map_EdgeScroll` has it.
        assert_eq!(run((0.0, 0.0)), Viewport::new(38, 19), "the top-left corner");

        // 3. One pixel inside the picture is not an edge.
        assert_eq!(run((311.0, 781.0)), start, "one canvas pixel in from the left");
        assert_eq!(run((1586.0, 781.0)), start, "and from the right");
        assert_eq!(run((949.0, 781.0)), start, "and the middle of the screen");

    }

    /// Holding the pointer at the edge scrolls the map, **at the original's
    /// rate and not at ours**, and reports that it did so the machine
    /// repaints. That report is what makes the gesture continuous rather than
    /// one step per mouse move.
    ///
    /// This used to be called `..._scrolls_every_tick_...` and asserted exactly
    /// that: one tile per fixed tick, which is 62.5 a second. The original's
    /// `Map_ScrollThrottle` gives 20 at the shipped default, so we were **three
    /// times too fast** — the defect a player reported as *"mouse scroll needs
    /// to be like… half that speed"*. The interval is now the original's
    /// formula, and this asserts the pacing rather than assuming there is none.
    #[test]
    fn holding_the_pointer_at_the_edge_scrolls_at_the_originals_rate() {
        let mut game = crate::Game::new(1);
        let assets = crate::game::Assets::placeholder();
        let mut s = MapScreen::new();
        s.view = Viewport::new(40, 20);
        s.pointer = (CANVAS_W - 1, 240);
        s.pointer_in = true;

        let every = s.scroll_interval_ticks();
        assert_eq!(every, 3, "the default 60 is 50 ms, which is three of our 16 ms ticks");

        let mut moved = 0;
        for tick in 1..=every * 3 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            s.update(&mut ctx);
            if s.viewport().col > 20 + moved {
                moved += 1;
                assert!(s.take_redraw(), "a tick that moved the map has to be drawn");
            }
            assert_eq!(
                s.viewport(),
                Viewport::new(40, 20 + moved),
                "tick {tick}: one tile every {every} ticks and no more",
            );
        }
        assert_eq!(moved, 3, "three steps in nine ticks, not nine");

        // In the middle of the screen nothing happens at all, however many
        // ticks go by.
        s.pointer = (240, 240);
        let settled = s.viewport();
        for _ in 0..every * 2 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            s.update(&mut ctx);
        }
        assert_eq!(s.viewport(), settled);
    }

    /// **The whole of `Map_ScrollThrottle`'s ladder**, at every setting the
    /// slider can produce: `((100 − speed) / 10) × 12 + 2` milliseconds, and
    /// speed 0 never scrolls at all.
    #[test]
    fn the_scroll_throttle_reproduces_the_originals_interval_at_every_setting() {
        let mut s = MapScreen::new();
        // speed, the original's interval in ms, and our tick count for it.
        let table = [
            (0, None, u32::MAX),
            (10, Some(110), 7),
            (20, Some(98), 6),
            (30, Some(86), 5),
            (40, Some(74), 5),
            (50, Some(62), 4),
            (60, Some(50), 3),
            (70, Some(38), 2),
            (80, Some(26), 2),
            (90, Some(14), 1),
            (100, Some(2), 1),
        ];
        for (speed, ms, ticks) in table {
            s.set_scroll_speed(speed);
            if let Some(ms) = ms {
                let q = (100 - speed) / 10;
                assert_eq!(q * 12 + 2, ms, "speed {speed}: the original's own arithmetic");
            }
            assert_eq!(s.scroll_interval_ticks(), ticks, "speed {speed}");
        }
        // Out of range is clamped rather than wrapped, at both ends.
        s.set_scroll_speed(-40);
        assert_eq!(s.scroll_interval_ticks(), u32::MAX, "below zero is still 'never'");
        s.set_scroll_speed(4_000);
        assert_eq!(s.scroll_interval_ticks(), 1);
    }

    /// Speed 0 is a real setting: `Map_ScrollThrottle` returns 0 for ever, so
    /// the map does not scroll however long the pointer is held at the edge.
    #[test]
    fn scroll_speed_zero_never_scrolls() {
        let mut game = crate::Game::new(1);
        let assets = crate::game::Assets::placeholder();
        let mut s = MapScreen::new();
        s.view = Viewport::new(40, 20);
        s.pointer = (CANVAS_W - 1, 240);
        s.pointer_in = true;
        s.set_scroll_speed(0);
        for _ in 0..200 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            s.update(&mut ctx);
        }
        assert_eq!(s.viewport(), Viewport::new(40, 20));
    }

    /// **Every pixel around a tile centre picks a tile.**
    ///
    /// `pick_tile` resolves a pixel against the diamond around each tile
    /// centre, and the half-extents have to be the **lattice**'s — the half
    /// pitch and the row step — for those diamonds to tile the plane. They were
    /// `tile_w / 2` and `tile_h / 2`, and at the near zoom the tile is 58 wide
    /// while the pitch is 60: two pixels narrow.
    ///
    /// The seams are **sparse** and are not on the line between two tile
    /// centres, which is why a coarser test than this one passes with the bug
    /// still in. At the near zoom the first missing pixel is 28 across and 1
    /// down from a centre: with `hw = 29` its own tile scores
    /// `28×15 + 1×29 = 449 > 435` and the neighbour scores `436 > 435`, so it
    /// belongs to nobody by one unit. So this sweeps a tile's whole
    /// neighbourhood — 56 pixels of it were dead — and requires all of it to
    /// resolve.
    #[test]
    fn the_diamonds_leave_no_pixel_unpicked() {
        for zoom in [NEAR, FAR] {
            let mut s = MapScreen::new();
            s.zoom = zoom;
            s.view = Viewport::new(60, 30).clamped(&zoom);
            // A tile whose neighbours are all comfortably on screen **and all
            // on the map**. The grid's own edge is a real hole and not a seam:
            // there is no tile beyond it to pick.
            let (tx, ty) = (1..l2_kingdom::MAP_DIM - 1)
                .flat_map(|y| (1..l2_kingdom::MAP_DIM - 1).map(move |x| (x, y)))
                .find(|&(x, y)| {
                    [(x, y), (x + 1, y), (x, y + 1), (x - 1, y), (x, y - 1)].iter().all(|&(a, b)| {
                        campaign::tile_centre(s.view, &zoom, a, b).is_some_and(|(cx, cy)| {
                            let c = s.map_clip();
                            cx - zoom.pitch > c.x0
                                && cx + zoom.pitch < c.x1
                                && cy - zoom.row_step * 2 > c.y0
                                && cy + zoom.row_step * 2 < c.y1
                        })
                    })
                })
                .expect("a tile with room around it");
            let (cx, cy) = campaign::tile_centre(s.view, &zoom, tx, ty).expect("in view");

            let mut misses = Vec::new();
            for dy in -zoom.row_step..=zoom.row_step {
                for dx in -zoom.half_pitch..=zoom.half_pitch {
                    if s.pick_tile(cx + dx, cy + dy).is_none() {
                        misses.push((dx, dy));
                    }
                }
            }
            assert!(
                misses.is_empty(),
                "zoom {}: {} pixels around a tile centre pick nothing, first at {:?} \
                 — the diamonds do not tile the plane",
                zoom.id,
                misses.len(),
                misses.first(),
            );
        }
    }

    /// **The half-extents are the lattice's**, which is the arithmetic the test
    /// above rests on, stated so a reader does not have to derive it from a
    /// pixel sweep. `Map_PickTile` divides by `g_mapTileHalfStep` and
    /// `g_mapRowStep`: the *pitch* halved and the row step, not the picture's
    /// width and height halved.
    #[test]
    fn the_pick_diamond_is_the_lattice_and_not_the_picture() {
        for zoom in [NEAR, FAR] {
            assert_eq!(zoom.half_pitch * 2, zoom.pitch, "zoom {}", zoom.id);
        }
        // The near zoom is the case that was wrong: 58 / 2 = 29, pitch / 2 = 30.
        assert_eq!(NEAR.tile_w / 2, 29);
        assert_eq!(NEAR.half_pitch, 30);
        assert_eq!(FAR.tile_w / 2, 5);
        assert_eq!(FAR.half_pitch, 6);
    }

    /// The screen opens where `Map_InitMode` opens: near zoom, row 0x4A,
    /// col 0x14. If that ever silently became "the whole map", this fails.
    #[test]
    fn the_screen_opens_at_the_originals_zoom_and_scroll_origin() {
        let s = MapScreen::new();
        assert_eq!(s.zoom().id, NEAR.id);
        assert_eq!(s.viewport(), Viewport::new(0x4A, 0x14));
        assert_eq!(s.map_clip(), NEAR.clip());
    }

    /// Zooming out and back in returns the near view where it was —
    /// `Map_ZoomOut` saves the origin and `Map_ZoomIn` restores it — and the
    /// far view lands on the original's own fixed position.
    #[test]
    fn the_zoom_toggle_saves_and_restores_the_near_views_position() {
        let assets = crate::game::Assets::placeholder();
        let mut game = crate::Game::new(1);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        let mut s = MapScreen::new();
        s.view = Viewport::new(60, 30);
        s.toggle_zoom(&mut ctx);
        assert_eq!(s.zoom().id, FAR.id);
        // **The projection**: `g_mapZoom` is a global three arms outside this
        // screen read, and `Game::map_zoom_far` is where they read it. Asserted
        // here rather than only in `tests/right_column.rs` because this is the
        // function that writes it.
        assert!(ctx.game.map_zoom_far, "the far zoom did not reach Game");
        assert_eq!(
            s.viewport(),
            Viewport::new(0, 14),
            "row clamps to 0, col 0x0E stands"
        );
        // And the far view cannot be scrolled off that position.
        assert!(!s.scroll(Dir::E));
        assert!(!s.scroll(Dir::N));
        s.toggle_zoom(&mut ctx);
        assert_eq!(s.zoom().id, NEAR.id);
        assert!(!ctx.game.map_zoom_far, "the near zoom did not reach Game");
        assert_eq!(s.viewport(), Viewport::new(60, 30), "back where it was");
        assert!(s.scroll(Dir::E), "and the near view scrolls again");
        assert_eq!(s.viewport(), Viewport::new(60, 31));
    }

    /// **Picking up an army is `g_screenId` `0x10`**, and putting it down is
    /// the map's own `0` again — `Map_BeginMoveSelection` and the three writes
    /// of `0` that leave it. `Tip_Update`'s *"Army Movement:"* arm reads it.
    ///
    /// Ablation: make `mode_screen_id` answer `None` and the middle assertion
    /// goes red.
    #[test]
    fn picking_up_an_army_is_screen_0x10_and_putting_it_down_is_not() {
        let assets = crate::game::Assets::placeholder();
        let mut game = crate::Game::new(1);
        let unit = game
            .kingdom
            .campaign
            .units
            .spawn(l2_kingdom::unit::Unit::new(l2_kingdom::UnitKind::Army, 1, 10, 10))
            .expect("a free slot");
        let mut s = MapScreen::new();
        assert_eq!(s.mode_screen_id(), None, "the map is 0, which its ScreenId already says");
        let ctx = Ctx { game: &mut game, assets: &assets };
        s.begin_move_selection(&ctx, unit);
        assert_eq!(s.mode_screen_id(), Some(0x10));
        s.cancel_move_selection();
        assert_eq!(s.mode_screen_id(), None);
    }
}
