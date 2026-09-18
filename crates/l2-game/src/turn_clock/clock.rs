#![allow(unused_imports)]
use super::*;
use super::draw_part::*;
use screens::*;
use l2_view::Canvas;
use crate::game::Game;
use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};

impl TurnClock {
    /// `DAT_005440C8`.
    pub fn remaining(&self) -> i32 {
        self.remaining
    }

    pub fn end_turn_pending(&self) -> bool {
        self.end_turn
    }

    pub fn take_end_turn(&mut self) -> bool {
        core::mem::take(&mut self.end_turn)
    }

    /// ```c
    /// if (DAT_00553FC8 != 0 || (DAT_0055403C != 0 && DAT_00553018 == 0)) …
    /// ```
    ///
    /// Twenty-seven sites, twenty-six of which close their screen —
    /// [`CLOSED_BY_TURN_END`]. `DAT_0055403C` is what `Turn_End`
    /// (`0x0043AC23`) writes — `2` in single player — and `Turn_Tick`'s
    /// restart clears it on the first frame of the person's next live turn, so
    /// the guard stands for **the whole turn that follows**, not only for the
    /// frame the clock ran out on. That is [`TurnClock::restart_pending`]
    /// exactly, and it is a wider window than [`TurnClock::end_turn_pending`],
    /// which the map takes as soon as it is on top.
    ///
    /// **Two clauses are not ours and are named so the reader knows what is
    /// `DAT_00553FC8`
    /// is the multiplayer sync-wait latch, which nothing in this engine sets
    /// (`docs/netcode.md`); `DAT_00553018` is the F12 debug override, whose one
    /// setter is `App_WndProc`'s `VK_F12` arm and which we do not have. With
    /// both absent the guard reduces to this one field.
    pub fn force_close(&self) -> bool {
        self.restart_pending
    }

    /// **`Turn_Tick`'s phase-2 arm: `DAT_0055403C = 0; Siege_LaunchAssault(…)`.**
    ///
    /// Every assault the phase launches — refused or fought, anybody's — clears
    /// the flag the restart waits on, so a turn with a siege assault in it does
    /// not give the person his full limit back: his next turn counts on from
    /// wherever the last one's start left it. **[D]** on the write, and
    /// `docs/bugs.md` B99 has what follows from it.
    pub fn assault_launched(&mut self) {
        self.restart_pending = false;
    }

    pub fn tick(&mut self, f: Frame) -> Tick {
        if !self.started {
            // `Setup_StartGame` (`0x004329EC`), and the network game's two
            // start handlers: `DAT_005440C8 = g_optTimeLimit; _DAT_00568D9C =
            // timeGetTime();`.
            self.started = true;
            self.remaining = f.limit;
            self.elapsed_ms = 0;
        }
        if f.battle {
            // `Battle_Start`: `DAT_0053E994 = DAT_005440C8 + 10`. `Turn_Tick` is
            // behind `g_battlePhase == 0` and does not run until it is over.
            if self.battle_saved.is_none() {
                self.battle_saved = Some(self.remaining + BATTLE_GRACE);
            }
            return Tick::Paused;
        }
        if let Some(saved) = self.battle_saved.take() {
            // `start = now + (g_optTimeLimit - DAT_0053E994) * -1000;
            //  DAT_005440C8 = DAT_0053E994;`
            self.remaining = saved;
            self.elapsed_ms = (i64::from(f.limit) - i64::from(saved)) * 1000;
        }
        self.elapsed_ms += i64::from(crate::TICK_MS);

        if f.turn == TurnState::Ended {
            if !self.was_ended {
                // `Turn_End`: `DAT_0055403C = 2` in single player. Every turn
                // begins with one, whichever door it came through.
                self.restart_pending = true;
                self.was_ended = true;
            }
            self.end_turn = false;
            return Tick::Waiting;
        }
        self.was_ended = false;

        if self.remaining > EXPIRED && f.limit > 0 {
            let seconds = (self.elapsed_ms / 1000).clamp(i64::from(i32::MIN), i64::from(i32::MAX));
            self.remaining = f.limit.saturating_sub(seconds as i32);
            if self.remaining < 0 {
                self.remaining = EXPIRED;
                if !self.end_turn {
                    self.end_turn = true;
                    self.restart_pending = true;
                }
                return Tick::Expired;
            }
        }
        // `if (1 < DAT_0055403C && g_realms[g_localPlayer].aiStep != 999)` —
        // single player's `local_8 = 0xBB9` passes the 2.5-second wait at once.
        if self.restart_pending && !self.end_turn {
            self.restart_pending = false;
            self.remaining = f.limit;
            self.elapsed_ms = 0;
            return Tick::Restarted;
        }
        Tick::Running
    }

    /// **`FUN_0041A639`'s guard**, less the two clauses the frame driver
    /// answers — the screen byte and `g_appPhase` — and with `ended` standing
    /// for the person's `aiStep == 999`.
    pub fn value(&self, limit: i32, ended: bool, battle: bool) -> Option<i32> {
        let up = !battle
            && self.started
            && 0 < self.remaining
            && (!self.restart_pending || ended)
            && 0 < limit;
        up.then_some(self.remaining)
    }
}

