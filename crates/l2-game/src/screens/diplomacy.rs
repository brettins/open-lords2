//! **The other lords** — `g_screenId` `0x0B`, and the seven dialogs behind it
//! at `0x1A`.
//!
//! `Diplo_DrawScreen` (`0x00416CF3`) and `Screen_DiploDialog` (`0x0041789B`).
//! Both were shells: `0x0B` drew a window and one of its four menu layouts,
//! and `0x1A` was not in the table at all — `docs/screens-county.md` could not
//! identify it. It is the compose page, and its seven shapes are
//! `g_diploKind` 0..=6.
//!
//! This is **the player's whole side of diplomacy**. Everything a person can do
//! to an AI lord goes through these two screens and out through `Diplo_Post`;
//! there is no other door. `docs/diplomacy.md` §7.
//!
//! # The screen, out of the painter
//!
//! ```text
//! Diplo_DefaultTarget()                       if the target is gone
//! g_diploMenuState = 3 if pair[target][me].hasMail else 1/0/2 by my ally
//! FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)        the window, 448 x 432 at (16,32)
//! Ui_OkButton(0x1A8, 0x1A6, 0)
//! File_ReadChunk("faces.pl8", …)
//! for each rival still in play:  Diplo_DrawLordCard(realm, slot++)
//! Ui_DrawText(g_playerNames[target], 0xD0, 0x3D, heading)
//! then one of four menus from L2.eng group 72
//! ```
//!
//! and one card, out of `Diplo_DrawLordCard` (`0x004171EE`):
//!
//! ```text
//! Ui_DrawInsetRect(0x30, slot*100 + 0x31, 0x52, 0x4E)      82 x 78
//! faces.pl8 frame lord*3 - 3   at (0x31, slot*100 + 0x32)  frame 12 for a human
//! Misc_cty.pl8 frame shield + 0x55 at (0x20, slot*100 + 0x37)
//! Ui_DrawText(g_playerNames[realm], 0x20, slot*100 + 0x83)
//! if realm == target:  two outlines at (0x2F, +0x30) and (0x2E, +0x2F)
//! if pair[realm][me].hasMail:  frame 15 at (0x1E, slot*100 + 0x50)
//! if pair[realm][me].allied:   frame 13 at (0x9E, slot*100 + 0x41)
//! else if pair[realm][me].atWar: frame 14 at (0x92, slot*100 + 0x41)
//! the standing thermometer: 10 x 63 at x = 0x88, filled from +30 down
//! ```
//!
//! ## Two things the painter says that `docs/diplomacy.md` §7 has backwards
//!
//! **`g_diploMenuState == 3` is not *"this realm has already written to me"*.**
//! The test is `pair[target][localPlayer].hasMail` — the *target's* record,
//! indexed by *me* — and `Diplo_Post` sets `pair[to][from].hasMail`. So the
//! flag means **I** have a letter sitting unanswered in **their** inbox, which
//! is exactly what group 72 index 24 says: *"A message has been dispatched, my
//! Lord."* One letter per rival per turn, and the menu disappears until they
//! answer it.
//!
//! **The mail icon on a card is the same flag**, read the same way round, so it
//! marks a rival you have written to and not one who has written to you. A
//! person's own inbox is never read by anything: `Diplo_AnswerInbox` is AI turn
//! step 1 and a human realm's AI turn is skipped entirely.
//!
//! # The compose page's three shapes
//!
//! One `g_screenId` and three widget tables, chosen by `g_diploKind`:
//!
//! | kind | painter | widgets | what it needs |
//! |---|---|---|---|
//! | 0 gift | `Diplo_DrawGiftGold` `0x00417960` | `0x004DD9D0`, **four** | an amount |
//! | 1–4 letters | `Diplo_DrawLetter` `0x00417AEF` | `0x004DDA30`, two | a 199-character draft |
//! | 5–6 requests | `Diplo_DrawCountyRequest` `0x00417CEF` | `0x004DDA60`, two | a county |
//!
//! The gift table's extra pair is the **+10 / −10** stepper at (184, 240) and
//! (216, 240), handled by `FUN_00436372`, which clamps the amount to
//! `[0, my gold]` on every click.
//!
//! **The free-text letter is not written here, and the four buffers are not
//! what you type into.** Screen `0x1A`'s arm calls
//! `FUN_0040210C(g_diploLetterDraft + (kind - 1) * 200, 199)` on every frame
//! the widget test declines — and `FUN_0040210C` is a bounded copy **out of
//! `DAT_005CD550`**, the game's one shared text-edit buffer, which
//! `FUN_00401D26(ch)` inserts typed characters into at cursor `DAT_005BB4A8`.
//! So there is a single editor, and the four 200-byte buffers at
//! `g_diploLetterDraft` are snapshots harvested from it once a frame;
//! `Diplo_OpenCompliment` and its three siblings run the copy the other way
//! (`FUN_00402009`) when the dialog opens.
//!
//! That matters for whoever builds text entry: the letter is not four
//! independent fields, it is one field with four save slots. Text entry is
//! another branch's; this screen carries the draft as a `String` and lets that
//! branch fill it.
//!
//! # `Diplo_SendClicked`'s six refusals, and the order they are tested in
//!
//! `Diplo_SendClicked` (`0x00436408`) is the tick. Two things about it are
//! worth having in front of you, because both are surprising:
//!
//! * **it closes the screen before it validates.** `g_screenId = 0` and the
//!   letter is copied into the player's slot at the top of the function; every
//!   refusal below that point is a message on the *map*, not a red light on the
//!   dialog. A refused send has still left the diplomacy screen.
//! * **the county tests are asymmetric.** Kind 5 wants a county that is
//!   **mine** and has an enemy standing in it (`+0x19C`); kind 6 wants one that
//!   is **not** mine and not my ally's. Group 242 *"does not belong to us"* is
//!   raised only by kind 5 — kind 6 answers a county of my own with group 244,
//!   *"This county is part of our alliance"*, which is not what has gone wrong.
//!   Reproduced, and catalogued.

use l2_kingdom::diplomacy::{group, Kind};
use l2_kingdom::realm::MAX_REALMS;
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

/// `L2.eng` group 72 — *"Diplomacy."*, and every label on both screens.
pub const GROUP: usize = 72;

/// `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)` — the window, in cells of 16.
pub const WINDOW: Rect = Rect::new(0x10, 0x20, 0x1C * 16, 0x1B * 16);

/// `Ui_OkButton(0x1A8, 0x1A6, 0)`.
pub const OK: Rect = Rect::new(0x1A8, 0x1A6, 32, 32);

/// One lord card, for slot `n`: `Ui_DrawInsetRect(0x30, n*100 + 0x31, 0x52,
/// 0x4E)`. The stride of 100 is the original's, and it is not the card's
/// height — the card is 78 tall and there are 22 pixels of air between.
pub fn card_rect(slot: usize) -> Rect {
    Rect::new(0x30, slot as i32 * 100 + 0x31, 0x52, 0x4E)
}

/// The standing thermometer: 10 wide, 63 tall, at x = 0x88 inside the card.
/// `Diplo_DrawLordCard` fills it from +30 downward to the standing.
pub const THERMOMETER_W: i32 = 10;
pub const THERMOMETER_H: i32 = 63;
pub const THERMOMETER_X: i32 = 0x88;

/// The three colours the thermometer is filled in, and **they are the AI's own
/// two alliance thresholds**: `Diplo_ReplyAllianceOffer` accepts outright at
/// ≥ +11 and refuses outright below −10, which is the same pair of numbers
/// written by different code. `docs/diplomacy.md` §1.1.
pub const THERMOMETER_WARM: i8 = 11;
pub const THERMOMETER_COLD: i8 = -11;

/// Which of the four menu layouts `g_diploMenuState` holds, and what each one
/// offers. The indices are into `L2.eng` group 72.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// 0 — I have no ally. Gift, compliment, insult, **offer an alliance**.
    NoAlly,
    /// 1 — this realm **is** my ally. Gift, compliment, insult, terminate, ask
    /// for help, ask for an attack.
    Allied,
    /// 2 — I am allied to somebody else. Gift, compliment, insult, and nothing
    /// more: you cannot even offer, because the offer would be refused.
    AlliedElsewhere,
    /// 3 — a letter of mine is already sitting in their inbox. Group 72 index
    /// 24 alone, *"A message has been dispatched, my Lord."*, and **no
    /// widgets at all**: `g_diploWidgetCount = 0`.
    Dispatched,
}

impl Menu {
    /// `Diplo_DrawScreen`'s opening test, in its own order — the mail flag
    /// first, and only then the three alliance cases.
    pub fn of(ctx: &Ctx, target: u8) -> Menu {
        let me = ctx.game.player;
        let realms = &ctx.game.kingdom.realms;
        let Some(theirs) = realms.get(target as usize) else { return Menu::NoAlly };
        if theirs.pair(me).has_mail {
            return Menu::Dispatched;
        }
        let mine = realms.get(me as usize).map_or(0, |r| r.ally);
        if mine == target {
            Menu::Allied
        } else if mine == 0 {
            Menu::NoAlly
        } else {
            Menu::AlliedElsewhere
        }
    }

    /// The group 72 rows this layout draws, top to bottom. `Diplo_DrawScreen`
    /// lists them literally; the widget rectangles come from `g_diploWidgets`
    /// and are the same six regardless, taken from the top.
    pub fn rows(self) -> &'static [usize] {
        match self {
            Menu::NoAlly => &[2, 3, 4, 5],
            Menu::Allied => &[2, 3, 4, 6, 7, 8],
            Menu::AlliedElsewhere => &[2, 3, 4],
            Menu::Dispatched => &[24],
        }
    }

    /// What each row does. `Diplo_OpenAlliance` (`0x00436229`) is **one widget
    /// for two rows** — row 5 *"Offer an alliance"* and row 6 *"Terminate
    /// alliance"* are the same handler, and it picks `g_diploKind` 4 over 3
    /// exactly when the target is already my ally. So the kind follows from the
    /// menu state and not from which row was drawn.
    pub fn kind_of_row(row: usize) -> Option<Kind> {
        match row {
            2 => Some(Kind::Gift),
            3 => Some(Kind::Compliment),
            4 => Some(Kind::Insult),
            5 => Some(Kind::OfferAlliance),
            6 => Some(Kind::EndAlliance),
            7 => Some(Kind::AskHelp),
            8 => Some(Kind::AskAttack),
            _ => None,
        }
    }
}

/// `g_diploWidgets` (`0x004DD940`) — six 32-pixel widgets at (400, 102 + 50n).
/// The menu layout decides how many of them are live (`g_diploWidgetCount`),
/// and the rows are taken from the top in order.
pub fn menu_widget(slot: usize) -> Rect {
    Rect::new(400, 102 + slot as i32 * 50, 32, 32)
}

/// The label beside a menu widget. `FUN_0040328E(72, row, 0xE0, y, 0xA0, 100)`
/// puts the text at x = 0xE0 and the first row at y = 0x70, 50 apart — so the
/// text sits 32 pixels above its own widget, which is the original's layout and
/// not a mistake here.
pub fn menu_label_y(slot: usize) -> i32 {
    0x70 + slot as i32 * 50
}

// ------------------------------------------------------------ the 0x0B screen

/// Screen `0x0B`.
pub struct DiplomacyScreen {
    /// `g_diploTarget` (`0x0053F03C`). `None` until the first draw, because
    /// `Diplo_DefaultTarget` needs the realm array and a screen is built
    /// without one — the same resolve-on-first-use `castle.rs` uses.
    target: Option<u8>,
}

impl DiplomacyScreen {
    pub fn new() -> DiplomacyScreen {
        DiplomacyScreen { target: None }
    }

    /// `Diplo_DrawScreen`'s first two lines: **a target that has been knocked
    /// out is replaced**, so the screen can never be looking at a dead realm.
    /// `Diplo_DefaultTarget` (`0x004A1E6C`) takes the first in-play realm that
    /// is not the local player, or 0.
    pub fn target(&self, ctx: &Ctx) -> u8 {
        let t = self.target.unwrap_or(0);
        if t != 0 && ctx.game.kingdom.realms.get(t as usize).is_some_and(|r| r.strength != 0) {
            return t;
        }
        l2_kingdom::diplomacy::default_target(&ctx.game.kingdom.realms, ctx.game.player)
    }

    /// The rivals with a card, in the order the painter stacks them: ascending
    /// realm id, skipping the local player and anybody out of play.
    pub fn cards(ctx: &Ctx) -> Vec<u8> {
        (1..MAX_REALMS)
            .filter(|&id| {
                id as u8 != ctx.game.player
                    && ctx.game.kingdom.realms[id].strength != 0
            })
            .map(|id| id as u8)
            .collect()
    }
}

impl Default for DiplomacyScreen {
    fn default() -> Self {
        DiplomacyScreen::new()
    }
}

impl Screen for DiplomacyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Diplomacy
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Diplomacy".into()
    }

    /// The painter draws over whatever was underneath and clears nothing.
    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let Event::Click { x, y } = event else {
            return match event {
                // arm: 0x0042FF10/diplo-right-exit
                //
                // `Screen_FrameInput`'s `0x0B` arm: `if (rightReleased) {
                // g_screenId = 0; }` **before** it even asks about the corner
                // button, and the same statement again in the else of the two
                // modal guards. Right-click leaves the screen — the gesture
                // `docs/agents.md` records this project as systematically
                // missing.
                Event::RightClick { .. } => Transition::Pop,
                Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
                _ => Transition::Stay,
            };
        };
        // arm: 0x0040E7E4/diplo-ok
        //
        // `Ui_OkButtonClicked` — the corner picture `Ui_OkButton(0x1A8, 0x1A6)`
        // drew, hit-tested as a **24 × 24** box at that origin on the left
        // button's *release*. Ours is the 32-pixel button rectangle.
        if OK.contains(x, y) {
            return Transition::Pop;
        }
        // arm: 0x004369BD/diplo-pick-lord
        //
        // `FUN_004369BD` walks the same cards the painter stacked and hit-tests
        // each card rectangle; the first hit becomes `g_diploTarget`. It runs
        // **after** the widget test, so a click that lands on both belongs to
        // the menu — which cannot happen, since the menu is at x = 400 and the
        // cards end at 0x30 + 0x52 = 130.
        let target = self.target(ctx);
        let menu = Menu::of(ctx, target);
        for (slot, row) in menu.rows().iter().enumerate() {
            if menu == Menu::Dispatched {
                break;
            }
            if !menu_widget(slot).contains(x, y) {
                continue;
            }
            // arm: 0x00436141/diplo-open-compose
            //
            // The six widget handlers — `Diplo_OpenGift`, `…Compliment`,
            // `…Insult`, `…Alliance`, `…AskHelp`, `…AskAttack` — are one arm
            // here because they differ only in the `g_diploKind` they set and
            // in which draft buffer they clear. `Diplo_OpenAlliance` is the one
            // with a decision in it, and [`Menu::kind_of_row`] is that
            // decision: row 6 is only drawn when the target is already my ally.
            let Some(kind) = Menu::kind_of_row(*row) else { break };
            return Transition::Push(ScreenId::DiploCompose(target, kind.byte()));
        }
        for (slot, realm) in DiplomacyScreen::cards(ctx).iter().enumerate() {
            if card_rect(slot).contains(x, y) {
                self.target = Some(*realm);
                return Transition::Stay;
            }
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let a = &ctx.assets.shell;
        widget::panel(canvas, ink, WINDOW);

        let target = self.target(ctx);
        let me = ctx.game.player;
        for (slot, realm) in DiplomacyScreen::cards(ctx).iter().enumerate() {
            let r = card_rect(slot);
            widget::panel(canvas, ink, r);
            if *realm == target {
                widget::frame(canvas, r, ink.highlight);
            }
            let rr = &ctx.game.kingdom.realms[*realm as usize];
            // The portrait is `faces.pl8` frame `lord * 3 - 3`, or frame 12 for
            // a human rival. We have no sheet loaded here, so the lord's name
            // stands in for the face — `L2.eng` group 7 indexed by the lord
            // byte, which is exactly what `Game_NewGame` copies into
            // `g_playerNames` in the first place.
            let name = a.text(7, rr.lord.min(4) as usize).to_uppercase();
            text::draw(canvas, r.x + 4, r.y + 4, &name, ink.text);
            let colour = ink.realm[(rr.shield_index.clamp(1, 5)) as usize];
            widget::frame(canvas, Rect::new(r.x + 2, r.y + r.h - 14, 12, 10), colour);

            // The three status icons, all read out of the **rival's** record
            // indexed by me. `allied` and `atWar` are exclusive in the painter:
            // an at-war icon is only drawn when the allied one was not.
            let their = rr.pair(me);
            let mut icons = String::new();
            if their.allied {
                icons.push('A');
            } else if their.at_war {
                icons.push('W');
            }
            if their.has_mail {
                icons.push('M');
            }
            if !icons.is_empty() {
                text::draw(canvas, r.x + r.w - 24, r.y + 4, &icons, ink.highlight);
            }

            // The thermometer, filled from +30 down to the standing, in the
            // colour its band names.
            let standing = their.standing;
            let bar = Rect::new(THERMOMETER_X, r.y + 2, THERMOMETER_W, THERMOMETER_H);
            widget::frame(canvas, bar, ink.border);
            let fill = if standing >= THERMOMETER_WARM {
                ink.good
            } else if standing <= THERMOMETER_COLD {
                ink.bad
            } else {
                ink.dim
            };
            let filled = (i32::from(standing) + 30) * THERMOMETER_H / 60;
            if filled > 0 {
                canvas.fill_rect(
                    bar.x + 1,
                    bar.y + THERMOMETER_H - filled,
                    THERMOMETER_W - 2,
                    filled,
                    fill,
                );
            }
            text::draw(canvas, bar.x - 4, bar.y + THERMOMETER_H + 2, &format!("{standing}"), ink.dim);
        }

        // The heading: the selected rival's name at (0xD0, 0x3D).
        let heading = ctx
            .game
            .kingdom
            .realms
            .get(target as usize)
            .map(|r| a.text(7, r.lord.min(4) as usize).to_uppercase())
            .unwrap_or_default();
        text::draw(canvas, 0xD0, 0x3D, &heading, ink.highlight);

        let menu = Menu::of(ctx, target);
        for (slot, row) in menu.rows().iter().enumerate() {
            let label = a.text(GROUP, *row).to_uppercase();
            if menu == Menu::Dispatched {
                // Index 24 alone, and no widget under it.
                text::draw(canvas, 0xE0, 0xA2, &label, ink.text);
                break;
            }
            let w = menu_widget(slot);
            widget::panel(canvas, ink, w);
            text::draw(canvas, 0xE0, menu_label_y(slot), &label, ink.text);
        }

        widget::button(canvas, ink, OK, "OK", true);
    }
}

// ------------------------------------------------------------ the 0x1A screen

/// `FUN_00436372` — the gift stepper's step, and it is ten crowns whichever
/// button was pressed. Hotspot 1 adds, hotspot 0 subtracts.
pub const GIFT_STEP: i32 = 10;

/// `g_giftWidgets` (`0x004DD9D0`) — plus, minus, tick, cross.
pub const GIFT_MORE: Rect = Rect::new(184, 240, 32, 32);
pub const GIFT_LESS: Rect = Rect::new(216, 240, 32, 32);
pub const GIFT_SEND: Rect = Rect::new(288, 280, 32, 32);
pub const GIFT_CANCEL: Rect = Rect::new(324, 284, 32, 32);
/// `0x004DDA30` — the letter dialog's two.
pub const LETTER_SEND: Rect = Rect::new(288, 292, 32, 32);
pub const LETTER_CANCEL: Rect = Rect::new(324, 296, 32, 32);
/// `0x004DDA60` — the county dialog's two.
pub const COUNTY_SEND: Rect = Rect::new(320, 244, 32, 32);
pub const COUNTY_CANCEL: Rect = Rect::new(356, 248, 32, 32);

/// The three windows, `FUN_004093E0(x, y, cols, rows)` from the three painters.
pub const GIFT_WINDOW: Rect = Rect::new(0x40, 0xA0, 0x16 * 16, 0x0B * 16);
pub const LETTER_WINDOW: Rect = Rect::new(0x10, 0x90, 0x1C * 16, 0x0D * 16);
pub const COUNTY_WINDOW: Rect = Rect::new(0x30, 0x80, 0x18 * 16, 0x0F * 16);

/// Why a send was refused, each one an `L2.eng` group of its own with
/// *"Message not sent."* at index 0. `Diplo_SendClicked`'s order is preserved,
/// because the order decides which message a doubly-wrong county gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// 240 — *"You did not select a county, my Lord."*
    NoCounty,
    /// 241 — *"Your requested county does not belong to anyone, my Lord."*
    Unowned,
    /// 242 — *"Your requested county does not belong to us, my Lord."* Kind 5
    /// only.
    NotOurs,
    /// 243 — *"Our county does not have an enemy in it, my Lord."* Kind 5 only.
    NoEnemy,
    /// 244 — *"This county is part of our alliance, my Lord."* Kind 6, and it
    /// is **also what a county of your own gets**, which is the wrong sentence
    /// for that case. Reproduced.
    Allied,
    /// 219 — *"You cannot ally with this player, my Lord, until they end their
    /// current treaty."* Kind 3, and **only against a human target**: the guard
    /// is `g_realms[target].isHuman != 0 && ally != 0`, so an AI that already
    /// has an ally is not refused here — the offer goes out and comes back as
    /// group 178.
    TargetAlreadyAllied,
}

impl Refusal {
    /// The `L2.eng` group this refusal is drawn from.
    pub fn group(self) -> u16 {
        match self {
            Refusal::NoCounty => group::NO_COUNTY_SELECTED,
            Refusal::Unowned => group::COUNTY_UNOWNED,
            Refusal::NotOurs => group::COUNTY_NOT_OURS,
            Refusal::NoEnemy => group::COUNTY_UNTHREATENED,
            Refusal::Allied => group::COUNTY_IS_ALLIED,
            Refusal::TargetAlreadyAllied => group::ALREADY_IN_ALLIANCE,
        }
    }
}

/// `Diplo_SendClicked` (`0x00436408`)'s validation, as a function of the state
/// it reads. Separated from the click so that the ladder can be tested at every
/// rung — `docs/decisions.md` C26 is exactly about a rule with one fixture.
pub fn refusal(ctx: &Ctx, target: u8, kind: Kind, county: u8) -> Option<Refusal> {
    let me = ctx.game.player;
    if matches!(kind, Kind::AskHelp | Kind::AskAttack) {
        if county == 0 {
            return Some(Refusal::NoCounty);
        }
        let c = ctx.game.kingdom.counties.get(county as usize)?;
        if c.owner == 0 {
            return Some(Refusal::Unowned);
        }
        if kind == Kind::AskHelp {
            if c.owner != me {
                return Some(Refusal::NotOurs);
            }
            // County `+0x19C` — the enemy-troop count the panel draws. Asking
            // for help in a county nothing is threatening is refused.
            if c.enemy_troops == 0 {
                return Some(Refusal::NoEnemy);
            }
        } else {
            if c.owner == me {
                return Some(Refusal::Allied);
            }
            if ctx.game.kingdom.realms.get(me as usize).map_or(0, |r| r.ally) == c.owner {
                return Some(Refusal::Allied);
            }
        }
    }
    if kind == Kind::OfferAlliance {
        let t = ctx.game.kingdom.realms.get(target as usize)?;
        if t.is_human && t.ally != 0 {
            return Some(Refusal::TargetAlreadyAllied);
        }
    }
    None
}

/// The rectangle `FUN_0043B4CB` hit-tests: 128 × 128 at (96, 176).
///
/// **The picture is two pixels left and three down of it.**
/// `FUN_00410C71(county, x, y)` blits the raster at `(x - 2, y + 3)` and then
/// `Minimap_DrawOverlay(county, x - 2, y + 3, 0)` on top, while the hit test
/// uses `(x, y)` unadjusted. That is the same disagreement the sidebar minimap
/// has between `Minimap_Draw` and `Minimap_Click` — `l2_view::chrome`'s
/// `MINIMAP_X` against `MINIMAP_HIT_X` — and it is the original's in both
/// places. The hit rectangle is what is reproduced here, because it is what
/// decides which county you picked.
pub const PICKER: Rect =
    Rect::new(0x60, 0xB0, l2_view::chrome::MINIMAP_DIM, l2_view::chrome::MINIMAP_DIM);

/// The county under a pixel of the compose dialog's map, or `None`.
///
/// **`None` covers two different things and the original conflates them too**:
/// outside the rectangle, and inside it on a pixel whose county is 0. Both
/// return 0 from `FUN_0043B4CB`, and 0 means *"not consumed"* — so a click on
/// the sea inside the map falls through to the corner button behind it.
pub fn county_at_picker(ctx: &Ctx, x: i32, y: i32) -> Option<u8> {
    if !PICKER.contains(x, y) {
        return None;
    }
    let minimap = ctx.assets.minimap(ctx.game.map_slot)?;
    let dim = l2_view::chrome::MINIMAP_DIM;
    let (dx, dy) = (x - PICKER.x, y - PICKER.y);
    let county = *minimap.counties.get((dy * dim + dx) as usize)?;
    (county != 0).then_some(county)
}

/// Screen `0x1A` — one of the seven compose dialogs.
pub struct ComposeScreen {
    target: u8,
    kind: Kind,
    /// `g_diploGold` (`0x0057A0F8`), for kind 0 only.
    pub gold: i32,
    /// `g_pickedCounty`, for kinds 5 and 6. `Diplo_OpenAskHelp` clears it.
    pub county: u8,
    /// One of the four 200-byte buffers at `g_diploLetterDraft` (`0x0053F2B8`).
    /// **Nothing here fills it** — the original's arm calls the keyboard entry
    /// field, which is a different branch's work. Carried so that the branch
    /// has somewhere to write and so that a send copies something.
    pub draft: String,
    /// What the tick did, for the caller.
    pub sent: Option<Result<Kind, Refusal>>,
}

impl ComposeScreen {
    pub fn new(target: u8, kind: u8) -> ComposeScreen {
        ComposeScreen {
            target,
            kind: Kind::from_byte(kind).unwrap_or(Kind::Gift),
            gold: 0,
            county: 0,
            draft: String::new(),
            sent: None,
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn target(&self) -> u8 {
        self.target
    }

    /// The two widgets every shape has, by shape.
    fn buttons(&self) -> (Rect, Rect) {
        match self.kind {
            Kind::Gift => (GIFT_SEND, GIFT_CANCEL),
            Kind::AskHelp | Kind::AskAttack => (COUNTY_SEND, COUNTY_CANCEL),
            _ => (LETTER_SEND, LETTER_CANCEL),
        }
    }

    /// `FUN_00436372` — step the gift, then clamp to `[0, my gold]`. **The
    /// clamp is on every click**, so a player who has just spent his treasury
    /// finds the amount follow it down.
    pub fn step_gift(&mut self, ctx: &Ctx, by: i32) {
        self.gold += by;
        if self.gold < 0 {
            self.gold = 0;
        }
        let purse = ctx.game.kingdom.realms.get(ctx.game.player as usize).map_or(0, |r| r.gold);
        if self.gold > purse {
            self.gold = purse;
        }
    }

    /// `Diplo_SendClicked`'s hotspot 1.
    ///
    /// The order is the original's and it matters: **the screen closes first**,
    /// the draft is copied into the player's per-realm slot second, and the
    /// validation is third. A refusal is a message on the map.
    fn send(&mut self, ctx: &mut Ctx) -> Transition {
        let refused = refusal(ctx, self.target, self.kind, self.county);
        if let Some(why) = refused {
            self.sent = Some(Err(why));
            return Transition::Pop;
        }
        // `if (g_diploKind != 0) g_diploGold = 0;` — only a gift carries gold,
        // and the field is cleared rather than ignored.
        let gold = if self.kind == Kind::Gift { self.gold } else { 0 };
        let me = ctx.game.player;
        ctx.game.kingdom.post_letter(me, self.target, self.kind, gold, self.county);
        self.sent = Some(Ok(self.kind));
        Transition::Pop
    }
}

impl Screen for ComposeScreen {
    fn id(&self) -> ScreenId {
        ScreenId::DiploCompose(self.target, self.kind.byte())
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Dispatch a message".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let Event::Click { x, y } = event else {
            return match event {
                // arm: 0x0042FF10/compose-right-exit
                //
                // The `0x1A` arm's right-release, and note **where it goes**:
                // `g_screenId = 0`, the campaign map, not back to `0x0B`. Only
                // the cross inside the dialog returns to the lord cards. Two
                // exits from one screen that land in different places, and the
                // difference is not visible from the dialog.
                Event::RightClick { .. } => Transition::Replace(ScreenId::Campaign),
                Event::KeyDown(Key::Escape) => Transition::Pop,
                Event::KeyDown(Key::Enter) => self.send(ctx),
                _ => Transition::Stay,
            };
        };
        // arm: 0x0043B4CB/compose-pick-county
        //
        // `FUN_0043B4CB(0x60, 0xB0)` — a 128 × 128 county raster drawn at
        // (96, 176), read straight out of `g_minimapCounty`. It is tested
        // **before** the corner button, and a pixel that resolves to county 0
        // is *not* a hit, so a click on the sea falls through to the OK test
        // and can leave the screen. `g_diploKind < 5` short-circuits it: the
        // other five dialogs have no map on them.
        if matches!(self.kind, Kind::AskHelp | Kind::AskAttack) {
            if let Some(county) = county_at_picker(ctx, x, y) {
                self.county = county;
                return Transition::Stay;
            }
        }
        let (send, cancel) = self.buttons();
        // arm: 0x00436408/diplo-send
        if send.contains(x, y) {
            return self.send(ctx);
        }
        // arm: 0x00436408/diplo-cancel
        //
        // Hotspot 0 of the same handler, and it is the whole of the function's
        // first statement: `g_screenId = 0xB`, back to the lord cards.
        if cancel.contains(x, y) {
            return Transition::Pop;
        }
        if self.kind == Kind::Gift {
            // arm: 0x00436372/diplo-gift-step
            if GIFT_MORE.contains(x, y) {
                let ctx: &Ctx = ctx;
                self.step_gift(ctx, GIFT_STEP);
                return Transition::Stay;
            }
            if GIFT_LESS.contains(x, y) {
                let ctx: &Ctx = ctx;
                self.step_gift(ctx, -GIFT_STEP);
                return Transition::Stay;
            }
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let a = &ctx.assets.shell;
        let me = ctx.game.player;
        let name = ctx
            .game
            .kingdom
            .realms
            .get(self.target as usize)
            .map(|r| a.text(7, r.lord.min(4) as usize).to_uppercase())
            .unwrap_or_default();

        match self.kind {
            Kind::Gift => {
                widget::panel(canvas, ink, GIFT_WINDOW);
                // 72/10 "Send gift of gold to" + the name.
                let line = format!("{} {name}", a.text(GROUP, 10).to_uppercase());
                text::draw(canvas, 0x60, 0xB8, &line, ink.text);
                // 72/23 "Last gift was" + pair[me][target].bestGift — read from
                // **my** record about them, which is the one `Diplo_ReplyGift`
                // never writes. See the test below: the ratchet the AI judges
                // by lives in the AI's record, and this panel shows the
                // player's own, which is always zero in single player.
                let best = ctx
                    .game
                    .kingdom
                    .realms
                    .get(me as usize)
                    .map_or(0, |r| r.pair(self.target).best_gift);
                let line = format!("{} {best}", a.text(GROUP, 23).to_uppercase());
                text::draw(canvas, 0x60, 0xD8, &line, ink.dim);
                // 72/18 "Gift of" and the amount at 0x100.
                text::draw(canvas, 0x60, 0xF8, &a.text(GROUP, 18).to_uppercase(), ink.text);
                text::draw(canvas, 0x100, 0xF8, &format!("{}", self.gold), ink.highlight);
                text::draw(canvas, 0xA0, 0x120, &a.text(GROUP, 17).to_uppercase(), ink.text);
                widget::button(canvas, ink, GIFT_MORE, "+", true);
                widget::button(canvas, ink, GIFT_LESS, "-", true);
            }
            Kind::AskHelp | Kind::AskAttack => {
                widget::panel(canvas, ink, COUNTY_WINDOW);
                let row = if self.kind == Kind::AskHelp { 15 } else { 16 };
                let line = format!("{} {name}", a.text(GROUP, row).to_uppercase());
                text::draw(canvas, 0x50, 0x98, &line, ink.text);
                // 72/19 or 20 while no county is picked; 21 or 22 plus the
                // county's own name once one is.
                if self.county == 0 {
                    let row = if self.kind == Kind::AskHelp { 19 } else { 20 };
                    text::draw(canvas, 0x50, 0x140, &a.text(GROUP, row).to_uppercase(), ink.text);
                } else {
                    let row = if self.kind == Kind::AskHelp { 21 } else { 22 };
                    let index = super::army::county_name_index(ctx, self.county);
                    let line = format!(
                        "{} {}",
                        a.text(GROUP, row).to_uppercase(),
                        a.text(100, index).to_uppercase()
                    );
                    text::draw(canvas, 0x50, 0x140, &line, ink.text);
                }
                text::draw(canvas, 0x140, 0xE0, &a.text(GROUP, 17).to_uppercase(), ink.text);
            }
            _ => {
                widget::panel(canvas, ink, LETTER_WINDOW);
                // 72/11 + (kind - 1): "Give a compliment to", "Insult",
                // "Ask for an alliance with", "End alliance with".
                let row = 11 + self.kind.byte() as usize - 1;
                let line = format!("{} {name}", a.text(GROUP, row).to_uppercase());
                text::draw(canvas, 0x30, 0xA8, &line, ink.text);
                // `FUN_00417BD3` — the draft box: `Ui_DrawInsetRect(0x20, 0xC0,
                // 0x1A0, 0x60)` with the 199-character buffer in it.
                let box_rect = Rect::new(0x20, 0xC0, 0x1A0, 0x60);
                widget::panel(canvas, ink, box_rect);
                text::draw(canvas, 0x30, 0xC8, &self.draft.to_uppercase(), ink.text);
                text::draw(canvas, 0x70, 0x130, &a.text(GROUP, 17).to_uppercase(), ink.text);
            }
        }
        let (send, cancel) = self.buttons();
        widget::button(canvas, ink, send, "OK", true);
        widget::button(canvas, ink, cancel, "X", true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four layouts are the painter's four, in its own order, and the
    /// alliance row is **one widget for two rows**.
    #[test]
    fn the_four_menu_layouts_are_the_painters_four() {
        assert_eq!(Menu::NoAlly.rows(), &[2, 3, 4, 5]);
        assert_eq!(Menu::Allied.rows(), &[2, 3, 4, 6, 7, 8]);
        assert_eq!(Menu::AlliedElsewhere.rows(), &[2, 3, 4]);
        assert_eq!(Menu::Dispatched.rows(), &[24]);
        // Row 5 and row 6 are `Diplo_OpenAlliance`'s two, and they are
        // different kinds.
        assert_eq!(Menu::kind_of_row(5), Some(Kind::OfferAlliance));
        assert_eq!(Menu::kind_of_row(6), Some(Kind::EndAlliance));
        // And the two request rows only appear on the allied layout, which is
        // the gate `docs/diplomacy.md` §4 calls "the gate on kinds 5 and 6".
        for row in [7, 8] {
            assert!(Menu::Allied.rows().contains(&row));
            assert!(!Menu::NoAlly.rows().contains(&row));
            assert!(!Menu::AlliedElsewhere.rows().contains(&row));
        }
    }

    /// The six menu widgets are 50 apart and never overlap a lord card. The
    /// cards run to x = 130 and the widgets start at 400, which is what makes
    /// `FUN_004369BD` running after the widget test harmless.
    #[test]
    fn the_menu_and_the_cards_cannot_both_be_hit() {
        for slot in 0..6 {
            let w = menu_widget(slot);
            assert_eq!(w.y, 102 + slot as i32 * 50);
            for card in 0..5 {
                let c = card_rect(card);
                assert!(c.x + c.w < w.x, "card {card} reaches widget {slot}");
            }
        }
    }

    /// The cards stack on a 100-pixel pitch and do not touch: 78 tall with 22
    /// pixels of air. A reader who assumed the stride was the height would
    /// place every card but the first wrongly.
    #[test]
    fn the_cards_stack_on_a_pitch_wider_than_they_are() {
        for slot in 0..4 {
            let a = card_rect(slot);
            let b = card_rect(slot + 1);
            assert_eq!(b.y - a.y, 100);
            assert!(a.y + a.h < b.y, "card {slot} runs into the next");
        }
        assert_eq!(card_rect(0).h, 0x4E);
    }

    /// The thermometer's two break points are the AI's two alliance decisions,
    /// and the fill runs the whole bar from −30 to +30.
    #[test]
    fn the_thermometer_bands_are_the_alliance_thresholds() {
        assert_eq!(THERMOMETER_WARM, 11, "Diplo_ReplyAllianceOffer accepts at >= 11");
        assert_eq!(THERMOMETER_COLD, -11, "and refuses outright below -10");
        let filled = |s: i32| (s + 30) * THERMOMETER_H / 60;
        assert_eq!(filled(-30), 0);
        assert_eq!(filled(30), THERMOMETER_H);
        assert!(filled(0) > 0 && filled(0) < THERMOMETER_H);
    }

    /// Every refusal names a distinct `L2.eng` group, and all six exist.
    #[test]
    fn the_six_refusals_are_six_groups() {
        let all = [
            Refusal::NoCounty,
            Refusal::Unowned,
            Refusal::NotOurs,
            Refusal::NoEnemy,
            Refusal::Allied,
            Refusal::TargetAlreadyAllied,
        ];
        let mut groups: Vec<u16> = all.iter().map(|r| r.group()).collect();
        groups.sort_unstable();
        groups.dedup();
        assert_eq!(groups.len(), all.len(), "two refusals share a group");
        assert_eq!(Refusal::NoCounty.group(), 240);
        assert_eq!(Refusal::TargetAlreadyAllied.group(), 219);
    }

    /// The three windows are the three painters', and none of them covers the
    /// whole screen — every one is an inset over the diplomacy screen.
    #[test]
    fn the_three_compose_windows_are_insets() {
        for w in [GIFT_WINDOW, LETTER_WINDOW, COUNTY_WINDOW] {
            assert!(w.x > 0 && w.y > 0);
            assert!(w.x + w.w <= 640 && w.y + w.h <= 480);
        }
        // The gift's stepper and its tick are inside its own window.
        for r in [GIFT_MORE, GIFT_LESS, GIFT_SEND, GIFT_CANCEL] {
            assert!(r.x >= GIFT_WINDOW.x && r.y >= GIFT_WINDOW.y, "{r:?} is outside the window");
        }
    }
}
