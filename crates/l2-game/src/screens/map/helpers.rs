#![allow(unused_imports)]
use super::*;
use super::sidebar::*;
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

/// **The field markers, which are debug overlay now.**
///
/// This module used to hold a brush *popup* of ours on the campaign map, opened
/// by a left click on a field. It is gone: in the original that click is
/// `Map_Click`'s farmland arm — `_DAT_005681CC = 3; g_screenId = 4;
/// FUN_0041B032();` — which opens **screen `0x04`, the same information panel a
/// right click opens**.
/// (`FUN_0041C996`, `FUN_00438990`). See [`crate::screens::info`].
///
/// What is left is the squares we drew on the county's fields, coloured by what
/// each is used for. **The original draws nothing there** — `Sprite_TopIt`'s
/// farm arm is the pasture herd and nothing else.
/// artwork (`Terrain_Set`, which [`MapScreen::field_graphics`] reproduces) — so
/// they are drawn only with [`crate::game::Prefs::debug_overlay`] on.
mod brush {
    /// Half-width of a field marker. **Ours**, debug overlay only.
    pub const FIELD_MARKER: i32 = 3;
}

/// **Where the town's 2 × 2 block is re-stamped to, by the county's own
/// population.** `FUN_0046ac22` is called with `'/'`, `'3'` or `'7'` — 47, 51
/// and 55 —
///
/// The thresholds are the original's literals `0x321` and `0x4b1`, tested as
/// `pop < 801` and `pop < 1201`.
pub(super) const TOWN_FRAME_BASE: [(i32, u8); 3] = [(801, 47), (1201, 51), (i32::MAX, 55)];

/// The plane-1 byte a town tile carries: bank `0x0c`, `Town1a.pl8`.
pub(crate) const TOWN_BANK: u8 = 0x0c;

/// One fixed simulation tick in milliseconds — `main::TICK`.
///
/// **This is a constant, not a clock.** Nothing here asks how long a frame
/// took; the number exists so an interval the original states in
/// milliseconds can be converted to the whole ticks this crate is allowed to
/// count. `VillageScreen::CLICK_SETTLE_TICKS` makes the same conversion by
/// hand and for the same reason (`docs/netcode.md`).
pub(crate) const TICK_MS: u32 = 16;

/// `g_optScrollSpeed`'s shipped default, written by the options-defaults
/// routine at `0x004AE310`. **[V]**
pub const DEFAULT_SCROLL_SPEED: i32 = 60;

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

/// `Eng_DrawString(101, g_scenarioIndex, …)` — the map's own name, from the
/// player's own file,
pub fn map_name(ctx: &Ctx) -> String {
    let s = ctx.assets.shell.text(FAR_BOX_MAP_GROUP, ctx.game.map_slot);
    if s.is_empty() {
        return format!("MAP {}", ctx.game.map_slot);
    }
    s.to_string()
}

