#![allow(unused_imports)]
use super::*;

use super::*;
use super::drawing::*;
use super::helpers::*;
use super::*;
use l2_sim::runner::Formation;
use l2_sim::terrain::DIM;
use l2_view::{text, Canvas};
use crate::battlefield::{
    self, BannerLayout, Button, Cursor, LiveBattle, Mode, OVERVIEW, TILE, VIEW, VIEW_COLS,
    VIEW_ROWS,
};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::menubar;
use crate::shell::{font, Pen};
use crate::turn::{self, TurnStep};

impl Screen for BattlefieldScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Battlefield
    }

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn title(&self, ctx: &Ctx) -> String {
        match ctx.game.battle.as_ref().map(|b| b.mode) {
            Some(Mode::Outcome) => "The battle is over".into(),
            _ => "The battlefield".into(),
        }
    }

    /// `Screen_DrawBattlefield` (`0x004233F7`) ends every repaint of `0x28`
    /// … `0x2A` with
    ///
    /// ```c
    /// if (g_battleIsSiege == 0) Palette_Set(0x568ee0);   /* t32_bat1.256 */
    /// else                      Palette_Set(0x5675a0);   /* t32_stn1.256 */
    /// ```
    ///
    /// **[V]**, and the two buffers are records 2 and 1 of `g_preloadTable`
    /// (`0x004D9F48`) — the filenames are in the table's own bytes, and
    /// `Res_LoadStatic` (`0x00499859`) is the `local_10 == 1 →
    /// &DAT_005675A0`, `local_10 == 2 → &DAT_00568EE0` ladder that fills them.
    ///
    /// Neither is `Battle_LoadAssets`' read. `Palette_Set` (`0x004B0AB5`) is a
    /// plain copy — no remap, no shade table.
    ///
    /// The siege arm was left out because the siege *tileset* was not ported
    /// and *"one without the other would be wrong both ways"*. C200 measured
    /// it the other way round — a siege drew every wall and every man in the
/// field's colours, the whole screen wrong —
    /// and **C201 then ported the tiles**, so neither is wrong now.
    fn palette(&self) -> Option<&'static str> {
        Some(self.ground.palette())
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        if ctx.game.battle.is_none() {
            return Transition::Pop;
        }
        if self.confirm.is_some() {
            let fired = self.press.event(&CONFIRM_WIDGETS, event);
            if fired.is_some() || self.press.busy() {
                self.redraw = true;
            }
            return Transition::Stay;
        }

        let mode = ctx.game.battle.as_ref().map(|b| b.mode).unwrap_or(Mode::Field);
        match event {
            Event::Pointer { x, y } => {
                let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
                live.pointer = (x, y);
                live.pointer_in = true;
                live.update_hover();
                if mode == Mode::Drag {
                    live.drag_to(x, y);
                    // // arm: 0x0042FF10/drag-leaves-field hover
                    if !live.hover.on_field {
                        live.mode = Mode::Field;
                        live.drag = None;
                    }
                }
                self.redraw = true;
                Transition::Stay
            }
            Event::PointerLeft => {
                let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
                live.pointer_in = false;
                live.update_hover();
                Transition::Stay
            }
            // The `0x29` ladder in `Screen_FrameInput`'s own order: the buttons
            // (`FUN_004329A4`), then the drag (`FUN_0043BF07`), then the order
            // (`FUN_0043C57D`), then the unit panel (`FUN_0043C2A9`), then the
            // epilogue. Each `return` is one of its `goto LAB_00431F25`s.
            Event::Click { x, y } => {
                if mode == Mode::Field {
                    // `Screen_FrameInput`'s arm opens
                    // `Menu_OpenDropdown(&g_menuBarItems, 3)` *before*
                    // `Map_EdgeScroll`, `Battle_ButtonClicked`,
                    // `Battle_DragSelect`, `Battle_OrderClicked` and
                    // `Battle_UnitPanelClicked`, and **nothing gates it** — not
                    // `g_battlePhase`, not the pause word, not
                    // `g_battleChoiceOwner`. So File, Options and Help are live
                    // through a battle and all sixteen rows dispatch: a player
                    // can save, load, start a new game, quit, or open any
                    // options page from the battlefield. **[V]**
                    //
                    // arm: 0x0040DECA/battle-menu-bar left-press
                    if let Some(title) = menubar::title_at(&*ctx, x, y) {
                        return Transition::Push(ScreenId::MenuBar(title));
                    }
                    if let Some(b) = Button::at(x, y) {
                        return self.press_button(ctx, b);
                    }
                    let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
                    live.pointer = (x, y);
                    live.update_hover();
                    if live.press_field(x, y) || live.click_banner(x, y) {
                        self.redraw = true;
                        return Transition::Stay;
                    }
                }
                BattlefieldScreen::epilogue(ctx, x, y, false);
                self.redraw = true;
                Transition::Stay
            }
            Event::Release { x, y } => {
                let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
                live.pointer = (x, y);
                live.update_hover();
                // **The ladder short-circuits.** `FUN_0043BF07` runs first, and
                // when it consumes the release — a committed box or a picked man
                // — `Screen_FrameInput` jumps past `FUN_0043C57D`.
                if live.release_field(x, y) {
                    self.redraw = true;
                    return Transition::Stay;
                }
                self.redraw |= live.order_at(x, y);
                Transition::Stay
            }
            Event::DoubleClick { x, y } => {
                let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
                live.double_click_field(x, y);
                self.redraw = true;
                Transition::Stay
            }
            Event::RightClick { x, y } => {
                let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
                live.pointer = (x, y);
                live.update_hover();
                match live.mode {
                    Mode::Outcome => {
                        live.skip_outcome();
                    }
                    // // arm: 0x0042FF10/cancel-drag right-release
                    Mode::Drag => {
                        live.mode = Mode::Field;
                        live.drag = None;
                    }
                    Mode::Field => {
                        if !live.right_deselect(x, y) {
                            BattlefieldScreen::epilogue(ctx, x, y, true);
                        }
                    }
                }
                self.redraw = true;
                Transition::Stay
            }
            Event::KeyDown(key) => {
                let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
                let acted = match key {
                    Key::Char(c @ '1'..='9') => live.recall_group(c as u8),
                    Key::CtrlChar(c @ '1'..='9') => live.store_group(c as u8),
                    Key::Char('H') => live.key_formation(Formation::Line),
                    Key::Char('V') => live.key_formation(Formation::Column),
                    _ => false,
                };
                self.redraw |= acted;
                Transition::Stay
            }
            Event::Text(_) => Transition::Stay,
            // **The right button's down edge is nothing here.** The epilogue
            // that reads `g_mouseRightPressed` (`0x004EABE0`) is guarded on
            // `g_screenId != 0x12` and on `g_battlePhase == 0`, so it drops no
            // byte during a battle; `0x2A`'s own arms all read the release.
            Event::RightPress { .. } => Transition::Stay,
        }
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        self.redraw |= self.note_ground(ctx);
        self.banner.tick();
        if let Some(widget) = self.press.tick().next() {
            return self.answer_confirm(ctx, widget == 0);
        }
        if self.confirm.is_some() && self.press.any_pressed() {
            self.redraw = true;
        }
        // `Smk_OnFinished`'s one battle line: `if (g_screenId == 0x2B)
        // DAT_00568470 = 0x1389;` — 5001, one past the banner's wait. The film
        // *was* the wait, so the banner leaves with it. A film that failed to
        // open never reaches `Smk_OnFinished`, and then the banner keeps its
        // ordinary five thousand frames.
        let after_film = matches!(ctx.game.films.finished, Some(crate::movie::Film::Battle { .. }));
        if after_film {
            ctx.game.films.finished = None;
        }
        let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
        if after_film {
            live.skip_outcome();
        }
        live.edge_scroll();
        live.tick();
        let done = live.mode == Mode::Outcome && live.outcome_ticks > battlefield::OUTCOME_FRAMES;
        {
            let ctx = Ctx { game: &mut *ctx.game, assets: ctx.assets };
            self.step_overview(&ctx);
        }
        let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
        let raised = live.mode == Mode::Outcome && !self.outcome_seen;
        let decides = live.choice_owner != 0;
        self.redraw |= live.take_redraw();
        if raised {
            self.outcome_seen = true;
            // `Battle_CheckOutcome` (`0x00477DFC`), straight after it raises
            // `0x2B` and paints the banner:
            //
            // ```c
            // if (g_optAnimations != 0 && g_battleChoiceOwner != 0) {
            //     Music_Stop(0);
            //     Smk_Play(bat_win1.smk + (g_battleOutcome * 4 + DAT_0053F084) * 0x10,
            //              0x27, 0x49, 0, g_screenId);
            //     if (3 < ++DAT_0053F084) DAT_0053F084 = 0;
            // }
            // ```
            //
            // `[D]` on one guard it is inside: `g_siegeCount < 2 || !siege`
            // decides whether there is a banner at all, and this engine has no
            // count of the turn's sieges to put in it.
            //
            // arm: 0x00477DFC/outcome-film frame
            //
            // `painting_the_battlefield_with_its_artwork_does_not_change_the_battle`
            // goes red at tick 620. `docs/netcode.md` — presentation state stays
            // out. The film's ground is [`crate::movie::Film::is_over_a_screen`]'s
            // to keep, not this arm's.
            if ctx.game.prefs.animations && decides {
                if let Some(banner) = ctx.game.battle.as_deref().map(|l| outcome_banner(ctx.game, l)) {
                    let take = ctx.game.films.next_battle() as usize;
                    let file = crate::movie::BATTLE_FILMS[banner.min(5)][take];
                    return Transition::Push(ScreenId::Movie(crate::movie::Film::Battle { file }));
                }
            }
        }
        if done {
            return self.settle(ctx);
        }
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw) | self.press.take_redraw()
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        // **The repaint opens black, and that is the bottom band's only
        // painter.** `Screen_DrawBattlefield` (`0x004233F7`) calls
        // `Gfx_ClearScreen` (`0x004B1867`) before its first blit;
        // `Gfx_ClearScreen` is `FUN_004B3E51(DAT_004EA1A8, 0x4B000)`, and
        // `FUN_004B3E51` (`0x004B3E51`) is a `memset` to **0** — 640 × 480
        // bytes, the whole frame. **[V]**
        //
        // Nothing repaints x < `0x1E0`, y ≥ `0x1D8` afterwards: every blit in
        // `Screen_DrawBattlefield` and `FUN_00423530` (`0x00423530`) starts at
        // x `0x1E0`, and every sprite is clipped to `Clip_Vertical(0x18,
        // 0x1D8)`. So the eight rows 472 … 479 left of the column are the
        // clear and nothing else — the 480-line frame's unpainted remainder,
        // not a chrome strip and not the map's last row. The one thing that
        // ever draws into them is the multiplayer heartbeat `FUN_0041A844`
        // (`0x0041A844`), a bar at (2, 476), which is not ported.
        canvas.clear(0);
        self.note_ground(ctx);
        let banner = self.banner_of(ctx);
        // `Screen_DrawBattlefield` (`0x004233F7`) opens the screen with
        // `g_mapRedraw = 1; FUN_004bc1d1(0x50);` — the panel is whole before the
        // first frame is presented, even if no frame has run yet.
        let have_sheets = if self.overview.full {
            self.step_overview(ctx)
        } else {
            ctx.assets.battle.as_ref().is_some_and(|a| a.ground(self.ground).overview.is_some())
        };
        let Some(live) = ctx.game.battle.as_ref() else { return };
        let ink = &ctx.assets.ink;
        let p = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };

        match ctx.assets.battle.as_ref() {
            Some(art) => {
                let cam = l2_view::scene::Camera::clamped(live.cam.0, live.cam.1);
                l2_view::scene::draw(canvas, &live.runner, art, self.ground, cam, banner);
            }
            None => draw_placeholder_field(canvas, live, ink),
        }

        // The selection markers. The original draws a coloured tick over a
        // picked man (`selctd seen`, figure `+0x0A`); ours is a box, and it is
        // ours because that sprite has not been located.
        let cam = l2_view::scene::Camera::clamped(live.cam.0, live.cam.1);
        for f in live.runner.selected_fighters(live.owner) {
            let fig = &live.runner.fighters[f];
            let (sx, sy) = l2_view::scene::figure_origin(fig, cam);
            if VIEW.contains(sx, sy) {
                crate::widget::frame(canvas, Rect::new(sx, sy, TILE, TILE), ink.highlight);
            }
        }

        // **The rubber band, while one is being drawn** — `Battlefield_DrawBand`
        // (`0x0041298A`), the twin of the village's `Village_DrawBand`. Both
        // normalise the box against the live pointer and hand it to
        // `Ui_DrawRectOutline` (`0x00403CF4`) in colour `0x20`; this one clamps
        // to x < `0x1E0` and y `0x18 … 0x1D8`. Ours is un-clamped and in our own
        // ink, which is all that is left of the difference.
        if let Some(d) = live.drag {
            let r = Rect::new(
                d.anchor_px.0.min(d.px.0),
                d.anchor_px.1.min(d.px.1),
                (d.px.0 - d.anchor_px.0).abs().max(1),
                (d.px.1 - d.anchor_px.1).abs().max(1),
            );
            crate::widget::frame(canvas, r, ink.highlight);
        }

        draw_overview(canvas, &self.overview, live, ink, have_sheets);
        let art_column = draw_column_chrome(
            canvas,
            &p,
            ctx.assets.chrome.as_ref(),
            side_shields(ctx.game, live),
            side_men(live),
            live.paused,
        );
        draw_banner_plates(canvas, &p, ctx.assets.chrome.as_ref(), live, ink);
        if !art_column {
            for b in Button::ALL {
                let r = b.rect();
                crate::widget::button(
                    canvas,
                    ink,
                    r,
                    b.label(),
                    matches!(b, Button::Pause) && live.paused,
                );
            }
        }

        if live.paused && live.mode != Mode::Outcome {
            // `FUN_00423B4F`: group 32 index 0 in the heading font at
            // (0x10E, 0x1AE), with system frame 0x4D beside it at (400, 0x1C4).
            let s = ctx.assets.shell.text(GROUP_PAUSED, 0).to_string();
            let s = if s.is_empty() { "PAUSED".to_string() } else { s };
            p.heading(canvas, 0x10E, 0x1AE, &s, font::TEXT);
        }

        // `Screen_BattleOutcome` (`0x00423241`), the un-animated arm:
        //
        // ```text
        //   FUN_004093E0(0x10, 0x90, 0x1C, 0x0A)      the window, border set 1
        //   Ui_OkButton(0x1A0, 0x100, 0)
        //   Eng_DrawString(0x52, pair*2,     0x30, 0xA8, heading)
        //   FUN_0040328E (0x52, pair*2 + 1,  0x30, 0xE0, 0x180, 100, …)
        // ```
        //
        // ```text
        //   FUN_0047703A(); FUN_004B1310();             the field dimmed
        //   FUN_004093E0(0x10, 0x30, 0x1C, 0x16)        a taller window
        //   Ui_DrawInsetRect(0x27, 0x48, 0x192, 0xC2)   the film's well
        //   Ui_OkButton(0x1A0, 0x160, 0)
        //   Eng_DrawString(0x52, pair*2,     0x30, 0x138, heading)
        //   FUN_0040328E (0x52, pair*2 + 1,  0x30, 0x158, 0x180, 100, 0x20, 0x1A0, …)
        // ```
        //
        // `FUN_0047703A` is unread, and `FUN_0040328E`'s last two numbers here
        // differ from every other call of it and are not modelled.
        let animated = ctx.game.prefs.animations && live.choice_owner != 0;
        if live.mode == Mode::Outcome && animated {
            let pair = outcome_banner(ctx.game, live);
            let palette = ctx
                .assets
                .shell
                .palette(l2_view::scene::TILE_PALETTE)
                .unwrap_or(&ctx.assets.palette);
            canvas.remap(&l2_view::canvas::shade_table(palette));
            p.window(canvas, 0x10, 0x30, 0x1C, 0x16, BOX_SET);
            crate::shell::inset_rect(canvas, 0x27, 0x48, 0x192, 0xC2);
            p.ok_button(canvas, 0x1A0, 0x160, 0);
            let head = ctx.assets.shell.text(GROUP_BANNER, pair * 2).to_string();
            let body = ctx.assets.shell.text(GROUP_BANNER, pair * 2 + 1).to_string();
            p.heading(canvas, 0x30, 0x138, &head, font::TEXT);
            p.body_wrapped(canvas, 0x30, 0x158, 0x180, &body, font::TEXT);
        } else if live.mode == Mode::Outcome {
            let pair = outcome_banner(ctx.game, live);
            let head = ctx.assets.shell.text(GROUP_BANNER, pair * 2).to_string();
            let body = ctx.assets.shell.text(GROUP_BANNER, pair * 2 + 1).to_string();
            p.window(canvas, OUTCOME_BOX.x, OUTCOME_BOX.y, OUTCOME_COLS, OUTCOME_ROWS, BOX_SET);
            p.ok_button(canvas, OUTCOME_OK.0, OUTCOME_OK.1, 0);
            let head = if head.is_empty() { "THE BATTLE IS OVER.".into() } else { head };
            p.heading(canvas, OUTCOME_HEAD.0, OUTCOME_HEAD.1, &head, font::TEXT);
            if !body.is_empty() {
                p.body_wrapped(canvas, OUTCOME_BODY.0, OUTCOME_BODY.1, OUTCOME_BODY.2, &body, font::TEXT);
            }
        }

        // `Screen_ConfirmBox` (`0x0040CCFA`) is three statements and the first
        // is the ground: `FUN_004093E0(g_confirmX − 0x10, g_confirmY − 0x10,
        // 0xE, 8)` — the shared box in **border set 1**, not a plate of ours.
        if let Some(prompt) = self.confirm {
            p.window(canvas, CONFIRM_BOX.x, CONFIRM_BOX.y, CONFIRM_COLS, CONFIRM_ROWS, BOX_SET);
            let s = ctx.assets.shell.text(GROUP_CONFIRM, prompt).to_string();
            let s = if s.is_empty() { ours_confirm(prompt).to_string() } else { s };
            p.body(canvas, 0xA0 + 0x10, 0xA0 + 0x10, &s, font::TEXT);
            // `g_confirmWidgets` (`0x004DD310`) carries frames **29 and 31** —
            // a mailed hand thumb up and thumb down. This drew `system::OK` and
            // `system::OK + 2`, which are the close corner and its neighbour:
            //
            // **And the pressed picture is `base + 1`.** `Widget_Draw`
            // (`0x0040CFD2`) is
            // `if (kind == 4 || kind == 5) { frame = rec[0x04]; if (rec[0x0D])
            // frame = rec[0x04] + 1; }` — a *sprite index*, not a colour
            // effect — and it then marks the rectangle with
// `Gfx_MarkWidgetUrgent` so the
            // depressed picture appears on the same frame as the press. This is
            // the only place in this engine that draws one.
            for (i, (r, frame, label)) in
                [(CONFIRM_YES, CONFIRM_YES_FRAME, "YES"), (CONFIRM_NO, CONFIRM_NO_FRAME, "NO")]
                    .into_iter()
                    .enumerate()
            {
                let frame = if self.press.is_pressed(i) { frame + 1 } else { frame };
                let drawn = ctx
                    .assets
                    .chrome
                    .as_ref()
                    .is_some_and(|c| c.draw_system(canvas, frame, r.x, r.y));
                if !drawn {
                    crate::widget::button(canvas, ink, r, label, false);
                }
            }
        }

        // **`Screen_DrawMenuBar` (`0x00419C78`), and it is the last thing
        // painted.** `Battle_Frame` (`0x004B99C0`) runs it *after* `Screen_Draw`
        // and `Screen_DrawWidgets`, so the bar sits over everything this
// function has just drawn — so it is here and not at the top,
        // and why the outcome banner's `canvas.remap` above does **not** dim it.
        crate::screens::map::draw_menu_bar(canvas, ctx, true);

        let _ = live.cursor();
    }
}


