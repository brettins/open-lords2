#![allow(unused_imports)]
use super::*;
use super::compose::*;
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

    /// `Ui_DrawInsetRect(0xD0, 0x60, 0xE8, h)` — the recess behind the menu, and
    /// the height is the only thing the four arms vary about it.
    ///
    /// **The dispatched layout draws none**, which is the reason this returns an
/// option: the one-line *"A message has been
    /// dispatched, my Lord."* sits directly on the window's parchment.
    pub fn inset_height(self) -> Option<i32> {
        match self {
            Menu::NoAlly => Some(0xD0),
            Menu::Allied => Some(0x130),
            Menu::AlliedElsewhere => Some(0xA0),
            Menu::Dispatched => None,
        }
    }

    /// Whether `Pl8_DrawFrame(g_miscCtySheet, 0x1D, 0x140, 0x140)` runs.
    ///
    /// **Three of the four** — its
    /// recess is `0x130` tall and reaches `0x60 + 0x130 = 0x190`, past the
/// picture's own `0x140`. Read out of the four arms
    /// about; the geometry is offered as the likely *why* and is not evidence.
    pub fn draws_seal(self) -> bool {
        self != Menu::Allied
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
///
pub fn menu_widget(slot: usize) -> Rect {
    Rect::new(400, 102 + slot as i32 * 50, 32, 32)
}

/// `g_diploWidgets` holds six records; the allied layout uses all six.
pub(super) const MENU_SLOTS: usize = 6;

/// **`g_diploWidgets` as a table** — one record per row the layout draws,
/// every one **kind 5**, read out of `+0x0F` of `0x004DD940` … `0x004DD9B8`.
/// The dispatched layout has none: `g_diploWidgetCount = 0`.
///
/// The `arm!` is the six handlers' one marker — `Diplo_OpenGift`,
/// `…Compliment`, `…Insult`, `…Alliance`, `…AskHelp`, `…AskAttack` differ only
/// in the `g_diploKind` they set and in which draft buffer they clear —
/// kind they are answered with.
fn menu_widgets(menu: Menu) -> Vec<Widget> {
    if menu == Menu::Dispatched {
        return Vec::new();
    }
    (0..menu.rows().len().min(MENU_SLOTS))
        .map(|slot| {
            Widget::new(menu_widget(slot), crate::arm!("0x00436141/diplo-open-compose", Delayed))
        })
        .collect()
}

/// The label beside a menu widget. `FUN_0040328E(72, row, 0xE0, y, 0xA0, 100)`
/// puts the text at x = 0xE0 and the first row at y = 0x70, 50 apart — so the
/// text sits 32 pixels above its own widget, which is the original's layout and
/// not a mistake here.
pub fn menu_label_y(slot: usize) -> i32 {
    0x70 + slot as i32 * 50
}

// ------------------------------------------------------------ the 0x0B screen

impl DiplomacyScreen {
    pub fn new() -> DiplomacyScreen {
        DiplomacyScreen { target: None, press: Press::new(), pending_rows: [None; MENU_SLOTS] }
    }

    /// **A press or a double click on the verb buttons.** True when one went
    /// down; the dialog opens from [`Screen::update`] twenty ticks later.
    fn press_menu(&mut self, ctx: &Ctx, event: Event) -> bool {
        let menu = Menu::of(ctx, self.target(ctx));
        let table = menu_widgets(menu);
        let fired = self.press.event(&table, event);
        debug_assert!(fired.is_none(), "every verb button is kind 5");
        let (Event::Click { x, y } | Event::DoubleClick { x, y }) = event else { return false };
        let Some(slot) = table.iter().position(|w| w.rect.contains(x, y)) else { return false };
        self.pending_rows[slot] = Some(menu.rows()[slot]);
        true
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
        "Diplomacy".into()
    }

    /// The painter draws over whatever was underneath and clears nothing.
    fn is_overlay(&self) -> bool {
        true
    }

    /// `Widget_Test`'s countdown over `g_diploWidgets`: the verb button the
    /// player pressed opens its dialog when its twenty frames run out.
    ///
    /// A second verb button whose twenty frames are still running when the
    /// first opens its dialog **waits under the dialog**: the machine ticks the
    /// top screen only, as the original's dispatcher walks `g_diploWidgets` on
    /// `0x0B` only, and it opens its own dialog when this screen is back on top.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        for slot in self.press.tick() {
            let Some(row) = self.pending_rows.get_mut(slot).and_then(Option::take) else {
                continue;
            };
            let Some(kind) = Menu::kind_of_row(row) else { continue };
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            let target = self.target(&read);
            return Transition::Push(ScreenId::DiploCompose(target, kind.byte()));
        }
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **A double click reaches the verb buttons and nothing else.** They
        // are `Widget_Test` kind 5, whose guard reads `g_mouseLeftPressed ||
        // g_mouseLeftDoubleClick`, so it puts the button down and restarts its
        // twenty frames; the card pick `FUN_004369BD` opens
        // `if (g_mouseLeftPressed != 0)`
        // a release. `[V]` This screen dropped it.
        if let Event::DoubleClick { .. } = event {
            self.press_menu(ctx, event);
            return Transition::Stay;
        }
        let Event::Click { x, y } = event else {
            return match event {
                // arm: 0x0042FF10/diplo-right-exit right-release
                //
                // `Screen_FrameInput`'s `0x0B` arm: `if (rightReleased) {
                // g_screenId = 0; }` **before** it even asks about the corner
                // button
                // modal guards. Right-click leaves the screen — the gesture
                // `docs/agents.md` records this project as systematically
                // missing.
                Event::RightClick { .. } => Transition::Pop,
                // `Ui_OkButtonClicked` — the corner picture
                // `Ui_OkButton(0x1A8, 0x1A6)` drew, hit-tested as a **24 x 24**
                // box at that origin **on the left button's release**, which is
                // the function's first statement and was ours to get right:
                // this answered on the press. Our rectangle is the 32-pixel
                // button.
                // arm: 0x0040E7E4/diplo-ok left-release
                Event::Release { x, y } if OK.contains(x, y) => Transition::Pop,
                Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
                _ => Transition::Stay,
            };
        };
        // arm: 0x004369BD/diplo-pick-lord left-press
        //
        // `FUN_004369BD` walks the same cards the painter stacked and hit-tests
        // each card rectangle; the first hit becomes `g_diploTarget`. It runs
        // **after** the widget test
        // the menu — which cannot happen
        // cards end at 0x30 + 0x52 = 130.
        //
        // The six widget handlers are one arm, declared on [`menu_widgets`].
        // `Diplo_OpenAlliance` is the one with a decision in it, and
        // [`Menu::kind_of_row`] is that decision: row 6 is only drawn when the
        // target is already my ally.
        //
        // **Kind 5**, so this does not open the dialog: it puts the button
        // down and [`Screen::update`] opens it twenty ticks later.
        if self.press_menu(ctx, event) {
            return Transition::Stay;
        }
        for (slot, realm) in DiplomacyScreen::cards(ctx).iter().enumerate() {
            if card_rect(slot).contains(x, y) {
                self.target = Some(*realm);
                return Transition::Stay;
            }
        }
        Transition::Stay
    }

    /// `Diplo_DrawScreen` (`0x00416CF3`), statement for statement.
    ///
    /// **This painter drew none of the original's ground.** Every rectangle on
    /// it was a `widget::panel` of ours in the interface's own `Ink`: the
    /// window, each lord card, and one filled box per menu row —
    /// of those is not a box the original has at all. It draws **one**
    /// `Ui_DrawInsetRect` behind the whole menu, whose height is the layout's,
    ///
    /// `System.pl8` frame **64**.
    ///
    /// The card is the sharper case and it is `docs/decisions.md` C61's
    /// armoury bug again: `Ui_DrawInsetRect` is **four lines and no fill**, so
    /// filling the card painted a hole in the window's parchment — invisible
    /// under our palette, where `ink.panel` *is* the parchment colour, and
    /// black under a real one.
    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let a = &ctx.assets.shell;
        // `Diplo_DrawScreen` sets neither `DAT_0058FE2C` (drop capitals) nor
        // `DAT_005AEA40` (the emboss kill), and `0x0B` is neither `0x1C` nor
        // `0x1F`, so the shadow pair is the ordinary one. The three compose
// painters below set the caps flag, so their pen differs.
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        // `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)` — border set **1**.
        pen.window(canvas, WINDOW.x, WINDOW.y, WINDOW_COLS, WINDOW_ROWS, WINDOW_SET);
        // `Ui_OkButton(0x1A8, 0x1A6, 0)`: `System.pl8` frame `0x33`, an arrow
        // pointing into a hole. **Not the word OK**, which is what this screen
        // drew — the last of the three `Ui_OkButton` inventions the draw audit
        // found
        pen.ok_button(canvas, OK.x, OK.y, 0);

        let target = self.target(ctx);
        for (slot, realm) in DiplomacyScreen::cards(ctx).iter().enumerate() {
            self.draw_card(&pen, ctx, canvas, slot, *realm, target);
        }

        // `Ui_DrawText(&g_playerNames + target * 0x2C, 0xD0, 0x3D,
        // &g_fontHeading, 0x3F)` — **`g_playerNames`, not `L2.eng` group 7**,
        // and the heading font. This screen drew the lord's *title* here
        // is only what `Game_NewGame` seeds the field with; a person who typed
        // a name on setup page 4 saw somebody else's. `ComposeScreen` below and
        // `screens/county.rs` had already settled the same question.
        pen.heading(canvas, 0xD0, 0x3D, &lord_name(ctx, target), font::TEXT);

        let menu = Menu::of(ctx, target);
        // **One inset behind the whole menu, and its height is the layout's.**
        // `Ui_DrawInsetRect(0xD0, 0x60, 0xE8, h)` with `h` `0xD0`, `0x130` or
        // `0xA0`; the dispatched layout draws no inset at all.
        if let Some(h) = menu.inset_height() {
            pen.inset(canvas, Rect::new(MENU_INSET_X, MENU_INSET_Y, MENU_INSET_W, h));
        }
        // `Pl8_DrawFrame(g_miscCtySheet, 0x1D, 0x140, 0x140)` — drawn on three
        // of the four layouts and **not on the allied one**, whose taller inset
// reaches down over that corner. Transcribed.
        if menu.draws_seal() {
            pen.misc_frame(canvas, SEAL_FRAME, SEAL_AT.0, SEAL_AT.1);
        }
        for (slot, row) in menu.rows().iter().enumerate() {
            if menu == Menu::Dispatched {
                // `FUN_0040328E(72, 24, 0xE0, 0xA2, 0xA0, 100, …)` — index 24
                // alone, at the second row's y, and no widget under it.
                let s = a.text(GROUP, *row).to_string();
                pen.body_wrapped(canvas, MENU_LABEL_X, 0xA2, MENU_LABEL_W, &s, font::TEXT);
                break;
            }
            // `Widget_Draw(0, 0, &g_diploWidgets, g_diploWidgetCount)`: six
            // records at (400, 102 + 50n), every one carrying `System.pl8`
            // frame 64. **[V]** `tools/oracle/widgets.js widgets 4dd940 6`.
            // Our own outline is the picture-is-missing fallback, not the
            // picture — it used to be a filled panel standing in for it.
            let w = menu_widget(slot);
            // Kind 5, so `Widget_Draw` shows `base + 1` for the twenty frames
            // between the press and the dialog opening.
            let frame = if self.press.is_pressed(slot) { MENU_FRAME + 1 } else { MENU_FRAME };
            if !pen.system_frame(canvas, frame, w.x, w.y) {
                widget::frame(canvas, w, ink.border);
            }
            // `FUN_0040328E(72, row, 0xE0, y, 0xA0, 100, …)` — **wrapped** at
            // 160 pixels, and not uppercased: every string on this screen used
            // to be `.to_uppercase()`d, which is a spelling the game does not
            // have.
            let s = a.text(GROUP, *row).to_string();
            pen.body_wrapped(canvas, MENU_LABEL_X, menu_label_y(slot), MENU_LABEL_W, &s, font::TEXT);
        }
    }
}

/// `Diplo_DrawLordCard` (`0x004171EE`), statement for statement.
impl DiplomacyScreen {
    fn draw_card(
        &self,
        pen: &Pen,
        ctx: &Ctx,
        canvas: &mut Canvas,
        slot: usize,
        realm: u8,
        target: u8,
    ) {
        let ink = pen.ink;
        let me = ctx.game.player;
        let r = card_rect(slot);
        let rr = &ctx.game.kingdom.realms[realm as usize];
        // `Ui_DrawInsetRect(0x30, slot*100 + 0x31, 0x52, 0x4E)` — **no fill.**
        pen.inset(canvas, r);
        // `Sprite_WGenSprite(lord*3 - 3, 0x31, slot*100 + 0x32)`, frame 12 for
        // a human rival, out of the `faces.pl8` the painter has just read.
        // [`super::message::face_frame`] is that rule, already ported.
        let frame = super::super::message::face_frame(rr.lord, rr.is_human, realm);
        let drew = pen
            .assets
            .sheet(FACES)
            .and_then(|s| s.frame(frame))
            .map(|b| canvas.blit(&b, r.x + 1, r.y + 1))
            .is_some();
        if !drew {
            // OURS, and only with no `Faces.pl8`: the lord's title where his
            // face belongs, so an install without the sheet still says who this
            // card is.
            let title = pen.assets.text(LORD_TITLE_GROUP, rr.lord.min(4) as usize).to_string();
            pen.body(canvas, r.x + 4, r.y + 4, &title, font::TEXT);
        }
        // `Pl8_DrawFrame(g_miscCtySheet, shieldIndex + 0x55, 0x20,
        // slot*100 + 0x37)` — the same `Misc_cty` banner run the menu bar draws
        // its realm flags from
        // **in the realm record** before using it.
        let shield = rr.shield_index.clamp(1, 5);
        if !pen.misc_frame(canvas, SHIELD_BASE + shield as usize, 0x20, r.y + 6) {
            let colour = ink.realm[shield as usize];
            canvas.fill_rect(0x20, r.y + 6, 12, 10, colour);
        }
        // `Ui_DrawText(&g_playerNames + realm * 0x2C, 0x20, slot*100 + 0x83,
        // &g_fontBody, 0x3F)` — below the card, not inside it.
        pen.body(canvas, 0x20, r.y + 0x52, &lord_name(ctx, realm), font::TEXT);
        if realm == target {
            // Two `FUN_00403CF4` outlines, one pixel apart, in the painter's
            // own two palette indices — `0xF9` inside `0x3F`.
            pen.outline(canvas, 0x2F, r.y - 1, 0x54, 0x50, SELECTED_INNER);
            pen.outline(canvas, 0x2E, r.y - 2, 0x56, 0x52, SELECTED_OUTER);
        }

        // The three status icons, all read out of the **rival's** record
        // indexed by me, and all `Sprite_WGenSprite` frames of `faces.pl8`
// `allied` and `atWar` are exclusive in the
        // painter: the at-war icon is only drawn when the allied one was not.
        let their = rr.pair(me);
        let mut icons: Vec<(usize, i32, i32, &str)> = Vec::new();
        if their.has_mail {
            icons.push((MAIL_ICON, 0x1E, r.y + 0x1F, "M"));
        }
        if their.allied {
            icons.push((ALLIED_ICON, 0x9E, r.y + 0x10, "A"));
        } else if their.at_war {
            icons.push((AT_WAR_ICON, 0x92, r.y + 0x10, "W"));
        }
        for (frame, x, y, letter) in icons {
            match pen.assets.sheet(FACES).and_then(|s| s.frame(frame)) {
                Some(b) => canvas.blit(&b, x, y),
                // OURS, and only with no `Faces.pl8`.
                None => {
                    pen.body(canvas, x, y, letter, font::HIGHLIGHT);
                }
            }
        }

        // **The thermometer is only drawn for an AI rival.** The painter's
        // whole block is inside `if (g_realms[realm].isHuman == 0)`, which is
        // the one thing about it this screen did not have — a human rival got a
        // bar of a standing nothing maintains.
        if rr.is_human {
            return;
        }
        let standing = i32::from(their.standing);
        // `Ui_DrawInsetRect(0x88, slot*100 + 0x40, 10, 0x3F)`, then
        // `FUN_0040437D(0x89, slot*100 + 0x41, 8, 0x3D, 0x3F)` — the recess,
        // then the whole column in the empty colour.
        pen.inset(
            canvas,
            Rect::new(THERMOMETER_X, r.y + 0xF, THERMOMETER_W, THERMOMETER_H),
        );
        let (bx, by) = (THERMOMETER_X + 1, r.y + 0x10);
        canvas.fill_rect(bx, by, THERMOMETER_FILL_W, THERMOMETER_FILL_H, THERMOMETER_EMPTY);
        // The fill loop, transcribed: `for (v = 30; v > -31; v--)` filling row
        // `30 - v` when `v <= standing`. So the column fills **downward from
        // the standing's own row**
// standing.
        let colour = if standing >= i32::from(THERMOMETER_WARM) {
            THERMOMETER_HIGH
        } else if standing <= i32::from(THERMOMETER_COLD) {
            THERMOMETER_LOW
        } else {
            THERMOMETER_MID
        };
        for row in 0..THERMOMETER_FILL_H {
            if 30 - row <= standing {
                canvas.fill_rect(bx, by + row, THERMOMETER_FILL_W, 1, colour);
            }
        }
    }
}

