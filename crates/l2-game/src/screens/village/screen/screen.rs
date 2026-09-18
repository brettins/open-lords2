#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::*;
use l2_kingdom::county::County;
use l2_kingdom::tables::{JOB_IDLE_TOWNSFOLK, JOB_NAMES};
use l2_view::village::{self as vill, ICONS_PER_CLUSTER};
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

impl Screen for VillageScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Village(self.county)
    }

    /// **The village is three `g_screenId`s, not one** — `0x02` idle, `0x05`
    /// the band, `0x06` the carried selection, the table at the top of this
    /// file. The pointer is chosen from the byte (`g_cursorByScreen`,
    /// `0x004E3098`), so the mode has to be askable: the question mark belongs
    /// to `0x02` alone and `0x06` gets the peasant.
    fn mode_screen_id(&self) -> Option<u8> {
        Some(match self.phase {
            Phase::Idle => 0x02,
            Phase::Band => 0x05,
            Phase::Carry => 0x06,
        })
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("Village of county {}", self.county)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // ```c
        // if (Minimap_ModeButtonClicked()   ||   /* FUN_0043292d, the four minimap modes */
        //     Sidebar_ButtonClicked()       ||   /* FUN_00432967, the six sidebar buttons */
        //     CountyStrip_Click()           ||
        //     Labour_SplitSliderDrag()      ||   /* FUN_00439122, the farm/industry split */
        //     CountyStrip_JobClick()        ||
        //     FUN_00439079()) goto done;         /* consumed: no village verb runs */
        // /* only now: Ui_OkButtonClicked, Village_BandStart, Village_DoubleClick, ... */
        // ```
        //
        // All six are the **campaign map's** right-hand column: `FUN_0043292d`
        // is `Hotspot_Test(0x262, 0x20, &g_minimapModeButtons, 4)` and
        // `FUN_00432967` is `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)`,
        // and every one of the six hit-tests `x >= 0x1DE` — 478, which is
        // [`campaign::PANEL_X`]. So the rule is exactly *"the column at x >=
        // 478 keeps working"*, and nothing else does: `Map_Click` is **not** in
        // this ladder,
        //
        // **The two drag states do not do this.** The `0x05` (banding) and
        // `0x06` (carrying) arms test no sidebar guard at all — they run
        // `Village_BandRelease` / `Village_Drop` and nothing else — so the
        // sidebar goes dead for the duration of a peasant drag and comes back
        // when it ends. That is [`Phase::Idle`] below, and it is the kind of
        // detail that reads as intermittent to a player and gets "fixed" into
        // uniformity by mistake. C59.
        if self.phase == Phase::Idle && Self::belongs_to_the_sidebar(event) {
            return Transition::Pass;
        }
        match event {
            // ```c
            // /* 0x02, idle */              if (rightReleased) { g_screenId = 0; ... }
            // /* 0x06, carrying peasants */ if (rightReleased) { g_screenId = 0x02;
            //                                    g_villageDragCluster = 0;
            //                                    FUN_004120E0(); FUN_00432893(1); }
            // ```
            //
            // This popped the screen from every phase, which meant a player who
            // picked up peasants and changed his mind lost the village as well
            // as the selection. **A wrong arm,
            // nothing looked broken** — `docs/decisions.md` C76.
            //
            // arm: 0x0042FF10/village-right-leaves right-release
            // arm: 0x0042FF10/carry-right-cancels right-release
            Event::RightClick { .. } => {
                if self.phase == Phase::Carry {
                    self.clear_drag();
                    self.pending_click = None;
                    self.status = "PUT BACK".into();
                    return Transition::Stay;
                }
                if self.phase == Phase::Band {
                    return Transition::Stay;
                }
                return Transition::Pop;
            }
            // arm: ours/village-keyboard key
            Event::KeyDown(Key::Escape) => {
                if self.phase == Phase::Idle {
                    return Transition::Pop;
                }
                self.clear_drag();
                self.pending_click = None;
                self.status = "CANCELLED".into();
            }
            Event::KeyDown(Key::Enter) => return Transition::Pop,
            Event::Pointer { x, y } => {
                self.pointer = (x, y);
                if self.phase == Phase::Band {
                    self.box_select(&*ctx);
                } else if self.phase == Phase::Idle {
                    if let Some((ax, ay)) = self.anchor {
                        if (x - ax).abs() >= vill::DRAG_DEAD_ZONE
                            || (y - ay).abs() >= vill::DRAG_DEAD_ZONE
                        {
                            self.phase = Phase::Band;
                            self.box_select(&*ctx);
                        }
                    }
                }
            }
            // **`Village_DoubleClick` (`0x00439DF0`) is its own input arm**, and
            // it is the only reader of the double-click flag in the whole
            // binary. It sits *between* `Village_BandStart` and
            // `Village_ClickJob` in `Screen_FrameInput`'s screen-`0x02` ladder,
            // which is the order kept here: a drag in progress wins, then the
            // double click, then — only once it has settled — the job popup.
            //
            // arm: 0x00439DF0/double-click-balances double-click
            Event::DoubleClick { x, y } => {
                self.pointer = (x, y);
                // The pending single click is cancelled outright: the original
                // clears `DAT_004E65E8` the moment `DAT_004EABC5` is set, in
                // the poll itself, so the popup never opens behind the move.
                self.pending_click = None;
                if self.phase != Phase::Idle {
                    return Transition::Stay;
                }
                if let Some(cluster) = self.double_click_cluster(&*ctx, x, y) {
                    self.balance(ctx, cluster - 1);
                }
            }
            Event::Click { x, y } => {
                self.pointer = (x, y);
                // arm: 0x004399B0/drop left-press
                if self.phase == Phase::Carry {
                    self.drop_on(ctx, x, y);
                    return Transition::Stay;
                }
                // arm: 0x004393EB/band-start left-press
                if VillageScreen::in_band_area(&*ctx, x, y) {
                    self.anchor = Some((x, y));
                }
            }
            Event::Release { x, y } => {
                self.pointer = (x, y);
                // `Ui_OkButtonClicked` (`0x0040E7E4`) is `if
                // (g_mouseLeftReleased == 0) return 0;` and then a 24 x 24 box
                // at the position `Ui_OkButton` last drew — **the release, not
                // the press**, on all twenty-six of `Screen_FrameInput`'s
                // calls. Ours answered on the press here, which is a player's
                // *"the game waited on mouse-up"* from the other side.
                //
                // arm: 0x0040E7E4/village-corner-closes left-release
                if VillageScreen::ok_button(VillageScreen::top_y(&*ctx)).contains(x, y) {
                    return Transition::Pop;
                }
                match self.phase {
                    // arm: 0x00439541/band-release left-release
                    Phase::Band => {
                        // `FUN_00439541`: something selected means carry it,
                        // nothing means the band was for nothing.
                        if self.drag_cluster != 0 {
                            self.phase = Phase::Carry;
                            self.status = format!("{} PICKED UP", self.drag_count);
                        } else {
                            self.clear_drag();
                        }
                    }
                    Phase::Idle => {
                        // The original arms `DAT_004E65E8` here and opens the
                        // popup only when 300 ms have gone by without a second
                        // press (`Village_ClickJob` reads `DAT_004EABF0`, which
                        // is that timer expiring). See
                        // [`VillageScreen::CLICK_SETTLE_TICKS`]; `update` is
                        // where it lands. Ownership is not tested: you cannot
                        // reach the village of a county you do not hold in the
                        // first place.
                        //
                        // arm: 0x0043A123/click-opens-the-job-popup left-release
                        if self.anchor.take().is_some() {
                            self.pending_click = Some((x, y, Self::CLICK_SETTLE_TICKS));
                        }
                    }
                    Phase::Carry => {}
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        self.animated = self.clock.tick(Self::TICK_MS);
        let Some((x, y, left)) = self.pending_click else { return Transition::Stay };
        if left > 1 {
            self.pending_click = Some((x, y, left - 1));
            return Transition::Stay;
        }
        self.pending_click = None;
        match self.job_under(&*ctx, x, y) {
            Some(job) => Transition::Push(ScreenId::Job(self.county, job)),
            None => Transition::Stay,
        }
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.animated)
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let top = VillageScreen::top_y(ctx);

        let art = ctx.assets.village.as_ref();
        let drew = art.is_some_and(|a| a.draw_scene(canvas, top));
        if !drew {
            canvas.fill_rect(vill::SCENE_X, top, vill::SCENE_W, vill::SCENE_H, ink.panel);
            widget::frame(
                canvas,
                Rect::new(vill::SCENE_X, top, vill::SCENE_W, vill::SCENE_H),
                ink.border,
            );
        }
        if let Some(c) = self.county_ref(ctx) {
            if ctx.game.kingdom.options.advanced_farming {
                art.map(|a| a.draw_tops(canvas, c.weather.index() as usize));
            }
            let has = VillageScreen::resource_flags(c);
            art.map(|a| a.draw_resources(canvas, has, top));
            art.map(|a| a.draw_animations(canvas, has, top, &self.clock));
            self.draw_clusters(ctx, canvas, c, top, drew);
        }

        let ok = VillageScreen::ok_button(top);
        let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| {
            ch.draw_system(canvas, l2_view::chrome::system::OK_ALT, ok.x, ok.y)
        });
        if !drawn {
            widget::button(canvas, ink, ok, "CLOSE", false);
        }

        // **`Village_DrawBand` (`0x00412795`) — the drag selection box, and it
        // is the original's.**
        //
        // C173 put it behind the debug overlay on the strength of *"nothing in
        // the decompiled corpus was found drawing the band"*, and a player
        // found the hole in a minute: *"the drag selection box has disappeared,
        // it was probably a debug thing that you removed with other debug
        // boxes."* It was not. `FUN_00412795` runs on screen `0x05` and nothing
        // else, clamps the box to the band area and calls the rectangle outline
        // at `0x00403CF4` in colour `0x20`. See [`VillageScreen::band_rect`] for
        // the clamp and the colour.
        let debug = ctx.game.prefs.debug_overlay;
        if self.phase == Phase::Band {
            if let Some(r) = self.band_rect(top) {
                let band_ink =
                    if ctx.assets.chrome.is_some() { BAND_INK } else { ink.highlight };
                widget::frame(canvas, r, band_ink);
            }
        }

        let mid = vill::SCENE_X + vill::SCENE_W / 2;
        let caption = match self.phase {
            Phase::Carry => format!("CARRYING {} - CLICK A JOB", self.drag_count),
            _ => format!("VILLAGE OF COUNTY {}", self.county),
        };
        if debug {
            text::draw_centred(canvas, mid, top + 4, &caption, ink.text);
        }
        let mut line = top + vill::SCENE_H - 12;
        let mut say = |canvas: &mut Canvas, s: &str, colour: u8| {
            text::draw_centred(canvas, mid, line, s, colour);
            line -= 12;
        };
        if ctx.assets.village.as_ref().is_none_or(|a| !a.has_grid()) {
            say(canvas, "NO DROP GRID - VILL_GD8.PL8 MISSING", ink.bad);
        }
        if debug && !ctx.game.is_players(self.county) {
            say(canvas, "NOT YOURS", ink.bad);
        }
        if debug && !self.status.is_empty() {
            say(canvas, &self.status, ink.dim);
        }
    }
}

