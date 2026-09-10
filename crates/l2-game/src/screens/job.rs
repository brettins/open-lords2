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
//! **This is nine panels sharing one painter, and the number that says so is
//! 5 against 113.** `Panel_JobDetail`'s own body makes five draw calls; its
//! tree makes 113, because it dispatches to one of five body painters on
//! `g_jobPanelJob`. Per *frame* the original makes five plus whichever body
//! runs, so job 3 (field reclamation) is 11 draws and job 2 (cattle) is 34.
//! Summing the tree counts nine panels at once and is the wrong denominator for
//! any single one of them.
//!
//! # The painter, address by address
//!
//! ```text
//! Panel_JobDetail():                                            0x00412B33
//!   if (g_jobPanelJob == 0) return                     nothing at all
//!   File_ReadChunk("iconvill.pl8", scratch, 160000, 0)
//!   g_spriteWidth = 0x19                              25 cells = 400 px
//!   if (g_jobPanelJob == 8):                          the blacksmith, below
//!     Screen_DrawMenuBar(); Sound_PlayFile("fire.wav"); Sound_RestartSlot(8)
//!   else:
//!     Sound_RestartSlot(g_jobSound[job])              if it has one
//!     g_spriteHeight = g_jobPanelRows[job]
//!     Ui_DrawBox(0x30, 0x60, 0x19, rows)              (48, 96) 400 x rows*16
//!     FUN_00403CF4(0x40, 0x68, 0x32, 0x32, 0x3F)  a 50 x 50 **outline** in
//!                                                 colour 0x3F at (64, 104)
//!     Sprite_WGenSprite(DAT_004D2974[job], 0x41, 0x69)   the icon (65, 105)
//!     Eng_DrawString(74, job, 0x80, 0x6A, heading)    the job's name (128, 106)
//!     colour = 0x3F
//!     if   (labour[job].workers < labour[job].wanted) colour = 0xF9
//!     elif (labour[job].useful  < labour[job].workers) colour = 0xFC
//!     Ui_DrawCount(labour[job].workers, job * 2 + 0x1E, 0x80, 0x88, body, colour)
//!   match job:  1 -> Panel_JobGrain()        27 draw calls
//!               2 -> Panel_JobCattle()       29
//!               3 -> Panel_JobReclamation()   6
//!               4 -> FUN_00414220()          13  = Castle_DrawStatusBlock
//!               5,6,7 -> Panel_JobIndustry() 12
//!               8 -> Panel_JobBlacksmith()   21
//!               9 -> nothing                  0
//!   if (job == 8) Ui_OkButton(0x1C0, 0x1C0, 0)                  (448, 448)
//!   else          Ui_OkButton(0x1A4, rows * 0x10 + 0x44, 0)
//!   [Screen_DrawWidgets 0x0F] if (g_jobPanelJob == 8) FUN_00413526()
//! ```
//!
//! `g_jobPanelRows` (`0x004D29A0`), for jobs 1…9: **13, 13, 9, 11, 9, 9, 9, 9,
//! 9** — thirteen cells for grain and cattle, eleven for castle building, nine
//! for the rest. `DAT_004D2974`, the `iconvill.pl8` frame per job: **0, 2, 3,
//! 5, 6, 7, 8, 9, 10**, with job 9 overridden to `0x10` in the painter itself.
//! `L2.eng` group 74 index 1…9 is what fixes the nine slots' order:
//! *Grain farming. Cattle farming. Field reclamation. Castle building. Iron
//! mining. Stone quarrying. Wood cutting. Blacksmith. Idle townsfolk.* Index 0,
//! *"Idle people."*, is unreachable — the painter returns on job 0.
//!
//! `Ui_DrawCount(n, job * 2 + 0x1E)` picks the singular or plural noun out of
//! group 8, whose pairs at 32, 34, 36 … 48 are *Farmer*, *Dairy maid*, *Serf*,
//! *Builder*, *Miner*, *Quarrier*, *Forester*, *Blacksmith*, *Peasant* — job
//! `j` (1-based) takes pair `j * 2 + 30`, which for job 1 is 32. **[V]** against
//! the shipped `L2.eng`.
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
//! # Job 8 is not this panel at all, and that is the audit's finding here
//!
//! The blacksmith takes the *other* branch of every `if` in the painter. It
//! draws **no** `Ui_DrawBox(0x30, 0x60, …)`, no icon recess, no group-74 title
//! and no worker count; instead it calls `Screen_DrawMenuBar()`, starts
//! `fire.wav`, and hands over to `Panel_JobBlacksmith` (`0x00413155`), which is
//! a **full-screen page**:
//!
//! ```text
//! Panel_JobBlacksmith():                                        0x00413155
//!   File_ReadChunk("smithy.pl8", scratch, 200000, 0)
//!   Sprite_WGenSprite(0, 0, 0x18)                     the shop, at (0, 24)
//!   FUN_004B414A(0, 0x1A8, 0)                         a 30 x 56 strip at y 424
//!   File_ReadChunk("hearth.pl8", scratch, 200000, 0)
//!   Pl8_DrawFrameClipped(scratch, weapon + 0xB, 0, 0x1A8 - DAT_004D29E0[weapon])
//!   Ui_DrawBox(0, 0x180, 0x1E, 6)              (0, 384) 480 x 96 — a footer
//!   Eng_DrawString(74, 8, 0x10, 0x186, heading)  "Blacksmith."   (16, 390)
//!   Ui_DrawCentred(75, 0, 0, 0x1CC, 0x1CC, body)
//!                        "Click on a weapon to change production."  y = 460
//!   ... six more lines from group 76, then
//!   Ui_DrawInsetRect(0x140, 0x186, 0x8E, 0x20)        (320, 390) 142 x 32
//!   Pl8_DrawFrame(Misc_cty, 0x2C, 0x146, 0x187)  iron icon       (326, 391)
//!   Ui_DrawNumber(DAT_004D8994[weapon], …, 0x16A, 0x18E, body)   (362, 398)
//!   Pl8_DrawFrame(Misc_cty, 0x2E, 0x189, 0x18A)  wood icon       (393, 394)
//!   Ui_DrawNumber(g_weaponCost[weapon], …, 0x1B4, 0x18E, body)   (436, 398)
//!   Ui_OkButton(0x1C0, 0x1C0, 0)                                (448, 448)
//! ```
//!
//! And `Screen_DrawWidgets`'s `0x0F` arm — the only per-frame drawing outside
//! any painter on this screen — is `if (g_jobPanelJob == 8) FUN_00413526()`,
//! which is **the forge fire**:
//!
//! ```text
//! FUN_00413526():                                               0x00413526
//!   if (g_pulse80 == 0) return
//!   DAT_004E59BC = (DAT_004E59BC + 1) % 11
//!   Pl8_DrawFrameClipped(scratch, DAT_004E59BC, 0x58, 0x9D)      (88, 157)
//!   Gfx_MarkSpriteDirty(0x58, 0x9D, 8, 8, 1)
//! ```
//!
//! `scratch` still holds `hearth.pl8` from `Panel_JobBlacksmith`'s second load,
//! and `Hearth.pl8` has **17 frames: 0 … 10 are eleven 69 × 52 fire frames and
//! 11 … 16 are six hearth pictures, one per weapon** — which is exactly the
//! `weapon + 0xB` the painter draws and exactly the `% 11` the animator cycles.
//! The sheet's own frame table is the check on both. **[V]**
//!
//! So screen `0x0F` is really *two* pages that happen to share an id, and this
//! module draws the small one for all nine jobs. The blacksmith is recorded
//! missing rather than approximated.
//!
//! **The two `Ui_OkButton` call sites are an if/else, not two buttons.** Worth
//! saying because `Ui_OkButton` (`0x0040D1BC`) stashes only the last call's
//! position, so a screen that really drew two would have a dead first one —
//! here only one ever runs per frame, and the mechanical count of 2 is a count
//! of *call sites*.
//!
//! **The five other bodies are not here.** `Panel_JobGrain`, `Panel_JobCattle`,
//! `Panel_JobReclamation`, `Castle_DrawStatusBlock` and `Panel_JobIndustry` are
//! about 4,000 bytes over 87 draw calls, drawing `L2.eng` groups **77** (grain,
//! herd and reclamation vocabulary, 31 strings) and **76** (the industry
//! efficiency lines, 9 strings) against county fields `+0x204`, `+0x214`,
//! `+0x22C`, `+0x230`, `+0x24C`, `+0x270`, `+0x274`, `+0x278`, `+0x280`,
//! `+0x284`, `+0x288`, `+0x28C` and `+0x2FC` — none of which `docs/kingdom.md`
//! §1.3 has. The window, the head and the colour rule are the part that is
//! read; [`body_stub`] says so.

use l2_kingdom::county::County;
use l2_kingdom::tables::{JOB_COUNT, JOB_NAMES};
use l2_view::chrome::system;
use l2_view::{text, Canvas, Ink};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `Ui_DrawBox(0x30, 0x60, …)` — the window's origin and its width in cells.
const BOX_X: i32 = 48;
const BOX_Y: i32 = 96;
const BOX_COLS: i32 = 25;

/// `DAT_004D29A0`, the per-job height in 16-pixel cells, indexed by the
/// original's 1-based job number. Slot 0 here is job 1.
const BOX_ROWS: [i32; JOB_COUNT] = [13, 13, 9, 11, 9, 9, 9, 9, 9];

/// `FUN_00403CF4(0x40, 0x68, 0x32, 0x32, 0x3F)` — **a one-pixel outline, not a
/// recess.** The function is four `FUN_00403A8F` line calls in colour `0x3F`
/// round the given box, clipped to the screen; there is no fill and no
/// lighting. `Sprite_WGenSprite(DAT_004D2974[job], 0x41, 0x69)` puts the job's
/// `iconvill.pl8` picture one pixel inside it.
const ICON_BOX: Rect = Rect::new(64, 104, 50, 50);
/// The outline's colour, which is the painter's own fifth argument.
const ICON_BOX_INK: u8 = 0x3F;

/// `Eng_DrawString(0x4A, job, 0x80, 0x6A, …)` and
/// `Ui_DrawCount(n, job*2+30, 0x80, 0x88, …)`.
const NAME_X: i32 = 128;
const NAME_Y: i32 = 106;
const COUNT_Y: i32 = 136;

/// **`L2.eng` group 74** — the nine job names, index `job + 1` for a zero-based
/// job, which is what fixes the nine slots' order. Index 0, *"Idle people."*,
/// is unreachable: `Panel_JobDetail` returns when `g_jobPanelJob` is 0.
pub const JOB_GROUP: usize = 74;

/// `L2.eng` group 8, the countable-noun table, whose pairs are singular then
/// plural. `Ui_DrawCount(n, job * 2 + 0x1E)` with the original's 1-based job is
/// `job * 2 + 32` with ours, so [`WORKER_NOUN_0`] is *Farmer* / *Farmers*.
pub const COUNT_NOUN_GROUP: usize = 8;
pub const WORKER_NOUN_0: usize = 32;
pub const WORKER_NOUN_STRIDE: usize = 2;

/// Our transcription of those nine pairs, for a machine with no `L2.eng`.
/// Upper case because the fallback font has no lower case.
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

/// **Job 8 (zero-based 7), the blacksmith, is a different page.** The painter
/// branches away from the window, the icon, the title and the count for this
/// one job and hands over to `Panel_JobBlacksmith`, a full-screen page over
/// `smithy.pl8` and `hearth.pl8`. We draw the small panel for it, and say so.
pub const BLACKSMITH: usize = 7;

/// `Ui_OkButton(0x1C0, 0x1C0, 0)` — the blacksmith page's corner, which is not
/// where the other eight put theirs.
const BLACKSMITH_OK: Rect = Rect::new(0x1C0, 0x1C0, system::OK_DIM, system::OK_DIM);

/// The three colours `Panel_JobDetail` passes to `Ui_DrawCount`, as literal
/// palette indices rather than as [`Ink`](l2_view::Ink) choices of ours.
const COUNT_RIGHT: u8 = 0x3F;
const COUNT_SHORT: u8 = 0xF9;
const COUNT_WASTED: u8 = 0xFC;

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
    /// own origin — the corner picture hangs four pixels below the bottom row, which is
    /// the original's arrangement and worth not tidying.
    ///
    /// **The blacksmith's is somewhere else entirely**, because the blacksmith
    /// is a full-screen page: `Ui_OkButton(0x1C0, 0x1C0, 0)`. The painter has
    /// two `Ui_OkButton` call sites and they are the two arms of one `if`, so
    /// only ever one of them runs.
    pub fn ok_button(job: usize) -> Rect {
        let job = job.min(JOB_COUNT - 1);
        if job == BLACKSMITH {
            return BLACKSMITH_OK;
        }
        Rect::new(0x1A4, BOX_ROWS[job] * 0x10 + 0x44, system::OK_DIM, system::OK_DIM)
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
            // **The minimap is live under this popup**, and it is the one thing
            // from the right-hand column that is: `0x0F`'s arm runs
            // `Ui_OkButtonClicked` and a right-release test and **none** of the
            // six sidebar guards, while `Screen_FrameInput`'s epilogue runs
            // `Minimap_Click` on every screen id but `0x12`. So a press on the
            // raster selects that county, re-centres the map and drops the
            // popup — and the epilogue names this screen specially while doing
            // it: `if (g_screenId == 0x0F) Sound_StopOneShot();`, because the
            // job popup is one of the few screens that starts a voice clip.
            //
            // Passing only the raster, and not the whole column, is the
            // difference between reproducing the arm and inventing five more.
            // arm: 0x0042FF10/minimap-under-the-job-popup left-press
            Event::Click { x, y } if l2_view::chrome::minimap_hit_area().contains(x, y) => {
                Transition::Pass
            }
            // `Screen_FrameInput`'s `0x0F` arm: the corner picture **or** a right release
            // closes the popup, and it returns to whichever screen opened it —
            // the village when `DAT_005533F4` is zero, the campaign map's
            // sidebar otherwise. `Transition::Pop` is both, because the stack
            // remembers what our `g_screenId` cannot.
            // arm: 0x0042FF10/job-popup-closes right-release
            Event::RightClick { .. } => Transition::Pop,
            // **Ours, and counted.** `0x0F`'s arm reads no key at all.
            // arm: ours/job-popup-keyboard key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
            Event::Click { x, y } if JobScreen::ok_button(self.job).contains(x, y) => {
                Transition::Pop
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let w = JobScreen::window(self.job);
        // `Ui_DrawBox(0x30, 0x60, 0x19, g_jobPanelRows[job])`, border set 0.
        pen.window(canvas, w.x, w.y, BOX_COLS, BOX_ROWS[self.job], 0);

        // `FUN_00403CF4(0x40, 0x68, 0x32, 0x32, 0x3F)` — **four lines in colour
        // 0x3F, no fill.** `iconvill.pl8` is loaded only by this panel and
        // `l2-view` does not carry that sheet, so the box stays empty and looks
        // it; the frame the original puts in it is `DAT_004D2974[job + 1]`,
        // one pixel inside at (65, 105).
        for (x, y, w2, h) in [
            (ICON_BOX.x, ICON_BOX.y, ICON_BOX.w, 1),
            (ICON_BOX.x, ICON_BOX.y + ICON_BOX.h - 1, ICON_BOX.w, 1),
            (ICON_BOX.x, ICON_BOX.y, 1, ICON_BOX.h),
            (ICON_BOX.x + ICON_BOX.w - 1, ICON_BOX.y, 1, ICON_BOX.h),
        ] {
            canvas.fill_rect(x, y, w2, h, ICON_BOX_INK);
        }

        // `Eng_DrawString(74, job + 1, 0x80, 0x6A, heading, 0x3F)`.
        let title = eng(ctx, JOB_GROUP, self.job + 1, JOB_NAMES[self.job]);
        pen.heading(canvas, NAME_X, NAME_Y, &title, COUNT_RIGHT);

        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        let n = c.labour[self.job];
        let colour = match staffing(c, self.job) {
            Staffing::Short => COUNT_SHORT,
            Staffing::Wasted => COUNT_WASTED,
            Staffing::Right => COUNT_RIGHT,
        };
        // `Ui_DrawCount(labour[job].workers, job * 2 + 0x1E, 0x80, 0x88, body,
        // colour)` — the number, then group 8's singular or plural. The
        // original takes the **singular** at ±1 and the plural everywhere else,
        // zero included: *"0 Farmers"*.
        let (one, many) = WORKER_NOUN[self.job];
        let singular = n.abs() == 1;
        let index = WORKER_NOUN_0 + self.job * WORKER_NOUN_STRIDE + usize::from(!singular);
        let ours = if singular { one } else { many };
        let x = pen.body(canvas, NAME_X, COUNT_Y, &format!("{n} "), colour);
        pen.body(canvas, x, COUNT_Y, &eng(ctx, COUNT_NOUN_GROUP, index, ours), colour);

        body_stub(canvas, ink, w, self.job);

        // `Ui_OkButton(0x1A4, rows * 0x10 + 0x44, 0)` — or, for the blacksmith,
        // `(0x1C0, 0x1C0)`. `Pen::ok_button` draws `System.pl8` frame 0x33 and
        // falls back to a recess of ours **with no letters in it**, because the
        // original's picture is an arrow into a hole and never the word "OK".
        let ok = JobScreen::ok_button(self.job);
        pen.ok_button(canvas, ok.x, ok.y, 0);
    }
}

/// One `L2.eng` string, from the install if it has one and from our own
/// transcription if it does not — the same shape
/// [`screens::county`](crate::screens::county) uses.
fn eng(ctx: &Ctx, group: usize, index: usize, fallback: &str) -> String {
    let s = ctx.assets.shell.text(group, index);
    if s.is_empty() {
        fallback.to_uppercase()
    } else {
        s.to_string()
    }
}

/// **A stub, and it looks like one.** The five body painters report sowing,
/// herd forecasts, reclamation, efficiency and weapons; none of that is
/// modelled at the fidelity the panel prints it. Every line here is **ours**,
/// in our own 5 × 7 font, so that a screenshot cannot be mistaken for the
/// original's page.
fn body_stub(canvas: &mut Canvas, ink: &Ink, w: Rect, job: usize) {
    let y = w.y + 80;
    text::draw(canvas, w.x + 16, y, "THIS JOB'S OWN REPORT", ink.dim);
    text::draw(canvas, w.x + 16, y + 14, "NOT SIMULATED", ink.bad);
    if job == BLACKSMITH {
        // And this one is not even this window — see the module docs.
        text::draw(canvas, w.x + 16, y + 28, "THE SMITHY IS A FULL PAGE", ink.bad);
    }
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

    /// Every one of the nine windows is on screen and holds its own corner —
    /// **except the blacksmith's**, whose corner is at (448, 448) because the
    /// blacksmith is not this window at all.
    #[test]
    fn every_job_window_is_on_screen_and_holds_its_own_ok_button() {
        for job in 0..JOB_COUNT {
            let w = JobScreen::window(job);
            assert!(w.x + w.w <= 640 && w.y + w.h <= 480, "job {job}: {w:?}");
            let ok = JobScreen::ok_button(job);
            assert!(ok.x + ok.w <= 640 && ok.y + ok.h <= 480, "job {job}: {ok:?} is off screen");
            if job == BLACKSMITH {
                continue;
            }
            assert!(ok.x >= w.x && ok.x + ok.w <= w.x + w.w, "job {job}: the corner escapes in x");
            assert!(ok.y >= w.y, "job {job}: the corner is below the box's top");
        }
        // The two farm jobs get the tallest windows, which is where their own
        // reports go.
        assert_eq!(BOX_ROWS[0], 13);
        assert_eq!(BOX_ROWS[1], 13);
        assert_eq!(BOX_ROWS[3], 11, "castle building is the third-tallest");
    }

    /// **`g_jobPanelRows` (`0x004D29A0`), verbatim**, and the blacksmith's
    /// separate corner. The literals are pinned from the binary's own bytes —
    /// `0, 13, 13, 9, 11, 9, 9, 9, 9, 9` for jobs 0…9, one-based — rather than
    /// computed from [`BOX_ROWS`], so ablating the table turns this red.
    #[test]
    fn the_blacksmith_is_the_one_job_that_is_not_this_window() {
        assert_eq!(BOX_ROWS, [13, 13, 9, 11, 9, 9, 9, 9, 9]);
        assert_eq!(BLACKSMITH, 7, "job 8 one-based; L2.eng 74/8 is \"Blacksmith.\"");
        assert_eq!(JobScreen::ok_button(BLACKSMITH), BLACKSMITH_OK);
        assert_eq!(BLACKSMITH_OK.x, 0x1C0);
        assert_eq!(BLACKSMITH_OK.y, 0x1C0);
        for job in 0..JOB_COUNT {
            if job == BLACKSMITH {
                continue;
            }
            assert_eq!(
                JobScreen::ok_button(job).y,
                BOX_ROWS[job] * 0x10 + 0x44,
                "job {job}: Ui_OkButton(0x1A4, rows * 0x10 + 0x44, 0)"
            );
            assert_eq!(JobScreen::ok_button(job).x, 0x1A4);
        }
    }

    /// **`Ui_DrawCount(n, job * 2 + 0x1E)` on `L2.eng` group 8**, whose pairs
    /// are singular then plural. Checked against the shipped file's own words:
    /// 32/33 *Farmer(s)*, 34/35 *Dairy maid(s)*, … 48/49 *Peasant(s)*. The
    /// index arithmetic is the assertion; the transcription is the fallback.
    #[test]
    fn the_worker_noun_is_group_eight_at_job_times_two_plus_thirty() {
        assert_eq!(COUNT_NOUN_GROUP, 8);
        assert_eq!(WORKER_NOUN_0, 32, "job 1 one-based is 1 * 2 + 30");
        for job in 0..JOB_COUNT {
            let singular = WORKER_NOUN_0 + job * WORKER_NOUN_STRIDE;
            assert_eq!(singular % 2, 0, "group 8 is singular at even indices");
            assert!(singular + 1 < 74, "group 8 has 74 strings");
        }
        assert_eq!(WORKER_NOUN_0 + BLACKSMITH * WORKER_NOUN_STRIDE, 46, "8/46 Blacksmith");
        assert_eq!(JOB_GROUP, 74, "L2.eng 74, index job + 1");
    }
}
