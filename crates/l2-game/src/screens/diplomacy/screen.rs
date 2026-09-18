#![allow(unused_imports)]
use super::*;

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


impl DiplomacyScreen {
    pub fn new() -> DiplomacyScreen {
        DiplomacyScreen { target: None, press: Press::new(), pending_rows: [None; MENU_SLOTS] }
    }

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

    /// `Diplo_DefaultTarget` (`0x004A1E6C`) takes the first in-play realm that
    /// is not the local player, or 0.
    pub fn target(&self, ctx: &Ctx) -> u8 {
        let t = self.target.unwrap_or(0);
        if t != 0 && ctx.game.kingdom.realms.get(t as usize).is_some_and(|r| r.strength != 0) {
            return t;
        }
        l2_kingdom::diplomacy::default_target(&ctx.game.kingdom.realms, ctx.game.player)
    }

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

    fn is_overlay(&self) -> bool {
        true
    }

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
                Event::RightClick { .. } => Transition::Pop,
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
        if let Some(h) = menu.inset_height() {
            pen.inset(canvas, Rect::new(MENU_INSET_X, MENU_INSET_Y, MENU_INSET_W, h));
        }
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
            let w = menu_widget(slot);
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
        pen.inset(canvas, r);
        let frame = super::super::message::face_frame(rr.lord, rr.is_human, realm);
        let drew = pen
            .assets
            .sheet(FACES)
            .and_then(|s| s.frame(frame))
            .map(|b| canvas.blit(&b, r.x + 1, r.y + 1))
            .is_some();
        if !drew {
            let title = pen.assets.text(LORD_TITLE_GROUP, rr.lord.min(4) as usize).to_string();
            pen.body(canvas, r.x + 4, r.y + 4, &title, font::TEXT);
        }
        let shield = rr.shield_index.clamp(1, 5);
        if !pen.misc_frame(canvas, SHIELD_BASE + shield as usize, 0x20, r.y + 6) {
            let colour = ink.realm[shield as usize];
            canvas.fill_rect(0x20, r.y + 6, 12, 10, colour);
        }
        pen.body(canvas, 0x20, r.y + 0x52, &lord_name(ctx, realm), font::TEXT);
        if realm == target {
            // Two `FUN_00403CF4` outlines, one pixel apart, in the painter's
            // own two palette indices — `0xF9` inside `0x3F`.
            pen.outline(canvas, 0x2F, r.y - 1, 0x54, 0x50, SELECTED_INNER);
            pen.outline(canvas, 0x2E, r.y - 2, 0x56, 0x52, SELECTED_OUTER);
        }

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
                None => {
                    pen.body(canvas, x, y, letter, font::HIGHLIGHT);
                }
            }
        }

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


