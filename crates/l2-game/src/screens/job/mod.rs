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
//! `DAT_005533F4` remembers — so this is a screen the machine pushes
//!.
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
//! module draws both: the small window for eight jobs and [`blacksmith`] for the
//! ninth.
//!
//! # The weapon choice, which is the only control on any of the nine
//!
//! A player: *"I can't choose what type of weapon my blacksmiths are making."*
//!
//! `Screen_HandleInput` (`0x004BA9C8`) hit-tests six hotspots over the smithy
//! picture and only when the job is the blacksmith —
//! `Hotspot_Test(0, 0x18, &DAT_004DCA10, 6)` — all six dispatching to
//! `FUN_0043A950` and thence to `FUN_0043A997(g_selectedCounty, id)`, which is
//! [`l2_kingdom::Kingdom::set_weapon_type`]. [`WEAPON_HOTSPOTS`] is the table and
//! [`HOTSPOT_ORIGIN`] the two offsets; **the offsets are half the fact**, because
//! `0x18` is the picture's own y and reading the rects as screen coordinates puts
//! every weapon 24 pixels high.
//!
//! It is dispatched from `Screen_HandleInput` and not from `Screen_FrameInput`,
//! so an enumeration of the dispatcher alone scored this zero without
//! it ever appearing as a miss — `docs/decisions.md` C61's denominator note.
//!
//! And the page says so in words, from the player's own file:
//! **`Panel_JobBlacksmith` is `L2.eng` group 75's only consumer in the whole
//! binary**, and index 0 is *"Click on a weapon to change production."* A group
//! with one consumer *is* that screen's vocabulary (`CLAUDE.md` rule 6); this is
//! the sentence that tells a player the picture is a control at all.
//!
//! **The two `Ui_OkButton` call sites are an if/else, not two buttons.** Worth
//! saying because `Ui_OkButton` (`0x0040D1BC`) stashes only the last call's
//! position,
//! here only one ever runs per frame, and the mechanical count of 2 is a count
//! of *call sites*.
//!
//! # The five bodies, and the words they are made of
//!
//! The five body painters below are the original's, call for call. Each draws
//! **absolute** screen coordinates, and each
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
//! | 7 blacksmith | [`blacksmith`] = `Panel_JobBlacksmith` (`0x00413155`) | 21 | **75**, 74, 76, 8 |
//!
//! **Group 77 has six consumers, 76 two and 71 four**, so none of them is this
//! popup's alone; what makes them its vocabulary is that the popup draws them
//! and nothing of ours drew them before. **Group 75 is the exception and has
//! exactly one**, [`blacksmith`], which makes it that page's specification
//! `docs/formats/eng.md` §5.
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
//! add the saved value back**,
//! places its own noun from its own start and never counts the pen twice.
//! Every [`Pen`] method returns the absolute x of the next glyph, which is that
//! sum already. **[V]**, both bodies read.
//!
//! # What is still not drawn here
//!
//! * **`Gfx_MarkAllDirty`**, `Panel_JobBlacksmith`'s last statement. Our canvas
//!   is repainted whole.
//! * **`Castle_DrawStatusBlock`'s other caller**, `TileInfo_DrawCastle`
//!   (`0x0041DA2F`), which draws it at `(8, 0x30, row)` for a castle under
//!   construction on the player's own tile. `screens/info.rs`'s layout ladder
//!   has no castle arm to call it from — it returns row `0x0A` for nothing.
//!   **Not this module's** — it is `info.rs`'s, and it is left alone here.

mod screen;
pub use screen::*;
mod common;
pub use common::*;
mod blacksmith;
pub use blacksmith::*;
mod bodies;
pub use bodies::*;
mod castle;
pub use castle::*;

use l2_kingdom::county::County;
use l2_kingdom::tables::{
    Commodity, Tables, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_NAMES, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
};
use l2_view::chrome::system;
use l2_view::Canvas;

/// `Ui_DrawBox(0x30, 0x60, …)` — the window's origin and its width in cells.
const BOX_X: i32 = 48;
const BOX_Y: i32 = 96;
const BOX_COLS: i32 = 25;

/// `DAT_004D29A0`, the per-job height in 16-pixel cells, indexed by the
/// original's 1-based job number. Slot 0 here is job 1.
const BOX_ROWS: [i32; JOB_COUNT] = [13, 13, 9, 11, 9, 9, 9, 9, 9];

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

/// **The forge fire's clock**, which is `Tick_Pulses`' 80 ms pulse — the same
/// one the armoury's torches run on, so the divider chain is
/// [`super::armoury`]'s and not a second copy. See [`super::armoury::Anim`] for
/// why a 20 ms gate on a 16 ms tick is 32 ms and not 20.
#[derive(Debug, Default)]
struct Forge {
    acc_ms: u32,
    div: u8,
    /// `DAT_004E59BC`, 0…10.
    frame: usize,
}

impl Forge {
    /// One fixed tick; `true` when the fire moved.
    fn tick(&mut self) -> bool {
        self.acc_ms += super::armoury::TICK_MS;
        if self.acc_ms < super::armoury::PULSE_MS {
            return false;
        }
        self.acc_ms = 0;
        self.div += 1;
        if self.div < super::armoury::PULSE80_DIVIDER {
            return false;
        }
        self.div = 0;
        self.frame = (self.frame + 1) % FORGE_FRAMES;
        true
    }
}

/// The three colours `Panel_JobDetail` passes to `Ui_DrawCount`, as literal
/// palette indices.
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
/// is tested first,
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
    /// The blacksmith page's fire. Ticks on every job and is only drawn on
/// job 7,
    /// one `g_jobPanelJob`.
    forge: Forge,
    redraw: bool,
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
/// `0, 13, 13, 9, 11, 9, 9, 9, 9, 9` for jobs 0…9, one-based.
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

