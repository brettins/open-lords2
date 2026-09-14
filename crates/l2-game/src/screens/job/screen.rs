#![allow(unused_imports)]
use super::*;
use super::common::*;
use super::blacksmith_part::*;
use super::bodies::*;
use super::castle::*;
use l2_kingdom::county::County;
use l2_kingdom::tables::{
    Commodity, Tables, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_NAMES, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
};
use l2_view::chrome::system;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{count_noun, font, Face, Pen};


impl JobScreen {
    pub fn new(county: u8, job: usize) -> JobScreen {
        JobScreen {
            county,
            job: job.min(JOB_COUNT - 1),
            forge: Forge::default(),
            redraw: false,
        }
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
/// one of them runs.
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

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // **The minimap is live under this popup**, and it is the one thing
            // from the right-hand column that is: `0x0F`'s arm runs
            // `Ui_OkButtonClicked` and a right-release test and **none** of the
            // six sidebar guards, while `Screen_FrameInput`'s epilogue runs
            // `Minimap_Click` on every screen id but `0x12`.
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
            // **The weapon choice, and the only control on any job popup.**
            // `Screen_HandleInput`'s (`0x004BA9C8`) `0x0F` arm:
            // `if (g_jobPanelJob == 8) Hotspot_Test(0, 0x18, &DAT_004DCA10, 6)`,
            // six kind-1 records dispatching to `FUN_0043A950`:
            //
            // ```c
            // DAT_005530B8 = g_uiHotspotId;
            // if (g_multiplayer == 0) FUN_0043A997(g_selectedCounty, id);
            // else                    Net_SendCommand(0x21, 0);
            // ```
            //
            // The multiplayer arm sends the *command* and lets the handler at
            // `0x00447E5B` run `FUN_0043A997(g_selectedCounty, DAT_005530B8)` on
            // every machine — which is this crate's lockstep model already, so
            // the branch is one call here and the `Kingdom` method is the
            // command. `docs/netcode.md`.
            //
            // It is tested **after** the OK button because the corner picture at
            // (448, 448) is below every hotspot; the original's order is the
            // other way and cannot collide either.
            // arm: 0x004BA9C8/blacksmith-weapon-choice left-press
            Event::Click { x, y } if self.job == BLACKSMITH => {
                match weapon_at(x, y) {
                    Some(w) => {
                        ctx.game.kingdom.set_weapon_type(self.county as usize, w);
                        Transition::Stay
                    }
                    None => Transition::Stay,
                }
            }
            _ => Transition::Stay,
        }
    }

    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        self.redraw |= self.forge.tick();
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.redraw)
    }

    pub(crate) fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let w = JobScreen::window(self.job);
        let face = crate::shell::Face::Body;

        // **`Panel_JobDetail`'s `if (g_jobPanelJob == 8)` is the whole head of
        // the painter, not one line of it.** The blacksmith takes the other
        // branch of every `if`: no window, no icon recess, no group-74 title in
        // the window and no worker count — `Screen_DrawMenuBar()` and
        // `Panel_JobBlacksmith` instead.
        if self.job != BLACKSMITH {
            // `Ui_DrawBox(0x30, 0x60, 0x19, g_jobPanelRows[job])`, border set 0.
            pen.window(canvas, w.x, w.y, BOX_COLS, BOX_ROWS[self.job], 0);

            // `FUN_00403CF4(0x40, 0x68, 0x32, 0x32, 0x3F)` — **four lines in
            // colour 0x3F, no fill** — and then
            // `Sprite_WGenSprite(DAT_004D2974[job], 0x41, 0x69)`, the job's
            // `Iconvill.pl8` picture one pixel inside it.
            for (x, y, w2, h) in [
                (ICON_BOX.x, ICON_BOX.y, ICON_BOX.w, 1),
                (ICON_BOX.x, ICON_BOX.y + ICON_BOX.h - 1, ICON_BOX.w, 1),
                (ICON_BOX.x, ICON_BOX.y, 1, ICON_BOX.h),
                (ICON_BOX.x + ICON_BOX.w - 1, ICON_BOX.y, 1, ICON_BOX.h),
            ] {
                canvas.fill_rect(x, y, w2, h, ICON_BOX_INK);
            }
            if let Some(f) =
                ctx.assets.shell.sheet(ICON_SHEET).and_then(|s| s.frame(JOB_ICON[self.job]))
            {
                canvas.blit(&f, ICON_AT.0, ICON_AT.1);
            }

            // `Eng_DrawString(74, job + 1, 0x80, 0x6A, heading, 0x3F)`.
            let title = eng(ctx, JOB_GROUP, self.job + 1, JOB_NAMES[self.job]);
            pen.heading(canvas, NAME_X, NAME_Y, &title, COUNT_RIGHT);
        }

        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        if self.job != BLACKSMITH {
            let n = c.labour[self.job];
            let colour = match staffing(c, self.job) {
                Staffing::Short => COUNT_SHORT,
                Staffing::Wasted => COUNT_WASTED,
                Staffing::Right => COUNT_RIGHT,
            };
            // `Ui_DrawCount(labour[job].workers, job * 2 + 0x1E, 0x80, 0x88,
            // body, colour)` — the number, then group 8's singular or plural.
            // The original takes the **singular** at ±1 and the plural
            // everywhere else, zero included: *"0 Farmers"*.
            let (one, many) = WORKER_NOUN[self.job];
            let singular = n.abs() == 1;
            let index = WORKER_NOUN_0 + self.job * WORKER_NOUN_STRIDE + usize::from(!singular);
            let ours = if singular { one } else { many };
            // Through `Pen::count_with_noun` for `Ui_DrawCount`'s `'@'` lead and
            // empty suffix. Built by hand as `"{n} "`, the digits sat four
            // pixels left.
            let noun = eng(ctx, COUNT_NOUN_GROUP, index, ours);
            pen.count_with_noun(face, canvas, NAME_X, COUNT_Y, i32::from(n), &noun, colour);
        }

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
            BLACKSMITH => blacksmith(&pen, ctx, canvas, c, self.forge.frame),
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

