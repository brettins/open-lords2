//! The job popup — one of nine jobs, and the workers on it.
//!
//! `Panel_JobDetail` (`0x00412B33`), screen `0x0F`.
//!
//! # This game has two overlay mechanisms, and this is the one with a frame
//!
//! Worth stating outright, because the absence of the other one reads as
//! evidence and is not. A screen here can float over what is beneath it in two
//! quite different ways:
//!
//! * **A framed window** — `Ui_DrawBox` (`0x00409397`) with `Ui_DrawBoxBorder`
//!   and `Ui_DrawBoxInterior`, a kit of 16-pixel cells out of `Panels.pl8`.
//!   The four county panels and this one are drawn that way.
//! * **A raw blit** — a single sprite straight into the framebuffer at a fixed
//!   origin, with no frame and no clear. **The village is that**, at (64, 64).
//!
//! So *"`Village_Draw` contains no `Ui_DrawBox` call"* says nothing about
//! whether the village is a page: it says only that the village is the other
//! kind. Reading it as evidence for a full screen is exactly the mistake
//! `docs/decisions.md` C22 records.
//!
//! This one is a floating `Ui_DrawBox` over whatever opened it, and it can be
//! opened from two places:
//! a click on a village cluster (`FUN_0043A123`) or a click on a job row in the
//! campaign sidebar (`0x00438E3B`). It returns to whichever it was —
//! `DAT_005533F4` remembers — which is why this is a screen the machine pushes
//! rather than something the village owns.
//!
//! # What the original draws, and what is here
//!
//! The panel has a common head and nine different bodies. The head is
//!
//! * `Ui_DrawBox(0x30, 0x60, 0x19, h)` — 400 pixels wide at (48, 96), with the
//!   height read from a nine-entry table at `0x004D29A0`: **13 cells for grain
//!   and cattle, 11 for castle building, 9 for the rest**;
//! * a 50 x 50 recess at (64, 104) with the job's picture from `iconvill.pl8`
//!   inside it;
//! * the job's name at (128, 106) in the heading font — `L2.eng` group 74,
//!   index 1 … 9, which is what fixes the nine slots' order;
//! * and the worker count at (128, 136) through `Ui_DrawCount(n, job*2 + 30)`,
//!   which picks a singular or plural noun out of group 8: *Farmer*, *Dairy
//!   maid*, *Serf*, *Builder*, *Miner*, *Quarrier*, *Forester*, *Blacksmith*,
//!   *Peasant*.
//!
//! **That count is drawn in one of three colours**, and it is the reason this
//! screen exists here at all:
//!
//! ```c
//! colour = 0x3F;
//! if (labour[job]   < wanted[job]) colour = 0xF9;   // short
//! else if (useful[job] < labour[job]) colour = 0xFC; // wasted
//! ```
//!
//! Those are the second and third words of the labour record, which this
//! project imported as nothing until now. The village draws the same two facts
//! as ghost and surplus icons; this draws them as a colour and a number.
//!
//! **The nine bodies are not here.** `Panel_JobGrain`, `Panel_JobCattle`,
//! `Panel_JobReclamation`, `Panel_JobBlacksmith` and `Panel_JobIndustry` are
//! five more painters over about 4,000 bytes of code, and most of what they
//! report — sacks to be sown, seasons to harvest, efficiency ramps — reads
//! county fields `docs/kingdom.md` §1.3 does not have. The window, the head and
//! the colour rule are the part that is read; the body says so.

use l2_kingdom::county::County;
use l2_kingdom::tables::{JOB_COUNT, JOB_NAMES};
use l2_view::chrome::system;
use l2_view::{text, Canvas, Ink};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

/// `Ui_DrawBox(0x30, 0x60, …)` — the window's origin and its width in cells.
const BOX_X: i32 = 48;
const BOX_Y: i32 = 96;
const BOX_COLS: i32 = 25;

/// `DAT_004D29A0`, the per-job height in 16-pixel cells, indexed by the
/// original's 1-based job number. Slot 0 here is job 1.
const BOX_ROWS: [i32; JOB_COUNT] = [13, 13, 9, 11, 9, 9, 9, 9, 9];

/// `FUN_00403CF4(0x40, 0x68, 0x32, 0x32, 0x3F)` — the recess the job's picture
/// sits in, and `FUN_0040A682(frame, 0x41, 0x69)` the picture itself.
const ICON_RECESS: Rect = Rect::new(64, 104, 50, 50);

/// `Eng_DrawString(0x4A, job, 0x80, 0x6A, …)` and
/// `Ui_DrawCount(n, job*2+30, 0x80, 0x88, …)`.
const NAME_X: i32 = 128;
const NAME_Y: i32 = 106;
const COUNT_Y: i32 = 136;

/// `L2.eng` group 8, at `job*2 + 32` for a zero-based job — the singular of
/// each job's worker. Ours, transcribed; the workspace reads no `L2.eng`.
const WORKER_NOUN: [(&str, &str); JOB_COUNT] = [
    ("FARMER", "FARMERS"),
    ("DAIRY MAID", "DAIRY MAIDS"),
    ("SERF", "SERFS"),
    ("BUILDER", "BUILDERS"),
    ("MINER", "MINERS"),
    ("QUARRIER", "QUARRIERS"),
    ("FORESTER", "FORESTERS"),
    ("BLACKSMITH", "BLACKSMITHS"),
    ("PEASANT", "PEASANTS"),
];

/// What the count's colour says about the job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staffing {
    /// `labour < wanted` — colour `0xF9`, the same red a missed ration uses.
    Short,
    /// `useful < labour` — colour `0xFC`, a second warning state.
    Wasted,
    /// Colour `0x3F`, which is every other line on every other panel.
    Right,
}

/// The colour rule out of `Panel_JobDetail`, in the original's own order: short
/// is tested first, so a job that is somehow both reads as short.
pub fn staffing(c: &County, job: usize) -> Staffing {
    let (have, wanted, useful) = (c.labour[job], c.labour_wanted[job], c.labour_useful[job]);
    if have < wanted {
        Staffing::Short
    } else if useful < have {
        Staffing::Wasted
    } else {
        Staffing::Right
    }
}

pub struct JobScreen {
    county: u8,
    job: usize,
}

impl JobScreen {
    pub fn new(county: u8, job: usize) -> JobScreen {
        JobScreen { county, job: job.min(JOB_COUNT - 1) }
    }

    pub fn job(&self) -> usize {
        self.job
    }

    pub fn window(job: usize) -> Rect {
        Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS[job.min(JOB_COUNT - 1)] * 16)
    }

    /// `Ui_OkButton(0x1A4, rows * 0x10 + 0x44, 0)`. The 0x44 is not the box's
    /// own origin — the tick hangs four pixels below the bottom row, which is
    /// the original's arrangement and worth not tidying.
    pub fn ok_button(job: usize) -> Rect {
        let rows = BOX_ROWS[job.min(JOB_COUNT - 1)];
        Rect::new(0x1A4, rows * 0x10 + 0x44, system::OK_DIM, system::OK_DIM)
    }
}

impl Screen for JobScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Job(self.county, self.job)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("{} - county {}", JOB_NAMES[self.job], self.county)
    }

    /// **A window over whatever opened it.** This is the game's *other* overlay
    /// mechanism — a `Ui_DrawBox` frame kit — where the village is a raw blit;
    /// see the module docs. Either way nothing clears the screen.
    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            // `FUN_0042FF10`'s `0x0F` arm: the tick **or** a right release
            // closes the popup, and it returns to whichever screen opened it —
            // the village when `DAT_005533F4` is zero, the campaign map's
            // sidebar otherwise. `Transition::Pop` is both, because the stack
            // remembers what our `g_screenId` cannot.
            Event::KeyDown(Key::Escape)
            | Event::KeyDown(Key::Enter)
            | Event::RightClick { .. } => Transition::Pop,
            Event::Click { x, y } if JobScreen::ok_button(self.job).contains(x, y) => {
                Transition::Pop
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let w = JobScreen::window(self.job);
        let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| {
            ch.draw_box(canvas, w.x, w.y, BOX_COLS, BOX_ROWS[self.job], 0);
            ch.panels().frame_count() >= 196
        });
        if !drawn {
            widget::panel(canvas, ink, w);
        }

        // The recess. `iconvill.pl8` is loaded only by this panel and we do not
        // load it, so the recess stays empty and looks it.
        canvas.fill_rect(ICON_RECESS.x, ICON_RECESS.y, ICON_RECESS.w, ICON_RECESS.h, ink.background);
        widget::frame(canvas, ICON_RECESS, ink.border);

        text::draw(canvas, NAME_X, NAME_Y, &JOB_NAMES[self.job].to_uppercase(), ink.highlight);

        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        let n = c.labour[self.job];
        let (one, many) = WORKER_NOUN[self.job];
        let noun = if n.abs() == 1 { one } else { many };
        let colour = match staffing(c, self.job) {
            Staffing::Short => ink.bad,
            Staffing::Wasted => ink.highlight,
            Staffing::Right => ink.text,
        };
        text::draw(canvas, NAME_X, COUNT_Y, &format!("{n} {noun}"), colour);

        // OURS: the original says the same thing with a colour a modern player
        // has no legend for. The two numbers behind it are not on its panel.
        let note = match staffing(c, self.job) {
            Staffing::Short => format!("SHORT - WANTS {}", c.labour_wanted[self.job]),
            Staffing::Wasted => format!("WASTED - USES {}", c.labour_useful[self.job]),
            Staffing::Right => String::new(),
        };
        if !note.is_empty() {
            text::draw(canvas, NAME_X, COUNT_Y + 14, &note, colour);
        }
        body_stub(canvas, ink, w);

        let ok = JobScreen::ok_button(self.job);
        let drawn = ctx
            .assets
            .chrome
            .as_ref()
            .is_some_and(|ch| ch.draw_system(canvas, system::OK, ok.x, ok.y));
        if !drawn {
            widget::button(canvas, ink, ok, "OK", false);
        }
    }
}

/// **A stub, and it looks like one.** The five body painters report sowing,
/// herd forecasts, reclamation, efficiency and weapons; none of that is
/// modelled at the fidelity the panel prints it.
fn body_stub(canvas: &mut Canvas, ink: &Ink, w: Rect) {
    let y = w.y + 80;
    text::draw(canvas, w.x + 16, y, "THIS JOB'S OWN REPORT", ink.dim);
    text::draw(canvas, w.x + 16, y + 14, "NOT SIMULATED", ink.bad);
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_kingdom::county::{LABOUR_NO_FLOOR, LABOUR_UNBOUNDED};
    use l2_kingdom::tables::{JOB_CATTLE_FARMING, JOB_WOOD_CUTTING};

    fn county() -> County {
        let mut c = County::new();
        c.population = 435;
        c.pop_band = 18;
        c.labour[JOB_CATTLE_FARMING] = 218;
        c.labour_wanted[JOB_CATTLE_FARMING] = 302;
        c.labour_useful[JOB_CATTLE_FARMING] = 302;
        c.labour[JOB_WOOD_CUTTING] = 217;
        c.labour_wanted[JOB_WOOD_CUTTING] = LABOUR_NO_FLOOR;
        c.labour_useful[JOB_WOOD_CUTTING] = LABOUR_UNBOUNDED;
        c
    }

    /// County 1 of the shipped save reads exactly this way: its cattle are
    /// short and its foresters are not, and the two statements come from two
    /// different words of the same twelve-byte record.
    #[test]
    fn the_colour_rule_is_the_two_words_the_record_carries() {
        let c = county();
        assert_eq!(staffing(&c, JOB_CATTLE_FARMING), Staffing::Short);
        assert_eq!(staffing(&c, JOB_WOOD_CUTTING), Staffing::Right);

        let mut c2 = c;
        c2.labour[JOB_CATTLE_FARMING] = 400;
        assert_eq!(staffing(&c2, JOB_CATTLE_FARMING), Staffing::Wasted, "past the ceiling");
        c2.labour[JOB_CATTLE_FARMING] = 302;
        assert_eq!(staffing(&c2, JOB_CATTLE_FARMING), Staffing::Right, "exactly right");

        // -1 and 100,000 are the two sentinels, and neither can ever fire.
        let mut c3 = County::new();
        c3.labour_wanted = [LABOUR_NO_FLOOR; JOB_COUNT];
        c3.labour_useful = [LABOUR_UNBOUNDED; JOB_COUNT];
        for job in 0..JOB_COUNT {
            c3.labour[job] = 0;
            assert_eq!(staffing(&c3, job), Staffing::Right);
            c3.labour[job] = 99_999;
            assert_eq!(staffing(&c3, job), Staffing::Right);
        }
    }

    /// Every one of the nine windows is on screen and holds its own tick.
    #[test]
    fn every_job_window_is_on_screen_and_holds_its_own_ok_button() {
        for job in 0..JOB_COUNT {
            let w = JobScreen::window(job);
            assert!(w.x + w.w <= 640 && w.y + w.h <= 480, "job {job}: {w:?}");
            let ok = JobScreen::ok_button(job);
            assert!(ok.x >= w.x && ok.x + ok.w <= w.x + w.w, "job {job}: tick escapes in x");
            assert!(ok.y >= w.y, "job {job}: the tick is below the box's top");
        }
        // The two farm jobs get the tallest windows, which is where their own
        // reports go.
        assert_eq!(BOX_ROWS[0], 13);
        assert_eq!(BOX_ROWS[1], 13);
        assert_eq!(BOX_ROWS[3], 11, "castle building is the third-tallest");
    }
}
