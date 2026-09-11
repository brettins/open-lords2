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
//! # The five bodies, and the words they are made of
//!
//! The five body painters below are the original's, call for call. Each draws
//! **absolute** screen coordinates rather than box-relative ones, and each
//! draws its words out of the player's own `L2.eng` — rule 6 — with our
//! transcription only where the install has no file:
//!
//! | job (ours) | painter | calls | groups |
//! |---|---|---:|---|
//! | 0 grain | [`grain`] = `Panel_JobGrain` (`0x00413590`) | 27 | 77, 22, 8 |
//! | 1 cattle | [`cattle`] = `Panel_JobCattle` (`0x00413B30`) | 29 | 77, 8 |
//! | 2 reclamation | [`reclamation`] = `Panel_JobReclamation` (`0x004140F3`) | 6 | 77, 8 |
//! | 3 castle | [`castle_status_block`] = `Castle_DrawStatusBlock` (`0x0041DEDB`) via `FUN_00414220` | 13 | 71, 8 |
//! | 4, 5, 6 industry | [`industry`] = `Panel_JobIndustry` (`0x00412E6B`) | 12 | 76, 8 |
//!
//! **Group 77 has six consumers, 76 two and 71 four**, so none of them is this
//! popup's alone; what makes them its vocabulary is that the popup draws them
//! and nothing of ours drew them before. `docs/formats/eng.md` §5.
//!
//! Every `Ui_DrawCount` here is `'@'` and `""` (C155), and the three suffixes
//! that are not were read out of the shipped exe: `Panel_JobIndustry`'s
//! efficiency is `&DAT_004D3E74` = `"%"`, `Panel_JobReclamation`'s count is
//! `&DAT_004D3EE0` = `""`, and `Castle_DrawStatusBlock`'s tax bonus is
//! `&DAT_004D4290` = `" %"` beside a barracks figure of `&DAT_004D4294` = `" "`.
//! Every zero row's `Ui_DrawNumber(0, '@', …)` and every `Ui_DrawDelta` prefix
//! and suffix in the grain and cattle bodies — `&DAT_004D3EA4` …
//! `&DAT_004D3EDC`, fifteen pointers — holds `" "`. **[V]**
//!
//! # `g_penAdvance`, and why the chains below are plain
//!
//! A line such as `Ui_DrawCount(v, 2, 0x40, y)` followed by
//! `Eng_DrawString(77, 1, g_penAdvance + 0x40, y)` is one sentence: the second
//! piece starts where the first ended. `Ui_DrawCount` (`0x0041AB67`) and
//! `Ui_DrawDelta` (`0x00402E0C`) both **save `g_penAdvance`, zero it, draw, and
//! add the saved value back**, so a count passed `x = g_penAdvance + 0x40`
//! places its own noun from its own start and never counts the pen twice.
//! Every [`Pen`] method returns the absolute x of the next glyph, which is that
//! sum already. **[V]**, both bodies read.
//!
//! # What is still not drawn here
//!
//! * **The blacksmith**, job 7: `Panel_JobBlacksmith` is a full-screen page over
//!   `smithy.pl8` and `hearth.pl8`, 21 calls, and it is recorded above.
//!   [`smithy_stub`] says so on screen.
//! * **The job's picture**, `Sprite_WGenSprite(DAT_004D2974[job], 0x41, 0x69)`:
//!   `iconvill.pl8` is not a sheet this crate loads.
//! * **`Castle_DrawStatusBlock`'s other caller**, `TileInfo_DrawCastle`
//!   (`0x0041DA2F`), which draws it at `(8, 0x30, row)` for a castle under
//!   construction on the player's own tile. `screens/info.rs`'s layout ladder
//!   has no castle arm to call it from — it returns row `0x0A` for nothing.

use l2_kingdom::county::County;
use l2_kingdom::tables::{
    Commodity, Tables, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_NAMES, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
};
use l2_view::chrome::system;
use l2_view::{text, Canvas, Ink};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{count_noun, font, Face, Pen};

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
        // Through `Pen::count_with_noun` for `Ui_DrawCount`'s `'@'` lead and empty
        // suffix. Built by hand as `"{n} "`, the digits sat four pixels left.
        let noun = eng(ctx, COUNT_NOUN_GROUP, index, ours);
        let face = crate::shell::Face::Body;
        pen.count_with_noun(face, canvas, NAME_X, COUNT_Y, i32::from(n), &noun, colour);

        // `Panel_JobDetail`'s dispatch on `g_jobPanelJob`, zero-based here:
        // one-based 5, 6 and 7 take `Panel_JobIndustry`, 9 takes nothing.
        match self.job {
            JOB_GRAIN_FARMING => grain(&pen, ctx, canvas, c),
            JOB_CATTLE_FARMING => cattle(&pen, ctx, canvas, c),
            JOB_FIELD_RECLAMATION => reclamation(&pen, ctx, canvas, c),
            // `FUN_00414220` is `Castle_DrawStatusBlock(g_selectedCounty, -0x20,
            // 0x40, 0)` and nothing else.
            JOB_CASTLE_BUILDING => castle_status_block(&pen, ctx, canvas, c, -0x20, 0x40, 0),
            JOB_IRON_MINING | JOB_STONE_QUARRYING | JOB_WOOD_CUTTING => {
                industry(&pen, ctx, canvas, c, self.job)
            }
            BLACKSMITH => smithy_stub(canvas, ink, w),
            _ => {}
        }

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

/// **Ours, and it looks like it.** The blacksmith is `Panel_JobBlacksmith`, a
/// full-screen page, and this module draws the small window for it instead —
/// so the window says so, in our own 5 × 7 font, where a screenshot cannot
/// mistake it for the original's page.
fn smithy_stub(canvas: &mut Canvas, ink: &Ink, w: Rect) {
    text::draw(canvas, w.x + 16, w.y + 80, "THE SMITHY IS A FULL PAGE", ink.bad);
}

// ------------------------------------------------------------ the five bodies

/// **`L2.eng` group 77** — the grain, herd and reclamation forecast lines.
/// Consumers in the binary: `Panel_JobGrain`, `Panel_JobCattle`,
/// `Panel_JobReclamation`, `TileInfo_DrawGrain`, `TileInfo_DrawHerd` and
/// `Msg_DrawWindow`'s event arm.
pub const FORECAST_GROUP: usize = 77;
/// **`L2.eng` group 76** — the industry lines. `Panel_JobIndustry` draws 0…3,
/// `Panel_JobBlacksmith` 4…8.
pub const INDUSTRY_GROUP: usize = 76;
/// **`L2.eng` group 71** — castle selection and status; this body draws 6, 7,
/// 8, 11, 12, 16 and 17.
pub const CASTLE_GROUP: usize = 71;
/// **`L2.eng` group 22** — the seven fertility phrases, index
/// `(fertility + 100) / 29`.
pub const FERTILITY_GROUP: usize = 22;

/// Group 8's singular index for each count these bodies draw; `Ui_DrawCount`
/// takes the plural one along for anything but ±1.
pub const NOUN_SACK: usize = 2;
pub const NOUN_ANIMAL: usize = 4;
pub const NOUN_IRON: usize = 0x0C;
pub const NOUN_STONE: usize = 0x0E;
pub const NOUN_WOOD: usize = 0x10;
pub const NOUN_BUILDER: usize = 0x26;
pub const NOUN_SEASON: usize = 0x42;

/// The ink of every line of every body: the painters' literal `0x3F`.
const BODY_INK: u8 = font::TEXT;
/// `Ui_DrawDelta`'s `colourNeg` at all six body call sites.
const DELTA_NEG: u8 = 0xF9;

/// Our transcription of group 77, for an install with no `L2.eng`.
const OURS_77: [&str; 31] = [
    "from",
    "to be sown, yielding",
    "in 4 seasons.",
    "harvested in",
    "sown in spring.",
    "Calf births expected",
    "Cow deaths expected",
    "Change due to farming",
    "Low herd crowding.",
    "Average herd crowding.",
    "Herd overcrowded.",
    "Massive overcrowding!!",
    "field being reclaimed",
    "fields being reclaimed",
    "Next field reclaimed in",
    "No field reclamation with 0 labourers",
    "gained last season, due to weather.",
    "lost last season, due to weather.",
    "Weather had no effect last season.",
    "No outside events affected the herd this season.",
    "died of disease.",
    "taken by wolves.",
    "had to be put down.",
    "born, over expectations.",
    "No outside factors affected stored grain.",
    "eaten by rats.",
    "found as surplus.",
    "Change due to eating",
    "Overall change",
    "extra deaths.",
    "extra births.",
];

/// Group 76, the same.
const OURS_76: [&str; 9] = [
    "Working with an efficiency of",
    "will be produced next season.",
    "will be used by the blacksmiths.",
    "needed by castle builders.",
    "Serfs working at",
    "efficiency.",
    "Will produce",
    "next season.",
    "Smiths working.",
];

/// Group 71, the same.
const OURS_71: [&str; 20] = [
    "Select a castle to build",
    "Wooden palisade.",
    "Motte and bailey.",
    "Norman keep.",
    "Stone castle.",
    "Royal castle.",
    "of stone needed,",
    "of wood needed.",
    "will take",
    "to build.",
    "Build this castle",
    "Barracks for",
    "troops.",
    "currently stationed here.",
    "View these troops?",
    "Start construction?",
    "Boosts tax revenues by",
    "No castle building in progress.",
    "Needed",
    "Enemy troops are barracked here.",
];

/// Group 22, the same.
const OURS_22: [&str; 7] = [
    "Infertile - almost no production.",
    "Very poor fertility - mainly weeds.",
    "Poor fertility - crops grow less well.",
    "Average fertility - no effect on crops.",
    "Good fertility - crops are boosted.",
    "Very High fertility - many extra crops.",
    "Excellent fertility - bumper crop!",
];

/// Our transcription of one string these bodies draw.
fn ours(group: usize, index: usize) -> &'static str {
    let table: &[&str] = match group {
        FORECAST_GROUP => &OURS_77,
        INDUSTRY_GROUP => &OURS_76,
        CASTLE_GROUP => &OURS_71,
        FERTILITY_GROUP => &OURS_22,
        COUNT_NOUN_GROUP => {
            return match index {
                2 => "Sack",
                3 => "Sacks",
                4 => "Animal",
                5 => "Animals",
                0x0C | 0x0E | 0x10 => "Tonne",
                0x0D | 0x0F | 0x11 => "Tonnes",
                0x26 => "Builder",
                0x27 => "Builders",
                0x42 => "Season",
                0x43 => "Seasons",
                _ => "",
            }
        }
        _ => &[],
    };
    table.get(index).copied().unwrap_or("")
}

/// `Eng_DrawString(group, index, x, y, &g_fontBody, 0x3F)`. Returns the x the
/// next piece of the sentence starts at.
#[allow(clippy::too_many_arguments)]
fn say(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, group: usize, index: usize, x: i32, y: i32) -> i32 {
    let s = eng(ctx, group, index, ours(group, index));
    pen.body(canvas, x, y, &s, BODY_INK)
}

/// `Ui_DrawCount(value, noun, x, y, &g_fontBody, 0x3F)`.
#[allow(clippy::too_many_arguments)]
fn count(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, value: i32, noun: usize, x: i32, y: i32) -> i32 {
    let index = count_noun(value, noun);
    let s = eng(ctx, COUNT_NOUN_GROUP, index, ours(COUNT_NOUN_GROUP, index));
    pen.count_with_noun(Face::Body, canvas, x, y, value, &s, BODY_INK)
}

/// `Ui_DrawNumber(0, '@', " ", x, y, &g_fontBody, 0x3F)` — the zero every
/// `Ui_DrawDelta` row draws instead when its value is zero, because mode 0
/// would draw nothing at all.
fn zero(pen: &Pen, canvas: &mut Canvas, x: i32, y: i32) {
    pen.number_in(Face::Body, canvas, x, y, 0, '@', " ", BODY_INK);
}

/// `Ui_DrawDelta(value, 0, " ", " ", x, y, &g_fontBody, 0x3F, 0xF9)` — the
/// prefix, then the magnitude with its sign as the lead character, both in the
/// sign's colour. Every caller here tests zero first and draws [`zero`] 8
/// pixels right instead (`0x130` against `0x128`), which is exactly where this
/// one's digits land: the prefix is four pixels of space and four of
/// `Ui_DrawText`'s trailer.
fn delta(pen: &Pen, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    if value == 0 {
        return;
    }
    let colour = if value < 0 { DELTA_NEG } else { BODY_INK };
    let next = pen.body(canvas, x, y, " ", colour);
    let (lead, shown) = if value < 0 { ('-', value.wrapping_neg()) } else { ('+', value) };
    pen.number_in(Face::Body, canvas, next, y, shown, lead, " ", colour);
}

/// The weather's line, which `Panel_JobGrain` and `Panel_JobCattle` both write
/// out in full at `y = 0xC0`, advanced farming only:
///
/// ```c
/// g_penAdvance = 0;
/// if (v < 1) {
///   if (v < 0) { Ui_DrawCount(-v, noun, 0x40, 0xc0); Eng_DrawString(77, 0x11, pen + 0x40, 0xc0); }
///   else       { Eng_DrawString(77, 0x12, 0x40, 0xc0); }
/// } else       { Ui_DrawCount(v, noun, 0x40, 0xc0);  Eng_DrawString(77, 0x10, pen + 0x40, 0xc0); }
/// ```
fn weather_line(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, v: i32, noun: usize) {
    const Y: i32 = 0xC0;
    if v < 0 {
        let at = count(pen, ctx, canvas, v.wrapping_neg(), noun, 0x40, Y);
        say(pen, ctx, canvas, FORECAST_GROUP, 0x11, at, Y);
    } else if v == 0 {
        say(pen, ctx, canvas, FORECAST_GROUP, 0x12, 0x40, Y);
    } else {
        let at = count(pen, ctx, canvas, v, noun, 0x40, Y);
        say(pen, ctx, canvas, FORECAST_GROUP, 0x10, at, Y);
    }
}

/// **`Panel_JobGrain` (`0x00413590`)**, 27 call sites. The store, the fertility
/// band, what last season's event and weather did to the store, then either
/// what will be sown (facing Spring) or what is growing and when it comes in,
/// and the two signed rows.
///
/// ```text
/// Ui_DrawCount(grain, 2, 0x130, 0x88)
/// [adv] Eng_DrawString(22, (fertility + 100) / 0x1D, 0x80, 0x98)
/// +0x278 == 0 ? 77/0x18 at (0x40, 0xB0)
///             : Ui_DrawCount(+0x278, 2, 0x40, 0xB0) + 77/0x19 (event 0x87) or 77/0x1A (0x8B)
/// [adv] the weather line on +0x24C
/// g_seasonNext == 1:
///   Ui_DrawCount(+0x230, 2, 0x40, 0xD8)                + 77/1
///   Ui_DrawCount(+0x230 * g_grainYieldPerSack, 2, 0x40, 0xE8) + 77/2
/// otherwise:
///   Ui_DrawCount(season 4 ? crop[2] : +0x2FC, 2, 0x40, 0xD8) + 77/3
///     + Ui_DrawCount(season 2 ? 3 : season 3 ? 2 : 1, 0x42, pen + 0x40, 0xD8)
///   77/0 at (0x40, 0xE8) + Ui_DrawCount(crop[0], 2, pen + 0x40, 0xE8) + 77/4
/// 77/0x1B at (0x40, 0x108); grainEaten == 0 ? Ui_DrawNumber(0, '@', " ", 0x130)
///                                           : Ui_DrawDelta(-grainEaten, 0, " ", " ", 0x128)
/// 77/0x1C at (0x40, 0x118); +0x22C == 0 ? the same zero : Ui_DrawDelta(+0x22C, …)
/// ```
fn grain(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County) {
    let k = &ctx.game.kingdom;
    let advanced = k.options.advanced_farming;
    count(pen, ctx, canvas, c.grain, NOUN_SACK, 0x130, 0x88);
    if advanced {
        // `(fertility + 100) / 0x1D`, C division; `+0x208` is -100…100, so the
        // band is 0…6 and group 22 has exactly seven strings.
        let band = (c.fertility + 100) / 0x1D;
        say(pen, ctx, canvas, FERTILITY_GROUP, band.max(0) as usize, 0x80, 0x98);
    }

    // `+0x278` — what last season's random event did to the store.
    const EVENT_Y: i32 = 0xB0;
    if c.grain_event_change == 0 {
        say(pen, ctx, canvas, FORECAST_GROUP, 0x18, 0x40, EVENT_Y);
    } else {
        let at = count(pen, ctx, canvas, c.grain_event_change, NOUN_SACK, 0x40, EVENT_Y);
        // `county.eventId`, the county's own `+0x1AA`: *Rats* and *Grain
        // found*. Any other id leaves the number with no words after it.
        match c.event_id {
            0x87 => {
                say(pen, ctx, canvas, FORECAST_GROUP, 0x19, at, EVENT_Y);
            }
            0x8B => {
                say(pen, ctx, canvas, FORECAST_GROUP, 0x1A, at, EVENT_Y);
            }
            _ => {}
        }
    }
    if advanced {
        weather_line(pen, ctx, canvas, c.grain_weather_change, NOUN_SACK);
    }

    if k.season_next == 1 {
        let at = count(pen, ctx, canvas, c.grain_sown_expected, NOUN_SACK, 0x40, 0xD8);
        say(pen, ctx, canvas, FORECAST_GROUP, 1, at, 0xD8);
        // `+0x230 * g_grainYieldPerSack`, an i32 product in the original.
        let yielded = c.grain_sown_expected.wrapping_mul(k.tables.grain.yield_per_sack);
        let at = count(pen, ctx, canvas, yielded, NOUN_SACK, 0x40, 0xE8);
        say(pen, ctx, canvas, FORECAST_GROUP, 2, at, 0xE8);
    } else {
        let seasons = match k.season_next {
            2 => 3,
            3 => 2,
            _ => 1,
        };
        let crop = if k.season_next == 4 { c.crop[2] } else { c.grain_grown_expected };
        let at = count(pen, ctx, canvas, crop, NOUN_SACK, 0x40, 0xD8);
        let at = say(pen, ctx, canvas, FORECAST_GROUP, 3, at, 0xD8);
        count(pen, ctx, canvas, seasons, NOUN_SEASON, at, 0xD8);
        let at = say(pen, ctx, canvas, FORECAST_GROUP, 0, 0x40, 0xE8);
        let at = count(pen, ctx, canvas, c.crop[0], NOUN_SACK, at, 0xE8);
        say(pen, ctx, canvas, FORECAST_GROUP, 4, at, 0xE8);
    }

    say(pen, ctx, canvas, FORECAST_GROUP, 0x1B, 0x40, 0x108);
    if c.grain_eaten == 0 {
        zero(pen, canvas, 0x130, 0x108);
    } else {
        delta(pen, canvas, c.grain_eaten.wrapping_neg(), 0x128, 0x108);
    }
    say(pen, ctx, canvas, FORECAST_GROUP, 0x1C, 0x40, 0x118);
    if c.grain_change_expected == 0 {
        zero(pen, canvas, 0x130, 0x118);
    } else {
        delta(pen, canvas, c.grain_change_expected, 0x128, 0x118);
    }
}

/// **`Panel_JobCattle` (`0x00413B30`)**, 29 call sites.
///
/// ```text
/// Ui_DrawCount(herd, 4, 0x130, 0x88)
/// herdCrowding 10 / 20 / 30 / else -> 77/8 / 9 / 10 / 11 at (0x40, 0xA0)
/// +0x274 == 0 ? 77/0x13 at (0x40, 0xB0)
///             : Ui_DrawCount(+0x274, 4, 0x40, 0xB0) + 77/0x14 (0x88) 0x15 (0x89) 0x16 (0x8C) 0x17 (0x8D)
/// [adv] the weather line on +0x270
/// 77/5 at (0x40, 0xD8), Ui_DrawCount(births expected, 4, 0x130, 0xD8)
/// 77/6 at (0x40, 0xE8), Ui_DrawCount(deaths expected, 4, 0x130, 0xE8)
/// 77/7 at (0x40, 0xF8), births == deaths ? zero at 0x130 : Ui_DrawDelta(births - deaths, …, 0x128)
/// 77/0x1B at (0x40, 0x108), herdEaten == 0 ? zero : Ui_DrawDelta(-herdEaten, …)
/// 77/0x1C at (0x40, 0x118), +0x258 == 0 ? zero : Ui_DrawDelta(+0x258, …)
/// ```
fn cattle(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County) {
    let advanced = ctx.game.kingdom.options.advanced_farming;
    count(pen, ctx, canvas, c.herd, NOUN_ANIMAL, 0x130, 0x88);
    let band = match c.herd_crowding {
        10 => 8,
        20 => 9,
        30 => 10,
        _ => 11,
    };
    say(pen, ctx, canvas, FORECAST_GROUP, band, 0x40, 0xA0);

    // `+0x274` — what last season's random event did to the herd.
    const EVENT_Y: i32 = 0xB0;
    if c.herd_event_change == 0 {
        say(pen, ctx, canvas, FORECAST_GROUP, 0x13, 0x40, EVENT_Y);
    } else {
        let at = count(pen, ctx, canvas, c.herd_event_change, NOUN_ANIMAL, 0x40, EVENT_Y);
        let word = match c.event_id {
            0x88 => Some(0x14),
            0x89 => Some(0x15),
            0x8C => Some(0x16),
            0x8D => Some(0x17),
            _ => None,
        };
        if let Some(index) = word {
            say(pen, ctx, canvas, FORECAST_GROUP, index, at, EVENT_Y);
        }
    }
    if advanced {
        weather_line(pen, ctx, canvas, c.herd_weather_change, NOUN_ANIMAL);
    }

    say(pen, ctx, canvas, FORECAST_GROUP, 5, 0x40, 0xD8);
    count(pen, ctx, canvas, c.herd_births_expected, NOUN_ANIMAL, 0x130, 0xD8);
    say(pen, ctx, canvas, FORECAST_GROUP, 6, 0x40, 0xE8);
    count(pen, ctx, canvas, c.herd_deaths_expected, NOUN_ANIMAL, 0x130, 0xE8);

    say(pen, ctx, canvas, FORECAST_GROUP, 7, 0x40, 0xF8);
    if c.herd_births_expected == c.herd_deaths_expected {
        zero(pen, canvas, 0x130, 0xF8);
    } else {
        let farming = c.herd_births_expected.wrapping_sub(c.herd_deaths_expected);
        delta(pen, canvas, farming, 0x128, 0xF8);
    }
    say(pen, ctx, canvas, FORECAST_GROUP, 0x1B, 0x40, 0x108);
    if c.herd_eaten == 0 {
        zero(pen, canvas, 0x130, 0x108);
    } else {
        delta(pen, canvas, c.herd_eaten.wrapping_neg(), 0x128, 0x108);
    }
    say(pen, ctx, canvas, FORECAST_GROUP, 0x1C, 0x40, 0x118);
    if c.herd_change_expected == 0 {
        zero(pen, canvas, 0x130, 0x118);
    } else {
        delta(pen, canvas, c.herd_change_expected, 0x128, 0x118);
    }
}

/// **`Panel_JobReclamation` (`0x004140F3`)**, 6 call sites.
///
/// ```text
/// g_penAdvance = 0;
/// Ui_DrawNumber((byte) +0x204, '@', "", 0x40, 0xB8)
/// Eng_DrawString(77, +0x204 == 1 ? 0xC : 0xD, pen + 0x40, 0xB8)
/// +0x214 == 0 ? 77/0xF at (0x40, 200)
///             : 77/0xE at (0x40, 200) + Ui_DrawCount(+0x214, 0x42, pen + 0x40, 200)
/// ```
fn reclamation(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County) {
    // `(uint)(byte)field_0x204` — the byte, whatever our wider field holds.
    let fields = i32::from(c.fields_reclaiming as u8);
    let at = pen.number_in(Face::Body, canvas, 0x40, 0xB8, fields, '@', "", BODY_INK);
    say(pen, ctx, canvas, FORECAST_GROUP, if fields == 1 { 0x0C } else { 0x0D }, at, 0xB8);
    if c.reclaim_seasons_to_next == 0 {
        say(pen, ctx, canvas, FORECAST_GROUP, 0x0F, 0x40, 200);
    } else {
        let at = say(pen, ctx, canvas, FORECAST_GROUP, 0x0E, 0x40, 200);
        count(pen, ctx, canvas, c.reclaim_seasons_to_next, NOUN_SEASON, at, 200);
    }
}

/// **`Panel_JobIndustry` (`0x00412E6B`)**, 12 call sites, for iron, stone and
/// wood. One-based jobs 5, 6 and 7 map to industry records 1, 3 and 0 and to
/// group 8 nouns `0xC`, `0xE` and `0x10` — *Tonne*, three times over.
///
/// ```text
/// [adv] Eng_DrawString(76, 0, 0x40, 0xA0) + Ui_DrawNumber((char) +0x294 + r*0x18, '@', "%", pen + 0x40, 0xA0)
/// Ui_DrawCount(+0x2A8 + r*0x18, noun, 0x40, 0xB0) + 76/1
/// iron:  Ui_DrawCount(+0x284, noun, 0x40, 0xC0) + 76/2
/// stone: Ui_DrawCount(+0x28C, noun, 0x40, 0xC0) + 76/3
/// wood:  Ui_DrawCount(+0x280, noun, 0x40, 0xC0) + 76/2
///        Ui_DrawCount(+0x288, noun, 0x40, 0xD0) + 76/3
/// ```
///
/// `+0x280 … +0x28C` are `Industry_LabourEstimate`'s four figures, which
/// [`l2_kingdom::industry::panel_figures`] recomputes (C164): the blacksmiths'
/// wood and iron, and the castle's wood and stone still owed. The painter's
/// `g_jobPanelJob == 8` arm, which looks up a weapon's noun, is unreachable —
/// `Panel_JobDetail` calls this for 5, 6 and 7 only.
fn industry(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County, job: usize) {
    let k = &ctx.game.kingdom;
    let (record, noun) = match job {
        JOB_IRON_MINING => (Commodity::Iron, NOUN_IRON),
        JOB_STONE_QUARRYING => (Commodity::Stone, NOUN_STONE),
        _ => (Commodity::Wood, NOUN_WOOD),
    };
    let r = &c.industry[record.index()];
    if k.options.advanced_farming {
        let at = say(pen, ctx, canvas, INDUSTRY_GROUP, 0, 0x40, 0xA0);
        // `(int)*(char *)` — the efficiency byte, signed.
        let efficiency = i32::from(r.efficiency as u8 as i8);
        pen.number_in(Face::Body, canvas, at, 0xA0, efficiency, '@', "%", BODY_INK);
    }
    let at = count(pen, ctx, canvas, r.next_season, noun, 0x40, 0xB0);
    say(pen, ctx, canvas, INDUSTRY_GROUP, 1, at, 0xB0);

    let [smiths_wood, smiths_iron, castle_wood, castle_stone] =
        l2_kingdom::industry::panel_figures(&k.tables, c);
    match job {
        JOB_IRON_MINING => {
            let at = count(pen, ctx, canvas, smiths_iron, noun, 0x40, 0xC0);
            say(pen, ctx, canvas, INDUSTRY_GROUP, 2, at, 0xC0);
        }
        JOB_STONE_QUARRYING => {
            let at = count(pen, ctx, canvas, castle_stone, noun, 0x40, 0xC0);
            say(pen, ctx, canvas, INDUSTRY_GROUP, 3, at, 0xC0);
        }
        _ => {
            let at = count(pen, ctx, canvas, smiths_wood, noun, 0x40, 0xC0);
            say(pen, ctx, canvas, INDUSTRY_GROUP, 2, at, 0xC0);
            let at = count(pen, ctx, canvas, castle_wood, noun, 0x40, 0xD0);
            say(pen, ctx, canvas, INDUSTRY_GROUP, 3, at, 0xD0);
        }
    }
}

/// **`Castle_DrawStatusBlock(county, x, y, row)` (`0x0041DEDB`)**, 13 call
/// sites, at a caller-chosen origin. The job popup passes `(-0x20, 0x40, 0)`,
/// so its column is `x + 0x60 = 0x40` and its first line `y + 0x68 = 0xA8`.
///
/// ```text
/// 71/0x10 at (x+0x60, r+0x68) + Ui_DrawNumber(DAT_004D8A24[type], ' ', " %", pen + x+0x60)
/// 71/0xB  at (x+0x60, r+0x78) + Ui_DrawNumber(DAT_004D8A0C[type], ' ', " ",  pen + x+0x60) + 71/0xC
/// Ui_DrawCount(+0x1D0, 0xE,  x+0x60, r+0x90) + 71/6
/// Ui_DrawCount(+0x1D4, 0x10, x+0x60, r+0xA0) + 71/7
/// +0x1A6 == 0 ? 71/0x11 at (x+0x60, r+0xB0)
///             : Ui_DrawCount(labour[3], 0x26, x+0x60, r+0xB0) + 71/8 + Ui_DrawCount(+0x1A6, 0x42, pen + x+0x60)
/// ```
///
/// where `r = row * 0x10 + y`. `+0x1A6` is `Castle_BuildEstimate`'s byte, which
/// [`l2_kingdom::industry::castle_seasons_left`] recomputes (C164); the byte's
/// width is kept, so an estimate past 255 wraps as the original's store does.
#[allow(clippy::too_many_arguments)]
pub fn castle_status_block(
    pen: &Pen,
    ctx: &Ctx,
    canvas: &mut Canvas,
    c: &County,
    x: i32,
    y: i32,
    row: i32,
) {
    let t = &ctx.game.kingdom.tables;
    let left = x + 0x60;
    let top = row * 0x10 + y;

    let at = say(pen, ctx, canvas, CASTLE_GROUP, 0x10, left, top + 0x68);
    let bonus = castle_word(t, CASTLE_TAX_BONUS_BASE + usize::from(c.castle_type));
    pen.number_in(Face::Body, canvas, at, top + 0x68, bonus, ' ', " %", BODY_INK);

    let at = say(pen, ctx, canvas, CASTLE_GROUP, 0x0B, left, top + 0x78);
    let barracks = castle_word(t, CASTLE_BARRACKS_BASE + usize::from(c.castle_type));
    let at = pen.number_in(Face::Body, canvas, at, top + 0x78, barracks, ' ', " ", BODY_INK);
    say(pen, ctx, canvas, CASTLE_GROUP, 0x0C, at, top + 0x78);

    let at = count(pen, ctx, canvas, c.castle_stone_owed, NOUN_STONE, left, top + 0x90);
    say(pen, ctx, canvas, CASTLE_GROUP, 6, at, top + 0x90);
    let at = count(pen, ctx, canvas, c.castle_wood_owed, NOUN_WOOD, left, top + 0xA0);
    say(pen, ctx, canvas, CASTLE_GROUP, 7, at, top + 0xA0);

    let seasons = l2_kingdom::industry::castle_seasons_left(t, c) as u8;
    if seasons == 0 {
        say(pen, ctx, canvas, CASTLE_GROUP, 0x11, left, top + 0xB0);
    } else {
        let builders = c.labour[JOB_CASTLE_BUILDING];
        let at = count(pen, ctx, canvas, builders, NOUN_BUILDER, left, top + 0xB0);
        let at = say(pen, ctx, canvas, CASTLE_GROUP, 8, at, top + 0xB0);
        count(pen, ctx, canvas, i32::from(seasons), NOUN_SEASON, at, top + 0xB0);
    }
}

/// **The castle tables as the original addresses them: one run of 28 words
/// from `0x004D89E8`.** `CASTLE_WORKFORCE` is ten of them (to `0x004D8A10`),
/// then `g_castleGarrisonCap` six, the tax bonuses six (`0x004D8A28`) and the
/// free archers six (`0x004D8A40`).
///
/// `Castle_DrawStatusBlock` indexes two of them by `castleType` from a base one
/// word low — `&DAT_004D8A0C + type * 4` and `&DAT_004D8A24 + type * 4` — which
/// is right for types 1…5 and **reads the neighbouring table at type 0**: the
/// barracks line then says `CASTLE_WORKFORCE[4].1`, **2500**, and the tax line
/// the garrison table's trailing zero. Both words read out of the shipped exe
/// at those addresses (`c4 09 00 00`, `00 00 00 00`). `[V]` on the bytes; that a
/// player building a first castle sees *"Barracks for 2500 troops."* is `[I]`
/// — the reading of the painter, not observed.
fn castle_word(t: &Tables, index: usize) -> i32 {
    let c = &t.castle;
    let mut run = [0i32; 28];
    for (i, &(a, b)) in c.workforce.iter().enumerate() {
        run[i * 2] = a;
        run[i * 2 + 1] = b;
    }
    run[10..16].copy_from_slice(&c.garrison_cap);
    run[16..22].copy_from_slice(&c.tax_bonus_pct);
    run[22..28].copy_from_slice(&c.free_archers);
    run.get(index).copied().unwrap_or(0)
}

/// `&DAT_004D8A0C`, as a word of [`castle_word`]'s run.
const CASTLE_BARRACKS_BASE: usize = (0x004D_8A0C - 0x004D_89E8) / 4;
/// `&DAT_004D8A24`, the same.
const CASTLE_TAX_BONUS_BASE: usize = (0x004D_8A24 - 0x004D_89E8) / 4;

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

    /// **`Castle_DrawStatusBlock`'s two tables, as the painter indexes them.**
    /// Every expected number is a literal: the words at `0x004D8A0C + type*4`
    /// and `0x004D8A24 + type*4` read out of the shipped `Lords2.exe` —
    /// `c4 09 00 00 96 00 00 00` and `00 00 00 00 32 00 00 00` — and the rest of
    /// each row from `tools/oracle/kingdom.ps1`'s confirmed layout. Nothing here
    /// is computed from [`castle_word`]'s own offsets.
    #[test]
    fn the_castle_block_reads_the_neighbouring_word_at_castle_type_zero() {
        let t = Tables::DEFAULT;
        let barracks: Vec<i32> =
            (0..=5).map(|ty| castle_word(&t, CASTLE_BARRACKS_BASE + ty)).collect();
        assert_eq!(barracks, [2500, 150, 200, 200, 400, 600], "&DAT_004D8A0C + type * 4");
        let bonus: Vec<i32> =
            (0..=5).map(|ty| castle_word(&t, CASTLE_TAX_BONUS_BASE + ty)).collect();
        assert_eq!(bonus, [0, 50, 75, 100, 125, 150], "&DAT_004D8A24 + type * 4");
        assert_eq!(CASTLE_BARRACKS_BASE, 9);
        assert_eq!(CASTLE_TAX_BONUS_BASE, 15);
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
