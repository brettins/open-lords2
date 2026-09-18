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
            tip_seat: None,
        }
    }

    pub fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    pub fn tooltips(&self) -> &crate::tooltip::Tooltips {
        &self.tooltips
    }

    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn ids(&self) -> Vec<ScreenId> {
        self.stack.iter().map(|s| s.id()).collect()
    }

    pub fn top_id(&self) -> Option<ScreenId> {
        self.stack.last().map(|s| s.id())
    }

    pub fn top_screen_id(&self) -> Option<ScreenId> {
        self.stack.iter().rev().map(|s| s.id()).find(|id| *id != ScreenId::Message)
    }

    pub fn top_screen_byte(&self, game: &Game) -> Option<u8> {
        let s = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message)?;
        s.mode_screen_id().or_else(|| crate::tip::screen_byte(s.id(), game))
    }

    /// **The pointer this frame** — `Battle_Frame`'s two-way choice
    /// (`0x004B99C0`): the battlefield ids run the hover ladder, every other
    /// screen is one lookup in `g_cursorByScreen` (`0x004E3098`). See
    /// [`crate::cursor`].
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

    pub fn push(&mut self, id: ScreenId) {
        self.stack.push(id.build());
        self.dirty = true;
    }

    pub fn handle(&mut self, event: Event, ctx: &mut Ctx) {
        // Chosen because the original answers it with nothing at all: the
        // window procedure (`0x004B29BE`) has `WM_KEYDOWN` arms for Backspace,
        // Enter, Control, Escape, End, Home, the arrows, Insert, Delete, the
        // digits, two keypad keys and F1 … F9, F11 and F12 — no letter — and the
        // `WM_CHAR` it sends for Ctrl+D is `0x04`, which `Edit_TypeChar`
        // (`0x00401A20`) rejects. F5 is `main.rs`'s window snap and is not
        // delivered here. Taken before the stack so that a text field cannot
        // swallow it and a screen that reads [`Key::CtrlChar`] — only the
        // battlefield does, and only for the nine digits — never meets it.
        //
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
            | Event::RightClick { x, y }
            | Event::RightPress { x, y } => {
                self.pointer = (x, y);
                self.pointer_changed = true;
            }
            Event::KeyDown(_) | Event::Text(_) | Event::PointerLeft => {}
        }
        for depth in (0..self.stack.len()).rev() {
            let t = self.stack[depth].handle(event, ctx);
            let clicked = self.stack[depth].take_clicks();
            self.clicks = self.clicks.wrapping_add(clicked as u32);
            self.autosave |= self.stack[depth].take_autosave();
            if t == Transition::Pass {
                continue;
            }
            self.apply_at(depth, t);
            break;
        }
        self.dirty = true;
    }

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
            self.run_tooltips(ctx);
            return;
        }
        if let Some(top) = self.stack.last_mut() {
            let t = top.update(ctx);
            if top.take_redraw() {
                self.dirty = true;
            }
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

    /// ```c
    /// if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490d(); Turn_Tick(); Units_Tick(); }
    /// else if ((g_battlePhase == 2) && (ticksDue != 0)) { …the battle's passes… }
    /// ```
    ///
    /// `[V]`, `0x004B99C0`, and **the absence is the finding**:
    ///
    /// [`crate::screens::message`] — stopped the turn while it was open. So did
    /// walking into a county panel. `docs/decisions.md` C197.
    ///
    // arm: 0x004B99C0/frame-winds-the-turn frame
    fn wind_turn(&mut self, ctx: &mut Ctx) -> bool {
        if ctx.game.battle.is_some() || self.stack.iter().any(|s| s.id() == ScreenId::Battlefield) {
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

    /// The original's inner loop is one `if / else if` on `g_battlePhase` with
    /// **no `g_screenId` test on either side**,
/// drop-down as a campaign turn steps under an open letter. Ours
    /// stepped only from [`Screen::update`], which the machine gives to the top
    /// screen alone, so the menu bar this screen has just been given would have
    /// frozen the fight every time a player opened it. Same absence, same fix,
    /// one screen along: `docs/decisions.md` C197 is the turn's half of it.
    fn wind_battle(&mut self, ctx: &mut Ctx, depth: usize) {
        let t = self.stack[depth].update(ctx);
        if self.stack[depth].take_redraw() {
            self.dirty = true;
        }
        self.clicks = self.clicks.wrapping_add(self.stack[depth].take_clicks() as u32);
        self.autosave |= self.stack[depth].take_autosave();
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
    ///
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

    // arm: 0x00472E46/pump-screen-ladder frame
    fn pump_messages(&mut self, ctx: &mut Ctx) {
        // arm: 0x00472E46/battle-phase-swallows-messages frame
        let fighting = self.ids().iter().any(|id| matches!(id, ScreenId::Battlefield));
        if fighting && ctx.game.messages.is_open() {
            crate::message::dismiss(ctx.game);
            self.dirty = true;
            return;
        }
        if self.top_id() == Some(ScreenId::Message) {
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
            Some(ScreenId::Tip) => true,
            Some(ScreenId::Job(_, job)) => job + 1 == crate::message::PUMP_JOB,
            _ => false,
        };
        if pumps {
            if ctx.game.messages.pull() {
                self.push(ScreenId::Message);
            }
        } else if ctx.game.messages.is_open() {
            crate::message::dismiss(ctx.game);
            self.dirty = true;
        }
    }

    /// `Turn_Tick` (`0x0049A010`) is called from `Battle_Frame`'s loop whenever
    /// `g_battlePhase == 0`, not from any screen, so the count goes on whether
    /// the person is looking at the map, a county panel or the village. That
    /// makes the frame driver its place, for the same reason it is `Msg_Pump`'s.
    ///
    /// When it runs out it calls `Turn_End` (`0x0043AC23`), and three things
    /// follow:
    ///
    // arm: 0x0049A010/turn-time-limit timer
    fn run_turn_clock(&mut self, ctx: &mut Ctx) {
        if !self.stack.iter().any(|s| s.id() == ScreenId::Campaign) {
            return;
        }
        let before = crate::turn_clock::shown(ctx.game);
        let frame = crate::turn_clock::Frame::of(ctx.game);
        if ctx.game.turn_clock.tick(frame) == crate::turn_clock::Tick::Expired
            && ctx.game.messages.is_open()
        {
            crate::message::dismiss(ctx.game);
            self.dirty = true;
        }
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
        //
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
        if crate::turn_clock::shown(ctx.game) != before {
            self.dirty = true;
        }
    }

    /// **`Tip_Update` (`0x00476AA7`), and `g_screenId = 0x27` made a stack.**
    ///
    /// * **on** when `Tip_Show` posts — *under* the message scroll if one is up,
    ///   because a message open on the campaign map is painted over `g_screenId
    ///   0` and stays painted over `0x27` when the byte changes beneath it;
    /// * **off** when `FUN_00476E21` has restored the byte, which any
    ///   `Msg_Dismiss` does — including the dismissal of a message that was
    ///   already queued ahead of the tip, so the tip's own record can outlive
    ///   its screen and open later on the campaign map. That is the original.
    ///
    // arm: 0x00476AA7/tip-screen-ladder frame
    fn run_tips(&mut self, ctx: &mut Ctx) {
        self.seat_tip_host(ctx.game);
        let view = crate::tip::View::of(self, ctx.game);
        if crate::tip::tick(ctx.game, &view).is_some() {
            self.seat_tip_host(ctx.game);
        }
    }

    // arm: 0x00476E21/tip-restores-its-screen frame
    /// **The host is seated over the screen whose byte `Tip_Show` overwrote**,
    /// and not over the top of the stack.
    ///
    /// `Tip_Show` (`0x00476DA9`) writes `_DAT_004F0350 = g_screenId; g_screenId
    /// = 0x27` — **one** byte, the one that was current when the ladder posted.
    ///
    /// A screen opened afterwards has its own byte, and its own arm in
    /// `Screen_HandleInput` still runs: the save box's `g_screenId == '5' ||
    /// '6'` arm, which is the only caller of `SaveLoad_Tick` (`0x004AD9F0`), is
    /// how `DAT_0057D3C4` counts down. Seating the host at the top instead —
    /// which this did — put it over the box, whose `update` then never ran and
    /// whose countdown never reached zero: *"Saving game. Please wait."* for
    /// ever. [`Machine::tip_seat`] is that screen, and it lives exactly as long
    /// as the host it was written for: `Tip_Show` rewrites `_DAT_004F0350` on
    /// every post and `FUN_00476E21` (`0x00476E21`) hands it back once. Nothing
    /// but the unseat below takes the host off the stack — `force_close` stops
    /// at the first screen [`crate::turn_clock::closed_by_turn_end`] rejects and
    /// `0x27` is not in its table, so the re-seat this doc once claimed was for
    /// a popper that does not exist.
    fn seat_tip_host(&mut self, game: &Game) {
        let seated = self.stack.iter().any(|s| s.id() == ScreenId::Tip);
        if game.tips.hosting() && !seated {
            let at = match self.tip_seat {
                Some(id) => match self.stack.iter().rposition(|s| s.id() == id) {
                    Some(i) => i + 1,
                    None => {
                        eprintln!("tip host: seat {id:?} has closed; seating over the screen up now");
                        self.post_frame_seat()
                    }
                },
                None => self.post_frame_seat(),
            };
            self.tip_seat = at.checked_sub(1).map(|i| self.stack[i].id());
            self.stack.insert(at, ScreenId::Tip.build());
            self.dirty = true;
        } else if !game.tips.hosting() && seated {
            self.stack.retain(|s| s.id() != ScreenId::Tip);
            // `FUN_00476E21` leaves `_DAT_004F0350` standing, and it may:
            //
            // `Tip_Show` (`0x00476DA9`) writes the byte again on **every** post,
            // so the remembered screen can only ever apply to the host it was
            // written for. Carrying it to the next post is not the original and
            // is worse than dropping it — it seated the raise-army screen's own
            // tip *under* the armoury.
            self.tip_seat = None;
            self.dirty = true;
        }
    }

    /// Where `Tip_Show` (`0x00476DA9`) reads `g_screenId`: the top screen is the
    /// one whose byte the ladder read, under a scroll that is not a byte at all.
    fn post_frame_seat(&self) -> usize {
        match self.top_id() {
            Some(ScreenId::Message) => self.stack.len() - 1,
            _ => self.stack.len(),
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

    pub fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let from = self.base();
        for screen in &mut self.stack[from..] {
            screen.draw(ctx, canvas);
        }
        self.draw_turn_timer(ctx, canvas);
        // `FUN_00476E95` comes after `FUN_0041A639` in `Battle_Frame`.
        crate::tooltip::draw(ctx, &self.tooltips, canvas);
    }

    fn base(&self) -> usize {
        for i in (0..self.stack.len()).rev() {
            if !self.stack[i].is_overlay() {
                return i;
            }
        }
        0
    }

    /// The original has one display palette and only a *painter* writes it —
    /// `Screen_Armoury` ends with `Palette_Set(armoury.256)`,
    /// `Screen_DrawBattlefield` (`0x004233F7`) sets `T32_bat1.256` — and nothing
    /// that draws over a page touches it.
    ///
    /// `Tip_Show` (`0x00476DA9`) saves `g_screenId`, writes `0x27` and posts a
    /// message; `FUN_00476E21` puts the byte back; `Msg_DrawWindow`
    /// (`0x0047309E`) has no `Palette_Set` anywhere in its 10,915 bytes.
    ///
    /// window over the armoury is in the armoury's colours. `[V]`
    ///
    /// This used to ask the top screen alone, and an overlay that names no
    /// palette — the tip host, the message scroll, the menu bar, the options
    /// pages — handed the page beneath it the campaign palette. A player saw the
    /// raise-army and castle screens *"color reversed"* behind their first tip
    /// until he dismissed it. `docs/decisions.md` C178.
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

    pub fn present(&self, assets: &crate::game::Assets, canvas: &Canvas, rgba: &mut [u8]) {
        let live = self.live_palette();
        let palette = live.as_ref().unwrap_or_else(|| {
            self.palette_name()
                .and_then(|n| assets.shell.palette(n))
                .unwrap_or(&assets.palette)
        });
        // `FUN_004B0CB4` never touches the framebuffer — it rewrites the display
        // palette and lets the unchanged plane of indices resolve darker. See
        // `l2_view::fade` and [`Screen::fade`].
        let faded = self.fade().map(|phase| l2_view::fade::at(palette, phase));
        canvas.to_rgba(faded.as_ref().unwrap_or(palette), rgba);
    }

    pub fn live_palette(&self) -> Option<l2_formats::Palette> {
        self.stack.last().and_then(|s| s.live_palette())
    }

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

    fn apply_at(&mut self, depth: usize, t: Transition) {
        match t {
            Transition::Stay | Transition::Pass => {}
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

