//! **The siege-preparation screen** — `Screen_SiegePrep` (`0x00421F14`),
//! `g_screenId` `0x1D`, `L2.eng` group 83.
//!
//! It was one of the shells in [`crate::screens::shells`]: it drew the window
//! and the words and did nothing. It is the only place a player ever chooses
//! what to build for a siege, so a siege that could be laid but not equipped
//! stopped here.
//!
//! # What the original draws, read out of the painter
//!
//! ```text
//! Ui_DrawBox(0x10, 0x30, 0x1C, 0x19, 1)              the window
//! Pl8_DrawFrame(sgeplans.pl8, castleType - 1, 0x150, 0x40)   the castle's picture
//! Eng_DrawString(83, 0, 0x30, 0x58, heading)         "Siege preparations."
//! Eng_DrawString(83, 4, 0x40, 0x78)                  "Siege will take"
//! Ui_DrawCount(unit +0x19C, 0x42, 0x50, 0x88)        N Season(s)
//! Eng_DrawString(83, 5, pen + 0x20, 0x88)            "to make ready."
//! for each of the three rows:
//!     Eng_DrawString(83, 1 + row, 0x48, y)           the engine's name
//!     Ui_DrawInsetRect(0x50, y + 0x18, 0x34, 8)      the percent bar's well
//!     bar fill from record +2, half-width                 (percent / 2 of 0x32)
//!     Ui_DrawNumber(record +2, '@', 0x90, y + 0x19)      the percentage
//!     if record +0 == 0  Eng_DrawString(83, 8, 0xF0, …)  "- No engines to be built"
//!     else               one sprite per engine ordered
//! Ui_DrawBevelRect(0x68, 0x18C, 100, 0x1C) + Eng_DrawString(83, 6)   "Lift siege"
//! Ui_DrawBevelRect(0x108, 0x18C, 100, 0x1C) + Eng_DrawString(83, 7)  "Proceed"
//! ```
//!
//! **The row order is the record order, and that is what settles which engine
//! costs what.** Row 1 is drawn from `+0x182` and labelled `L2.eng` 83/1
//! *"Catapults"*; row 2 from `+0x188`, *"Siege towers"*; row 3 from `+0x18E`,
//! *"Battering rams"*. `Army_PrepareForBattle` maps those same three records to
//! troop types 7, 8 and 9, and `g_siegeEngineWork` is indexed by the same
//! record number — so the catapult is 200 man-seasons, the tower 200 and the
//! ram 400. `[V]`
//!
//! # The two buttons and the caps
//!
//! Each engine row has **two** hotspots, and `g_siegeWidgets` (`0x004DDF10`)
//! gives all six their coordinates: one increments the order (`0x0043B681`) and
//! one decrements it (`0x0043B741`), and both end in `0x0043B7C4`, which runs
//! `Siege_RecomputeBuildTime` — so the *"Siege will take N Season(s)"* line
//! moves as you click. The increment stops at
//! [`l2_kingdom::siege::ENGINE_ORDER_CAP`] — **four catapults, four towers, two
//! rams** — which nobody had read before. Cap times cost is 800 man-seasons for
//! all three rows, which is a second, independent statement that
//! `g_siegeEngineWork` is `[200, 200, 400]` in that order.
//!
//! *"Lift siege"* calls `Siege_Break` and closes. *"Proceed"* closes, and
//! **launches the assault only if the countdown is already zero** — which it is
//! when nothing has been ordered. So a player besieging a palisade can order
//! nothing and storm it the same season, and a player besieging a stone castle
//! who does the same is refused by `Siege_LaunchAssault`'s gate with `L2.eng`
//! 281.

use l2_kingdom::siege::{self, Engine, ENGINES, ENGINE_ORDER_CAP};
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

/// `FUN_004093E0(0x10, 0x30, 0x1C, 0x19)` — the window: origin in **pixels**,
/// size in 16-pixel **cells**, which is the call's own mixed convention and the
/// same one `Panel_JobDetail` uses. 448 × 400 at (16, 48).
pub const BOX_X: i32 = 0x10;
pub const BOX_Y: i32 = 0x30;
pub const BOX_COLS: i32 = 0x1C;
pub const BOX_ROWS: i32 = 0x19;

/// The y of each engine row's label — `0xC0`, `0x104`, `0x148`, and the rows
/// are 0x44 apart.
pub const ROW_Y: [i32; 3] = [0xC0, 0x104, 0x148];
/// `Eng_DrawString(83, 1 + row, 0x48, …)`.
pub const LABEL_X: i32 = 0x48;
/// `Ui_DrawInsetRect(0x50, rowY + 0x18, 0x34, 8)` — the percent bar's well.
pub const BAR_X: i32 = 0x50;
pub const BAR_W: i32 = 0x34;
pub const BAR_H: i32 = 8;
/// `Ui_DrawNumber(percent, '@', 0x90, rowY + 0x19)`.
pub const PERCENT_X: i32 = 0x90;
/// Where the ordered engines' sprites go — `0xD0` plus 0x3C a piece for the
/// first two rows and 0x50 for the rams, which are wider.
pub const SPRITE_X: i32 = 0xD0;
pub const SPRITE_STEP: [i32; 3] = [0x3C, 0x3C, 0x50];

/// `Ui_DrawBevelRect(0x68, 0x18C, 100, 0x1C)` — *"Lift siege"*.
pub const LIFT: Rect = Rect::new(0x68, 0x18C, 100, 0x1C);
/// `Ui_DrawBevelRect(0x108, 0x18C, 100, 0x1C)` — *"Proceed"*.
pub const PROCEED: Rect = Rect::new(0x108, 0x18C, 100, 0x1C);

/// The **six** hotspots, and these are the original's own coordinates.
///
/// `g_siegeWidgets` (`0x004DDF10`) holds six records: the even ones are the
/// increment buttons at **x 38, y 184 / 252 / 320** drawing button frame 21,
/// and the odd ones the decrement buttons **26 pixels below each** drawing
/// frame 23, with hotspot ids 0, 1, 2 for both. That table was read into
/// `docs/hypotheses.json` before this screen existed and is what makes the
/// layout the original's rather than ours.
///
/// The two handlers are `SiegePrep_OrderMore` (`0x0043B681`) and
/// `SiegePrep_OrderFewer` (`0x0043B741`), and each row's button pair sits eight
/// pixels above its `L2.eng` label at [`ROW_Y`].
pub const BUTTON_X: i32 = 38;
pub const BUTTON_Y: [i32; 3] = [184, 252, 320];
/// The decrement button's offset below its partner.
pub const BUTTON_STEP: i32 = 26;
/// The button sprites are `Panels.pl8` frames 21 and 23. Their size is not in
/// the table, so the 24 × 24 box is **ours**.
pub const BUTTON_DIM: i32 = 24;

pub fn row_plus(row: usize) -> Rect {
    Rect::new(BUTTON_X, BUTTON_Y[row.min(2)], BUTTON_DIM, BUTTON_DIM)
}

/// The other half of the pair, 26 pixels below. See [`row_plus`].
pub fn row_minus(row: usize) -> Rect {
    let plus = row_plus(row);
    Rect::new(plus.x, plus.y + BUTTON_STEP, BUTTON_DIM, BUTTON_DIM)
}

/// What the player asked the campaign to do when the screen closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeChoice {
    /// Still on the screen.
    None,
    /// *"Lift siege"* — `Siege_Break`, and the army is free again.
    Lift,
    /// *"Proceed"* with the engines already finished: `Siege_LaunchAssault`
    /// runs now.
    Assault,
    /// *"Proceed"* with work outstanding: the screen closes and turn phase 2
    /// carries the build on.
    Wait,
}

/// Screen `0x1D` for one besieging army.
pub struct SiegeScreen {
    /// `g_siegeScreenUnit`.
    unit: usize,
    /// What the last click decided, for a caller driving the campaign.
    pub choice: SiegeChoice,
}

impl SiegeScreen {
    pub fn new(unit: usize) -> SiegeScreen {
        SiegeScreen { unit, choice: SiegeChoice::None }
    }

    pub fn unit(&self) -> usize {
        self.unit
    }

    pub fn window() -> Rect {
        Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS * 16)
    }

    /// The seasons the *"Siege will take"* line prints.
    pub fn seasons(&self, ctx: &Ctx) -> u8 {
        ctx.game.kingdom.campaign.units.get(self.unit).map_or(0, |u| u.siege_seasons_left)
    }

    /// Order one more of an engine, or one fewer — `0x0043B681` and
    /// `0x0043B741`, both of which end in `0x0043B7C4`.
    pub fn order(&mut self, ctx: &mut Ctx, engine: Engine, delta: i16) -> bool {
        siege::order_engine(&mut ctx.game.kingdom.campaign.units, self.unit, engine, delta)
    }

    /// *"Lift siege"*, `L2.eng` 83/6.
    fn lift(&mut self, ctx: &mut Ctx) -> Transition {
        let l2_kingdom::Kingdom { counties, campaign, .. } = &mut ctx.game.kingdom;
        siege::break_siege(counties, &mut campaign.units, self.unit);
        self.choice = SiegeChoice::Lift;
        Transition::Pop
    }

    /// *"Proceed"*, `L2.eng` 83/7 — and the countdown decides which of the two
    /// things it means.
    fn proceed(&mut self, ctx: &mut Ctx) -> Transition {
        self.choice =
            if self.seasons(ctx) == 0 { SiegeChoice::Assault } else { SiegeChoice::Wait };
        Transition::Pop
    }
}

impl Screen for SiegeScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Siege(self.unit)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Siege preparations".to_string()
    }

    /// The painter clears nothing: it draws a `Ui_DrawBox` over the campaign
    /// map the click came from.
    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) => self.lift(ctx),
            Event::KeyDown(Key::Enter) => self.proceed(ctx),
            Event::Click { x, y } => {
                if LIFT.contains(x, y) {
                    return self.lift(ctx);
                }
                if PROCEED.contains(x, y) {
                    return self.proceed(ctx);
                }
                for (row, engine) in ENGINES.iter().enumerate() {
                    if row_plus(row).contains(x, y) {
                        self.order(ctx, *engine, 1);
                        break;
                    }
                    if row_minus(row).contains(x, y) {
                        self.order(ctx, *engine, -1);
                        break;
                    }
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let w = SiegeScreen::window();
        let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| {
            ch.draw_box(canvas, w.x, w.y, BOX_COLS, BOX_ROWS, 0);
            true
        });
        if !drawn {
            widget::panel(canvas, ink, w);
        }

        text::draw(canvas, 0x30, 0x58, "SIEGE PREPARATIONS.", ink.highlight);

        let Some(unit) = ctx.game.kingdom.campaign.units.get(self.unit) else { return };
        let seasons = unit.siege_seasons_left;
        let plural = if seasons == 1 { "SEASON" } else { "SEASONS" };
        text::draw(
            canvas,
            0x40,
            0x78,
            &format!("SIEGE WILL TAKE {seasons} {plural} TO MAKE READY."),
            ink.text,
        );

        for (row, engine) in ENGINES.iter().enumerate() {
            let record = unit.engines[engine.index()];
            let y = ROW_Y[row];
            text::draw(canvas, LABEL_X, y, &engine.name().to_uppercase(), ink.text);

            // The percent bar: a 0x34-wide well with 0x32 of fill, and the fill
            // is `percent / 2` — which is what makes 100 % exactly full.
            let well = Rect::new(BAR_X, y + 0x18, BAR_W, BAR_H);
            canvas.fill_rect(well.x, well.y, well.w, well.h, ink.background);
            widget::frame(canvas, well, ink.border);
            let fill = (record.percent.clamp(0, 100) as i32) / 2;
            if fill > 0 {
                canvas.fill_rect(well.x + 1, well.y + 1, fill, well.h - 2, ink.highlight);
            }
            text::draw(canvas, PERCENT_X, y + 0x19, &format!("{}%", record.percent), ink.text);

            if record.ordered == 0 {
                // `L2.eng` 83/8, drawn at (0xF0, rowY - 0x10).
                text::draw(canvas, 0xF0, y - 0x10, "- NO ENGINES TO BE BUILT", ink.text);
            } else {
                // One sprite an engine in the original; `misc_cty.pl8` frames
                // 0x43, 0x44 and 0x45. We have no sheet loaded for it, so the
                // count is drawn as a count and says which it is.
                for n in 0..record.ordered as i32 {
                    let x = SPRITE_X + n * SPRITE_STEP[row];
                    canvas.fill_rect(x, y - 0x10, 12, 12, ink.highlight);
                }
                text::draw(
                    canvas,
                    SPRITE_X,
                    y + 0x19,
                    &format!("{} OF {}", record.ordered, ENGINE_ORDER_CAP[engine.index()]),
                    ink.text,
                );
            }
        }

        widget::button(canvas, ink, LIFT, "LIFT SIEGE", false);
        widget::button(canvas, ink, PROCEED, "PROCEED", seasons == 0);

        // **Ours, and it says so.** The original's screen shows a picture of
        // the castle from `sgeplans.pl8` and no numbers beyond the percentages;
        // a player who has not read `docs/rules.md` has no way to know what a
        // click costs.
        // The six buttons, at the widget table's own coordinates. Their sprites
        // are `Panels.pl8` frames 21 and 23, which we do not draw; the boxes are
        // where the original's are.
        for (row, (record, cap)) in unit.engines.iter().zip(ENGINE_ORDER_CAP).enumerate() {
            widget::button(canvas, ink, row_plus(row), "+", record.ordered >= cap);
            widget::button(canvas, ink, row_minus(row), "-", record.ordered == 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three rows are the three records in order, and the caps are the
    /// screen's.
    #[test]
    fn the_three_rows_are_the_three_records_in_the_order_the_labels_name_them() {
        assert_eq!(ENGINES[0].name(), "Catapults");
        assert_eq!(ENGINES[1].name(), "Siege towers");
        assert_eq!(ENGINES[2].name(), "Battering rams");
        assert_eq!(ENGINE_ORDER_CAP, [4, 4, 2]);
        // Every row maxes out at the same 800 man-seasons — cap times cost is
        // constant across the three, which a swapped table would break.
        for (engine, cap) in ENGINES.iter().zip(ENGINE_ORDER_CAP) {
            assert_eq!(
                siege::ENGINE_WORK[engine.index()] * cap as i32,
                800,
                "{}",
                engine.name()
            );
        }
    }

    /// Six hotspots, none of them overlapping and none of them reaching the two
    /// buttons at the bottom.
    #[test]
    fn the_six_row_hotspots_are_distinct_and_clear_of_the_buttons() {
        let mut spots: Vec<Rect> = Vec::new();
        for row in 0..3 {
            spots.push(row_plus(row));
            spots.push(row_minus(row));
        }
        for (i, a) in spots.iter().enumerate() {
            for (j, b) in spots.iter().enumerate() {
                if i == j {
                    continue;
                }
                let apart = a.x + a.w <= b.x
                    || b.x + b.w <= a.x
                    || a.y + a.h <= b.y
                    || b.y + b.h <= a.y;
                assert!(apart, "hotspots {i} and {j} overlap");
            }
            assert!(a.y + a.h <= LIFT.y, "hotspot {i} runs into the buttons");
            assert!(SiegeScreen::window().contains(a.x, a.y), "hotspot {i} is off the window");
        }
    }
}
