//! **The siege-preparation screen** — `Screen_SiegePrep` (`0x00421F14`),
//! `g_screenId` `0x1D`, `L2.eng` group 83.
//!
//! It was one of the shells in [`crate::screens::shells`]: it drew the window
//! and the words and did nothing. It is the only place a player ever chooses
//! what to build for a siege, so a siege that could be laid but not equipped
//! stopped here.
//!
//! # The painter, address by address
//!
//! Transcribed from `Screen_SiegePrep` (`0x00421F14`), with the coordinates
//! resolved to decimal in the trailing comment. **`FUN_0040437D(x, y, w, h, c)`
//! is a filled rectangle** — a loop of `FUN_00403A8F` horizontal lines — and
//! **`FUN_004093E0` is `Ui_DrawBoxBorder(1, …)` plus `Ui_DrawBoxInterior`**, so
//! the window is border set **1**, not 0. Neither is in the draw-call
//! extractor's primitive list, which is why the mechanical count of this
//! painter (28) is eight short of the truth.
//!
//! ```text
//! Screen_SiegePrep():                                            0x00421F14
//!   FUN_004093E0(0x10, 0x30, 0x1C, 0x19)      window, set 1, (16, 48) 448 x 400
//!   File_ReadChunk("sgeplans.pl8", spriteBuffer, 100000, 0)
//!   DAT_0058FE2C := 1                         drop capitals on for the whole screen
//!   if county.castleType != 0:
//!     Sprite_WGenSprite(castleType - 1, 0x150, 0x40)       (336, 64) the castle plan
//!   Eng_DrawString(83, 0, 0x30, 0x58, heading)   "Siege preparations."   (48, 88)
//!   Eng_DrawString(83, 4, 0x40, 0x78, body)      "Siege will take"       (64, 120)
//!   Ui_DrawCount(unit +0x19C, 0x42, 0x50, 0x88)  N Season/Seasons        (80, 136)
//!   Eng_DrawString(83, 5, pen + 0x20, 0x88)      "to make ready."   y 136, after it
//!   for row in 0..3, y = 0xC0 + 0x44 * row:                    192, 260, 328
//!     Eng_DrawString(83, 1 + row, 0x48, y)       the engine's name       x 72
//!     Ui_DrawInsetRect(0x50, y + 0x18, 0x34, 8)  the well, FOUR LINES AND NO FILL
//!     FUN_0040437D(0x51, y + 0x19, 0x32, 6, 0xF9)          the empty bar, 50 wide
//!     FUN_0040437D(0x51, y + 0x19, record[+2] / 2, 6, 0xFA)     the fill, percent/2
//!     Ui_DrawNumber(record[+2], '@', "%", 0x90, y + 0x19)  the percentage   x 144
//!     if record[+0] == 0:
//!       Eng_DrawString(83, 8, 0xF0, y + 0x10)  "- No engines to be built"  x 240
//!     else for n in 0..record[+0]:
//!       Pl8_DrawFrame(misc_cty, 0x43 + row, 0xD0 + n * step, y - 8)   step 60/60/80
//!   Ui_DrawBevelRect(0x68, 0x18C, 100, 0x1C)                       (104, 396)
//!   FUN_0040437D(0x69, 0x18D, 0x62, 0x1A, 0x18)              the button's fill
//!   Eng_DrawString(83, 6, 0x70, 0x194)          "Lift siege"        (112, 404)
//!   Ui_DrawBevelRect(0x108, 0x18C, 100, 0x1C)                      (264, 396)
//!   FUN_0040437D(0x109, 0x18D, 0x62, 0x1A, 0x18)
//!   Eng_DrawString(83, 7, 0x110, 0x194)         "Proceed"           (272, 404)
//!   DAT_0058FE2C := 0
//! ```
//!
//! Three coordinates in the listing this module used to carry were wrong, and
//! all three are the same slip: *"- No engines to be built"* is `y + 0x10`,
//! **below** its label, and the engine sprites are `y - 8`, not `y - 0x10`.
//!
//! # The screen drew nothing through the game's own assets until this audit
//!
//! Every line of it was `l2_view::text::draw` — our 5 × 7 debug font — with
//! the English typed into the source, against a painter that makes eleven
//! `Eng_DrawString` calls on `L2.eng` group 83 and three `Pl8_DrawFrame`s.
//! Group 83 reads, in index order: *"Siege preparations."*, *"Catapults"*,
//! *"Siege towers"*, *"Battering rams"*, *"Siege will take"*, *"to make
//! ready."*, *"Lift siege"*, *"Proceed"*, *"- No engines to be built"* — nine
//! strings, and the words are what identifies the group, not the fact that
//! nine indices exist.
//!
//! Two further defects came out of the same reading and are fixed here:
//! **the window was drawn with border set 0** (the painter's `FUN_004093E0`
//! passes 1), and **the percent bar's well was filled before it was framed**,
//! which `Ui_DrawInsetRect` does not do — the exact defect `docs/decisions.md`
//! C61 records against the raise-army screen, where the fill is invisible
//! under our palette and pitch black under the game's.
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
use crate::shell::{font, Pen};
use crate::widget;

/// **`L2.eng` group 83.** Verified against the *words*, not against the indices
/// existing: index 0 is *"Siege preparations."* and 1…3 are *"Catapults"*,
/// *"Siege towers"*, *"Battering rams"* in the order the three engine records
/// sit in the unit. `docs/formats/eng.md` §5 files the group here too.
pub const GROUP: usize = 83;
pub const HEADING: usize = 0;
/// 1, 2, 3 — the three engine names, `ENGINE_LABEL[row]`.
pub const ENGINE_LABEL: [usize; 3] = [1, 2, 3];
/// *"Siege will take"* and *"to make ready."*, the two halves of one sentence
/// with a [`Ui_DrawCount`](Pen::count) between them.
pub const WILL_TAKE: usize = 4;
pub const TO_MAKE_READY: usize = 5;
pub const LIFT_SIEGE: usize = 6;
pub const PROCEED_LABEL: usize = 7;
pub const NO_ENGINES: usize = 8;

/// `Ui_DrawCount(seasons, 0x42, …)` — **group 8**, not group 83: index 66
/// *"Season"* and 67 *"Seasons"*. The singular/plural rule is
/// [`crate::shell::count_noun`] — `±1` takes the singular and everything else,
/// **zero included**, takes the plural.
pub const SEASON_NOUN: usize = 0x42;

/// `File_ReadChunk("sgeplans.pl8", …)` then
/// `Sprite_WGenSprite(castleType - 1, …)` — the sprite index is a frame of that
/// sheet, and `castleType` runs 1…5, so the frames are 0…4. **Unverified
/// against the artwork**: no fixture has a besieged county, so which picture is
/// which castle level rests on the subtraction alone.
pub const PLAN_SHEET: &str = "Sgeplans.pl8";
pub const PLAN_AT: (i32, i32) = (0x150, 0x40);

/// `Pl8_DrawFrame(g_miscCtySheet, 0x43 + row, …)` — `Misc_cty.pl8` in campaign
/// mode, one little picture per engine ordered.
pub const ENGINE_FRAME0: usize = 0x43;

/// `FUN_004093E0(0x10, 0x30, 0x1C, 0x19)` — the window: origin in **pixels**,
/// size in 16-pixel **cells**, which is the call's own mixed convention and the
/// same one `Panel_JobDetail` uses. 448 × 400 at (16, 48).
pub const BOX_X: i32 = 0x10;
pub const BOX_Y: i32 = 0x30;
pub const BOX_COLS: i32 = 0x1C;
pub const BOX_ROWS: i32 = 0x19;
/// **Border set one.** `FUN_004093E0`'s whole body is
/// `Ui_DrawBoxBorder(1, x, y, cols, rows)` followed by `Ui_DrawBoxInterior`
/// inset one cell; passing 0 draws the other frame kit and a visibly wrong
/// window. `screens/battle.rs` carries the same constant for the same reason.
pub const BOX_SET: usize = 1;

/// `Eng_DrawString(83, 0, 0x30, 0x58, heading)`.
pub const HEADING_AT: (i32, i32) = (0x30, 0x58);
/// `Eng_DrawString(83, 4, 0x40, 0x78, body)`.
pub const WILL_TAKE_AT: (i32, i32) = (0x40, 0x78);
/// `Ui_DrawCount(seasons, 0x42, 0x50, 0x88, body)`, and *"to make ready."*
/// follows it on the same line.
pub const SEASONS_AT: (i32, i32) = (0x50, 0x88);

/// The y of each engine row's label — `0xC0`, `0x104`, `0x148`, and the rows
/// are 0x44 apart.
pub const ROW_Y: [i32; 3] = [0xC0, 0x104, 0x148];
/// `Eng_DrawString(83, 1 + row, 0x48, …)`.
pub const LABEL_X: i32 = 0x48;
/// `Ui_DrawInsetRect(0x50, rowY + 0x18, 0x34, 8)` — the percent bar's well.
/// **Four lines and no fill**; see [`crate::shell::inset_rect`].
pub const BAR_X: i32 = 0x50;
pub const BAR_W: i32 = 0x34;
pub const BAR_H: i32 = 8;
pub const BAR_DY: i32 = 0x18;
/// `FUN_0040437D(0x51, rowY + 0x19, 0x32, 6, 0xF9)` then the same rectangle
/// `percent / 2` wide in `0xFA` — the trough is 50 pixels inside a 52-pixel
/// well, which is what makes `percent / 2` reach exactly the far end at 100.
pub const TROUGH: (i32, i32, i32, i32) = (0x51, 0x19, 0x32, 6);
pub const TROUGH_EMPTY: u8 = 0xF9;
pub const TROUGH_FULL: u8 = 0xFA;
/// `Ui_DrawNumber(percent, '@', "%", 0x90, rowY + 0x19)` — the suffix at
/// `0x004D4404` is one byte, `0x25`, the per-cent sign.
pub const PERCENT_X: i32 = 0x90;
pub const PERCENT_DY: i32 = 0x19;
/// `Eng_DrawString(83, 8, 0xF0, rowY + 0x10)` — **below** the engine's name,
/// not above it.
pub const NO_ENGINES_AT: (i32, i32) = (0xF0, 0x10);
/// Where the ordered engines' sprites go — `0xD0` plus 0x3C a piece for the
/// first two rows and 0x50 for the rams, which are wider, at `rowY - 8`.
pub const SPRITE_X: i32 = 0xD0;
pub const SPRITE_STEP: [i32; 3] = [0x3C, 0x3C, 0x50];
pub const SPRITE_DY: i32 = -8;
/// `Eng_DrawString(83, 6 | 7, boxX + 8, 0x194)` — the caption inside each bevel.
pub const BUTTON_LABEL_DX: i32 = 8;
pub const BUTTON_LABEL_Y: i32 = 0x194;
/// `FUN_0040437D(x + 1, y + 1, 0x62, 0x1A, 0x18)` — the plate inside the bevel.
pub const BUTTON_FILL: u8 = 0x18;

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
///
/// **The table holds exactly six records and every caller passes six.** It was
/// decoded to ten to look for `g_sendSuppliesWidgets`' cut row: records 6…9
/// are `g_smackTestWidgets` (`0x004DDFA0`) — the minus/plus pair at (208, 232)
/// and (240, 232) and the yes/no pair at (288, 280) and (324, 284), which
/// `docs/symbols.json` describes independently — so there is nothing hidden
/// behind this count.
pub const BUTTON_X: i32 = 38;
pub const BUTTON_Y: [i32; 3] = [184, 252, 320];
/// The decrement button's offset below its partner.
pub const BUTTON_STEP: i32 = 26;
/// The button sprites are **`System.pl8`** frames 21 and 23, and this module
/// used to say `Panels.pl8`. `Widget_Draw` (`0x0040CFD2`) picks the sheet from
/// the record's *size* field: below `0x18` it draws from `g_miscCtySheet` and
/// otherwise from `g_systemSheet`, and these six records carry 24. The size
/// **is** in the table, so the 24 × 24 box is the original's too.
pub const BUTTON_DIM: i32 = 24;
/// The frame each button draws, and `frame + 1` while it is held —
/// `Widget_Draw` adds one for as long as the press timer at `+0x0D` runs.
pub const WIDGET_FRAME_PLUS: usize = 21;
pub const WIDGET_FRAME_MINUS: usize = 23;

/// **`Ui_DrawBevelRect(x, y, w, h)` (`0x00403FDD`) — four lines and no fill**,
/// top and right in palette `0x1F` and bottom and left in `0x10`.
///
/// That is the exact inverse of [`crate::shell::inset_rect`]'s lighting, which
/// is what makes one read as raised and the other as recessed, and it is the
/// whole of the function. It lives here rather than in [`crate::shell`] beside
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
        // written out here rather than through [`Pen::count`] because the
        // original's own `g_penAdvance` is what places the tail of the
        // sentence.
        p.eng(canvas, GROUP, WILL_TAKE, WILL_TAKE_AT.0, WILL_TAKE_AT.1, font::TEXT);
        let seasons = unit.siege_seasons_left;
        let end = p.number(canvas, SEASONS_AT.0, SEASONS_AT.1, seasons as i32, false, font::TEXT);
        let noun = crate::shell::count_noun(seasons as i32, SEASON_NOUN);
        let end = p.eng(canvas, crate::shell::COUNT_NOUN_GROUP, noun, end, SEASONS_AT.1, font::TEXT);
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
            // `Ui_DrawNumber(percent, '@', "%", 0x90, y + 0x19)`. The `'@'`
            // lead is the blank alignment glyph, which [`Pen::number`] does not
            // model and which no `Pen` method can carry a `%` suffix through.
            p.body(
                canvas,
                PERCENT_X,
                y + PERCENT_DY,
                &format!("{}%", record.percent),
                font::TEXT,
            );

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
