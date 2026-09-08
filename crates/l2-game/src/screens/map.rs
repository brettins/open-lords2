//! The campaign map: a real shipped scenario's map, with counties you can
//! click, and the button that ends the turn.
//!
//! # The map is painted once
//!
//! The tiles do not change — 4,096 of them, from five PL8 banks — so they are
//! drawn once into a cached canvas together with the [`Tags`] plane that says
//! which county each pixel belongs to, and every frame after that starts by
//! copying the cache. The selection outline, the county markers and the bars
//! are painted on top, so what moves is redrawn and what does not is not.
//!
//! # Picking
//!
//! A click reads the tag plane, which is exact by construction: it answers with
//! the county whose tile is actually visible at that pixel, overlap and draw
//! order included. See `l2_view::campaign`.

use l2_view::campaign::{self, MapView};
use l2_view::{text, Canvas, Tags};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::turn;
use crate::widget;

/// The bar above the map, and where the map starts.
pub const TOP_BAR: i32 = 22;
/// The bar below it. The map is 387 pixels tall from `TOP_BAR`.
pub const BOTTOM_BAR_Y: i32 = TOP_BAR + 388;

pub const COUNTY_BUTTON: Rect = Rect::new(392, 448, 116, 20);
pub const END_TURN_BUTTON: Rect = Rect::new(516, 448, 116, 20);

/// The map's own area, which is where a click means "that county".
pub const MAP_AREA: Rect =
    Rect::new(0, TOP_BAR, l2_view::canvas::WIDTH as i32, BOTTOM_BAR_Y - TOP_BAR);

/// Half-width of a county's marker square.
const MARKER: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    None,
    County,
    EndTurn,
}

pub struct MapScreen {
    view: MapView,
    /// The painted tiles, and what county each pixel came from. Both are built
    /// on the first draw or the first click, whichever comes first.
    base: Canvas,
    tags: Tags,
    built_slot: Option<usize>,
    focus: Focus,
    /// One line of feedback about the last thing that happened.
    status: String,
}

impl MapScreen {
    pub fn new() -> MapScreen {
        MapScreen {
            view: MapView::fit(TOP_BAR),
            base: Canvas::screen(),
            tags: Tags::screen(),
            built_slot: None,
            focus: Focus::None,
            status: "CLICK A COUNTY".into(),
        }
    }

    /// Paint the tiles and stamp the county ids, once per map slot.
    ///
    /// Called from `draw` *and* from `handle`, because a click can arrive
    /// before a frame has been drawn — in a test it always does — and a pick
    /// plane that only exists after the first repaint is a pick plane that
    /// works everywhere except in the tests.
    fn ensure(&mut self, ctx: &Ctx) {
        if self.built_slot == Some(ctx.game.map_slot) {
            return;
        }
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else { return };
        self.base.clear(ctx.assets.ink.background);
        self.tags.clear();
        campaign::draw(&mut self.base, &slot, &ctx.assets.map, self.view, &mut self.tags);
        self.built_slot = Some(ctx.game.map_slot);
    }

    /// The county at a canvas pixel, or 0.
    pub fn county_at(&self, x: i32, y: i32) -> u8 {
        if !MAP_AREA.contains(x, y) {
            return 0;
        }
        self.tags.at(x, y)
    }

    fn end_turn(&mut self, ctx: &mut Ctx) {
        let before = ctx.game.gold();
        match turn::end_turn(ctx.game) {
            Some(outcome) => {
                let change = ctx.game.gold() - before;
                self.status = format!(
                    "{} {} - TREASURY {} - {} MESSAGES",
                    season_name(ctx.game.kingdom.season),
                    ctx.game.kingdom.year,
                    widget::signed(change),
                    outcome.report.messages.len()
                );
            }
            None => self.status = "THE TURN MACHINE DID NOT COME ROUND".into(),
        }
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
            Event::KeyDown(Key::Char('E')) | Event::KeyDown(Key::Space) => self.end_turn(ctx),
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
                if END_TURN_BUTTON.contains(x, y) {
                    self.end_turn(ctx);
                } else if COUNTY_BUTTON.contains(x, y) {
                    if ctx.game.selected != 0 {
                        return Transition::Push(ScreenId::County(ctx.game.selected));
                    }
                } else if MAP_AREA.contains(x, y) {
                    self.ensure(ctx);
                    let county = self.county_at(x, y);
                    if county == 0 {
                        ctx.game.select(0);
                        self.status = "CLICK A COUNTY".into();
                    } else if ctx.game.selected == county {
                        // A second click on the county already selected opens
                        // it, which is one click fewer than reaching for the
                        // button.
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
        let ink = &ctx.assets.ink;
        let game = &ctx.game;
        let k = &game.kingdom;

        canvas.pixels.copy_from_slice(&self.base.pixels);

        // The selected county, outlined on the shape the player can see.
        if game.selected != 0 {
            campaign::outline(canvas, &self.tags, game.selected, ink.highlight);
        }

        // One marker per county, at its anchor tile, coloured by owner.
        for id in k.county_ids() {
            let (ax, ay) = (game.anchor_x[id] as usize, game.anchor_y[id] as usize);
            let (cx, cy) = campaign::tile_centre(self.view, ax, ay);
            let owner = k.counties[id].owner as usize;
            let colour = ink.realm.get(owner).copied().unwrap_or(ink.dim);
            canvas.fill_rect(cx - MARKER - 1, cy - MARKER - 1, MARKER * 2 + 3, MARKER * 2 + 3, ink.background);
            canvas.fill_rect(cx - MARKER, cy - MARKER, MARKER * 2 + 1, MARKER * 2 + 1, colour);
        }

        // Top bar: the clock and the treasury.
        let bar = Rect::new(0, 0, canvas.width as i32, TOP_BAR);
        widget::panel(canvas, ink, bar);
        let clock = format!(
            "{} {}   TURN {}",
            season_name(k.season),
            k.year,
            k.turn_count
        );
        text::draw(canvas, 6, 8, &clock, ink.text);
        let purse = format!(
            "REALM {}   GOLD {} ({})",
            game.player,
            game.gold(),
            widget::signed(game.gold_change())
        );
        text::draw_right(canvas, canvas.width as i32 - 6, 8, &purse, ink.text);
        let held = format!("COUNTIES {}/{}", game.owned_by(game.player), k.county_count);
        text::draw_centred(canvas, canvas.width as i32 / 2, 8, &held, ink.dim);

        // Bottom bar: what is selected, what just happened, and the two things
        // the player can do from here.
        let panel = Rect::new(
            0,
            BOTTOM_BAR_Y,
            canvas.width as i32,
            canvas.height as i32 - BOTTOM_BAR_Y,
        );
        widget::panel(canvas, ink, panel);

        let line = match game.selected {
            0 => "NO COUNTY SELECTED".to_string(),
            id => {
                let c = &k.counties[id as usize];
                format!(
                    "COUNTY {id}  {}  POP {}  HAPPY {}  TAX {}%  GRAIN {}  HERD {}",
                    if c.owner == 0 {
                        "UNCLAIMED".to_string()
                    } else if c.owner == game.player {
                        "YOURS".to_string()
                    } else {
                        format!("REALM {}", c.owner)
                    },
                    c.population,
                    c.happiness,
                    c.tax_rate,
                    c.grain,
                    c.herd
                )
            }
        };
        text::draw(canvas, 6, BOTTOM_BAR_Y + 8, &line, ink.text);
        text::draw(canvas, 6, BOTTOM_BAR_Y + 22, &self.status, ink.dim);
        text::draw(canvas, 6, BOTTOM_BAR_Y + 36, "ESC MENU", ink.dim);

        widget::button(
            canvas,
            ink,
            COUNTY_BUTTON,
            "COUNTY PANEL",
            self.focus == Focus::County,
        );
        widget::button(canvas, ink, END_TURN_BUTTON, "END TURN", self.focus == Focus::EndTurn);
    }
}
