#![allow(unused_imports)]
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

/// **`Panel_Ration`'s number arguments, which are not the same as everybody
/// else's.** Every `Ui_DrawNumberRight` and `Ui_DrawNumber` call in this
/// painter passes `' '` as the lead and a pointer into the run of zero bytes at
/// `0x004D3E00` … `0x004D3E1B` as the suffix, so **the suffix is the empty
/// string** — six call sites, six distinct addresses, every one of them a NUL.
/// Fifteen of the image's other `Ui_DrawNumberRight` sites pass a one-space
/// suffix instead, which is what this panel was borrowing.
///
/// It matters because the suffix is inside what gets **measured**: the centring
/// tail `FUN_004025D7` is `x + max(0, (width − FUN_004014F0(buffer)) / 2)` and
/// `FUN_004014F0` charges four pixels for a space at any position. A trailing
/// space we invent therefore moves the digits two pixels left, on all five
/// columns at once. `docs/decisions.md` C140. **[V]**
const RATION_LEAD: char = ' ';
const RATION_SUFFIX: &str = "";

impl Panel {
    /// The panel's window in pixels — the rectangle `Ui_DrawBox` covers.
    pub fn window(self) -> Rect {
        let (x, y, cols, rows) = self.box_cells();
        Rect::new(x, y, cols * 16, rows * 16)
    }

    fn box_cells(self) -> (i32, i32, i32, i32) {
        self.box_cells_for(false)
    }

    /// The same with *Armies Eat* known, which is the one option that changes a
    /// panel's shape: `Panel_Ration` opens
    /// `rows = g_optArmiesEat == 1 ? 2 : 0; Ui_DrawBox(0x80, 0x60, 0x12, rows + 0xF)`,
    /// making room for the foraging line at y = 336. Everything else ignores it.
    fn box_cells_for(self, armies_eat: bool) -> (i32, i32, i32, i32) {
        match self {
            Panel::Population => POPULATION_BOX,
            Panel::Happiness => HAPPINESS_BOX,
            Panel::Tax => TAX_BOX,
            Panel::Ration => {
                let (x, y, cols, rows) = RATION_BOX;
                (x, y, cols, if armies_eat { rows + 2 } else { rows })
            }
        }
    }

    /// The graph area, for the two panels that have one.
    pub fn graph_rect(self) -> Option<Rect> {
        matches!(self, Panel::Population | Panel::Happiness).then_some(GRAPH)
    }

    /// `Ui_OkButton(x, y, 0)` — the 24 × 24 picture in the panel's own corner,
    /// hit-tests on a left
    /// release.
    ///
/// `System.pl8` frame `0x33` decodes to a cursor
    /// arrow pointing into a small black hole: a close button whose artwork is
    /// the instruction. It is a real target *and* the right button closes the
    /// panel from anywhere (`docs/screens-county.md` §2.6), so the game offers
    /// two ways out and so do we. §2.7; the name `OK` is ours and is kept.
    pub fn ok_button(self) -> Rect {
        self.ok_button_for(false)
    }

    /// The same, with the ration panel's *Armies Eat* height applied — its
    /// corner is `Ui_OkButton(0x184, rows * 0x10 + 0x44, 0)` and `rows` is the
    /// box's own, so turning the option on moves the button 32 pixels down.
    pub fn ok_button_for(self, armies_eat: bool) -> Rect {
        let (x, y) = match self {
            // Ui_OkButton(0x1B4, 0x184, 0)
            Panel::Population => (436, 388),
            // Ui_OkButton(0x1B4, 0x194, 0)
            Panel::Happiness => (436, 404),
            // Ui_OkButton(0x174, 0x104, 0)
            Panel::Tax => (372, 260),
            // Ui_OkButton(0x184, rows * 0x10 + 0x44, 0)
            Panel::Ration => {
                let (_, _, _, rows) = self.box_cells_for(armies_eat);
                (388, rows * 0x10 + 0x44)
            }
        };
        Rect::new(x, y, system::OK_DIM, system::OK_DIM)
    }

    /// The **up** arrow: record 0 of `g_taxWidgets` (`0x004DD790`) or of
    /// `g_rationWidgets` (`0x004DD7C0`). It is the left of the pair, which is
    /// the original's arrangement and not a transcription slip.
    pub fn increase_button(self) -> Option<Rect> {
        match self {
            Panel::Tax => Some(Rect::new(192, 162, system::ARROW, system::ARROW)),
            Panel::Ration => Some(Rect::new(344, 130, system::ARROW, system::ARROW)),
            _ => None,
        }
    }

    /// The **down** arrow: record 1 of the same table, 24 pixels to its right.
    pub fn decrease_button(self) -> Option<Rect> {
        match self {
            Panel::Tax => Some(Rect::new(224, 162, system::ARROW, system::ARROW)),
            Panel::Ration => Some(Rect::new(368, 130, system::ARROW, system::ARROW)),
            _ => None,
        }
    }

    /// Which quadrant of the county strip opens this panel.
    pub fn strip_hotspot(self) -> Rect {
        let (x, w) = match self {
            Panel::Population | Panel::Tax => (HOT_X0, HOT_X_LEFT_END - HOT_X0),
            Panel::Happiness | Panel::Ration => (HOT_X_RIGHT_START, HOT_X1 - HOT_X_RIGHT_START),
        };
        let (y, h) = match self {
            Panel::Population | Panel::Happiness => (HOT_Y0, HOT_Y_SPLIT - HOT_Y0),
            Panel::Tax | Panel::Ration => (HOT_Y_SPLIT, HOT_Y1 - HOT_Y_SPLIT),
        };
        Rect::new(x, y, w, h)
    }
}

// ------------------------------------------------- the produce rows, and
//                                                    the popup they open
//
// `FUN_0040FEC1` (`0x0040FEC1`) lays the 162 x 128 plate at y = 302 out into two
// columns, `CountyStrip_Draw` picks the two row pitches, and
// `CountyStrip_JobClick` (`0x00438E3B`) turns a press into a job popup. The
// three are one fact and live together here.

// ------------------------------------------- the sheet frames the panels draw
//
// `Misc_cty.pl8` in campaign mode — `Pen::misc_frame`, which is
// `Pl8_DrawFrame(g_miscCtySheet, …)`.

/// `Ui_DrawHappinessDelta`'s face, 20 × 18, and the three plain happiness
/// numbers on `Panel_Happiness` each get one too.
const FRAME_FACE: usize = 0x17;
/// `Panel_Ration`: an 8 × 18 glyph at (144, 282), left of *"Fed"*.
const FRAME_FED_MARK: usize = 0x18;
/// `Panel_Ration`: the grain sack, 36 × 27. Drawn twice — at the slider's left
/// end and over the first Fed/Eaten column.
const FRAME_GRAIN: usize = 0x21;
/// `Panel_Ration`: cattle, 37 × 24. Drawn twice, the same way.
const FRAME_CATTLE: usize = 0x26;
/// `Panel_Ration`: the third food column's icon, 23 × 19. `Misc_cty` frame
/// `0x2A` has not been decoded to a picture here; `docs/screens-county.md` §5.4
/// calls it sheep, which is **[D]** and not checked.
const FRAME_THIRD_FOOD: usize = 0x2A;
/// `Panel_Tax`: the vignette at (320, 160).
const FRAME_TAX_VIGNETTE: usize = 0x3E;
/// `Ui_DrawHappinessDelta` puts its closing bracket at `pen + 0x14`, which is
/// exactly [`FRAME_FACE`]'s width — so the face's box is 20 pixels wide whether
/// or not the sheet is there, and the bracket lands in the same place.
const FACE_W: i32 = 0x14;

// ---------------------------------------------------------------- the screen

impl CountyScreen {
    /// Opens on the panel the strip quadrant that was clicked names, which is
    /// the only way the original opens any of them ([`panel_at`]).
    pub fn new(county: u8, panel: Panel) -> CountyScreen {
        CountyScreen { county, panel, slider_held: false, press: Press::new() }
    }

    /// **The panel's own widget table**, with the kind byte its records carry.
    ///
    /// `g_taxWidgets` (`0x004DD790`) and `g_rationWidgets` (`0x004DD7C0`) are
    /// two 24-byte records each and **both are `Widget_Test` kind 4** — read out
    /// of `+0x0F` of all four records. That is auto-repeat: the press steps
    /// once, and holding steps again on [`crate::press::REPEAT_GATE`]'s ramp,
    /// 240 ms later and then faster until it is running flat out at 1.44 s.
    ///
    /// `docs/arms.json` filed this arm as `left-press` and it was wrong — the
    /// record named the two tables and nobody read their kind byte. A player
    /// reported the consequence: *"Holding on a button doesn't seem to make it
    /// go up faster. I recall you could click an up arrow and after a few
    /// seconds the number would go up fast."*
    ///
    /// Index 0 is **up** and index 1 is **down**, which is the tables' own order
    /// and puts the up arrow to the *left* of the pair.
    ///
    /// The `arm!` is `Screen_HandleInput`'s widget tables' marker.
    /// they are answered with, in one token.
    fn arrows(&self) -> Vec<Widget> {
        [self.panel.increase_button(), self.panel.decrease_button()]
            .into_iter()
            .flatten()
            .map(|r| Widget::new(r, crate::arm!("0x004BA9C8/tax-and-ration-arrows", Repeat)))
            .collect()
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    pub fn panel(&self) -> Panel {
        self.panel
    }

    pub fn open(&mut self, panel: Panel) {
        self.panel = panel;
    }

    fn panel_index(&self) -> usize {
        PANELS.iter().position(|&p| p == self.panel).unwrap_or(0)
    }

    /// Move the open panel's own value by `step`. Silently refused for a county
    /// the player does not hold, and there is nothing to move on the two panels
    /// that only report.
    fn adjust(&self, ctx: &mut Ctx, step: i32) {
        let id = self.county;
        let Some(c) = ctx.game.kingdom.counties.get(id as usize) else { return };
        match self.panel {
            Panel::Tax => {
                let next = (c.tax_rate + step).clamp(0, MAX_TAX_RATE);
                ctx.game.set_tax_rate(id, next);
            }
            Panel::Ration => {
                let next = (c.ration_wanted + step).clamp(0, RATION_LEVEL_COUNT as i32 - 1);
                ctx.game.set_ration(id, next);
            }
            Panel::Population | Panel::Happiness => {}
        }
    }

    /// **`Ration_SliderClick` (`0x0043A379`) — and it is a drag, not a click.**
    ///
    /// ```c
    /// if (counties[sel].owner != g_localPlayer) return 0;
    /// if (g_mouseLeftReleased) return 0;                    /* the release does nothing */
    /// if (!g_mouseLeftDoubleClick && !(g_mouseLeftDown && g_mouseInputChanged)) return 0;
    /// ```
    ///
    /// So it fires **while the button is held and the pointer has moved**, and
    /// the release is explicitly ignored. `held` is that condition; the caller
    /// passes it for both a press and a drag, which is what makes the thumb
    /// follow the cursor.
    ///
    /// The two gestures do not end the same way, and `g_uiHotspotArg` is what
    /// tells them apart: **1 for a jump on the track, 0 for an arrow.** A track
    /// jump that changes nothing springs back; an arrow keeps walking. The rule
    /// is [`l2_kingdom::Kingdom::set_ration_split`] and the flag is `sweep`
    /// there.
    ///
    /// One more thing the original does that reads oddly and is deliberate:
    /// the arrows only step on a **press** (`g_mouseLeftPressed ||
    /// g_mouseLeftDoubleClick`), so holding the button down on an arrow and
    /// wiggling does not repeat — but holding it on the **track** does, because
    /// the track branch reads `mouseX` every frame.
    fn split_click(&self, ctx: &mut Ctx, x: i32, y: i32, pressed: bool) -> bool {
        if self.panel != Panel::Ration {
            return false;
        }
        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return false };
        let current = c.ration_split;
        // The track is read on every frame of the drag; the arrows step only on
        // the press that started it.
        let (next, sweep) = if split_track().contains(x, y) {
            (x - SLIDER_TRACK_X, true)
        } else if split_down_button().contains(x, y) {
            if !pressed {
                return true;
            }
            (current - 1, false)
        } else if split_up_button().contains(x, y) {
            if !pressed {
                return true;
            }
            (current + 1, false)
        } else {
            return false;
        };
        let next = next.clamp(0, MAX_RATION_SPLIT);
        // `if (rationSplit == local_10) return 1;` — the arm consumes the input
        // and does not re-run the food pass for a value that has not moved.
        if next != current {
            ctx.game.set_ration_split(self.county, next, sweep);
        }
        true
    }
}

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
            // `[V]`, and `crates/l2-game/src/screens/army.rs` is the same line
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

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
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

impl CountyScreen {
    /// The pen every panel draws through: `Fntl2_14.pl8` and `Fntl2_22.pl8`,
    /// embossed with [`font::SHADOW`] — which is what `Ui_DrawText` picks for
    /// every screen id but `0x1C` and `0x1F`.
    fn pen<'a>(&self, ctx: &'a Ctx) -> Pen<'a> {
        Pen {
            assets: &ctx.assets.shell,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        }
    }

    fn draw_panel(&self, ctx: &Ctx, canvas: &mut Canvas) {
        let pen = self.pen(ctx);
        let ink = &ctx.assets.ink;
        let armies_eat = ctx.game.kingdom.options.armies_eat;
        let (bx, by, cols, rows) = self.panel.box_cells_for(armies_eat);
        // `Ui_DrawBox(x, y, cols, rows)`, border set 0. `Pen::window` falls back
        // to a flat plate of ours where the install has no `Panels.pl8`.
        pen.window(canvas, bx, by, cols, rows, 0);

        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        match self.panel {
            Panel::Population => {
                // `Eng_DrawString(73, 0, 0x14, 0x38, heading)` and then the
                // county's name — group 100 — immediately after it, which this
                // panel used to omit entirely.
                let x = pen.heading(canvas, 20, 56, &line_text(ctx, g73::TITLE), font::TEXT);
                pen.heading(canvas, x + 2, 56, &county_name(ctx, self.county), font::TEXT);

                self.draw_graph_stub(ctx, canvas, "POPULATION");

                // `Eng_DrawString(73, 8, 0xB0, 0xF0)` — and the two things that
                // follow it, the graph's peak and 73/9 *"people."*, need the
                // history array to have a peak at all. See the module docs.
                pen.body(canvas, 176, 240, &line_text(ctx, g73::GREATEST), font::TEXT);

                heading_row(&pen, canvas, 266, &line_text(ctx, g73::LAST), c.pop_last);
                let rows: [(i32, Line, i32); 3] = [
                    (298, g73::BIRTHS, c.births),
                    (314, g73::DEATHS, -c.deaths),
                    (330, g73::ARMY, c.army),
                ];
                for (y, label, value) in rows {
                    delta_row(&pen, canvas, y, &line_text(ctx, label), value);
                }
                if c.emigrants == 0 {
                    pen.body(canvas, LABEL_X, 346, &line_text(ctx, g73::NO_EMIGRATION), font::TEXT);
                } else {
                    // The original draws the destination county's **name**
                    // between the label and the number: `Eng_DrawString(100,
                    // slot * 20 + emigrantDestination, pen + 0x30, 0x15A)`.
                    let x =
                        pen.body(canvas, LABEL_X, 346, &line_text(ctx, g73::EMIGRANTS), font::TEXT);
                    pen.body(canvas, x, 346, &county_name(ctx, c.emigrant_destination), font::TEXT);
                    delta_value(&pen, canvas, 346, -c.emigrants);
                }
                if c.immigrants == 0 {
                    let s = line_text(ctx, g73::NO_IMMIGRATION);
                    pen.body(canvas, LABEL_X, 362, &s, font::TEXT);
                } else {
                    delta_row(&pen, canvas, 362, &line_text(ctx, g73::IMMIGRANTS), c.immigrants);
                }
                heading_row(&pen, canvas, 386, &line_text(ctx, g73::THIS), c.population);
            }
            Panel::Happiness => {
                let x = pen.heading(canvas, 20, 56, &line_text(ctx, g85::TITLE), font::TEXT);
                pen.heading(canvas, x + 2, 56, &county_name(ctx, self.county), font::TEXT);

                self.draw_graph_stub(ctx, canvas, "HAPPINESS");

                // 85/8 at (176, 241), the number after it, then the face.
                let x = pen.body(canvas, 176, 241, &line_text(ctx, g85::AVERAGE), font::TEXT);
                let x = pen.body(canvas, x, 241, &c.happiness_avg.to_string(), font::TEXT);
                pen.misc_frame(canvas, FRAME_FACE, x, 239);

                heading_row_face(&pen, canvas, 268, &line_text(ctx, g85::LAST), c.happiness_last);
                let rows: [(i32, Line, i32); 6] = [
                    (298, g85::FROM_TAXES, c.shown_tax),
                    (314, g85::FROM_RATION, c.shown_ration),
                    (330, g85::FROM_HEALTH, c.shown_health),
                    (346, g85::FROM_ARMY, c.shown_army),
                    (362, g85::FROM_ALE, c.shown_ale),
                    (378, g85::FROM_EVENTS, c.shown_events),
                ];
                for (y, label, value) in rows {
                    delta_row(&pen, canvas, y, &line_text(ctx, label), value);
                }
                heading_row_face(&pen, canvas, 402, &line_text(ctx, g85::THIS), c.happiness);
            }
            Panel::Tax => {
                // **No title.** `L2.eng` 86/0 *"Tax in"* is drawn by nothing —
                // see [`g86::TITLE_NEVER_DRAWN`] and the module docs. A "TAX IN"
                // heading of ours used to stand here at (96, 152).
                pen.misc_frame(canvas, FRAME_TAX_VIGNETTE, 320, 160);
                pen.body(canvas, 96, 168, &line_text(ctx, g86::RATE), font::TEXT);
                // `Ui_DrawNumber(taxRate, ' ', "%", 0x100, 0xA8, body)` — the
                // lead is a real space and the suffix is the per-cent sign.
                pen.body(canvas, 256, 168, &format!(" {}%", c.tax_rate), font::TEXT);

                let x = pen.body(canvas, 96, 200, &line_text(ctx, g86::PEOPLE_PAY), font::TEXT);
                // `Ui_DrawCount(taxShown, 0, …)` — the number and then group 8's
                // *"Crown."* / *"Crowns."*. We used to write "CROWNS" ourselves.
                pen.count(canvas, x, 200, c.tax_shown, CROWN_NOUN, font::TEXT);

                pen.body(canvas, 96, 232, &line_text(ctx, g86::THIS_COUNTY), font::TEXT);
                let empire = ctx
                    .game
                    .kingdom
                    .realms
                    .get(c.owner as usize)
                    .map_or(0i32, |r| r.tax_hap_empire as i32);
                happiness_delta(&pen, canvas, 240, 232, c.d_hap_tax_local + empire);
                pen.body(canvas, 96, 256, &line_text(ctx, g86::OTHER_COUNTIES), font::TEXT);
                happiness_delta(&pen, canvas, 240, 256, c.tax_hap_other);
            }
            Panel::Ration => {
                // `Ui_DrawCentred(87, 0, 0x80, 0x68, 0x120, heading, 0x3F)`.
                pen.heading_centred(canvas, 128, 104, 288, &line_text(ctx, g87::TITLE), font::TEXT);
                pen.body(canvas, 144, 136, &line_text(ctx, g87::WANTED), font::TEXT);
                pen.body(canvas, 240, 136, &ration_label(ctx, c.ration_wanted), font::TEXT);

                pen.body(canvas, 144, 161, &line_text(ctx, g87::ACHIEVED), font::TEXT);
                // **Red when it differs from wanted** — the painter's own
                // `local_8`, `0x3F` or `0xF9`.
                let colour = if c.ration_achieved == c.ration_wanted {
                    font::TEXT
                } else {
                    font::HIGHLIGHT
                };
                pen.body(canvas, 240, 161, &ration_label(ctx, c.ration_achieved), colour);
                happiness_delta(&pen, canvas, 340, 161, c.d_hap_ration);

                pen.body(canvas, 144, 186, &line_text(ctx, g87::HEALTH), font::TEXT);
                pen.body(canvas, 240, 186, &health_label(ctx, c.health_band), font::TEXT);
                happiness_delta(&pen, canvas, 340, 186, c.d_hap_health);

                // The slider's two flanking icons, then the three food columns'
                // icons, then the mark left of "Fed". Six `Pl8_DrawFrame`s the
                // panel used to draw none of.
                pen.misc_frame(canvas, FRAME_GRAIN, 144, 220);
                pen.misc_frame(canvas, FRAME_CATTLE, 370, 220);
                for (frame, x) in
                    [(FRAME_GRAIN, 224), (FRAME_CATTLE, 284), (FRAME_THIRD_FOOD, 344)]
                {
                    pen.misc_frame(canvas, frame, x, 256);
                }
                pen.misc_frame(canvas, FRAME_FED_MARK, 144, 282);

                self.draw_split_slider(ctx, canvas, c.ration_split);

                // **The Fed row, which used to say "NOT SIMULATED".** Its three
                // fields are county `+0x170`, `+0x174` and `+0x16C`, and this
                // module had them down as *"not in l2-kingdom at all, so there is
                // nothing to put here"*. They are `grainEaten * foodPerSack`,
                // `herdEaten * foodPerHead` and `herd * dairyPerHead` — products
                // of three fields that were always there. The absence was
                // recorded honestly, in a comment, beside the words on the
// screen, and read as a conclusion.
                //
                // **Three numbers, not two, and the third explains the panel**:
                // the standing herd feeds five people a head without being
                // slaughtered.
                // nothing at all and its slider has nothing to divide. That
                // number is what says so.
                //
                // **It is the same condition as the slider's inertness, read a
                // second way**, and the two were found an hour apart as separate
                // complaints — *"the slider is inoperable"* and *"sorely
                // missing: 'All your people are fed by dairy'"*.
                // such sentence in `L2.eng`; this figure reaching the county's
                // population **is** the game saying it, and it is why the two
                // reports have one fix. `docs/rules.md` §4.
                //
// Drawn through [`Pen`], and centred
// `Ui_DrawNumberRight` **centres**
                // (C110's neighbour).
                //
                // **These five are the only `Ui_DrawNumberRight` sites in the
                // image whose suffix is empty.** `Panel_Ration` passes
                // `&DAT_004D3E04`, `…08`, `…0C`, `…10` and `…14`, and all five
                // of those addresses hold a NUL — a run of zero bytes in `.data`
                // ending where `"villani1.pl8"` begins. The other fifteen live
                // sites pass a single space. Since `FUN_004025D7` centres the
                // *whole* buffer and `FUN_004014F0` charges four pixels for a
                // trailing space without trimming it, the `" {value} "` this
                // used to build put every one of these five two pixels left.
                // `docs/decisions.md` C140. **[V]**
                let (by_grain, by_meat, by_dairy) =
                    l2_kingdom::ration::people_fed(&ctx.game.kingdom.tables, c);
                // Both labels before any number, which is `Panel_Ration`'s own
                // order: `Eng_DrawString(87, 5, 0xA0, 0x11E)`,
                // `Eng_DrawString(87, 4, 0x90, 0x134)`, then the five columns.
                pen.body(canvas, 160, 0x11E, &line_text(ctx, g87::FED), font::TEXT);
                pen.body(canvas, 144, 0x134, &line_text(ctx, g87::EATEN), font::TEXT);
                {
                    // `Ui_DrawNumberRight(value, ' ', "", x, y, 0x40, body, 0x3F)`,
                    // five times, in the painter's order.
                    let mut col = |x: i32, y: i32, v: i32| {
                        let (lead, suffix) = (RATION_LEAD, RATION_SUFFIX);
                        pen.number_centred(canvas, x, y, FOOD_COL_W, v, lead, suffix, font::TEXT);
                    };
                    col(FOOD_COL_X[0], 0x134, c.grain_eaten);
                    col(FOOD_COL_X[0], 0x11E, by_grain);
                    col(FOOD_COL_X[1], 0x134, c.herd_eaten);
                    col(FOOD_COL_X[1], 0x11E, by_meat);
                    col(FOOD_COL_X[2], 0x11E, by_dairy);
                }

                if armies_eat {
                    // `g_penAdvance = 0;`
                    // `Ui_DrawNumber(+0x19C + +0x198, ' ', "", 0x88, 0x150, body, 0x3F);`
                    // `Eng_DrawString(87, 8, g_penAdvance + 0x88, 0x150, body, 0x3F);`
                    //
                    // **The suffix in that transcription was right.
                    // below it did not use it.** `Ui_DrawText` ends with
                    // `g_penAdvance += 4` — which is [`crate::shell::TRAILING`],
                    // and which [`Pen::body`] already adds — so the `" {men} "`
                    // this built charged the gap *twice* and put the group 87
                    // label four pixels right of where `Eng_DrawString` lands
                    // it. Same mistake as the five centred columns above, on a
                    // routine with no width argument, where it displaces the
// *next* string.
                    // `docs/decisions.md` C140. **[V]**
                    let men = c.friendly_troops + c.enemy_troops;
                    let s = format!("{RATION_LEAD}{men}{RATION_SUFFIX}");
                    let x = pen.body(canvas, 136, 336, &s, font::TEXT);
                    pen.body(canvas, x, 336, &line_text(ctx, g87::FORAGING), font::TEXT);
                }
            }
        }

        self.draw_buttons(ctx, canvas, armies_eat);
    }

    /// The corner picture, and the two arrows the two order panels have.
    fn draw_buttons(&self, ctx: &Ctx, canvas: &mut Canvas, armies_eat: bool) {
        let pen = self.pen(ctx);
        let ink = &ctx.assets.ink;
        let ok = self.panel.ok_button_for(armies_eat);
        pen.ok_button(canvas, ok.x, ok.y, 0);
        let live = ctx.game.is_players(self.county);
        // `Widget_Draw` (`0x0040CFD2`) picks record `+0x04` **plus one** while
        // the press timer at `+0x0D` runs, so each arrow has a pressed picture —
        // frames `0x16` and `0x18`. `Press::is_pressed` is that timer, and the
        // index is the table's: 0 up, 1 down.
        for (i, (rect, frame, label)) in [
            (self.panel.increase_button(), system::ARROW_UP, "+"),
            (self.panel.decrease_button(), system::ARROW_DOWN, "-"),
        ]
        .into_iter()
        .enumerate()
        {
            let Some(r) = rect else { continue };
            let frame = if self.press.is_pressed(i) { frame + 1 } else { frame };
            if !pen.system_frame(canvas, frame, r.x, r.y) {
                widget::button(canvas, ink, r, label, live);
            }
        }
    }

    /// `Panel_RationSlider` (`0x00411FDE`), which is drawn from
/// `Screen_DrawWidgets`'s `0x19` arm:
    ///
    /// ```text
    ///   Ui_DrawBoxInterior(0xDC, 0xD8, 8, 2)         (220, 216) 128 x 32
    ///   Pl8_DrawFrame(System, 0x4A, 200,   0xDC)     left cap   (200, 220)
    ///   Pl8_DrawFrame(System, 0x4B, 0x144, 0xDC)     right cap  (324, 220)
    ///   FUN_00403A8F(0xE0, 0xE5, 0x143, 0xE5, 0x10)  line  (224,229)-(323,229)
    ///   FUN_0040437D(0xE0, 0xE6, 100, 4, 0x3F)       fill  (224,230) 100 x 4
    ///   FUN_00403A8F(0xE0, 0xEA, 0x143, 0xEA, 0x1F)  line  (224,234)-(323,234)
    ///   Pl8_DrawFrame(System, 0x4C, 0xDC + split, 0xD8)  the knob at y 216
    /// ```
    ///
    /// **The caps sit at y = 220 and the knob at y = 216**.
    /// to draw both at 216 — which also put all three hit boxes four pixels
    /// high, because `Ration_SliderClick` (`0x0043A379`) tests
    /// `(200, 0xDC, 24, 24)`, `(0x145, 0xDC, 24, 24)` and
    /// `(0xE0, 0xDC, 0x66, 24)`, every one of them at **0xDC = 220**.
    fn draw_split_slider(&self, ctx: &Ctx, canvas: &mut Canvas, split: i32) {
        let pen = self.pen(ctx);
        let ink = &ctx.assets.ink;
        // The parchment well the whole control sits in, which we drew none of.
        pen.box_interior(canvas, 220, SLIDER_KNOB_Y, 8, 2);
        // The three track lines. The colours are the original's literals.
        canvas.fill_rect(SLIDER_TRACK_X, 229, SLIDER_TRACK_W, 1, TRACK_TOP);
        canvas.fill_rect(SLIDER_TRACK_X, 230, SLIDER_TRACK_W, 4, TRACK_FILL);
        canvas.fill_rect(SLIDER_TRACK_X, 234, SLIDER_TRACK_W, 1, TRACK_BOTTOM);
        let caps = {
            let l = pen.system_frame(
                canvas,
                system::SLIDER_CAP_LEFT,
                SLIDER_CAP_LEFT_X,
                SLIDER_CTRL_Y,
            );
            let r = pen.system_frame(
                canvas,
                system::SLIDER_CAP_RIGHT,
                SLIDER_CAP_RIGHT_X,
                SLIDER_CTRL_Y,
            );
            l && r
        };
        if !caps {
            widget::button(canvas, ink, split_down_button(), "<", false);
            widget::button(canvas, ink, split_up_button(), ">", false);
        }
        let knob_x = SLIDER_KNOB_ORIGIN + split.clamp(0, MAX_RATION_SPLIT);
        if !pen.system_frame(canvas, system::SLIDER_KNOB, knob_x, SLIDER_KNOB_Y) {
            canvas.fill_rect(knob_x, SLIDER_KNOB_Y, system::SLIDER_KNOB_W, 32, ink.highlight);
        }
    }

    /// **A stub, and it looks like one.** [`Ui_HistoryGraph`'s whole
    /// picture](self#what-is-still-ours-and-says-so) needs `g_countyHistory`
    /// and `Graphs.pl8`; `l2-kingdom` has no history array and `l2-view` does
    /// not load that sheet, so there is nothing to plot.
    ///
    /// The recess itself is real — `Ui_DrawInsetRect(0x20, 0x54, 0x192, 0x9B)`
    /// is the graph's own first call and is drawn here at its own coordinates.
    /// The two lines inside it are ours and say so.
    fn draw_graph_stub(&self, ctx: &Ctx, canvas: &mut Canvas, what: &str) {
        let ink = &ctx.assets.ink;
        let Some(r) = self.panel.graph_rect() else { return };
        canvas.fill_rect(r.x, r.y, r.w, r.h, ink.background);
        self.pen(ctx).inset(canvas, r);
        let mid = r.y + r.h / 2;
        // OURS, both of them: a diagnostic, in our own 5 x 7 font, so that a
        // screenshot cannot be mistaken for the original's graph. Debug overlay
        // only — the empty recess is the honest picture without it.
        if ctx.game.prefs.debug_overlay {
            text::draw_centred(canvas, r.centre_x(), mid - 10, &format!("{what} HISTORY"), ink.dim);
            text::draw_centred(canvas, r.centre_x(), mid + 2, "NOT SIMULATED", ink.bad);
        }
    }
}

/// One `L2.eng` string, from the install if it has one and from our own
/// transcription if it does not.
///
/// # `Ui_DrawNumberRight` centres, and its name is a false claim
///
/// Recorded here because it is what this panel's five numbers depend on and two
/// branches found it independently within a day. It is `Ui_NumberToBuffer`
/// followed by `FUN_004025D7`, which is
/// `Ui_DrawText(s, x + max(0, (width - textWidth) / 2), y, …)` — and
/// `Ui_DrawCentred` calls **the same function**. One alignment, two names, one
/// of them true. This module right-aligned these columns because the symbol said
/// *right*.
///
/// **The symbol's entry is `[V]` and its comment says the opposite of its
/// body**, so the verification carried the error: a wrong name with a wrong
/// verified comment is believed twice — once for the name and once for the tier
/// — with nothing left to contradict it. `[V]` records that somebody read it,
/// not that somebody read it correctly. Twenty call sites in the original
/// inherit it and **eighteen beyond this panel are unaudited.**
fn line_text(ctx: &Ctx, l: Line) -> String {
    eng(ctx, l.group, l.index, l.ours)
}

fn ration_name(level: i32) -> &'static str {
    RATION_NAMES
        .get(level.clamp(0, RATION_LEVEL_COUNT as i32 - 1) as usize)
        .copied()
        .unwrap_or("?")
}

/// `Eng_DrawString(20, healthBand, …)` — the five health words, with
/// [`HEALTH_BAND_NAMES`] as the fallback.
fn health_label(ctx: &Ctx, band: u8) -> String {
    let ours = HEALTH_BAND_NAMES.get(band as usize).copied().unwrap_or("?");
    eng(ctx, GROUP_HEALTH_BANDS, band as usize, ours)
}

/// A `Ui_DrawNumber(v, '@', "", 0x150, y, heading)` row: the label in the left
/// column and the value **left-aligned from x = 336**, both in the 22-pixel
/// font, which is what the three plain rows on the two graph panels are.
fn heading_row(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.heading(canvas, LABEL_X, y, label, font::TEXT);
    pen.heading(canvas, VALUE_LEFT, y, &value.to_string(), font::TEXT);
}

/// The same with `Pl8_DrawFrame(Misc_cty, 0x17, pen + 0x150, y + 3)` after it —
/// the happiness panel's *Last season* and *This Season* rows both carry the
/// face, three pixels below the text's own line.
fn heading_row_face(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.heading(canvas, LABEL_X, y, label, font::TEXT);
    let x = pen.heading(canvas, VALUE_LEFT, y, &value.to_string(), font::TEXT);
    pen.misc_frame(canvas, FRAME_FACE, x, y + 3);
}

/// `Ui_DrawDelta(value, 0, "", "", 0x150, y, body, 0x3F, 0xF9)` — the label and
/// the value.
fn delta_row(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.body(canvas, LABEL_X, y, label, font::TEXT);
    delta_value(pen, canvas, y, value);
}

/// The value half on its own, for the emigration row, which puts a county name
/// between the label and the number.
///
/// Three things it is easy to get wrong:
/// **a zero draws nothing at all** — mode 0 with `value == 0` returns before the
/// first `Ui_DrawText` — the digits are **left-aligned from `x`**
/// right-anchored to it, and the empty prefix still advances the pen by
/// [`TRAILING`], so they start four pixels right of it.
fn delta_value(pen: &Pen, canvas: &mut Canvas, y: i32, value: i32) {
    if value == 0 {
        return;
    }
    let colour = if value < 0 { font::HIGHLIGHT } else { font::TEXT };
    let sign = if value < 0 { '-' } else { '+' };
    pen.body(canvas, VALUE_LEFT + TRAILING, y, &format!("{sign}{}", value.abs()), colour);
}

/// `Ui_DrawHappinessDelta` (`0x0041AC95`), whole:
///
/// ```text
///   Ui_DrawText("(", x, y, font, 0x3F)              pen = w("(") + 4
///   pen -= 4
///   Ui_DrawDelta(value, 1, "", "", x + pen, y, font, colourPos, colourNeg)
///   Pl8_DrawFrame(g_miscCtySheet, 0x17, x + pen, y - 2)
///   Ui_DrawText(")", x + pen + 0x14, y, font, 0x3F)
/// ```
///
/// So it is `( ±n ☺ )`, the bracket sits at `x`, and the closing bracket is
/// [`FACE_W`] = 20 pixels past the face — which is exactly the face's own width
/// in `Misc_cty.pl8`, so the gap is the picture's and not a guess. **Mode 1
/// never suppresses a zero**, unlike the panel rows: a zero draws `0` with the
/// blank sign column.
fn happiness_delta(pen: &Pen, canvas: &mut Canvas, x: i32, y: i32, value: i32) {
    let after_bracket = pen.body(canvas, x, y, "(", font::TEXT) - TRAILING;
    let colour = if value < 0 { font::HIGHLIGHT } else { font::TEXT };
    let sign = match value.signum() {
        -1 => "-",
        1 => "+",
        _ => "",
    };
    let face_x =
        pen.body(canvas, after_bracket + TRAILING, y, &format!("{sign}{}", value.abs()), colour);
    pen.misc_frame(canvas, FRAME_FACE, face_x, y - 2);
    pen.body(canvas, face_x + FACE_W, y, ")", font::TEXT);
}

