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
//!
//! * **The clock.** Two globals: `DAT_005440C8`, whole seconds left, and
//!   `_DAT_00568D9C`, the `timeGetTime()` the count started from. `Turn_Tick`
//!   (`0x0049A010`) recomputes the first from the second **in its phase-4 arm
//!   only** — the players' turn — as `g_optTimeLimit - (now - start) / 1000`,
//!   and nothing in phases 5, 6, 7, 1, 2 or 3 touches either.
//!
//! * **Running out.** Below zero the same arm writes `-1`, calls `Turn_End`
//!   (`0x0043AC23`) — the End Turn button's own handler, which ends **the local
//!   player's** turn and nobody else's — and repaints the End Turn strip.
//!
//! * **Starting again.** `Turn_End` leaves `DAT_0055403C` at 2 in single
//!   player. The phase-4 arm's third clause, `1 < DAT_0055403C && aiStep !=
//!   999`, then resets the count to the full limit the first frame the person's
//!   next turn is live (a network game waits a further 2.5 s after phase 1).
//!
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
//!   `docs/draws-map.md` §5.11 and `docs/decisions.md` C140 both wrote the
//!   condition as `g_optTimeLimit > 0 && aiStep == 999`, which draws the timer
//!   only *after* the person has ended his turn — exactly backwards for a
//!   countdown. `DAT_0055403C` is 0 for the whole of his own turn, so the first
//! half holds and the timer is up while he plays.
//!
//! `&DAT_004D41D0` is `" "` — read out of `.data` — so the buffer is
//! `" 30 "`, centred in fifty pixels from x 424: `Ui_DrawNumberRight`
//! **centres** (C119), and both spaces are inside the measure (C140). **[V]**
//! on the suffix and the table below, both read from the image.
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
//! * the count carrying on through the rest of phase 4 after End Turn while the
//!   AI realms finish stepping — ours freezes it at the click, because our
//! phase 4 comes after 1, 2 and 3;
//! * the timer hiding during phases 1 … 3 (where `Turn_BeginPlayersTurn` has
//!   cleared the person's counter and `DAT_0055403C` is still 2) — the same
//!   rotation puts those phases first in ours;
//! * the network game's 2.5-second restart delay and its ten-second `Turn_End`
//!   resend. `g_multiplayer` is never set in this workspace, and a multiplayer
//!   clock is `docs/netcode.md`'s to design, not this module's to port;
//! * `FUN_004976A1`, the in-game load's `DAT_005440C8 += 10`. A game loaded
//!   here starts the count at the full limit, which is what `Setup_StartGame`'s
//!   own two lines do.

mod clock;
pub use clock::*;
mod draw_part;
pub use draw_part::*;

mod screens;
pub use screens::*;

use l2_view::Canvas;

use crate::game::Game;
use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};

pub const FRAME: usize = 0x60;
pub const FRAME_X: i32 = 0x194;
pub const FRAME_Y: i32 = 0x1AE;
pub const NUMBER_X: i32 = 0x1A8;
pub const NUMBER_Y: i32 = 0x1BA;
pub const NUMBER_W: i32 = 0x32;
pub const LEAD: char = ' ';
/// `&DAT_004D41D0` — `20 00 00 00` in `.data`, a single space. **[V]**
pub const SUFFIX: &str = " ";
pub const COLOUR: u8 = 0x3F;
/// `Battle_Start` (`0x004778A0`): `DAT_0053E994 = DAT_005440C8 + 10`.
pub const BATTLE_GRACE: i32 = 10;
pub const EXPIRED: i32 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    Players,
    Idle,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub limit: i32,
    pub turn: TurnState,
    pub battle: bool,
}

impl Frame {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    Running,
    Expired,
    Restarted,
    Waiting,
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
    elapsed_ms: i64,
    /// `DAT_0055403C > 1`: a `Turn_End` has happened and the count has not yet
    /// been restarted for the turn after it.
    restart_pending: bool,
    /// `DAT_0053E994`, while a battlefield is up.
    battle_saved: Option<i32>,
    was_ended: bool,
    end_turn: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// `DAT_005440C8 = limit - elapsed / 1000` goes 30, 29 … 1, 0, and the test
    /// is `< 0`; the draw's is `0 < DAT_005440C8`. The tick numbers are
    /// written out: with 16
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

    /// **The force-close guard is `DAT_0055403C`, and it is wider than the
    /// clock's own `Turn_End` request in both directions.**
    ///
    /// `Turn_End` (`0x0043AC23`) writes it whichever door the turn was ended
    /// through — the clock's, or the person clicking End Turn — and `Turn_Tick`
    /// clears it on the first frame of his next live turn. So twenty-six of
    /// `Screen_FrameInput`'s twenty-seven guard sites close their screen for the
    /// **whole turn in between**, not for the one frame the clock ran out on.
    ///
    /// **Ablation, run:** make `force_close` return `self.end_turn` and the
    /// first block goes red at once — ending the turn by hand raises no request.
    #[test]
    fn the_force_close_latch_stands_for_the_whole_turn_and_not_only_for_the_request() {
        let mut c = TurnClock::default();
        assert_eq!(c.tick(players(0)), Tick::Running);
        assert!(!c.force_close(), "a live turn closes nothing");
        let ended = Frame { turn: TurnState::Ended, ..players(0) };
        assert_eq!(c.tick(ended), Tick::Waiting);
        assert!(c.force_close(), "Turn_End writes DAT_0055403C whichever door it came through");
        assert!(!c.end_turn_pending(), "and it is NOT the clock's request: there is no limit");

        assert_eq!(run(&mut c, ended, 500), Tick::Waiting);
        assert!(c.force_close(), "still standing 500 ticks into the turn");

        assert_eq!(c.tick(players(0)), Tick::Restarted);
        assert!(!c.force_close(), "Turn_Tick's restart is the one writer that clears it");
    }

    #[test]
    fn no_limit_never_counts_and_never_ends_a_turn() {
        let mut c = TurnClock::default();
        run(&mut c, players(0), 100_000);
        assert!(!c.end_turn_pending());
        assert_eq!(c.value(0, false, false), None);
    }

    #[test]
    fn the_count_restarts_when_the_next_turn_is_live_and_not_before() {
        let mut c = TurnClock::default();
        run(&mut c, players(60), ticks_for(10_000));
        assert_eq!(c.value(60, false, false), Some(50));

        let ended = Frame { turn: TurnState::Ended, ..players(60) };
        assert_eq!(run(&mut c, ended, ticks_for(4_000)), Tick::Waiting);
        assert_eq!(c.value(60, true, false), Some(50), "frozen at the click, and still drawn");

        assert_eq!(c.tick(players(60)), Tick::Restarted);
        assert_eq!(c.value(60, false, false), Some(60));
    }

    #[test]
    fn a_turn_ended_early_hands_its_running_time_to_the_next_turn() {
        let ended = |limit| Frame { turn: TurnState::Ended, ..players(limit) };

        let mut c = TurnClock::default();
        run(&mut c, players(30), ticks_for(20_000));
        run(&mut c, ended(30), ticks_for(10_000));
        assert_eq!(c.tick(players(30)), Tick::Restarted);

        let mut c = TurnClock::default();
        run(&mut c, players(30), ticks_for(20_000));
        run(&mut c, ended(30), ticks_for(12_000));
        assert_eq!(c.tick(players(30)), Tick::Expired, "the countdown runs first");
        assert!(c.end_turn_pending());

        run(&mut c, ended(30), 10);
        assert_eq!(c.tick(players(30)), Tick::Restarted);
        assert_eq!(c.value(30, false, false), Some(30));
    }

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

    #[test]
    fn the_count_runs_under_a_battle_prompt_raised_on_an_ordinary_frame() {
        let mut c = TurnClock::default();
        let idle = Frame { turn: TurnState::Idle, ..players(30) };
        run(&mut c, idle, ticks_for(31_008));
        assert!(c.end_turn_pending());
    }

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


