#![allow(unused_imports)]
use super::*;
use super::screen_impl::*;
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
    pub(crate) fn arrows(&self) -> Vec<Widget> {
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

    pub(crate) fn panel_index(&self) -> usize {
        PANELS.iter().position(|&p| p == self.panel).unwrap_or(0)
    }

    /// Move the open panel's own value by `step`. Silently refused for a county
    /// the player does not hold, and there is nothing to move on the two panels
    /// that only report.
    pub(crate) fn adjust(&self, ctx: &mut Ctx, step: i32) {
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
    pub(crate) fn split_click(&self, ctx: &mut Ctx, x: i32, y: i32, pressed: bool) -> bool {
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

    pub(super) fn draw_panel(&self, ctx: &Ctx, canvas: &mut Canvas) {
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


