#![allow(unused_imports)]
use super::*;

use l2_kingdom::mercenary::ROSTER;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::armoury;
use crate::shell::{self, font, Pen};
use crate::widget;
use crate::screens::armoury::Raised;

impl RaiseArmyScreen {
    pub fn new(county: u8) -> RaiseArmyScreen {
        RaiseArmyScreen {
            county,
            status: "DRAG THE SLIDER, THEN CONTINUE".into(),
            left_down: false,
            press: Press::new(),
        }
    }

    /// The table as it stands: Continue, and the tick and cross only when the
    /// band can be afforded.
    fn table(&self, ctx: &Ctx) -> Vec<Widget> {
        let offer = self.offer(ctx) != 0;
        widgets(offer, offer && self.affordable(ctx))
    }

    /// **`Screen_HandleInput`'s `0x17` arm** — `Widget_Test(0, 0,
    /// &DAT_004DD340, n)` over the three kind-5 records.
    ///
    /// The return is what the original's `if (Screen_HandleInput() == 0)`
    /// tests: *did a record take this event*. It is not *did a handler run* —
    /// every record here is kind 5, so a press consumes the event and runs
    /// nothing, and [`Screen::update`] runs the handler twenty ticks later.
    /// That distinction is the reason this is a hit test and not the
    /// `Option<usize>` [`Press::event`] returns.
    fn widget_press(&mut self, ctx: &mut Ctx, event: Event) -> bool {
        let table = self.table(&Ctx { game: ctx.game, assets: ctx.assets });
        let fired = self.press.event(&table, event);
        debug_assert!(fired.is_none(), "every DAT_004DD340 record is kind 5");
        // `Widget_Test`'s guard on both arms is `g_mouseLeftPressed ||
        // g_mouseLeftDoubleClick`, so those are the two events a record can
        // consume. A release, a move and a pointer leaving are bookkeeping:
        // they go to `Press` above and consume nothing.
        matches!(
            event,
            Event::Click { x, y } | Event::DoubleClick { x, y }
                if table.iter().any(|w| w.rect.contains(x, y))
        )
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    /// The band on offer in this county — county `+0x1AD`, which
    /// `Mercenary_AdvanceAll` refreshes once a season.
    pub fn offer(&self, ctx: &Ctx) -> u8 {
        ctx.game
            .kingdom
            .counties
            .get(self.county as usize)
            .map_or(0, |c| c.mercenary_offer)
    }

    /// `Levy_SliderClick` (`0x00435CEF`) — the three zones, on the original's
    /// numbers. Returns whether the slider took the input, which is the
    /// function's own return: `Screen_FrameInput`'s `0x17` arm reads the right
    /// release and the corner picture **only when this answers 0**.
    ///
    /// **The three zones read three different things**, and that is the whole
    /// of the gesture:
    ///
    /// ```c
    /// if (x < 0xC4)       { if (!pressed && !doubleClick) return 0;  percent--; }
    /// else if (x < 0x129) { if (!g_mouseLeftDown)         return 0;  percent = x - 0xC4; }
    /// else                { if (!pressed && !doubleClick) return 0;  percent++; }
    /// clamp 0..100;  Levy_SetPercent(sel, percent);  g_redrawRequest = 2;  return 1;
    /// ```
    ///
    /// The arrows step once on a press or a double click and **do not repeat**:
    /// they are not a widget record, so `Widget_Test`'s kind-4 ramp never sees
    /// them. The track reads the **level** of the button, not an edge and not
    /// `g_mouseInputChanged`, so it answers on every frame the button is down
    /// inside the band — which is a drag, and a drag that also starts from a
    /// press anywhere and slides in. Off the track the level does nothing: a
    /// drag carried past either end leaves the knob where the track last put
    /// it, and only an arrow's own press reaches the clamp. `[V]`
    ///
    /// Ours was reachable from `Event::Click` alone, so the knob jumped to the
    /// press and then ignored the pointer: *"Slider bar in army recruitment
    /// cant be dragged."* `pressed` is the edge, `down` the level.
    ///
    /// **It calls `Levy_SetPercent` and nothing else.** An earlier revision of
    /// this module said the slider also ran `FUN_004AA90A` and that *"re-seeding
    /// the basket is what makes a slider move throw away the equipment"*. The
    /// tail of the function is two statements and neither is that call. The
    /// equipment does get thrown away, but by the *door into the armoury*,
    /// which re-seeds on every entry — so the visible effect survives the
    /// correction and the mechanism does not.
    fn slider_click(&mut self, ctx: &mut Ctx, x: i32, y: i32, pressed: bool, down: bool) -> bool {
        let b = base(self.offer(&Ctx { game: ctx.game, assets: ctx.assets }) != 0);
        if !(SLIDER_HIT_X.0..SLIDER_HIT_X.1).contains(&x)
            || !((b + SLIDER_HIT_DY.0)..(b + SLIDER_HIT_DY.1)).contains(&y)
        {
            return false;
        }
        let percent = if x < SLIDER_X {
            if !pressed {
                return false;
            }
            ctx.game.levy.percent - 1
        } else if x < SLIDER_RIGHT {
            if !down {
                return false;
            }
            x - SLIDER_X
        } else {
            if !pressed {
                return false;
            }
            ctx.game.levy.percent + 1
        };
        ctx.game.set_levy_percent(percent);
        self.status = format!(
            "{} MEN, {} HAPPINESS",
            ctx.game.levy.men, ctx.game.levy.happiness_cost
        );
        true
    }

    /// `FUN_00435CBF` — *Continue*: `g_screenId = 0x0A` and
    /// `FUN_004AA90A(g_selectedCounty, g_levyMen)`, in that order. The re-seed
    /// is why walking back and forth strips the levy.
    fn open_armoury(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.game.seed_levy_basket();
        Transition::Replace(ScreenId::Armoury(self.county))
    }

    /// The band's price, and whether the treasury can meet it. The painter asks
    /// this twice — once to pick `DAT_00522F58` and once to choose between
    /// `69/3` and the *"Hire mercenaries ?"* block — and both times against
    /// `g_realms[g_localPlayer].gold`.
    fn affordable(&self, ctx: &Ctx) -> bool {
        let band = self.offer(ctx) as usize;
        band != 0 && ctx.game.gold() >= ROSTER[band].price
    }
}

impl Screen for RaiseArmyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::RaiseArmy(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Raise an army".to_string()
    }

    /// `armoury.256` — the palette `Screen_Armoury` set on the frame that
    /// painted the room under this window.
    fn palette(&self) -> Option<&'static str> {
        Some("Armoury.256")
    }

    /// **A page, not an inset.** The window is a `Ui_DrawBox`, but what is
    /// behind it is the armoury this screen paints itself, not whatever opened
    /// it — see the module docs.
    fn is_overlay(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        // `Sidebar_Button` seeds before it sets `g_screenId`, so a screen that
        // finds no order for its county was opened by something that did not
        // go through `Game::open_levy` — the demo index, or a test. Seed here
        if ctx.game.levy.county != self.county {
            ctx.game.levy.county = self.county;
            let percent = ctx.game.levy.percent;
            ctx.game.set_levy_percent(percent);
            ctx.game.seed_levy_basket();
            ctx.game.levy.hire = false;
        }
        // `Widget_Test`'s countdown over `DAT_004DD340`.
        for widget in self.press.tick() {
            match widget {
                // `RaiseArmy_Continue` (`0x00435CBF`): `g_screenId = 0x0A`.
                0 => return self.open_armoury(ctx),
                // `RaiseArmy_HireToggle` (`0x00435C89`):
                // `DAT_0055446C = (g_uiHotspotId == 1)`.
                1 => ctx.game.levy.hire = true,
                _ => ctx.game.levy.hire = false,
            }
        }
        Transition::Stay
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio layer.
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// The tick or the cross changed the flag's word with no event. See
    /// [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let offer = self.offer(&Ctx { game: ctx.game, assets: ctx.assets }) != 0;
        match event {
            // The corner picture's other half: `Screen_FrameInput`'s `0x17` arm
            // sends a right release to the armoury, not to the map.
            //
            // **`0x17` is the exception to *"right click closes"*, and the two
            // ways out do different things.** Verbatim:
            //
            // ```c
            // if (!Levy_SliderClick()) {
            //   if (!rightReleased) { if (Ui_OkButtonClicked()) { g_screenId = 0; ... } }
            //   else { g_screenId = 0x0A; Levy_Seed(g_selectedCounty, g_levyMen); }
            // }
            // ```
            //
            // So the button a player reaches by habit goes **forward** to the
            // armoury and the corner picture abandons the levy. `docs/arms.json`
            // called the right release *"COMMITS the levy"*; it does not.
            // `Levy_Seed` (`0x004AA90A`) zeroes the eight basket slots, fills
            // their available counts from the realm's weapon stocks and puts the
            // headcount in slots 0 and 7 — it **prepares the armoury** for the
            // number the slider chose, and its own comment names its three
            // callers as *"every door into the armoury"*, `Sidebar_Button` and
            // `RaiseArmy_Continue` being the other two. No man is levied and no
            // gold is spent until *Create* on the armoury.
            // arm: 0x0042FF10/levy-right-commits right-release
            Event::KeyDown(Key::Escape) => Transition::Pop,
            // `if (!Levy_SliderClick()) { … rightReleased … }` — the slider is
            // asked first, so on a frame the left button is still down on the
            // track it answers 1 and the right release is never read.
            //
            // **`Screen_HandleInput` is not in front of this one**, because it
            // is left-button only: `docs/symbols.md` records that it names no
            // right-button global anywhere. So the right release meets the
            // slider first and the widget table never.
            Event::RightClick { x, y } => {
                if self.left_down && self.slider_click(ctx, x, y, false, true) {
                    return Transition::Stay;
                }
                self.open_armoury(ctx)
            }
            Event::KeyDown(Key::Enter) => self.open_armoury(ctx),
            Event::KeyDown(Key::Left) => {
                let p = ctx.game.levy.percent - 1;
                ctx.game.set_levy_percent(p);
                Transition::Stay
            }
            Event::KeyDown(Key::Right) => {
                let p = ctx.game.levy.percent + 1;
                ctx.game.set_levy_percent(p);
                Transition::Stay
            }
            Event::KeyDown(Key::Char('H')) => {
                if offer {
                    ctx.game.levy.hire = !ctx.game.levy.hire;
                }
                Transition::Stay
            }
            // The arrows: one step on the press, no repeat. The press also puts
            // the button down, which is what the track reads.
            // arm: 0x00435CEF/levy-slider-step left-press
            Event::Click { x, y } => {
                // `WM_LBUTTONDOWN` sets the down bit whatever the press landed
                // on, so this is written before the table is asked and not in
                // its `else`. See the field.
                self.left_down = true;
                if self.widget_press(ctx, event) {
                    return Transition::Stay;
                }
                self.slider_click(ctx, x, y, true, true);
                Transition::Stay
            }
            // **A double click is a press to a kind-5 record**, and restarts its
            // twenty frames; `Widget_Test`'s guard is `g_mouseLeftPressed ||
            // g_mouseLeftDoubleClick`. It sets no down bit, so past the table
            // it steps an arrow and leaves the track alone.
            Event::DoubleClick { x, y } => {
                if self.widget_press(ctx, event) {
                    return Transition::Stay;
                }
                self.slider_click(ctx, x, y, true, false);
                Transition::Stay
            }
            // The release ends the hold in both halves at once: it clears the
            // record's `held` in `Press` and `g_mouseLeftDown` for the track.
            // Neither consumes it —
            // and the slider reads a level, not an edge.
            //
            // **And the corner picture is here, not in the press.** The `0x17`
            // arm is
            //
            // ```c
            // if (!Levy_SliderClick()) {
            //   if (!rightReleased) { if (Ui_OkButtonClicked()) { g_screenId = 0; … } }
            //   else { g_screenId = 0x0A; Levy_Seed(…); }
            // }
            // ```
            //
            // and `Ui_OkButtonClicked` (`0x0040E7E4`) opens
            // `if (g_mouseLeftReleased == 0) return 0;`. This screen closed on
            // the press. `Levy_SliderClick` is asked first and cannot answer a
            // release — its two arrow branches want `g_mouseLeftPressed ||
            // g_mouseLeftDoubleClick` and its track branch `g_mouseLeftDown`,
            // all three clear on the release frame — so the order is kept and
            // changes nothing. The corner is at x `0x24C` and the slider's band
            // ends at `0x176`, so they do not overlap either.
            // arm: 0x0042FF10/raise-army-ok left-release
            Event::Release { x, y } => {
                self.widget_press(ctx, event);
                self.left_down = false;
                if OK.contains(x, y) {
                    return Transition::Pop;
                }
                Transition::Stay
            }
            // **The track follows the button's level.** See
            // [`RaiseArmyScreen::slider_click`]: nothing but `g_mouseLeftDown`
            // is read there, so every pointer position while the button is down
            // is a new percentage, and a release ends it. The table is
            // re-hit-tested first
            // frame, so a pointer that has walked off a record drops its hold.
            // arm: 0x00435CEF/levy-slider-track drag
            Event::Pointer { x, y } => {
                self.widget_press(ctx, event);
                if self.left_down {
                    self.slider_click(ctx, x, y, false, true);
                }
                Transition::Stay
            }
            Event::PointerLeft => {
                self.widget_press(ctx, event);
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        // The room underneath, the same painter `Screen_Draw` runs first. The
        // three labels come out dim: they are painted on this screen and dead
        // on it.
        armoury::page(ctx, canvas, false);

        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let band = self.offer(ctx);
        let on = band != 0;
        let b = base(on);
        let levy = ctx.game.levy;
        let w = window(on);
        pen.window(canvas, w.x, w.y, BOX_COLS, rows(on) + 1, 0);

        // 69/16 "Raising an army in", then the county's own name out of group
        // 100 at `scenarioIndex * 20 + county` — the painter chains them on
        // `g_penAdvance`, and so do we now that [`Pen::body`] returns where it
// stopped.
        let x = pen.eng(canvas, GROUP, RAISING_IN, RACK_X, b - 8, font::TEXT);
        pen.eng(canvas, COUNTY_NAMES, county_name_index(ctx, self.county), x, b - 8, font::TEXT);

        // The slider. `Ui_DrawInsetRect` is four lines and no fill — see
        // [`shell::inset_rect`] — and the five pictures round it are
        // `System.pl8` frames 0x51, 0x4E, 0x50, 0x4F and 0x52, the last of
        // which is the knob at `g_levyPercent + 0xC4`.
        let well = Rect::new(WELL_X, b + 0x28, WELL_W, WELL_H);
        shell::inset_rect(canvas, well.x, well.y, well.w, well.h);
        let knob = match ctx.assets.chrome.as_ref() {
            Some(c) => {
                c.draw_system(canvas, SLIDER_LEFT_ICON, 0x80, b + 0x10);
                c.draw_system(canvas, SLIDER_LEFT_ARROW, 0xB9, b + 0x21);
                c.draw_system(canvas, SLIDER_RIGHT_ARROW, 0x132, b + 0x21);
                c.draw_system(canvas, SLIDER_RIGHT_ICON, 0x145, b + 0x10);
                c.draw_system(canvas, SLIDER_KNOB, SLIDER_X + levy.percent, b + 0x1A)
            }
            None => false,
        };
        if !knob {
            canvas.fill_rect(SLIDER_X + levy.percent, b + 0x1A, 3, 22, ink.highlight);
        }

        let county = ctx.game.kingdom.counties.get(self.county as usize);
        let population = county.map_or(0, |c| c.population);
        let happiness = county.map_or(0, |c| c.happiness);
        // Both are plain `Ui_DrawNumber` at the coordinate given — **left
        // aligned**, in the body font. This module drew the second of them
        // right-aligned to `0x145 + 40`, which is 40 pixels of drift on the
        // number a player reads to decide how many men to take.
        // `Ui_DrawNumber(…, '@', &DAT_004D40B0 | &DAT_004D40B4, …)`, both NUL. **[V]**
        let body = shell::Face::Body;
        pen.number_in(body, canvas, 0x80, b + 0x44, population - levy.men, '@', "", font::TEXT);
        pen.number_in(body, canvas, 0x145, b + 0x44, levy.men, '@', "", font::TEXT);

        // The six weapon stocks: `arm_it_<colour>.pl8` frames 15..20 with the
        // realm's stock printed 0x20 to the right of each.
        let y = rack_row(on);
        let realm = &ctx.game.kingdom.realms[ctx.game.player as usize];
        let sheet = ctx.assets.shell.sheet(armoury::items_sheet(realm.shield_index));
        for (i, troop) in [
            TroopType::Crossbowman,
            TroopType::Maceman,
            TroopType::Swordsman,
            TroopType::Pikeman,
            TroopType::Archer,
            TroopType::Knight,
        ]
        .iter()
        .enumerate()
        {
            let x = RACK_X + i as i32 * RACK_STEP;
            let stock = realm.weapons[i];
            match sheet.and_then(|s| s.frame(ICON_FRAME_BASE + i)) {
                Some(f) => canvas.blit(&f, x, y - 4),
                None => {
                    // **Ours**, and only when the sheet is not installed: the
                    // troop's name where its icon would have been.
                    let n = troop.name().to_uppercase();
                    text::draw(canvas, x, y - 18, &n[..4.min(n.len())], ink.dim);
                }
            }
            // `Ui_DrawNumber(stock, ' ', &DAT_004D40B8, …)` — a **space** lead
            // here, not the '@' the rest of the screen uses, and the only
// suffix on this screen that is one space. **[V]**
            let dx = x + RACK_NUMBER_DX;
            pen.number_in(shell::Face::Body, canvas, dx, y, stock, ' ', " ", font::TEXT);
        }

        // The happiness pair. The painter picks 69/10 + 69/11 when the levy
        // costs something and 69/12 + 69/13 when it does not, and the number is
        // **chained on to the second word, on that word's own line** — not on a
        // line of its own sixteen pixels lower, which is where this module put
        // it. Then `system` frame 0x53 after the number, on both branches.
        let (a, c, value) = if levy.happiness_cost < 1 {
            (HAPPINESS_STAYS, HAPPINESS_AT, happiness)
        } else {
            (HAPPINESS_WILL, HAPPINESS_BE, happiness - levy.happiness_cost)
        };
        pen.eng(canvas, GROUP, a, 0x188, b + 0x18, font::TEXT);
        let x = pen.eng(canvas, GROUP, c, 0x188, b + 0x30, font::TEXT);
        // `Ui_DrawNumber(…, '@', &DAT_004D40C0 | &DAT_004D40BC, g_penAdvance +
        // 0x188, …)`, both NUL, then the face at `g_penAdvance + 0x188`. The old
        // no-lead-plus-space put the digits four left and the face exactly
// where it belongs, so nobody saw it. **[V]**
        let x = pen.number_in(shell::Face::Body, canvas, x, b + 0x30, value, '@', "", font::TEXT);
        pen.system_frame(canvas, HAPPINESS_ICON, x, b + 0x30);

        // The mercenary block.
        let well = merc_well(on);
        shell::inset_rect(canvas, well.x, well.y, well.w, well.h);
        if on {
            let rules = &ROSTER[band as usize];
            // **The headline line is the heading font**, all three pieces of
            // it: the count, `L2.eng` 16/band, and the group 8 troop noun that
            // `Ui_DrawUnitNoun` picks. This module drew the whole line in the
            // body font with our own troop name in place of the noun.
            // `Ui_DrawNumber(men, '@', &DAT_004D40C4, 0x70, base + 0x60,
            // &g_fontHeading)` — a NUL suffix. This was built by hand as
            // `"{men} "`: no lead and an invented space, the same pair
            // `Pen::number(…, true)` drew. **[V]**
            let heading = shell::Face::Heading;
            let x = pen.number_in(heading, canvas, 0x70, b + 0x60, rules.men, '@', "", font::TEXT);
            let s = ctx.assets.shell.text(GROUP_NATIONALITY, band as usize).to_string();
            let s = if s.is_empty() { rules.nationality.to_string() } else { s };
            let x = pen.heading(canvas, x, b + 0x60, &s, font::TEXT);
            // `Ui_DrawUnitNoun` is singular only at exactly 1 — unlike
            // `Ui_DrawCount`, which also takes it at −1.
            let index = NOUN_BASE + rules.troop.index() * 2 + usize::from(rules.men != 1);
            let s = ctx.assets.shell.text(NOUN_GROUP, index).to_string();
            let s = if s.is_empty() { format!("{}s", rules.troop.name()) } else { s };
            pen.heading(canvas, x, b + 0x60, &s, font::TEXT);

            // 69/0 "crowns to hire." and 69/1 "crowns seasonal wages.", with
            // `men / 2` for the second — this screen's own number. Both
            // numbers are `'@'` with `&DAT_004D40C8` / `&DAT_004D40CC`, NULs;
            // each noun lands where it did before, and each number four pixels
            // right of where it was. **[V]**
            let body = shell::Face::Body;
            let x = pen.number_in(body, canvas, 0x70, b + 0x7C, rules.price, '@', "", font::TEXT);
            let x = pen.eng(canvas, GROUP, CROWNS_TO_HIRE, x, b + 0x7C, font::TEXT);
            let x = pen.number_in(body, canvas, x, b + 0x7C, rules.men / 2, '@', "", font::TEXT);
            pen.eng(canvas, GROUP, CROWNS_WAGES, x, b + 0x7C, font::TEXT);

            if !self.affordable(ctx) {
                // 69/3, wrapped the same way: `FUN_0040328E(0x45, 3, 0x80,
                // base + 0x94, 400, 100, …)`.
                let s = ctx.assets.shell.text(GROUP, CANNOT_AFFORD).to_string();
                pen.body_wrapped(canvas, 0x80, b + 0x94, PARAGRAPH_W, &s, font::TEXT);
            } else {
                // 69/15 "You have" and `Ui_DrawCount(gold, 0)` — the treasury
                // and *"Crown."* / *"Crowns."*. **This pair was missing**; the
                // module docs described it and the painter did not draw it, so
                // the player was asked to buy without being told what he had.
                let x = pen.eng(canvas, GROUP, YOU_HAVE, 0x72, b + 0x92, font::TEXT);
                let gold = ctx.game.gold();
                // `Ui_DrawCount(gold, 0, g_penAdvance + 0x72, base + 0x92, body)`
                // in `Screen_RaiseArmy` — through `Pen::count`, which carries its
                // `'@'` lead and empty suffix. This line used to build the pair by
                // hand from the old `Pen::number(…, true)`, which drew the digits
                // four pixels left of the original's. It also takes the singular
// at **±1**.
                pen.count(canvas, x, b + 0x92, gold, CROWN_NOUN, font::TEXT);

                pen.eng(canvas, GROUP, HIRE_QUESTION, 0x92, b + 0xA4, font::TEXT);
                // The word is a read-out; the tick and the cross are the
                // buttons, and they are records 1 and 2 of the widget table —
                // frames 29 and 31 of the button sheet, not labels of ours.
                let yes_no = if levy.hire { 0 } else { 1 };
                let t = ctx.assets.shell.text(GROUP_YESNO, yes_no).to_string();
                let label = if t.is_empty() {
                    if levy.hire { "YES" } else { "NO" }.to_string()
                } else {
                    t
                };
                let r = hire_readout(on);
                pen.heading(canvas, r.x, r.y + 2, &label, font::TEXT);
                let (yes, no) = (hire_yes(on), hire_no(on));
                let up = |i: usize| usize::from(self.press.is_pressed(i));
                if !pen.system_frame(canvas, HIRE_YES_FRAME + up(1), yes.x, yes.y) {
                    widget::frame(canvas, yes, if levy.hire { ink.highlight } else { ink.border });
                }
                if !pen.system_frame(canvas, HIRE_NO_FRAME + up(2), no.x, no.y) {
                    widget::frame(canvas, no, if levy.hire { ink.border } else { ink.highlight });
                }
            }
        } else {
            // 69/4, and it really is wrapped: `FUN_0040328E(0x45, 4, 0x80,
            // base + 0x60, 400, 100, 0, 0, …)` — a 400-pixel column, which is
            // what the sentence needs and what drawing it as one line does not
            // give it.
            let s = ctx.assets.shell.text(GROUP, NO_MERCENARIES).to_string();
            pen.body_wrapped(canvas, 0x80, b + 0x60, PARAGRAPH_W, &s, font::TEXT);
        }

        // The footer: the realm's weapon total, 69/14, and 69/9 "Continue"
        // beside the button that leaves for the armoury.
        let fy = footer_row(on);
        let total: i32 = realm.weapons.iter().sum();
        // `Ui_DrawNumber(total, '@', &DAT_004D40D0, 0x70, …)`, a NUL. **[V]**
        let x = pen.number_in(shell::Face::Body, canvas, RACK_X, fy, total, '@', "", font::TEXT);
        pen.eng(canvas, GROUP, TOTAL_WEAPONS, x, fy, font::TEXT);
        pen.eng(canvas, GROUP, CONTINUE, CONTINUE_LABEL_X, fy, font::TEXT);
        let cont = continue_button(on);
        let cont_frame = CONTINUE_FRAME + usize::from(self.press.is_pressed(0));
        if !pen.system_frame(canvas, cont_frame, cont.x, cont.y) {
            widget::frame(canvas, cont, ink.highlight);
        }

        // **Ours**: one line of feedback, below the original's window. Debug
        // overlay only.
        if ctx.game.prefs.debug_overlay {
            text::draw(canvas, BOX_X, w.y + w.h + 4, &self.status, ink.dim);
        }
    }
}

