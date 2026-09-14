#![allow(unused_imports)]
use super::*;
use super::constants::*;
use super::tests_part::*;
use l2_kingdom::divide::{SplitBasket, SplitInto, SplitRefusal};
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_view::{text, Canvas};
use crate::press::{Press, Widget};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};
use crate::widget;

/// What the screen did before it closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divided {
    None,
    /// The daughter army is on the map, in this slot.
    Split(usize),
    /// The men went home: the county they joined and how many.
    ///
    /// **Nothing on this screen produces it any more.** The disband button was
    /// ours; the original's is record 1 of the information panel's
    /// `g_infoUnitButtons`, and it is [`crate::screens::info`]'s now. The
    /// variant is kept because the outcome is still a thing a caller may want
    /// to read and because deleting it would delete the evidence that a button
    /// was here — `docs/arms.json` `ours/divide-disband-button`.
    Disbanded(u8, i32),
    Refused,
}

/// Screen `0x11` for one army.
pub struct DivideScreen {
    /// `DAT_00553074` — the army being divided.
    unit: usize,
    basket: SplitBasket,
    /// Which row the keyboard is on.
    pub(super) row: usize,
    pub outcome: Divided,
    status: String,
    seeded: bool,
    /// **The held `<` / `>` button**, and the acceleration that goes with it.
    ///
    /// Sixteen of this screen's eighteen widgets are `Widget_Test` **kind 4**
    /// — read out of `g_splitWidgets` (`0x004DD388`) in the player's own
    /// `Lords2.exe`, byte `+0x0F` of each record — which means the original
    /// repeats them while the button is down, on the ramp in
    /// [`crate::press::REPEAT_GATE`]. Ours fired once per click, which is the
    /// half of *"holding on a button doesn't seem to make it go up faster"*
    /// that lives on this screen.
    ///
    /// The index is `row * 2` for a parent button and `row * 2 + 1` for a
    /// daughter one, which is the order `g_splitWidgets` itself is in.
    press: Press,
}

impl DivideScreen {
    pub fn new(unit: usize) -> DivideScreen {
        DivideScreen {
            unit,
            basket: SplitBasket::default(),
            row: 0,
            outcome: Divided::None,
            status: String::new(),
            seeded: false,
            press: Press::new(),
        }
    }

    pub fn unit(&self) -> usize {
        self.unit
    }

    /// **One widget's handler**, whichever way it was reached — a kind-4 press
    /// or repeat, or a kind-5 countdown expiring twenty ticks after the press.
    ///
    /// The index is [`widgets`]'.
    pub(crate) fn fire(&mut self, ctx: &mut Ctx, widget: usize) -> Transition {
        match widget {
            // **`SplitScreen_ToParent` (`0x00437D65`) and
            // `SplitScreen_ToDaughter` (`0x00437E9E`)** — sixteen widgets in
            // eight rows, `g_uiHotspotId` carrying the slot. Row 7 is the
// mercenary band and swaps whole, in the
            // function itself. The four arms are declared on [`widgets`].
            i if i < TICK_INDEX => {
                self.row = i / 2;
                self.move_men(i / 2, i % 2 == 0, CLICK_MEN);
                Transition::Stay
            }
            TICK_INDEX => self.split(ctx),
            // The cross is the same handler reading `g_uiHotspotId == 0`, and
// it lands on `0x04`.
            _ => Transition::Pop,
        }
    }

    pub fn basket(&self) -> &SplitBasket {
        &self.basket
    }

    /// `FUN_004378B3`'s seeding: everyone in the parent's column, the band with
    /// them.
    fn seed(&mut self, ctx: &Ctx) {
        if self.seeded {
            return;
        }
        self.seeded = true;
        let Some(unit) = ctx.game.kingdom.campaign.units.get(self.unit) else {
            self.status = "NO SUCH ARMY".into();
            return;
        };
        self.basket = SplitBasket::seed(unit);
        // The Readme's rule, and message 0x95 = group 149 in the original.
        self.status = if unit.moves_used >= 1 {
            "THIS ARMY HAS ALREADY MARCHED THIS SEASON".into()
        } else {
            format!("{} MEN. MOVE THEM RIGHT TO SPLIT THEM OFF", unit.men)
        };
    }

    fn move_men(&mut self, row: usize, to_daughter: bool, n: i32) {
        if row == MERC_ROW {
            if self.basket.move_mercenaries(to_daughter) {
                self.status = "THE BAND MARCHES WHOLE OR NOT AT ALL".into();
            }
            return;
        }
        let Some(troop) = TroopType::from_index(row) else { return };
        let moved = if to_daughter {
            self.basket.to_daughter(troop, n)
        } else {
            self.basket.to_parent(troop, n)
        };
        if moved > 0 {
            self.status =
                format!("{} / {}", self.basket.parent_total(), self.basket.daughter_total());
        }
    }

    /// `FUN_00437AFB` with no destination county — the plain field split.
    fn split(&mut self, ctx: &mut Ctx) -> Transition {
        match ctx.game.split_army(self.unit, &self.basket, SplitInto::Field) {
            Ok(id) => {
                self.outcome = Divided::Split(id);
                Transition::Pop
            }
            Err(no) => {
                self.outcome = Divided::Refused;
                self.status = match no {
                    SplitRefusal::NotAnArmy => "THAT IS NOT YOUR ARMY".into(),
                    SplitRefusal::AlreadyMoved => "IT HAS ALREADY MARCHED THIS SEASON".into(),
                    SplitRefusal::Empty => "ONE SIDE IS EMPTY - NOTHING TO SPLIT".into(),
                    SplitRefusal::TooFew => "BOTH HALVES NEED 50 MEN".into(),
                    SplitRefusal::GarrisonFull(n) => format!("ONLY {n} WOULD FIT"),
                    SplitRefusal::NowhereToStand => "NOWHERE NEARBY TO STAND".into(),
                };
                Transition::Stay
            }
        }
    }

}

/// One row's pair of arrow records — `System.pl8` frames 27 and 25 at the
/// geometry `g_splitWidgets` gives them. Falls back to our own outline when
/// the sheet is not loaded.
pub(crate) fn arrows(pen: &Pen, canvas: &mut Canvas, row: usize, press: &Press) {
    // `Widget_Draw` adds one to the frame while the press timer at `+0x0D`
    // runs. The index is [`widgets`]`: `row * 2` parent, `+ 1` daughter.
    for (half, (rect, frame)) in
        [(parent_button(row), TO_PARENT_FRAME), (daughter_button(row), TO_DAUGHTER_FRAME)]
            .into_iter()
            .enumerate()
    {
        let frame = if press.is_pressed(row * 2 + half) { frame + 1 } else { frame };
        if !pen.system_frame(canvas, frame, rect.x, rect.y) {
            crate::widget::frame(canvas, rect, pen.ink.border);
        }
    }
}

impl Screen for DivideScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Divide(self.unit)
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// A held stepper moved men: `Screen_DrawWidgets`' `0x11` arm runs
    /// `Screen_SplitArmyRows` every frame. See [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Army division".to_string()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            self.seed(&read);
        }
        // `Widget_Test`'s per-frame pass: the steppers' ramp and the tick and
        // cross's twenty-frame countdowns, each record on its own timer.
        for widget in self.press.tick() {
            let t = self.fire(ctx, widget);
            if t != Transition::Stay {
                return t;
            }
        }
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            self.seed(&read);
        }
        match event {
            // **`0x11` goes back to `0x04`, not to the map**, and all three of
            // its ways out say so: the turn-ended latch, the right release and
            // `Ui_OkButtonClicked` each write `g_screenId = 0x04`. Five of the
// nineteen right-close arms step back one level
            // campaign map — `0x0C` → `0x08`, `0x0D` → `0x0A`, `0x11` → `0x04`,
            // `0x17` → `0x0A`, `0x2A` → `0x29` — and a stack pop is that, so
            // long as the screen underneath is the one the original names.
            //
            // It was not: this screen was reached by `Transition::Replace` from
            // the information panel, so popping it landed on the campaign map.
            // [`crate::screens::info`] pushes now.
            //
            // **The right release was missing entirely** — this screen had no
            // right-button arm at all, so the button every other window in the
            // game closes with did nothing here.
            // arm: 0x0042FF10/back-one-rather-than-to-the-map right-release
            Event::RightClick { .. } => Transition::Pop,
            // **Ours, and counted.** `Screen_HandleInput` names no key on this
            // screen and the window procedure has no `0x11` case, so every one
            // of these is an invention. Kept, because the arrows are the only
            // way to move one man at a time now that a click moves ten.
            // arm: ours/divide-keyboard key
            Event::KeyDown(Key::Escape) => Transition::Pop,
            Event::KeyDown(Key::Up) => {
                self.row = self.row.saturating_sub(1);
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.row = (self.row + 1).min(MERC_ROW);
                Transition::Stay
            }
            Event::KeyDown(Key::Right) => {
                self.move_men(self.row, true, 1);
                Transition::Stay
            }
            Event::KeyDown(Key::Left) => {
                self.move_men(self.row, false, 1);
                Transition::Stay
            }
            Event::KeyDown(Key::Enter) => self.split(ctx),
            // Eighteen widgets of two kinds; [`widgets`] says which is which and
            // [`DivideScreen::fire`] is what each one does.
            //
            // **A double click is a press to all eighteen**, because every one
            // is `Widget_Test` kind 4 or 5 and both guards read
            // `g_mouseLeftPressed || g_mouseLeftDoubleClick`: a stepper steps
            // once more and does not repeat, the tick or cross restarts its
            // twenty frames. Nothing else on `0x11` answers one — the corner is
            // `Ui_OkButtonClicked`, a release, and the minimap epilogue is
            // guarded by `g_mouseLeftPressed`. `[V]` This screen dropped it.
            Event::Click { .. } | Event::DoubleClick { .. } => match self.press.event(&widgets(), event) {
                Some(i) => self.fire(ctx, i),
                // A kind-5 press consumed the click and is counting down.
                None => Transition::Stay,
            },
            // The hold ends when the button comes up, and it also ends when the
            // pointer slides off the widget — the original never says so
// because it re-runs the hit test every frame and stops
            // matching. [`Press::pointer`] is that, said out loud.
            Event::Release { x, y } => {
                self.press.release();
                // `Ui_OkButtonClicked()` — the corner picture, and the third of
                // this screen's three exits. It also writes `g_screenId = 0x04`.
                //
                // **On the RELEASE**, which is the whole of `Ui_OkButtonClicked`
                // (`0x0040E7E4`): `if (g_mouseLeftReleased == 0) return 0;` and
                // then a 24 x 24 box. `Screen_FrameInput` calls it twenty-six
                // times, so it is the single most-used way out of anything in
                // the game, and every one of ours answered on the press.
                // arm: 0x0040E7E4/divide-ok left-release
                if OK.contains(x, y) {
                    return Transition::Pop;
                }
                Transition::Stay
            }
            Event::Pointer { .. } | Event::PointerLeft => {
                let fired = self.press.event(&widgets(), event);
                debug_assert!(fired.is_none(), "no divide widget is a release widget");
                Transition::Stay
            }
            _ => Transition::Stay,
        }
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
        let w = window();
        pen.window(canvas, w.x, w.y, BOX_COLS, BOX_ROWS, 0);
        pen.eng(canvas, GROUP, 0, 0x68, 0x44, font::TEXT);
        pen.eng(canvas, GROUP, 1, 0x78, 0x1AE, font::TEXT);

        // `Screen_SplitArmyRows`' first statement — `Ui_DrawBoxInterior(0x18,
        // 0x80, 0x1A, 0x12)`, **the parchment field on its own**, tiled out of
        // `Panels.pl8` frame `0x34` like every other well in the game.
        //
        // It used to be `fill_rect(ink.background)` with an outline of ours over
        // it, which is `docs/decisions.md` C61's armoury hole exactly: under our
        // own palette `ink.background` reads as a dark plate and looks
        // deliberate, and under `Panels.pl8`'s it is a black rectangle in the
        // middle of the window. The border half is right to be missing — this
        // primitive has none.
        let well = rows_well();
        pen.box_interior(canvas, well.x, well.y, ROWS_WELL_COLS, ROWS_WELL_ROWS);

        let band = self.basket.mercenaries;
        for (row, troop) in ALL_TROOP_TYPES.iter().enumerate() {
            let y = row_y(row);
            let (left, right) =
                (self.basket.parent[troop.index()], self.basket.daughter[troop.index()]);
            // `Ui_DrawUnitNoun(2, 0x34 + t*2, 0x18, y, body)` — the literal 2 is
            // the painter's, so the plural is always the plural here.
            let noun = ctx.assets.shell.text(NOUN_GROUP, NOUN_BASE + row * 2 + 1).to_string();
            let label = if noun.is_empty() { format!("{}s", troop.name()) } else { noun };
            pen.body(canvas, NOUN_X, y, &label, font::TEXT);
            if row == self.row {
                canvas.fill_rect(NOUN_X - 4, y, 2, 14, ink.highlight);
            }
            arrows(&pen, canvas, row, &self.press);
            // `Ui_DrawNumber(…, '@', &DAT_004D40E8 | &DAT_004D40EC, 0xD8 | 0x188,
            // …)`: every number on this screen is `'@'` with a NUL suffix —
            // eight call sites, eight NULs, read out of the image. **[V]**
            pen.number_in(Face::Body, canvas, PARENT_NUMBER_X, y, left, '@', "", font::TEXT);
            pen.number_in(Face::Body, canvas, DAUGHTER_NUMBER_X, y, right, '@', "", font::TEXT);
        }

        // Row 7 — the band, drawn only when there is one,
        // painter's `bVar1` gates it.
        if let Some(m) = band {
            let y = row_y(MERC_ROW);
            let nationality = ctx.assets.shell.text(GROUP_NATIONALITY, m.band as usize).to_string();
            let label = if nationality.is_empty() {
                l2_kingdom::mercenary::ROSTER[m.band as usize].nationality.to_string()
            } else {
                nationality
            };
            // The painter puts the nationality on one line and the troop noun
            // on a **second**, indented — not the two joined with a space.
            pen.body(canvas, NOUN_X, y, &label, font::TEXT);
            let troop_noun = ctx
                .assets
                .shell
                .text(NOUN_GROUP, NOUN_BASE + m.troop.index() * 2 + 1)
                .to_string();
            let troop_noun =
                if troop_noun.is_empty() { format!("{}s", m.troop.name()) } else { troop_noun };
            pen.body(canvas, NOUN_X + 0x40, y + 0x10, &troop_noun, font::TEXT);
            if self.row == MERC_ROW {
                canvas.fill_rect(NOUN_X - 4, y, 2, 14, ink.highlight);
            }
            let (left, right) = if self.basket.mercenaries_leave {
                (0, m.men())
            } else {
                (m.men(), 0)
            };
            arrows(&pen, canvas, MERC_ROW, &self.press);
            pen.number_in(Face::Body, canvas, PARENT_NUMBER_X, y, left, '@', "", font::TEXT);
            pen.number_in(Face::Body, canvas, DAUGHTER_NUMBER_X, y, right, '@', "", font::TEXT);
        }

        // The two "Total men" lines, on the row the painter picks.
        let ty = totals_y(band.is_some());
        let total = ctx.assets.shell.text(NOUN_GROUP, TOTAL_MEN_NOUN).to_string();
        let total = if total.is_empty() { "Total men".to_string() } else { total };
        pen.body(canvas, NOUN_X, ty, &total, font::TEXT);
        // `Ui_DrawNumber(DAT_00554468, '@', …, 0xD8, y)` and
        // `Ui_DrawNumber(DAT_00554040, '@', …, 0x188, y)` — both in the body
        // face at colour `0x3F`. **The daughter's total used to be two
        // identical `text::draw_right` calls in our 5 × 7 debug font**, in the
        // highlight ink and right-aligned to a column of ours; nothing in
        // `Screen_SplitArmyRows` draws it that way.
        let (parent, daughter) = (self.basket.parent_total(), self.basket.daughter_total());
        pen.number_in(Face::Body, canvas, PARENT_NUMBER_X, ty, parent, '@', "", font::TEXT);
        pen.number_in(Face::Body, canvas, DAUGHTER_NUMBER_X, ty, daughter, '@', "", font::TEXT);

        // `Ui_OkButton(0x1AC, 0x1B4, 0)` — frame 0x33, an arrow into a hole.
        if !pen.system_frame(canvas, OK_FRAME, OK.x, OK.y) {
            widget::frame(canvas, OK, ink.border);
        }

        // `Widget_Draw(0, 0, &g_splitWidgets, …)` records 0 and 1 — `System.pl8`
        // frames 29 and 31, the tick and the cross. **They are the original's,
        // and they replace three buttons of ours** that stood where the painter
        // draws nothing: SPLIT, DISBAND and CANCEL in our own font at y 446,
        // one of which overlapped this tick.
        let tick = if self.press.is_pressed(TICK_INDEX) { 30 } else { 29 };
        let cross = if self.press.is_pressed(TICK_INDEX + 1) { 32 } else { 31 };
        if !pen.system_frame(canvas, tick, SPLIT_TICK.x, SPLIT_TICK.y) {
            widget::frame(canvas, SPLIT_TICK, ink.highlight);
        }
        if !pen.system_frame(canvas, cross, SPLIT_CROSS.x, SPLIT_CROSS.y) {
            widget::frame(canvas, SPLIT_CROSS, ink.border);
        }
        // **Ours**: the original answers a refusal with a message scroll. Debug
        // overlay only.
        if ctx.game.prefs.debug_overlay {
            text::draw(canvas, 16, 452, &self.status, ink.dim);
        }
    }
}

