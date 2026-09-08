//! The county panels — **four of them**, not one.
//!
//! This screen used to be one invented full-screen page listing twenty-two
//! fields with two arrow rows bolted underneath. It was not what the original
//! draws, and nobody had looked. `docs/screens-county.md` is the reading; this
//! is the rebuild.
//!
//! # What the original does
//!
//! The county's numbers live in the **county strip**, the 162 × 94 plate at
//! (478, 156) in the campaign map's right-hand column, and the strip is a
//! **2 × 2 hotspot**: `CountyStrip_Click` (`0x00438CEB`) opens the population
//! panel from its top-left quadrant, happiness from top-right, tax from
//! bottom-left and rations from bottom-right, and there is no other way into
//! any of them. Each is a floating `Ui_DrawBox` window over whatever was
//! underneath, with a tick button in its bottom-right corner.
//!
//! So this screen draws the strip at the original's own coordinates, over the
//! original's own `Misc_cty.pl8` plates, with the original's own quadrants
//! live; and it draws one of the four panels at the original's own rectangle,
//! with the original's own rows in the original's own order, using the
//! original's `Panels.pl8` box kit and `System2.pl8` buttons.
//!
//! # What is still ours, and says so
//!
//! * **The font.** The original draws `Fntl2_14.pl8` for body lines,
//!   `Fntl2_22.pl8` for headings and `Fntl2_9.pl8` for the strip. We draw our
//!   own 5 × 7 font at the original's coordinates.
//! * **The history graph.** `Ui_HistoryGraph` fills 402 × 155 of both the
//!   population and the happiness panel from `g_countyHistory` — 400 turns ×
//!   16 counties × 8 bytes, and part of the save. `l2-kingdom` keeps no
//!   history, so that rectangle is an empty recess that says so. It is a stub
//!   and it is meant to look like one.
//! * **What is behind the panels.** The original has the campaign map there.
//!   The map screen is another file; this one paints a flat ground.
//! * **The strings.** Ours, transcribed from the `L2.eng` group each row names.
//!   Nothing here reads `L2.eng`; the workspace has no decoder for it.
//! * **The bottom strip** reads BACK TO MAP. The original's is End Turn, which
//!   is the map screen's business.
//!
//! # What is not here at all
//!
//! Labour, sowing, ale and field types, because **none of them is on a county
//! panel in the original either**. Peasants are moved by rubber-band drag on
//! the village screen (0x02), ale is bought from the merchant (0x08), and
//! fields are painted on the map. `docs/screens-county.md` §6.4 and §6.5.

use l2_kingdom::tables::{HEALTH_BAND_NAMES, RATION_LEVEL_COUNT, RATION_NAMES};
use l2_view::chrome::{self, system};
use l2_view::{text, Canvas, Ink};

use crate::game::{Assets, MAX_RATION_SPLIT, MAX_TAX_RATE};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

/// The four panels, in the order Up and Down cycle them.
///
/// **The order is ours**; the original has no ordering because it has no
/// keyboard route in. Tax and Ration lead because they are the two that take
/// orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Tax,
    Ration,
    Population,
    Happiness,
}

pub const PANELS: [Panel; 4] = [Panel::Tax, Panel::Ration, Panel::Population, Panel::Happiness];

// ----------------------------------------------------------------- geometry
//
// Every rectangle below is `Ui_DrawBox(x, y, cols, rows)` out of the panel's
// own painter, in cells of 16 pixels. `docs/screens-county.md` §5.

/// `Panel_Population` (`0x004110B1`): `Ui_DrawBox(0x10, 0x30, 0x1C, 0x17)`.
const POPULATION_BOX: (i32, i32, i32, i32) = (16, 48, 28, 23);
/// `Panel_Happiness` (`0x004116FB`): `Ui_DrawBox(0x10, 0x30, 0x1C, 0x18)`.
const HAPPINESS_BOX: (i32, i32, i32, i32) = (16, 48, 28, 24);
/// `Panel_Tax` (`0x0041152F`): `Ui_DrawBox(0x50, 0x90, 0x14, 0x09)`.
const TAX_BOX: (i32, i32, i32, i32) = (80, 144, 20, 9);
/// `Panel_Ration` (`0x00411B72`): `Ui_DrawBox(0x80, 0x60, 0x12, 0x0F)`. The
/// height is 17 cells with *Armies Eat* on; we always draw the short one,
/// because we never draw the foraging line it makes room for.
const RATION_BOX: (i32, i32, i32, i32) = (128, 96, 18, 15);

/// `Ui_HistoryGraph(0x20, 0x54, mode)` opens with
/// `Ui_DrawInsetRect(x, y, 0x192, 0x9B)` — 402 × 155 at (32, 84) — and both
/// graph panels call it with the same origin.
const GRAPH: Rect = Rect::new(32, 84, 402, 155);

/// The county strip's own rectangle: `Misc_cty` frame 55 is 162 × 94 and
/// `Screen_DrawCampaign` draws it at (478, 156).
pub const STRIP: Rect = Rect::new(478, chrome::PANEL_MIDDLE_Y, 162, 94);

/// `CountyStrip_Click`'s outer guard, made half-open: the original tests
/// `mx > 487 && mx < 630` and `my > 181 && my < 241`.
const HOT_X0: i32 = 488;
const HOT_X1: i32 = 630;
const HOT_Y0: i32 = 182;
const HOT_Y1: i32 = 241;
/// Its two splits. `548 … 567` is a dead band 19 pixels wide, and the health
/// thermometer — 14 wide, drawn at x = 552 — sits inside it with two pixels
/// spare on each side. The gap exists for the bar.
const HOT_X_LEFT_END: i32 = 548;
const HOT_X_RIGHT_START: i32 = 568;
const HOT_Y_SPLIT: i32 = 212;

/// `Pl8_DrawFrameHere(g_miscCtySheet, healthBand + 0x46, 0x228, 0xB5)`.
const THERMOMETER: (i32, i32) = (552, 181);
const THERMOMETER_FRAME: usize = 0x46;
const THERMOMETER_W: i32 = 14;

/// Both graph panels put their labels at x = 48 and right-anchor their values
/// at x = 336 (`0x30` and `0x150`).
const LABEL_X: i32 = 48;
const VALUE_RIGHT: i32 = 336;

impl Panel {
    /// The panel's window in pixels — the rectangle `Ui_DrawBox` covers.
    pub fn window(self) -> Rect {
        let (x, y, cols, rows) = self.box_cells();
        Rect::new(x, y, cols * 16, rows * 16)
    }

    fn box_cells(self) -> (i32, i32, i32, i32) {
        match self {
            Panel::Population => POPULATION_BOX,
            Panel::Happiness => HAPPINESS_BOX,
            Panel::Tax => TAX_BOX,
            Panel::Ration => RATION_BOX,
        }
    }

    /// The graph area, for the two panels that have one.
    pub fn graph_rect(self) -> Option<Rect> {
        matches!(self, Panel::Population | Panel::Happiness).then_some(GRAPH)
    }

    /// `Ui_OkButton(x, y, 0)` — the tick, 24 × 24, in the panel's own corner.
    pub fn ok_button(self) -> Rect {
        let (x, y) = match self {
            // Ui_OkButton(0x1B4, 0x184, 0)
            Panel::Population => (436, 388),
            // Ui_OkButton(0x1B4, 0x194, 0)
            Panel::Happiness => (436, 404),
            // Ui_OkButton(0x174, 0x104, 0)
            Panel::Tax => (372, 260),
            // Ui_OkButton(0x184, (0 + 0xF) * 0x10 + 0x44, 0)
            Panel::Ration => (388, 308),
        };
        Rect::new(x, y, system::OK_DIM, system::OK_DIM)
    }

    /// The **up** arrow: record 0 of `g_taxWidgets` (`0x004DD790`) or of
    /// `g_rationWidgets` (`0x004DD7C0`). It is the left of the pair, which is
    /// the original's arrangement and not a transcription slip.
    pub fn increase_button(self) -> Option<Rect> {
        match self {
            Panel::Tax => Some(Rect::new(192, 162, system::ARROW, system::ARROW)),
            Panel::Ration => Some(Rect::new(344, 130, system::ARROW, system::ARROW)),
            _ => None,
        }
    }

    /// The **down** arrow: record 1 of the same table, 24 pixels to its right.
    pub fn decrease_button(self) -> Option<Rect> {
        match self {
            Panel::Tax => Some(Rect::new(224, 162, system::ARROW, system::ARROW)),
            Panel::Ration => Some(Rect::new(368, 130, system::ARROW, system::ARROW)),
            _ => None,
        }
    }

    /// Which quadrant of the county strip opens this panel.
    pub fn strip_hotspot(self) -> Rect {
        let (x, w) = match self {
            Panel::Population | Panel::Tax => (HOT_X0, HOT_X_LEFT_END - HOT_X0),
            Panel::Happiness | Panel::Ration => (HOT_X_RIGHT_START, HOT_X1 - HOT_X_RIGHT_START),
        };
        let (y, h) = match self {
            Panel::Population | Panel::Happiness => (HOT_Y0, HOT_Y_SPLIT - HOT_Y0),
            Panel::Tax | Panel::Ration => (HOT_Y_SPLIT, HOT_Y1 - HOT_Y_SPLIT),
        };
        Rect::new(x, y, w, h)
    }
}

// -------------------------------------------------- the ration split slider
//
// `Panel_RationSlider` (`0x00411FDE`) draws it and `Ration_SliderClick`
// (`0x0043A379`) hit-tests it. The caps are drawn at 200 and 324 and are 24
// wide, the track runs 224 … 323, and the knob is drawn at `220 + value`.

const SLIDER_Y: i32 = 216;
const SLIDER_CAP_LEFT_X: i32 = 200;
const SLIDER_CAP_RIGHT_X: i32 = 324;
const SLIDER_TRACK_X: i32 = 224;
const SLIDER_TRACK_W: i32 = 100;
const SLIDER_KNOB_ORIGIN: i32 = 220;

/// `(200, 0xDC, 0x18, 0x18)` — steps the split down by one.
pub fn split_down_button() -> Rect {
    Rect::new(SLIDER_CAP_LEFT_X, SLIDER_Y, system::SLIDER_CAP, system::SLIDER_CAP)
}

/// `(0x145, 0xDC, 0x18, 0x18)` — up by one. 0x145 is 325, one pixel right of
/// where the cap is drawn; that off-by-one is the original's.
pub fn split_up_button() -> Rect {
    Rect::new(SLIDER_CAP_RIGHT_X + 1, SLIDER_Y, system::SLIDER_CAP, system::SLIDER_CAP)
}

/// `(0xE0, 0xDC, 0x66, 0x18)` — a click here jumps the split to `mouseX - 224`.
pub fn split_track() -> Rect {
    Rect::new(SLIDER_TRACK_X, SLIDER_Y, 102, system::SLIDER_CAP)
}

// -------------------------------------------------------------------- labels
//
// Ours, transcribed from the `L2.eng` group each panel draws. The workspace has
// no `L2.eng` decoder, so these are copies rather than reads, and they are
// upper case because our font has no lower case.

/// `L2.eng` group 73 — the population panel.
mod g73 {
    pub const TITLE: &str = "POPULATION IN";
    pub const LAST: &str = "LAST SEASON";
    pub const BIRTHS: &str = "BIRTHS";
    pub const DEATHS: &str = "DEATHS";
    pub const ARMY: &str = "ARMY";
    pub const EMIGRANTS: &str = "EMIGRANTS TO";
    pub const IMMIGRANTS: &str = "TOTAL IMMIGRANTS";
    pub const THIS: &str = "THIS SEASON";
    pub const GREATEST: &str = "GREATEST POPULATION";
    pub const NO_EMIGRATION: &str = "NO EMIGRATION.";
    pub const NO_IMMIGRATION: &str = "NO IMMIGRATION.";
}

/// `L2.eng` group 85 — the happiness panel.
mod g85 {
    pub const TITLE: &str = "HAPPINESS IN";
    pub const LAST: &str = "LAST SEASON";
    pub const FROM_TAXES: &str = "FROM TAXES";
    pub const FROM_RATION: &str = "FROM RATION";
    pub const FROM_HEALTH: &str = "FROM HEALTH";
    pub const FROM_ARMY: &str = "FROM ARMY";
    pub const FROM_ALE: &str = "FROM ALE";
    pub const THIS: &str = "THIS SEASON";
    pub const AVERAGE: &str = "AVERAGE HAPPINESS";
    pub const FROM_EVENTS: &str = "FROM EVENTS";
}

/// `L2.eng` group 86 — the tax panel; and group 61, the strip's two captions.
mod g86 {
    pub const TITLE: &str = "TAX IN";
    pub const RATE: &str = "TAX RATE";
    pub const PEOPLE_PAY: &str = "PEOPLE PAY";
    pub const THIS_COUNTY: &str = "THIS COUNTY";
    pub const OTHER_COUNTIES: &str = "OTHER COUNTIES";
    pub const STRIP_TAX: &str = "TAX";
    pub const STRIP_RATION: &str = "RATION";
}

/// `L2.eng` group 87 — the ration panel.
mod g87 {
    pub const TITLE: &str = "RATION";
    pub const WANTED: &str = "WANTED:";
    pub const ACHIEVED: &str = "ACHIEVED:";
    pub const HEALTH: &str = "HEALTH:";
    pub const EATEN: &str = "EATEN";
    pub const FED: &str = "FED";
}

// ---------------------------------------------------------------- the screen

pub struct CountyScreen {
    county: u8,
    panel: Panel,
}

impl CountyScreen {
    /// Opens on the tax panel. The original opens on whichever quadrant was
    /// clicked and has no default; this has to start somewhere, and tax is the
    /// order the player gives most.
    pub fn new(county: u8) -> CountyScreen {
        CountyScreen { county, panel: Panel::Tax }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    pub fn panel(&self) -> Panel {
        self.panel
    }

    pub fn open(&mut self, panel: Panel) {
        self.panel = panel;
    }

    /// **Ours.** The original's bottom strip is End Turn; leaving a panel is
    /// its tick, and leaving the map is not a thing you do. Our stack pushed
    /// this screen, so something has to pop it.
    pub fn back_button() -> Rect {
        Rect::new(478, chrome::PANEL_END_TURN_Y, 162, 20)
    }

    fn panel_index(&self) -> usize {
        PANELS.iter().position(|&p| p == self.panel).unwrap_or(0)
    }

    /// Move the open panel's own value by `step`. Silently refused for a county
    /// the player does not hold, and there is nothing to move on the two panels
    /// that only report.
    fn adjust(&self, ctx: &mut Ctx, step: i32) {
        let id = self.county;
        let Some(c) = ctx.game.kingdom.counties.get(id as usize) else { return };
        match self.panel {
            Panel::Tax => {
                let next = (c.tax_rate + step).clamp(0, MAX_TAX_RATE);
                ctx.game.set_tax_rate(id, next);
            }
            Panel::Ration => {
                let next = (c.ration_wanted + step).clamp(0, RATION_LEVEL_COUNT as i32 - 1);
                ctx.game.set_ration(id, next);
            }
            Panel::Population | Panel::Happiness => {}
        }
    }

    fn split_click(&self, ctx: &mut Ctx, x: i32, y: i32) -> bool {
        if self.panel != Panel::Ration {
            return false;
        }
        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return false };
        let current = c.ration_split;
        let next = if split_down_button().contains(x, y) {
            current - 1
        } else if split_up_button().contains(x, y) {
            current + 1
        } else if split_track().contains(x, y) {
            x - SLIDER_TRACK_X
        } else {
            return false;
        };
        ctx.game.set_ration_split(self.county, next.clamp(0, MAX_RATION_SPLIT));
        true
    }
}

impl Screen for CountyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::County(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("County {}", self.county)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => return Transition::Pop,
            Event::KeyDown(Key::Up) => {
                self.panel = PANELS[(self.panel_index() + PANELS.len() - 1) % PANELS.len()];
            }
            Event::KeyDown(Key::Down) => {
                self.panel = PANELS[(self.panel_index() + 1) % PANELS.len()];
            }
            Event::KeyDown(Key::Left) => self.adjust(ctx, -1),
            Event::KeyDown(Key::Right) => self.adjust(ctx, 1),
            Event::Click { x, y } => {
                if CountyScreen::back_button().contains(x, y)
                    || self.panel.ok_button().contains(x, y)
                {
                    return Transition::Pop;
                }
                // The strip's four quadrants, exactly as CountyStrip_Click
                // splits them. Tested first because the strip never overlaps a
                // panel: every panel window ends at x = 464 at the latest.
                for p in PANELS {
                    if p.strip_hotspot().contains(x, y) {
                        self.panel = p;
                        return Transition::Stay;
                    }
                }
                if self.split_click(ctx, x, y) {
                    return Transition::Stay;
                }
                if self.panel.increase_button().is_some_and(|r| r.contains(x, y)) {
                    self.adjust(ctx, 1);
                } else if self.panel.decrease_button().is_some_and(|r| r.contains(x, y)) {
                    self.adjust(ctx, -1);
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        // OURS: the original has the campaign map here.
        canvas.clear(ink.background);

        let mine = ctx.game.is_players(self.county);
        draw_right_column(canvas, ctx.assets, mine);
        self.draw_strip(ctx, canvas);
        self.draw_panel(ctx, canvas);

        // OURS: the original's bottom strip ends the turn.
        widget::button(canvas, ink, CountyScreen::back_button(), "BACK TO MAP", false);
    }
}

/// `Misc_cty.pl8` frames 54 / 55 / 66 / 56 / 57 / 59, or 54 / 58 / 57 / 59 for
/// a county you do not hold — the whole right column, y 24 … 480 with no gap.
fn draw_right_column(canvas: &mut Canvas, assets: &Assets, own: bool) {
    if let Some(chrome) = assets.chrome.as_ref() {
        if chrome.draw_right_panel(canvas, own) > 0 {
            return;
        }
    }
    // OURS: a flat column, for an install with no Misc_cty.pl8.
    widget::panel(canvas, &assets.ink, Rect::new(478, chrome::PANEL_TOP_Y, 162, 480 - 24));
}

impl CountyScreen {
    /// The county strip: what `CountyStrip_Draw` puts in the 162 × 94 plate, at
    /// its own coordinates.
    fn draw_strip(&self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        let mine = ctx.game.is_players(self.county);

        // Ui_DrawCentred(group 100, ..., 0x1E0, 0xA5, 0xA0): the county's name,
        // centred across 160 pixels. Ours reads "COUNTY n" — the names are in
        // L2.eng group 100 and we do not read L2.eng.
        text::draw_centred(canvas, 478 + 80, 165, &format!("COUNTY {}", self.county), ink.text);

        if !mine {
            // Group 15, "Sovereign land / of", then the owner's name out of
            // g_playerNames. We have the realm number and not the name.
            let owner = if c.owner == 0 {
                "UNCLAIMED".to_string()
            } else {
                format!("REALM {}", c.owner)
            };
            text::draw_centred(canvas, 478 + 80, 240, "SOVEREIGN LAND", ink.dim);
            text::draw_centred(canvas, 478 + 80, 260, &owner, ink.text);
            text::draw_centred(canvas, 478 + 80, 290, "NOT YOURS", ink.bad);
            return;
        }

        // (0x1FC, 0xBD) and (0x25A, 0xBD).
        text::draw(canvas, 508, 189, &c.population.to_string(), ink.text);
        text::draw_right(canvas, 602, 189, &c.happiness.to_string(), ink.text);
        // Group 61, centred in 76 pixels at (0x1E0, 0xD5) and (0x234, 0xD5).
        text::draw_centred(canvas, 480 + 38, 213, g86::STRIP_TAX, ink.dim);
        text::draw_centred(canvas, 564 + 38, 213, g86::STRIP_RATION, ink.dim);
        // (0x1FA, 0xE2), and the ration level centred in 76 at (0x234, 0xE2).
        text::draw(canvas, 506, 226, &format!("{}%", c.tax_rate), ink.text);
        // "Red when it differs from rationWanted" is the original's own rule.
        let colour = if c.ration_achieved == c.ration_wanted { ink.text } else { ink.bad };
        text::draw_centred(canvas, 564 + 38, 226, ration_name(c.ration_achieved), colour);

        // Pl8_DrawFrameHere(g_miscCtySheet, band + 0x46, 0x228, 0xB5) — the
        // five-level health thermometer, in the dead band the hotspot leaves.
        let band = c.health_band.min(4);
        let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| {
            ch.draw_misc(canvas, THERMOMETER_FRAME + band as usize, THERMOMETER.0, THERMOMETER.1)
        });
        if !drawn {
            // OURS: a five-segment bar where the thermometer goes.
            for i in 0..5i32 {
                let lit = 4 - i <= band as i32;
                let y = THERMOMETER.1 + i * 12;
                let colour = if lit { ink.good } else { ink.border };
                canvas.fill_rect(THERMOMETER.0, y, THERMOMETER_W, 10, colour);
            }
        }

        // OURS: the original's quadrants are invisible, because it is a mouse
        // game. A one-pixel outline is how a keyboard player sees where it is.
        widget::frame(canvas, self.panel.strip_hotspot(), ink.highlight);
    }

    fn draw_panel(&self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let (bx, by, cols, rows) = self.panel.box_cells();
        let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| {
            ch.draw_box(canvas, bx, by, cols, rows, 0);
            ch.panels().frame_count() >= 196
        });
        if !drawn {
            // OURS: a flat panel, for an install with no Panels.pl8.
            widget::panel(canvas, ink, self.panel.window());
        }

        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        match self.panel {
            Panel::Population => {
                text::draw(canvas, 20, 56, g73::TITLE, ink.highlight);
                self.draw_graph_stub(ctx, canvas, "POPULATION");
                text::draw(canvas, 176, 240, g73::GREATEST, ink.dim);
                row(canvas, ink, 266, g73::LAST, &c.pop_last.to_string(), ink.text);
                delta_row(canvas, ink, 298, g73::BIRTHS, c.births);
                delta_row(canvas, ink, 314, g73::DEATHS, -c.deaths);
                delta_row(canvas, ink, 330, g73::ARMY, c.army);
                if c.emigrants == 0 {
                    text::draw(canvas, LABEL_X, 346, g73::NO_EMIGRATION, ink.dim);
                } else {
                    delta_row(canvas, ink, 346, g73::EMIGRANTS, -c.emigrants);
                }
                if c.immigrants == 0 {
                    text::draw(canvas, LABEL_X, 362, g73::NO_IMMIGRATION, ink.dim);
                } else {
                    delta_row(canvas, ink, 362, g73::IMMIGRANTS, c.immigrants);
                }
                row(canvas, ink, 386, g73::THIS, &c.population.to_string(), ink.highlight);
            }
            Panel::Happiness => {
                text::draw(canvas, 20, 56, g85::TITLE, ink.highlight);
                self.draw_graph_stub(ctx, canvas, "HAPPINESS");
                text::draw(canvas, 176, 241, g85::AVERAGE, ink.dim);
                text::draw(
                    canvas,
                    176 + text::width(g85::AVERAGE) + 8,
                    241,
                    &c.happiness_avg.to_string(),
                    ink.text,
                );
                row(canvas, ink, 268, g85::LAST, &c.happiness_last.to_string(), ink.text);
                delta_row(canvas, ink, 298, g85::FROM_TAXES, c.shown_tax);
                delta_row(canvas, ink, 314, g85::FROM_RATION, c.shown_ration);
                delta_row(canvas, ink, 330, g85::FROM_HEALTH, c.shown_health);
                delta_row(canvas, ink, 346, g85::FROM_ARMY, c.shown_army);
                delta_row(canvas, ink, 362, g85::FROM_ALE, c.shown_ale);
                delta_row(canvas, ink, 378, g85::FROM_EVENTS, c.shown_events);
                row(canvas, ink, 402, g85::THIS, &c.happiness.to_string(), ink.highlight);
            }
            Panel::Tax => {
                text::draw(canvas, 96, 152, g86::TITLE, ink.highlight);
                text::draw(canvas, 96, 168, g86::RATE, ink.dim);
                text::draw(canvas, 256, 168, &format!("{}%", c.tax_rate), ink.highlight);
                text::draw(canvas, 96, 200, g86::PEOPLE_PAY, ink.dim);
                text::draw(
                    canvas,
                    96 + text::width(g86::PEOPLE_PAY) + 8,
                    200,
                    &format!("{} CROWNS", c.tax_shown),
                    ink.text,
                );
                text::draw(canvas, 96, 232, g86::THIS_COUNTY, ink.dim);
                let empire = ctx
                    .game
                    .kingdom
                    .realms
                    .get(c.owner as usize)
                    .map_or(0i32, |r| r.tax_hap_empire as i32);
                happiness_delta(canvas, ink, 240, 232, c.d_hap_tax_local + empire);
                text::draw(canvas, 96, 256, g86::OTHER_COUNTIES, ink.dim);
                happiness_delta(canvas, ink, 240, 256, c.tax_hap_other);
            }
            Panel::Ration => {
                text::draw_centred(canvas, 128 + 144, 104, g87::TITLE, ink.highlight);
                text::draw(canvas, 144, 136, g87::WANTED, ink.dim);
                text::draw(canvas, 240, 136, ration_name(c.ration_wanted), ink.highlight);
                text::draw(canvas, 144, 161, g87::ACHIEVED, ink.dim);
                let colour = if c.ration_achieved == c.ration_wanted { ink.text } else { ink.bad };
                text::draw(canvas, 240, 161, ration_name(c.ration_achieved), colour);
                happiness_delta(canvas, ink, 340, 161, c.d_hap_ration);
                text::draw(canvas, 144, 186, g87::HEALTH, ink.dim);
                text::draw(canvas, 240, 186, health_name(c.health_band), ink.text);
                happiness_delta(canvas, ink, 340, 186, c.d_hap_health);
                self.draw_split_slider(ctx, canvas, c.ration_split);
                // The Fed row's three fields — county +0x16C, +0x170 and
                // +0x174 — are not in l2-kingdom at all, so there is nothing to
                // put here. docs/screens-county.md §8.4.
                text::draw(canvas, 160, 286, g87::FED, ink.dim);
                text::draw(canvas, 208, 286, "NOT SIMULATED", ink.bad);
                text::draw(canvas, 144, 308, g87::EATEN, ink.dim);
                text::draw_right(canvas, 208, 308, &c.grain_eaten.to_string(), ink.text);
                text::draw_right(canvas, 266, 308, &c.herd_eaten.to_string(), ink.text);
            }
        }

        self.draw_buttons(ctx, canvas);
    }

    /// The tick, and the two arrows the two order panels have.
    fn draw_buttons(&self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let chrome = ctx.assets.chrome.as_ref();
        let ok = self.panel.ok_button();
        if !chrome.is_some_and(|ch| ch.draw_system(canvas, system::OK, ok.x, ok.y)) {
            widget::button(canvas, ink, ok, "OK", false);
        }
        let live = ctx.game.is_players(self.county);
        for (rect, frame, label) in [
            (self.panel.increase_button(), system::ARROW_UP, "+"),
            (self.panel.decrease_button(), system::ARROW_DOWN, "-"),
        ] {
            let Some(r) = rect else { continue };
            if !chrome.is_some_and(|ch| ch.draw_system(canvas, frame, r.x, r.y)) {
                widget::button(canvas, ink, r, label, live);
            }
        }
    }

    /// `Panel_RationSlider`: two caps, a 100-pixel track, and a knob at
    /// `220 + split`.
    fn draw_split_slider(&self, ctx: &Ctx, canvas: &mut Canvas, split: i32) {
        let ink = &ctx.assets.ink;
        let chrome = ctx.assets.chrome.as_ref();
        // The original draws a line at y 229 in colour 0x10, a 100 x 4 fill at
        // y 230 in 0x3F and a second line at y 234 in 0x1F. The colours are
        // ours; the geometry is the original's.
        canvas.fill_rect(SLIDER_TRACK_X, 229, SLIDER_TRACK_W, 1, ink.border);
        canvas.fill_rect(SLIDER_TRACK_X, 230, SLIDER_TRACK_W, 4, ink.panel);
        canvas.fill_rect(SLIDER_TRACK_X, 234, SLIDER_TRACK_W, 1, ink.dim);
        let caps = chrome.is_some_and(|ch| {
            let l = ch.draw_system(canvas, system::SLIDER_CAP_LEFT, SLIDER_CAP_LEFT_X, SLIDER_Y);
            let r = ch.draw_system(canvas, system::SLIDER_CAP_RIGHT, SLIDER_CAP_RIGHT_X, SLIDER_Y);
            l && r
        });
        if !caps {
            widget::button(canvas, ink, split_down_button(), "<", false);
            widget::button(canvas, ink, split_up_button(), ">", false);
        }
        let knob_x = SLIDER_KNOB_ORIGIN + split.clamp(0, MAX_RATION_SPLIT);
        if !chrome.is_some_and(|ch| ch.draw_system(canvas, system::SLIDER_KNOB, knob_x, SLIDER_Y)) {
            canvas.fill_rect(knob_x, SLIDER_Y, system::SLIDER_KNOB_W, 32, ink.highlight);
        }
    }

    /// **A stub, and it looks like one.** `Ui_HistoryGraph` fills this
    /// rectangle from `g_countyHistory`; `l2-kingdom` has no history array, so
    /// there is nothing to plot.
    fn draw_graph_stub(&self, ctx: &Ctx, canvas: &mut Canvas, what: &str) {
        let ink = &ctx.assets.ink;
        let Some(r) = self.panel.graph_rect() else { return };
        canvas.fill_rect(r.x, r.y, r.w, r.h, ink.background);
        widget::frame(canvas, r, ink.border);
        let mid = r.y + r.h / 2;
        text::draw_centred(canvas, r.centre_x(), mid - 10, &format!("{what} HISTORY"), ink.dim);
        text::draw_centred(canvas, r.centre_x(), mid + 2, "NOT SIMULATED", ink.bad);
    }
}

fn ration_name(level: i32) -> &'static str {
    RATION_NAMES
        .get(level.clamp(0, RATION_LEVEL_COUNT as i32 - 1) as usize)
        .copied()
        .unwrap_or("?")
}

fn health_name(band: u8) -> &'static str {
    HEALTH_BAND_NAMES.get(band as usize).copied().unwrap_or("?")
}

/// A label in the panel's left column and a value right-anchored in its value
/// column — x = 48 and x = 336, which is what both graph panels use.
fn row(canvas: &mut Canvas, _ink: &Ink, y: i32, label: &str, value: &str, colour: u8) {
    text::draw(canvas, LABEL_X, y, label, colour);
    text::draw_right(canvas, VALUE_RIGHT, y, value, colour);
}

/// `Ui_DrawDelta(value, 0, ...)`, including the part that is easy to miss:
/// **a zero row draws no number at all**, rather than a `0`.
fn delta_row(canvas: &mut Canvas, ink: &Ink, y: i32, label: &str, value: i32) {
    text::draw(canvas, LABEL_X, y, label, ink.dim);
    if value == 0 {
        return;
    }
    let colour = if value < 0 { ink.bad } else { ink.good };
    text::draw_right(canvas, VALUE_RIGHT, y, &widget::signed(value), colour);
}

/// `Ui_DrawHappinessDelta`: `( ±n <face> )`. The face is `Misc_cty` frame 0x17
/// and we do not draw it; everything else is the original's shape.
fn happiness_delta(canvas: &mut Canvas, ink: &Ink, x: i32, y: i32, value: i32) {
    let colour = if value < 0 { ink.bad } else { ink.good };
    text::draw(canvas, x, y, &format!("({})", widget::signed(value)), colour);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x + inner.w <= outer.x + outer.w
            && inner.y + inner.h <= outer.y + outer.h
    }

    /// The four windows are the four `Ui_DrawBox` calls, and each holds its own
    /// tick button. A window that ran off the screen, or a tick outside it,
    /// would mean a cell count or a button coordinate was misread.
    #[test]
    fn every_panel_window_is_on_screen_and_holds_its_own_ok_button() {
        for p in PANELS {
            let w = p.window();
            assert!(w.x >= 0 && w.y >= 0, "{p:?} starts on screen");
            assert!(w.x + w.w <= 640 && w.y + w.h <= 480, "{p:?} ends on screen: {w:?}");
            assert!(inside(w, p.ok_button()), "{p:?}: the tick is outside its window {w:?}");
        }
    }

    /// The two order panels' arrows sit inside their own windows, on one line,
    /// with the up arrow to the *left* of the down arrow; the two panels that
    /// only report have none.
    ///
    /// The two pairs are spaced differently and that is the original's, not a
    /// slip: `g_taxWidgets` puts them at x 192 and 224 — a frame is 24 wide, so
    /// eight pixels apart — and `g_rationWidgets` at 344 and 368, flush.
    #[test]
    fn only_the_two_order_panels_have_arrows_and_both_pairs_are_inside_them() {
        for p in PANELS {
            match p {
                Panel::Tax | Panel::Ration => {
                    let up = p.increase_button().expect("an up arrow");
                    let down = p.decrease_button().expect("a down arrow");
                    assert!(inside(p.window(), up), "{p:?} up arrow {up:?} escapes its window");
                    assert!(inside(p.window(), down), "{p:?} down arrow escapes its window");
                    assert!(up.x < down.x, "{p:?}: up is the left of the pair");
                    assert_eq!(up.y, down.y, "{p:?}: and they are on one line");
                    assert!(
                        down.x - up.x >= system::ARROW,
                        "{p:?}: the pair may touch but must not overlap"
                    );
                }
                _ => {
                    assert!(p.increase_button().is_none(), "{p:?} takes no orders");
                    assert!(p.decrease_button().is_none(), "{p:?} takes no orders");
                }
            }
        }
        assert_eq!(
            Panel::Tax.decrease_button().unwrap().x - Panel::Tax.increase_button().unwrap().x,
            32,
            "g_taxWidgets records 0 and 1 are at x 192 and 224"
        );
        assert_eq!(
            Panel::Ration.decrease_button().unwrap().x
                - Panel::Ration.increase_button().unwrap().x,
            24,
            "g_rationWidgets records 0 and 1 are at x 344 and 368, flush"
        );
    }

    /// `CountyStrip_Click`'s four quadrants do not overlap, all four sit inside
    /// the 162 x 94 plate, and the gap between the two columns is exactly where
    /// the health thermometer is drawn — which is why the gap exists.
    #[test]
    fn the_strip_quadrants_fit_the_plate_and_leave_the_thermometer_unclickable() {
        let rects: Vec<Rect> = PANELS.iter().map(|p| p.strip_hotspot()).collect();
        for (i, a) in rects.iter().enumerate() {
            assert!(inside(STRIP, *a), "quadrant {i} {a:?} escapes the plate {STRIP:?}");
            for (j, b) in rects.iter().enumerate().skip(i + 1) {
                let over =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!over, "quadrants {i} and {j} overlap: {a:?} {b:?}");
            }
        }
        for x in THERMOMETER.0..THERMOMETER.0 + THERMOMETER_W {
            for r in &rects {
                assert!(!r.contains(x, 200), "thermometer column {x} is clickable in {r:?}");
            }
        }
        // And the four between them cover both columns and both rows, so no
        // quadrant is a degenerate strip.
        for r in &rects {
            assert!(r.w > 40 && r.h > 20, "{r:?} is too small to click");
        }
    }

    /// The slider closes: the caps meet the track with no gap and no overlap,
    /// and the knob's travel is the track's width.
    #[test]
    fn the_split_sliders_caps_track_and_knob_travel_agree() {
        assert_eq!(SLIDER_CAP_LEFT_X + system::SLIDER_CAP, SLIDER_TRACK_X, "left cap meets track");
        assert_eq!(SLIDER_TRACK_X + SLIDER_TRACK_W, SLIDER_CAP_RIGHT_X, "track meets right cap");
        let centre = |v: i32| SLIDER_KNOB_ORIGIN + v + system::SLIDER_KNOB_W / 2;
        assert_eq!(centre(0), SLIDER_TRACK_X + 1, "at 0 the knob centres on the track's start");
        assert_eq!(
            centre(MAX_RATION_SPLIT),
            SLIDER_TRACK_X + SLIDER_TRACK_W + 1,
            "at 100 it centres on the track's end"
        );
        assert!(inside(Panel::Ration.window(), split_track()), "the track is inside the panel");
    }

    /// No panel window reaches the county strip, which is what lets the click
    /// handler test the strip's quadrants before the panel's own widgets.
    #[test]
    fn no_panel_window_reaches_the_county_strip() {
        for p in PANELS {
            let w = p.window();
            assert!(
                w.x + w.w <= STRIP.x,
                "{p:?} at {w:?} overlaps the strip column at x = {}",
                STRIP.x
            );
        }
    }
}
