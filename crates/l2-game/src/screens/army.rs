//! **The raise-army screen** — `Screen_RaiseArmy` (`0x00418653`), `g_screenId`
//! `0x17`, `L2.eng` group 69.
//!
//! # The name was wrong, and the name was the problem
//!
//! `screens/shells.rs` called this *"Hire mercenaries"* while
//! `docs/symbols.json` called its painter **`Screen_RaiseArmy`**. Both halves
//! matter and the shell table had the wrong one: **there is no mercenaries
//! screen in the game at all.** The mercenary offer is a block on *this*
//! screen, below the levy slider, and this screen is the only door to
//! `Army_Create` a player has. Filing it as optional content is what let the
//! most gameplay-critical shell in the table sit there for weeks;
//! `docs/decisions.md` C45 records it as the fifth correction of the form *a
//! name is a claim*, and the first where the cost was priority rather than a
//! wrong belief about a rule.
//!
//! # What the painter draws, read out of it
//!
//! Two layouts, chosen by whether the county has a standing offer at `+0x1AD`.
//! Everything is placed relative to one local, which is `0x80` with an offer
//! and `0xA0` without — [`base`] — and the window is `rows + 1` cells tall for
//! `rows` of `0x10` or `0x0D`:
//!
//! ```text
//! Ui_DrawBox(0x50, base - 0x10, 0x1E, rows + 1)
//! Eng_DrawString(69, 0x10, 0x70, base - 8)          the heading
//! Eng_DrawString(100, scenario*20 + county, pen, base - 8)   ... + the county's name
//! Ui_DrawInsetRect(0xC3, base + 0x28, 0x6F, 4)      the slider's well
//! system 0x51 (0x80, base+0x10)   0x4E (0xB9, base+0x21)     the left icon and arrow
//! system 0x50 (levyPercent + 0xC4, base + 0x1A)              THE KNOB
//! system 0x4F (0x132, base+0x21)  0x52 (0x145, base+0x10)    the right arrow and icon
//! Ui_DrawNumber(population - levyMen, 0x80,  base + 0x44)    who stays
//! Ui_DrawNumber(levyMen,              0x145, base + 0x44)    who marches
//! for i in 0..6:  weapon icon (0x70 + i*0x48, y - 4), stock (0x90 + i*0x48, y)
//!                 where y = base + (rows - 4) * 0x10
//! 69/10 + 69/11 + happiness - cost      at (0x188, base + 0x18 / + 0x30)
//! 69/12 + 69/13 + happiness             the same place when the cost is zero
//! ```
//!
//! **`g_levyPercent + 0xC4` is the knob's x**, which closes with the slider
//! handler's `g_levyPercent = mouseX - 0xC4` over a 101-pixel track: the two
//! are exact inverses, which is what makes [`SLIDER_X`] `[V]` rather than a
//! measurement.
//!
//! The mercenary block, when `+0x1AD` is set:
//!
//! ```text
//! Ui_DrawInsetRect(0x70, base + 0x58, 0x1A0, 0x60)
//! Ui_DrawNumber(band.men) + L2.eng 16/band + the troop noun   at (0x70, base + 0x60)
//! Ui_DrawNumber(band.price) + 69/0 "crowns to hire."          at (0x70, base + 0x7C)
//! Ui_DrawNumber(band.men / 2) + 69/1 "crowns seasonal wages." on the same line
//! gold < price  ->  69/3 "You cannot afford ..."              at (0x80, base + 0x94)
//! otherwise     ->  69/15 + the treasury, 69/2 "Hire mercenaries ?", and
//!                   L2.eng 18/0 or 18/1 — Yes or No — at (0x1D0, base + 0x98)
//! ```
//!
//! **`men / 2` is the seasonal wage this screen prints**, and it is not what
//! the treasury pays: `Wages_ForUnit` charges `men / 4`, and `g_mercWage` holds
//! a third number that nothing reads. `docs/armies.md` §6 says so; this is the
//! screen that shows the first of the three.
//!
//! # The click dispatch, and the one thing that is ours
//!
//! `Screen_Draw`'s `0x17` arm is two branches: `FUN_00435CEF` — the slider —
//! and, if that did not take the click, either the OK hotspot (close) or
//! **`g_screenId = 0x0A` with `FUN_004AA90A(county, g_levyMen)`**: the player
//! goes to the *armoury* to equip the levy, and the armoury's `+`/`−` move one
//! man at a time between the unequipped pool and a weapon rack.
//!
//! **The armoury is still a shell, so the equip controls are on this screen and
//! they say so.** The *rules* they drive are the original's —
//! [`l2_kingdom::LevyBasket::equip`] and `Levy_AutoEquip`'s ten-at-a-time
//! round-robin — and only their placement is ours. `docs/decisions.md` C21:
//! where we cannot establish what the original drew, it is visibly ours, in our
//! own font, below the original's window rather than inside it.
//!
//! The confirm is `FUN_00435B4D`, and it is what [`crate::Game::raise_army`]
//! reproduces: two size guards that a mercenary hire bypasses, then
//! `Army_Create`, then message `0xDD` if there was nowhere to stand.

use l2_kingdom::levy::{self, Levy, LevyRefusal};
use l2_kingdom::mercenary::ROSTER;
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_kingdom::LevyBasket;
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};
use crate::widget;

/// `L2.eng` group 69 — this screen's own text.
pub const GROUP: usize = 69;
/// `L2.eng` group 16 — the twelve nationalities, indexed by band id.
pub const GROUP_NATIONALITY: usize = 16;
/// `L2.eng` group 18 — *"Yes"* and *"No"*, in that order (index 0 is *"Yes"*:
/// the painter draws 18/1 when the hire flag is clear).
pub const GROUP_YESNO: usize = 18;

/// The painter's one local: `0x80` when the county has a mercenary offer,
/// `0xA0` when it has not. Everything on the screen is placed off it.
pub const fn base(offer: bool) -> i32 {
    if offer {
        0x80
    } else {
        0xA0
    }
}

/// `Ui_DrawBox`'s row count, less the `+ 1` the call adds.
pub const fn rows(offer: bool) -> i32 {
    if offer {
        0x10
    } else {
        0x0D
    }
}

/// The window: `Ui_DrawBox(0x50, base - 0x10, 0x1E, rows + 1)`.
pub const BOX_X: i32 = 0x50;
pub const BOX_COLS: i32 = 0x1E;

pub fn window(offer: bool) -> Rect {
    Rect::new(BOX_X, base(offer) - 0x10, BOX_COLS * 16, (rows(offer) + 1) * 16)
}

/// **The slider track's left end in pixels.** `Pl8_DrawFrame(system, 0x50,
/// g_levyPercent + 0xC4, …)` draws the knob and `FUN_00435CEF` reads
/// `g_levyPercent = mouseX - 0xC4` back out of it, over 101 pixels for 0…100.
pub const SLIDER_X: i32 = 0xC4;
/// Where the track stops and the right arrow begins.
pub const SLIDER_RIGHT: i32 = 0x129;
/// `FUN_00435CEF`'s hit band: `0x80 <= x < 0x176`, and `base + 0x10 <= y <
/// base + 0x40`. The three zones inside it are decrement, track, increment.
pub const SLIDER_HIT_X: (i32, i32) = (0x80, 0x176);
pub const SLIDER_HIT_DY: (i32, i32) = (0x10, 0x40);

/// `Ui_DrawInsetRect(0xC3, base + 0x28, 0x6F, 4)` — the well the knob runs in.
pub const WELL_X: i32 = 0xC3;
pub const WELL_W: i32 = 0x6F;
pub const WELL_H: i32 = 4;

/// The six weapon racks: icon at `0x70 + i * 0x48`, stock number `0x20` right
/// of it, on the row `base + (rows - 4) * 0x10`.
pub const RACK_X: i32 = 0x70;
pub const RACK_STEP: i32 = 0x48;
pub const RACK_NUMBER_DX: i32 = 0x20;

pub fn rack_row(offer: bool) -> i32 {
    base(offer) + (rows(offer) - 4) * 0x10
}

/// One weapon rack's hotspot. **The box is ours** — the painter draws a sprite
/// and the sheet's frame size is not in the widget table — but the origin and
/// the pitch are the painter's.
pub fn rack(offer: bool, i: usize) -> Rect {
    Rect::new(RACK_X + i as i32 * RACK_STEP, rack_row(offer) - 20, RACK_STEP - 8, 36)
}

/// The mercenary block's inset well, `0x60` tall with an offer and `0x32`
/// without.
pub fn merc_well(offer: bool) -> Rect {
    Rect::new(0x70, base(offer) + 0x58, 0x1A0, if offer { 0x60 } else { 0x32 })
}

/// `Eng_DrawString(18, 0 or 1, 0x1D0, base + 0x98)` — the Yes/No the hire flag
/// toggles, and the original's own hotspot for it.
pub fn hire_toggle(offer: bool) -> Rect {
    Rect::new(0x1D0, base(offer) + 0x98 - 2, 0x40, 20)
}

/// **Ours**, and below the original's window so it cannot be mistaken for it:
/// auto-equip, unequip, raise and cancel.
pub const OURS_Y: i32 = 404;
pub fn auto_button() -> Rect {
    Rect::new(0x50, OURS_Y, 116, 20)
}
pub fn strip_button() -> Rect {
    Rect::new(0x50 + 124, OURS_Y, 116, 20)
}
pub fn raise_button() -> Rect {
    Rect::new(0x50 + 248, OURS_Y, 100, 20)
}
pub fn cancel_button() -> Rect {
    Rect::new(0x50 + 356, OURS_Y, 100, 20)
}

/// `Levy_AutoEquip`'s own granularity — ten men a move. A rack click here moves
/// the same ten rather than one, because the original's one-at-a-time `+` lives
/// on a screen we do not drive yet and inventing a *different* step would be
/// inventing a rule.
pub const RACK_STEP_MEN: i32 = 10;

/// What the last confirm did, for a caller or a test that wants to know without
/// reading the kingdom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raised {
    /// Still on the screen.
    None,
    /// The army is on the map, in this slot.
    Army(usize),
    /// `FUN_00435B4D` refused, and with which message.
    Refused(LevyRefusal),
}

/// Screen `0x17` for one county.
pub struct RaiseArmyScreen {
    county: u8,
    /// `g_levyPercent` (`0x0056D65C`) — where the player put the slider, which
    /// is **not** necessarily what the county gives up. `Levy_SetPercent` takes
    /// the percentage by value and walks it back without writing it home.
    percent: i32,
    /// `g_levyMen` and `g_levyHappinessCost`, recomputed on every slider move.
    levy: Levy,
    /// `g_levyBasket` for the local player.
    basket: LevyBasket,
    /// `DAT_0055446C` — the hire flag the Yes/No toggles, and the argument
    /// `Army_Create` takes.
    hire: bool,
    pub outcome: Raised,
    /// One line of feedback. **Ours.**
    status: String,
}

impl RaiseArmyScreen {
    pub fn new(county: u8) -> RaiseArmyScreen {
        RaiseArmyScreen {
            county,
            percent: 0,
            levy: Levy { men: 0, happiness_cost: 0, settled: 0 },
            basket: LevyBasket::default(),
            hire: false,
            outcome: Raised::None,
            status: "DRAG THE SLIDER, THEN RAISE".into(),
        }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    pub fn percent(&self) -> i32 {
        self.percent
    }

    pub fn levy(&self) -> Levy {
        self.levy
    }

    pub fn basket(&self) -> &LevyBasket {
        &self.basket
    }

    pub fn hire(&self) -> bool {
        self.hire
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

    /// `Levy_SetPercent(county, pct)` then `FUN_004AA90A(county, g_levyMen)` —
    /// the two calls the slider makes, in that order. Re-seeding the basket is
    /// what makes a slider move throw away the equipment: the original does the
    /// same thing the moment the player goes back to the armoury.
    fn set_percent(&mut self, ctx: &Ctx, percent: i32) {
        self.percent = percent.clamp(0, 100);
        let Some(county) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        self.levy = levy::set_percent(&ctx.game.kingdom.tables, county, self.percent);
        let realm = &ctx.game.kingdom.realms[ctx.game.player as usize];
        self.basket = LevyBasket::seed(realm, self.levy.men);
        self.status = format!("{} MEN, {} HAPPINESS", self.levy.men, self.levy.happiness_cost);
    }

    /// `FUN_00435CEF` — the slider's three zones, on the original's numbers.
    /// Returns whether the click was the slider's.
    fn slider_click(&mut self, ctx: &Ctx, x: i32, y: i32) -> bool {
        let b = base(self.offer(ctx) != 0);
        if !(SLIDER_HIT_X.0..SLIDER_HIT_X.1).contains(&x)
            || !((b + SLIDER_HIT_DY.0)..(b + SLIDER_HIT_DY.1)).contains(&y)
        {
            return false;
        }
        let percent = if x < SLIDER_X {
            self.percent - 1
        } else if x < SLIDER_RIGHT {
            x - SLIDER_X
        } else {
            self.percent + 1
        };
        self.set_percent(ctx, percent);
        true
    }

    /// `Levy_AutoEquip` (`0x004AAD5F`), the AI's own round-robin, run for the
    /// player because the screen that would do it by hand is a shell.
    fn auto_equip(&mut self) {
        self.basket.auto_equip();
        self.status = format!("{} STILL CARRY NOTHING", self.basket.unequipped());
    }

    /// Everyone back into the unequipped pool — the armoury's *"take all"*
    /// button, `FUN_00435A0C`, applied to every rack rather than the selected
    /// one.
    fn strip(&mut self) {
        for troop in ALL_TROOP_TYPES {
            let held = self.basket.troops()[troop.index()];
            self.basket.unequip(troop, held);
        }
        self.status = "EVERY MAN A PEASANT".into();
    }

    fn equip(&mut self, troop: TroopType, n: i32) {
        let moved = self.basket.equip(troop, n);
        self.status = if moved == 0 {
            format!("NO {} TO GIVE OUT", troop.name().to_uppercase())
        } else {
            format!("{} {}", moved, troop.name().to_uppercase())
        };
    }

    /// The band's price, and whether the treasury can meet it. The painter asks
    /// this twice — once to pick `DAT_00522F58` and once to choose between
    /// `69/3` and the *"Hire mercenaries ?"* block — and both times against
    /// `g_realms[g_localPlayer].gold`.
    fn affordable(&self, ctx: &Ctx) -> bool {
        let band = self.offer(ctx) as usize;
        band != 0 && ctx.game.gold() >= ROSTER[band].price
    }

    /// `FUN_00435B4D`'s yes-arm.
    fn confirm(&mut self, ctx: &mut Ctx) -> Transition {
        let band = self.offer(ctx);
        let hire = (self.hire && band != 0 && self.affordable(ctx)).then_some(band);
        match ctx.game.raise_army(self.county, &self.basket, self.levy.happiness_cost, hire) {
            Ok(id) => {
                self.outcome = Raised::Army(id);
                Transition::Pop
            }
            Err(no) => {
                self.outcome = Raised::Refused(no);
                self.status = match no {
                    // Message 0xA8 = group 168.
                    LevyRefusal::NoMen => "AN ARMY OF ZERO MEN IS NO ARMY".into(),
                    // Message 0x94 = group 148.
                    LevyRefusal::TooFew => "FEWER THAN 50 MEN IS IMPRACTICAL".into(),
                    // Message 0xDD = group 221.
                    LevyRefusal::NowhereToStand => "NOWHERE IN THE COUNTY TO STAND".into(),
                };
                Transition::Stay
            }
        }
    }
}

impl Screen for RaiseArmyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::RaiseArmy(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Raise an army".to_string()
    }

    /// `Ui_DrawBox` over the campaign map, and no clear anywhere in the painter.
    fn is_overlay(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        // The levy is a function of the county, and the county is not this
        // screen's to own. Seeding on the first tick rather than in `new`
        // is what lets `ScreenId::build` stay a constructor with no world.
        if self.basket.total() == 0 && self.percent == 0 && self.levy.men == 0 {
            let ctx = Ctx { game: ctx.game, assets: ctx.assets };
            self.set_percent(&ctx, 0);
        }
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let offer = self.offer(&Ctx { game: ctx.game, assets: ctx.assets });
        match event {
            Event::KeyDown(Key::Escape) => Transition::Pop,
            Event::KeyDown(Key::Enter) => self.confirm(ctx),
            Event::KeyDown(Key::Left) => {
                let p = self.percent - 1;
                self.set_percent(&Ctx { game: ctx.game, assets: ctx.assets }, p);
                Transition::Stay
            }
            Event::KeyDown(Key::Right) => {
                let p = self.percent + 1;
                self.set_percent(&Ctx { game: ctx.game, assets: ctx.assets }, p);
                Transition::Stay
            }
            Event::KeyDown(Key::Char('A')) => {
                self.auto_equip();
                Transition::Stay
            }
            Event::KeyDown(Key::Char('H')) => {
                if offer != 0 {
                    self.hire = !self.hire;
                    self.status =
                        if self.hire { "HIRING".into() } else { "NOT HIRING".into() };
                }
                Transition::Stay
            }
            Event::Click { x, y } => {
                let read = Ctx { game: ctx.game, assets: ctx.assets };
                if self.slider_click(&read, x, y) {
                    return Transition::Stay;
                }
                if offer != 0 && hire_toggle(true).contains(x, y) {
                    self.hire = !self.hire;
                    self.status = if self.hire { "HIRING".into() } else { "NOT HIRING".into() };
                    return Transition::Stay;
                }
                let on = offer != 0;
                for (i, troop) in ALL_TROOP_TYPES.iter().enumerate().skip(1) {
                    if rack(on, i - 1).contains(x, y) {
                        self.equip(*troop, RACK_STEP_MEN);
                        return Transition::Stay;
                    }
                }
                if auto_button().contains(x, y) {
                    self.auto_equip();
                } else if strip_button().contains(x, y) {
                    self.strip();
                } else if raise_button().contains(x, y) {
                    return self.confirm(ctx);
                } else if cancel_button().contains(x, y) {
                    return Transition::Pop;
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
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
        let band = self.offer(ctx);
        let on = band != 0;
        let b = base(on);
        let w = window(on);
        pen.window(canvas, w.x, w.y, BOX_COLS, rows(on) + 1, 0);

        // The heading, and the county's name after it out of group 100.
        pen.eng(canvas, GROUP, 0x10, RACK_X, b - 8, font::TEXT);

        // The slider: the well, then the knob at `percent + 0xC4`.
        let well = Rect::new(WELL_X, b + 0x28, WELL_W, WELL_H);
        canvas.fill_rect(well.x, well.y, well.w, well.h, ink.background);
        widget::frame(canvas, well, ink.border);
        canvas.fill_rect(SLIDER_X + self.percent, b + 0x1A, 3, 22, ink.highlight);

        let county = ctx.game.kingdom.counties.get(self.county as usize);
        let population = county.map_or(0, |c| c.population);
        let happiness = county.map_or(0, |c| c.happiness);
        text::draw(canvas, 0x80, b + 0x44, &format!("{}", population - self.levy.men), ink.text);
        text::draw_right(canvas, 0x145 + 40, b + 0x44, &format!("{}", self.levy.men), ink.highlight);
        text::draw(canvas, WELL_X, b + 0x14, &format!("{}%", self.percent), ink.dim);

        // The six racks. `Pl8_DrawFrame(DAT_0056D5B8, i + 0x0F, …)` is the
        // weapon sprite; the sheet is not one we load, so the name goes there.
        let y = rack_row(on);
        for (i, troop) in ALL_TROOP_TYPES.iter().enumerate().skip(1) {
            let x = RACK_X + (i as i32 - 1) * RACK_STEP;
            let stock = ctx.game.kingdom.realms[ctx.game.player as usize].weapons[i - 1];
            let held = self.basket.troops()[troop.index()];
            text::draw(canvas, x, y - 18, &troop.name().to_uppercase()[..4.min(troop.name().len())], ink.dim);
            text::draw(canvas, x + RACK_NUMBER_DX, y, &format!("{stock}"), ink.text);
            if held > 0 {
                text::draw(canvas, x, y + 10, &format!("+{held}"), ink.good);
            }
        }

        // The happiness pair. The painter picks 69/10 + 69/11 when the levy
        // costs something and 69/12 + 69/13 when it does not, and prints the
        // county's happiness *after* the cost on the first branch.
        let (a, c, value) = if self.levy.happiness_cost < 1 {
            (12, 13, happiness)
        } else {
            (10, 11, happiness - self.levy.happiness_cost)
        };
        pen.eng(canvas, GROUP, a, 0x188, b + 0x18, font::TEXT);
        pen.eng(canvas, GROUP, c, 0x188, b + 0x30, font::TEXT);
        text::draw(canvas, 0x188, b + 0x40, &format!("{value}"), ink.text);

        // The mercenary block.
        let well = merc_well(on);
        canvas.fill_rect(well.x, well.y, well.w, well.h, ink.background);
        widget::frame(canvas, well, ink.border);
        if on {
            let rules = &ROSTER[band as usize];
            let nationality = {
                let s = ctx.assets.shell.text(GROUP_NATIONALITY, band as usize);
                if s.is_empty() { rules.nationality } else { s }
            };
            pen.body(
                canvas,
                0x70,
                b + 0x60,
                &format!("{} {} {}", rules.men, nationality, rules.troop.name()),
                font::TEXT,
            );
            // 69/0 "crowns to hire." and 69/1 "crowns seasonal wages.", with
            // `men / 2` for the second — this screen's own number.
            let mut x = pen.body(canvas, 0x70, b + 0x7C, &format!("{} ", rules.price), font::TEXT);
            x = {
                pen.eng(canvas, GROUP, 0, x, b + 0x7C, font::TEXT);
                x + 140
            };
            let x = pen.body(canvas, x, b + 0x7C, &format!("{} ", rules.men / 2), font::TEXT);
            pen.eng(canvas, GROUP, 1, x, b + 0x7C, font::TEXT);

            if !self.affordable(ctx) {
                pen.eng(canvas, GROUP, 3, 0x80, b + 0x94, font::TEXT);
            } else {
                pen.eng(canvas, GROUP, 2, 0x92, b + 0xA4, font::TEXT);
                let yes_no = if self.hire { 0 } else { 1 };
                let t = ctx.assets.shell.text(GROUP_YESNO, yes_no).to_string();
                let label = if t.is_empty() {
                    if self.hire { "YES" } else { "NO" }.to_string()
                } else {
                    t
                };
                widget::button(canvas, ink, hire_toggle(true), &label, self.hire);
            }
        } else {
            // 69/4, the wrapped paragraph: "There are no mercenaries currently
            // available for hire in the county."
            pen.eng(canvas, GROUP, 4, 0x80, b + 0x60, font::TEXT);
        }

        // ---- ours, and below the window so it reads as ours ---------------
        widget::button(canvas, ink, auto_button(), "AUTO-EQUIP", false);
        widget::button(canvas, ink, strip_button(), "UNEQUIP ALL", false);
        widget::button(
            canvas,
            ink,
            raise_button(),
            "RAISE",
            levy::refuse_levy(self.basket.total(), self.hire && on).is_none(),
        );
        widget::button(canvas, ink, cancel_button(), "CANCEL", false);
        text::draw(canvas, 0x50, OURS_Y + 26, &self.status, ink.dim);
        text::draw(
            canvas,
            0x50,
            OURS_Y + 38,
            "EQUIP BUTTONS AND THE RAISE BUTTON ARE OURS - THE ORIGINAL EQUIPS ON SCREEN 0x0A",
            ink.dim,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The knob's x and the handler's arithmetic are exact inverses over the
    /// whole 101-pixel track, which is what makes both `[V]`.
    #[test]
    fn the_knob_and_the_slider_handler_are_inverses_over_the_whole_track() {
        for pct in 0..=100 {
            let knob = SLIDER_X + pct;
            assert!(knob >= SLIDER_X && knob < SLIDER_RIGHT, "{pct} lands off the track");
            assert_eq!(knob - SLIDER_X, pct);
        }
        assert_eq!(SLIDER_RIGHT - SLIDER_X, 101, "0 ... 100 inclusive");
    }

    /// The two layouts, and the fact that the taller one is the one with an
    /// offer on it — which is the opposite of what a reader guesses from the
    /// smaller base y.
    #[test]
    fn the_window_grows_downward_when_a_band_is_offering() {
        let (with, without) = (window(true), window(false));
        assert_eq!((with.x, with.y, with.w, with.h), (0x50, 0x70, 480, 272));
        assert_eq!((without.x, without.y, without.w, without.h), (0x50, 0x90, 480, 224));
        assert!(with.h > without.h);
        for r in [with, without] {
            assert!(r.x + r.w <= 640 && r.y + r.h <= 480, "off screen");
        }
    }

    /// Six racks, evenly pitched, none of them overlapping and all of them
    /// inside the window.
    #[test]
    fn the_six_weapon_racks_are_inside_the_window_and_do_not_overlap() {
        for on in [true, false] {
            let w = window(on);
            for i in 0..6usize {
                let r = rack(on, i);
                assert!(w.contains(r.x, r.y), "rack {i} is outside the window");
                assert!(w.contains(r.x + r.w - 1, r.y + r.h - 1), "rack {i} runs off it");
                if i > 0 {
                    assert!(rack(on, i - 1).x + rack(on, i - 1).w <= r.x, "racks overlap");
                }
            }
        }
    }

    /// Our four buttons are below the taller of the two windows, so nothing we
    /// drew can be read as something the original drew.
    #[test]
    fn our_own_controls_are_outside_the_originals_window() {
        let w = window(true);
        for r in [auto_button(), strip_button(), raise_button(), cancel_button()] {
            assert!(r.y >= w.y + w.h, "{r:?} sits inside the original's window");
            assert!(r.x + r.w <= 640 && r.y + r.h <= 480);
        }
    }
}
