#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::*;
use super::panel::*;
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

impl Screen for CountyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::County(self.county, self.panel)
    }

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("County {}", self.county)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // ```c
        // if (FUN_0043292D() == 0 && FUN_00432967() == 0 &&      /* modes, sidebar  */
        //     CountyStrip_Click() == 0 && FUN_00439122() == 0 && /* strip, split    */
        //     CountyStrip_JobClick() == 0 && FUN_00439079() == 0) {
        //   if (!rightReleased) { if (Ui_OkButtonClicked()) { g_screenId = 0; } }
        //   else                                            { g_screenId = 0; }
        // }
        // ```
        //
        // [`Transition::Pass`] is the whole of it, and it is the same mechanism
        // the village already used for the same six guards
        // (`docs/decisions.md` C59). The strip quadrants go down with the rest,
        // so the map's own `CountyStrip_Click` answers and its `Push` lands at
        // the map's depth — which is `g_screenId = 0x14` and not a second panel
        // stacked on the first.
        if crate::screens::belongs_to_the_right_column(event) {
            return Transition::Pass;
        }
        if matches!(event, Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft) {
            let fired = self.press.event(&self.arrows(), event);
            debug_assert!(fired.is_none(), "no arrow is a release widget");
        }
        match event {
            Event::Release { x, y } => {
                self.slider_held = false;
                // `Ui_OkButtonClicked` (`0x0040E7E4`) — the 24 x 24 corner
                // picture the panel's own `Ui_OkButton` call stashed, **on the
                // release**: its first statement is
                // `if (g_mouseLeftReleased == 0) return 0;`. Ours tested it on
                // the press.
                //
                // arm: 0x0040E7E4/panel-corner-closes left-release
                if self
                    .panel
                    .ok_button_for(ctx.game.kingdom.options.armies_eat)
                    .contains(x, y)
                {
                    return Transition::Pop;
                }
                return Transition::Stay;
            }
            Event::Pointer { x, y } if self.slider_held => {
                self.split_click(ctx, x, y, false);
                return Transition::Stay;
            }
            // game says so in its own words: `Screen_SliderBox` prints `L2.eng`
            // group 12 index 0, *"Click Right to Exit"*, under its caption.
            //
            // arm: 0x0042FF10/panel-right-closes right-release
            Event::RightClick { .. } => return Transition::Pop,
            // `docs/decisions.md` C61 found nine of.
            //
            // arm: ours/county-panel-keyboard key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => return Transition::Pop,
            Event::KeyDown(Key::Up) => {
                self.panel = PANELS[(self.panel_index() + PANELS.len() - 1) % PANELS.len()];
            }
            Event::KeyDown(Key::Down) => {
                self.panel = PANELS[(self.panel_index() + 1) % PANELS.len()];
            }
            Event::KeyDown(Key::Left) => self.adjust(ctx, -1),
            Event::KeyDown(Key::Right) => self.adjust(ctx, 1),
            Event::Click { x, y } => {
                // `Ui_OkButtonClicked` (`0x0040E7E4`) — the 24 × 24 corner
                // picture the panel's own `Ui_OkButton` call stashed.
                //
                // `Ration_SliderClick` (`0x0043A379`) — `0x19`'s own extra
                // guard.
                //
                // arm: 0x0043A379/ration-split-slider left-press
                if self.split_click(ctx, x, y, true) {
                    self.slider_held = true;
                    return Transition::Stay;
                }
                // `Screen_HandleInput`'s widget tables: `g_taxWidgets`
                // (`0x004DD790`) and `g_rationWidgets` (`0x004DD7C0`), two
                // records each, up then down.
                if let Some(i) = self.press.event(&self.arrows(), event) {
                    self.adjust(ctx, if i == 0 { 1 } else { -1 });
                }
            }
            // **It does not start a drag, and this arm did.** `slider_held` is
            // `g_mouseLeftDown`, and `App_WndProc` (`0x004B29BE`) answers
            // `0x203` with `DAT_004EADA1 |= 1` and nothing else — only `0x201`
            // sets the down bit. So `Ration_SliderClick`'s track branch,
            // `g_mouseLeftDown && g_mouseInputChanged`, is false for as long as
            // the second press is held, and moving the pointer after a double
            // click on the track moves nothing. Ours latched the drag on and
            // the thumb then followed the cursor with the button up.
            //
            // `[V]`, and `crates/l2-game/src/screens/army/mod.rs` is the same line
            // on the levy slider, which already passed `down: false` here.
            Event::DoubleClick { x, y } => {
                if self.split_click(ctx, x, y, true) {
                    return Transition::Stay;
                }
                if let Some(i) = self.press.event(&self.arrows(), event) {
                    self.adjust(ctx, if i == 0 { 1 } else { -1 });
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        for i in self.press.tick() {
            self.adjust(ctx, if i == 0 { 1 } else { -1 });
        }
        Transition::Stay
    }

    /// **These are four windows over the campaign map**, which is what this
    /// module's own header has said all along — *"each is a floating
    /// `Ui_DrawBox` window over whatever was underneath"* — while `draw` went
    /// on clearing the screen and painting a flat ground where the map should
    /// be. The village's correction (`docs/decisions.md` C22) is the same
    /// mistake one screen along, and this is the other half of it.
    fn is_overlay(&self) -> bool {
        true
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {

        draw_strip(ctx, canvas, self.county, Some(self.panel));
        self.draw_panel(ctx, canvas);

    }
}

