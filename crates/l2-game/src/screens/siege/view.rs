#![allow(unused_imports)]
use super::*;

use l2_kingdom::siege::{self, Engine, ENGINES, ENGINE_ORDER_CAP};
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};
use crate::widget;

/// **`Ui_DrawBevelRect(x, y, w, h)` (`0x00403FDD`) — four lines and no fill**,
/// top and right in palette `0x1F` and bottom and left in `0x10`.
///
/// That is the exact inverse of [`crate::shell::inset_rect`]'s lighting, which
/// is what makes one read as raised and the other as recessed, and it is the
/// whole of the function. It lives here
/// `inset_rect` and `button_recess` only because that module was not this
/// audit's to edit; three screens want it.
pub fn bevel_rect(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    const LIGHT: u8 = 0x1F;
    const DARK: u8 = 0x10;
    if w < 1 || h < 1 {
        return;
    }
    canvas.fill_rect(x, y, w, 1, LIGHT);
    canvas.fill_rect(x + w - 1, y, 1, h, LIGHT);
    canvas.fill_rect(x, y + h - 1, w, 1, DARK);
    canvas.fill_rect(x, y, 1, h, DARK);
}

pub fn row_plus(row: usize) -> Rect {
    Rect::new(BUTTON_X, BUTTON_Y[row.min(2)], BUTTON_DIM, BUTTON_DIM)
}

/// The other half of the pair, 26 pixels below. See [`row_plus`].
pub fn row_minus(row: usize) -> Rect {
    let plus = row_plus(row);
    Rect::new(plus.x, plus.y + BUTTON_STEP, BUTTON_DIM, BUTTON_DIM)
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
        // `DAT_0058FE2C := 1` is the painter's second statement and `:= 0` its
        // last, so every string on this screen is drawn with drop capitals.
        let p = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: Some(1),
        };
        // `FUN_004093E0(0x10, 0x30, 0x1C, 0x19)` — **set 1**, see [`BOX_SET`].
        p.window(canvas, BOX_X, BOX_Y, BOX_COLS, BOX_ROWS, BOX_SET);

        let Some(unit) = ctx.game.kingdom.campaign.units.get(self.unit) else { return };

        // `Sprite_WGenSprite(county.castleType - 1, 0x150, 0x40)` out of
        // `sgeplans.pl8`, which the painter reads whole immediately before it.
        let castle = ctx
            .game
            .kingdom
            .counties
            .get(unit.besieging_county as usize)
            .map_or(0, |c| c.castle_type);
        if castle != 0 {
            if let Some(f) =
                p.assets.sheet(PLAN_SHEET).and_then(|s| s.frame(castle as usize - 1))
            {
                canvas.blit(&f, PLAN_AT.0, PLAN_AT.1);
            }
        }

        // `Eng_DrawString(83, 0, 0x30, 0x58, &g_fontHeading, 0x3F)`.
        let title = p.assets.text(GROUP, HEADING).to_string();
        p.heading(canvas, HEADING_AT.0, HEADING_AT.1, &title, font::TEXT);

        // *"Siege will take"* / N Season(s) / *"to make ready."* — three draws
        // on two lines, and the third starts where the count ended.
        // `Ui_DrawCount` is itself two draws (a number then the group 8 noun),
        // and [`Pen::count`] returns where the noun ended, which is what places
        // the tail of the sentence. It used to be written out here with the
        // old `Pen::number(…, false)`, whose invented trailing space put the noun
        // four pixels right of `Ui_DrawCount`'s — the lead was right and the
        // suffix was not.
        p.eng(canvas, GROUP, WILL_TAKE, WILL_TAKE_AT.0, WILL_TAKE_AT.1, font::TEXT);
        let seasons = unit.siege_seasons_left;
        let end = p.count(canvas, SEASONS_AT.0, SEASONS_AT.1, seasons as i32, SEASON_NOUN, font::TEXT);
        p.eng(canvas, GROUP, TO_MAKE_READY, end, SEASONS_AT.1, font::TEXT);

        for (row, engine) in ENGINES.iter().enumerate() {
            let record = unit.engines[engine.index()];
            let y = ROW_Y[row];
            // `Eng_DrawString(83, 1 + row, 0x48, y)` — the engine's own name.
            p.eng(canvas, GROUP, ENGINE_LABEL[row], LABEL_X, y, font::TEXT);

            // `Ui_DrawInsetRect` is **four lines and no fill**: the trough is
            // painted by the two `FUN_0040437D` rectangles inside it, and the
            // fill this used to draw first was the C61 black hole.
            p.inset(canvas, Rect::new(BAR_X, y + BAR_DY, BAR_W, BAR_H));
            canvas.fill_rect(TROUGH.0, y + TROUGH.1, TROUGH.2, TROUGH.3, TROUGH_EMPTY);
            let fill = (record.percent.clamp(0, 100) as i32) / 2;
            if fill > 0 {
                canvas.fill_rect(TROUGH.0, y + TROUGH.1, fill, TROUGH.3, TROUGH_FULL);
            }
            // `Ui_DrawNumber(percent, '@', &DAT_004D4404 | …08 | …0C, 0x90,
            // y + 0x19)`, each suffix `"%"`. This comment used to say no `Pen`
            // method could carry the lead and the suffix, and so the line was
            // built as `"{percent}%"` — **no lead, digits four pixels left**.
            // `Pen::number_in` carries both. **[V]**
            let percent = record.percent as i32;
            p.number_in(Face::Body, canvas, PERCENT_X, y + PERCENT_DY, percent, '@', "%", font::TEXT);

            if record.ordered == 0 {
                // `Eng_DrawString(83, 8, 0xF0, y + 0x10)`.
                p.eng(canvas, GROUP, NO_ENGINES, NO_ENGINES_AT.0, y + NO_ENGINES_AT.1, font::TEXT);
            } else {
                // `Pl8_DrawFrame(g_miscCtySheet, 0x43 + row, 0xD0 + n * step,
                // y - 8)`, one per engine ordered.
                for n in 0..record.ordered as i32 {
                    let x = SPRITE_X + n * SPRITE_STEP[row];
                    if !p.misc_frame(canvas, ENGINE_FRAME0 + row, x, y + SPRITE_DY) {
                        canvas.fill_rect(x, y + SPRITE_DY, 12, 12, ink.highlight);
                    }
                }
            }
        }

        // `Ui_DrawBevelRect` + `FUN_0040437D` + the caption, twice.
        for (r, index) in [(LIFT, LIFT_SIEGE), (PROCEED, PROCEED_LABEL)] {
            bevel_rect(canvas, r.x, r.y, r.w, r.h);
            canvas.fill_rect(r.x + 1, r.y + 1, 0x62, 0x1A, BUTTON_FILL);
            p.eng(canvas, GROUP, index, r.x + BUTTON_LABEL_DX, BUTTON_LABEL_Y, font::TEXT);
        }

        // The six order buttons are drawn by `Widget_Draw(0, 0,
        // &g_siegePrepWidgets, 6)` from `Screen_DrawWidgets`, not by the
        // painter: `System.pl8` frames 21 and 23 at the table's own
        // coordinates. **The frames are ours to the extent that we substitute a
        // labelled box where the sheet is missing.**
        for (row, (record, cap)) in unit.engines.iter().zip(ENGINE_ORDER_CAP).enumerate() {
            let (plus, minus) = (row_plus(row), row_minus(row));
            if !p.system_frame(canvas, WIDGET_FRAME_PLUS, plus.x, plus.y) {
                widget::button(canvas, ink, plus, "+", record.ordered >= cap);
            }
            if !p.system_frame(canvas, WIDGET_FRAME_MINUS, minus.x, minus.y) {
                widget::button(canvas, ink, minus, "-", record.ordered == 0);
            }
        }

        // **Ours, and it says so**: with no `L2.eng` every label above is the
        // empty string, so the screen would otherwise be a blank window.
        if p.assets.text(GROUP, HEADING).is_empty() {
            text::draw(canvas, BOX_X + 8, BOX_Y + 8, "NO L2.ENG: SIEGE PREPARATIONS", ink.dim);
        }
    }
}

