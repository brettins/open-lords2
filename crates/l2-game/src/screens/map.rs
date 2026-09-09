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
//! `Map_DrawCountyFlag` clips the map to `x < 478`, `y < 474`.
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
//! game's `Fntl2_*.pl8`), the yellow county outline, the county marker squares,
//! the keys that scroll, and the layout of the numbers inside the right panel's
//! frames. The original also *measures* the selected county — `Map_DrawFrame`
//! tallies how much of the viewport each county fills and takes the maximum —
//! and we do not; here the selection only changes when the player clicks.
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

use l2_kingdom::field::{self, BrushRefusal, FieldType};
use l2_kingdom::industry;
use l2_formats::maps::Plane;
use l2_view::campaign::{self, Dir, Lattice, Viewport, Zoom, FAR, NEAR, PANEL_W, PANEL_X};
use l2_view::chrome::{self, Minimap};
use l2_view::{text, Canvas, Clip, Ink, Tags};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::county;
use crate::screens::saveload::Mode as SaveLoadMode;
use crate::turn;
use crate::widget;

/// The menu bar: `Screen_DrawMenuBar`'s 640 × 24 strip at y 0.
pub const TOP_BAR: i32 = campaign::TOP_BAR_H;

/// The screen the original ran at, and the only size a canvas ever is.
pub const CANVAS_W: i32 = l2_view::canvas::WIDTH as i32;
pub const CANVAS_H: i32 = l2_view::canvas::HEIGHT as i32;

/// The right column: `Misc_cty` frames 54 … 59 at x 478, 162 wide.
pub const PANEL: Rect = Rect::new(PANEL_X, TOP_BAR, PANEL_W, 480 - TOP_BAR);

/// The End Turn strip — `Misc_cty` frame 59 (162 × 20) at (478, 460), with
/// `L2.eng` group 4 centred in it.
pub const END_TURN_BUTTON: Rect = Rect::new(
    PANEL_X,
    chrome::PANEL_END_TURN_Y,
    PANEL_W,
    480 - chrome::PANEL_END_TURN_Y,
);

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
pub const SIDEBAR_BUTTONS: [SidebarButton; 5] = [
    // `Levy_SetPercent(sel, g_levyPercent); FUN_004AA90A(sel, g_levyMen);
    // g_screenId = 0x17`, and the mercenary band is loaded on top of it when
    // the county has an offer. `L2.eng` group 69 index 0x10 is "Raising an
    // army in", so 0x17 is the **raise-army** screen and the mercenary offer is
    // an optional half of it — `shells.rs` calls the whole screen "Hire
    // mercenaries", which is the smaller half naming the larger.
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
/// (`g_minimapRatingRamp`, `0x004D28F8`) we have not transcribed. The fourth is
/// **the zoom toggle**, and it is the control `docs/screens.md` §7 says we
/// replaced with the `Z` key; it is wired.
///
/// The second record's `y1` is `0x42` where the pattern wants `0x3F`, so band 2
/// is 34 pixels tall and overlaps band 3's first two rows. `Hotspot_Test`
/// returns on the first match, so y 96 and 97 select mode 2. **That is the
/// original's own data**, transcribed rather than tidied.
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
        Rect::new(
            PANEL_X + self.x,
            chrome::PANEL_STATUS_Y,
            self.w,
            chrome::PANEL_END_TURN_Y - chrome::PANEL_STATUS_Y,
        )
    }
}

/// What one of them does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    /// The `g_screenId` the table's handler sets. Every one is a
    /// [`crate::screens::shells`] entry, so the button reaches the original's
    /// own artwork and the shell says for itself what it does not yet do.
    Screen(u8),
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
pub const SPLIT_SLIDER: Rect = Rect::new(PANEL_X, 257, PANEL_W, 296 - 257 + 1);

/// The slider's own arithmetic, verbatim: left of the track steps down by four,
/// right of it up by four, and on the track the value is
/// `((x - 531) * 2) & 0xFC` — masked, so it lands on a multiple of four.
pub fn split_from_click(x: i32, current: i32) -> i32 {
    let next = if x < 531 {
        current - 4
    } else if x > 594 {
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

/// The field brush popup, and how far it is from the original.
///
/// **The original's:** which brushes exist, which menu a tile opens, and the
/// button geometry — five 48 × 48 buttons on the row `y 184 … 232`, three at
/// `x 240/304/368` for a field and two at `x 304/368` for waste. All of that is
/// read out of `Lords2.exe` by `crates/l2-kingdom/tests/oracle.rs`, and the
/// popup's vertical offset is `g_uiPopupRow << 4` in `FUN_00438990`.
///
/// **Ours, and it should look it:** the buttons hold our words rather than the
/// original's pictures, the frame is `widget::panel`, and the *targets* — the
/// twenty field tiles — are drawn as small squares coloured by what the field
/// is being used for. The original does not mark fields at all; it repaints the
/// tile artwork itself (`FUN_0046D7F4` picks a graphics bank and frame from the
/// same terrain value), and we deliberately do not, because that ladder's bank
/// byte is only half understood and `docs/decisions.md` C21 is what happens
/// when verified data is given invented presentation. A marker says *we know
/// what this field is*; painted artwork would claim *this is what the game
/// looked like*.
mod brush {
    /// One brush button, `BUTTON` on a side.
    pub const BUTTON: i32 = 48;
    /// The row the original puts them on, before its `g_uiPopupRow` offset.
    pub const ROW_Y: i32 = 184;
    /// The three x positions, of which the two-button menu uses the last two.
    pub const COLUMNS: [i32; 3] = [240, 304, 368];
    /// Half-width of a field marker. **Ours.**
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
    built: Option<(usize, u8, Viewport, u32)>,
    /// The two 128 × 128 rasters for this slot, decoded once.
    minimap: Option<Minimap>,
    minimap_slot: Option<usize>,
    focus: Focus,
    /// One line of feedback about the last thing that happened. **Ours.**
    status: String,
    /// The field brush popup, open over a tile the player clicked.
    ///
    /// `Map_Click` reaches `Field_SetType` exactly this way and no other: there
    /// is no field control on any county panel. See [`brush`].
    picked_field: Option<PickedField>,
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
}

/// A field tile the player has clicked, and the menu its terrain opens.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PickedField {
    county: u8,
    tile: usize,
    /// `FUN_00438990`'s choice between the two hotspot tables, made on the
    /// tile's own terrain.
    menu: &'static [FieldType],
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
            minimap: None,
            minimap_slot: None,
            focus: Focus::None,
            status: "CLICK A COUNTY".into(),
            picked_field: None,
            pointer: (CANVAS_W / 2, CANVAS_H / 2),
            pointer_in: false,
            scrolled: false,
        }
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

    /// The county's **town**: the 2 × 2 block on plane-0 bit `0x40`.
    ///
    /// The constant is still spelled `CASTLE` in `l2-kingdom` and its own doc
    /// comment explains why (`docs/decisions.md` C25); the bit is the town.
    pub fn town(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::tiles_with(ctx, county, l2_kingdom::map::flags::CASTLE)
    }

    /// The rectangle of one brush button, `i` counting from the left of the
    /// menu that is open.
    fn brush_button(menu_len: usize, i: usize) -> Rect {
        // The two-button menu uses the *right-hand* two columns, which is what
        // the hotspot table holds: x 304 and 368, not 240 and 304.
        let first = brush::COLUMNS.len() - menu_len;
        Rect::new(
            brush::COLUMNS[first + i],
            brush::ROW_Y,
            brush::BUTTON,
            brush::BUTTON,
        )
    }

    /// The popup's frame — the smallest box holding its buttons, with a margin.
    fn brush_panel(menu_len: usize) -> Rect {
        let first = Self::brush_button(menu_len, 0);
        let last = Self::brush_button(menu_len, menu_len - 1);
        Rect::new(
            first.x - 8,
            first.y - 20,
            last.x + last.w + 8 - (first.x - 8),
            brush::BUTTON + 28,
        )
    }

    /// `Field_SetType`, reached the way the original reaches it.
    fn paint(&mut self, ctx: &mut Ctx, picked: &PickedField, brush: FieldType) {
        match ctx
            .game
            .kingdom
            .paint_field(picked.county as usize, picked.tile, brush)
        {
            Ok(()) => {
                let c = &ctx.game.kingdom.counties[picked.county as usize];
                self.status = format!(
                    "{} - {} GRAIN {} PASTURE {} FALLOW",
                    brush.name().to_uppercase(),
                    c.fields_grain,
                    c.fields_cattle,
                    c.fields_fallow
                );
            }
            // Every refusal is one of the original's own guards, and saying
            // which is more useful than a beep.
            Err(why) => {
                self.status = match why {
                    BrushRefusal::NotAField => "NOT ONE OF THIS COUNTY'S FIELDS".into(),
                    BrushRefusal::Blighted => "THAT FIELD IS RUINED THIS SEASON".into(),
                    BrushRefusal::WrongMenu => "NOT OFFERED ON THAT FIELD".into(),
                };
            }
        }
        self.picked_field = None;
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
        // The turn count is in the key because [`town_graphics`] depends on
        // every county's population, which the end of a turn moves.
        let key = (ctx.game.map_slot, self.zoom.id, self.view, ctx.game.kingdom.turn_count);
        if self.built == Some(key) {
            return;
        }
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else {
            return;
        };
        let lattice = Lattice::build(&slot);
        self.base.clear(ctx.assets.ink.background);
        self.tags.clear();
        campaign::draw(
            &mut self.base,
            &slot,
            &lattice,
            &ctx.assets.map,
            self.view,
            &self.zoom,
            &mut self.tags,
            &Self::town_graphics(ctx),
        );
        self.built = Some(key);
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
        }
        out
    }

    fn ensure_minimap(&mut self, ctx: &Ctx) {
        if self.minimap_slot == Some(ctx.game.map_slot) {
            return;
        }
        self.minimap = ctx.assets.minimap(ctx.game.map_slot);
        self.minimap_slot = Some(ctx.game.map_slot);
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
    fn toggle_zoom(&mut self) {
        if self.zoom.id == NEAR.id {
            self.saved = self.view;
            self.zoom = FAR;
            self.view = Viewport::new(0x0C, 0x0E).clamped(&FAR);
            self.status = "ZOOMED OUT".into();
        } else {
            self.zoom = NEAR;
            self.view = self.saved.clamped(&NEAR);
            self.status = "ZOOMED IN".into();
        }
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
    }

    fn centre_on_county(&mut self, anchor: (usize, usize)) {
        self.centre_on_tile(anchor.0, anchor.1);
    }

    /// End the turn, and leave for screen `0x1C` if that ended the game.
    ///
    /// `FUN_00476768` is the original's shape: dismissing the message that set
    /// `DAT_0053F0C4` calls `FUN_00497879` — which advances the campaign counter
    /// on a win — and sets `g_screenId = 0x1C`. There is no message window here
    /// yet, so the turn's own end is the dismissal.
    fn end_turn(&mut self, ctx: &mut Ctx) -> Transition {
        let before = ctx.game.gold();
        match turn::end_turn(ctx.game) {
            Some(outcome) => {
                let change = ctx.game.gold() - before;
                self.status = format!(
                    "{} {} {} - {} MSG",
                    season_name(ctx.game.kingdom.season),
                    ctx.game.kingdom.year,
                    widget::signed(change),
                    outcome.report.messages.len()
                );
                if outcome.outcome.is_over() {
                    ctx.game.campaign.enter_conquest_screen();
                    // `Replace`, not `Push`: the campaign map underneath is a map
                    // of a game that is over, and the original leaves it — the
                    // OK button on `0x1C` goes on to `Game_NewGame` or the front
                    // end, never back to it.
                    return Transition::Replace(ScreenId::Conquest);
                }
            }
            None => self.status = "THE TURN MACHINE DID NOT COME ROUND".into(),
        }
        Transition::Stay
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

impl Screen for MapScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Campaign
    }

    fn title(&self, ctx: &Ctx) -> String {
        format!(
            "Lords of the Realm II - {} {}",
            season_name(ctx.game.kingdom.season),
            ctx.game.kingdom.year
        )
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
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
            Event::KeyDown(Key::Char('Z')) => self.toggle_zoom(),
            Event::KeyDown(Key::Char('E')) | Event::KeyDown(Key::Space) => {
                return self.end_turn(ctx)
            }
            Event::Pointer { x, y } => {
                self.pointer = (x, y);
                self.pointer_in = true;
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
            Event::RightClick { x, y } if self.map_clip().contains(x, y) => {
                if self.picked_field.take().is_some() {
                    self.status = "NO CHANGE".into();
                    return Transition::Stay;
                }
                return Transition::Push(ScreenId::Shell(0x04));
            }
            // **Ours.** `winit` has no "the pointer left"; `main.rs` synthesises
            // this so a cursor that walked off the window stops scrolling the
            // map from wherever it was last seen.
            Event::PointerLeft => {
                self.pointer_in = false;
                self.focus = Focus::None;
            }
            Event::Click { x, y } => {
                // The brush popup is modal over the map, the way
                // `Hotspot_Test` makes it: while it is up its buttons are
                // tested first and a click anywhere else dismisses it.
                if let Some(picked) = self.picked_field.clone() {
                    for (i, &b) in picked.menu.iter().enumerate() {
                        if Self::brush_button(picked.menu.len(), i).contains(x, y) {
                            self.paint(ctx, &picked, b);
                            return Transition::Stay;
                        }
                    }
                    self.picked_field = None;
                    self.status = "NO CHANGE".into();
                    return Transition::Stay;
                }
                if END_TURN_BUTTON.contains(x, y) {
                    return self.end_turn(ctx);
                } else if let Some(b) = SIDEBAR_BUTTONS.iter().find(|b| b.rect().contains(x, y)) {
                    // `g_sidebarButtons`. Three of the five reach a screen we
                    // can draw; the other two name the function the original
                    // dispatches to and do nothing, which is the honest state.
                    // Three of the five are gated on the county being yours,
                    // and the two that are not are the court and the lords.
                    let SidebarAction::Screen(id) = b.action;
                    let gated = matches!(id, 0x17 | 0x18 | 0x1B);
                    if gated && !ctx.game.is_players(ctx.game.selected) {
                        self.status = "NOT YOUR COUNTY".into();
                        return Transition::Stay;
                    }
                    return Transition::Push(ScreenId::Shell(id));
                } else if let Some(i) =
                    MINIMAP_MODE_BUTTONS.iter().position(|r| r.contains(x, y))
                {
                    // `Minimap_ModeButton`. The fourth is the zoom toggle and
                    // it works; the first three need the rating ramp at
                    // `0x004D28F8`, which is not transcribed.
                    if i == 3 {
                        self.toggle_zoom();
                    } else {
                        self.status = "MINIMAP RATINGS NOT DRAWN".into();
                    }
                } else if SPLIT_SLIDER.contains(x, y) && ctx.game.selected != 0 {
                    // `FUN_00439122`, the farm/industry split.
                    let id = ctx.game.selected as usize;
                    let current = ctx.game.kingdom.counties[id].industry_share;
                    let next = split_from_click(x, current);
                    if ctx.game.kingdom.set_industry_share(id, next) {
                        self.status = format!("INDUSTRY {next}% FARM {}%", 100 - next);
                    }
                } else if let Some(panel) = county::panel_at(x, y) {
                    // **The county strip is a 2 x 2 hotspot and it is the whole
                    // navigation into the four county panels** — there is no
                    // other way into any of them, in the original or here
                    // (`docs/screens-county.md` §2.3). It used not to be tested
                    // on this screen at all, which is why a player could reach
                    // tax and nothing else: our own COUNTY PANEL button opened
                    // the county screen on its own default.
                    if ctx.game.selected != 0 {
                        return Transition::Push(ScreenId::County(ctx.game.selected, panel));
                    }
                } else if chrome::minimap_hit_area().contains(x, y) {
                    // `Minimap_Click`: the county raster decides, then
                    // `Map_CentreOnTile` moves the viewport onto it.
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
                } else if self.map_clip().contains(x, y) {
                    self.ensure(ctx);
                    let county = self.county_at(x, y);
                    // **A click on one of your own buildings or fields takes
                    // precedence over selecting the county**, and in that
                    // order, which is `Map_Click`'s own plane-0 dispatch:
                    // `0x80` is a settlement and switches its industry, `0x40`
                    // is the county town and opens the village, `0x20` is
                    // farmland and opens the field brush. All three are gated
                    // on the county being the local player's.
                    if county != 0 && ctx.game.is_players(county) {
                        if let Some(tile) =
                            self.tile_at(x, y, Self::settlements(ctx, county).into_iter())
                        {
                            let terrain = ctx.game.kingdom.campaign.map.terrain[tile];
                            match industry::map_toggle_for_graphic(terrain) {
                                Some(what) => {
                                    let on = ctx.game.kingdom.toggle_industry(county as usize, what);
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
                        if let Some(tile) = self.tile_at(x, y, Self::town(ctx, county).into_iter())
                        {
                            let (tx, ty) = l2_kingdom::map::coords(tile);
                            self.centre_on_tile(tx as usize, ty as usize);
                            return Transition::Push(ScreenId::Village(county));
                        }
                        let fields = ctx.game.kingdom.field_tiles(county as usize);
                        if let Some(tile) =
                            self.tile_at(x, y, fields.into_iter().map(|(t, _)| t))
                        {
                            let terrain = ctx.game.kingdom.campaign.map.terrain[tile];
                            match field::menu_for(terrain) {
                                Some(menu) => {
                                    self.picked_field = Some(PickedField { county, tile, menu });
                                    self.status = format!(
                                        "FIELD: {}",
                                        field::classify(terrain).name().to_uppercase()
                                    );
                                }
                                None => self.status = "THAT FIELD IS RUINED THIS SEASON".into(),
                            }
                            return Transition::Stay;
                        }
                    }
                    if county == 0 {
                        ctx.game.select(0);
                        self.status = "CLICK A COUNTY".into();
                    } else if ctx.game.selected == county {
                        // A second click on the county already selected opens
                        // it, which is one click fewer than reaching for the
                        // strip. **Ours**; the original re-selects and no more.
                        return Transition::Push(ScreenId::County(county, county::Panel::Tax));
                    } else {
                        ctx.game.select(county);
                        self.status = format!("COUNTY {county} SELECTED");
                    }
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
    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        // The brush popup is modal, and a modal popup that scrolled the map out
        // from under its own target would be worse than one that does not.
        if self.picked_field.is_some() {
            return Transition::Stay;
        }
        if let Some(dir) = self.edge_direction() {
            if self.scroll(dir) {
                self.scrolled = true;
            }
        }
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::replace(&mut self.scrolled, false)
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        self.ensure(ctx);
        self.ensure_minimap(ctx);
        let ink = &ctx.assets.ink;
        let game = &ctx.game;
        let k = &game.kingdom;
        let clip = self.map_clip();

        canvas.pixels.copy_from_slice(&self.base.pixels);

        // Ours: the selected county outlined on the shape the player can see.
        // The original has no such outline (see the module docs).
        if game.selected != 0 {
            campaign::outline(canvas, &self.tags, game.selected, ink.highlight, clip);
        }

        // Ours: one marker per county in view, at its anchor tile, coloured by
        // owner. The original draws a county flag from `Flags1a.pl8` over the
        // castle tile; we have not placed those yet.
        for id in k.county_ids() {
            let (ax, ay) = (game.anchor_x[id] as usize, game.anchor_y[id] as usize);
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

        // Below the map at the far zoom the original draws a `Ui_DrawBox` of
        // 30 x 4 cells at (0, 412) — 480 x 64 — and puts the map's name, the
        // season and the year in it. The box is the original's; the words in it
        // are ours.
        if self.zoom.id == FAR.id {
            match &ctx.assets.chrome {
                Some(c) => c.draw_box(canvas, 0, 412, 30, 4, 0),
                None => widget::panel(canvas, ink, Rect::new(0, 412, 480, 64)),
            }
            text::draw(canvas, 24, 432, &self.status, ink.text);
        }

        // Ours: the player's own county's fields, marked by what each is being
        // used for, so the brush has visible targets. See [`brush`] for what
        // the original does instead and why we do not.
        if game.is_players(game.selected) {
            for (tile, kind) in k.field_tiles(game.selected as usize) {
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

        draw_menu_bar(canvas, ctx);
        draw_right_panel(self, canvas, ctx);

        // Last, so it sits over everything: the brush popup.
        if let Some(picked) = &self.picked_field {
            let panel = Self::brush_panel(picked.menu.len());
            widget::panel(canvas, ink, panel);
            let terrain = k.campaign.map.terrain[picked.tile];
            text::draw(
                canvas,
                panel.x + 8,
                panel.y + 6,
                &format!(
                    "FIELD IS {}",
                    field::classify(terrain).name().to_uppercase()
                ),
                ink.text,
            );
            for (i, &b) in picked.menu.iter().enumerate() {
                let r = Self::brush_button(picked.menu.len(), i);
                widget::panel(canvas, ink, r);
                let swatch = 12;
                fill_clipped(
                    canvas,
                    r.x + r.w / 2,
                    r.y + 14,
                    swatch,
                    field_colour(ink, b),
                    Clip::WHOLE,
                );
                text::draw(
                    canvas,
                    r.x + 4,
                    r.y + r.h - 12,
                    &b.name().to_uppercase(),
                    ink.text,
                );
            }
        }
    }
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

/// `Screen_DrawMenuBar`, as far as we can reproduce it.
///
/// **The original's:** the 640 × 24 background tiled from `Panels.pl8` frames
/// 196 + (c mod 8) — 25 cells from x 0 and 2 more from x 592 — and one 13 × 16
/// `Misc_cty` banner per live realm at `x = 270 + 16i, y = 4`.
///
/// **Ours:** the File / Options / Help menu titles are not drawn (they come
/// from `L2.eng` groups 1–3 and open drop-downs nothing here implements), and
/// the clock and treasury are our font at the original's x positions.
fn draw_menu_bar(canvas: &mut Canvas, ctx: &Ctx) {
    let ink = &ctx.assets.ink;
    let game = &ctx.game;
    let k = &game.kingdom;

    match &ctx.assets.chrome {
        Some(c) => {
            c.draw_menu_bar_background(canvas);
            // `Screen_DrawMenuBar`: realms 1..=5 that are in play, banner at
            // 270 + 16i.
            let mut slot = 0;
            for id in 1..k.realms.len() {
                if !k.realms[id].in_play {
                    continue;
                }
                let raw = game.realm_colour.get(id).copied().unwrap_or(0);
                let colour = chrome::realm_colour(raw);
                if c.draw_banner(canvas, slot, colour) {
                    slot += 1;
                }
            }
        }
        None => widget::panel(canvas, ink, Rect::new(0, 0, canvas.width as i32, TOP_BAR)),
    }

    // Our text, at the original's coordinates: the year and season at x 360 and
    // the treasury at x 500, both 6 pixels down.
    let clock = format!("{} {}", season_name(k.season), k.year);
    text::draw(canvas, 360, 6, &clock, ink.text);
    text::draw(canvas, 500, 6, &format!("GOLD {}", game.gold()), ink.text);
    text::draw(canvas, 6, 6, &format!("TURN {}", k.turn_count), ink.dim);
    let held = format!("COUNTIES {}/{}", game.owned_by(game.player), k.county_count);
    text::draw(canvas, 150, 6, &held, ink.dim);
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

    // The minimap, tinted from `Lords2.exe`'s own realm ramp.
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
        chrome::draw_minimap(canvas, m, game.selected, &owner);
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
    } else {
        text::draw_centred(canvas, PANEL_X + 80, 200, "NO COUNTY SELECTED", ink.dim);
    }

    // **Ours, and it should look it.** `0x00438E3B` turns the 162 x 128 plate
    // at y = 302 into two columns of job rows — farm jobs left of x = 560,
    // industry right of it — and a click opens the job popup for that job
    // (`docs/screens-county.md` §2.4). We do not lay those rows out yet, so the
    // plate carries a dark box of ours with the county's stores in it and the
    // one status line this interface has. A stub that says so beats one that
    // looks finished.
    let jobs = Rect::new(PANEL_X + 3, chrome::PANEL_OWN_C_Y + 4, PANEL_W - 6, 120);
    if ctx.assets.chrome.is_some() {
        widget::panel(canvas, ink, jobs);
    }
    let x = PANEL_X + 8;
    let right = PANEL_X + PANEL_W - 8;
    let mut y = jobs.y + 5;
    text::draw(canvas, x, y, "JOBS: NOT DRAWN", ink.dim);
    y += 14;
    if let Some(c) = k.counties.get(game.selected as usize).filter(|_| game.selected != 0) {
        for (label, value) in [
            ("GRAIN", c.grain.to_string()),
            ("HERD", c.herd.to_string()),
            ("FIELDS", c.fields_grain.to_string()),
        ] {
            widget::stat(canvas, ink, x, y, right, label, &value);
            y += 12;
        }
    }
    text::draw(canvas, x, y + 6, &screen.status, ink.dim);

    // **The five sidebar buttons.** `Misc_cty` frame 57 already drew them; all
    // this adds is which one the pointer is over, because the original's
    // feedback is a pressed frame in its own hotspot table and we do not have
    // the pressed frames identified. Nothing is drawn over the icons.
    if let Focus::Sidebar(i) = screen.focus {
        let r = SIDEBAR_BUTTONS[i].rect();
        widget::frame(canvas, r, ink.highlight);
        text::draw_centred(canvas, r.centre_x(), r.y - 10, SIDEBAR_BUTTONS[i].name, ink.highlight);
    }

    // `Screen_DrawEndTurn` (`0x0041A734`):
    // `Ui_DrawCentred(4, 0, 0x1DE, 0x1CE, 0xA2, &g_fontSmall, 0x16)` — `L2.eng`
    // group 4, centred in 162 pixels at (478, 462), in the strip's own 9-pixel
    // font. We had it two lines lower and in words of ours.
    let end = if screen.focus == Focus::EndTurn {
        ink.highlight
    } else {
        ink.text
    };
    let label = ctx.assets.shell.text(4, 0);
    let label = if label.is_empty() { "END TURN" } else { label };
    county::strip_centred_at(ctx, canvas, PANEL_X, 462, PANEL_W, label, end);
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
            assert_eq!(r.y + r.h, END_TURN_BUTTON.y, "{b:?} does not meet the end-turn strip");
            assert!(PANEL.contains(r.x, r.y), "{b:?} starts outside the sidebar");
            assert!(r.x + r.w <= PANEL.x + PANEL.w, "{b:?} runs past the screen edge");
        }
        assert_eq!(END_TURN_BUTTON.y + END_TURN_BUTTON.h, 480);
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
        for b in SIDEBAR_BUTTONS {
            let SidebarAction::Screen(id) = b.action;
            assert!(
                crate::screens::shells::SHELLS.iter().any(|s| s.id == id),
                "{b:?} names screen {id:#04X}, which has no shell"
            );
        }
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

    /// And a tick with the pointer held at the edge actually moves the view,
    /// and reports that it did so the machine repaints. That report is what
    /// makes the gesture continuous rather than one step per mouse move.
    #[test]
    fn holding_the_pointer_at_the_edge_scrolls_every_tick_and_asks_for_a_repaint() {
        let mut game = crate::Game::new(1);
        let assets = crate::game::Assets::placeholder();
        let mut s = MapScreen::new();
        s.view = Viewport::new(40, 20);
        s.pointer = (CANVAS_W - 1, 240);
        s.pointer_in = true;
        for step in 1..=3 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            s.update(&mut ctx);
            assert_eq!(s.viewport(), Viewport::new(40, 20 + step), "tick {step}");
            assert!(s.take_redraw(), "a tick that moved the map has to be drawn");
            assert!(!s.take_redraw(), "and taking the flag clears it");
        }
        // In the middle of the screen nothing happens at all.
        s.pointer = (240, 240);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        s.update(&mut ctx);
        assert_eq!(s.viewport(), Viewport::new(40, 23));
        assert!(!s.take_redraw());
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
        let mut s = MapScreen::new();
        s.view = Viewport::new(60, 30);
        s.toggle_zoom();
        assert_eq!(s.zoom().id, FAR.id);
        assert_eq!(
            s.viewport(),
            Viewport::new(0, 14),
            "row clamps to 0, col 0x0E stands"
        );
        // And the far view cannot be scrolled off that position.
        assert!(!s.scroll(Dir::E));
        assert!(!s.scroll(Dir::N));
        s.toggle_zoom();
        assert_eq!(s.zoom().id, NEAR.id);
        assert_eq!(s.viewport(), Viewport::new(60, 30), "back where it was");
        assert!(s.scroll(Dir::E), "and the near view scrolls again");
        assert_eq!(s.viewport(), Viewport::new(60, 31));
    }
}
