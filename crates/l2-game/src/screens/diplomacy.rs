//! **Diplomacy** — `Diplo_DrawScreen` (`0x00416CF3`), `g_screenId` `0x0B`.
//!
//! # The name in the shell table named half of it
//!
//! It was *"The other lords"*, which is the left-hand column. `L2.eng` group 72
//! index 0 is literally **`"Diplomacy."`**, every symbol on the screen is
//! `Diplo_*`, and the right-hand half is a menu of six actions. The group
//! number the table carried — 72 — is **correct**, verified as the literal
//! argument at all fourteen of the painter's `Eng_DrawString` sites.
//!
//! # Where this screen stops
//!
//! Every one of the six buttons opens **screen `0x1A`**, `Screen_DiploDialog`,
//! with `g_diploKind` 0…6. That screen is not built and is not this module's:
//! the gift, the compliment, the insult, the alliance offer and the two ally
//! requests are all a different agent's. **The seam is deliberate and it is the
//! original's own**: this painter draws the cards and the menu and nothing
//! else, and the dialog is a separate `g_screenId`. What this module owns is
//! the panel; what it hands on is a [`Action`].
//!
//! # The painter, address by address
//!
//! ```text
//! Diplo_DrawScreen():                                           0x00416CF3
//!   if (target == 0 || !realms[target].inPlay) Diplo_DefaultTarget()
//!   g_diploMenuState = pair.hasMail ? 3
//!                    : realms[me].ally == target ? 1
//!                    : realms[me].ally == 0      ? 0 : 2
//!   FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)     border set 1 at (16, 32) 448x432
//!   Ui_OkButton(0x1A8, 0x1A6, 0)                                 (424, 422)
//!   File_ReadChunk("faces.pl8", scratch, 100000, 0)
//!   for realm in 1..=5, skipping me and the eliminated:
//!       Diplo_DrawLordCard(realm, slot++)                        0x004171EE
//!   Ui_DrawText(playerNames[target], 0xD0, 0x3D, heading)        (208, 61)
//!   one of the four menu layouts
//!   FUN_0045240A()
//! ```
//!
//! The window's right edge is 464, so the 176-pixel sidebar stays visible —
//! which is why `overlay: true` and why the sidebar buttons keep working with
//! this screen up.
//!
//! **`shells.rs` said `heading: None`.** The painter draws the selected lord's
//! name at (208, 61) in the 22-pixel font. It is a `g_playerNames` string
//! rather than an `L2.eng` line, which is why it did not fit the shell's `Line`
//! type — but *"no heading"* is not what the painter does.
//!
//! # The lord card — `Diplo_DrawLordCard` (`0x004171EE`)
//!
//! `s` is the **slot**, packed over the rivals still in play, not the realm id.
//!
//! ```text
//!   Ui_DrawInsetRect(0x30, s*100 + 0x31, 0x52, 0x4E)      (48, 49 + 100s) 82x78
//!   FUN_0040A682(frame, 0x31, s*100 + 0x32)               portrait (49, 50 + 100s)
//!         frame = realm.lord * 3 - 3 for an AI, 12 for the human
//!   Pl8_DrawFrame(Misc_cty, shieldIndex + 0x55, 0x20, s*100 + 0x37)   (32, 55 + 100s)
//!   Ui_DrawText(playerNames[realm], 0x20, s*100 + 0x83, body)         (32, 131 + 100s)
//!   selected: FUN_00403CF4(0x2F, s*100 + 0x30, 0x54, 0x50, 0xF9)   inner ring
//!             FUN_00403CF4(0x2E, s*100 + 0x2F, 0x56, 0x52, 0x3F)   outer ring
//!   hasMail:  FUN_0040A682(0x0F, 0x1E, s*100 + 0x50)      faces 15  (30, 80 + 100s)
//!   allied:   FUN_0040A682(0x0D, 0x9E, s*100 + 0x41)      faces 13 (158, 65 + 100s)
//!   at war:   FUN_0040A682(0x0E, 0x92, s*100 + 0x41)      faces 14 (146, 65 + 100s)
//!   AI only:  Ui_DrawInsetRect(0x88, s*100 + 0x40, 10, 0x3F)   the thermometer
//!             FUN_0040437D(0x89, s*100 + 0x41, 8, 0x3D, 0x3F)   empty
//!             for v = 30 down to -30, row = 0..60:
//!               if (v <= standing) FUN_0040437D(0x89, s*100 + row + 0x41, 8, 1, c)
//!               c = 0xFA if standing >= 11, 0xF9 if standing < -10, else 0xFC
//! ```
//!
//! **The thermometer is the whole of the lord's opinion in one column**: sixty
//! one-pixel rows from +30 at the top to −30 at the bottom, filled up to the
//! standing, in one of three colours by band. It is drawn for an **AI only** —
//! a human rival has no meter, which is the original saying that a person's
//! intentions are not readable.
//!
//! # The four menu layouts
//!
//! `g_diploMenuState` (`0x00553F38`) is written **only** by the painter's
//! prologue and read **only** by it. No input path consults it; the widget
//! count `g_diploWidgetCount` is what makes the buttons match the layout.
//!
//! | state | when | inset | buttons | group 72 lines |
//! |---|---|---|---|---|
//! | 0 | I have no ally | (208, 96) 232 × 208 | 4 | 2, 3, 4, 5 at y 112, 162, 212, 262 |
//! | 1 | this lord **is** my ally | (208, 96) 232 × 304 | 6 | 2, 3, 4, **6, 7, 8** at y 112 … 362 |
//! | 2 | I am allied elsewhere | (208, 96) 232 × 160 | 3 | 2, 3, 4 |
//! | 3 | a letter from this lord is waiting | none | 0 | **24** at y 162 alone |
//!
//! Every line is drawn by `FUN_0040328E` at x 224, **word-wrapped at 160
//! pixels** — not `Eng_DrawString` — so *"Send a compliment."* can take two
//! lines at a 16-pixel advance. `Misc_cty` frame `0x1D` sits at (320, 320) in
//! every state **except 1**, where the taller inset covers it.
//!
//! **In state 3 there are zero widgets and the card picker still works**, so a
//! lord with a pending letter is never a dead end: click another card and the
//! menu comes back next frame.
//!
//! # The buttons, and the twenty-frame delay
//!
//! `g_diploWidgets` (`0x004DD940`), six 24-byte records, all at x 400, 32
//! pixels square, `System.pl8` frame 64:
//!
//! | i | y | handler | kind |
//! |---|---|---|---|
//! | 0 | 102 | `Diplo_OpenGift` `0x00436141` | gift |
//! | 1 | 152 | `Diplo_OpenCompliment` `0x0043618B` | compliment |
//! | 2 | 202 | `Diplo_OpenInsult` `0x004361DA` | insult |
//! | 3 | 252 | `Diplo_OpenAlliance` `0x00436229` | **offer *or* terminate** |
//! | 4 | 302 | `Diplo_OpenAskHelp` `0x004362CA` | ask for help |
//! | 5 | 352 | `Diplo_OpenAskAttack` `0x0043631E` | ask to attack |
//!
//! **One button serves two menu rows.** `Diplo_OpenAlliance` re-tests
//! `ally == target` and picks `g_diploKind` 4 (terminate) over 3 (offer), which
//! is why row 5 and row 6 of the two layouts share widget 3. Every button sits
//! **ten pixels above** its caption.
//!
//! `Widget_Test` **kind 5 is deferred**: a left press plays a sound, sets a
//! twenty-frame timer and returns *consumed* without calling the handler; the
//! handler fires when the timer runs out. We fire at once. That is an
//! invention, it is recorded as one, and it is the same one the court makes.
//!
//! # The card picker
//!
//! `FUN_004369BD` — on a left **press**, test `(48, 49 + 100s, 82, 78)` for
//! each in-play rival and set `g_diploTarget`. It is the **only** place a
//! player sets the target, and it is tested *after* the widgets.
//!
//! # How it is reached
//!
//! `FUN_0043611B`, from `Sidebar_Button`'s hotspot id 5 — **x 608…638, y
//! 430…458** — and it is **ungated**. It zeroes `g_diploGold` and does *not*
//! touch `g_diploTarget`; the painter's prologue heals a stale one.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `L2.eng` group 72.
pub const GROUP: usize = 72;
pub const GIFT: usize = 2;
pub const COMPLIMENT: usize = 3;
pub const INSULT: usize = 4;
pub const OFFER_ALLIANCE: usize = 5;
pub const END_ALLIANCE: usize = 6;
pub const ASK_HELP: usize = 7;
pub const ASK_ATTACK: usize = 8;
pub const DISPATCHED: usize = 24;

/// `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)`.
pub const BOX: (i32, i32, i32, i32, usize) = (0x10, 0x20, 0x1C, 0x1B, 1);
pub const OK: Rect = Rect::new(0x1A8, 0x1A6, 24, 24);
/// `Ui_DrawText(playerNames[target], 0xD0, 0x3D, heading)`.
pub const TARGET_NAME_AT: (i32, i32) = (0xD0, 0x3D);

/// One lord card's geometry, as a function of the **slot**.
pub const CARD_PITCH: i32 = 100;
pub const CARD: Rect = Rect::new(0x30, 0x31, 0x52, 0x4E);
pub const PORTRAIT_AT: (i32, i32) = (0x31, 0x32);
pub const SHIELD_AT: (i32, i32) = (0x20, 0x37);
pub const CARD_NAME_AT: (i32, i32) = (0x20, 0x83);
/// `Misc_cty.pl8` frame `shieldIndex + 0x55`.
pub const SHIELD_FRAME0: usize = 0x55;
/// The standing thermometer's well and its 8 x 61 column.
pub const METER: Rect = Rect::new(0x88, 0x40, 10, 0x3F);
pub const METER_BAR: Rect = Rect::new(0x89, 0x41, 8, 0x3D);
/// Sixty rows from +30 down to −30.
pub const METER_TOP: i32 = 30;
pub const METER_ROWS: i32 = 61;
/// The three colours: friendly, neutral, hostile.
pub const METER_GOOD: u8 = 0xFA;
pub const METER_NEUTRAL: u8 = 0xFC;
pub const METER_BAD: u8 = 0xF9;
/// `FUN_00403CF4`'s two selection outlines: colour `0xF9` inside `0x3F`.
pub const RING_INNER: (i32, i32, i32, i32, u8) = (0x2F, 0x30, 0x54, 0x50, 0xF9);
pub const RING_OUTER: (i32, i32, i32, i32, u8) = (0x2E, 0x2F, 0x56, 0x52, 0x3F);

/// The card's rectangle for slot `s`, which is also its hit box —
/// `FUN_004369BD` tests the inset byte for byte.
pub fn card_rect(slot: usize) -> Rect {
    Rect::new(CARD.x, CARD.y + slot as i32 * CARD_PITCH, CARD.w, CARD.h)
}

/// What one menu row asks for. **`0x1A` is not built**, so this is where this
/// screen stops and the diplomacy agent's begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Gift,
    Compliment,
    Insult,
    OfferAlliance,
    EndAlliance,
    AskHelp,
    AskAttack,
}

/// `Diplo_OpenAlliance`'s re-test: one widget, two verbs.
impl Action {
    /// `g_diploKind`, the byte `Screen_DiploDialog` switches on.
    pub fn kind(self) -> u8 {
        match self {
            Action::Gift => 0,
            Action::Compliment => 1,
            Action::Insult => 2,
            Action::OfferAlliance => 3,
            Action::EndAlliance => 4,
            Action::AskHelp => 5,
            Action::AskAttack => 6,
        }
    }
}

/// `g_diploMenuState` — the four layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// 0 — I have no ally.
    Free,
    /// 1 — this lord is my ally.
    Allied,
    /// 2 — I am allied to somebody else.
    AlliedElsewhere,
    /// 3 — a letter from this lord is waiting; the menu is one line of prose.
    Mail,
}

impl Menu {
    /// The painter's prologue, in its own order.
    pub fn of(has_mail: bool, my_ally: u8, target: u8) -> Menu {
        if has_mail {
            Menu::Mail
        } else if my_ally == target && target != 0 {
            Menu::Allied
        } else if my_ally == 0 {
            Menu::Free
        } else {
            Menu::AlliedElsewhere
        }
    }

    /// The rows, top to bottom. `Menu::Mail`'s single line is index 24 and is
    /// not an action, so it is not here.
    pub fn rows(self) -> &'static [(usize, Action)] {
        match self {
            Menu::Free => &[
                (GIFT, Action::Gift),
                (COMPLIMENT, Action::Compliment),
                (INSULT, Action::Insult),
                (OFFER_ALLIANCE, Action::OfferAlliance),
            ],
            Menu::Allied => &[
                (GIFT, Action::Gift),
                (COMPLIMENT, Action::Compliment),
                (INSULT, Action::Insult),
                (END_ALLIANCE, Action::EndAlliance),
                (ASK_HELP, Action::AskHelp),
                (ASK_ATTACK, Action::AskAttack),
            ],
            Menu::AlliedElsewhere => &[
                (GIFT, Action::Gift),
                (COMPLIMENT, Action::Compliment),
                (INSULT, Action::Insult),
            ],
            Menu::Mail => &[],
        }
    }

    /// `Ui_DrawInsetRect(208, 96, 232, h)` — the menu's well, or none.
    pub fn well(self) -> Option<Rect> {
        let h = match self {
            Menu::Free => 208,
            Menu::Allied => 304,
            Menu::AlliedElsewhere => 160,
            Menu::Mail => return None,
        };
        Some(Rect::new(208, 96, 232, h))
    }
}

/// The line x and the fifty-pixel row pitch, and the wrap width.
pub const LINE_X: i32 = 224;
pub const LINE_Y0: i32 = 112;
pub const LINE_PITCH: i32 = 50;
pub const LINE_WRAP: i32 = 0xA0;
/// The button column: x 400, ten pixels above its caption, 32 square.
pub const BUTTON_X: i32 = 400;
pub const BUTTON_LIFT: i32 = 10;
pub const BUTTON_DIM: i32 = 32;
pub const BUTTON_FRAME: usize = 64;
/// `Misc_cty` frame `0x1D` at (320, 320), in every state but [`Menu::Allied`].
pub const ORNAMENT: (usize, i32, i32) = (0x1D, 320, 320);

pub fn button_rect(row: usize) -> Rect {
    Rect::new(BUTTON_X, LINE_Y0 + row as i32 * LINE_PITCH - BUTTON_LIFT, BUTTON_DIM, BUTTON_DIM)
}

pub struct DiplomacyScreen {
    /// `g_diploTarget`. 0 until the first draw heals it, exactly as the
    /// original's prologue does.
    target: u8,
    /// The last action a button asked for, for a test and for the note.
    pub asked: Option<Action>,
}

impl DiplomacyScreen {
    pub fn new() -> DiplomacyScreen {
        DiplomacyScreen { target: 0, asked: None }
    }

    pub fn target(&self) -> u8 {
        self.target
    }

    /// The rivals still in play, in realm order, skipping the local player.
    /// **The slot is the position in this list**, not the realm id.
    pub fn rivals(ctx: &Ctx) -> Vec<u8> {
        let k = &ctx.game.kingdom;
        (1..k.realms.len() as u8)
            .filter(|&r| r != ctx.game.player)
            .filter(|&r| k.realms.get(r as usize).is_some_and(|r| r.in_play))
            .collect()
    }

    /// `Diplo_DefaultTarget` (`0x004A1E6C`) — the first realm in play that is
    /// not me, or zero.
    fn heal_target(&mut self, ctx: &Ctx) {
        let alive = ctx
            .game
            .kingdom
            .realms
            .get(self.target as usize)
            .is_some_and(|r| r.in_play);
        if self.target == 0 || !alive {
            self.target = Self::rivals(ctx).first().copied().unwrap_or(0);
        }
    }

    fn menu(&self, ctx: &Ctx) -> Menu {
        let k = &ctx.game.kingdom;
        let my_ally = k.realms.get(ctx.game.player as usize).map_or(0, |r| r.ally);
        // `pair(me -> them).hasMail`, which is the painter.s own direction.
        let has_mail = k
            .realms
            .get(ctx.game.player as usize)
            .is_some_and(|r| r.pair(self.target).has_mail);
        Menu::of(has_mail, my_ally, self.target)
    }
}

impl Default for DiplomacyScreen {
    fn default() -> DiplomacyScreen {
        DiplomacyScreen::new()
    }
}

impl Screen for DiplomacyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Diplomacy
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Diplomacy — screen 0x0B".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let Event::Click { x, y } = event else {
            return match event {
                // arm: 0x0042FF10/diplo-right
                Event::RightClick { .. } => Transition::Pop,
                // arm: ours/diplomacy-keyboard-close
                Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
                _ => Transition::Stay,
            };
        };
        // arm: 0x0042FF10/diplo-ok
        if OK.contains(x, y) {
            return Transition::Pop;
        }
        // `Screen_HandleInput` tests the widgets **first**, then the cards.
        let rows = self.menu(ctx).rows();
        for (i, &(_, action)) in rows.iter().enumerate() {
            if button_rect(i).contains(x, y) {
                // `Diplo_Open*` — every one of them is `g_screenId = 0x1A` with
                // `g_diploKind` set. `0x1A` is not built, so the ask is
                // recorded and the screen stays up.
                // arm: 0x00436141/diplo-action
                self.asked = Some(action);
                return Transition::Stay;
            }
        }
        // `FUN_004369BD` — the card picker, the only writer of `g_diploTarget`.
        // arm: 0x004369BD/diplo-pick-lord
        for (slot, &realm) in Self::rivals(ctx).iter().enumerate() {
            if card_rect(slot).contains(x, y) {
                self.target = realm;
                self.asked = None;
                return Transition::Stay;
            }
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        self.heal_target(ctx);
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        pen.window(canvas, BOX.0, BOX.1, BOX.2, BOX.3, BOX.4);
        pen.ok_button(canvas, OK.x, OK.y, 0);

        // The lord cards down the left.
        for (slot, &realm) in Self::rivals(ctx).iter().enumerate() {
            let r = card_rect(slot);
            let top = slot as i32 * CARD_PITCH;
            pen.inset(canvas, r);
            let record = ctx.game.kingdom.realms.get(realm as usize);
            pen.misc_frame(
                canvas,
                SHIELD_FRAME0 + record.map_or(1, |r| r.shield_index.clamp(1, 5)) as usize,
                SHIELD_AT.0,
                SHIELD_AT.1 + top,
            );
            // **The portrait is `faces.pl8`, which this crate does not load.**
            // The card names the lord instead of drawing him, which is visibly
            // ours; `PORTRAIT_AT` and the frame rule are recorded above so the
            // next reader has the whole of it.
            let _ = PORTRAIT_AT;
            l2_view::text::draw(
                canvas,
                CARD_NAME_AT.0,
                CARD_NAME_AT.1 + top,
                &format!("LORD {realm}"),
                ink.text,
            );

            if realm == self.target {
                // The two outlines `FUN_00403CF4` draws, inner then outer.
                for &(x, y, w, h, c) in &[RING_INNER, RING_OUTER] {
                    canvas.fill_rect(x, y + top, w, 1, c);
                    canvas.fill_rect(x, y + top + h - 1, w, 1, c);
                    canvas.fill_rect(x, y + top, 1, h, c);
                    canvas.fill_rect(x + w - 1, y + top, 1, h, c);
                }
            }

            // The standing thermometer — **AI rivals only**.
            if record.is_some_and(|r| !r.is_human) {
                pen.inset(canvas, Rect::new(METER.x, METER.y + top, METER.w, METER.h));
                canvas.fill_rect(METER_BAR.x, METER_BAR.y + top, METER_BAR.w, METER_BAR.h, 0x3F);
                // **[I] which direction this reads.** The card is *this lord*,
                // so the meter is taken as HIS opinion of the player -
                // `realms[him].pair(me).standing` - and the relationship is
                // asymmetric, so the other reading is a different number. The
                // decompilation was not read for it and this is flagged rather
                // than asserted.
                let standing = record.map_or(0, |r| r.pair(ctx.game.player).standing) as i32;
                let colour = if standing >= 11 {
                    METER_GOOD
                } else if standing < -10 {
                    METER_BAD
                } else {
                    METER_NEUTRAL
                };
                for row in 0..METER_ROWS {
                    if METER_TOP - row <= standing {
                        canvas.fill_rect(METER_BAR.x, METER_BAR.y + top + row, 8, 1, colour);
                    }
                }
            }
        }

        // The selected lord's name, in the heading font.
        l2_view::text::draw(
            canvas,
            TARGET_NAME_AT.0,
            TARGET_NAME_AT.1,
            &format!("LORD {}", self.target),
            ink.text,
        );

        // The menu.
        let menu = self.menu(ctx);
        if let Some(well) = menu.well() {
            pen.inset(canvas, well);
        }
        if menu != Menu::Allied {
            pen.misc_frame(canvas, ORNAMENT.0, ORNAMENT.1, ORNAMENT.2);
        }
        if menu == Menu::Mail {
            let s = a.text(GROUP, DISPATCHED).to_string();
            pen.body_wrapped(canvas, LINE_X, LINE_Y0 + LINE_PITCH, LINE_WRAP, &s, font::TEXT);
            return;
        }
        for (i, &(index, _)) in menu.rows().iter().enumerate() {
            let y = LINE_Y0 + i as i32 * LINE_PITCH;
            let s = a.text(GROUP, index).to_string();
            // `FUN_0040328E` at a 160-pixel wrap, not `Eng_DrawString`.
            pen.body_wrapped(canvas, LINE_X, y, LINE_WRAP, &s, font::TEXT);
            let b = button_rect(i);
            pen.system_frame(canvas, BUTTON_FRAME, b.x, b.y);
        }

        if let Some(asked) = self.asked {
            l2_view::text::draw(
                canvas,
                4,
                470,
                &format!("{asked:?} - 0x1A THE DIPLOMACY DIALOG IS NOT BUILT"),
                ink.dim,
            );
        }
    }
}
