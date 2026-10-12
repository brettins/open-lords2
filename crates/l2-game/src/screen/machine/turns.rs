#![allow(unused_imports)]
use super::*;
use super::state::*;
use super::frame::*;
use super::drawing::*;
use super::super::*;
use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

impl Machine {
    /// `Msg_Pump` (`0x00472E46`) is not called by any screen: `Battle_Frame`
    /// (`0x004B99C0`) calls it once a frame, which makes this — our frame
    /// driver's per-tick step — the place it belongs. It is also why the screen
/// test inside it is a test of `g_screenId`
    /// message knows: see [`Machine::pump_messages`].
    pub fn update(&mut self, ctx: &mut Ctx) {
        self.update_screens(ctx);
        self.keep_halt_on_top(ctx.game);
    }

    /// `[D]` a stopped network game's box opens over everything and stays there:
    /// a battle prompt opened the same frame buried it (2026-10-11, `halt_box`).
    /// C260: the waiting box is held the same way, and goes when nobody is awaited (C262: dropped or silent).
    pub(crate) fn keep_halt_on_top(&mut self, game: &Game) {
        let net = &game.net;
        let halt = net.halted.is_some() && net.after_halt.is_none();
        let awaited = !net.waiting.is_empty() || !net.quiet.is_empty();
        let wait = !halt && net.lockstep && awaited && game.leaving.is_none();
        self.keep_on_top(ScreenId::NetWait, wait);
        self.keep_on_top(ScreenId::NetHalt, halt);
    }

    fn keep_on_top(&mut self, id: ScreenId, wanted: bool) {
        match self.stack.iter().position(|s| s.id() == id) {
            None if wanted => self.push(id),
            Some(i) if !wanted => {
                self.stack.remove(i);
                self.dirty = true;
            }
            Some(i) if i + 1 < self.stack.len() => {
                let screen = self.stack.remove(i);
                self.stack.push(screen);
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn update_screens(&mut self, ctx: &mut Ctx) {
        self.trace_screen(ctx.game);
        // Battle_Frame 004b0000.c:6707 ticksDue, the ticks (6940), then on loop exit
        // Tip_Update/Msg_Pump (6783-6784), Event_Post (6867), Screen_FrameInput (6871) [V].
        // FUN_00476E21 0x00476E21 restored g_screenId at the dismissal, before this gate [V].
        self.seat_tip_host(ctx.game);
        let withheld = self.ticks_withheld(ctx.game);
        // Turn_Tick 0x0049A010 00490000.c:4787 counts DAT_005440C8 down inside the tick [V].
        // A network game's clock is `net::frame`'s, on the session's tick.
        if !ctx.game.net.lockstep {
            self.run_turn_clock(ctx);
        } else {
            // Screen_FrameInput 0x0042FF10 force_close, per seat off the shared clock [I]
            self.dirty |= crate::turn_clock::seat_saw_expiry(ctx.game, &mut self.clock_seen);
            self.dirty |= tick_gate::close_on_turn_end(&mut self.stack, ctx.game);
        }
        let wound = self.join_battle(ctx) || self.wind_turn(ctx, withheld);
        self.run_tips(ctx);
        self.pump_messages(ctx);
        // Event_Post(g_selectedCounty) 004b0000.c:6867 after Msg_Pump: its letter leaves the ring next frame [V].
        crate::message::post_event(ctx.game);
        if wound {
            self.run_tooltips(ctx);
            return;
        }
        // Battle_Frame 0x004B99C0 004b0000.c:6718-6721: the map under a letter still redraws [V].
        if let Some(i) = self.stack.iter().rposition(|s| s.id() != ScreenId::Message) {
            if i + 1 < self.stack.len() {
                self.stack[i].under_letter(ctx);
            }
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
        self.trace_screen(ctx.game);
        self.run_tooltips(ctx);
    }

    /// A write to `g_screenId` (`0x004EAC50`), as the detour on it sees it. C246.
    pub(super) fn trace_screen(&self, game: &crate::Game) {
        let _ = game;
        l2_net::trace_changed!(self.traced_screen_byte(game).map_or(-1, i64::from); "screen", 0x004EAC50, "g_screenId", id = self.traced_screen_byte(game).map_or(-1, i64::from), ours = format!("{:?}", self.top_id()));
        // Sampled once a frame, as native/l2-trace watches 0x00569584 at each present (caller null).
        let phase = game.kingdom.turn.phase as u8;
        l2_net::trace_changed!(i64::from(phase); "phase", 0x00569584, "g_turnPhase", phase = phase);
    }

    /// ```c
    /// if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490d(); Turn_Tick(); Units_Tick(); }
    /// else if ((g_battlePhase == 2) && (ticksDue != 0)) { …the battle's passes… }
    /// ```
    ///
    /// `[V]` `0x004B99C0`; `ticksDue` is FUN_004BB978 (0x004BB978), which is 0 under
    /// `DAT_004E31B0[g_screenId] != 0` or an open letter: [`tick_gate`]. C197 missed it.
    ///
    // arm: 0x004B99C0/frame-winds-the-turn frame
    /// A network game: a battle this seat's realm is in comes to it, whoever
    /// took the field. `docs/netcode.md` §4.
    fn join_battle(&mut self, ctx: &mut Ctx) -> bool {
        let me = ctx.game.player;
        let mine = crate::command::battle_sides(ctx.game).is_some_and(|(a, d)| a == me || d == me);
        if !ctx.game.net.lockstep || !mine || self.stack.iter().any(|s| s.id() == ScreenId::Battlefield) {
            return false;
        }
        let Some(depth) = self.stack.iter().position(|s| s.id() == ScreenId::Campaign) else {
            return false;
        };
        self.apply_at(depth, Transition::Push(ScreenId::Battlefield));
        self.dirty = true;
        true
    }

    /// FUN_004BB978 0x004BB978: no ticks under DAT_004E31B0[g_screenId] or a letter.
    fn ticks_withheld(&self, game: &crate::Game) -> bool {
        let byte = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message).and_then(|s| {
            s.mode_screen_id().or_else(|| crate::turn_clock::screen_byte(s.id()))
        });
        tick_gate::withheld(byte, game.messages.is_open(), game.multiplayer)
    }

    fn wind_turn(&mut self, ctx: &mut Ctx, withheld: bool) -> bool {
        // Battle_Frame 004b0000.c:6711 Season_FinishFade() sits after the tick loop: the fade
        // finishes whatever FUN_004BB978 returns.
        let fading = self.stack.iter().any(|s| s.id() == ScreenId::Campaign && s.fade().is_some());
        if withheld && !fading {
            return false;
        }
        if !withheld && (ctx.game.battle.is_some() || self.stack.iter().any(|s| s.id() == ScreenId::Battlefield)) {
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
        // arm: 0x0043AD25/film-skip-season-end frame
        if !fading && self.stack[depth].fade().is_some() {
            if let Some(film) = tick_gate::season_skips_film(&self.ids(), depth) {
                // Smk_Skip 0x0042DF30 -> Smk_OnFinished
                ctx.game.films.finished = Some(film);
            }
            // FUN_0043AD25 00430000.c:6185 `g_screenId = 0;` under every screen [V]. [D] after
            // Season_Advance, not before (00490000.c:4716): the stack is presentation and
            // closes the frame phase 7 runs, before any screen that step pushes [I].
            let mut at = 0;
            let before = self.stack.len();
            self.stack.retain(|s| {
                at += 1;
                at <= depth + 1 || tick_gate::season_keeps(s.id())
            });
            self.dirty |= self.stack.len() != before;
        }
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

    /// `0x004B99C0`'s battle arm, at the battlefield's depth; withheld with the turn's
    /// by [`tick_gate`] (FUN_004BB978 0x004BB978).
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

}
