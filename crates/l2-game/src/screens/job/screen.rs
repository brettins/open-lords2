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
use crate::screen::{Ctx, Dirty, Screen, ScreenId, Transition};
use crate::shell::{count_noun, font, Face, Pen};


impl JobScreen {
    /// `Panel_JobDetail`'s (`0x00412B33`) third statement is
    /// `Gfx_MarkAllDirty()`, before the icon sheet is even read — the panel
    /// opens with the whole frame marked whichever job it is.
    pub fn new(county: u8, job: usize) -> JobScreen {
        let mut dirty = Dirty::EMPTY;
        dirty.mark_all();
        JobScreen { county, job: job.min(JOB_COUNT - 1), forge: Forge::default(), dirty }
    }

    pub fn take_dirty_rect(&mut self) -> Option<(i32, i32, i32, i32)> {
        self.dirty.take()
    }

    pub fn job(&self) -> usize {
        self.job
    }

    pub fn window(job: usize) -> Rect {
        Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS[job.min(JOB_COUNT - 1)] * 16)
    }

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

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // arm: 0x0042FF10/minimap-under-the-job-popup left-press
            Event::Click { x, y } if l2_view::chrome::minimap_hit_area().contains(x, y) => {
                Transition::Pass
            }
            // `Screen_FrameInput`'s `0x0F` arm: the corner picture **or** a right release
            // closes the popup, and it returns to whichever screen opened it —
            // the village when `DAT_005533F4` is zero, the campaign map's
            // sidebar otherwise. `Transition::Pop` is both, because the stack
            // remembers what our `g_screenId` cannot.
            //
            // arm: 0x0042FF10/job-popup-closes right-release
            Event::RightClick { .. } => Transition::Pop,
            // arm: ours/job-popup-keyboard key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
            Event::Click { x, y } if JobScreen::ok_button(self.job).contains(x, y) => {
                Transition::Pop
            }
            // `Screen_HandleInput`'s (`0x004BA9C8`) `0x0F` arm:
            //
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
            // arm: 0x004BA9C8/blacksmith-weapon-choice left-press
            Event::Click { x, y } if self.job == BLACKSMITH => {
                match weapon_at(x, y) {
                    Some(w) => {
                        ctx.game.kingdom.set_weapon_type(self.county as usize, w);
                        // `FUN_0043A997` repaints the page through
                        // `Panel_JobBlacksmith` (`0x00413155`), whose last
                        // statement is `Gfx_MarkAllDirty()`.
                        self.dirty.mark_all();
                        Transition::Stay
                    }
                    None => Transition::Stay,
                }
            }
            _ => Transition::Stay,
        }
    }

    /// **`Screen_DrawWidgets`'s (`0x004BA26E`) `0x0F` arm is one guard deep**:
    ///
    /// `if (g_jobPanelJob == 8) FUN_00413526()`. The fire steps on the
    /// blacksmith page and nowhere else, so every other job popup stops asking
    /// for frames the moment it has opened. Ours ticked the forge on all nine.
    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        if self.job == BLACKSMITH && self.forge.tick() {
            // `FUN_00413526`'s own last statement:
            self.dirty.mark_sprite(FORGE_AT.0, FORGE_AT.1, 8, 8, 1);
        }
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        self.dirty.take().is_some()
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
        let face = crate::shell::Face::Body;

        if self.job != BLACKSMITH {
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
            let (one, many) = WORKER_NOUN[self.job];
            let singular = n.abs() == 1;
            let index = WORKER_NOUN_0 + self.job * WORKER_NOUN_STRIDE + usize::from(!singular);
            let ours = if singular { one } else { many };
            let noun = eng(ctx, COUNT_NOUN_GROUP, index, ours);
            pen.count_with_noun(face, canvas, NAME_X, COUNT_Y, i32::from(n), &noun, colour);
        }

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

        let ok = JobScreen::ok_button(self.job);
        pen.ok_button(canvas, ok.x, ok.y, 0);
    }
}

