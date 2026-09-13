//! **When a tick falls due** — the arithmetic behind `main`'s event loop, kept
//! here because a binary's code cannot be called by a test.
//!
//! # What the original does, and why this has to exist at all
//!
//! `App_WinMain`'s message pump (`0x0040E9AB`) is **unthrottled**, `[V]`:
//! `PeekMessageA`, and when the queue is empty `App_IdleFrame`
//! (`0x0040E8BB`) → `App_Draw` → `Battle_Frame`, straight round again with no
//! wait of any kind. The only `Sleep` in it is the 200 ms one taken when the
//! window is inactive. So **the original has no tick**: it draws as fast as the
//! machine allows and every paced thing in it reads a clock for itself.
//!
//! Ours cannot do that — an unthrottled loop repaints faster than the surface
//! can present and wgpu rejects the submission (`main.rs`) — so it steps on a
//! fixed [`crate::TICK_MS`] tick instead, and everything timed is counted in
//! ticks. That trade is only sound while **a tick is really 16 ms of wall
//! clock**, and this module is what makes it so.
//!
//! # The film is what proves it
//!
//! `Smk_PlayLoop` (`0x0042DBC7`) advances a film only when
//! `SmackWait` (ordinal 32, called at `0x0042DBF7`) answers 0, and
//! `_SmackWait@4` is a `timeGetTime` comparison — `[V]`: it is 320 bytes at
//! RVA `0x3170` of `Smackw32.dll` and calls `WINMM.dll!timeGetTime` at `+0xB0`,
//! the only import it calls. So the film's clock is **real milliseconds**,
//! while its sound track is played out by the device in real time as well. The
//! two agree by construction: measured over the install's 45 films, the track
//! runs `frames × period` long to within **1 ms over 131 s**
//! (`crates/l2-smk/tests/corpus.rs`), so "the audio buffer" and "the header's
//! frame rate" are the same clock and neither is the game's frame count.
//!
//! A tick that is not 16 ms therefore desynchronises the picture from the
//! sound, and only in one direction. `docs/decisions.md` C193.
//!
//! # The rule, and the rule it replaces
//!
//! ```text
//! self.next_tick = Instant::now() + TICK;      // what main.rs used to do
//! ```
//!
//! `Instant::now()` there is read **after** the wait, so it is the deadline
//! *plus* however far the wait overshot, and the next deadline is measured from
//! that. Overshoot is never negative, so the clock can only ever run slow, and
//! the error is kept: it compounds once per tick. Measured
//! with winit 0.30's own wait primitive — `CreateWaitableTimerExW` with
//! `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` and `WaitForSingleObject`, which is
//! what `ControlFlow::WaitUntil` runs on Windows — a 16 ms tick came out at
//! **16.31–16.42 ms**, +1.9 % to +2.6 %. Over `intro.smk`'s 131.5 s that is
//! two and a half to three and a half seconds of picture behind sound.
//!
//! [`Ticker`] measures every deadline from the **previous deadline**, so an
//! overshoot is repaid on the next tick instead of being carried, and the tick
//! count is a true clock. What a wake cannot repay in [`MAX_CATCH_UP`] ticks it
//! gives up on, which is the one place time is allowed to be lost: a machine
//! that has been stopped for a second must not then run a second of game at
//! once.

use crate::TICK_MS;

/// One tick, in nanoseconds — [`crate::TICK_MS`] in the unit a monotonic
/// reading arrives in.
pub const TICK_NS: u64 = TICK_MS as u64 * 1_000_000;

/// **The most ticks one wake may run.** 128 ms of catch-up.
///
/// Without a cap, a loop that was stopped — the window dragged, the machine
/// suspended, a breakpoint — returns owing every tick of the gap and runs them
/// back to back, which is worse than the lost time: the game visibly
/// fast-forwards. With it, a stall is *dropped* and the clock restarts from
/// now.
pub const MAX_CATCH_UP: u32 = 8;

/// **The application's frame clock.** Pure arithmetic over a monotonic reading
/// the caller supplies: nothing here reads a clock, which is what lets it live
/// below `main.rs` (`docs/netcode.md` D-12 — the simulation may not consult
/// time, and this does not; it only says how many fixed steps are owed).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ticker {
    /// When the next tick falls due, in the caller's own nanoseconds. `None`
    /// until the first reading, which is what starts the clock — so the first
    /// call always owes exactly one tick and the game begins at once.
    next: Option<u64>,
    ticks: u64,
    dropped: u64,
}

impl Ticker {
    pub fn new() -> Ticker {
        Ticker::default()
    }

    /// **How many ticks are owed at `now_ns`** — 0 while the deadline is in the
    /// future, 1 in the ordinary case, more when a wake came back late.
    pub fn due(&mut self, now_ns: u64) -> u32 {
        let next = *self.next.get_or_insert(now_ns);
        if now_ns < next {
            return 0;
        }
        let mut owed = (now_ns - next) / TICK_NS + 1;
        if owed > MAX_CATCH_UP as u64 {
            self.dropped += owed - MAX_CATCH_UP as u64;
            owed = MAX_CATCH_UP as u64;
            // The debt is written off: the next deadline is
            // a whole tick from *now*, not from a deadline in the past.
            self.next = Some(now_ns + TICK_NS);
        } else {
            self.next = Some(next + owed * TICK_NS);
        }
        self.ticks += owed;
        owed as u32
    }

    /// When the next tick falls due, for the loop's `WaitUntil`.
    pub fn next_ns(&self) -> Option<u64> {
        self.next
    }

    /// Ticks handed out since the clock started.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Ticks a stall cost, which a true clock would have run. Nothing reads it
    /// but a test; it exists so that the one place time is lost is countable.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every wake overshoots its deadline by `over_ns`, as a real one does.
    /// Answers the ticks run over `wall_ns` of wall clock.
    fn run(t: &mut Ticker, wall_ns: u64, over_ns: u64) -> u64 {
        let mut now = 0;
        while now <= wall_ns {
            t.due(now);
            now = match t.next_ns() {
                Some(next) => next + over_ns,
                None => now + TICK_NS,
            };
        }
        t.ticks()
    }

    /// **The measurement that produced the fix**, as a property: 400 µs of
    /// overshoot per wake — the middle of what a high-resolution waitable timer
    /// — and one minute of wall clock must still
    /// be 3,750 ticks.
    #[test]
    fn an_overshooting_wake_does_not_lose_the_time() {
        let mut t = Ticker::new();
        let minute = 60 * 1_000_000_000;
        let ticks = run(&mut t, minute, 400_000);
        let want = minute / TICK_NS;
        assert!(
            ticks.abs_diff(want) <= 1,
            "{ticks} ticks in a minute, wanted {want}"
        );
        assert_eq!(t.dropped(), 0, "and nothing was given up on");
    }

    /// **The ablation: the rule this replaced.** `next = now + TICK`, measured
    /// from after the wait, over `intro.smk`'s 131.5 s. It is here so that the
    /// test above is not vacuous — it must lose at least a second, which is
    /// what a player saw.
    #[test]
    fn the_old_rule_loses_a_second_over_the_intro() {
        let over_ns = 400_000;
        let intro_ns = 131_495_000_000u64;
        let (mut now, mut next, mut ticks) = (0u64, 0u64, 0u64);
        while now <= intro_ns {
            if now >= next {
                next = now + TICK_NS; // the defect: measured from *now*
                ticks += 1;
            }
            now = next + over_ns;
        }
        let true_ticks = intro_ns / TICK_NS;
        let behind_ms = (true_ticks - ticks) * TICK_NS / 1_000_000;
        assert!(
            behind_ms >= 1_000,
            "the old rule was only {behind_ms} ms behind over the intro"
        );

        let mut t = Ticker::new();
        let fixed = run(&mut t, intro_ns, over_ns);
        assert!(fixed.abs_diff(true_ticks) <= 1, "and the new one is not");
    }

    /// A wake that comes back a whole second late runs [`MAX_CATCH_UP`] ticks,
    /// not sixty-two, and says how many it gave up.
    #[test]
    fn a_stall_is_capped_rather_than_fast_forwarded() {
        let mut t = Ticker::new();
        assert_eq!(t.due(0), 1, "the first reading starts the clock");
        assert_eq!(t.due(1_000_000_000), MAX_CATCH_UP);
        // A second's worth of ticks were owed from the deadline one tick in,
        // and eight of them were run.
        let owed = (1_000_000_000 - TICK_NS) / TICK_NS + 1;
        assert_eq!((owed, t.dropped()), (62, owed - MAX_CATCH_UP as u64));
        // And the clock restarts from the stall.
        assert_eq!(t.due(1_000_000_000), 0);
        assert_eq!(t.due(1_000_000_000 + TICK_NS), 1);
    }

    /// Nothing is owed before the deadline, and exactly one tick at it.
    #[test]
    fn a_tick_is_owed_once_per_period() {
        let mut t = Ticker::new();
        assert_eq!(t.due(0), 1);
        assert_eq!(t.due(TICK_NS - 1), 0);
        assert_eq!(t.due(TICK_NS), 1);
        assert_eq!(t.due(TICK_NS * 2 + TICK_NS / 2), 1, "half a tick late is still one");
        assert_eq!(t.due(TICK_NS * 3), 1, "and the half is repaid, not carried");
        assert_eq!(t.ticks(), 4);
    }
}
