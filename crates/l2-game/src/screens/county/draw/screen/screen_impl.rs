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

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// A held arrow's repeat stepped the number: `Tax_IncreaseCounty` ends
    /// `Panel_Tax()`. See [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("County {}", self.county)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **The whole right-hand column is live under an open panel**.
// arm says so. `0x14`'s, verbatim:
        //
        // ```c
        // if (FUN_0043292D() == 0 && FUN_00432967() == 0 &&      /* modes, sidebar  */
        //     CountyStrip_Click() == 0 && FUN_00439122() == 0 && /* strip, split    */
        //     CountyStrip_JobClick() == 0 && FUN_00439079() == 0) {
        //   if (!rightReleased) { if (Ui_OkButtonClicked()) { g_screenId = 0; } }
        //   else                                            { g_screenId = 0; }
        // }
        // ```
        //
        // Six guards, every one of them hit-testing `x >= 0x1DE`, and every one
        // of them ahead of the panel's own two ways out. `0x15` and `0x16` are
        // the same six; `0x19` inserts `Ration_SliderClick` after the second,
// so the split *below* is tested here and the column is not.
        //
        // Two things follow. Clicking the strip while a panel is open
// **switches** panel — which is how a player goes
        // from population to tax without a trip via the map — and the five
        // sidebar buttons, the minimap, its four mode icons, the farm/industry
        // slider and the produce rows all keep working with a panel up. None of
        // that was reproduced: this screen tested the four strip quadrants
        // itself and swallowed everything else in the column.
        //
        // [`Transition::Pass`] is the whole of it, and it is the same mechanism
        // the village already used for the same six guards
        // (`docs/decisions.md` C59). The strip quadrants go down with the rest,
        // so the map's own `CountyStrip_Click` answers and its `Push` lands at
        // the map's depth — which is `g_screenId = 0x14` and not a second panel
        // stacked on the first.
        //
        // The six guards themselves are the map screen's and are recorded there;
        // the arm here is that *this screen's ladder runs them first*, which is
        // one predicate shared with the village — so one record, and it lives on
        // [`crate::screens::belongs_to_the_right_column`].
        if crate::screens::belongs_to_the_right_column(event) {
            return Transition::Pass;
        }
        // **The arrows' bookkeeping, for the events that only end the hold.**
        // The table holds nothing of [`crate::press::Kind::Release`], so this can never fire a
        // handler; it is the release that stops the repeat, and the pointer
// walking off the button, which in the original is the hit test
        // failing to match on the next frame. The press itself is answered in
        // the `Event::Click` arm below, where the ladder's order matters.
        if matches!(event, Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft) {
            let fired = self.press.event(&self.arrows(), event);
            debug_assert!(fired.is_none(), "no arrow is a release widget");
        }
        match event {
            // **The slider is dragged.** `Ration_SliderClick` returns 0 on the
            // release and fires on `g_mouseLeftDown && g_mouseInputChanged` —
            // held and moved — so the thumb follows the cursor for as long as
            // the button is down. Ours was reachable only from a press, which
            // is the fourth place our input model differs from the original's
// by *category*.
            //
            // The release ends the drag and does nothing else,
            // first line of the original's ladder says.
            Event::Release { x, y } => {
                self.slider_held = false;
                // `Ui_OkButtonClicked` (`0x0040E7E4`) — the 24 x 24 corner
                // picture the panel's own `Ui_OkButton` call stashed, **on the
                // release**: its first statement is
                // `if (g_mouseLeftReleased == 0) return 0;`. Ours tested it on
                // the press.
// `docs/arms.json` now records a gesture KIND
                // an arm's existence.
                //
                // **The BACK TO MAP button that used to be tested here is gone.**
                // It was ours, it was drawn at (478, 460), and that is the
                // original's **End Turn** strip to the pixel — record 5 of
                // `g_sidebarButtons`.
                // live control of the game's. The column now passes down and
                // the strip ends the turn.
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
            // **`Ui_DrawBox` panels are dismissed by the right button**.
            // game says so in its own words: `Screen_SliderBox` prints `L2.eng`
            // group 12 index 0, *"Click Right to Exit"*, under its caption.
            // `Screen_FrameInput`'s arm for each of `0x14`, `0x15`, `0x16` and
            // `0x19` is the same shape — the strip, the sidebar and the ration
            // slider are tested first, so a click on those *switches* panel
// and only then does a right release set
            // `g_screenId = 0`.
            //
            // It closes from *anywhere*, the strip included: every guard in
            // that chain tests a left press or a left release, so none of them
            // consumes a right one.
            //
            // arm: 0x0042FF10/panel-right-closes right-release
            Event::RightClick { .. } => return Transition::Pop,
            // **Ours, and counted.** The original has no keyboard route out of a
            // panel and none into another one: its only `VK_ESCAPE` handler
            // quits the game, and the four panels are four screen ids with no
            // ordering between them at all. Kept because a keyboard player has
            // nothing else, and recorded in `docs/arms.json` as an invention
            //
            // `docs/decisions.md` C61 found nine of.
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
                // **The BACK TO MAP button that used to be tested here is gone.**
                // It was ours, it was drawn at (478, 460), and that is the
                // original's **End Turn** strip to the pixel — record 5 of
                // `g_sidebarButtons`.
                // live control of the game's, which is the same defect a player
                // reported about the five sidebar icons a fortnight ago. The
                // column now passes down and the strip ends the turn.
                // The strip's four quadrants are in the column and went down
                // with it, so nothing is tested for them here.
                // `Ration_SliderClick` (`0x0043A379`) — `0x19`'s own extra
                // guard.
                // longer than the other three.
                // arm: 0x0043A379/ration-split-slider left-press
                if self.split_click(ctx, x, y, true) {
                    self.slider_held = true;
                    return Transition::Stay;
                }
                // `Screen_HandleInput`'s widget tables: `g_taxWidgets`
                // (`0x004DD790`) and `g_rationWidgets` (`0x004DD7C0`), two
                // records each, up then down.
                // **Kind 4**, so the press steps once and the hold keeps
                // stepping: see [`CountyScreen::arrows`], where the arm is.
                if let Some(i) = self.press.event(&self.arrows(), event) {
                    self.adjust(ctx, if i == 0 { 1 } else { -1 });
                }
            }
            // **A double click is a press to both of this ladder's live tests**,
            // and this screen used to drop it on the floor. Windows sends
            // `WM_LBUTTONDBLCLK` *instead of* the second `WM_LBUTTONDOWN`.
            // fast second press arrives as `g_mouseLeftDoubleClick` and never as
            // `g_mouseLeftPressed`. `Ration_SliderClick` reads the double-click
            // flag in its own guard (see [`CountyScreen::split_click`]), and
            // `Widget_Test`'s kind-4 arm tests `g_mouseLeftPressed ||
            // g_mouseLeftDoubleClick` — so in the original a quick double click
            // on the tax arrow is **two steps and two clicks**, and here it was
            // one and one.
            //
            // Found by `tests/click.rs`, which asserted the click's own guard
// through the machine
            // alone — `tests/gestures.rs` covers the double click at the `Press`
            // level, which is exactly the layer that could not see a screen
            // declining to ask.
            //
            // Not an arm of its own: kind 4 *is* "press or double click", and
            // the arm below the `Click` pattern above already carries that kind.
            //
            // Not reached for a double click in the right column, because
            // [`crate::screens::belongs_to_the_right_column`] matches a press, a
            // release and a pointer and nothing else. That is recorded rather
            // than changed here; the column is the map's.
            //
            // **It does not start a drag, and this arm did.** `slider_held` is
            // `g_mouseLeftDown`, and `App_WndProc` (`0x004B29BE`) answers
            // `0x203` with `DAT_004EADA1 |= 1` and nothing else — only `0x201`
            // sets the down bit. So `Ration_SliderClick`'s track branch,
            // `g_mouseLeftDown && g_mouseInputChanged`, is false for as long as
            // the second press is held, and moving the pointer after a double
            // click on the track moves nothing. Ours latched the drag on and
            // the thumb then followed the cursor with the button up.
            // `crate::input::LeftButton` is the bit; this is one reader of it.
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

    /// `Widget_Test`'s per-frame pass over `g_taxWidgets` / `g_rationWidgets`:
    /// the press timer counts down and the repeat counter walks the ramp.
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

    pub(crate) fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        // **No clear.** The campaign map is the screen underneath on the stack.

        // **The seven column plates are no longer repainted here, and that is a
        // one-line change with a visible consequence.**
        //
        // This used to call `draw_right_column`, which puts `Misc_cty` frames 54
        // … 59 down over the whole column, y 24 … 480 — including the **minimap**
        // at (480, 25), the four mode icons beside it and the End Turn caption,
        // none of which this screen redraws. So *opening a county panel blanked
        // the minimap.* That was harmless while the column was dead under a
        // panel; now that every control in it is live it would be a control a
        // player can click and cannot see.
        //
        // The original does not repaint it either: `Screen_Draw`'s `0x14`,
        // `0x15`, `0x16` and `0x19` arms are `Panel_Population` and its three
        // siblings, and not one of them calls `CountyStrip_Draw` or
// `Minimap_Draw`. The column is still there from the last frame —
        // §3.1's *""
        //
        // The **strip** is still drawn, and only because it is the one part of
        // the column that carries the focus outline (ours) and because the plate
        // it lands on is identical to the one the map screen drew a moment ago.
        draw_strip(ctx, canvas, self.county, Some(self.panel));
        self.draw_panel(ctx, canvas);

        // **The bottom strip is not ours to draw.** It is the campaign map's
        // End Turn button, the map screen is underneath on the stack and draws
        // it, and the column now passes clicks down to it. A BACK TO MAP
        // rectangle of ours used to be painted over it — and hit-tested ahead
        // of it — from right here.
    }
}

