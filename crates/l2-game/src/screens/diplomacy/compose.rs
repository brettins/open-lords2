#![allow(unused_imports)]
use super::*;
use super::main::*;
use super::tests::*;
use l2_kingdom::diplomacy::{group, Kind};
use l2_kingdom::realm::MAX_REALMS;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::message::lord_name;
use crate::shell::{font, Pen};
use crate::widget;

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

/// The button-sheet frames those records name. `System.pl8`
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
// the words are checked against `L2.eng` in `crates/l2-game/tests/shell/main.rs`.
// **The check is on existence and these indices are verified against the
// words**

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

/// `Edit_Begin(&g_diploLetterDraft + (kind - 1) * 200, 200, 10000, 0)` —
/// `Diplo_OpenCompliment` (`0x0043618B`) and its three siblings. The character
/// limit is **200**, the pixel limit 10,000, which no draft box reaches, so
/// only the character limit bites. **[V]**
pub const LETTER_MAX_LEN: usize = 200;
pub const LETTER_MAX_PIXELS: i32 = 10_000;

/// `Edit_Commit(g_diploLetterDraft + (kind - 1) * 200, 199)` — the harvest in
/// `Screen_HandleInput`'s `0x1A` arm. **199, one less than the field takes**,
/// so the two-hundredth character a person may type is the one a send drops.
/// **[V]**
pub const LETTER_COMMIT: usize = 199;

/// `Eng_CopyString(g_diploLetterDraft + k * 200, 0xE2, k, 200)` in
/// `Options_SetDefaults` (`0x004AE310`): the four drafts a game starts with,
/// **`L2.eng` group 226 indices 0…3**, each cut at the first character below
/// `0x20`. One consumer, so `CLAUDE.md` rule 6 makes it this screen's
/// vocabulary. **[V]**
pub const LETTER_DEFAULT_GROUP: usize = 226;

/// The same four transcribed, for an install with no `L2.eng`.
const LETTER_DEFAULT_OURS: [&str; 4] = [
    "Verily, your oppression of the weak and your flattery of the strong are worthy of emulation.  Pray, teach me more.",
    "You are ugly, and your mother dresses you funny.",
    "Sire, these are troubled times, I ask you to forget our past differences. We needs must fight together, to see off those that would do us harm.",
    "It pleases me to report that happier times seem to be upon us now my friend. We now no longer have need for our pact. I shall remember your loyalty ere I lift the crown.",
];

/// Where the caret lands inside the wrapped draft — the pen position
/// `FUN_0040352F` hands back to `FUN_00417BD3` as `g_caretX`, `g_caretY`.
///
/// **[I] on the column.** Our wrap splits on whitespace and drops it, so a
/// caret standing on a run of spaces is drawn at the end of the word before
/// them; the original measures the spaces. Elsewhere the two agree.
fn caret_pen(pen: &Pen, text: &str, caret: usize, x: i32, y: i32, width: i32) -> (i32, i32) {
    use crate::text::Metrics;
    let m = crate::text::FontMetrics::of(pen.assets);
    let line_h = pen.assets.body.as_ref().map_or(16, |f| f.line);
    let head: String = text.chars().take(caret).collect();
    let rows = pen.wrap(&head, width);
    let row = rows.len().saturating_sub(1) as i32;
    let last: Vec<char> = rows.last().map_or_else(Vec::new, |s| s.chars().collect());
    (x + m.width(&last), y + row * line_h)
}

/// The draft a letter of `kind` (1…4) opens with.
///
/// The cut at the first control character is `Options_SetDefaults`' own loop,
/// and it **latches**: everything from the first byte under `0x20` onwards is
/// zeroed, not only that byte.
pub fn letter_default(ctx: &Ctx, kind: Kind) -> String {
    let k = (kind.byte() as usize).saturating_sub(1).min(3);
    let from_eng = ctx.assets.shell.text(LETTER_DEFAULT_GROUP, k);
    let s = if from_eng.is_empty() { LETTER_DEFAULT_OURS[k] } else { from_eng };
    s.split(|c: char| (c as u32) < 0x20).next().unwrap_or("").to_string()
}

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
/// **`None` covers two different things**:
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

impl ComposeScreen {
    pub fn new(target: u8, kind: u8) -> ComposeScreen {
        ComposeScreen {
            target,
            kind: Kind::from_byte(kind).unwrap_or(Kind::Gift),
            gold: 0,
            county: 0,
            letter: None,
            sent: None,
            press: Press::new(),
        }
    }

    /// **The dialog's widget table, with the kind byte each record carries.**
    ///
    /// The send and cancel pair is `Diplo_SendClicked`'s — six records across
    /// three layouts at `0x004DDA00`, all **kind 5**, so the gauntlet goes down
    ///
    /// records are `FUN_00436372`'s at `0x004DD9D0`, **kind 4**, so holding
    /// `+` walks the gold up on the ramp.
    ///
    /// Index 0 is send, 1 is cancel, 2 is `+` and 3 is `−`.
    ///
    /// Each `arm!` is the marker and the kind in one token. The gift stepper's
    /// two records share one handler and one arm, so they share one
    /// declaration.
    pub(super) fn widgets(&self) -> Vec<Widget> {
        const GIFT_STEP: crate::press::Kind = crate::arm!("0x00436372/diplo-gift-step", Repeat);
        let (send, cancel) = self.buttons();
        let mut out = vec![
            Widget::new(send, crate::arm!("0x00436408/diplo-send", Delayed)),
            Widget::new(cancel, crate::arm!("0x00436408/diplo-cancel", Delayed)),
        ];
        if self.kind == Kind::Gift {
            out.push(Widget::new(GIFT_MORE, GIFT_STEP));
            out.push(Widget::new(GIFT_LESS, GIFT_STEP));
        }
        out
    }

    /// One widget's handler, whichever way it was reached. The arms are
    /// declared on [`ComposeScreen::widgets`].
    fn fire(&mut self, ctx: &mut Ctx, widget: usize) -> Transition {
        match widget {
            // `Diplo_SendClicked` (`0x00436408`), hotspot 1.
            0 => self.send(ctx),
            // Hotspot 0 of the same handler, and it is the whole of the
            // function's first statement: `g_screenId = 0xB`, back to the lord
            // cards.
            1 => Transition::Pop,
            // `FUN_00436372`, the gift stepper.
            i => {
                let read: &Ctx = ctx;
                self.step_gift(read, if i == 2 { GIFT_STEP } else { -GIFT_STEP });
                Transition::Stay
            }
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Whether this shape has a letter in it: `g_diploKind` 1…4, which is the
    /// `else if (g_diploKind < 5)` rung of the `0x1A` arm.
    pub fn is_letter(&self) -> bool {
        matches!(self.kind, Kind::Compliment | Kind::Insult | Kind::OfferAlliance | Kind::EndAlliance)
    }

    /// The draft editor, seeded on first use — `Edit_Begin` (`0x00402009`) as
    /// `Diplo_OpenCompliment` (`0x0043618B`) calls it.
    fn field(&mut self, ctx: &Ctx) -> &mut crate::text::TextField {
        let kind = self.kind;
        self.letter.get_or_insert_with(|| {
            crate::text::TextField::begin(
                &letter_default(ctx, kind),
                LETTER_MAX_LEN,
                LETTER_MAX_PIXELS,
                crate::text::Kind::Text,
            )
        })
    }

    /// What is in the draft box now. Empty for the three shapes that have none.
    pub fn draft(&self) -> String {
        self.letter.as_ref().map_or(String::new(), |f| f.text())
    }

    /// `Edit_Commit(…, 199)` — what a send copies into
    /// `g_diploLetter + localPlayer * 0xCA`.
    pub fn committed(&self) -> String {
        self.letter.as_ref().map_or(String::new(), |f| f.commit(LETTER_COMMIT))
    }

    pub fn target(&self) -> u8 {
        self.target
    }

    /// One widget record, drawn the way `Widget_Draw` draws it: the button
    /// sheet's frame at the record's own `(x, y)`.
    ///
    /// **`Widget_Draw` is shared and excluded from the draw-call denominator,
    /// but a record is one thing on the screen**, so the four the gift dialog
    /// carries
    /// `tools/audit/draws-F.json`. Our own recess stands in when the sheet is
    /// missing, so the button is still a button on a bare install and is
    /// visibly not the original's.
    /// `frame` is the record`s `+0x04`; `Widget_Draw` adds one to it while the
    /// press timer at `+0x0D` runs, and `index` says which record this is in
    /// [`ComposeScreen::widgets`].
    fn widget(&self, pen: &Pen, canvas: &mut Canvas, frame: usize, r: Rect, index: usize) {
        let frame = if self.press.is_pressed(index) { frame + 1 } else { frame };
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
    /// clamp is on every click**
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
    /// the draft is copied into the player's per-realm slot second
    /// validation is third. A refusal is a message on the map.
    fn send(&mut self, ctx: &mut Ctx) -> Transition {
        let refused = refusal(ctx, self.target, self.kind, self.county);
        if let Some(why) = refused {
            self.sent = Some(Err(why));
            return Transition::Pop;
        }
        // `if (g_diploKind != 0) g_diploGold = 0;` — only a gift carries gold,
//
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

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// A countdown or a held stepper changed the screen with no event: the
    /// gift stepper's `FUN_00436372` sets `g_redrawRequest = 2`. See
    /// [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Dispatch a message".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // arm: 0x0042FF10/compose-letter-text type
        //
        // `Screen_HandleInput`'s `0x1A` arm, the `else if (g_diploKind < 5)`
        // rung: `Widget_Test(0x004DDA30, 2)` first, and **only when it
        // declines** `g_editActive = 1; Edit_Recount(); Edit_Commit(draft +
        // (kind - 1) * 200, 199)`. A click on send or cancel therefore never
        // reaches the field
        // takes keys alone. `Edit_Commit` is the harvest out of the one shared
        // editor at `0x005CD550`, not the entry — `docs/diplomacy.md` §10.10.
        if self.is_letter() {
            let assets = ctx.assets;
            let m = crate::text::FontMetrics::of(&assets.shell);
            if self.field(ctx).event(event, &m) {
                return Transition::Stay;
            }
        }
        let Event::Click { x, y } = event else {
            return match event {
                // arm: 0x0042FF10/compose-right-exit right-release
                //
                // The `0x1A` arm's right-release, and note **where it goes**:
                // `g_screenId = 0`, the campaign map, not back to `0x0B`. Only
                // the cross inside the dialog returns to the lord cards. Two
                // exits from one screen that land in different places
                // difference is not visible from the dialog.
                Event::RightClick { .. } => Transition::Replace(ScreenId::Campaign),
                Event::KeyDown(Key::Escape) => Transition::Pop,
                Event::KeyDown(Key::Enter) => self.send(ctx),
                // The release ends a gift stepper's hold; nothing here is a
                // release widget, so nothing can fire.
                Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft => {
                    let fired = self.press.event(&self.widgets(), event);
                    debug_assert!(fired.is_none(), "no compose widget is kind 3");
                    Transition::Stay
                }
                // **A double click reaches the four widgets and nothing else.**
                // Send and cancel are kind 5 and the gift stepper kind 4
                // guarded by `g_mouseLeftPressed || g_mouseLeftDoubleClick`; the
                // county picker `FUN_0043B4CB` opens `else if
                // (g_mouseLeftPressed == 0) return 0`
                // `Ui_OkButtonClicked`, a release. `[V]` This screen dropped it.
                Event::DoubleClick { .. } => match self.press.event(&self.widgets(), event) {
                    Some(i) => self.fire(ctx, i),
                    None => Transition::Stay,
                },
                _ => Transition::Stay,
            };
        };
        // arm: 0x0043B4CB/compose-pick-county left-press
        //
        // `FUN_0043B4CB(0x60, 0xB0)` — a 128 × 128 county raster drawn at
        // (96, 176), read straight out of `g_minimapCounty`. It is tested
        // **before** the corner button, and a pixel that resolves to county 0
        // is *not* a hit
        // and can leave the screen. `g_diploKind < 5` short-circuits it: the
        // other five dialogs have no map on them.
        if matches!(self.kind, Kind::AskHelp | Kind::AskAttack) {
            if let Some(county) = county_at_picker(ctx, x, y) {
                self.county = county;
                return Transition::Stay;
            }
        }
        // Four widgets of two kinds; [`ComposeScreen::widgets`] says which is
        // which and [`ComposeScreen::fire`] is what each one does.
        let table = self.widgets();
        if let Some(i) = self.press.event(&table, event) {
            return self.fire(ctx, i);
        }
        Transition::Stay
    }

    /// `Widget_Test`'s per-frame pass: the gauntlets' countdown
    /// stepper's ramp.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        // The caret's blink. `Edit_DrawCaret` counts frames in itself; ours
        // steps on a tick, because nothing below the renderer reads a clock.
        // See [`crate::text`].
        if let Some(f) = self.letter.as_mut() {
            f.tick();
        }
        for i in self.press.tick() {
            let t = self.fire(ctx, i);
            if t != Transition::Stay {
                return t;
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
// out in colour 1. `DAT_005AEA40` — the
        // emboss kill the *front end* uses — is never touched here, and `0x1A`
        // is neither `0x1C` nor `0x1F`, so `Ui_DrawText`'s shadow pair is the
        // ordinary [`font::SHADOW`]. Both facts are `docs/screens-county.md`
// §4.4's four emboss details, applied.
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: Some(1),
        };
        // `Ui_DrawText(&g_playerNames + target * 0x2C, …)` — the typed name
        // first, which is what this module once got wrong by drawing the lord's
        // *title* unconditionally. [`lord_name`] keeps that order and puts the
        // title back only where the field is empty, which is where the front
        // end would have written it.
        let name = lord_name(ctx, self.target);

        match self.kind {
            // ---------------------------------------- Diplo_DrawGiftGold
            Kind::Gift => {
                pen.window(canvas, GIFT_WINDOW.x, GIFT_WINDOW.y, 0x16, 0x0B, WINDOW_SET);
                pen.ok_button(canvas, GIFT_OK.x, GIFT_OK.y, 0);
                // `Eng_DrawString(72, 10, 0x60, 0xB8)`
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
                // `Ui_DrawCount` is a number *and* a group 8 noun
                // noun was missing: the line read "Last gift was 40" where the
                // original reads "Last gift was 40 Crowns."
                pen.count(canvas, w, 0xD8, best, CROWN_NOUN, font::TEXT);
                pen.eng(canvas, GROUP, GIFT_OF, 0x60, 0xF8, font::TEXT);
                pen.count(canvas, 0x100, 0xF8, self.gold, CROWN_NOUN, font::TEXT);
                pen.eng(canvas, GROUP, DISPATCH, 0xA0, 0x120, font::TEXT);
                self.widget(&pen, canvas, PLUS_FRAME, GIFT_MORE, 2);
                self.widget(&pen, canvas, MINUS_FRAME, GIFT_LESS, 3);
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
// Transcribed. **[I]** the effect is
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
                    let index = super::super::army::county_name_index(ctx, self.county);
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
                // wrapped text — the original's order
                pen.box_interior(canvas, LETTER_DRAFT.x, LETTER_DRAFT.y, 0x1A, 6);
                pen.inset(canvas, LETTER_DRAFT);
                let (dx, dy, dw) = LETTER_DRAFT_TEXT;
                let draft = self.draft();
                pen.body_wrapped(canvas, dx, dy, dw, &draft, font::TEXT);
                // `FUN_00417BD3` ends `Edit_DrawCaret(0x005AF8F0, 0x3F)` at the
                // pen position `FUN_0040352F` left — `g_caretX + 0x30`,
                // `g_caretY + 200` — so the caret of a *wrapped* field is an
                // absolute point, not the anchor plus the field's own width.
                // [`crate::text::TextField::draw_caret`] adds that width back,
                // so the anchor handed to it is the point less `caret_x`.
                if let Some(f) = self.letter.as_ref() {
                    let m = crate::text::FontMetrics::of(a);
                    let (cx, cy) = caret_pen(&pen, &draft, f.caret(), dx, dy, dw);
                    f.draw_caret(canvas, cx - f.caret_x(&m), cy, font::TEXT, &m);
                }
                pen.eng(canvas, GROUP, DISPATCH, 0x70, 0x130, font::TEXT);
            }
        }
        let (send, cancel) = self.buttons();
        self.widget(&pen, canvas, THUMB_UP_FRAME, send, 0);
        self.widget(&pen, canvas, THUMB_DOWN_FRAME, cancel, 1);
    }
}

