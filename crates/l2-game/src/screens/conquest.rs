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
//! # The painter, address by address
//!
//! ```text
//! Screen_DrawConquest():                                        0x0041E1DD
//!   if (DAT_0055302C == 2) { FUN_004B11CE(); return; }        nothing at all
//!   Music_Play(campaignMap < 8 ? "setup.wav" : "setup2.wav")
//!   File_ReadChunk("gateway.256", 0x004EA8A0, 0x300, 0)          the palette
//!   FUN_00408FCB("gateway.pl8", 0x1E0)             the whole 640 x 480 ground
//!   File_ReadChunk("panels2.pl8", scratch, 160000, 0)
//!   FUN_00409346(panels2, 0x70, 8, 0x1A, campaignMap < 8 ? 0x0E : 0x12)
//!                                       (112, 8), 416 x 224 or 416 x 288
//!   DAT_0058FE2C = 1                        drop capitals on for every line
//!   won:                                              lost:
//!     Ui_DrawCentred(36,  0, 0x70, 0x20, 0x1A0, hd)     (36,  4, … 0x20, hd)
//!     Ui_DrawCentred(36,  1, 0x70, 0x40, 0x1A0, bd)     (36,  5, … 0x40, bd)
//!     Ui_DrawCentred(101, g_campaignLastMap, … 0x58, hd)      the same line
//!     Ui_DrawCentred(36,  2, 0x70, 0x80, 0x1A0, bd)     (36,  6, … 0x80, bd)
//!     Ui_DrawCentred(36,  3, 0x70, 0x98, 0x1A0, bd)     (36,  7, … 0x98, bd)
//!     Ui_DrawCentred(101, g_scenarioIndex, … 0xB0, hd)  (36,  8, … 0xB0, bd)
//!   finished (g_campaignMap >= 8):
//!     Ui_DrawCentred(36,  0, 0x70, 0x20, 0x1A0, hd)
//!     Ui_DrawCentred(36,  1, 0x70, 0x40, 0x1A0, bd)
//!     Ui_DrawCentred(101, g_campaignLastMap, 0x70, 0x58, 0x1A0, hd)
//!     Ui_DrawCentred(36, 9..15, 0x70, 0x80 + 0x14n, 0x1A0, bd)   seven lines
//!   Palette_Set(0x4EA8A0)
//! ```
//!
//! **Twenty-two `Ui_DrawCentred` call sites and no other primitive**, split
//! 6 / 6 / 10 across three branches that cannot both run, so the most this
//! screen ever puts on the glass in one frame is ten lines and a window. All
//! twenty-two are reproduced below, branch for branch.
//!
//! # Group 36 has nineteen strings and the painter draws sixteen
//!
//! Indices **16, 17 and 18** — *"You have mastered the first challenge."*,
//! *"To continue your campaign however"*, *"you must buy Lords2 !!"* — are the
//! **demo's** nag, and `grep` over the whole decompilation finds no consumer of
//! group 36 outside this painter and no reference to those three indices at
//! all. They are dead text in the retail build, the same shape as `L2.eng`
//! 31/21 *"Morale"*. `[V]`
//!
//! # The background is one `read()`, not a draw call
//!
//! `FUN_00408FCB(name, lines)` (`0x00408FCB`) is
//! `File_ReadChunk(name, g_backBufferBits, g_screenStride * lines, 0x18)` —
//! it seeks 24 bytes into the `.pl8` and reads 640 × 480 bytes **straight into
//! the back buffer**. There is no sprite decode, no blit and no clip: a
//! full-screen `.pl8` is a raw image with a 24-byte header, and the eleven
//! screens that call it (this one, the armoury, the two Battle Master pages,
//! the castle chooser, the standings, the merchant, the trade goods, the Lords
//! of Magic advertisement) each paint their whole ground with one `read`. `[V]`
//!
//! That is why `Screen_LordsOfMagicAd` (`0x0041E5D0`) makes **zero** draw
//! calls and is still a full screen of artwork, and why a draw-call census over
//! the twenty-six pixel primitives has to count backgrounds separately rather
//! than treat a zero as a missing screen.
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
use crate::victory::ConquestBranch;

/// `L2.eng` group 36 — **nineteen strings, of which the painter draws sixteen**
/// (0…15). Verified against the words: index 0 is *"Congratulations!!"*, 4
/// *"You have lost."*, 9 *"The whole of Christendom (along with a"*.
pub const GROUP: usize = 36;
/// Group 101, the sixty map names. Verified against the words: index 0 is
/// *"England"*, 6 *"Europe"*, and 24 onward are the placeholders *"map no
/// 25"* … *"map no 60"*.
pub const GROUP_MAPS: usize = 101;
/// The highest index group 101 has, and the clamp every lookup here uses.
pub const MAX_MAP: usize = 59;
/// `File_ReadChunk("gateway.256", …)` then `FUN_00408FCB("gateway.pl8", 0x1E0)`
/// — the palette and the whole 640 × 480 ground.
pub const BACKGROUND: &str = "Gateway.pl8";
/// The palette that goes with it.
pub const PALETTE: &str = "Gateway.256";
/// `File_ReadChunk("panels2.pl8", …)`, the sheet `FUN_00409346` takes the
/// window's frame and interior out of.
pub const WINDOW_SHEET: &str = "Panels2.pl8";
/// The seven-line run of the campaign-end branch, and its 20-pixel pitch —
/// `Ui_DrawCentred(36, 9 + n, 0x70, 0x80 + 0x14 * n, …)`.
pub const FINISHED_FIRST: usize = 9;
pub const FINISHED_LAST: usize = 15;
pub const FINISHED_PITCH: i32 = 0x14;

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
    /// Whether the game's own ending has been read yet. A screen is built with
    /// no [`Ctx`], so the adoption happens on the first tick instead — and it
    /// happens **once**, or the arrow keys would be overwritten every frame and
    /// the shell would stop being walkable.
    adopted: bool,
}

impl ConquestScreen {
    pub fn new() -> ConquestScreen {
        ConquestScreen { outcome: Outcome::Won, adopted: false }
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// Take the branch from the game if the game has one.
    ///
    /// A game that is still in play has nothing to say here — that is the shell
    /// walking the screen — so the cycling default stands. A game that has ended
    /// decides, and `l2_game::victory::Campaign::branch` is the decision:
    /// the outcome plus whether the campaign counter has reached eight.
    fn adopt(&mut self, ctx: &Ctx) {
        if self.adopted {
            return;
        }
        self.adopted = true;
        if !ctx.game.outcome().is_over() {
            return;
        }
        self.outcome = match ctx.game.campaign.branch() {
            ConquestBranch::Won => Outcome::Won,
            ConquestBranch::Lost => Outcome::Lost,
            ConquestBranch::Finished => Outcome::Finished,
        };
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
        Some(PALETTE)
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

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        let ctx: &Ctx = ctx;
        self.adopt(ctx);
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        self.adopt(ctx);
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
        // `FUN_00408FCB("gateway.pl8", 0x1E0)` — a 640 x 480 raw image read
        // straight into the back buffer, not a sprite draw. See the header.
        if !shell::background(canvas, a, BACKGROUND) {
            canvas.clear(ctx.assets.ink.background);
        }
        let rows = if self.outcome == Outcome::Finished { BOX_ROWS_LONG } else { BOX_ROWS_SHORT };
        pen.window_from(canvas, WINDOW_SHEET, BOX_X, BOX_Y, BOX_COLS, rows);

        // The map just fought over. `DAT_00553E78` is the campaign's own
        // record of it, which this workspace does not keep; the map the
        // scenario loaded is the closest true thing to show, and it is the
        // same lookup — group 101 indexed by a map slot.
        let map = ctx.game.map_slot.min(MAX_MAP);
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
                let next = a.text(GROUP_MAPS, (map + 1).min(MAX_MAP)).to_string();
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
                for (n, i) in (FINISHED_FIRST..=FINISHED_LAST).enumerate() {
                    body(canvas, i, 0x80 + n as i32 * FINISHED_PITCH);
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
