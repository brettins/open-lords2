#![allow(unused_imports)]
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

/// `Tick_Pulses` (`0x004BBC80`)' 80 ms pulse, kept for the one counter the
/// battlefield reads off it. The dividers and the rounding are
/// [`crate::screens::armoury::Anim`]'s — a pulse is 20 ms rounded **up** to
/// whole frames, and every fourth is `g_pulse80`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Banner {
    acc_ms: u32,
    div: u8,
    /// `DAT_004E5B18`, 0 … 7.
    phase: u8,
}

impl Banner {
    fn tick(&mut self) {
        self.acc_ms += crate::screens::armoury::TICK_MS;
        if self.acc_ms < crate::screens::armoury::PULSE_MS {
            return;
        }
        self.acc_ms = 0;
        self.div += 1;
        if self.div >= crate::screens::armoury::PULSE80_DIVIDER {
            self.div = 0;
            self.phase = (self.phase + 1) % l2_view::scene::BANNER_PHASES;
        }
    }
}

/// **The overview panel's framebuffer and its row cursor.**
///
/// `FUN_004BC1D1` (`0x004BC1D1`) is the whole schedule:
///
/// ```c
/// DAT_004E5D74 += param_1;                                  /* the cursor  */
/// if (DAT_004E6570 - param_1 < DAT_004E5D74) DAT_004E5D74 = 0;   /* 80 rows */
/// if (DAT_004E5D58 == 2) FUN_004BC51A(DAT_004E5D74, param_1);
/// ```
///
/// and its two callers set the rhythm. `Screen_DrawBattlefield` (`0x004233F7`)
/// enters the screen with `g_mapRedraw = 1; FUN_004bc1d1(0x50);` — a full
/// eighty-row pass. `Battle_Frame` (`0x004B99C0`) then runs, once a frame while
/// `g_battlePhase == 2` and `0x27 < g_screenId < 0x2B`:
///
/// ```c
/// if (g_mapRedraw == 0) { FUN_004bc1d1(4);    Gfx_MarkSpriteDirty(0x1E0, 0x18, 10, 10, 1); }
/// else                  { FUN_004bc1d1(0x50); Gfx_MarkAllDirty(); }
/// FUN_004bc142(cameraX, cameraY);      /* …which ends `if (g_mapRedraw) g_mapRedraw--;` */
/// ```
///
/// **So the panel is full only on the frame after it is entered, and four rows
/// a frame — a twenty-frame sweep — for the rest of the battle.** `[V]`; that
/// last decrement is what settles it, and without reading `FUN_004BC142` the
/// obvious reading is that the full pass runs every frame.
///
/// The original paints into the back buffer and the seventy-six rows it did not
/// visit keep the pixels they already had; ours keeps them in a raster of its
/// own and blits the whole of it, because our canvas has a yes/no box and a film
/// pushed over it and the original's screen has neither.
///
/// One difference that follows from that and is left: the original repeats the
/// entry pass every time `Screen_DrawBattlefield` runs, which includes the
/// return from an outcome film. The raster survives the push, so ours does not
/// need to — it is up to twenty frames behind for that one moment instead of
/// none.
struct Overview {
    raster: Canvas,
    /// `DAT_004E5D74`.
    row: usize,
    /// `g_mapRedraw`, as this panel sees it: the next visit paints all eighty
    /// rows. Set on entry, cleared by the visit itself.
    full: bool,
}

impl Overview {
    fn new() -> Overview {
        Overview {
            raster: Canvas::new(l2_view::scene::OVERVIEW_SIDE, l2_view::scene::OVERVIEW_SIDE),
            row: 0,
            full: true,
        }
    }
}

impl Screen for BattlefieldScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Battlefield
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn title(&self, ctx: &Ctx) -> String {
        match ctx.game.battle.as_ref().map(|b| b.mode) {
            Some(Mode::Outcome) => "The battle is over".into(),
            _ => "The battlefield".into(),
        }
    }

    /// **Two files, and the battle's kind picks between them.**
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
    /// Neither is `Battle_LoadAssets`' read. `Palette_Set` (`0x004B0AB5`) is a
    /// plain copy — no remap, no shade table.
    ///
    /// The siege arm was left out because the siege *tileset* was not ported
    /// and *"one without the other would be wrong both ways"*. C200 measured
    /// it the other way round — a siege drew every wall and every man in the
/// field's colours, the whole screen wrong —
    /// and **C201 then ported the tiles**, so neither is wrong now.
    ///
    /// [`BattlefieldScreen::ground`] is the cached flag, because this is
    /// handed no world; one palette serves both castle families.
    fn palette(&self) -> Option<&'static str> {
        Some(self.ground.palette())
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        if ctx.game.battle.is_none() {
            return Transition::Pop;
        }
        // The yes/no box is modal in the original too: `Ui_OpenConfirm` sets
        // `g_screenId = 0x1E`, so none of the battlefield's arms run under it.
        if self.confirm.is_some() {
            // **Kind 5, and the press does not answer.** `Widget_Test`'s
            // kind-5 branch sets `rec[0x0D] = 0x14` and returns *without*
            // calling the handler; the handler runs from the countdown at the
            // top of the next call, on the frame the timer reaches zero. So
            // this returns nothing and [`Screen::update`] gives the answer.
            // A double click is a press to kind 5 as well, and restarts the
            // gauntlet's twenty frames.
            //
            // The two arms are declared on [`CONFIRM_WIDGETS`], beside their kind.
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
                    // `0x2A`'s last clause: the pointer leaving the field ends
                    // the drag. It is tested *after* the two selection guards,
                    //
                    // commits.
                    //
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
                    // **Guard 1 of the `0x29` ladder, and it is the menu bar.**
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
                    // `0x2A` and `0x2B` do **not** have this arm — only `0x29`
                    // — which is what `mode == Mode::Field` is.
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
                // box does *not* also order at the corner it ended on. When it
                // declines (a click that moved nothing and hit nobody) the order
                // arm behind it gets the release, and that is the only way a
                // click on empty ground ever becomes an order.
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
                    // `0x2B`'s whole arm.
                    Mode::Outcome => {
                        live.skip_outcome();
                    }
                    // `0x2A`'s: cancel the drag and go back to `0x29`. The box
                    // that was being drawn is **not** undone — the original
                    // leaves the live selection
                    // it, because the cancel is a screen change and nothing
                    // else.
                    //
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
            // `WM_CHAR`. The battlefield's four key arms are all `WM_KEYDOWN`
            // virtual-key ones — the nine digits and H/V —
            // field on any of the three battle screens, so the character
            // message has nothing to do here. See `crate::text`.
            Event::Text(_) => Transition::Stay,
        }
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        // `g_battleIsSiege` and the castle's level, which the painter reads
        // and [`Screen::palette`] cannot. First statement, ahead of every
        // early return; a change of ground repaints.
        self.redraw |= self.note_ground(ctx);
        // `Tick_Pulses` runs once a frame regardless of the battle's pause, and
        // `BattleBanner_Draw` reads only `g_pulse80` — a paused castle still
        // flies its flag.
        self.banner.tick();
        // `Widget_Test`'s countdown loop, which runs whether or not anything is
        // under the pointer. Index 0 is the tick, index 1 the cross. The first
        // answer closes the box, and a table nobody walks fires nothing more.
        if let Some(widget) = self.press.tick().next() {
            return self.answer_confirm(ctx, widget == 0);
        }
        if self.confirm.is_some() && self.press.any_pressed() {
            // The gauntlet is down; the picture has to move while it is.
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
        // `Battle_Frame`'s own placement: after the tick, before the field is
        // painted, once a frame whatever the pause word says. See [`Overview`].
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
            // arm: 0x00477DFC/outcome-film frame
            //
            // **The push happens on the frame the banner is raised, not after
            // it is painted**, and that is deliberate. `Screen_BattleOutcome`
            // paints the recess and `Battle_CheckOutcome` plays into it in one
            // pass; here `draw` is a frame behind `update`, so the film starts
            // over whatever the last paint left. Holding the push until a flag
            // `draw` sets makes the picture decide the battle — measured:
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
        // `|`, not `||`: both flags are drained on every call.
        std::mem::take(&mut self.redraw) | self.press.take_redraw()
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
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

        // --- the field ---------------------------------------------------
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
        // It follows the man where he is *drawn* — `l2_view::scene::figure_origin`
        // — so it walks with him instead of waiting on the square he is leaving.
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

        // --- the right column ---------------------------------------------
        draw_overview(canvas, &self.overview, live, ink, have_sheets);
        draw_banners(canvas, live, ink);
        for b in Button::ALL {
            let r = b.rect();
            crate::widget::button(canvas, ink, r, b.label(), matches!(b, Button::Pause) && live.paused);
        }

        // --- the pause banner ----------------------------------------------
        if live.paused && live.mode != Mode::Outcome {
            // `FUN_00423B4F`: group 32 index 0 in the heading font at
            // (0x10E, 0x1AE), with system frame 0x4D beside it at (400, 0x1C4).
            let s = ctx.assets.shell.text(GROUP_PAUSED, 0).to_string();
            let s = if s.is_empty() { "PAUSED".to_string() } else { s };
            p.heading(canvas, 0x10E, 0x1AE, &s, font::TEXT);
        }

        // --- the outcome banner --------------------------------------------
        //
        // `Screen_BattleOutcome` (`0x00423241`), the un-animated arm:
        //
        // ```text
        //   FUN_004093E0(0x10, 0x90, 0x1C, 0x0A)      the window, border set 1
        //   Ui_OkButton(0x1A0, 0x100, 0)
        //   Eng_DrawString(0x52, pair*2,     0x30, 0xA8, heading)
        //   FUN_0040328E (0x52, pair*2 + 1,  0x30, 0xE0, 0x180, 100, …)
        // ```
        //
        // **All four of those numbers were ours.** The box was a 416 × 128
        // `fill_rect` of `ink.background` at (32, 96) with an outline over it,
        // and the two strings were laid out inside it by eye. The window is
        // 448 × 160 at (16, 144); the heading starts at (48, 168) and the body
        // is wrapped to 384 at (48, 224). The corner picture was absent
        // entirely.
        //
        // **And the animated arm**, when `g_optAnimations` is set and the local
        // player decided the battle — the one the film plays in:
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

        // --- the yes/no box --------------------------------------------------
        //
        // `Screen_ConfirmBox` (`0x0040CCFA`) is three statements and the first
        // is the ground: `FUN_004093E0(g_confirmX − 0x10, g_confirmY − 0x10,
        // 0xE, 8)` — the shared box in **border set 1**, not a plate of ours.
        // The geometry above was already the original's; the ground was a
        // `fill_rect` of `ink.background`, which is the one colour that looks
        // right under our own palette and is a hole under the game's.
        if let Some(prompt) = self.confirm {
            p.window(canvas, CONFIRM_BOX.x, CONFIRM_BOX.y, CONFIRM_COLS, CONFIRM_ROWS, BOX_SET);
            let s = ctx.assets.shell.text(GROUP_CONFIRM, prompt).to_string();
            let s = if s.is_empty() { ours_confirm(prompt).to_string() } else { s };
            p.body(canvas, 0xA0 + 0x10, 0xA0 + 0x10, &s, font::TEXT);
            // `g_confirmWidgets` (`0x004DD310`) carries frames **29 and 31** —
            // a mailed hand thumb up and thumb down. This drew `system::OK` and
            // `system::OK + 2`, which are the close corner and its neighbour:
            // the right sheet, the wrong frames, and a canvas diff would have
            // passed on either.
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

        // --- the menu bar ----------------------------------------------------
        //
        // **`Screen_DrawMenuBar` (`0x00419C78`), and it is the last thing
        // painted.** `Battle_Frame` (`0x004B99C0`) runs it *after* `Screen_Draw`
        // and `Screen_DrawWidgets`, so the bar sits over everything this
// function has just drawn — so it is here and not at the top,
        // and why the outcome banner's `canvas.remap` above does **not** dim it.
        //
        // The 640 × 24 band at y 0 was blank on this screen: the field starts at
        // `VIEW.y == 24` and nothing filled the strip above it. A player read
        // that as *"the menu buttons are deactivated in battle mode"*, and he
// was looking at an empty bar — the original
        // has **no disabled state anywhere in the menu bar**, on this screen or
        // any other. `true` is `g_battlePhase != 0`: the shields and the
        // year-and-season go, the three titles and the treasury stay.
        crate::screens::map::draw_menu_bar(canvas, ctx, true);

// The cursor kind, printed: the pointer itself is the
        // host's and we have no cursor sheet. It is here because the ladder that
        // chooses it is an arm and a test reads it.
        let _ = live.cursor();
    }
}

/// **Ours**, and it looks it: flat cells and a block per man, for an install
/// with no `T32_bat1.pl8`. `docs/decisions.md` C21 — a stub that is visibly ours
/// beats one that looks finished.
///
/// The blocks stand where the artwork's men would —
/// `l2_view::scene::figure_origin`, `BattleMan_Step`'s cell and trail — so the
/// placeholder walks the way the picture does.
fn draw_placeholder_field(canvas: &mut Canvas, live: &LiveBattle, ink: &l2_view::Ink) {
    let cam = l2_view::scene::Camera::clamped(live.cam.0, live.cam.1);
    canvas.fill_rect(VIEW.x, VIEW.y, VIEW.w, VIEW.h, ink.background);
    for row in 0..VIEW_ROWS {
        for col in 0..VIEW_COLS {
            let cx = live.cam.0 + col;
            let cy = live.cam.1 + row;
            let cell = live.runner.field.at(cx as usize, cy as usize);
            let shade = if cell.impassable() { ink.border } else { ink.dim };
            canvas.fill_rect(VIEW.x + col * TILE, VIEW.y + row * TILE, TILE - 1, TILE - 1, shade);
        }
    }
    for i in 0..live.runner.fighters.len() {
        if !live.runner.is_alive(i) {
            continue;
        }
        let f = &live.runner.fighters[i];
        let (sx, sy) = l2_view::scene::figure_origin(f, cam);
        if !VIEW.contains(sx, sy) {
            continue;
        }
        let c = if f.side == l2_sim::SIDE_A { ink.highlight } else { ink.text };
        canvas.fill_rect(sx + 8, sy + 8, TILE - 16, TILE - 16, c);
    }
}

/// **Cell byte `+5`, as `FUN_004BC51A` reads it, already turned into the
/// `t2_spri.pl8` frame it picks.** One byte a cell: the man's owning realm's
/// `shieldIndex`, `6` for the ownerless, `0` —
/// also the value the original's `if (DAT_005C9288 != 0)` guard drops.
///
/// ```c
/// if (g_battleMen[cell[+5]].owner == 6) colour = 6;
/// else colour = g_realms[g_battleMen[cell[+5]].owner].shieldIndex;
/// ```
///
/// The cell is [`l2_view::scene::drawn_cell`]'s, not `(f.x, f.y)`: byte `+5`
/// moves with `mapX`/`mapY`, and `FUN_00491B1F` moves those at the *start* of a
/// crossing. A man walking east is on the minimap's next cell for the whole of
/// it, as he is in the viewport. **[V]**
fn overview_occupants(game: &crate::Game, live: &LiveBattle) -> Vec<u8> {
    let mut occupants = vec![0u8; l2_sim::terrain::CELLS];
    for i in 0..live.runner.fighters.len() {
        if !live.runner.is_alive(i) {
            continue;
        }
        let f = &live.runner.fighters[i];
        let owner = live.runner.sim.figures[f.sim].owner;
        let colour = if owner == l2_kingdom::levy::OWNERLESS {
            l2_kingdom::levy::OWNERLESS
        } else {
            game.kingdom.realms.get(owner as usize).map_or(0, |r| r.shield_index)
        };
        if colour == 0 {
            continue;
        }
        let ((cx, cy), _) = l2_view::scene::drawn_cell(f);
        if (0..DIM as i32).contains(&cx) && (0..DIM as i32).contains(&cy) {
            occupants[cy as usize * DIM + cx as usize] = colour;
        }
    }
    occupants
}

/// The 80 × 80 field at two pixels a cell — the raster `BattleMap_Click` hit
/// tests, painted by `FUN_004BC51A` (`0x004BC51A`) and scheduled by
/// [`Overview`].
///
///
/// `FUN_004BC51A` draws two things and neither is a rectangle: a terrain tile
/// per cell and a man over it. **[V]** That nothing *else* writes inside
/// `(0x1E0, 0x18)`–`(0x280, 0xB8)` is **[I]**: `Screen_DrawBattlefield`'s three
/// `Misc_bat.pl8` blits all start at `y 0xB8` or below, and the earliest banner
/// in `DAT_004D31F4` is at `y 185`.
///
/// `have_sheets` is false on an install that does not ship `T2_bat1.pl8` beside
/// the executable — the older DOS tree does not — and on
/// [`crate::game::Assets::placeholder`]. Then the panel is a flat fill and a dot
/// a side, which is ours and is marked as ours.
fn draw_overview(
    canvas: &mut Canvas,
    panel: &Overview,
    live: &LiveBattle,
    ink: &l2_view::Ink,
    have_sheets: bool,
) {
    if have_sheets {
        canvas.blit_raster(
            &panel.raster.pixels,
            l2_view::scene::OVERVIEW_SIDE,
            OVERVIEW.x,
            OVERVIEW.y,
            1,
        );
        return;
    }
    canvas.fill_rect(OVERVIEW.x, OVERVIEW.y, OVERVIEW.w, OVERVIEW.h, ink.background);
    for i in 0..live.runner.fighters.len() {
        if !live.runner.is_alive(i) {
            continue;
        }
        let f = &live.runner.fighters[i];
        let c = if f.side == l2_sim::SIDE_A { ink.highlight } else { ink.text };
        canvas.fill_rect(OVERVIEW.x + f.x as i32 * 2, OVERVIEW.y + f.y as i32 * 2, 2, 2, c);
    }
}

/// One banner per figure the player holds, in the layout the count picks.
fn draw_banners(canvas: &mut Canvas, live: &LiveBattle, ink: &l2_view::Ink) {
    let picked = live.runner.selected_fighters(live.owner);
    let layout = BannerLayout::for_count(picked.len());
    for (slot, &f) in picked.iter().enumerate() {
        if slot >= layout.slots {
            break;
        }
        let r = layout.rect(slot);
        canvas.fill_rect(r.x, r.y, r.w, r.h, ink.dim);
        crate::widget::frame(canvas, r, ink.border);
        let troop = live.runner.fighters[f].troop;
        let stem = l2_view::figures::stem(troop).unwrap_or("eng");
        text::draw(canvas, r.x + 2, r.y + 2, &stem.to_uppercase(), ink.text);
    }
}

/// The cursor the ladder picks, exposed for the tests — the picture has no
/// cursor sheet to draw it with.
pub fn cursor_of(live: &LiveBattle) -> Cursor {
    live.cursor()
}

