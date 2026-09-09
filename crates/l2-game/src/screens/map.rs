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
use l2_view::campaign::{self, Dir, Lattice, Viewport, Zoom, FAR, NEAR, PANEL_W, PANEL_X};
use l2_view::chrome::{self, Minimap};
use l2_view::{text, Canvas, Clip, Ink, Tags};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::saveload::Mode as SaveLoadMode;
use crate::turn;
use crate::widget;

/// The menu bar: `Screen_DrawMenuBar`'s 640 × 24 strip at y 0.
pub const TOP_BAR: i32 = campaign::TOP_BAR_H;

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

/// `Misc_cty` frame 57 (162 × 30) at (478, 430). **Ours:** the original puts a
/// status line here, not a button; we use it to open the county panel, because
/// our county panel is a separate screen and the original's *is* this column.
pub const COUNTY_BUTTON: Rect = Rect::new(
    PANEL_X,
    chrome::PANEL_STATUS_Y,
    PANEL_W,
    chrome::PANEL_END_TURN_Y - chrome::PANEL_STATUS_Y,
);

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
    County,
    EndTurn,
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
    built: Option<(usize, u8, Viewport)>,
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

    /// The county's settlement tiles — its four industry sites and its castle
    /// block. `Map_Click`'s own test: plane-0 bit `0x80`.
    fn settlements(ctx: &Ctx, county: u8) -> Vec<usize> {
        let map = &ctx.game.kingdom.campaign.map;
        (0..map.terrain.len())
            .filter(|&t| {
                map.county[t] == county && map.flags[t] & l2_kingdom::map::flags::SETTLEMENT != 0
            })
            .collect()
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
        let key = (ctx.game.map_slot, self.zoom.id, self.view);
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
        );
        self.built = Some(key);
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
            Event::KeyDown(Key::Enter) => {
                if ctx.game.selected != 0 {
                    return Transition::Push(ScreenId::County(ctx.game.selected));
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
            // **Ours, and only the key is.** The original opens the village by
            // clicking the county's *town* on the map: `FUN_0043CE1A` tests bit
            // 0x20 of the clicked cell's attribute byte, checks the county is
            // the local player's, centres on `+0x70` and sets `g_screenId = 2`.
            // We do not read that attribute plane yet, so the destination is
            // the original's and the way in is not.
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
                self.focus = if COUNTY_BUTTON.contains(x, y) {
                    Focus::County
                } else if END_TURN_BUTTON.contains(x, y) {
                    Focus::EndTurn
                } else {
                    Focus::None
                };
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
                } else if COUNTY_BUTTON.contains(x, y) {
                    if ctx.game.selected != 0 {
                        return Transition::Push(ScreenId::County(ctx.game.selected));
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
                        // strip.
                        return Transition::Push(ScreenId::County(county));
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

    // **Ours.** The county's numbers, written into the one part of the original
    // column that is plain: `Misc_cty` frame 56, y 302..429. The two frames
    // above it hold the original's gauges, portrait and crop icons
    // (`docs/screens.md` §4.3) and we do not fill them, so putting our text
    // over them would read as a bug rather than as a stub. Anything the county
    // screen shows properly is that screen's business, not this one's.
    let top = if ctx.assets.chrome.is_some() {
        // On its own dark backing, so it reads as our overlay rather than as
        // part of the original's panel.
        let ours = Rect::new(PANEL_X + 3, chrome::PANEL_OWN_C_Y + 4, PANEL_W - 6, 108);
        widget::panel(canvas, ink, ours);
        ours.y + 5
    } else {
        chrome::PANEL_MIDDLE_Y + 6
    };
    let x = PANEL_X + 8;
    let right = PANEL_X + PANEL_W - 8;
    let mut y = top;
    match game.selected {
        0 => {
            text::draw(canvas, x, y, "NO COUNTY SELECTED", ink.dim);
        }
        id => {
            let c = &k.counties[id as usize];
            text::draw(canvas, x, y, &format!("COUNTY {id}"), ink.highlight);
            y += 12;
            let held = if c.owner == 0 {
                "UNCLAIMED".to_string()
            } else if c.owner == game.player {
                "YOURS".to_string()
            } else {
                format!("REALM {}", c.owner)
            };
            text::draw(canvas, x, y, &held, ink.text);
            y += 14;
            for (label, value) in [
                ("POP", c.population.to_string()),
                ("HAPPY", c.happiness.to_string()),
                ("TAX", format!("{}%", c.tax_rate)),
                ("GRAIN", c.grain.to_string()),
                ("HERD", c.herd.to_string()),
            ] {
                widget::stat(canvas, ink, x, y, right, label, &value);
                y += 12;
            }
        }
    }

    // The two strips at the bottom. The End Turn one is the original's own
    // button; the status line above it is ours.
    let label = if screen.focus == Focus::County {
        ink.highlight
    } else {
        ink.text
    };
    text::draw_centred(
        canvas,
        COUNTY_BUTTON.centre_x(),
        COUNTY_BUTTON.y + 4,
        "COUNTY PANEL",
        label,
    );
    text::draw_centred(
        canvas,
        COUNTY_BUTTON.centre_x(),
        COUNTY_BUTTON.y + 16,
        &screen.status,
        ink.dim,
    );
    let end = if screen.focus == Focus::EndTurn {
        ink.highlight
    } else {
        ink.text
    };
    text::draw_centred(
        canvas,
        END_TURN_BUTTON.centre_x(),
        END_TURN_BUTTON.y + 6,
        "END TURN",
        end,
    );
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
        assert_eq!(COUNTY_BUTTON.y + COUNTY_BUTTON.h, END_TURN_BUTTON.y);
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
