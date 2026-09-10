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
//! underneath, and each has **two ways out**: the 24 × 24 picture in its
//! bottom-right corner, which is a live hotspot, and the **right mouse button**
//! anywhere. That picture is a cursor arrow pointing into a black hole — a
//! close button whose artwork is the instruction, and not the tick this file
//! used to call it. `docs/screens-county.md` §2.6 and §2.7.
//!
//! So this screen draws the strip at the original's own coordinates, over the
//! original's own `Misc_cty.pl8` plates, with the original's own quadrants
//! live; and it draws one of the four panels at the original's own rectangle,
//! with the original's own rows in the original's own order, using the
//! original's `Panels.pl8` box kit and `System2.pl8` buttons.
//!
//! # What is still ours, and says so
//!
//! * **The font, on the four panels.** The original draws `Fntl2_14.pl8` for
//!   body lines and `Fntl2_22.pl8` for headings; the panels still use our own
//!   5 × 7 font at the original's coordinates. **The strip does not** — it is
//!   `Fntl2_9.pl8`, which is the only place in the game that font is used, and
//!   [`draw_strip`] draws it where the install has it.
//! * **The history graph.** `Ui_HistoryGraph` fills 402 × 155 of both the
//!   population and the happiness panel from `g_countyHistory` — 400 turns ×
//!   16 counties × 8 bytes, and part of the save. `l2-kingdom` keeps no
//!   history, so that rectangle is an empty recess that says so. It is a stub
//!   and it is meant to look like one.
//! *(**Fixed.** This list used to carry a fourth entry: "what is behind the
//! panels — the original has the campaign map there; the map screen is another
//! file, so this one paints a flat ground." It no longer does. The panels are
//! [overlays](crate::screen::Screen::is_overlay) and the machine paints the map
//! screen beneath them, which is the same correction the village needed —
//! `docs/decisions.md` C22.)*
//! * **The strings on the panels.** Ours, transcribed from the `L2.eng` group
//!   each row names. The strip reads `L2.eng` properly — group 100 for the
//!   county's name, 61 for its two captions and 21 for the ration level — and
//!   falls back to the transcriptions when the install has no `L2.eng`.
//! *(**Fixed.** This list used to carry a fifth entry: "the bottom strip reads
//! BACK TO MAP; the original's is End Turn, which is the map screen's
//! business." It was a rectangle of ours drawn over — and hit-tested ahead of —
//! a live control of the game's, at (478, 460), which is `g_sidebarButtons`
//! record 5 to the pixel. It is gone, and the whole right-hand column is now
//! passed down to the map screen the way `Screen_FrameInput`'s six guards pass
//! it.)*
//!
//! # What is not here at all
//!
//! Labour, sowing, ale and field types, because **none of them is on a county
//! panel in the original either**. Peasants are moved by rubber-band drag on
//! the village screen (0x02), ale is bought from the merchant (0x08), and
//! fields are painted on the map. `docs/screens-county.md` §6.4 and §6.5.

use l2_kingdom::tables::{
    HEALTH_BAND_NAMES, JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT, RATION_NAMES,
};
use l2_view::chrome::{self, misc_cty, system};
use l2_view::{text, Canvas, Ink};

use crate::game::{MAX_RATION_SPLIT, MAX_TAX_RATE};
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

/// The county strip's three farming rows, by labour slot, as
/// (plain, below the floor, **above the ceiling — the blue ring**).
///
/// **`[V]`.** Every drawer in `CountyStrip_Draw`'s left-hand row loop asks the
/// same question first: `if (county.labour[slot].useful <
/// county.labour[slot].workers)` — *more people on this job than it can use* —
/// and draws the ringed frame two pixels up and left of where the plain one
/// goes. That is the signal the player described as *"a blue outline for idle
/// peasants (eg too many on dairy)"*, and dairy is the second row.
///
/// A `None` in the middle column is a job whose drawer has no shortfall frame
/// at all: `FUN_004103C5` tests only the ceiling. The frame numbers themselves
/// are [`misc_cty::RINGED_PAIRS`](l2_view::chrome::misc_cty::RINGED_PAIRS).
///
/// | slot | drawer | job |
/// |---|---|---|
/// | 0 | `FUN_0041023A` | grain farming |
/// | 1 | `FUN_004100AF` | cattle farming — the dairy |
/// | 2 | `FUN_004103C5` | field reclamation |
const PRODUCE_ICONS: [(usize, Option<usize>, usize); 3] = [
    (misc_cty::RINGED_PAIRS[0].0, Some(misc_cty::SHORTFALL_GRAIN), misc_cty::RINGED_PAIRS[0].1),
    (misc_cty::RINGED_PAIRS[1].0, Some(misc_cty::SHORTFALL_CATTLE), misc_cty::RINGED_PAIRS[1].1),
    (misc_cty::RINGED_PAIRS[2].0, None, misc_cty::RINGED_PAIRS[2].1),
];

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

    /// `Ui_OkButton(x, y, 0)` — the 24 × 24 picture in the panel's own corner,
    /// and the box `Ui_OkButtonClicked` (`0x0040E7E4`) hit-tests on a left
    /// release.
    ///
    /// **It is not a tick.** `System.pl8` frame `0x33` decodes to a cursor
    /// arrow pointing into a small black hole: a close button whose artwork is
    /// the instruction. It is a real target *and* the right button closes the
    /// panel from anywhere (`docs/screens-county.md` §2.6), so the game offers
    /// two ways out and so do we. §2.7; the name `OK` is ours and is kept.
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

// ------------------------------------------------- the produce rows, and
//                                                    the popup they open
//
// `FUN_0040FEC1` (`0x0040FEC1`) lays the 162 x 128 plate at y = 302 out into two
// columns, `CountyStrip_Draw` picks the two row pitches, and
// `CountyStrip_JobClick` (`0x00438E3B`) turns a press into a job popup. The
// three are one fact and live together here.

/// `CountyStrip_JobClick`'s hit box: `x 0x1DE … 0x27F, y 0x12E … 0x1AD`, and the
/// column split at `0x230`.
const JOBS_Y0: i32 = 0x12E;
const JOBS_Y1: i32 = 0x1AE;
const JOBS_SPLIT_X: i32 = 0x230;

/// **`FUN_0040FEC1`'s left-hand list**, verbatim and in its own order: cattle,
/// then grain, then reclamation — each entry the **labour slot** the row is
/// about, which is also what `CountyStrip_JobClick` writes into `g_jobPanelJob`
/// plus one.
pub fn farm_rows(c: &l2_kingdom::county::County) -> Vec<usize> {
    let mut rows = Vec::with_capacity(3);
    if c.fields_cattle != 0 || c.herd != 0 {
        rows.push(1);
    }
    if c.fields_grain != 0 || c.grain != 0 {
        rows.push(0);
    }
    if c.fields_reclaiming != 0 {
        rows.push(2);
    }
    rows
}

/// **`FUN_0040FEC1`'s right-hand list**, and its order is not the industry
/// array's: wood, iron, stone, weapons, then the castle.
///
/// Each of the four is gated on `industry[n].enabled` — county `+0x297 + n*0x18`,
/// the byte `Industry_ToggleFromMap` (`0x0043D309`) XORs when you click the
/// building on the campaign map — and the castle row on `castleDegraded`.
pub fn industry_rows(c: &l2_kingdom::county::County) -> Vec<usize> {
    let mut rows = Vec::with_capacity(5);
    for (industry, slot) in [(0usize, 6usize), (1, 4), (3, 5), (2, 7)] {
        if c.industry[industry].enabled {
            rows.push(slot);
        }
    }
    if c.castle_degraded != 0 {
        rows.push(3);
    }
    rows
}

/// `DAT_0053E970` and `DAT_0053F04C`, the two row pitches, which
/// `CountyStrip_Draw` picks from the two counts.
///
/// **They are not the same rule.** The farm column has two cases and the
/// industry column three, because the industry column can hold five rows:
///
/// ```c
/// DAT_0053E970 = farmRows     < 3 ? 0x3C : 0x2D;
/// DAT_0053F04C = industryRows < 3 ? 0x3C : industryRows < 4 ? 0x2D : 0x1E;
/// ```
pub fn farm_pitch(rows: usize) -> i32 {
    if rows < 3 {
        0x3C
    } else {
        0x2D
    }
}

pub fn industry_pitch(rows: usize) -> i32 {
    if rows < 3 {
        0x3C
    } else if rows < 4 {
        0x2D
    } else {
        0x1E
    }
}

/// **`CountyStrip_JobClick` (`0x00438E3B`)** — which labour slot a press in the
/// sidebar's lower plate opens the job popup for, or `None`.
///
/// The caller supplies the ownership gate, which is the function's own first
/// line: `if (g_counties[g_selectedCounty].owner == g_localPlayer)`. Everything
/// else is here, including the two ways it refuses — a row index past the end of
/// its list returns 0 rather than falling through to the other column.
pub fn job_row_at(c: &l2_kingdom::county::County, x: i32, y: i32) -> Option<usize> {
    if !(478..640).contains(&x) || !(JOBS_Y0..JOBS_Y1).contains(&y) {
        return None;
    }
    let (rows, pitch) = if x < JOBS_SPLIT_X {
        let rows = farm_rows(c);
        let pitch = farm_pitch(rows.len());
        (rows, pitch)
    } else {
        let rows = industry_rows(c);
        let pitch = industry_pitch(rows.len());
        (rows, pitch)
    };
    let n = ((y - JOBS_Y0) / pitch) as usize;
    rows.get(n).copied()
}

/// `CountyStrip_Click` (`0x00438CEB`) itself: which panel a pixel opens, or
/// `None` for the outer guard and for the thermometer's dead band.
///
/// **This is the whole navigation into the four panels** — §2.3 — so it lives
/// here as one function rather than as four rectangles the caller loops over,
/// and the campaign map and the county screen both call it.
pub fn panel_at(x: i32, y: i32) -> Option<Panel> {
    PANELS.into_iter().find(|p| p.strip_hotspot().contains(x, y))
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
    /// Opens on the panel the strip quadrant that was clicked names, which is
    /// the only way the original opens any of them ([`panel_at`]).
    pub fn new(county: u8, panel: Panel) -> CountyScreen {
        CountyScreen { county, panel }
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
        ScreenId::County(self.county, self.panel)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("County {}", self.county)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **The whole right-hand column is live under an open panel**, and the
        // arm says so rather than leaving it to be inferred. `0x14`'s, verbatim:
        //
        // ```c
        // if (FUN_0043292D() == 0 && FUN_00432967() == 0 &&      /* modes, sidebar  */
        //     CountyStrip_Click() == 0 && FUN_00439122() == 0 && /* strip, split    */
        //     CountyStrip_JobClick() == 0 && FUN_00439079() == 0) {
        //   if (!rightReleased) { if (Ui_OkButtonClicked()) { g_screenId = 0; } }
        //   else                                            { g_screenId = 0; }
        // }
        // ```
        //
        // Six guards, every one of them hit-testing `x >= 0x1DE`, and every one
        // of them ahead of the panel's own two ways out. `0x15` and `0x16` are
        // the same six; `0x19` inserts `Ration_SliderClick` after the second,
        // which is why the split *below* is tested here and the column is not.
        //
        // Two things follow. Clicking the strip while a panel is open
        // **switches** panel rather than closing it — which is how a player goes
        // from population to tax without a trip via the map — and the five
        // sidebar buttons, the minimap, its four mode icons, the farm/industry
        // slider and the produce rows all keep working with a panel up. None of
        // that was reproduced: this screen tested the four strip quadrants
        // itself and swallowed everything else in the column.
        //
        // [`Transition::Pass`] is the whole of it, and it is the same mechanism
        // the village already used for the same six guards
        // (`docs/decisions.md` C59). The strip quadrants go down with the rest,
        // so the map's own `CountyStrip_Click` answers and its `Push` lands at
        // the map's depth — which is `g_screenId = 0x14` and not a second panel
        // stacked on the first.
        //
        // The six guards themselves are the map screen's and are recorded there;
        // the arm here is that *this screen's ladder runs them first*, which is
        // one predicate shared with the village — so one record, and it lives on
        // [`crate::screens::belongs_to_the_right_column`].
        if crate::screens::belongs_to_the_right_column(event) {
            return Transition::Pass;
        }
        match event {
            // **`Ui_DrawBox` panels are dismissed by the right button**, and the
            // game says so in its own words: `Screen_SliderBox` prints `L2.eng`
            // group 12 index 0, *"Click Right to Exit"*, under its caption.
            // `Screen_FrameInput`'s arm for each of `0x14`, `0x15`, `0x16` and
            // `0x19` is the same shape — the strip, the sidebar and the ration
            // slider are tested first, so a click on those *switches* panel
            // rather than closing it, and only then does a right release set
            // `g_screenId = 0`.
            //
            // It closes from *anywhere*, the strip included: every guard in
            // that chain tests a left press or a left release, so none of them
            // consumes a right one.
            //
            // arm: 0x0042FF10/panel-right-closes
            Event::RightClick { .. } => return Transition::Pop,
            // **Ours, and counted.** The original has no keyboard route out of a
            // panel and none into another one: its only `VK_ESCAPE` handler
            // quits the game, and the four panels are four screen ids with no
            // ordering between them at all. Kept because a keyboard player has
            // nothing else, and recorded in `docs/arms.json` as an invention
            // rather than left as a comment admitting a choice — which is what
            // `docs/decisions.md` C61 found nine of.
            // arm: ours/county-panel-keyboard
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
                // `Ui_OkButtonClicked` (`0x0040E7E4`) — the 24 × 24 corner
                // picture the panel's own `Ui_OkButton` call stashed.
                //
                // **The BACK TO MAP button that used to be tested here is gone.**
                // It was ours, it was drawn at (478, 460), and that is the
                // original's **End Turn** strip to the pixel — record 5 of
                // `g_sidebarButtons`. So a rectangle of ours sat on top of a
                // live control of the game's, which is the same defect a player
                // reported about the five sidebar icons a fortnight ago. The
                // column now passes down and the strip ends the turn.
                // arm: 0x0040E7E4/panel-corner-closes
                if self.panel.ok_button().contains(x, y) {
                    return Transition::Pop;
                }
                // The strip's four quadrants are in the column and went down
                // with it, so nothing is tested for them here.
                // `Ration_SliderClick` (`0x0043A379`) — `0x19`'s own extra
                // guard, and the reason the ration panel's arm is one line
                // longer than the other three.
                // arm: 0x0043A379/ration-split-slider
                if self.split_click(ctx, x, y) {
                    return Transition::Stay;
                }
                // `Screen_HandleInput`'s widget tables: `g_taxWidgets`
                // (`0x004DD790`) and `g_rationWidgets` (`0x004DD7C0`), two
                // records each, up then down.
                // arm: 0x004BA9C8/tax-and-ration-arrows
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

    /// **These are four windows over the campaign map**, which is what this
    /// module's own header has said all along — *"each is a floating
    /// `Ui_DrawBox` window over whatever was underneath"* — while `draw` went
    /// on clearing the screen and painting a flat ground where the map should
    /// be. The village's correction (`docs/decisions.md` C22) is the same
    /// mistake one screen along, and this is the other half of it.
    fn is_overlay(&self) -> bool {
        true
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        // **No clear.** The campaign map is the screen underneath on the stack.

        // **The seven column plates are no longer repainted here, and that is a
        // one-line change with a visible consequence.**
        //
        // This used to call `draw_right_column`, which puts `Misc_cty` frames 54
        // … 59 down over the whole column, y 24 … 480 — including the **minimap**
        // at (480, 25), the four mode icons beside it and the End Turn caption,
        // none of which this screen redraws. So *opening a county panel blanked
        // the minimap.* That was harmless while the column was dead under a
        // panel; now that every control in it is live it would be a control a
        // player can click and cannot see.
        //
        // The original does not repaint it either: `Screen_Draw`'s `0x14`,
        // `0x15`, `0x16` and `0x19` arms are `Panel_Population` and its three
        // siblings, and not one of them calls `CountyStrip_Draw` or
        // `Minimap_Draw`. The column is simply still there from the last frame —
        // §3.1's *"there is no screen clear anywhere in this engine"*.
        //
        // The **strip** is still drawn, and only because it is the one part of
        // the column that carries the focus outline (ours) and because the plate
        // it lands on is identical to the one the map screen drew a moment ago.
        draw_strip(ctx, canvas, self.county, Some(self.panel));
        self.draw_panel(ctx, canvas);

        // **The bottom strip is not ours to draw.** It is the campaign map's
        // End Turn button, the map screen is underneath on the stack and draws
        // it, and the column now passes clicks down to it. A BACK TO MAP
        // rectangle of ours used to be painted over it — and hit-tested ahead
        // of it — from right here.
    }
}

// -------------------------------------------------------- drawing the strip
//
// A free function, not a method, because **the campaign map draws it too**.
// `Screen_DrawCampaign` puts `CountyStrip_Draw` in the sidebar of the map
// itself; until now our map screen drew a box of our own numbers over the jobs
// plate below it and left this plate empty, which is the thing a player looks
// at every turn and the one place the original's own layout was going spare.

/// The one line of text the strip's font draws.
///
/// `CountyStrip_Draw` uses **`Fntl2_9.pl8`**, and it is the only caller of that
/// font in the whole game (`docs/screens-county.md` §2.1). We draw it where the
/// install has it and fall back to our own 5 × 7 font where it does not, so the
/// *layout* is the original's on every machine and the *letters* are only ours
/// on a machine with no game.
///
/// `colour` is a resolved palette index rather than one of `shell::font`'s
/// constants, because one of the rules here is a colour: the achieved ration is
/// **red when it differs from the wanted one**. That rule is the original's; the
/// index we spell red with is ours, out of [`Ink`].
fn strip_text(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) {
    match ctx.assets.shell.small.as_ref() {
        Some(f) => {
            // `CountyStrip_Draw` sets `DAT_005AEA40 = 1` for the whole numeric
            // block and clears it after, and that global switches
            // `Ui_DrawText`'s emboss **off**. The strip's numbers are flat.
            let style = crate::shell::font::Style { colour, shadow: None, caps: None };
            f.draw(canvas, x, y, s, &style);
        }
        None => {
            text::draw(canvas, x, y, s, colour);
        }
    }
}

/// The same, centred in `width` from `x` — `Ui_DrawCentred`, which clamps the
/// offset at zero rather than letting a long string start left of its box.
///
/// Public under a longer name because the End Turn caption is drawn in this
/// font too (`Screen_DrawEndTurn`), and it is the map screen that draws it.
pub fn strip_centred_at(
    ctx: &Ctx,
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    s: &str,
    colour: u8,
) {
    strip_centred(ctx, canvas, x, y, width, s, colour)
}

fn strip_centred(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, width: i32, s: &str, colour: u8) {
    let w = match ctx.assets.shell.small.as_ref() {
        Some(f) => f.width(s),
        None => text::width(s),
    };
    strip_text(ctx, canvas, x + ((width - w) / 2).max(0), y, s, colour);
}

/// One line in the **body** font (`Fntl2_14.pl8`), centred in `width` from `x`.
///
/// The strip's own font is the 9-pixel one, but the county's name and the
/// three "sovereign land of …" lines are drawn with `g_fontBody`, embossed —
/// `DAT_005AEA40` is only set for the numeric block between them.
/// `Ui_DrawNumberRight`'s anchoring: right-aligned in `w` pixels from `x`.
///
/// **Flat, not embossed.** Each produce row sets `DAT_005AEA40 = 1` around its
/// number and clears it after — the same switch the strip's own figures are
/// drawn under — and that global turns `Ui_DrawText`'s emboss off.
/// **`Ui_DrawDelta` (`0x00402E0C`) — the produce rows' signed forecast.**
///
/// A player: *"Sidebar doesn't show grain being planted as a negative number."*
/// This is the routine that would have. The original, in full:
///
/// ```c
/// if (value == 0 && mode == 0) return;                     /* nothing at all */
/// Ui_DrawText(prefix, x, y, font, value < 0 ? colourNeg : colourPos);
/// if      (mode == 2) Ui_DrawNumber( value, '@', suffix, x + g_penAdvance, …, colourPos);
/// else if (value < 0) Ui_DrawNumber(-value, '-', suffix, x + g_penAdvance, …, colourNeg);
/// else if (value < 1) Ui_DrawNumber( value, '@', suffix, x + g_penAdvance, …, colourPos);
/// else                Ui_DrawNumber( value, '+', suffix, x + g_penAdvance, …, colourPos);
/// ```
///
/// Four things in it are worth having exactly, and three of them are the sort a
/// reimplementation drops without noticing:
///
/// * **The minus is a lead *character*, not a mark.** `Ui_DrawNumber` writes it
///   over `g_numberBuffer[0]`, the slot `Ui_NumberToBuffer(value, 1, 0)` leaves
///   free for a sign, so sign and digits go out in one `Ui_DrawText`. There is
///   no separate glyph to place or to lose.
/// * **A positive value carries an explicit `'+'`.** Only the *sign* tells the
///   player which way a forecast runs; the row has no other cue.
/// * **`mode == 0` and a value of zero draw nothing whatever.** All eight
///   produce rows pass mode 0. That is why an absent delta has read as a quiet
///   row rather than as an obvious hole — a county with nothing happening looks
///   the same either way.
/// * **The colour is the sign too**: `0xFA` positive, `0xF9` negative, at every
///   one of the eight call sites.
///
/// The prefix and the suffix are a single space at all eight — read out of
/// `Lords2.exe` at `0x004D3D40 … 0x004D3D84`, where the only one that is not
/// `" "` is the tax rate's `"%"`. They are drawn as two separate strings, so
/// [`TRAILING`](crate::shell::TRAILING)'s four pixels fall between
/// the prefix and the number and **not** between the number and its suffix.
/// Concatenating the three into one string would lose those four pixels, which
/// is the whole reason this is not a `format!`.
///
/// **Ours:** the font. The original uses `g_font10` — preload entry 5,
/// `font_10` — and this workspace loads `fntl2_9`, `fntl2_14` and `fntl2_22`
/// and not that one. The 9-pixel font is the closest we have and the row is a
/// pixel short because of it; loading `font_10` is its own job.
fn strip_delta(ctx: &Ctx, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    // `if ((value != 0) || (mode != 0))` — every produce row passes mode 0.
    if value == 0 {
        return;
    }
    let colour = if value < 0 { DELTA_NEG } else { DELTA_POS };
    let lead = if value < 0 { '-' } else { '+' };
    // `Ui_DrawText(prefix, x, y, font, colour)`, then the number at
    // `x + g_penAdvance` — which is the prefix's width plus `Ui_DrawText`'s own
    // four trailing pixels, not the prefix's width alone.
    let prefix = " ";
    let advance = match ctx.assets.shell.small.as_ref() {
        Some(f) => f.width(prefix),
        None => text::width(prefix),
    } + crate::shell::TRAILING;
    strip_text(ctx, canvas, x, y, prefix, colour);
    // `Ui_DrawNumber(|value|, lead, suffix, …)` — one string, lead in slot 0.
    strip_text(ctx, canvas, x + advance, y, &format!("{lead}{} ", value.abs()), colour);
}

/// `Ui_DrawDelta`'s `colourPos`, the eighth argument at all eight produce-row
/// call sites.
const DELTA_POS: u8 = 0xFA;

/// `Ui_DrawDelta`'s `colourNeg`, the ninth. It is the same index
/// [`font::HIGHLIGHT`](crate::shell::font::HIGHLIGHT) carries and they are kept
/// apart on purpose: that one is *"this is the thing you are looking at"* and
/// this one is *"this number is negative"*, and a rename of either must not
/// drag the other.
const DELTA_NEG: u8 = 0xF9;

/// **`Ui_DrawNumberRight` (`0x004030C6`) centres.** It is not right-aligned and
/// it never was: its tail is `FUN_004025D7(buf, x, y, width, font, colour)`,
/// whose whole body is
///
/// ```c
/// Ui_DrawText(str, x + max(0, (width - Ui_TextWidth(str, font)) / 2), y, font, colour);
/// ```
///
/// The name is the original's shape rather than ours — `docs/symbols.json`'s
/// comment said *"Ui_DrawNumber, right-aligned inside width"* and that comment
/// is corrected on this branch. Two draw audits found it independently in the
/// same week, which is the usual sign that a name has been believed instead of
/// read.
///
/// It lands here: the produce rows' stock figure was anchored at x = 540 and
/// belongs centred between 480 and 540. This helper used to be `body_right` and
/// used to do that, which is the same defect the row's missing delta was
/// reported alongside — *"grain not shown as a negative"* and *"grain in the
/// wrong place"* would have looked like one complaint.
fn body_centred_in(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, w: i32, s: &str, colour: u8) {
    let style = crate::shell::font::Style { colour, shadow: None, caps: None };
    body_centred_styled(ctx, canvas, x, y, w, s, style);
}

/// Centred in `width` from `x`, with the emboss pair chosen by the caller — because
/// `CountyStrip_Draw` uses **two different ones** in the same plate. See
/// [`crate::shell::font::SHADOW_GREY`].
fn body_centred_styled(
    ctx: &Ctx,
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    w: i32,
    s: &str,
    style: crate::shell::font::Style,
) {
    match ctx.assets.shell.body.as_ref() {
        Some(f) => {
            f.draw_centred(canvas, x, y, w, s, &style);
        }
        None => {
            text::draw_centred(canvas, x + w / 2, y, s, style.colour);
        }
    }
}

/// The county's name: `L2.eng` group 100, index `scenarioIndex * 20 + countyId`
/// — and `g_scenarioIndex` *is* the map slot ([`crate::game::Game::map_slot`]).
///
/// Falls back to `COUNTY n` for an install with no `L2.eng`, which is also what
/// the tests run against.
pub fn county_name(ctx: &Ctx, id: u8) -> String {
    let index = ctx.game.map_slot * 20 + id as usize;
    let name = ctx.assets.shell.text(100, index);
    if name.is_empty() {
        format!("COUNTY {id}")
    } else {
        name.to_string()
    }
}

/// One `L2.eng` string with a fallback, for the two the strip needs by name.
fn eng(ctx: &Ctx, group: usize, index: usize, fallback: &str) -> String {
    let s = ctx.assets.shell.text(group, index);
    if s.is_empty() {
        fallback.to_string()
    } else {
        s.to_string()
    }
}

/// The ration level's name — `L2.eng` group 21, which is what `CountyStrip_Draw`
/// indexes with county `+0x15D`.
fn ration_label(ctx: &Ctx, level: i32) -> String {
    let level = level.clamp(0, RATION_LEVEL_COUNT as i32 - 1);
    eng(ctx, 21, level as usize, ration_name(level))
}

/// **The county strip: what `CountyStrip_Draw` (`0x0040F7D3`) puts in the
/// 162 × 94 plate at (478, 156), at its own coordinates.**
///
/// `focus` outlines one quadrant. That outline is ours — the original's
/// quadrants are invisible because it is a mouse game — and it is drawn only
/// when a panel is actually open, so the map's sidebar carries none.
pub fn draw_strip(ctx: &Ctx, canvas: &mut Canvas, county: u8, focus: Option<Panel>) {
    let ink = &ctx.assets.ink;
    let Some(c) = ctx.game.kingdom.counties.get(county as usize) else { return };
    let mine = ctx.game.is_players(county);
    // **The strip's ink is the original's literal `0x3F`, which is black.**
    //
    // Every string `CountyStrip_Draw` writes — the county's name, its
    // population, its happiness, the tax rate, both group-61 captions — passes
    // colour `0x3F` to `Ui_DrawText`, and `0x3F` in `Base01.256` is
    // `rgb(0, 0, 0)`. The one exception is the achieved ration when it is not
    // the wanted one, which is `0xF9`.
    //
    // These were `ink.text`, which resolves to *white*, and a player reported
    // it: *"the text should be black not white over the happiness."* He is
    // right, and the reason [`Ink`](l2_view::Ink) is not the answer here is
    // that this text is written on the **original's own plate** — `Misc_cty`
    // frame `0x37` — so the index is a reading of the binary and not a choice
    // of ours. With no chrome loaded there is no plate to be black on, and the
    // fallback is our own ink.
    let strip_ink =
        if ctx.assets.chrome.is_some() { crate::shell::font::TEXT } else { ink.text };
    let strip_bad =
        if ctx.assets.chrome.is_some() { crate::shell::font::HIGHLIGHT } else { ink.bad };

    // `Ui_DrawCentred(100, scenarioIndex*0x14 + county, 0x1E0, 0xA5, 0xA0,
    // &g_fontBody, 0x3F)` — the county's name, centred across 160 pixels at
    // (480, 165), and **in the 14-pixel body font, not the strip's 9-pixel
    // one**. `docs/screens-county.md` §2.1 says "all of it in the 9-pixel
    // font"; the name is the exception, and `DAT_005AEA40` is set to 1 only
    // *after* it, so the name is embossed and the numbers below it are not.
    //
    // The unowned plate is 162 × 274 rather than 162 × 94 and puts the name
    // fifteen pixels lower, at `0xB4`; that is the original's own difference,
    // not a rounding of ours.
    //
    // **The emboss is the parchment pair on both plates, and over the cloudy
    // one that is a bug of the original's that we reproduce.** `Ui_DrawText`
    // picks its emboss from `g_screenId` and two globals, never from the
    // caller, and `CountyStrip_Draw` sets neither of them around this call —
    // so the name is drawn with `0x10`/`0x1F`, a dark olive over a pale
    // parchment yellow, whether it is standing on the parchment plate
    // (`Misc_cty` frame `0x37`) or on the cloudy one (frame `0x3A`). On the
    // cloudy plate the highlight is a colour that is not in the picture and
    // the name reads as though it were fading into paper that is not there.
    //
    // A player reported it and asked for it to be reproduced *and* switchable:
    // [`crate::game::Quirks::grey_county_name`] is the switch and it is off by
    // default. `docs/bugs.md` B64.
    let name = county_name(ctx, county);
    let name_y = if mine { 165 } else { 180 };
    let name_style = if ctx.assets.quirks.grey_county_name && !mine {
        crate::shell::font::Style {
            colour: strip_ink,
            shadow: Some(crate::shell::font::SHADOW_GREY),
            caps: None,
        }
    } else {
        crate::shell::font::Style::new(strip_ink)
    };
    body_centred_styled(ctx, canvas, 480, name_y, 160, &name, name_style);

    if !mine {
        // **`owner != 0` — and unclaimed land is a third case, not a second.**
        //
        // `CountyStrip_Draw`'s else-arm draws the cloudy plate and the name for
        // *any* county that is not yours, and then:
        //
        // ```c
        // if (g_counties[g_selectedCounty].owner != 0) {
        //   DAT_0058fe9c = 1;                                   /* the grey emboss */
        //   colour = g_realms[owner].field_0x8;
        //   Ui_DrawCentred(0xf, 0, 0x1e0, 0xf0,  0xa0, &g_fontBody, colour);
        //   Ui_DrawCentred(0xf, 1, 0x1e0, 0x104, 0xa0, &g_fontBody, colour);
        //   FUN_004025d7(&g_playerNames + owner * 0x2c, 0x1e0, 0x118, 0xa0, &g_fontBody, colour);
        //   DAT_0058fe9c = 0;
        // }
        // ```
        //
        // So a county nobody holds gets the plate and its name and **nothing
        // else** — no banner, no owner line. `L2.eng` group 15 is two strings,
        // `"Sovereign land"` and `"of"`, and the third line is a lord's name
        // out of `g_playerNames`; there is no wording in the file for an
        // unowned county because the original never needs one.
        //
        // We drew `SOVEREIGN LAND / OF / UNCLAIMED` here, which is a sentence
        // the original cannot produce. A player reported it in one line:
        // *"Unclaimed lands have no 'sovereign land of'."*
        if c.owner == 0 {
            return;
        }
        // The three lines carry the **grey** emboss — `DAT_0058FE9C = 1` —
        // and not the parchment one the name above them uses. Two emboss pairs
        // in one plate is the original's own arrangement, and ours had
        // collapsed them into one.
        //
        // The *colour* is still ours: `g_realms[owner].field_0x8` is a palette
        // byte out of the save and we have the realm number instead. Only the
        // emboss is read out of the binary here.
        //
        // **The third line is a name now.** It read `REALM 3` because nothing
        // in this workspace filled `g_playerNames`; the front end fills it at
        // *Start* — the local player's from what was typed on setup page 4, an
        // AI lord's from `L2.eng` group 7 — so the line says *SOVEREIGN LAND /
        // OF / THE BARON* the way the original's does.
        //
        // **The fallback is not decoration.** A world that did not come through
        // the front end — a `.sav` imported by `l2-scenario`, a kingdom a test
        // built — has no names in it, and an empty third line under two full
        // ones looks like a drawing fault rather than like missing data.
        let owner = match ctx.game.player_names[c.owner as usize].as_str() {
            n if n.is_empty() => format!("REALM {}", c.owner),
            n => n,
        };
        let colour = ink.realm.get(c.owner as usize).copied().unwrap_or(ink.text);
        let style = crate::shell::font::Style {
            colour,
            shadow: Some(crate::shell::font::SHADOW_GREY),
            caps: None,
        };
        let banner = eng(ctx, 15, 0, "SOVEREIGN LAND");
        body_centred_styled(ctx, canvas, 480, 240, 160, &banner, style);
        body_centred_styled(ctx, canvas, 480, 260, 160, &eng(ctx, 15, 1, "OF"), style);
        body_centred_styled(ctx, canvas, 480, 280, 160, &owner, style);
        return;
    }

    // `Ui_DrawNumber(pop, ' ', " ", 0x1FC, 0xBD, &g_fontSmall, 0x3F)` and the
    // identical call for happiness at `0x25A`.
    //
    // **Both are left origins.** `Ui_DrawNumber` takes no anchoring argument —
    // the two calls differ only in their value and their x — so the happiness
    // figure starts at 602 rather than ending there. We right-anchored it,
    // which put a two-digit number on top of the plate's heart and would have
    // put a three-digit one further left still. See `docs/decisions.md` C42.
    strip_text(ctx, canvas, 508, 189, &c.population.to_string(), strip_ink);
    strip_text(ctx, canvas, 602, 189, &c.happiness.to_string(), strip_ink);
    // Group 61, centred in 76 pixels at (0x1E0, 0xD5) and (0x234, 0xD5), and
    // **in colour 0x3F, the same as the numbers** — the captions are not dimmed
    // in the original and ours were unreadable against the plate.
    strip_centred(ctx, canvas, 480, 213, 76, &eng(ctx, 61, 0, g86::STRIP_TAX), strip_ink);
    strip_centred(ctx, canvas, 564, 213, 76, &eng(ctx, 61, 1, g86::STRIP_RATION), strip_ink);
    // (0x1FA, 0xE2), and the ration level centred in 76 at (0x234, 0xE2).
    strip_text(ctx, canvas, 506, 226, &format!("{}%", c.tax_rate), strip_ink);
    // "Red when it differs from rationWanted" is the original's own rule, and
    // the colour it picks is `0xF9` rather than `0x3F`.
    let colour = if c.ration_achieved == c.ration_wanted { strip_ink } else { strip_bad };
    strip_centred(ctx, canvas, 564, 226, 76, &ration_label(ctx, c.ration_achieved), colour);

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

    // `Pl8_DrawFrame(g_miscCtySheet, 0x3D, share / 2 + 0x214, 0x106)` — the
    // farm/industry split's thumb, on the 162 × 52 plate at (478, 250) that
    // `CountyStrip_Draw` paints as its last act. It is drawn *here* rather than
    // by the map screen because the county panels repaint the whole sidebar
    // over the map, and a thumb only the map drew would vanish whenever a panel
    // was open.
    //
    // # The blue outline, which is a **frame**
    //
    // `CountyStrip_Draw`'s last branch, verbatim:
    //
    // ```c
    // if (county.labour[8].workers == 0)
    //     Pl8_DrawFrame(g_miscCtySheet, 0x3D, share / 2 + 0x214, 0x106);
    // else
    //     Pl8_DrawFrame(g_miscCtySheet, 0x55, share / 2 + 0x212, 0x104);
    // ```
    //
    // Slot 8 is *Idle townsfolk*, so **the thumb changes the moment anybody in
    // the county has nothing to do** — which is exactly what the player
    // remembered: *"the peasant slider I think had a blue outline if there
    // were idle peasants as well."* This module used to guess the castle job;
    // it is the idle pool.
    //
    // And the outline is measurable rather than described. Frame `0x3D` is
    // 9 × 33 and frame `0x55` is 13 × 37 — four wider and four taller — drawn
    // two pixels left and two pixels up, so it is *the same thumb inside a
    // two-pixel ring*. Every one of the ring's 124 pixels is one of three
    // palette entries, and all three are blue: `95` = `rgb(0, 0, 121)`,
    // `65` = `rgb(157, 202, 234)` and `64` = `rgb(194, 230, 255)`.
    // `crates/l2-view/tests/install.rs` asserts that against the player's own
    // `Misc_cty.pl8`.
    let share = c.industry_share.clamp(0, 100);
    let idle = c.labour[JOB_IDLE_TOWNSFOLK] != 0;
    let (frame, tx, ty) = if idle {
        (misc_cty::SPLIT_THUMB_IDLE, share / 2 + 530, 260)
    } else {
        (misc_cty::SPLIT_THUMB, share / 2 + 532, 262)
    };
    let thumb = ctx.assets.chrome.as_ref().is_some_and(|ch| ch.draw_misc(canvas, frame, tx, ty));
    if !thumb {
        // OURS, for an install with no `Misc_cty.pl8`: the ring is drawn as a
        // ring, because that is what it is.
        canvas.fill_rect(share / 2 + 532, 262, 9, 33, ink.highlight);
        if idle {
            widget::frame(canvas, Rect::new(share / 2 + 530, 260, 13, 37), ink.realm[2]);
        }
    }

    draw_produce_rows(ctx, canvas, c, strip_ink);

    // OURS: the original's quadrants are invisible. A one-pixel outline is how
    // a keyboard player sees which of the four is open.
    if let Some(p) = focus {
        widget::frame(canvas, p.strip_hotspot(), ink.highlight);
    }
}

/// **The produce rows, and the blue outline that is the point of them.**
///
/// The plate below the slider — `Misc_cty` frame `0x38` at (478, 302) — carries
/// one row per thing the county makes. `FUN_0040FEC1` decides *which* rows,
/// into two lists, and `CountyStrip_Draw` then walks them; this is the **left**
/// list, the three farming rows, which is where the dairy is.
///
/// ```c
/// if (fieldsCattle || herd)     rows[n++] = 1;   // FUN_004100AF
/// if (fieldsGrain  || grain)    rows[n++] = 0;   // FUN_0041023A
/// if (fieldsReclaiming)         rows[n++] = 2;   // FUN_004103C5
/// DAT_0053E970 = n < 3 ? 0x3C : 0x2D;            // the row pitch
/// ```
///
/// Each drawer opens with the same three-way choice, and it is the one the
/// player asked about:
///
/// ```c
/// if (labour[slot].useful  < labour[slot].workers) ringed frame, two px up-left
/// else if (labour[slot].workers < labour[slot].wanted) shortfall frame
/// else                                                 plain frame
/// ```
///
/// So **the county's cow gets a blue ring around it the moment more people are
/// milking than the herd can use** — *"there's no blue outline for idle
/// peasants (eg too many on dairy)"*, exactly. The ceiling is
/// [`l2_kingdom::county::County::labour_useful`], the same word
/// `Village_RebuildIcons` uses to decide which peasants in the village are
/// drawn sitting down, and the floor is the one `Panel_JobDetail` already
/// colours the count red below.
///
/// # What is not here
///
/// * **The right-hand list** — wood, iron, stone, weapons and the castle. Three
///   of the five have no state at all (frames `0x2C`, `0x2D`, `0x2E`, drawn
///   flat), and the two that do — the blacksmith and the castle — pick their
///   frame from county `+0x290`, an unnamed byte, and from `+0x1B0`. Neither is
///   settled, so neither is drawn.
/// * **The seasonal deltas** — and a player found the hole before this comment
///   was rewritten: *"Sidebar doesn't show grain being planted as a negative
///   number."* He is right, and he is describing **Spring**.
///
///   Every drawer follows its icon with a `Ui_DrawDelta` (`0x00402E0C`), which
///   is a *signed* number: `value < 0` draws `Ui_DrawNumber(-value, '-', …)` in
///   `colourNeg` (`0xF9`), `value > 0` gets a `'+'` lead in `colourPos`
///   (`0xFA`), and zero gets `'@'`, the blank glyph that keeps a zero
///   column-aligned. The minus is **not a separate mark** — it overwrites
///   `g_numberBuffer[0]`, the slot `Ui_NumberToBuffer(value, 1, 0)` leaves free
///   for a sign, and the whole string goes out in one `Ui_DrawText`. With
///   `mode == 0`, which is what all eight rows pass, a value of zero draws
///   **nothing at all**.
///
///   **It is not "the change since last season".** The tooltip layer says so in
///   the game's own words — `L2.eng` group 220 index 15 is *"Cattle, and change
///   next season"* and 16 is *"Wheat, and change next season"* — and the code
///   agrees: `County_RefreshEstimates(county, g_seasonNext)`.
///
///   **The grain row's value is county `+0x22C`, and this workspace never
///   computes it.** `Grain_LabourEstimate` (`0x0044D374`) writes it in a tail
///   *after* the search loop [`l2_kingdom::land::grain_labour_estimate`]
///   reproduces:
///
///   ```c
///   staff = county.labour[0].workers;                    /* the real staffing */
///   county.field_0x230 = Grain_Sow(county, staff, county.grain);
///   if (season == 4) county.crop[2]      = Grain_Harvest(county, staff, county.crop[1]);
///   if (season == 2 || season == 3) county.field_0x2FC = Grain_Grow(county, staff, county.crop[1]);
///
///   if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
///   else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
///   else                  county.field_0x22C = -county.grainEaten;
///   ```
///
///   So in **Spring** the row is `−(sown) − eaten`, which cannot be anything but
///   negative — the player's sentence, exactly. Our port returns
///   `GrainEstimate { wanted, useful }` and stops at the loop, so all four of
///   those writes are missing, and **the sign question never arises because the
///   number never arrives.** It cannot be recovered from the estimate either:
///   the loop calls `Grain_Sow(county, workers, grain − grainEaten)` and the
///   tail calls `Grain_Sow(county, staff, grain)` — a different third argument.
///   `crate::field`'s module docs already say the estimate round runs twice
///   "for … the panel forecasts, which the estimates fill from whatever the
///   allocator last decided"; these are those forecasts.
///
///   The four industry rows read a different quantity again — commodity `c`'s
///   i32 at county `0x2A8 + c * 0x18`, which `docs/records.json` gives to
///   `Industry[c + 1]`'s unnamed head word, and the stone row reads `0x2F0`,
///   one whole record past the end of a four-record array. Reported, not
///   guessed at. `docs/draws-map.md` §5.10.
fn draw_produce_rows(
    ctx: &Ctx,
    canvas: &mut Canvas,
    c: &l2_kingdom::county::County,
    strip_ink: u8,
) {
    // `FUN_0040FEC1`, in its own order: cattle, then grain, then reclamation —
    // and the same list `job_row_at` hit-tests, so the picture and the target
    // cannot drift apart.
    let rows = farm_rows(c);
    // `DAT_0053E970`: three rows or more and they close up.
    let pitch = farm_pitch(rows.len());

    for (n, &slot) in rows.iter().enumerate() {
        let y = pitch * n as i32;
        let (plain, short, ringed) = PRODUCE_ICONS[slot];
        let state = if c.labour_useful[slot] < c.labour[slot] {
            // The ringed frame, and its own position — two pixels up and left
            // of the plain one, because it is the plain one plus a ring.
            Some(ringed)
        } else if c.labour[slot] < c.labour_wanted[slot] {
            short
        } else {
            None
        };
        let (frame, x, dy) = match (slot, state) {
            (1, Some(f)) => (f, 482, 0x131),
            (1, None) => (plain, 484, 0x133),
            (0, Some(f)) if f == ringed => (f, 484, 0x12D),
            (0, Some(f)) => (f, 485, 0x12E),
            (0, None) => (plain, 486, 0x12F),
            (_, Some(f)) => (f, 491, 0x130),
            (_, None) => (plain, 493, 0x132),
        };
        let drawn =
            ctx.assets.chrome.as_ref().is_some_and(|ch| ch.draw_misc(canvas, frame, x, y + dy));
        if !drawn {
            // OURS, with no `Misc_cty.pl8`: a label and, when it applies, the
            // ring — because the ring is the thing being said.
            let ink = &ctx.assets.ink;
            let name = ["GRAIN", "DAIRY", "RECLAIM"][slot];
            text::draw(canvas, x, y + dy + 8, name, ink.dim);
            if state == Some(ringed) {
                widget::frame(canvas, Rect::new(x - 2, y + dy - 2, 44, 32), ink.realm[2]);
            }
        }
        // **The row's forecast for next season.** `Ui_DrawDelta(value, 0, " ",
        // " ", 0x204, pitch*row + dy, &g_font10, 0xFA, 0xF9)` — the same `x` at
        // all three farm rows, and `dy` `0x139` for the two that have a stock
        // and `0x133` for reclamation, which has a countdown instead.
        //
        // **Only the cattle row draws one.** Its value is
        // `Herd_LabourEstimate`'s tail — `(births − deaths) − herdEaten` — and
        // [`l2_kingdom::land::herd_preview`] is that tail, ported, written on
        // every season tick and carried in the save. Grain's is county `+0x22C`
        // and reclamation's `+0x20C`; neither is computed anywhere in this
        // workspace, so neither row can draw one yet, and the doc comment above
        // says what it would take. **CNEW-grain-forecast.**
        let (delta, delta_dy) = match slot {
            1 => (Some(c.herd_change_expected), 0x139),
            0 => (None, 0x139),
            _ => (None, 0x133),
        };
        if let Some(v) = delta {
            strip_delta(ctx, canvas, v, 0x204, y + delta_dy);
        }
        // `Ui_DrawNumberRight(store, ' ', …, 0x1E0, y + 0x14D, 0x3C,
        // &g_fontBody, 0x3F)` — the store itself.
        //
        // **`Ui_DrawNumberRight` centres.** Its tail is `FUN_004025D7`, which
        // computes `x + (width − textWidth) / 2`; the name and the
        // `docs/symbols.json` comment both said right-aligned and both were
        // wrong, found independently by two draw audits. So this is centred in
        // sixty pixels from x = 480, not anchored at 540.
        // Reclamation's number is a field this project has not named, so that
        // row carries none.
        let store = match slot {
            0 => Some(c.grain),
            1 => Some(c.herd),
            _ => None,
        };
        if let Some(v) = store {
            body_centred_in(ctx, canvas, 480, y + 0x14D, 0x3C, &v.to_string(), strip_ink);
        }
    }
}

impl CountyScreen {
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
            widget::button(canvas, ink, ok, "CLOSE", false);
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

/// Geometry tests. The canvas tests that read numbers back off the pixels live
/// in `tests/screens.rs`, because they need the shipped install.
///
/// **Six mutations were checked against these and those**, each turning exactly
/// one test red and no others:
///
/// | mutation | test that went red |
/// |---|---|
/// | `MAX_TAX_RATE` 50 → 51 | `the_tax_rate_stops_at_the_originals_own_ceiling_of_fifty` |
/// | `delta_row`'s zero guard removed | `a_zero_delta_row_draws_its_label_and_no_number` |
/// | `HOT_X_LEFT_END` 548 → 560 | `the_strip_quadrants_fit_the_plate_and_leave_the_thermometer_unclickable` |
/// | `Chrome::load` preferring `System2.pl8` | `the_ration_split_slider_sets_the_field_the_original_sets` |
/// | the population panel's first row y 266 → 267 | `the_population_panel_opens_from_its_own_quadrant_and_lays_out_where_it_should` |
/// | the strip's population x 508 → 509 | `the_county_strip_shows_the_saves_numbers_where_the_original_puts_them` |
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
    /// corner picture. A window that ran off the screen, or a corner outside
    /// it, would mean a cell count or a button coordinate was misread.
    #[test]
    fn every_panel_window_is_on_screen_and_holds_its_own_ok_button() {
        for p in PANELS {
            let w = p.window();
            assert!(w.x >= 0 && w.y >= 0, "{p:?} starts on screen");
            assert!(w.x + w.w <= 640 && w.y + w.h <= 480, "{p:?} ends on screen: {w:?}");
            assert!(inside(w, p.ok_button()), "{p:?}: the corner is outside its window {w:?}");
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
