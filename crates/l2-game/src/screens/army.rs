//! **The raise-army screen** — `Screen_RaiseArmy` (`0x00418653`), `g_screenId`
//! `0x17`, `L2.eng` group 69.
//!
//! # The name was wrong, and the name was the problem
//!
//! `screens/shells.rs` called this *"Hire mercenaries"* while
//! `docs/symbols.json` called its painter **`Screen_RaiseArmy`**. Both halves
//! matter and the shell table had the wrong one: **there is no mercenaries
//! screen in the game at all.** The mercenary offer is a block on *this*
//! screen, below the levy slider, and this screen is the only door to the
//! armoury a player has. Filing it as optional content is what let the most
//! gameplay-critical shell in the table sit there for weeks;
//! `docs/decisions.md` C45 records it as the fifth correction of the form *a
//! name is a claim*, and the first where the cost was priority rather than a
//! wrong belief about a rule.
//!
//! # It is a window on the armoury, and that was the second finding
//!
//! ```c
//! else if (g_screenId == '\x17') { if (firstFrame == 1) Screen_Armoury(1);
//!                                  Screen_RaiseArmy(); }
//! ```
//!
//! **This screen has no background of its own.** `Screen_Draw` paints the
//! armoury under it and `Screen_Armoury` ends with `Palette_Set(armoury.256)`,
//! so the levy window is a `Ui_DrawBox` over a forge, in the armoury's palette
//! — not a popup over the campaign map in the campaign palette, which is what
//! this module used to answer and what a player was looking at when he called
//! it *"a weird popup"*. [`crate::screens::armoury::page`] is the painter both
//! screens share, exactly as the binary shares it. `docs/decisions.md` C61.
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
//! Eng_DrawString(69, 0x10, 0x70, base - 8)          "Raising an army in"
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
//! Ui_DrawNumber(realm + 0x138) + 69/14  at (0x70,  (rows-2)*0x10 + base + 4)
//! Eng_DrawString(69, 9)                 at (0x180, the same row)
//! ```
//!
//! **`g_levyPercent + 0xC4` is the knob's x**, which closes with the slider
//! handler's `g_levyPercent = mouseX - 0xC4` over a 101-pixel track: the two
//! are exact inverses, which is what makes [`SLIDER_X`] `[V]` rather than a
//! measurement.
//!
//! **The six weapon icons come out of the armoury's own sheet.** They are
//! `Pl8_DrawFrame(DAT_0056D5B8, i + 0x0F, …)`, and `DAT_0056D5B8` is the buffer
//! `Screen_Armoury` reads `arm_it_<colour>.pl8` into — frames 15…20 of the
//! twenty-one, the six 30 × 26 icons that sit below its troop portraits. This
//! module used to say *"the sheet is not one we load"*; it is, and it is
//! [`crate::screens::armoury::items_sheet`].
//!
//! **`realm + 0x138` is the realm's weapon total** and 69/14 is *"Total
//! weapons"* — `Realm_RecountWeapons` (`0x004487A9`) keeps it as the sum of the
//! six stocks, so we sum rather than store one.
//!
//! **`Eng_DrawString(69, 9)` is *"Continue"*, and it labels the one widget on
//! this screen that goes anywhere.** See below.
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
//! # The three widgets, and the two that are a pair
//!
//! `DAT_004DD340`, drawn and tested at an offset of `(0, 0x10)` when the county
//! has an offer and `(0, 0)` when it has not, with a count of 1 or 3:
//!
//! | x | y | frame | handler | |
//! |---:|---:|---:|---|---|
//! | 480 | 336 | 33 | `FUN_00435CBF` | **Continue** — `g_screenId = 0x0A` |
//! | 352 | 256 | 29 | `FUN_00435C89` | the tick: hire = yes |
//! | 400 | 260 | 31 | `FUN_00435C89` | the cross: hire = no |
//!
//! Only the first is always there; the tick and the cross appear when the
//! county has a band on offer **and the treasury can meet its price**, which is
//! the `DAT_00522F58 = 3` branch. The *"Yes"* / *"No"* word this screen prints
//! at `(0x1D0, base + 0x98)` is a **read-out of the flag, not a button** — it
//! is 88 pixels to the right of the cross and nothing tests it. This module
//! used to call that rectangle *"the original's own hotspot"*; it is
//! [`hire_readout`] now, and [`hire_yes`] and [`hire_no`] are the two that
//! click.
//!
//! # There is no Raise button here
//!
//! `Screen_FrameInput`'s `0x17` arm is `Levy_SliderClick()`, then — if that did
//! not take the click — a **right release** to `g_screenId = 0x0A` or the
//! corner picture to the campaign map. `Army_RaiseConfirm` is not reachable
//! from this screen at all: it is *Create*, a hotspot on the **armoury**.
//!
//! So the AUTO-EQUIP, UNEQUIP ALL, RAISE and CANCEL buttons this module used to
//! carry — ours, drawn below the original's window and labelled as ours — are
//! gone, and every one of them has a home now. Equipping is the armoury's
//! `+`/`−`, raising is *Create*, and cancelling is the corner picture. The one
//! thing with no counterpart was AUTO-EQUIP: `Levy_AutoEquip`'s round-robin is
//! the **AI's** function and no button in the game runs it, which is why the
//! address this module printed for it (`0x004AAD5F`) named nothing — it falls
//! inside `Battle_AutoResolve`. [`l2_kingdom::LevyBasket::auto_equip`] keeps
//! the rule for the AI and this screen no longer offers it.
//!
//! # Where the state lives
//!
//! Not here. The slider's percentage, the headcount, the happiness cost, the
//! basket and the hire flag are five globals in the original and one
//! [`crate::game::LevyOrder`] here, because the armoury reads and writes all
//! five and this screen is *replaced* on the way there.

use l2_kingdom::mercenary::ROSTER;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::armoury;
use crate::shell::{self, font, Pen};
use crate::widget;

/// `L2.eng` group 69 — this screen's own text, shared with the armoury.
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
/// g_levyPercent + 0xC4, …)` draws the knob and `Levy_SliderClick` reads
/// `g_levyPercent = mouseX - 0xC4` back out of it, over 101 pixels for 0…100.
pub const SLIDER_X: i32 = 0xC4;
/// Where the track stops and the right arrow begins.
pub const SLIDER_RIGHT: i32 = 0x129;
/// `Levy_SliderClick`'s hit band: `0x80 <= x < 0x176`, and `base + 0x10 <= y <
/// base + 0x40`. The three zones inside it are decrement, track, increment.
pub const SLIDER_HIT_X: (i32, i32) = (0x80, 0x176);
pub const SLIDER_HIT_DY: (i32, i32) = (0x10, 0x40);

/// `Ui_DrawInsetRect(0xC3, base + 0x28, 0x6F, 4)` — the well the knob runs in.
pub const WELL_X: i32 = 0xC3;
pub const WELL_W: i32 = 0x6F;
pub const WELL_H: i32 = 4;

/// The six weapon icons: icon at `0x70 + i * 0x48`, stock number `0x20` right
/// of it, on the row `base + (rows - 4) * 0x10`. **They are display only** —
/// nothing on this screen tests them, and the racks that do are the armoury's.
pub const RACK_X: i32 = 0x70;
pub const RACK_STEP: i32 = 0x48;
pub const RACK_NUMBER_DX: i32 = 0x20;
/// `Pl8_DrawFrame(DAT_0056D5B8, i + 0x0F, …)` — frames 15…20 of
/// `arm_it_<colour>.pl8`, the armoury's own sheet.
pub const ICON_FRAME_BASE: usize = 0x0F;

pub fn rack_row(offer: bool) -> i32 {
    base(offer) + (rows(offer) - 4) * 0x10
}

/// The row `69/14 "Total weapons"` and `69/9 "Continue"` share:
/// `(rows - 2) * 0x10 + base + 4`.
pub fn footer_row(offer: bool) -> i32 {
    (rows(offer) - 2) * 0x10 + base(offer) + 4
}

/// `Eng_DrawString(69, 9, 0x180, …)`.
pub const CONTINUE_LABEL_X: i32 = 0x180;

/// `L2.eng` group 100 — the county names, **twenty per scenario**, indexed
/// `scenarioIndex * 0x14 + county`. `Screen_RaiseArmy` prints one after 69/16.
pub const COUNTY_NAMES: usize = 100;
pub const COUNTY_NAMES_STRIDE: usize = 0x14;

pub fn county_name_index(ctx: &Ctx, county: u8) -> usize {
    ctx.game.map_slot * COUNTY_NAMES_STRIDE + county as usize
}

/// `System.pl8` frames the slider is built from, in the order the painter draws
/// them: the icon at each end, the arrow at each end, and the knob.
pub const SLIDER_LEFT_ICON: usize = 0x51;
pub const SLIDER_LEFT_ARROW: usize = 0x4E;
pub const SLIDER_KNOB: usize = 0x50;
pub const SLIDER_RIGHT_ARROW: usize = 0x4F;
pub const SLIDER_RIGHT_ICON: usize = 0x52;

/// `FUN_0040328E(group, index, x, y, 400, 100, 0, 0, …)` — the wrapped
/// paragraph's column. Both of this screen's two long sentences use it, and
/// drawing either as a single line runs it off the window.
pub const PARAGRAPH_W: i32 = 400;

/// The mercenary block's inset well, `0x60` tall with an offer and `0x32`
/// without.
pub fn merc_well(offer: bool) -> Rect {
    Rect::new(0x70, base(offer) + 0x58, 0x1A0, if offer { 0x60 } else { 0x32 })
}

/// `Widget_Draw(0, yOffset, &DAT_004DD340, …)` — the offset the whole table is
/// drawn and tested at. `0x10` with an offer, `0` without.
pub fn widget_offset(offer: bool) -> i32 {
    if offer {
        0x10
    } else {
        0
    }
}

/// **The Continue button** — record 0, frame 33, 24 pixels, at (480, 336) plus
/// the table offset. `FUN_00435CBF` sets `g_screenId = 0x0A` and re-seeds the
/// basket.
pub fn continue_button(offer: bool) -> Rect {
    Rect::new(480, 336 + widget_offset(offer), 24, 24)
}

/// The tick — record 1, frame 29, hotspot id 1, 32 pixels at (352, 256).
pub fn hire_yes(offer: bool) -> Rect {
    Rect::new(352, 256 + widget_offset(offer), 32, 32)
}

/// The cross — record 2, frame 31, hotspot id 0, 32 pixels at (400, 260).
pub fn hire_no(offer: bool) -> Rect {
    Rect::new(400, 260 + widget_offset(offer), 32, 32)
}

/// `Eng_DrawString(18, 0 or 1, 0x1D0, base + 0x98)` — **not a button.** The
/// word the flag prints, kept as a rectangle only so that the drawing code and
/// the test that says nothing tests it can name the same thing.
pub fn hire_readout(offer: bool) -> Rect {
    Rect::new(0x1D0, base(offer) + 0x98 - 2, 0x40, 20)
}

/// `Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x70, 1)` — stashed by
/// `Screen_Armoury`, which is the painter that ran underneath, so it is the
/// armoury's corner picture that closes this screen too. Both go to the map.
pub const OK: Rect = armoury::OK;

/// What the last confirm did. Kept so that a caller or a test can ask this
/// screen what happened even though the button that raises an army is now the
/// armoury's — [`crate::screens::armoury::Raised`] is where it comes from.
pub use crate::screens::armoury::Raised;

/// Screen `0x17` for one county.
///
/// **It owns almost nothing.** The percentage, the men, the cost, the basket
/// and the hire flag are [`crate::game::LevyOrder`], because `0x17 → 0x0A →
/// 0x17` destroys and rebuilds this screen twice and the original's globals
/// survive that.
pub struct RaiseArmyScreen {
    county: u8,
    /// One line of feedback. **Ours.**
    status: String,
}

impl RaiseArmyScreen {
    pub fn new(county: u8) -> RaiseArmyScreen {
        RaiseArmyScreen { county, status: "DRAG THE SLIDER, THEN CONTINUE".into() }
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
    /// numbers. Returns whether the click was the slider's.
    ///
    /// **It calls `Levy_SetPercent` and nothing else.** An earlier revision of
    /// this module said the slider also ran `FUN_004AA90A` and that *"re-seeding
    /// the basket is what makes a slider move throw away the equipment"*. The
    /// tail of the function is two statements and neither is that call. The
    /// equipment does get thrown away, but by the *door into the armoury*,
    /// which re-seeds on every entry — so the visible effect survives the
    /// correction and the mechanism does not.
    fn slider_click(&mut self, ctx: &mut Ctx, x: i32, y: i32) -> bool {
        let b = base(self.offer(&Ctx { game: ctx.game, assets: ctx.assets }) != 0);
        if !(SLIDER_HIT_X.0..SLIDER_HIT_X.1).contains(&x)
            || !((b + SLIDER_HIT_DY.0)..(b + SLIDER_HIT_DY.1)).contains(&y)
        {
            return false;
        }
        let percent = if x < SLIDER_X {
            ctx.game.levy.percent - 1
        } else if x < SLIDER_RIGHT {
            x - SLIDER_X
        } else {
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
        // rather than draw a levy of nobody.
        if ctx.game.levy.county != self.county {
            ctx.game.levy.county = self.county;
            let percent = ctx.game.levy.percent;
            ctx.game.set_levy_percent(percent);
            ctx.game.seed_levy_basket();
            ctx.game.levy.hire = false;
        }
        Transition::Stay
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
            // arm: 0x0042FF10/levy-right-commits
            Event::KeyDown(Key::Escape) => Transition::Pop,
            Event::RightClick { .. } | Event::KeyDown(Key::Enter) => self.open_armoury(ctx),
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
            Event::Click { x, y } => {
                if self.slider_click(ctx, x, y) {
                    return Transition::Stay;
                }
                if OK.contains(x, y) {
                    return Transition::Pop;
                }
                if continue_button(offer).contains(x, y) {
                    return self.open_armoury(ctx);
                }
                // The tick and the cross only exist on the affordable branch —
                // `DAT_00522F58` is 1 otherwise and `Widget_Test` never reaches
                // records 1 and 2.
                if offer && self.affordable(&Ctx { game: ctx.game, assets: ctx.assets }) {
                    if hire_yes(offer).contains(x, y) {
                        ctx.game.levy.hire = true;
                        return Transition::Stay;
                    }
                    if hire_no(offer).contains(x, y) {
                        ctx.game.levy.hire = false;
                        return Transition::Stay;
                    }
                }
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
        // stopped rather than how far it went.
        let x = pen.eng(canvas, GROUP, 0x10, RACK_X, b - 8, font::TEXT);
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
        text::draw(canvas, 0x80, b + 0x44, &format!("{}", population - levy.men), ink.text);
        text::draw_right(canvas, 0x145 + 40, b + 0x44, &format!("{}", levy.men), ink.highlight);
        text::draw(canvas, WELL_X, b + 0x14, &format!("{}%", levy.percent), ink.dim);

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
                    let n = troop.name().to_uppercase();
                    text::draw(canvas, x, y - 18, &n[..4.min(n.len())], ink.dim);
                }
            }
            text::draw(canvas, x + RACK_NUMBER_DX, y, &format!("{stock}"), ink.text);
        }

        // The happiness pair. The painter picks 69/10 + 69/11 when the levy
        // costs something and 69/12 + 69/13 when it does not, and prints the
        // county's happiness *after* the cost on the first branch.
        let (a, c, value) = if levy.happiness_cost < 1 {
            (12, 13, happiness)
        } else {
            (10, 11, happiness - levy.happiness_cost)
        };
        pen.eng(canvas, GROUP, a, 0x188, b + 0x18, font::TEXT);
        pen.eng(canvas, GROUP, c, 0x188, b + 0x30, font::TEXT);
        text::draw(canvas, 0x188, b + 0x40, &format!("{value}"), ink.text);

        // The mercenary block.
        let well = merc_well(on);
        shell::inset_rect(canvas, well.x, well.y, well.w, well.h);
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
            let x = pen.body(canvas, 0x70, b + 0x7C, &format!("{}", rules.price), font::TEXT);
            let x = pen.eng(canvas, GROUP, 0, x, b + 0x7C, font::TEXT);
            let x = pen.body(canvas, x, b + 0x7C, &format!("{}", rules.men / 2), font::TEXT);
            pen.eng(canvas, GROUP, 1, x, b + 0x7C, font::TEXT);

            if !self.affordable(ctx) {
                // 69/3, wrapped the same way: `FUN_0040328E(0x45, 3, 0x80,
                // base + 0x94, 400, 100, …)`.
                let s = ctx.assets.shell.text(GROUP, 3).to_string();
                pen.body_wrapped(canvas, 0x80, b + 0x94, PARAGRAPH_W, &s, font::TEXT);
            } else {
                pen.eng(canvas, GROUP, 2, 0x92, b + 0xA4, font::TEXT);
                // The word is a read-out; the tick and the cross are the
                // buttons.
                let yes_no = if levy.hire { 0 } else { 1 };
                let t = ctx.assets.shell.text(GROUP_YESNO, yes_no).to_string();
                let label = if t.is_empty() {
                    if levy.hire { "YES" } else { "NO" }.to_string()
                } else {
                    t
                };
                let r = hire_readout(on);
                pen.heading(canvas, r.x, r.y + 2, &label, font::TEXT);
                widget::button(canvas, ink, hire_yes(on), "YES", levy.hire);
                widget::button(canvas, ink, hire_no(on), "NO", !levy.hire);
            }
        } else {
            // 69/4, and it really is wrapped: `FUN_0040328E(0x45, 4, 0x80,
            // base + 0x60, 400, 100, 0, 0, …)` — a 400-pixel column, which is
            // what the sentence needs and what drawing it as one line does not
            // give it.
            let s = ctx.assets.shell.text(GROUP, 4).to_string();
            pen.body_wrapped(canvas, 0x80, b + 0x60, PARAGRAPH_W, &s, font::TEXT);
        }

        // The footer: the realm's weapon total, 69/14, and 69/9 "Continue"
        // beside the button that leaves for the armoury.
        let fy = footer_row(on);
        let total: i32 = realm.weapons.iter().sum();
        let x = pen.body(canvas, RACK_X, fy, &format!("{total}"), font::TEXT);
        pen.eng(canvas, GROUP, 14, x, fy, font::TEXT);
        pen.eng(canvas, GROUP, 9, CONTINUE_LABEL_X, fy, font::TEXT);
        let cont = continue_button(on);
        widget::frame(canvas, cont, ink.highlight);

        text::draw(canvas, BOX_X, w.y + w.h + 4, &self.status, ink.dim);
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
            assert!((SLIDER_X..SLIDER_RIGHT).contains(&knob), "{pct} lands off the track");
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

    /// Six icons, evenly pitched, none of them overlapping and all of them
    /// inside the window.
    #[test]
    fn the_six_weapon_icons_are_inside_the_window() {
        for on in [true, false] {
            let w = window(on);
            let y = rack_row(on);
            for i in 0..6i32 {
                let x = RACK_X + i * RACK_STEP;
                assert!(w.contains(x, y - 4), "icon {i} is outside the window");
                assert!(w.contains(x + RACK_NUMBER_DX + 20, y), "icon {i}'s number runs off it");
            }
        }
    }

    /// **The widgets are the original's, and the read-out is not one of them.**
    /// The word "Yes" sits 88 pixels right of the cross; if a future edit makes
    /// the read-out clickable, this is what says the original did not.
    #[test]
    fn the_hire_readout_is_not_where_the_tick_and_the_cross_are() {
        for on in [true, false] {
            let (yes, no, word) = (hire_yes(on), hire_no(on), hire_readout(on));
            assert!(yes.x + yes.w <= no.x, "the tick and the cross overlap");
            assert!(no.x + no.w <= word.x, "the read-out overlaps the cross");
            assert_eq!(word.x - (no.x + no.w), 32, "the gap the painter leaves");
        }
    }

    /// The Continue button is inside the original's window, is clear of the
    /// slider's hit band, and moves with the table offset rather than with the
    /// layout's base.
    #[test]
    fn the_continue_button_is_the_only_way_on_and_it_is_inside_the_window() {
        for on in [true, false] {
            let (w, c) = (window(on), continue_button(on));
            assert!(w.contains(c.x, c.y), "Continue is outside the window");
            assert!(w.contains(c.x + c.w - 1, c.y + c.h - 1), "Continue runs off it");
            assert!(
                c.y >= base(on) + SLIDER_HIT_DY.1 || c.x >= SLIDER_HIT_X.1,
                "Continue is inside the slider's hit band, which is tested first",
            );
        }
        // The two positions differ by exactly the table offset and nothing
        // else: the original moves the widget table, not the widget.
        assert_eq!(continue_button(true).y - continue_button(false).y, 0x10);
    }

    /// The corner picture is the armoury's, because the armoury is the painter
    /// that ran underneath and `Ui_OkButton` stashes only its last call.
    #[test]
    fn the_close_button_is_the_one_the_armoury_stashed() {
        assert_eq!(OK.x, 640 - 0x1C);
        assert_eq!(OK.y, 480 - 0x70);
        for on in [true, false] {
            let w = window(on);
            assert!(OK.x >= w.x + w.w || OK.y >= w.y + w.h, "the corner picture is under the box");
        }
    }
}
