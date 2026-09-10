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
use crate::shell::{font, Pen};
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

/// **The three tables are three windows onto one array**, and that is why none
/// of them hides a record.
///
/// Decoded out of `Lords2.exe`, `0x004DD9D0` is a contiguous run of 24-byte
/// widgets: records 0…3 are the gift's four, records 4…5 *are* `0x004DDA30`
/// (`0x004DD9D0 + 4 × 24`) and records 6…7 *are* `0x004DDA60`
/// (`+ 6 × 24`). Every slice is exactly as long as the count its
/// `Screen_DrawWidgets` arm passes, so the `g_sendSuppliesWidgets` failure —
/// eight records, six ever drawn — has no counterpart here. The run continues
/// past the compose dialogs into other screens' tick/cross pairs at
/// `0x004DDA90` and beyond, whose handlers (`FUN_004367FF`, `FUN_00436872`,
/// `FUN_004368FD`) are message replies rather than anything `0x1A` draws.
/// **[V]** `tools/oracle/widgets.js widgets 4dd9d0 8`.
pub const WIDGET_TABLE: u32 = 0x004D_D9D0;

/// The button-sheet frames those records name. `System.pl8`, and the pairing
/// is the one `tools/oracle/widgets.js` anchors on this very table:
/// **frame 68 is plus** — its record carries hotspot id 1, and `FUN_00436372`
/// reads `if (id == 1) g_diploGold += 10` — and **66 is minus**.
pub const PLUS_FRAME: usize = 68;
pub const MINUS_FRAME: usize = 66;
/// 29 and 31 are the mailed hand, thumb up and thumb down. **Not a tick and a
/// cross** — `docs/screens-county.md` §4.2 decoded them.
pub const THUMB_UP_FRAME: usize = 29;
pub const THUMB_DOWN_FRAME: usize = 31;
/// Every record in the three slices is `size` 32.
pub const WIDGET_DIM: i32 = 32;

/// The three windows, `FUN_004093E0(x, y, cols, rows)` from the three painters.
/// `FUN_004093E0` is `Ui_DrawBoxBorder(**1**, …)` plus `Ui_DrawBoxInterior`
/// inset a cell, so the border set is 1 on all three.
pub const GIFT_WINDOW: Rect = Rect::new(0x40, 0xA0, 0x16 * 16, 0x0B * 16);
pub const LETTER_WINDOW: Rect = Rect::new(0x10, 0x90, 0x1C * 16, 0x0D * 16);
pub const COUNTY_WINDOW: Rect = Rect::new(0x30, 0x80, 0x18 * 16, 0x0F * 16);
pub const WINDOW_SET: usize = 1;

/// `Ui_OkButton(x, y, 0)` — **a third button on every one of the seven**, and
/// one this module drew nothing for until the draw-call audit counted them.
/// It is `System.pl8` frame `0x33`, the cursor-into-a-hole close picture, and
/// it sits to the right of the thumb pair inside the same window.
pub const GIFT_OK: Rect = Rect::new(0x178, 0x126, 24, 24);
pub const LETTER_OK: Rect = Rect::new(0x1A8, 0x136, 24, 24);
pub const COUNTY_OK: Rect = Rect::new(0x188, 0x146, 24, 24);

// ------------------------------------------- the compose dialogs' own strings
//
// Every one of these is a literal argument to an `Eng_DrawString` in
// `Diplo_DrawGiftGold`, `Diplo_DrawLetter` or `Diplo_DrawCountyRequest`, and
// the words are checked against `L2.eng` in `crates/l2-game/tests/shell.rs`.
// **The check is on existence and these indices are verified against the
// words**, which is the stronger claim and the one group 16 failed.

/// 72/10 *"Send gift of gold to"*, with the target's name after it.
pub const GIFT_TO: usize = 10;
/// 72/23 *"Last gift was"*.
pub const LAST_GIFT: usize = 23;
/// 72/18 *"Gift of"*.
pub const GIFT_OF: usize = 18;
/// 72/17 *"Dispatch ?"* — the caption over the thumb pair, and **all three**
/// dialogs draw it, at three different places.
pub const DISPATCH: usize = 17;
/// 72/11 + (kind − 1): *"Give a compliment to"*, *"Insult"*, *"Ask for an
/// alliance with"*, *"End alliance with"*.
pub const LETTER_BASE: usize = 11;
/// 72/15 + k: *"Plead for help from"*, *"Plan strategic attack with"*.
pub const REQUEST_BASE: usize = 15;
/// 72/19 + k while no county is picked: *"Choose the county you want help
/// in."* / *"…attacked."*
pub const REQUEST_PROMPT: usize = 19;
/// 72/21 + k once one is, with the county's name after it.
pub const REQUEST_PICKED: usize = 21;
/// `Eng_DrawString(100, g_scenarioIndex * 0x14 + county, …)` — the county
/// names, twenty per scenario.
pub const COUNTY_NAME_GROUP: usize = 100;
/// `Ui_DrawCount(value, **0**, …)`, so `L2.eng` group 8 index 0 *"Crown."* or
/// index 1 *"Crowns."* — the noun the amount is drawn with.
pub const CROWN_NOUN: usize = 0;

/// `FUN_00417BD3`'s draft box: `Ui_DrawBoxInterior(0x20, 0xC0, 0x1A, 6)` and
/// `Ui_DrawInsetRect(0x20, 0xC0, 0x1A0, 0x60)` over the top of it, the same
/// 416 × 96 twice.
pub const LETTER_DRAFT: Rect = Rect::new(0x20, 0xC0, 0x1A0, 0x60);
/// `FUN_0040352F(draft, 0x30, 200, 0x180, body, 0x3F)` — the text inside it,
/// wrapped at 384 pixels from (48, 200).
pub const LETTER_DRAFT_TEXT: (i32, i32, i32) = (0x30, 200, 0x180);

/// `FUN_00410C71(county, 0x60, 0xB0)` blits the raster at `(x − 2, y + 3)`.
/// [`PICKER`] is the rectangle the hit test uses, which is the unadjusted one.
pub const PICKER_DRAW: (i32, i32) = (PICKER.x - 2, PICKER.y + 3);

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

    /// One widget record, drawn the way `Widget_Draw` draws it: the button
    /// sheet's frame at the record's own `(x, y)`.
    ///
    /// **`Widget_Draw` is shared and excluded from the draw-call denominator,
    /// but a record is one thing on the screen**, so the four the gift dialog
    /// carries and the two the other six carry are counted separately —
    /// `tools/audit/draws-F.json`. Our own recess stands in when the sheet is
    /// missing, so the button is still a button on a bare install and is
    /// visibly not the original's.
    fn widget(&self, pen: &Pen, canvas: &mut Canvas, frame: usize, r: Rect) {
        if !pen.system_frame(canvas, frame, r.x, r.y) {
            crate::shell::button_recess(canvas, r.x, r.y, WIDGET_DIM, WIDGET_DIM);
        }
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
        // **[V]** Each of the three painters brackets everything it draws in
        // `DAT_0058FE2C = 1`, which is the drop-capital switch: `A` … `Z` come
        // out in colour 1 rather than the caller's `0x3F`. `DAT_005AEA40` — the
        // emboss kill the *front end* uses — is never touched here, and `0x1A`
        // is neither `0x1C` nor `0x1F`, so `Ui_DrawText`'s shadow pair is the
        // ordinary [`font::SHADOW`]. Both facts are `docs/screens-county.md`
        // §4.4's four emboss details, applied rather than quoted.
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: Some(1),
        };
        // `Ui_DrawText(&g_playerNames + target * 0x2C, …)`. **Not `L2.eng`
        // group 7.** This module drew the lord's *title* here — *"The Baron"* —
        // which is what `Game_NewGame` copies into `g_playerNames` for an AI
        // lord but is not what the field holds once a person has typed a name
        // on setup page 4. `screens/county.rs` settled the same question the
        // same way, and carries the same fallback for a world that never came
        // through the front end.
        let name = match ctx.game.player_names[self.target as usize].as_str() {
            n if n.is_empty() => format!("REALM {}", self.target),
            n => n,
        };

        match self.kind {
            // ---------------------------------------- Diplo_DrawGiftGold
            Kind::Gift => {
                pen.window(canvas, GIFT_WINDOW.x, GIFT_WINDOW.y, 0x16, 0x0B, WINDOW_SET);
                pen.ok_button(canvas, GIFT_OK.x, GIFT_OK.y, 0);
                // `Eng_DrawString(72, 10, 0x60, 0xB8)` and the name at
                // `g_penAdvance + 0x60` — two draws, not one formatted string,
                // because the pen advance is what puts the four-pixel gap in.
                let w = pen.eng(canvas, GROUP, GIFT_TO, 0x60, 0xB8, font::TEXT);
                pen.body(canvas, w, 0xB8, &name, font::TEXT);
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
                let w = pen.eng(canvas, GROUP, LAST_GIFT, 0x60, 0xD8, font::TEXT);
                // `Ui_DrawCount` is a number *and* a group 8 noun, and the
                // noun was missing: the line read "Last gift was 40" where the
                // original reads "Last gift was 40 Crowns."
                pen.count(canvas, w, 0xD8, best, CROWN_NOUN, true, font::TEXT);
                pen.eng(canvas, GROUP, GIFT_OF, 0x60, 0xF8, font::TEXT);
                pen.count(canvas, 0x100, 0xF8, self.gold, CROWN_NOUN, true, font::TEXT);
                pen.eng(canvas, GROUP, DISPATCH, 0xA0, 0x120, font::TEXT);
                self.widget(&pen, canvas, PLUS_FRAME, GIFT_MORE);
                self.widget(&pen, canvas, MINUS_FRAME, GIFT_LESS);
            }
            // ------------------------------------ Diplo_DrawCountyRequest
            Kind::AskHelp | Kind::AskAttack => {
                let k = usize::from(self.kind == Kind::AskAttack);
                pen.window(canvas, COUNTY_WINDOW.x, COUNTY_WINDOW.y, 0x18, 0x0F, WINDOW_SET);
                pen.ok_button(canvas, COUNTY_OK.x, COUNTY_OK.y, 0);
                let w = pen.eng(canvas, GROUP, REQUEST_BASE + k, 0x50, 0x98, font::TEXT);
                pen.body(canvas, w, 0x98, &name, font::TEXT);
                // `FUN_00410C71(g_pickedCounty, 0x60, 0xB0)`, and with no
                // county picked the original passes **`0x14`**, not 0:
                //
                // ```c
                // if (g_pickedCounty == 0) FUN_00410c71(0x14, 0x60, 0xb0);
                // else                     FUN_00410c71(g_pickedCounty, …);
                // ```
                //
                // Transcribed rather than tidied. **[I]** the effect is
                // *nothing lit*, because group 100 gives twenty county names
                // per scenario and county 20 is the last of them — England has
                // fourteen — so on most maps the highlight lands on a county
                // that does not exist. A map that really has twenty would light
                // its last one, and no fixture here has one.
                if let Some(m) = ctx.assets.minimap(ctx.game.map_slot) {
                    let owner =
                        |c: u8| ctx.game.kingdom.counties.get(c as usize).map_or(0, |c| c.owner);
                    let lit = if self.county == 0 { 20 } else { self.county };
                    l2_view::chrome::draw_minimap_at(
                        canvas,
                        &m,
                        PICKER_DRAW,
                        lit,
                        &l2_view::chrome::MinimapTint::Owner(&owner),
                    );
                }
                // 72/19 or 20 while no county is picked; 21 or 22 plus the
                // county's own name once one is.
                if self.county == 0 {
                    pen.eng(canvas, GROUP, REQUEST_PROMPT + k, 0x50, 0x140, font::TEXT);
                } else {
                    let w = pen.eng(canvas, GROUP, REQUEST_PICKED + k, 0x50, 0x140, font::TEXT);
                    let index = super::army::county_name_index(ctx, self.county);
                    pen.eng(canvas, COUNTY_NAME_GROUP, index, w, 0x140, font::TEXT);
                }
                pen.eng(canvas, GROUP, DISPATCH, 0x140, 0xE0, font::TEXT);
            }
            // ------------------------------------------ Diplo_DrawLetter
            _ => {
                pen.window(canvas, LETTER_WINDOW.x, LETTER_WINDOW.y, 0x1C, 0x0D, WINDOW_SET);
                pen.ok_button(canvas, LETTER_OK.x, LETTER_OK.y, 0);
                // 72/11 + (kind - 1): "Give a compliment to", "Insult",
                // "Ask for an alliance with", "End alliance with".
                let row = LETTER_BASE + self.kind.byte() as usize - 1;
                let w = pen.eng(canvas, GROUP, row, 0x30, 0xA8, font::TEXT);
                pen.body(canvas, w, 0xA8, &name, font::TEXT);
                // `FUN_00417BD3`, which is drawn **twice a frame**: once by
                // `Diplo_DrawLetter` and again by `Screen_DrawWidgets`'s
                // `0x1A` arm, so the draft repaints without the dialog being
                // repainted. Parchment first, then the recess over it, then the
                // wrapped text — the original's order, and the reason the box
                // is not a hole in the window.
                pen.box_interior(canvas, LETTER_DRAFT.x, LETTER_DRAFT.y, 0x1A, 6);
                pen.inset(canvas, LETTER_DRAFT);
                let (dx, dy, dw) = LETTER_DRAFT_TEXT;
                pen.body_wrapped(canvas, dx, dy, dw, &self.draft, font::TEXT);
                pen.eng(canvas, GROUP, DISPATCH, 0x70, 0x130, font::TEXT);
            }
        }
        let (send, cancel) = self.buttons();
        self.widget(&pen, canvas, THUMB_UP_FRAME, send);
        self.widget(&pen, canvas, THUMB_DOWN_FRAME, cancel);
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

    /// **The three widget tables are three windows onto one array**, and that
    /// is why none of them hides a record the way `g_sendSuppliesWidgets` does.
    ///
    /// Decoded, `0x004DD9D0` runs: `(184,240,f68) (216,240,f66) (288,280,f29)
    /// (324,284,f31) (288,292,f29) (324,296,f31) (320,244,f29) (356,248,f31)`.
    /// The addresses are 24 apart, so `0x004DDA30` is record **4** and
    /// `0x004DDA60` is record **6** — and 4 + 2 = 6, 6 + 2 = 8. Each slice ends
    /// exactly where the next begins.
    #[test]
    fn the_three_widget_tables_are_three_slices_of_one_array() {
        const REC: u32 = 24;
        assert_eq!(WIDGET_TABLE + 4 * REC, 0x004D_DA30, "the letters' table is record 4");
        assert_eq!(WIDGET_TABLE + 6 * REC, 0x004D_DA60, "the requests' table is record 6");
        // Four, then two, then two: no gap and no overhang.
        assert_eq!(0x004D_DA30 + 2 * REC, 0x004D_DA60);
    }

    /// The pair the whole `widgets.js` convention note is anchored on, written
    /// down here so it cannot drift back: **68 is plus.**
    #[test]
    fn frame_sixty_eight_is_the_plus() {
        // `FUN_00436372` reads `if (g_uiHotspotId == 1) g_diploGold += 10`, and
        // the record carrying hotspot id 1 is the frame-68 one at (184, 240).
        assert_eq!(PLUS_FRAME, 68);
        assert_eq!(MINUS_FRAME, 66);
        assert_eq!(GIFT_MORE, Rect::new(184, 240, WIDGET_DIM, WIDGET_DIM));
        assert_eq!(GIFT_STEP, 10);
    }

    /// **All three `Ui_OkButton` calls are in different branches**, so the
    /// "only the last one is clickable" quirk cannot bite here.
    ///
    /// `Ui_OkButton` stashes `(x, y)` into `DAT_0055CE78` / `DAT_0057C8A0` and
    /// keeps only the last call's, so a painter drawing two makes one dead.
    /// `Screen_DiploDialog` is an `if` / `else if` chain on `g_diploKind` —
    /// one arm per frame — so exactly one is drawn and exactly one is live.
    #[test]
    fn each_dialog_draws_exactly_one_close_button_inside_its_own_window() {
        for (ok, w) in
            [(GIFT_OK, GIFT_WINDOW), (LETTER_OK, LETTER_WINDOW), (COUNTY_OK, COUNTY_WINDOW)]
        {
            assert!(ok.x >= w.x && ok.y >= w.y, "{ok:?} starts outside {w:?}");
            assert!(ok.x + ok.w <= w.x + w.w && ok.y + ok.h <= w.y + w.h, "{ok:?} leaves {w:?}");
        }
        // And the three are distinct, which is what makes them three branches
        // rather than one constant drawn three times.
        assert_ne!(GIFT_OK, LETTER_OK);
        assert_ne!(LETTER_OK, COUNTY_OK);
    }

    /// Every `L2.eng` index this half of the module draws is inside group 72's
    /// own run, and the four letter kinds map onto `g_diploKind` 1..=4.
    #[test]
    fn the_compose_indices_are_the_painters_literal_arguments() {
        assert_eq!(GIFT_TO, 10);
        assert_eq!(LAST_GIFT, 23);
        assert_eq!(GIFT_OF, 18);
        assert_eq!(DISPATCH, 17);
        // `Eng_DrawString(0x48, kind + 0xB, …)` with kind = g_diploKind - 1.
        for (k, kind) in
            [Kind::Compliment, Kind::Insult, Kind::OfferAlliance, Kind::EndAlliance]
                .iter()
                .enumerate()
        {
            assert_eq!(LETTER_BASE + kind.byte() as usize - 1, 11 + k);
        }
        // `Eng_DrawString(0x48, kind + 0xF)` / `+ 0x13` / `+ 0x15`, k in 0..2.
        assert_eq!((REQUEST_BASE, REQUEST_PROMPT, REQUEST_PICKED), (15, 19, 21));
    }
}
