//! **The turn timer** — `g_optTimeLimit`'s countdown, the two draws it makes,
//! and the turn it ends.
//!
//! # What it is, from the binary
//!
//! **A per-turn time limit on the person's own turn, in single player as well
//! as in a network game.** **[D]** throughout unless marked, addresses the GOG
//! build's.
//!
//! * **The setting.** *Time limit*, the custom game's drop-down 10, which the
//!   single-player page (`g_setupPage` 7, `FUN_0041F86D(999)`) draws with the
//!   other eleven. `Setup_CommitOptions` (`0x00499DC3`) puts its index through
//!   `g_timeLimitSeconds` (`0x004DBBF8`) = `30, 60, 120, 240, 480, 600, 0` into
//!   `g_optTimeLimit` (`0x0053F26C`). `Setup_DefaultOptions` picks *no limit*
//!   for a single player and *4 mins* for a network game, and
//!   `Campaign_LoadEntry` forces it to 0 — **so the campaign never has one, and
//!   a single-player custom game has one only if the person chose it.** See
//!   [`crate::setup`].
//! * **The clock.** Two globals: `DAT_005440C8`, whole seconds left, and
//!   `_DAT_00568D9C`, the `timeGetTime()` the count started from. `Turn_Tick`
//!   (`0x0049A010`) recomputes the first from the second **in its phase-4 arm
//!   only** — the players' turn — as `g_optTimeLimit - (now - start) / 1000`,
//!   and nothing in phases 5, 6, 7, 1, 2 or 3 touches either.
//! * **Running out.** Below zero the same arm writes `-1`, calls `Turn_End`
//!   (`0x0043AC23`) — the End Turn button's own handler, which ends **the local
//!   player's** turn and nobody else's — and repaints the End Turn strip.
//! * **Starting again.** `Turn_End` leaves `DAT_0055403C` at 2 in single
//!   player. The phase-4 arm's third clause, `1 < DAT_0055403C && aiStep !=
//!   999`, then resets the count to the full limit the first frame the person's
//!   next turn is live (a network game waits a further 2.5 s after phase 1).
//! * **The draw.** `FUN_0041A639`, once a frame from `Battle_Frame`, never from
//!   a painter:
//!
//!   ```c
//!   if (g_battlePhase == 0 && 2 < g_appPhase && 0 < DAT_005440C8 &&
//!       (DAT_0055403C < 1 || g_realms[g_localPlayer].aiStep == 999) &&
//!       0 < g_optTimeLimit && DAT_004D2E80[g_screenId] == 0) {
//!       Pl8_DrawFrame(g_miscCtySheet, 0x60, 0x194, 0x1AE);
//!       Ui_DrawNumberRight(DAT_005440C8, ' ', &DAT_004D41D0, 0x1A8, 0x1BA, 0x32, &g_fontBody, 0x3F);
//!   }
//!   ```
//!
//!   **`aiStep == 999` is one half of an `||`, not the guard.**
//!   `docs/draws-map.md` §5.11 and `docs/decisions.md` C140 both wrote the
//!   condition as `g_optTimeLimit > 0 && aiStep == 999`, which draws the timer
//!   only *after* the person has ended his turn — exactly backwards for a
//!   countdown. `DAT_0055403C` is 0 for the whole of his own turn, so the first
//!   half holds and the timer is up while he plays.
//!
//! `&DAT_004D41D0` is `" "` — read out of `.data` — so the buffer is
//! `" 30 "`, centred in fifty pixels from x 424: `Ui_DrawNumberRight`
//! **centres** (C119), and both spaces are inside the measure (C140). **[V]**
//! on the suffix and the table below, both read from the image.
//!
//! # The trap, and the mapping that avoids it
//!
//! Our turn model is the original's rotated. Its phase 4 *is* the interactive
//! phase with the person parked at `aiStep == 1`; ours parks him on the map
//! with the machine at phase 1 and runs 1 … 7 inside one press of End Turn, so
//! between turns **every** realm's counter is already at or past 999. So
//! nothing here reads `ai_step`:
//!
//! * *"the local player's `aiStep == 999`"* is a turn in flight that is not an
//!   idle battle — [`crate::turn::players_turn_ended`] — or a `Turn_End` the
//!   clock has asked for and the map has not yet carried out;
//! * *"`Turn_Tick`'s phase-4 arm"* is every tick on which that is false.
//!
//! # What is reproduced and what is not
//!
//! Reproduced, each with a test in this module: the arithmetic (a 30-second
//! limit ends the turn on the tick that passes **31** seconds, and `0` is never
//! drawn); the restart; the countdown running **before** the restart in the
//! same frame, which hands a slow turn's processing to the next turn (`docs/
//! bugs.md` B99); a fought battle's `+10`
//! (`Battle_Start` saves `DAT_005440C8 + 10` into `DAT_0053E994`,
//! `Battle_ReturnToCampaign(1)` rebases the start on it); and a siege assault
//! zeroing `DAT_0055403C` so that the next turn does not restart the count.
//!
//! **Not reproduced, and said here rather than left to be inferred:**
//!
//! * the count carrying on through the rest of phase 4 after End Turn while the
//!   AI realms finish stepping — ours freezes it at the click, because our
//!   phase 4 comes after 1, 2 and 3 rather than before them;
//! * the timer hiding during phases 1 … 3 (where `Turn_BeginPlayersTurn` has
//!   cleared the person's counter and `DAT_0055403C` is still 2) — the same
//!   rotation puts those phases first in ours;
//! * the network game's 2.5-second restart delay and its ten-second `Turn_End`
//!   resend. `g_multiplayer` is never set in this workspace, and a multiplayer
//!   clock is `docs/netcode.md`'s to design, not this module's to port;
//! * `FUN_004976A1`, the in-game load's `DAT_005440C8 += 10`. A game loaded
//!   here starts the count at the full limit, which is what `Setup_StartGame`'s
//!   own two lines do.
//!
//! # Determinism
//!
//! **No clock is read.** The original's `timeGetTime()` difference is counted
//! here in fixed ticks of [`crate::TICK_MS`], so the same inputs run out on the
//! same tick every time. The clock is session state on [`Game`], not world
//! state: it is not in the save — the original restarts it on a start and on a
//! load — and not in the lockstep digest, and the only thing it can do to the
//! world is press End Turn, which is a person's input.

use l2_view::Canvas;

use crate::game::Game;
use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};

/// `Pl8_DrawFrame(g_miscCtySheet, 0x60, …)` — the plate behind the number.
pub const FRAME: usize = 0x60;
/// `0x194` — the plate's x. **Inside the map viewport**, 74 pixels left of the
/// sidebar, which is the only thing on the campaign screen that is.
pub const FRAME_X: i32 = 0x194;
/// `0x1AE`.
pub const FRAME_Y: i32 = 0x1AE;
/// `Ui_DrawNumberRight(…, 0x1A8, 0x1BA, 0x32, …)` — the box the number is
/// centred in.
pub const NUMBER_X: i32 = 0x1A8;
pub const NUMBER_Y: i32 = 0x1BA;
pub const NUMBER_W: i32 = 0x32;
/// The lead, `' '`.
pub const LEAD: char = ' ';
/// `&DAT_004D41D0` — `20 00 00 00` in `.data`, a single space. **[V]**
pub const SUFFIX: &str = " ";
/// `0x3F`, the body text colour.
pub const COLOUR: u8 = 0x3F;
/// `Battle_Start` (`0x004778A0`): `DAT_0053E994 = DAT_005440C8 + 10`.
pub const BATTLE_GRACE: i32 = 10;
/// What `Turn_Tick` writes when the count goes below zero.
pub const EXPIRED: i32 = -1;

/// **`DAT_004D2E80` — which screens the timer is drawn over**, indexed by
/// `g_screenId`, and 0 means *draw*. **[V]**, read out of `.data`.
///
/// It has **one reader in the whole binary**, `FUN_0041A639`, so it is the
/// timer's own vocabulary and not a general screen property borrowed for it.
/// Seventy bytes for `g_screenId` `0x00 … 0x45`, the range every write of the
/// byte covers; the two bytes after it are zero and are not claimed.
pub const SCREENS: [u8; 0x46] = [
    0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 1, 1, 1, 0, 0, // 0x00
    0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 2, // 0x10
    1, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // 0x20
    2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2, 2, 2, // 0x30
    2, 2, 2, 2, 2, 2, // 0x40
];

/// **The screens `Screen_FrameInput` (`0x0042FF10`) closes once a turn has
/// been ended** — every `g_screenId` whose arm carries the guard
/// `DAT_00553FC8 != 0 || (DAT_0055403C != 0 && DAT_00553018 == 0)`.
///
/// Twenty-seven sites of that guard in the function's body, and **every one but
/// `0x00`'s closes**. Nine test it and close at once — `0x02`, `0x04`, `0x05`,
/// `0x06`, `0x10`, `0x11`, `0x1D`, `0x26` and `0x39`, read. Eighteen test its
/// negation around the arm's input instead; the `else` was read for `0x09`,
/// `0x13`, `0x14` and `0x35`/`0x36`, all of which close, and the other thirteen
/// have the same shape and were not each opened. `0x00` is the map itself, where
/// the negated guard only stops the menus and the strip taking clicks. `0x11`
/// closes to `0x04`, `0x05` and `0x06` to `0x02`, `0x35`/`0x36` and `0x39` to the
/// screen they were opened over, so those chains reach the map a frame later.
/// **[D]** — and `docs/arms.json` `0x0042FF10/force-close-on-turn-end`, which
/// counts twenty-nine.
pub const CLOSED_BY_TURN_END: [u8; 26] = [
    0x02, 0x04, 0x05, 0x06, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x10, 0x11, 0x13, 0x14, 0x15,
    0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1D, 0x20, 0x26, 0x35, 0x36,
];

/// `0x39`, the advanced options, whose guard also tests `g_battlePhase == 0`.
/// Listed apart only because the array above is sized by hand.
pub const CLOSED_BY_TURN_END_OUTSIDE_BATTLE: u8 = 0x39;

/// **Our screen as the original's `g_screenId` byte**, or `None` for a screen
/// the original has no id for.
///
/// `None` is ours — the demo index, the quirks page — or the message scroll,
/// which the original paints over whatever is up without touching the byte;
/// [`timer_screen`] looks under it.
pub fn screen_byte(id: ScreenId) -> Option<u8> {
    use crate::screens::county::Panel;
    use crate::screens::options::Page;
    Some(match id {
        ScreenId::Campaign => 0x00,
        ScreenId::Village(_) => 0x02,
        ScreenId::Info(_) => 0x04,
        ScreenId::Merchant(_) => 0x08,
        ScreenId::Court => 0x09,
        ScreenId::Armoury(_) => 0x0A,
        ScreenId::Diplomacy => 0x0B,
        ScreenId::Trade(..) => 0x0C,
        ScreenId::Rack(..) => 0x0D,
        ScreenId::Job(..) => 0x0F,
        ScreenId::Divide(_) => 0x11,
        ScreenId::BattlePrompt => 0x12,
        ScreenId::BattleResult => 0x13,
        ScreenId::County(_, Panel::Population) => 0x14,
        ScreenId::County(_, Panel::Tax) => 0x15,
        ScreenId::County(_, Panel::Happiness) => 0x16,
        ScreenId::RaiseArmy(_) => 0x17,
        ScreenId::Supplies(_) => 0x18,
        ScreenId::County(_, Panel::Ration) => 0x19,
        ScreenId::DiploCompose(..) => 0x1A,
        ScreenId::Castle(_) => 0x1B,
        ScreenId::Conquest => 0x1C,
        ScreenId::Siege(_) => 0x1D,
        ScreenId::Setup(_) => 0x1F,
        ScreenId::Nobles => 0x20,
        // A film: `Smk_Play` parks `g_screenId` at `0x22`. `SCREENS[0x22]` is 2, so
        // the timer is not drawn over a film, and `0x22` is not in
        // `CLOSED_BY_TURN_END`. Both follow from the tables above.
        ScreenId::Movie(_) => 0x22,
        ScreenId::About => 0x25,
        // The tip's own screen: `Tip_Show` writes `g_screenId = 0x27` (see
        // `crate::tip::screen_byte`). `SCREENS[0x27]` is 2, so the timer is not
        // drawn while a tip is up, and `0x27` is not in `CLOSED_BY_TURN_END`, so
        // a turn end does not close it. Both follow from the tables above.
        ScreenId::Tip => 0x27,
        ScreenId::Battlefield => 0x29,
        ScreenId::Ratings => 0x2E,
        ScreenId::Options(Page::Help) => 0x31,
        ScreenId::MenuBar(_) => 0x32,
        ScreenId::SaveLoad(mode) => mode.screen_id(),
        ScreenId::Options(Page::Advanced) => 0x39,
        ScreenId::Options(Page::Sound) => 0x42,
        ScreenId::Options(Page::Display) => 0x43,
        ScreenId::Options(Page::Quirks) | ScreenId::Message | ScreenId::Menu | ScreenId::Index => {
            return None
        }
    })
}

/// Which screen's byte `FUN_0041A639` would read: the top of the stack, looking
/// through the message scroll.
pub fn timer_screen(stack: impl DoubleEndedIterator<Item = ScreenId>) -> Option<ScreenId> {
    stack.rev().find(|id| *id != ScreenId::Message)
}

/// `DAT_004D2E80[g_screenId] == 0`.
pub fn drawn_over(id: ScreenId) -> bool {
    screen_byte(id).is_some_and(|b| SCREENS.get(b as usize) == Some(&0))
}

/// Whether `Screen_FrameInput` closes this screen while a `Turn_End` stands.
pub fn closed_by_turn_end(id: ScreenId, in_battle: bool) -> bool {
    match screen_byte(id) {
        Some(CLOSED_BY_TURN_END_OUTSIDE_BATTLE) => !in_battle,
        Some(b) => CLOSED_BY_TURN_END.contains(&b),
        None => false,
    }
}

/// Where the person's turn is, as the clock needs to know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    /// His own turn: the original's phase 4 with his counter below 999.
    Players,
    /// **Still his own turn**, with a battle raised on an ordinary frame
    /// suspended over it. `Turn_Tick` runs under the prompt screen — it is
    /// `g_battlePhase`, not the screen, that stops it.
    Idle,
    /// He has ended it and the turn is being run: `aiStep == 999`.
    Ended,
}

/// Everything the clock reads in one tick, so the clock itself reads nothing
/// else and can be driven by a test with no world at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// `g_optTimeLimit`, seconds.
    pub limit: i32,
    pub turn: TurnState,
    /// `g_battlePhase != 0` — a battlefield is up and `Turn_Tick` is not called.
    pub battle: bool,
}

impl Frame {
    /// This tick's frame, read off the game.
    pub fn of(game: &Game) -> Frame {
        let turn = if crate::turn::players_turn_ended(game) {
            TurnState::Ended
        } else if crate::turn::turn_in_flight(game) {
            TurnState::Idle
        } else {
            TurnState::Players
        };
        Frame { limit: game.kingdom.options.time_limit, turn, battle: game.battle.is_some() }
    }
}

/// What one tick did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// The count moved, or there is no limit.
    Running,
    /// **`Turn_End`.** The count went below zero on this tick.
    Expired,
    /// The next turn is live and the count went back to the full limit.
    Restarted,
    /// The person's turn is over and the turn is being run; phase 4 is not.
    Waiting,
    /// A battlefield is up.
    Paused,
}

/// **The countdown**: `DAT_005440C8`, `_DAT_00568D9C`, `DAT_0055403C` and
/// `DAT_0053E994`, and the `Turn_End` request the map carries out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TurnClock {
    /// Whether `Setup_StartGame`'s `DAT_005440C8 = g_optTimeLimit; start =
    /// timeGetTime();` has happened for this game. A game — new or loaded — is
    /// a fresh [`Game`], so this is false exactly until its first tick.
    started: bool,
    /// `DAT_005440C8`: whole seconds left, or [`EXPIRED`].
    remaining: i32,
    /// `timeGetTime() - _DAT_00568D9C`, counted in ticks. Signed, because a
    /// fought battle rebases it and the rebase can put it before zero.
    elapsed_ms: i64,
    /// `DAT_0055403C > 1`: a `Turn_End` has happened and the count has not yet
    /// been restarted for the turn after it.
    restart_pending: bool,
    /// `DAT_0053E994`, while a battlefield is up.
    battle_saved: Option<i32>,
    /// The previous tick's [`TurnState::Ended`], so the edge into a turn — which
    /// is always a `Turn_End` — is seen once.
    was_ended: bool,
    /// **`Turn_End`, asked for by the clock and not yet carried out.** The map
    /// is the only screen that can start a turn; see
    /// [`crate::screen::Machine::update`].
    end_turn: bool,
}

impl TurnClock {
    /// `DAT_005440C8`.
    pub fn remaining(&self) -> i32 {
        self.remaining
    }

    /// Whether the clock has ended the person's turn and nothing has started it
    /// yet.
    pub fn end_turn_pending(&self) -> bool {
        self.end_turn
    }

    /// The map, carrying the request out.
    pub fn take_end_turn(&mut self) -> bool {
        core::mem::take(&mut self.end_turn)
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

    /// One fixed tick. `Turn_Tick`'s phase-4 arm when the person's turn is
    /// live, and bookkeeping otherwise.
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
            // `Battle_ReturnToCampaign(1)`:
            // `start = now + (g_optTimeLimit - DAT_0053E994) * -1000;
            //  DAT_005440C8 = DAT_0053E994;`
            self.remaining = saved;
            self.elapsed_ms = (i64::from(f.limit) - i64::from(saved)) * 1000;
        }
        // The wall clock never stops, whatever phase the turn is in.
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

        // **`Turn_Tick`'s phase-4 arm, in its own order** — the countdown
        // first, the restart after it. The order is the reproduction: see
        // `docs/bugs.md` B99.
        if self.remaining > EXPIRED && f.limit > 0 {
            let seconds = (self.elapsed_ms / 1000).clamp(i64::from(i32::MIN), i64::from(i32::MAX));
            self.remaining = f.limit.saturating_sub(seconds as i32);
            if self.remaining < 0 {
                self.remaining = EXPIRED;
                // `Turn_End` is guarded on `aiStep != 999`, and a request still
                // standing is a turn already ended.
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

/// The number the timer shows this frame, or `None` when it is not drawn —
/// everything but the screen table.
pub fn shown(game: &Game) -> Option<i32> {
    let ended = crate::turn::players_turn_ended(game) || game.turn_clock.end_turn_pending();
    game.turn_clock.value(game.kingdom.options.time_limit, ended, game.battle.is_some())
}

/// **`FUN_0041A639`'s two draws.** The caller has already tested the screen.
pub fn draw(ctx: &Ctx, canvas: &mut Canvas) {
    let Some(seconds) = shown(ctx.game) else { return };
    // Nothing in `FUN_0041A639` touches `DAT_005AEA40` or `DAT_0058FE2C`, so
    // this is the ordinary embossed body pen, the menu bar's.
    let pen = Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };
    pen.misc_frame(canvas, FRAME, FRAME_X, FRAME_Y);
    pen.number_centred(canvas, NUMBER_X, NUMBER_Y, NUMBER_W, seconds, LEAD, SUFFIX, COLOUR);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ticks in `ms` milliseconds, as the clock counts them.
    fn ticks_for(ms: u32) -> u32 {
        ms.div_ceil(crate::TICK_MS)
    }

    fn players(limit: i32) -> Frame {
        Frame { limit, turn: TurnState::Players, battle: false }
    }

    fn run(c: &mut TurnClock, f: Frame, n: u32) -> Tick {
        let mut last = Tick::Running;
        for _ in 0..n {
            last = c.tick(f);
        }
        last
    }

    /// **A thirty-second limit runs out when thirty-one seconds have passed**,
    /// and `0` is a whole second in which nothing is drawn and nothing ends.
    ///
    /// `DAT_005440C8 = limit - elapsed / 1000` goes 30, 29 … 1, 0, and the test
    /// is `< 0`; the draw's is `0 < DAT_005440C8`. The tick numbers are
    /// written out rather than derived from the clock's own arithmetic: with 16
    /// ms ticks, 1,937 ticks is 30.992 s and 1,938 is 31.008 s.
    #[test]
    fn a_thirty_second_limit_runs_out_on_the_tick_that_passes_thirty_one_seconds() {
        let mut c = TurnClock::default();
        assert_eq!(c.tick(players(30)), Tick::Running);
        assert_eq!(c.value(30, false, false), Some(30), "the first tick shows the whole limit");

        assert_eq!(run(&mut c, players(30), 1936), Tick::Running);
        assert_eq!(c.remaining(), 0);
        assert_eq!(c.value(30, false, false), None, "0 is never drawn");
        assert!(!c.end_turn_pending(), "and 0 does not end the turn");

        assert_eq!(c.tick(players(30)), Tick::Expired, "tick 1,938 is 31.008 seconds");
        assert_eq!(c.remaining(), EXPIRED);
        assert!(c.end_turn_pending(), "Turn_End");
        assert_eq!(c.value(30, true, false), None, "-1 is not drawn either");
        assert_eq!(ticks_for(31_008), 1938);
    }

    /// No limit is `g_optTimeLimit == 0`: the countdown's guard fails, nothing is
    /// drawn and nothing ends, however long the person sits there.
    #[test]
    fn no_limit_never_counts_and_never_ends_a_turn() {
        let mut c = TurnClock::default();
        run(&mut c, players(0), 100_000);
        assert!(!c.end_turn_pending());
        assert_eq!(c.value(0, false, false), None);
    }

    /// **The count is up during the person's own turn, frozen while his ended
    /// turn runs, and back at the full limit when the next one is live.**
    #[test]
    fn the_count_restarts_when_the_next_turn_is_live_and_not_before() {
        let mut c = TurnClock::default();
        run(&mut c, players(60), ticks_for(10_000));
        assert_eq!(c.value(60, false, false), Some(50));

        // End Turn with 50 left: the turn runs for four seconds.
        let ended = Frame { turn: TurnState::Ended, ..players(60) };
        assert_eq!(run(&mut c, ended, ticks_for(4_000)), Tick::Waiting);
        assert_eq!(c.value(60, true, false), Some(50), "frozen at the click, and still drawn");

        assert_eq!(c.tick(players(60)), Tick::Restarted);
        assert_eq!(c.value(60, false, false), Some(60));
    }

    /// **`B99`: the countdown runs before the restart.**
    ///
    /// The start is not moved while the ended turn runs, so the first frame of
    /// the next turn computes `limit - (time he took + time the turn took)`. If
    /// that is below zero the same frame calls `Turn_End` — on a counter
    /// `Turn_BeginPlayersTurn` has just set to 0 — and the restart clause, which
    /// comes after it, then sees `aiStep == 999` and does nothing. The next turn
    /// is over before the person has seen it.
    #[test]
    fn a_turn_ended_early_hands_its_running_time_to_the_next_turn() {
        let ended = |limit| Frame { turn: TurnState::Ended, ..players(limit) };

        // Twenty seconds of thinking and ten of turn: 30 s, which is not < 0.
        let mut c = TurnClock::default();
        run(&mut c, players(30), ticks_for(20_000));
        run(&mut c, ended(30), ticks_for(10_000));
        assert_eq!(c.tick(players(30)), Tick::Restarted);

        // Twenty and twelve: 32 s, and the next turn ends on its first frame.
        let mut c = TurnClock::default();
        run(&mut c, players(30), ticks_for(20_000));
        run(&mut c, ended(30), ticks_for(12_000));
        assert_eq!(c.tick(players(30)), Tick::Expired, "the countdown runs first");
        assert!(c.end_turn_pending());

        // That turn runs; the one after it gets the full limit back, because
        // the count is -1 and only the restart clause is left to run.
        run(&mut c, ended(30), 10);
        assert_eq!(c.tick(players(30)), Tick::Restarted);
        assert_eq!(c.value(30, false, false), Some(30));
    }

    /// `Battle_Start` saves the count plus ten and `Battle_ReturnToCampaign(1)`
    /// rebases the start on it, so a fought battle costs no time and gives ten
    /// seconds back.
    #[test]
    fn a_fought_battle_stops_the_count_and_gives_ten_seconds_back() {
        let mut c = TurnClock::default();
        run(&mut c, players(120), ticks_for(100_000));
        assert_eq!(c.value(120, false, false), Some(20));

        let battle = Frame { battle: true, ..players(120) };
        assert_eq!(run(&mut c, battle, ticks_for(600_000)), Tick::Paused);
        assert_eq!(c.value(120, false, true), None, "g_battlePhase != 0 is not drawn over");

        c.tick(players(120));
        assert_eq!(c.value(120, false, false), Some(30));
    }

    /// `DAT_0055403C = 0` before every siege assault: the next turn does not
    /// restart the count, and counts on from the old start instead.
    #[test]
    fn a_siege_assault_leaves_the_next_turn_without_a_restart() {
        let mut c = TurnClock::default();
        run(&mut c, players(240), ticks_for(100_000));
        let ended = Frame { turn: TurnState::Ended, ..players(240) };
        run(&mut c, ended, ticks_for(1_000));
        c.assault_launched();
        run(&mut c, ended, ticks_for(1_000));
        assert_eq!(c.tick(players(240)), Tick::Running, "no restart");
        assert_eq!(c.value(240, false, false), Some(138));
    }

    /// **An idle battle is still the person's own turn.** The prompt is a
    /// screen, `g_battlePhase` is 0 under it, and the count goes on.
    #[test]
    fn the_count_runs_under_a_battle_prompt_raised_on_an_ordinary_frame() {
        let mut c = TurnClock::default();
        let idle = Frame { turn: TurnState::Idle, ..players(30) };
        run(&mut c, idle, ticks_for(31_008));
        assert!(c.end_turn_pending());
    }

    /// The table's zeroes, spot-checked against what each screen is: the map,
    /// the village, the county panels and the court are drawn over; the
    /// diplomacy page, the armoury, both battle screens and the setup pages
    /// are not. `docs/draws-map.md` §5.6.
    #[test]
    fn the_timer_is_drawn_over_the_map_and_its_insets_and_not_over_the_pages() {
        use crate::screens::county::Panel;
        for id in [
            ScreenId::Campaign,
            ScreenId::Village(1),
            ScreenId::County(1, Panel::Tax),
            ScreenId::County(1, Panel::Ration),
            ScreenId::Court,
            ScreenId::Info(crate::screens::info::Target::Tile(0)),
        ] {
            assert!(drawn_over(id), "{id:?}");
        }
        for id in [
            ScreenId::Diplomacy,
            ScreenId::Armoury(1),
            ScreenId::BattlePrompt,
            ScreenId::BattleResult,
            ScreenId::Battlefield,
            ScreenId::Conquest,
            ScreenId::Setup(crate::screens::setup::SetupPage::Title),
            ScreenId::Index,
        ] {
            assert!(!drawn_over(id), "{id:?}");
        }
        assert_eq!(SCREENS.iter().filter(|&&b| b == 0).count(), 27);
    }
}
