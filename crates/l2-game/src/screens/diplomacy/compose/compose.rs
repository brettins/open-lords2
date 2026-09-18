#![allow(unused_imports)]
use super::*;
use super::helper::*;
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

    /// The send and cancel pair is `Diplo_SendClicked`'s — six records across
    /// three layouts at `0x004DDA00`, all **kind 5**, so the gauntlet goes down
    ///
    /// records are `FUN_00436372`'s at `0x004DD9D0`, **kind 4**, so holding
    /// `+` walks the gold up on the ramp.
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

    pub(crate) fn fire(&mut self, ctx: &mut Ctx, widget: usize) -> Transition {
        match widget {
            // `Diplo_SendClicked` (`0x00436408`), hotspot 1.
            0 => self.send(ctx),
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

    pub fn draft(&self) -> String {
        self.letter.as_ref().map_or(String::new(), |f| f.text())
    }

    pub fn committed(&self) -> String {
        self.letter.as_ref().map_or(String::new(), |f| f.commit(LETTER_COMMIT))
    }

    pub fn target(&self) -> u8 {
        self.target
    }

    /// `frame` is the record`s `+0x04`; `Widget_Draw` adds one to it while the
    /// press timer at `+0x0D` runs, and `index` says which record this is in
    /// [`ComposeScreen::widgets`].
    fn widget(&self, pen: &Pen, canvas: &mut Canvas, frame: usize, r: Rect, index: usize) {
        let frame = if self.press.is_pressed(index) { frame + 1 } else { frame };
        if !pen.system_frame(canvas, frame, r.x, r.y) {
            crate::shell::button_recess(canvas, r.x, r.y, WIDGET_DIM, WIDGET_DIM);
        }
    }

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

    fn send(&mut self, ctx: &mut Ctx) -> Transition {
        let refused = refusal(ctx, self.target, self.kind, self.county);
        if let Some(why) = refused {
            self.sent = Some(Err(why));
            return Transition::Pop;
        }
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
                Event::RightClick { .. } => Transition::Replace(ScreenId::Campaign),
                Event::KeyDown(Key::Escape) => Transition::Pop,
                Event::KeyDown(Key::Enter) => self.send(ctx),
                Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft => {
                    let fired = self.press.event(&self.widgets(), event);
                    debug_assert!(fired.is_none(), "no compose widget is kind 3");
                    Transition::Stay
                }
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
        let table = self.widgets();
        if let Some(i) = self.press.event(&table, event) {
            return self.fire(ctx, i);
        }
        Transition::Stay
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
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
        let name = lord_name(ctx, self.target);

        match self.kind {
            Kind::Gift => {
                pen.window(canvas, GIFT_WINDOW.x, GIFT_WINDOW.y, 0x16, 0x0B, WINDOW_SET);
                pen.ok_button(canvas, GIFT_OK.x, GIFT_OK.y, 0);
                let w = pen.eng(canvas, GROUP, GIFT_TO, 0x60, 0xB8, font::TEXT);
                pen.body(canvas, w, 0xB8, &name, font::TEXT);
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
                if self.county == 0 {
                    pen.eng(canvas, GROUP, REQUEST_PROMPT + k, 0x50, 0x140, font::TEXT);
                } else {
                    let w = pen.eng(canvas, GROUP, REQUEST_PICKED + k, 0x50, 0x140, font::TEXT);
                    let index = super::super::super::army::county_name_index(ctx, self.county);
                    pen.eng(canvas, COUNTY_NAME_GROUP, index, w, 0x140, font::TEXT);
                }
                pen.eng(canvas, GROUP, DISPATCH, 0x140, 0xE0, font::TEXT);
            }
            _ => {
                pen.window(canvas, LETTER_WINDOW.x, LETTER_WINDOW.y, 0x1C, 0x0D, WINDOW_SET);
                pen.ok_button(canvas, LETTER_OK.x, LETTER_OK.y, 0);
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


