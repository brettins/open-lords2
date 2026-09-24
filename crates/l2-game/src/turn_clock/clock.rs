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

    /// **Block 174 is `DAT_005440C8` alone** — 4 bytes at file offset 266,448,
    /// `node tools/kingdom/savedump.js layout`. The anchor `_DAT_00568D9C`,
    /// `DAT_0055403C` and `DAT_0053E994` are in none of the 225 blocks, so the
    /// word below is the whole of the clock a file carries and
    /// [`TurnClock::resumed`] rebuilds the rest.
    pub fn from_save(remaining: i32) -> TurnClock {
        TurnClock { remaining, ..TurnClock::default() }
    }

    /// **`FUN_004976A1` (`0x004976A1`)**, what `Save_Read` runs once the blocks
    /// are in — `00490000.c:3080-3082`:
    ///
    /// ```c
    /// DAT_0055403C = 0;
    /// DAT_005440C8 = DAT_005440C8 + 10;
    /// _DAT_00568D9C = DVar1 + (g_optTimeLimit - DAT_005440C8) * -1000;
    /// ```
    ///
    /// So a load does **not** hand the whole limit back: the count resumes at
    /// what the file held plus ten seconds, and `Turn_Tick`'s
    /// `g_optTimeLimit - (now - start) / 1000` reads exactly that on the first
    /// frame. `Battle_ReturnToCampaign` rebases the same way off `DAT_0053E994`
    /// (`004a0000.c:4711-4713`). `DAT_0055403C = 0` is the loaded turn made
    /// live: `1 < DAT_0055403C` is what `Turn_Tick` restarts the count on and
    /// `DAT_0055403C < 1` what `FUN_0041A639` draws it behind. `[V]`
    ///
    /// `[D]` on the other two lines of `FUN_004976A1`, both multiplayer-only:
    /// `DAT_0052AFCC = 1` (`00490000.c:3079`) is the sync-skip counter, read in
    /// `FUN_00440666` (`00440000.c:239`) and `FUN_004407E5` and in neither
    /// before `g_multiplayer != 0`; `DAT_00553FC8 = 0` (`:3083`) is the
    /// sync-wait latch, raised only under `g_multiplayer != 0`
    /// (`00490000.c:5294`, `:5391`).
    ///
    /// `[D]` on one more: `Setup_StartGame` (`0x004329EC`) writes
    /// `DAT_005440C8 = g_optTimeLimit` before the first frame and ours starts
    /// on its first tick instead, so a file written before one would resume at
    /// its word plus ten rather than the limit plus ten. The frame driver ticks
    /// the clock every frame, so no save box is up before the first one.
    pub fn resumed(&mut self, limit: i32) {
        self.started = true;
        self.restart_pending = false;
        self.remaining = self.remaining.saturating_add(LOAD_GRACE);
        self.elapsed_ms = (i64::from(limit) - i64::from(self.remaining)) * 1000;
    }

    pub fn end_turn_pending(&self) -> bool {
        self.end_turn
    }

    /// Turn_End 0x0043AC23 pressed in phases 1-3: its 999 outlives them, since
    /// Turn_BeginPlayersTurn 0x0049B6D3 ran at the wrap, before them [V].
    pub fn defer_end_turn(&mut self) {
        self.end_turn = true;
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
    /// `DAT_00553018`: 1 from App_WinMain 0x0040E9AB (00400000.c:8248) and Turn_Tick's
    /// restart (00490000.c:4806); F12 writes it ([`TurnClock::f12`]) [V]. Turn_AllRealmsDone's
    /// network arm (:5337) clears it beside `DAT_00553FC8 = 1`: a network game keeps the latch
    /// [D] netcode. C249.
    pub fn force_close(&self, multiplayer: bool) -> bool {
        self.restart_pending && (multiplayer || self.gate_shut)
    }

    /// App_WndProc 0x004B29BE 004b0000.c:2053-2055 `case 0x7b: DAT_00553018 = 1;
    /// if (g_screenId == 9 && g_mapZoom == 2) DAT_00553018 = 0;` [V]
    pub fn f12(&mut self, court_far: bool) {
        self.gate_shut = court_far;
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
        // Turn_Tick 0x0049A010 00490000.c:4799
        // `if (1 < DAT_0055403C && g_realms[g_localPlayer].aiStep != 999)` —
        // single player's `local_8 = 0xBB9` passes the 2.5-second wait at once.
        if self.restart_pending && !self.end_turn {
            // Turn_Tick 0x0049A010 00490000.c:4806 `DAT_00553018 = 1` [V]
            self.gate_shut = false;
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

