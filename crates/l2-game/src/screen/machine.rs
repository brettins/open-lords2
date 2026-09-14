#![allow(unused_imports)]
use super::*;

use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

impl Machine {
    pub fn new(root: ScreenId) -> Machine {
        Machine {
            stack: vec![root.build()],
            quit: false,
            dirty: true,
            clicks: 0,
            pointer: (0, 0),
            pointer_changed: false,
            tooltips: crate::tooltip::Tooltips::new(),
            tooltip_screens: Vec::new(),
            tool_tips_seen: None,
            autosave: false,
        }
    }

    /// **The standing autosave request**, taken and cleared. The application
    /// pumps it through [`crate::saves::run_pending`], which is its only caller
    /// outside a test.
    pub fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    /// The tool-tip layer's state: which tip is up and where. See
    /// [`crate::tooltip`].
    pub fn tooltips(&self) -> &crate::tooltip::Tooltips {
        &self.tooltips
    }

    /// Every widget click the stack has made, ever. See the field.
    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The screen ids on the stack, bottom first. For tests and for the window
    /// title; nothing in the game branches on it.
    pub fn ids(&self) -> Vec<ScreenId> {
        self.stack.iter().map(|s| s.id()).collect()
    }

    pub fn top_id(&self) -> Option<ScreenId> {
        self.stack.last().map(|s| s.id())
    }

    /// **The screen `g_screenId` names** — the top of the stack, looking
    /// through the message scroll, which in the original is painted over a
    /// screen and never changes the byte.
    pub fn top_screen_id(&self) -> Option<ScreenId> {
        self.stack.iter().rev().map(|s| s.id()).find(|id| *id != ScreenId::Message)
    }

    /// **`g_screenId` itself**, for the screens [`crate::tip`] asks about. See
    /// [`Screen::mode_screen_id`] and [`crate::tip::screen_byte`].
    pub fn top_screen_byte(&self, game: &Game) -> Option<u8> {
        let s = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message)?;
        s.mode_screen_id().or_else(|| crate::tip::screen_byte(s.id(), game))
    }

    /// **The pointer this frame** — `Battle_Frame`'s two-way choice
    /// (`0x004B99C0`): the battlefield ids run the hover ladder, every other
    /// screen is one lookup in `g_cursorByScreen` (`0x004E3098`). See
    /// [`crate::cursor`].
    ///
    /// A screen whose byte [`crate::tip::screen_byte`] does not assert takes
    /// the arrow, which is what the table gives every screen but five.
    pub fn pointer(&self, game: &Game) -> crate::cursor::Pointer {
        let byte = self.top_screen_byte(game).unwrap_or(0);
        if (0x28..0x2B).contains(&byte) {
            return match game.battle.as_ref() {
                Some(b) => b.cursor().into(),
                None => crate::cursor::Pointer::Arrow,
            };
        }
        crate::cursor::by_screen(byte)
    }

    pub fn take_dirty(&mut self) -> bool {
        core::mem::replace(&mut self.dirty, false)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Put a screen on the stack from outside.
    ///
    /// This does not weaken the invariant at the top of this file. A *screen*
    /// still cannot reach the stack — it has no `&mut Machine` and never will.
    /// The application owns the machine, and so does a test that wants to open
    /// a screen the interface can only reach through three clicks.
    pub fn push(&mut self, id: ScreenId) {
        self.stack.push(id.build());
        self.dirty = true;
    }

    /// Deliver one event, top screen first, down through anything that passes.
    ///
    /// **The top screen still gets first refusal, and almost always keeps it.**
    /// A screen that does not return [`Transition::Pass`] ends the walk,
    /// popup is modal by default and two screens never act on one click.
    ///
    /// The exception is written down where it is used: `Screen_FrameInput`'s
    /// arm for a screen that is an *inset* can begin with guards belonging to
    /// the surface underneath, and the village's arm begins with six of the
    /// campaign map's. See [`Transition::Pass`].
    ///
    /// # A pass lands at the depth it came from
    ///
    /// The original has no stack: `g_screenId` is one byte, and 57 of the 100
    /// writes to it in `Screen_FrameInput` are the literal `0`.
    /// opened from the sidebar *while the village was up* still exits to the
    /// campaign map, because its arm's exit is a constant and not a memory of
    /// where it was opened from — the village goes with it. A player who tried
    /// it put it exactly: *"when you close that dialogue it will close town
    /// square and that dialogue"*.
    ///
    /// [`Machine::apply_at`] reproduces that by truncating the stack to the
    /// depth that acted before applying the transition. For the top screen —
    /// every other caller — truncating to the top is a no-op, so this is the
    /// same machine it has always been for everything that does not pass.
    /// `docs/bugs.md` B63 catalogues the collapse and what a switch would cost.
    pub fn handle(&mut self, event: Event, ctx: &mut Ctx) {
        // **Ours: Ctrl+D flips the debug overlay, on every screen, and no screen
        // sees the key.** See [`crate::game::Prefs::debug_overlay`].
        //
        // Chosen because the original answers it with nothing at all: the
        // window procedure (`0x004B29BE`) has `WM_KEYDOWN` arms for Backspace,
        // Enter, Control, Escape, End, Home, the arrows, Insert, Delete, the
        // digits, two keypad keys and F1 … F9, F11 and F12 — no letter — and the
        // `WM_CHAR` it sends for Ctrl+D is `0x04`, which `Edit_TypeChar`
        // (`0x00401A20`) rejects. F5 is `main.rs`'s window snap and is not
        // delivered here. Taken before the stack so that a text field cannot
        // swallow it and a screen that reads [`Key::CtrlChar`] — only the
        // battlefield does, and only for the nine digits — never meets it.
        // arm: ours/debug-overlay-toggle key
        if event == Event::KeyDown(crate::input::Key::CtrlChar('D')) {
            ctx.game.prefs.debug_overlay = !ctx.game.prefs.debug_overlay;
            self.dirty = true;
            return;
        }
        // `FUN_004B191E`'s `g_mouseInputChanged`: the position moved, or either
        // button went down or up. A key does not set it.
        match event {
            Event::Pointer { x, y } => {
                if (x, y) != self.pointer {
                    self.pointer = (x, y);
                    self.pointer_changed = true;
                }
            }
            Event::Click { x, y }
            | Event::Release { x, y }
            | Event::DoubleClick { x, y }
            | Event::RightClick { x, y } => {
                self.pointer = (x, y);
                self.pointer_changed = true;
            }
            Event::KeyDown(_) | Event::Text(_) | Event::PointerLeft => {}
        }
        for depth in (0..self.stack.len()).rev() {
            let t = self.stack[depth].handle(event, ctx);
            // **Before the transition, because the transition may drop the
            // screen that clicked.** `Widget_Test` plays the sound inside the
            // hit test and before it calls the handler; taking the count here
            // is that ordering, and it is why a press that opens a screen is
            // still heard. See [`Screen::take_clicks`].
            let clicked = self.stack[depth].take_clicks();
            self.clicks = self.clicks.wrapping_add(clicked as u32);
            // Before the transition for the same reason the clicks are: a new
            // game is started by a click, and the transition that starts it
            // drops the screen that asked. See [`Screen::take_autosave`].
            self.autosave |= self.stack[depth].take_autosave();
            if t == Transition::Pass {
                continue;
            }
            self.apply_at(depth, t);
            break;
        }
        self.dirty = true;
    }

    /// One fixed tick of the top screen — **and, before it, `Msg_Pump`.**
    ///
    /// `Msg_Pump` (`0x00472E46`) is not called by any screen: `Battle_Frame`
    /// (`0x004B99C0`) calls it once a frame, which makes this — our frame
    /// driver's per-tick step — the place it belongs. It is also why the screen
/// test inside it is a test of `g_screenId`
    /// message knows: see [`Machine::pump_messages`].
    pub fn update(&mut self, ctx: &mut Ctx) {
        self.run_tips(ctx);
        self.pump_messages(ctx);
        // `Battle_Frame`'s `FUN_00448d7e(g_selectedCounty)` at `0x004BA187`, in
        // its place: `Tip_Update` (106), `Msg_Pump` (107), … this (190), …
        // `Turn_Tick` (262). **After the pump on purpose** — a letter posted now
// is pulled off the ring on the next frame, as in the original,
        // which is what gives the player one frame of the county they just
        // clicked before the scroll covers it. See
        // [`crate::message::post_event`]. No screen test: the original has none.
        crate::message::post_event(ctx.game);
        self.run_turn_clock(ctx);
        if self.wind_turn(ctx) {
            // The stack moved underneath us, so the screen that was on top no
            // longer is. [`Machine::handle`] breaks for the same reason.
            self.run_tooltips(ctx);
            return;
        }
        if let Some(top) = self.stack.last_mut() {
            let t = top.update(ctx);
            if top.take_redraw() {
                self.dirty = true;
            }
            // **Nothing on this path clicks today**, and it is drained anyway.
            // `Widget_Test`'s auto-repeat and its delayed fire are both silent, so
            // `Screen::update` never counts one — but if a screen ever did, the
            // count would sit in its `Press` until the *next event* drained it in
            // [`Machine::handle`], and a click from a tick would be heard on the
            // release. The ablation that added a click to `Press::tick` stayed
            // green until this line existed.
            self.clicks = self.clicks.wrapping_add(top.take_clicks() as u32);
            // `FUN_0049A3E6`'s `Save_RotateAndWrite()`, which is the bottom of
            // the end-of-turn fade and reaches here through the map screen's
            // `tick_fade`. Before the transition, because a turn that ended the
            // game leaves for screen `0x1C` on the same frame.
            self.autosave |= top.take_autosave();
            if t != Transition::Stay {
                self.apply(t);
                self.dirty = true;
            }
        }
        self.run_tooltips(ctx);
    }

    /// **`Battle_Frame`'s `Turn_Tick(); Units_Tick();`** — the campaign winds
    /// on under whatever is on top of it.
    ///
    /// ```c
    /// if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490d(); Turn_Tick(); Units_Tick(); }
    /// else if ((g_battlePhase == 2) && (ticksDue != 0)) { …the battle's passes… }
    /// ```
    ///
    /// `[V]`, `0x004B99C0`, and **the absence is the finding**:
    /// `g_screenId` test on either arm. Everything in the loop's tail that does
    /// test the screen — `Screen_Draw`, `Screen_FrameInput`, the cursor ladder —
    /// is *drawing and input*. So the only thing on the stack that suspends a
    /// turn is a battle, which is `g_battlePhase != 0` and here is
    /// [`crate::game::Game::battle`] plus the battlefield screen.
    ///
    /// **The defect this replaces**: [`Screen::update`] is run for the top
    /// screen only, and the campaign map is what wound the turn,
    ///
    /// [`crate::screens::message`] — stopped the turn while it was open. So did
    /// walking into a county panel. `docs/decisions.md` C197.
    ///
/// **And the `else if` is the battle's**, so this is one function.
    /// The second arm has no `g_screenId` test either,
/// whatever is on top of *it* as a turn runs under whatever is on
    /// top of the map. That mattered the moment the battlefield got a menu bar:
    /// the drop-down is screen `0x32`, a push here, and
    /// [`Screen::update`] is the top screen's alone — so opening *File* over a
    /// battle froze the battle until the menu closed. See
    /// [`crate::screens::battlefield`].
    ///
    /// Returns whether the stack moved.
    // arm: 0x004B99C0/frame-winds-the-turn frame
    fn wind_turn(&mut self, ctx: &mut Ctx) -> bool {
        // `g_battlePhase != 0`. The battlefield is on the stack for the whole
        // of a fought battle and `Game::battle` for the whole of a suspended
        // one, and neither implies the other.
        if ctx.game.battle.is_some() || self.stack.iter().any(|s| s.id() == ScreenId::Battlefield) {
            // **`else if ((g_battlePhase == 2) && ticksDue) { …the battle's
            // passes… }`** — the loop's other arm, and it is the battlefield's
            // own tick run at whatever depth it sits. Only when something is
            // over it: on top it is [`Machine::update`]'s job and running both
            // would step the simulation twice a frame.
            // arm: 0x004B99C0/frame-winds-the-battle frame
            if let Some(depth) = self.stack.iter().position(|s| s.id() == ScreenId::Battlefield) {
                if depth + 1 < self.stack.len() {
                    self.wind_battle(ctx, depth);
                }
            }
            return false;
        }
        let Some(depth) = self.stack.iter().position(|s| s.id() == ScreenId::Campaign) else {
            return false;
        };
        let t = self.stack[depth].wind_turn(ctx);
        if self.stack[depth].take_redraw() {
            self.dirty = true;
        }
        self.autosave |= self.stack[depth].take_autosave();
        // **A screen the turn is already waiting on is not put up twice.**
        // `resume_turn` asks for `0x12`/`0x13` on every frame the question
        // stands, which before this ran only on the frame the map was on top.
        if let Transition::Push(id) = t {
            if self.stack.iter().any(|s| s.id() == id) {
                return false;
            }
        }
        if t == Transition::Stay {
            return false;
        }
        self.apply_at(depth, t);
        self.dirty = true;
        true
    }

    /// **`Battle_Frame`'s `g_battlePhase == 2` arm** — the battle's passes, run
    /// at `depth` while something else is on top of the battlefield.
    ///
    /// The original's inner loop is one `if / else if` on `g_battlePhase` with
    /// **no `g_screenId` test on either side**,
/// drop-down as a campaign turn steps under an open letter. Ours
    /// stepped only from [`Screen::update`], which the machine gives to the top
    /// screen alone, so the menu bar this screen has just been given would have
    /// frozen the fight every time a player opened it. Same absence, same fix,
    /// one screen along: `docs/decisions.md` C197 is the turn's half of it.
    ///
    /// A transition from down here is applied at `depth`, which is what
    /// [`Machine::apply_at`] is for: a battle that *ends* while a menu is open
/// settles underneath the menu.
    fn wind_battle(&mut self, ctx: &mut Ctx, depth: usize) {
        let t = self.stack[depth].update(ctx);
        if self.stack[depth].take_redraw() {
            self.dirty = true;
        }
        self.clicks = self.clicks.wrapping_add(self.stack[depth].take_clicks() as u32);
        self.autosave |= self.stack[depth].take_autosave();
        // **A screen it is already waiting on is not put up twice** — the same
        // guard `wind_turn` needs, for the same reason: the outcome film is
        // asked for on every frame the banner stands.
        if let Transition::Push(id) = t {
            if self.stack.iter().any(|s| s.id() == id) {
                return;
            }
        }
        if t != Transition::Stay {
            self.apply_at(depth, t);
            self.dirty = true;
        }
    }

    /// **`FUN_00476E95` (`0x00476E95`)** — the tool tips, near the end of
    /// `Battle_Frame` and after everything above. [`crate::tooltip`] has the
    /// decompilation; this is the three things only the machine can see.
    ///
    /// * **A repaint.** `Screen_Draw` opens with `FUN_0047703A`, which drops the
    /// tip and keeps its stamp. A painter runs when the screen changes,
    ///   change in the screens on the stack — looking through the message
    /// scroll — is that call. `[I]`, and the module
    ///   header says what it does not cover.
    /// * **`Opt_ToggleToolTips` (`0x004347C7`)** is three statements, and the
    ///   second is `_DAT_004EA830 = 0`. `g_optToolTips` has no other writer in
    /// play,
    /// * **`Map_InitMode` (`0x00498270`)** writes the same zero, from
    ///   `FUN_00497A34`, the campaign's bring-up: the campaign map arriving on
    ///   the stack.
    ///
    /// And the lookup: `DAT_004D6FB8[g_screenId]` for the top screen `g_screenId`
    /// names, resolved through the campaign map's minimap mode and the selected
    /// county's produce rows.
    fn run_tooltips(&mut self, ctx: &mut Ctx) {
        let changed = core::mem::take(&mut self.pointer_changed);
        let screens: Vec<ScreenId> =
            self.stack.iter().map(|s| s.id()).filter(|id| *id != ScreenId::Message).collect();
        if screens != self.tooltip_screens {
            let arrived = screens.contains(&ScreenId::Campaign)
                && !self.tooltip_screens.contains(&ScreenId::Campaign);
            if arrived {
                self.tooltips.rearm();
            }
            if self.tooltips.drop_tip() {
                self.dirty = true;
            }
            self.tooltip_screens = screens;
        }
        let enabled = ctx.game.prefs.tool_tips;
        if self.tool_tips_seen.is_some_and(|was| was != enabled) {
            self.tooltips.rearm();
        }
        self.tool_tips_seen = Some(enabled);

        let top = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message);
        let byte = top.and_then(|s| {
            crate::tooltip::screen_byte(s.id(), ctx.game, s.mode_screen_id())
        });
        let minimap = self.stack.iter().find_map(|s| s.minimap_mode()).unwrap_or(0);
        let game: &Game = ctx.game;
        let resolve = |x: i32, y: i32| match crate::tooltip::ladder_of(byte) {
            crate::tooltip::ladder::CAMPAIGN => {
                crate::tooltip::campaign_tip(&crate::tooltip::Sidebar::of(game, minimap), x, y)
            }
            crate::tooltip::ladder::BATTLE => crate::tooltip::battle_tip(x, y),
            _ => 0,
        };
        if self.tooltips.frame(enabled, changed, self.pointer, resolve) {
            self.dirty = true;
        }
    }

    /// **`Msg_Pump`'s screen ladder** — which screens the message scroll runs
    /// on, and what happens on the rest.
    ///
    /// ```c
    /// if (g_screenId == 0x00 || g_screenId == 0x27 ||
    ///     (g_screenId == 0x0F && g_jobPanelJob == 8) || g_screenId == 0x29) { … pump … }
    /// else if (g_messageGroup != 0) Msg_Dismiss();
    /// ```
    ///
    /// Two arms, and the second is the one nobody had written down: **opening
    /// any other screen while a message is up closes it.** Walk into the village
    /// with a letter on screen and the letter is gone.
    ///
    /// `0x27` has no screen here and `g_jobPanelJob` is the job slot **plus
    /// one**, which is what `CountyStrip_JobClick` writes — so the panel that
    /// pumps is slot 7.
    ///
    /// # `Msg_Pump` is one function and its two halves are exclusive
    ///
    /// ```c
    /// if (g_messageTimer < 1) { …pull one record, timer = 2000… }
    /// else                    { …count down, and maybe dismiss… }
    /// if (g_messageGroup != 0) Msg_DrawWindow();
    /// ```
    ///
    /// So the frame that opens a window **does draw it** — that trailing call is
    /// not in either arm — and does **not** count its timer down. Both halves
    /// are here, and the draw's own side effects are the message screen's
    /// `update`, which the caller runs immediately after this. Splitting the
    /// countdown out into the screen instead cost the message one tick of life:
    /// invisible in single player, where the timer is clamped and never expires,
    /// and a measurable 399 against 400 in a network game.
    // arm: 0x00472E46/pump-screen-ladder frame
    fn pump_messages(&mut self, ctx: &mut Ctx) {
        // **`Msg_Pump`'s first test, inside the ladder** — a battle swallows
        // messages:
        //
        // ```c
        // if (g_battlePhase == 2 && g_messageGroup != 0) Msg_Dismiss();
        // else { …pull, count down, draw… }
        // ```
        //
        // It is the *open* window it closes, not the pull: with the queue
        // non-empty and nothing up, the else arm still runs and the next record
        // is drawn for exactly one frame before this test dismisses it. So a
        // battle drains the ring, and a player fighting
        // one sees each letter flash. `g_battlePhase == 2` is the battlefield
        // anywhere in our stack, as `crate::audio::scene` reads it — the
        // original's 0x29, 0x2A and 0x2B are one screen of ours and a panel can
        // sit over them.
        // arm: 0x00472E46/battle-phase-swallows-messages frame
        let fighting = self.ids().iter().any(|id| matches!(id, ScreenId::Battlefield));
        if fighting && ctx.game.messages.is_open() {
            crate::message::dismiss(ctx.game);
            self.dirty = true;
            return;
        }
        if self.top_id() == Some(ScreenId::Message) {
            // The countdown half. When it expires the window closes and the
            // screen's own `update` pops itself on its first line.
            if ctx.game.messages.advance(ctx.game.multiplayer) == crate::message::Tick::TimedOut {
                // `Msg_Pump`'s two timeouts call `Msg_Dismiss`, and so reach
                // `FUN_00476E21`: a network game's tip that expires restores
// its screen as a clicked one does.
                ctx.game.tips.restore();
                self.dirty = true;
            }
            return;
        }
        let pumps = match self.top_id() {
            Some(ScreenId::Campaign) => true,
            Some(ScreenId::Battlefield) => true,
            // `g_screenId == 0x27`, the tip's own screen — the reason a tip
            // shown in the village gets a window at all.
            Some(ScreenId::Tip) => true,
            Some(ScreenId::Job(_, job)) => job + 1 == crate::message::PUMP_JOB,
            _ => false,
        };
        if pumps {
            // The pull half.
            if ctx.game.messages.pull() {
                self.push(ScreenId::Message);
            }
        } else if ctx.game.messages.is_open() {
            crate::message::dismiss(ctx.game);
            self.dirty = true;
        }
    }

    /// **`Turn_Tick`'s turn timer, and the frames after it runs out.**
    ///
    /// `Turn_Tick` (`0x0049A010`) is called from `Battle_Frame`'s loop whenever
    /// `g_battlePhase == 0`, not from any screen, so the count goes on whether
    /// the person is looking at the map, a county panel or the village. That
    /// makes the frame driver its place, for the same reason it is `Msg_Pump`'s.
    /// The clock itself is [`crate::turn_clock`].
    ///
    /// When it runs out it calls `Turn_End` (`0x0043AC23`), and three things
    /// follow:
    ///
    /// 1. `Turn_End`'s first statement, `if (g_messageGroup != 0) Msg_Dismiss();`;
    /// 2. from the next frame, `Screen_FrameInput` closes every screen whose arm
    ///    carries the turn-ended guard — [`crate::turn_clock::closed_by_turn_end`],
    ///    popped here top first for as long as the request stands;
    /// 3. the turn begins. **Only the map can start one here**, so the request
    ///    waits on [`crate::game::Game::turn_clock`] until the map is on top, and
    ///    `MapScreen::update` carries it out through the End Turn button's own
    ///    door.
    ///
    /// **One difference left, and the other one is gone.** The original runs
    /// the turn behind a screen the guard does not close — the job popup, an
    /// open menu, the About box — and so do we now: that used to wait for the
    /// person to close it, because the map wound the turn and only the top
    /// screen was ticked. [`Machine::wind_turn`] is where that stopped being
    /// true. What remains is `0x13`, the battle report, which is closed by the
    /// guard there and not here: popping it would leave the report unseen on
    /// the suspended turn and the map would put it straight back.
    // arm: 0x0049A010/turn-time-limit timer
    fn run_turn_clock(&mut self, ctx: &mut Ctx) {
        // `2 < g_appPhase` — a game is up, which here is a campaign map on the
        // stack. The front end and the demo index have no turn to time.
        if !self.stack.iter().any(|s| s.id() == ScreenId::Campaign) {
            return;
        }
        let before = crate::turn_clock::shown(ctx.game);
        let frame = crate::turn_clock::Frame::of(ctx.game);
        if ctx.game.turn_clock.tick(frame) == crate::turn_clock::Tick::Expired
            && ctx.game.messages.is_open()
        {
            // The message screen pops itself on its own update once its record
            // is gone, so it is not popped here.
            crate::message::dismiss(ctx.game);
            self.dirty = true;
        }
        // **`Screen_FrameInput`'s force-close guard**, and it is the standing
// latch:
        //
        //   if (DAT_00553FC8 != 0 || (DAT_0055403C != 0 && DAT_00553018 == 0))
        //
        // `Turn_End` (`0x0043AC23`) writes `DAT_0055403C = 2` — whichever door
        // the turn was ended through, the clock's or the person's own End Turn
        // — and `Turn_Tick`'s restart clears it on the first frame of his next
        // live turn. So the guard stands for the whole turn in between, and
        // twenty-six of the twenty-seven sites close their screen. This tested
        // `end_turn_pending` instead, which is the clock's request and is taken
        // by the map the moment the map is on top: a panel opened *during* the
        // turn that followed stayed open, and ending the turn by hand with one
        // up closed nothing at all.
        // arm: 0x0042FF10/force-close-on-turn-end timer
        if ctx.game.turn_clock.force_close() {
            let in_battle = ctx.game.battle.is_some();
            while let Some(top) = self.stack.last() {
                let id = top.id();
                if id == ScreenId::BattleResult
                    || !crate::turn_clock::closed_by_turn_end(id, in_battle)
                {
                    break;
                }
                self.stack.pop();
                self.dirty = true;
            }
        }
        // The number is whole seconds, so this is a repaint a second and not a
        // repaint a tick.
        if crate::turn_clock::shown(ctx.game) != before {
            self.dirty = true;
        }
    }

    /// **`Tip_Update` (`0x00476AA7`), and `g_screenId = 0x27` made a stack.**
    ///
    /// `Battle_Frame` calls `Tip_Update` immediately before `Msg_Pump`, which is
    /// the order here. The ladder is [`crate::tip::update`]; what this adds is
    /// the one thing the ladder cannot do, which is put screen `0x27` on the
    /// stack and take it off again:
    ///
    /// * **on** when `Tip_Show` posts — *under* the message scroll if one is up,
    ///   because a message open on the campaign map is painted over `g_screenId
    ///   0` and stays painted over `0x27` when the byte changes beneath it;
    /// * **off** when `FUN_00476E21` has restored the byte, which any
    ///   `Msg_Dismiss` does — including the dismissal of a message that was
    ///   already queued ahead of the tip, so the tip's own record can outlive
    ///   its screen and open later on the campaign map. That is the original.
    ///
    /// Seating is done at the *start* of the tick as well as after the ladder,
    /// because a dismissal happens in [`Machine::handle`] between two ticks.
    // arm: 0x00476AA7/tip-screen-ladder frame
    fn run_tips(&mut self, ctx: &mut Ctx) {
        self.seat_tip_host(ctx.game);
        let view = crate::tip::View::of(self, ctx.game);
        if crate::tip::tick(ctx.game, &view).is_some() {
            self.seat_tip_host(ctx.game);
        }
    }

    /// Keep [`ScreenId::Tip`] on the stack exactly while
    /// [`crate::tip::Tips::hosting`] says `g_screenId` is `0x27`.
    // arm: 0x00476E21/tip-restores-its-screen frame
    fn seat_tip_host(&mut self, game: &Game) {
        let seated = self.stack.iter().any(|s| s.id() == ScreenId::Tip);
        if game.tips.hosting() && !seated {
            let at = match self.top_id() {
                Some(ScreenId::Message) => self.stack.len() - 1,
                _ => self.stack.len(),
            };
            self.stack.insert(at, ScreenId::Tip.build());
            self.dirty = true;
        } else if !game.tips.hosting() && seated {
            self.stack.retain(|s| s.id() != ScreenId::Tip);
            self.dirty = true;
        }
    }

    /// **`FUN_0041A639` (`0x0041A639`) — the turn timer**, drawn over whatever is
    /// up, after it.
    ///
    /// Not a painter's draw: `Battle_Frame` calls it once a frame near the end of
/// its tail, after the widgets and the message window,
    /// drawn here after the stack and not by the campaign map. Whether it is
    /// drawn at all is `DAT_004D2E80[g_screenId]`, the table in
    /// [`crate::turn_clock::SCREENS`].
    fn draw_turn_timer(&self, ctx: &Ctx, canvas: &mut Canvas) {
        if !self.stack.iter().any(|s| s.id() == ScreenId::Campaign) {
            return;
        }
        let Some(id) = crate::turn_clock::timer_screen(self.stack.iter().map(|s| s.id())) else {
            return;
        };
        if crate::turn_clock::drawn_over(id) {
            crate::turn_clock::draw(ctx, canvas);
        }
    }

    /// Paint the stack from the last screen that is not an overlay upwards.
    ///
    /// An overlay is drawn over what was underneath, which is what the
/// original's management surface is; a page clears and replaces.
    /// The common case — a stack whose top is a page — draws exactly one
    /// screen, as it always did.
    ///
/// **The original has no screen clear anywhere**:
    /// `Screen_Draw` picks a painter and the painter fills a rectangle,
    /// whatever is outside it is still there from the last frame.
    /// `Village_Draw` repaints the campaign map itself and blits its picture on
    /// top of it; `Panel_JobDetail` draws a window over the village.
    pub fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let from = self.base();
        for screen in &mut self.stack[from..] {
            screen.draw(ctx, canvas);
        }
        self.draw_turn_timer(ctx, canvas);
        // `FUN_00476E95` comes after `FUN_0041A639` in `Battle_Frame`.
        crate::tooltip::draw(ctx, &self.tooltips, canvas);
    }

    /// The lowest screen that has to be painted for the top one to make sense.
    fn base(&self) -> usize {
        for i in (0..self.stack.len()).rev() {
            if !self.stack[i].is_overlay() {
                return i;
            }
        }
        0
    }

    /// The `.256` the stack runs under, or `None` for the campaign palette. The
    /// presenter is the only caller: it is the one place that turns indices
    /// into colour.
    ///
    /// **The nearest screen that names one, looking down through overlays.**
    /// The original has one display palette and only a *painter* writes it —
    /// `Screen_Armoury` ends with `Palette_Set(armoury.256)`,
    /// `Screen_DrawBattlefield` (`0x004233F7`) sets `T32_bat1.256` — and nothing
    /// that draws over a page touches it.
    /// `Tip_Show` (`0x00476DA9`) saves `g_screenId`, writes `0x27` and posts a
    /// message; `FUN_00476E21` puts the byte back; `Msg_DrawWindow`
    /// (`0x0047309E`) has no `Palette_Set` anywhere in its 10,915 bytes.
    /// window over the armoury is in the armoury's colours. `[V]`
    ///
    /// This used to ask the top screen alone, and an overlay that names no
    /// palette — the tip host, the message scroll, the menu bar, the options
    /// pages — handed the page beneath it the campaign palette. A player saw the
    /// raise-army and castle screens *"color reversed"* behind their first tip
    /// until he dismissed it. `docs/decisions.md` C178.
    ///
    /// A page that names none *is* the campaign palette and ends the search,
    /// which is the same boundary [`Machine::draw`] stops at for pixels.
    pub fn palette_name(&self) -> Option<&'static str> {
        for screen in self.stack.iter().rev() {
            if let Some(name) = screen.palette() {
                return Some(name);
            }
            if !screen.is_overlay() {
                return None;
            }
        }
        None
    }

    /// **The frame as colour** — `canvas` through a playing film's palette or
    /// else [`Machine::palette_name`]'s, and through the end-of-turn fade when
    /// the top screen is fading.
    ///
    /// `main.rs`'s presenter is this and a window. It lives here so that the
    /// colours a player is shown can be asserted without one: a canvas is a
    /// plane of indices, and every defect of the *"right picture, wrong
    /// colours"* kind is invisible to a test that stops at the canvas.
    pub fn present(&self, assets: &crate::game::Assets, canvas: &Canvas, rgba: &mut [u8]) {
// A film's palette changes as it plays; while one is
        // up it is the whole screen's (`Smk_ApplyPalette`), so it outranks
        // every `.256` on the stack. See [`Machine::live_palette`].
        let live = self.live_palette();
        let palette = live.as_ref().unwrap_or_else(|| {
            self.palette_name()
                .and_then(|n| assets.shell.palette(n))
                .unwrap_or(&assets.palette)
        });
        // **The end-of-turn fade, and it is the whole of the effect.**
        // `FUN_004B0CB4` never touches the framebuffer — it rewrites the display
        // palette and lets the unchanged plane of indices resolve darker. See
        // `l2_view::fade` and [`Screen::fade`].
        let faded = self.fade().map(|phase| l2_view::fade::at(palette, phase));
        canvas.to_rgba(faded.as_ref().unwrap_or(palette), rgba);
    }

    /// The top screen's [`Screen::live_palette`], which outranks
    /// [`Machine::palette_name`] when it answers.
    pub fn live_palette(&self) -> Option<l2_formats::Palette> {
        self.stack.last().and_then(|s| s.live_palette())
    }

    /// The end-of-turn fade phase of the top screen, or `None`. The presenter
    /// is the only caller; see [`Screen::fade`].
    pub fn fade(&self) -> Option<u8> {
        self.stack.last().and_then(|s| s.fade())
    }

    pub fn title(&self, ctx: &Ctx) -> String {
        self.stack.last().map(|s| s.title(ctx)).unwrap_or_default()
    }

    fn apply(&mut self, t: Transition) {
        let depth = self.stack.len().saturating_sub(1);
        self.apply_at(depth, t);
    }

    /// Apply a transition **asked for by the screen at `depth`**.
    ///
    /// Everything above `depth` is discarded first. For the top screen that is
/// nothing, so every existing caller is unaffected; for a screen
    /// that was reached by a [`Transition::Pass`] it is the whole point, and it
/// is the original's behaviour — see
    /// [`Machine::handle`].
    fn apply_at(&mut self, depth: usize, t: Transition) {
        match t {
            Transition::Stay | Transition::Pass => {}
            // `g_screenId = 0` from an arm that was reached by falling through:
            // this screen stays and everything opened over it goes.
            Transition::Reveal => self.stack.truncate(depth + 1),
            Transition::Push(id) => {
                self.stack.truncate(depth + 1);
                self.stack.push(id.build());
            }
            Transition::Pop => {
                self.stack.truncate(depth);
                if self.stack.is_empty() {
                    self.quit = true;
                }
            }
            Transition::Replace(id) => {
                self.stack.truncate(depth);
                self.stack.push(id.build());
            }
            Transition::Quit => {
                self.stack.clear();
                self.quit = true;
            }
            // `g_screenId = g_smkReturnScreen`. `depth` is deliberately not
            // consulted: the original writes the byte whatever was up, and the
            // destination may be *below* the screen that asked — which is the
            // whole case this exists for, a castle film ending on the map with
            // the chooser that raised it in between.
            Transition::Goto(id) => match self.stack.iter().position(|s| s.id() == id) {
                Some(at) => self.stack.truncate(at + 1),
                None => {
                    self.stack.clear();
                    self.stack.push(id.build());
                }
            },
        }
    }
}

