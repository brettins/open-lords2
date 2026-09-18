//! `App_WinMain`'s message pump (`0x0040E9AB`) is **unthrottled**, `[V]`:
//!
//! `PeekMessageA`, and when the queue is empty `App_IdleFrame`
//! (`0x0040E8BB`) → `App_Draw` → `Battle_Frame`, straight round again with no
//! wait of any kind. The only `Sleep` in it is the 200 ms one taken when the
//! window is inactive. So **the original has no tick**: it draws as fast as the
//! machine allows and every paced thing in it reads a clock for itself.
//!
//! `Smk_PlayLoop` (`0x0042DBC7`) advances a film only when
//! `SmackWait` (ordinal 32, called at `0x0042DBF7`) answers 0, and
//! `_SmackWait@4` is a `timeGetTime` comparison — `[V]`: it is 320 bytes at
//! RVA `0x3170` of `Smackw32.dll` and calls `WINMM.dll!timeGetTime` at `+0xB0`,
//! the only import it calls. So the film's clock is **real milliseconds**,
//! while its sound track is played out by the device in real time as well. The
//! two agree by construction: measured over the install's 45 films, the track
//! runs `frames × period` long to within **1 ms over 131 s**
//! (`crates/l2-smk/tests/corpus/main.rs`), so "the audio buffer" and "the header's
//! frame rate" are the same clock and neither is the game's frame count.
//!
//! A tick that is not 16 ms therefore desynchronises the picture from the
//! sound, and only in one direction. `docs/decisions.md` C193.

use crate::TICK_MS;

pub const TICK_NS: u64 = TICK_MS as u64 * 1_000_000;

pub const MAX_CATCH_UP: u32 = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ticker {
    next: Option<u64>,
    pub(super) ticks: u64,
    dropped: u64,
}

impl Ticker {
    pub fn new() -> Ticker {
        Ticker::default()
    }

    pub fn due(&mut self, now_ns: u64) -> u32 {
        let next = *self.next.get_or_insert(now_ns);
        if now_ns < next {
            return 0;
        }
        let mut owed = (now_ns - next) / TICK_NS + 1;
        if owed > MAX_CATCH_UP as u64 {
            self.dropped += owed - MAX_CATCH_UP as u64;
            owed = MAX_CATCH_UP as u64;
            self.next = Some(now_ns + TICK_NS);
        } else {
            self.next = Some(next + owed * TICK_NS);
        }
        self.ticks += owed;
        owed as u32
    }

    pub fn next_ns(&self) -> Option<u64> {
        self.next
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_stall_is_capped_rather_than_fast_forwarded() {
        let mut t = Ticker::new();
        assert_eq!(t.due(0), 1, "the first reading starts the clock");
        assert_eq!(t.due(1_000_000_000), MAX_CATCH_UP);
        let owed = (1_000_000_000 - TICK_NS) / TICK_NS + 1;
        assert_eq!((owed, t.dropped()), (62, owed - MAX_CATCH_UP as u64));
        assert_eq!(t.due(1_000_000_000), 0);
        assert_eq!(t.due(1_000_000_000 + TICK_NS), 1);
    }

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
