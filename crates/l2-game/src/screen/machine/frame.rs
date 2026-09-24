#![allow(unused_imports)]
use super::*;
use super::state::*;
use super::turns::*;
use super::drawing::*;
use super::super::*;
use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

impl Machine {
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
    pub(crate) fn run_tooltips(&mut self, ctx: &mut Ctx) {
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
    pub(super) fn pump_messages(&mut self, ctx: &mut Ctx) {
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
    pub(super) fn run_turn_clock(&mut self, ctx: &mut Ctx) {
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
        // Holds in a network game only: DAT_00553018 = 1 in single player
        // (00400000.c:8248) [V]; TurnClock::force_close. CNEW-gate-open-in-single-player.
        //
        // arm: 0x0042FF10/force-close-on-turn-end timer
        self.dirty |= tick_gate::close_on_turn_end(&mut self.stack, ctx.game);
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
    pub(super) fn run_tips(&mut self, ctx: &mut Ctx) {
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
    pub(super) fn draw_turn_timer(&self, ctx: &Ctx, canvas: &mut Canvas) {
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

}
