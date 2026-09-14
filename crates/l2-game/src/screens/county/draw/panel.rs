#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::helpers::*;
use super::*;
use super::layout::*;
use super::strip::*;
use l2_kingdom::tables::{
    HEALTH_BAND_NAMES, JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT, RATION_NAMES,
};
use l2_view::chrome::{self, misc_cty, system};
use l2_view::{text, Canvas};
use crate::game::{MAX_RATION_SPLIT, MAX_TAX_RATE};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen, TRAILING};
use crate::widget;

pub(crate) const RATION_SUFFIX: &str = "";

impl Panel {
    /// The panel's window in pixels — the rectangle `Ui_DrawBox` covers.
    pub fn window(self) -> Rect {
        let (x, y, cols, rows) = self.box_cells();
        Rect::new(x, y, cols * 16, rows * 16)
    }

    fn box_cells(self) -> (i32, i32, i32, i32) {
        self.box_cells_for(false)
    }

    /// The same with *Armies Eat* known, which is the one option that changes a
    /// panel's shape: `Panel_Ration` opens
    /// `rows = g_optArmiesEat == 1 ? 2 : 0; Ui_DrawBox(0x80, 0x60, 0x12, rows + 0xF)`,
    /// making room for the foraging line at y = 336. Everything else ignores it.
    pub(crate) fn box_cells_for(self, armies_eat: bool) -> (i32, i32, i32, i32) {
        match self {
            Panel::Population => POPULATION_BOX,
            Panel::Happiness => HAPPINESS_BOX,
            Panel::Tax => TAX_BOX,
            Panel::Ration => {
                let (x, y, cols, rows) = RATION_BOX;
                (x, y, cols, if armies_eat { rows + 2 } else { rows })
            }
        }
    }

    /// The graph area, for the two panels that have one.
    pub fn graph_rect(self) -> Option<Rect> {
        matches!(self, Panel::Population | Panel::Happiness).then_some(GRAPH)
    }

    /// `Ui_OkButton(x, y, 0)` — the 24 × 24 picture in the panel's own corner,
    /// hit-tests on a left
    /// release.
    ///
/// `System.pl8` frame `0x33` decodes to a cursor
    /// arrow pointing into a small black hole: a close button whose artwork is
    /// the instruction. It is a real target *and* the right button closes the
    /// panel from anywhere (`docs/screens-county.md` §2.6), so the game offers
    /// two ways out and so do we. §2.7; the name `OK` is ours and is kept.
    pub fn ok_button(self) -> Rect {
        self.ok_button_for(false)
    }

    /// The same, with the ration panel's *Armies Eat* height applied — its
    /// corner is `Ui_OkButton(0x184, rows * 0x10 + 0x44, 0)` and `rows` is the
    /// box's own, so turning the option on moves the button 32 pixels down.
    pub fn ok_button_for(self, armies_eat: bool) -> Rect {
        let (x, y) = match self {
            // Ui_OkButton(0x1B4, 0x184, 0)
            Panel::Population => (436, 388),
            // Ui_OkButton(0x1B4, 0x194, 0)
            Panel::Happiness => (436, 404),
            // Ui_OkButton(0x174, 0x104, 0)
            Panel::Tax => (372, 260),
            // Ui_OkButton(0x184, rows * 0x10 + 0x44, 0)
            Panel::Ration => {
                let (_, _, _, rows) = self.box_cells_for(armies_eat);
                (388, rows * 0x10 + 0x44)
            }
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

// ------------------------------------------- the sheet frames the panels draw
//
// `Misc_cty.pl8` in campaign mode — `Pen::misc_frame`, which is
// `Pl8_DrawFrame(g_miscCtySheet, …)`.

