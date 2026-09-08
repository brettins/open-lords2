//! Screen `0x1C` — the campaign interstitial between two maps.
//!
//! # `docs/screens-county.md` §1 called this "the front end". It is not.
//!
//! That row was written from the two files the painter loads — `gateway.pl8`
//! and `panels2.pl8` — which are indeed the front end's artwork. The strings
//! settle it the other way. `FUN_0041E1DD` draws **`L2.eng` group 36**, and
//! group 36 is:
//!
//! ```text
//!  0  "Congratulations!!"          4  "You have lost."
//!  1  "You have conquered"         5  "You have failed to conquer"
//!  2  "Events move on apace …"     6  "Until this country falls under your"
//!  3  "…now awaits you in"         7  "rule there can be no thought"
//!                                  8  "of further conquests, my lord."
//! ```
//!
//! with two map names out of group 101 between them: the one just fought over
//! (`DAT_00553E78`) and, on a win, the one coming next (`g_scenarioIndex`).
//! **[V]** — three branches, each of which reads as one whole sentence, and the
//! `g_scenarioIndex` lookup only happens in the branch that mentions a *next*
//! country.
//!
//! The third branch is the end of the campaign: when the progress counter
//! `DAT_0053F258` reaches 8 the painter drops indices 2 and 3 and draws 9
//! through 15 instead — *"The whole of Christendom … now lies firmly within
//! your iron fist. You are just too good at this! So may we suggest the custom
//! game option…"* Eight maps, and the counter is compared against 8.
//!
//! So `0x1C` is the screen between two campaign maps, and the front end proper
//! is [`super::setup`] page 1. `docs/screens-county.md` is corrected.
//!
//! # What the shell shows
//!
//! All three outcomes, because a shell with no campaign progress behind it has
//! no reason to prefer one, and because the point of walking the interface is
//! to see what the interface can say. The keyboard and a click cycle them.

use l2_view::Canvas;

use crate::input::{Event, Key};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `L2.eng` group 36.
pub const GROUP: usize = 36;
/// Group 101, the sixty map names.
pub const GROUP_MAPS: usize = 101;

/// The window: `FUN_00409346(panels2, 0x70, 8, 0x1A, h)`, 26 cells wide from
/// x = 112 — 416 pixels, centred on 640 with 112 either side — and 14 cells
/// tall for the two short outcomes, 18 for the long one.
const BOX_X: i32 = 0x70;
const BOX_Y: i32 = 8;
const BOX_COLS: i32 = 0x1A;
const BOX_ROWS_SHORT: i32 = 0x0E;
const BOX_ROWS_LONG: i32 = 0x12;
/// Every line is centred in 416 pixels from the same x.
const TEXT_W: i32 = 0x1A0;

/// Which of the three things this screen can say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The map was taken and another follows. Group 36 lines 0–3.
    Won,
    /// The map was lost. Lines 4–8.
    Lost,
    /// The eighth map was taken; the campaign is over. Lines 0, 1, 9–15.
    Finished,
}

impl Outcome {
    pub const ALL: [Outcome; 3] = [Outcome::Won, Outcome::Lost, Outcome::Finished];

    fn next(self) -> Outcome {
        match self {
            Outcome::Won => Outcome::Lost,
            Outcome::Lost => Outcome::Finished,
            Outcome::Finished => Outcome::Won,
        }
    }
}

pub struct ConquestScreen {
    outcome: Outcome,
}

impl ConquestScreen {
    pub fn new() -> ConquestScreen {
        ConquestScreen { outcome: Outcome::Won }
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }
}

impl Default for ConquestScreen {
    fn default() -> Self {
        ConquestScreen::new()
    }
}

impl Screen for ConquestScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Conquest
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Lords of the Realm II — conquest".into()
    }

    fn palette(&self) -> Option<&'static str> {
        Some("Gateway.256")
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) => Transition::Pop,
            Event::KeyDown(Key::Left) | Event::KeyDown(Key::Up) => {
                self.outcome = self.outcome.next().next();
                Transition::Stay
            }
            Event::KeyDown(_) | Event::Click { .. } => {
                self.outcome = self.outcome.next();
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        // `FUN_0041E1DD` sets `DAT_0058FE2C = 1` around *every* line it draws
        // and never touches `DAT_005AEA40`, so unlike the setup pages this
        // screen is embossed throughout and every capital is in colour 1.
        let pen = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW_GATEWAY),
            caps: Some(1),
        };
        if !shell::background(canvas, a, "Gateway.pl8") {
            canvas.clear(ctx.assets.ink.background);
        }
        let rows = if self.outcome == Outcome::Finished { BOX_ROWS_LONG } else { BOX_ROWS_SHORT };
        pen.window_from(canvas, "Panels2.pl8", BOX_X, BOX_Y, BOX_COLS, rows);

        // The map just fought over. `DAT_00553E78` is the campaign's own
        // record of it, which this workspace does not keep; the map the
        // scenario loaded is the closest true thing to show, and it is the
        // same lookup — group 101 indexed by a map slot.
        let map = ctx.game.map_slot.min(59);
        let name = a.text(GROUP_MAPS, map).to_string();

        let head = |canvas: &mut Canvas, i: usize, y: i32| {
            pen.eng_heading_centred(canvas, GROUP, i, BOX_X, y, TEXT_W, font::TEXT)
        };
        let body = |canvas: &mut Canvas, i: usize, y: i32| {
            pen.eng_centred(canvas, GROUP, i, BOX_X, y, TEXT_W, font::TEXT)
        };

        match self.outcome {
            Outcome::Won => {
                head(canvas, 0, 0x20);
                body(canvas, 1, 0x40);
                pen.heading_centred(canvas, BOX_X, 0x58, TEXT_W, &name, font::TEXT);
                body(canvas, 2, 0x80);
                body(canvas, 3, 0x98);
                // The next map: `g_scenarioIndex`, which in the original has
                // already been advanced by the time this is drawn.
                let next = a.text(GROUP_MAPS, (map + 1).min(59)).to_string();
                pen.heading_centred(canvas, BOX_X, 0xB0, TEXT_W, &next, font::TEXT);
            }
            Outcome::Lost => {
                head(canvas, 4, 0x20);
                body(canvas, 5, 0x40);
                pen.heading_centred(canvas, BOX_X, 0x58, TEXT_W, &name, font::TEXT);
                body(canvas, 6, 0x80);
                body(canvas, 7, 0x98);
                body(canvas, 8, 0xB0);
            }
            Outcome::Finished => {
                head(canvas, 0, 0x20);
                body(canvas, 1, 0x40);
                pen.heading_centred(canvas, BOX_X, 0x58, TEXT_W, &name, font::TEXT);
                // Seven lines, twenty apart, exactly as the painter lists them.
                for (n, i) in (9..=15usize).enumerate() {
                    body(canvas, i, 0x80 + n as i32 * 0x14);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_centred_on_the_screen() {
        assert_eq!(BOX_X + BOX_COLS * 16 + BOX_X, 640, "112 + 416 + 112");
        assert_eq!(BOX_COLS * 16, TEXT_W, "the text is centred in the box's own width");
    }

    #[test]
    fn the_long_outcome_needs_the_taller_box() {
        // Seven lines from y = 0x80, twenty apart, end at 0x80 + 6*20 = 0xF8.
        let last = 0x80 + 6 * 0x14;
        assert!(last < BOX_Y + BOX_ROWS_LONG * 16, "the last line must be inside the box");
        assert!(last > BOX_Y + BOX_ROWS_SHORT * 16, "and outside the short one");
    }

    #[test]
    fn the_three_outcomes_cycle() {
        assert_eq!(Outcome::Won.next(), Outcome::Lost);
        assert_eq!(Outcome::Lost.next(), Outcome::Finished);
        assert_eq!(Outcome::Finished.next(), Outcome::Won);
        assert_eq!(Outcome::ALL.len(), 3);
    }
}
