//! **The battlefield** — `g_screenId` `0x29`, its drag mode `0x2A`, and the
//! outcome banner `0x2B`.
//!
//! The state and the rules are [`crate::battlefield`]; this is the screen that
//! drives them and the picture. One [`ScreenId`] covers all three, because all
//! three are the same painter over the same globals and the difference between
//! them is which arms run — see [`crate::battlefield::Mode`].
//!
//! # Every arm of all three screens, and the function it reproduces
//!
//! `CLAUDE.md` rule 5. The middle column is `Screen_FrameInput`'s
//! (`0x0042FF10`) own dispatch order, which is a **ladder of guards**: the first
//! one that consumes the input ends the frame. Ours is the same ladder in the
//! same order, and where a guard declines we fall through exactly as it does.
//!
//! ## `0x29` — the field
//!
//! | # | arm | the original |
//! |---|---|---|
//! | 1 | the menu bar's three titles | `Menu_OpenDropdown` `0x0040DECA` — **not reproduced** |
//! | 2 | the pointer at the edge of the screen scrolls one cell | `Map_EdgeScroll` `0x00432221` |
//! | 3 | five buttons at (480, 448) | `FUN_004329A4` `0x004329A4`, table `0x004DC710` |
//! | 4 | left press on the field starts a box | `FUN_0043BF07` `0x0043BF07` |
//! | 5 | left release on the ground orders | `FUN_0043C57D` `0x0043C57D` |
//! | 6 | left press on a banner drops that man | `FUN_0043C2A9` `0x0043C2A9` |
//! | 7 | **right release clears the selection** | `FUN_0043C55C` `0x0043C55C` |
//! | 8 | the overview panel orders, or looks | `BattleMap_Click` `0x00432443` |
//! | 9 | the pointer picks one of four cursors | `Battle_Frame` `0x004B99C0`'s ladder |
//! | 10 | `1` … `9` recall a group | `FUN_0043C910` `0x0043C910` |
//! | 11 | `Ctrl` + `1` … `9` store one | `FUN_0043C885` `0x0043C885` |
//! | 12 | `H` forms a line | `FUN_0043C77A(0)` `0x0043C77A` |
//! | 13 | `V` forms a column | `FUN_0043C77A(1)` |
//! | 14 | `F2` cycles the debug panel | `0x004B29BE` — **not reproduced**, and gated on a debug flag |
//! | 15 | the arrow keys walk the debug figure | `0x004B29BE` — **not reproduced** |
//! | 16 | a right release dismisses the message scroll | `FUN_0047685D` — not this screen's |
//!
//! ## `0x2A` — the drag
//!
//! | # | arm | the original |
//! |---|---|---|
//! | 17 | the box follows the pointer | `FUN_0043BF07`'s held branch |
//! | 18 | release commits it | `FUN_0043BF07`'s release branch, `FUN_00479CF7` |
//! | 19 | **a double click commits it too** | the same branch's second test |
//! | 20 | right release cancels | inline, `g_screenId = 0x29` |
//! | 21 | the pointer leaving the field cancels | inline, `g_battleHoverOnField == 0` |
//! | 22 | the cursor is forced to the plain arrow | `Battle_Frame`'s ladder |
//!
//! ## `0x2B` — the outcome banner
//!
//! | # | arm | the original |
//! |---|---|---|
//! | 23 | a right release skips the five-thousand-frame wait | inline, `DAT_00568470 = 0x1389` |
//! | 24 | the wait itself | `Battle_CheckOutcome` `0x00477DFC` |
//!
//! ## `0x28`
//!
//! **Unreachable.** See [`crate::battlefield`]'s header: no instruction in the
//! binary writes `0x28` to `g_screenId`. Its two arms — edge scroll, and a right
//! release that would take it to `0x29` — are dead code and are deliberately not
//! built.
//!
//! # The yes/no box is ours for now
//!
//! Two of the five buttons open `Ui_OpenConfirm` (`0x0040E6F2`), which is screen
//! `0x1E`, a screen this tree does not have. The prompt indices are the
//! original's — `L2.eng` group 10, index 12 *"Retreat from field?"*, 11
//! *"Surrender castle?"*, 9 *"Autocalc battle?"* — and so is the geometry, a
//! 14 × 8 cell box at `(g_confirmX − 16, g_confirmY − 16)` with the thumbs at
//! `+ (64, 46)` and `+ (112, 50)`. What is ours is that it is drawn by this
//! screen instead of by `Screen_ConfirmBox` on a screen of its own.

use l2_sim::runner::Formation;
use l2_view::chrome::system;
use l2_view::{text, Canvas};

use crate::battlefield::{
    self, BannerLayout, Button, Cursor, LiveBattle, Mode, OVERVIEW, TILE, VIEW, VIEW_COLS,
    VIEW_ROWS,
};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};
use crate::turn::{self, TurnStep};

/// `L2.eng` group 10 — the game's directory of confirmable actions.
pub const GROUP_CONFIRM: usize = 10;
/// `L2.eng` group 32 index 0 — what `FUN_00423B4F` prints across the bottom of
/// a paused battlefield.
pub const GROUP_PAUSED: usize = 32;
/// `L2.eng` group 82 — the seven outcome heading/body pairs, drawn on `0x2B`.
pub const GROUP_BANNER: usize = 82;

/// `Ui_OpenConfirm(prompt, 0xA0, 0xA0, …)` and `Screen_ConfirmBox`
/// (`0x0040CCFA`): a 14 × 8 cell box at `(0xA0 − 0x10, 0xA0 − 0x10)`.
pub const CONFIRM_BOX: Rect = Rect::new(0xA0 - 0x10, 0xA0 - 0x10, 14 * 16, 8 * 16);
/// `g_confirmWidgets` (`0x004DD310`), offset by `(g_confirmX, g_confirmY)`.
pub const CONFIRM_YES: Rect = Rect::new(0xA0 + 64, 0xA0 + 46, 32, 32);
pub const CONFIRM_NO: Rect = Rect::new(0xA0 + 112, 0xA0 + 50, 32, 32);

/// The battlefield, for as long as [`crate::game::Game::battle`] is `Some`.
pub struct BattlefieldScreen {
    /// The open yes/no box's `L2.eng` group 10 prompt index, if one is up.
    confirm: Option<usize>,
    redraw: bool,
}

impl BattlefieldScreen {
    pub fn new() -> BattlefieldScreen {
        BattlefieldScreen { confirm: None, redraw: true }
    }

    fn live<'a>(ctx: &'a mut Ctx) -> Option<&'a mut LiveBattle> {
        ctx.game.battle.as_deref_mut()
    }

    /// The five buttons, in table order — `FUN_004329A4`'s `Hotspot_Test`.
    ///
    /// // arm: 0x004329A4/buttons left-press
    fn press_button(&mut self, ctx: &mut Ctx, b: Button) -> Transition {
        let is_siege = ctx.game.battle.as_ref().is_some_and(|l| l.is_siege());
        let garrison_is_local = ctx
            .game
            .battle
            .as_ref()
            .and_then(|l| ctx.game.kingdom.campaign.units.get(l.defender))
            .is_some_and(|u| u.owner == ctx.game.player);
        let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
        match b {
            Button::Pause => {
                live.press_pause();
            }
            Button::Retreat => self.confirm = live.press_retreat(),
            Button::Sally => {
                let _ = is_siege;
                let _ = live.press_sally(garrison_is_local);
            }
            Button::Charge => {
                live.press_charge();
            }
            Button::Autocalc => self.confirm = live.press_autocalc(),
        }
        self.redraw = true;
        Transition::Stay
    }

    /// The screen `0x1E` the two confirm buttons open, answered.
    fn answer_confirm(&mut self, ctx: &mut Ctx, yes: bool) -> Transition {
        self.confirm = None;
        self.redraw = true;
        if !yes {
            return Transition::Stay;
        }
        if let Some(live) = BattlefieldScreen::live(ctx) {
            live.confirm_autocalc();
        }
        self.settle(ctx)
    }

    /// The battle is over, one way or the other: hand it back to the turn.
    fn settle(&mut self, ctx: &mut Ctx) -> Transition {
        match turn::finish_battle(ctx.game) {
            TurnStep::Report(_) => Transition::Replace(ScreenId::BattleResult),
            TurnStep::Ask(_) => Transition::Replace(ScreenId::BattlePrompt),
            TurnStep::Running | TurnStep::Done(_) | TurnStep::Stuck => Transition::Pop,
        }
    }

    /// `Screen_FrameInput`'s **epilogue**, which runs after every arm on every
    /// screen but `0x12`: `if ((left || right) && FUN_004323FE())`, and
    /// `FUN_004323FE` is `BattleMap_Click` while `g_battlePhase == 2`. So the
    /// overview panel is live on the field, during a drag, and under the outcome
    /// banner alike.
    fn epilogue(ctx: &mut Ctx, x: i32, y: i32, right: bool) -> bool {
        match BattlefieldScreen::live(ctx) {
            Some(live) => live.click_overview(x, y, right),
            None => false,
        }
    }
}

impl Default for BattlefieldScreen {
    fn default() -> Self {
        BattlefieldScreen::new()
    }
}

impl Screen for BattlefieldScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Battlefield
    }

    fn title(&self, ctx: &Ctx) -> String {
        match ctx.game.battle.as_ref().map(|b| b.mode) {
            Some(Mode::Outcome) => "The battle is over".into(),
            _ => "The battlefield".into(),
        }
    }

    /// `Battle_LoadAssets` reads `T32_bat1.256` and `Palette_Set`s it: the
    /// battlefield does not run under the campaign palette.
    fn palette(&self) -> Option<&'static str> {
        Some(l2_view::scene::TILE_PALETTE)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        if ctx.game.battle.is_none() {
            return Transition::Pop;
        }
        // The yes/no box is modal in the original too: `Ui_OpenConfirm` sets
        // `g_screenId = 0x1E`, so none of the battlefield's arms run under it.
        if let Some(_prompt) = self.confirm {
            return match event {
                Event::Click { x, y } if CONFIRM_YES.contains(x, y) => {
                    self.answer_confirm(ctx, true)
                }
                Event::Click { x, y } if CONFIRM_NO.contains(x, y) => {
                    self.answer_confirm(ctx, false)
                }
                _ => Transition::Stay,
            };
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
                    // so a release on the very frame the pointer leaves still
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
                // — `Screen_FrameInput` jumps past `FUN_0043C57D`. So a finished
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
                    // leaves the live selection exactly as the last motion left
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
            // virtual-key ones — the nine digits and H/V — and there is no text
            // field on any of the three battle screens, so the character
            // message has nothing to do here. See `crate::text`.
            Event::Text(_) => Transition::Stay,
        }
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
        live.edge_scroll();
        live.tick();
        let done = live.mode == Mode::Outcome && live.outcome_ticks > battlefield::OUTCOME_FRAMES;
        self.redraw |= live.take_redraw();
        if done {
            return self.settle(ctx);
        }
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw)
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
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
                l2_view::scene::draw(canvas, &live.runner, art, cam);
            }
            None => draw_placeholder_field(canvas, live, ink),
        }

        // The selection markers. The original draws a coloured tick over a
        // picked man (`selctd seen`, figure `+0x0A`); ours is a box, and it is
        // ours because that sprite has not been located.
        for f in live.runner.selected_fighters(live.owner) {
            let fig = &live.runner.fighters[f];
            let (sx, sy) = cell_to_screen(live, fig.x, fig.y);
            if VIEW.contains(sx, sy) {
                crate::widget::frame(canvas, Rect::new(sx, sy, TILE, TILE), ink.highlight);
            }
        }

        // The rubber band, while one is being drawn.
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
        draw_overview(canvas, live, ink);
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
        if live.mode == Mode::Outcome {
            let pair = outcome_pair(ctx, live);
            let head = ctx.assets.shell.text(GROUP_BANNER, pair * 2).to_string();
            let body = ctx.assets.shell.text(GROUP_BANNER, pair * 2 + 1).to_string();
            let box_r = Rect::new(0x20, 0x60, 416, 128);
            canvas.fill_rect(box_r.x, box_r.y, box_r.w, box_r.h, ink.background);
            crate::widget::frame(canvas, box_r, ink.border);
            let head = if head.is_empty() { "THE BATTLE IS OVER.".into() } else { head };
            p.heading(canvas, box_r.x + 16, box_r.y + 24, &head, font::TEXT);
            if !body.is_empty() {
                p.body_wrapped(canvas, box_r.x + 16, box_r.y + 64, box_r.w - 32, &body, font::TEXT);
            }
        }

        // --- the yes/no box --------------------------------------------------
        if let Some(prompt) = self.confirm {
            canvas.fill_rect(CONFIRM_BOX.x, CONFIRM_BOX.y, CONFIRM_BOX.w, CONFIRM_BOX.h, ink.background);
            crate::widget::frame(canvas, CONFIRM_BOX, ink.border);
            let s = ctx.assets.shell.text(GROUP_CONFIRM, prompt).to_string();
            let s = if s.is_empty() { ours_confirm(prompt).to_string() } else { s };
            p.body(canvas, 0xA0 + 0x10, 0xA0 + 0x10, &s, font::TEXT);
            for (r, frame, label) in
                [(CONFIRM_YES, system::OK, "YES"), (CONFIRM_NO, system::OK + 2, "NO")]
            {
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

        // The cursor kind, printed rather than drawn: the pointer itself is the
        // host's and we have no cursor sheet. It is here because the ladder that
        // chooses it is an arm and a test reads it.
        let _ = live.cursor();
    }
}

/// `L2.eng` group 10's three battle prompts, for an install without it.
fn ours_confirm(prompt: usize) -> &'static str {
    match prompt {
        9 => "AUTOCALC BATTLE?",
        11 => "SURRENDER CASTLE?",
        _ => "RETREAT FROM FIELD?",
    }
}

fn outcome_pair(ctx: &Ctx, live: &LiveBattle) -> usize {
    let winner_is_mine = live
        .conclusion
        .map(|c| {
            let side_b_won = c.winner == l2_sim::SIDE_B;
            let mine = ctx
                .game
                .kingdom
                .campaign
                .units
                .get(if side_b_won { live.attacker } else { live.defender })
                .is_some_and(|u| u.owner == ctx.game.player);
            mine
        })
        .unwrap_or(false);
    match (live.is_siege(), winner_is_mine) {
        (false, true) => 0,
        (false, false) => 1,
        (true, true) => 2,
        (true, false) => 3,
    }
}

/// Where a battlefield cell's top-left pixel is, or off-screen.
fn cell_to_screen(live: &LiveBattle, x: u8, y: u8) -> (i32, i32) {
    (
        VIEW.x + (x as i32 - live.cam.0) * TILE,
        VIEW.y + (y as i32 - live.cam.1) * TILE,
    )
}

/// **Ours**, and it looks it: flat cells and a block per man, for an install
/// with no `T32_bat1.pl8`. `docs/decisions.md` C21 — a stub that is visibly ours
/// beats one that looks finished.
fn draw_placeholder_field(canvas: &mut Canvas, live: &LiveBattle, ink: &l2_view::Ink) {
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
        let (sx, sy) = cell_to_screen(live, f.x, f.y);
        if !VIEW.contains(sx, sy) {
            continue;
        }
        let c = if f.side == l2_sim::SIDE_A { ink.highlight } else { ink.text };
        canvas.fill_rect(sx + 8, sy + 8, TILE - 16, TILE - 16, c);
    }
}

/// The 80 × 80 field at two pixels a cell — the raster `BattleMap_Click` hit
/// tests.
fn draw_overview(canvas: &mut Canvas, live: &LiveBattle, ink: &l2_view::Ink) {
    canvas.fill_rect(OVERVIEW.x, OVERVIEW.y, OVERVIEW.w, OVERVIEW.h, ink.background);
    for i in 0..live.runner.fighters.len() {
        if !live.runner.is_alive(i) {
            continue;
        }
        let f = &live.runner.fighters[i];
        let c = if f.side == l2_sim::SIDE_A { ink.highlight } else { ink.text };
        canvas.fill_rect(OVERVIEW.x + f.x as i32 * 2, OVERVIEW.y + f.y as i32 * 2, 2, 2, c);
    }
    // Where the viewport is looking.
    crate::widget::frame(
        canvas,
        Rect::new(
            OVERVIEW.x + live.cam.0 * 2,
            OVERVIEW.y + live.cam.1 * 2,
            VIEW_COLS * 2,
            VIEW_ROWS * 2,
        ),
        ink.border,
    );
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
