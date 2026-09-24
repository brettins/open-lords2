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
        l2_net::trace_changed!(self.traced_screen_byte(ctx.game).map_or(-1, i64::from); "screen", 0x004EAC50, "g_screenId", id = self.traced_screen_byte(ctx.game).map_or(-1, i64::from), ours = format!("{:?}", self.top_id()));
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
        // A network game's clock is `net::frame`'s, on the session's tick.
        if !ctx.game.net.lockstep {
            self.run_turn_clock(ctx);
        } else {
            // Screen_FrameInput 0x0042FF10 force_close, per seat off the shared clock [I]
            self.dirty |= crate::turn_clock::seat_saw_expiry(ctx.game, &mut self.clock_seen);
            self.dirty |= tick_gate::close_on_turn_end(&mut self.stack, ctx.game);
        }
        if self.join_battle(ctx) || self.wind_turn(ctx) {
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

    fn wind_turn(&mut self, ctx: &mut Ctx) -> bool {
        // FUN_004BB978 0x004BB978: no ticks under DAT_004E31B0[g_screenId] or a letter
        let byte = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message).and_then(|s| {
            s.mode_screen_id().or_else(|| crate::turn_clock::screen_byte(s.id()))
        });
        let withheld = tick_gate::withheld(byte, ctx.game.messages.is_open(), ctx.game.multiplayer);
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
