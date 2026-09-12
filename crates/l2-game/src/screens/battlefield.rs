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
//! # The yes/no box: everything but the screen it lives on
//!
//! Two of the five buttons open `Ui_OpenConfirm` (`0x0040E6F2`), which is screen
//! `0x1E`, a screen this tree does not have. The prompt indices are the
//! original's — `L2.eng` group 10, index 12 *"Retreat from field?"*, 11
//! *"Surrender castle?"*, 9 *"Autocalc battle?"* — and so is the geometry, a
//! 14 × 8 cell box at `(g_confirmX − 16, g_confirmY − 16)` with the thumbs at
//! `+ (64, 46)` and `+ (112, 50)`. **What is ours is that it is drawn by this
//! screen instead of by `Screen_ConfirmBox` on a screen of its own** — and that
//! is now the only thing left that is: the ground is `Screen_ConfirmBox`'s own
//! `FUN_004093E0(…, 0xE, 8)` in border set 1 rather than a plate of ours, and
//! the two pictures are `g_confirmWidgets`' frames 29 and 31 rather than the
//! close corner and its neighbour.
//!
//! # The outcome banner, both arms, and the film
//!
//! `Screen_BattleOutcome` (`0x00423241`) has three arms. The short window is
//! the one with animations off. The animated arm — `g_optAnimations` set and
//! the local player one of the two sides — dims the field, draws a taller
//! window with a 402 × 194 recess at (39, 72) and moves its text down 168
//! pixels, and `Battle_CheckOutcome` then plays one of
//! [`crate::movie::BATTLE_FILMS`] in the recess; when the film ends the banner
//! goes with it. The third arm — `g_battleChoiceOwner == 0`, a battle between
//! two other realms — is the neutral pair 12/13. Which pair is
//! `Battle_SelectOutcomeBanner`'s four-way siege reading, [`outcome_banner`].

use l2_sim::runner::Formation;
use l2_view::{text, Canvas};

use crate::battlefield::{
    self, BannerLayout, Button, Cursor, LiveBattle, Mode, OVERVIEW, TILE, VIEW, VIEW_COLS,
    VIEW_ROWS,
};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
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

/// **The border set every one of these boxes is drawn in.** `FUN_004093E0` is
/// `Ui_DrawBoxBorder(1, …)` followed by `Ui_DrawBoxInterior` inset a cell —
/// the four-argument form is *always* set 1, and both painters below use it.
pub const BOX_SET: usize = 1;

/// `Ui_OpenConfirm(prompt, 0xA0, 0xA0, …)` and `Screen_ConfirmBox`
/// (`0x0040CCFA`): a 14 × 8 cell box at `(0xA0 − 0x10, 0xA0 − 0x10)`.
pub const CONFIRM_BOX: Rect = Rect::new(0xA0 - 0x10, 0xA0 - 0x10, 14 * 16, 8 * 16);
pub const CONFIRM_COLS: i32 = 14;
pub const CONFIRM_ROWS: i32 = 8;
/// `g_confirmWidgets` (`0x004DD310`), offset by `(g_confirmX, g_confirmY)`.
pub const CONFIRM_YES: Rect = Rect::new(0xA0 + 64, 0xA0 + 46, 32, 32);
pub const CONFIRM_NO: Rect = Rect::new(0xA0 + 112, 0xA0 + 50, 32, 32);
/// **Frames 29 and 31, and they are not a tick and a cross**: decoded, the pair
/// is a mailed hand with its thumb up and its thumb down. Every yes/no in the
/// game draws these two. `docs/screens-county.md` §4.2, and the symbol comment
/// on `g_confirmWidgets` says the same from the table's own bytes.
pub const CONFIRM_YES_FRAME: usize = 29;
pub const CONFIRM_NO_FRAME: usize = 31;

/// **`g_confirmWidgets` as a table, with the kind byte its two records carry.**
///
/// Both are `Widget_Test` kind **5**, read out of `0x004DD310` and `0x004DD328`:
/// the press puts the gauntlet down and the handler runs
/// [`crate::press::DELAYED_FRAMES`] frames later. That delay, with the picture
/// visibly held down through it, is what a player read as *"the game waited on
/// mouse-up, and the gauntlet would go down slightly when clicked."*
///
/// Order matters: index 0 is hotspot id **1**, the tick, and index 1 is id
/// **0**, the cross. `Ui_ConfirmClicked` (`0x00434E1F`) is
/// `g_confirmAnswer = g_uiHotspotId; (*g_confirmCallback)();` — the answer *is*
/// the hotspot id.
///
/// The two `arm!`s are the arms' markers, and each is the kind it declares.
pub const CONFIRM_WIDGETS: [Widget; 2] = [
    Widget::new(CONFIRM_YES, crate::arm!("0x00434E1F/confirm-yes", Delayed)),
    Widget::new(CONFIRM_NO, crate::arm!("0x00434E1F/confirm-no", Delayed)),
];

/// `Screen_BattleOutcome` (`0x00423241`)'s short window —
/// `FUN_004093E0(0x10, 0x90, 0x1C, 0x0A)` — and the three things inside it.
pub const OUTCOME_BOX: Rect = Rect::new(0x10, 0x90, 0x1C * 16, 0x0A * 16);
pub const OUTCOME_COLS: i32 = 0x1C;
pub const OUTCOME_ROWS: i32 = 0x0A;
/// `Ui_OkButton(0x1A0, 0x100, 0)`. **Decorative**: `Screen_FrameInput`'s `0x2B`
/// arm never calls `Ui_OkButtonClicked`, so the only way off the original's
/// outcome screen is a right-click. The picture is an instruction, not a
/// target, and ours is the same picture for the same reason.
pub const OUTCOME_OK: (i32, i32) = (0x1A0, 0x100);
/// `Eng_DrawString(0x52, pair*2, 0x30, 0xA8, &g_fontHeading, 0x3F)`.
pub const OUTCOME_HEAD: (i32, i32) = (0x30, 0xA8);
/// `FUN_0040328E(0x52, pair*2 + 1, 0x30, 0xE0, 0x180, 100, 0, 0, …)`.
pub const OUTCOME_BODY: (i32, i32, i32) = (0x30, 0xE0, 0x180);

/// The battlefield, for as long as [`crate::game::Game::battle`] is `Some`.
pub struct BattlefieldScreen {
    /// The open yes/no box's `L2.eng` group 10 prompt index, if one is up.
    confirm: Option<usize>,
    /// [`CONFIRM_WIDGETS`]' press timer — the twenty frames between the
    /// gauntlet going down and the answer being given.
    press: Press,
    redraw: bool,
    /// Whether this screen has seen the battle reach `0x2B` — the edge
    /// `Battle_CheckOutcome` plays its film on.
    outcome_seen: bool,
}

impl BattlefieldScreen {
    pub fn new() -> BattlefieldScreen {
        BattlefieldScreen { confirm: None, press: Press::new(), redraw: true, outcome_seen: false }
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

    /// **`t32_bat1.256`, and it is not `Battle_LoadAssets`' read.**
    /// `Res_LoadStatic` (`0x00499859`) preloads it into `0x00568EE0` at start-up,
    /// and `Screen_DrawBattlefield` (`0x004233F7`) ends every repaint of `0x28`
    /// … `0x2A` with `Palette_Set(0x568EE0)` — or `Palette_Set(0x5675A0)`,
    /// `t32_stn1.256`, for a siege, which is not ported: see
    /// [`crate::shell::PALETTES`]. `Palette_Set` (`0x004B0AB5`) is a plain
    /// copy — no remap, no shade table — so the battle's colour is this file
    /// and nothing else. **[V]**
    ///
    /// This comment used to say the battle "does not run under the campaign
    /// palette" while the name it returned was loaded by nobody, and so it did.
    fn palette(&self) -> Option<&'static str> {
        Some(l2_view::scene::TILE_PALETTE)
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
            // `Gfx_MarkWidgetUrgent` rather than `Gfx_MarkSpriteDirty` so the
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

/// **`Battle_SelectOutcomeBanner` (`0x00478419`)** — which of group 82's pairs,
/// and so which row of [`crate::movie::BATTLE_FILMS`].
///
/// ```c
/// if (!siege) outcome = (local == winnerOwner) ? 0 : 1;
/// else if (local == winnerOwner) outcome = (g_battleLoser == armyA) ? 2 : 4;
/// else                           outcome = (g_battleLoser == armyA) ? 5 : 3;
/// ```
///
/// `g_battleLoser` holds the **winner** despite its name, and army A is always
/// the besieger, so a siege reads four ways: took the castle (2), held it (4),
/// lost it (5), driven off it (3). **This used to collapse the four to two** —
/// a won siege was always 2 and a lost one always 3 — so a player who held his
/// castle was told he had taken it. Our attacker is `l2_sim::SIDE_B`
/// (`engagement.rs` writes the attacker's survivors from that side), which is
/// the original's army A. `Screen_BattleOutcome` shows the neutral pair 6 when
/// `g_battleChoiceOwner` is 0, before either of these is consulted.
fn outcome_banner(game: &crate::Game, live: &LiveBattle) -> usize {
    if live.choice_owner == 0 {
        return 6;
    }
    let Some(c) = live.conclusion else { return 1 };
    let attacker_won = c.winner == l2_sim::SIDE_B;
    let winner = if attacker_won { live.attacker } else { live.defender };
    let mine = game.kingdom.campaign.units.get(winner).is_some_and(|u| u.owner == game.player);
    match (live.is_siege(), mine, attacker_won) {
        (false, true, _) => 0,
        (false, false, _) => 1,
        (true, true, true) => 2,
        (true, true, false) => 4,
        (true, false, true) => 5,
        (true, false, false) => 3,
    }
}

/// **Ours**, and it looks it: flat cells and a block per man, for an install
/// with no `T32_bat1.pl8`. `docs/decisions.md` C21 — a stub that is visibly ours
/// beats one that looks finished.
///
/// The blocks stand where the artwork's men would —
/// `l2_view::scene::figure_origin`, `BattleMan_Step`'s cell and trail — so the
/// placeholder walks the way the picture does rather than a cell at a time.
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
