//! `Widget_Test` (`0x0040DA1E`) walks a table of 24-byte records and reads a
//! kind at `+0x0F`. Kind **4** — every `+`/`−` in the game — fires on the
//! **press**, shows the pressed frame for three frames, and **auto-repeats with
//! acceleration** while the button stays down on it. Kind **5** — the yes/no
//! box's two gauntlets, the divide screen's confirm, diplomacy's send — fires
//! **twenty frames after the press**, with the pressed frame up the whole time.
//!
//! Here: the acceleration table, the schedule it produces, and the two state
//! machines. **Not** here: the artwork. The pressed frame is `base + 1` on the
//! same sheet — `Widget_Draw` (`0x0040CFD2`) adds one to the frame at `+0x04`
//! whenever the press timer at `+0x0D` is non-zero — so it is a fact about a
//! *sprite index*, and the painters own it. [`Press::is_pressed`] is what a
//! painter asks, one record at a time.

mod kind;
pub use kind::*;
mod press;
pub use press::*;

use crate::input::{Event, Rect};

pub const TICK_MS: u32 = 16;

/// **The auto-repeat gate table at `0x004D2748`**, read out of `Lords2.exe`.
pub const REPEAT_GATE: [u8; 48] = [
    8, 8, 8, 8, 8, 8, 8, 8, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0,
    0, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0,
];

pub const REPEAT_GATE_ADDR: u32 = 0x004D_2748;

pub const REPEAT_FIRST_STEP: u8 = 8;

pub const REPEAT_CLAMP: u8 = 0x2F;

/// **30 ms**, the gate `FUN_004B20ED` puts on the repeat counter.
///
/// It is a `timeGetTime()` difference tested against `0x1E`, evaluated once per
/// frame in the whole-game frame function and stored in `DAT_004EA128`, and the
/// timestamp is **not advanced** when the difference is under 30. So the
/// counter advances at most once per 30 ms and at most once per frame,
/// whichever is slower.
pub const REPEAT_STEP_MS: u32 = 30;

pub const PRESS_FRAMES: u8 = 3;

pub const DELAYED_FRAMES: u8 = 20;

/// **320 ms**, `Hotspot_Test`'s kind-2 repeat, which is a flat pulse and not a
/// ramp: `DAT_0057D3C8` is one of `Tick_Pulses`' eight dividers — every four of
/// the 80 ms pulses. The two repeats are different mechanisms and conflating
/// so [`Kind::Held`] is a separate
/// branch of [`Press::tick`].
pub const HELD_PULSE_MS: u32 = 320;

/// `arm!("0x00437AFB/divide-confirm", Delayed)` *is* `Kind::Delayed` — it
/// expands to nothing else — and it is also the `docs/arms.json` marker for
/// that arm. `crates/l2-game/tests/arms.rs` reads the id out of the first
/// argument and the gesture out of the second, through [`Kind::gesture`],
/// the word the marker claims and the kind the widget is answered with are
/// the same identifier and cannot drift apart.
#[macro_export]
macro_rules! arm {
    ($id:literal, $kind:ident $(,)?) => {
        $crate::press::Kind::$kind
    };
}

pub const MAX_WIDGETS: usize = 32;

/// First every kind-5 record whose countdown reached zero, in record order —
/// the loop at the top of `Widget_Test` (`0x0040DA1E`) walks the whole table
/// and **does not return after a fire**, so two can expire in one call. Then
/// the held record's repeat, which is the hit test below that loop.
#[must_use = "a fire that is not run is a button that did nothing"]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fired {
    delayed: u32,
    held: Option<usize>,
}

impl Iterator for Fired {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.delayed != 0 {
            let i = self.delayed.trailing_zeros() as usize;
            self.delayed &= self.delayed - 1;
            return Some(i);
        }
        self.held.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repeat_accelerates_on_the_schedule_the_table_encodes() {
        let firing: Vec<u8> = (0..=REPEAT_CLAMP).filter(|&s| fires_on_step(s)).collect();
        assert_eq!(
            firing,
            vec![8, 14, 19, 23, 26, 29, 32, 34, 36, 39, 40, 41, 42, 43, 44, 45, 46],
            "the ramp"
        );
        assert!((0..8).all(|s| !fires_on_step(s)));
        assert!((REPEAT_CLAMP + 1..=u8::MAX).all(fires_on_step));
    }

    #[test]
    fn a_press_and_release_fires_exactly_once() {
        let mut p = Press::new();
        assert!(p.press(3));
        p.release();
        assert_eq!((0..200).map(|_| p.tick().count()).sum::<usize>(), 0, "no repeat after the release");
    }

    #[test]
    fn holding_a_button_repeats_slowly_then_quickly() {
        let mut p = Press::new();
        assert!(p.press(3));
        let mut fires = Vec::new();
        for t in 1..=120 {
            if p.tick().any(|w| w == 3) {
                fires.push(t);
            }
        }
        assert_eq!(fires[0], 16, "the first repeat is step 8, and a step is two ticks");
        assert_eq!(&fires[..4], &[16, 28, 38, 46], "steps 8, 14, 19, 23");
        let tail = &fires[fires.len() - 5..];
        assert!(
            tail.windows(2).all(|w| w[1] - w[0] == 2),
            "past the clamp every step fires: {tail:?}",
        );
    }

    #[test]
    fn sliding_off_the_button_stops_it_repeating() {
        let mut p = Press::new();
        p.press(1);
        for _ in 0..40 {
            let _ = p.tick();
        }
        p.pointer(None);
        assert_eq!((0..200).map(|_| p.tick().count()).sum::<usize>(), 0);
    }

    /// **Ablation:** drop the `Kind::Repeat` arm of [`Press::pointer`] and
    /// widget 2 never fires. `Widget_Test` reaches its kind-4 arm only from the
    /// loop that hit-tests the record under the pointer (`00400000.c:7513`),
    /// and the ramp starts again because the countdown loop zeroes `+0x0E` of
    /// every record whose `+0x0D` is `0` (`00400000.c:7486-7488`).
    #[test]
    fn dragging_onto_another_repeat_button_repeats_that_one() {
        let mut p = Press::new();
        p.press(1);
        for _ in 0..40 {
            let _ = p.tick();
        }
        p.pointer(Some((2, Kind::Repeat)));
        let mut fired = Vec::new();
        for t in 1..=40 {
            fired.extend(p.tick().map(|w| (t, w)));
        }
        assert_eq!(fired[0], (16, 2), "step 8 of a ramp that started at the drag");
        assert!(fired.iter().all(|&(_, w)| w == 2), "and only the record under the pointer");
    }

    /// The 320 ms count is `DAT_0058FEB0`, a `Tick_Pulses` divider zeroed at
    /// the press (`00400000.c:7882`) and nowhere else.
    ///
    /// **Ablation:** zero `since_step` in [`Press::pointer`] for `Kind::Held`
    /// too and the pulse lands twenty ticks late.
    #[test]
    fn a_kind_two_record_left_and_re_entered_rejoins_the_count() {
        let ticks = HELD_PULSE_MS / TICK_MS;
        let mut p = Press::new();
        p.press_held(0);
        for _ in 0..ticks / 2 {
            assert_eq!(p.tick().next(), None);
        }
        p.pointer(None);
        for _ in 0..ticks / 2 - 1 {
            assert_eq!(p.tick().next(), None, "off the record no pulse reaches a handler");
        }
        p.pointer(Some((0, Kind::Held)));
        assert_eq!(p.tick().collect::<Vec<_>>(), vec![0], "the press's own count, resumed");
    }

    /// `Hotspot_Test` tests the record under the pointer and `g_mouseLeftDown`
    /// (`00400000.c:7878-7891`); the release clears the down bit
    /// (`004b0000.c:2119`).
    #[test]
    fn dragging_onto_another_kind_two_record_pulses_that_one_until_the_release() {
        let ticks = HELD_PULSE_MS / TICK_MS;
        let mut p = Press::new();
        p.press_held(0);
        for _ in 0..5 {
            let _ = p.tick();
        }
        p.pointer(Some((1, Kind::Held)));
        let mut fired = Vec::new();
        for t in 6..=ticks {
            fired.extend(p.tick().map(|w| (t, w)));
        }
        assert_eq!(fired, vec![(ticks, 1)], "the divider's count, the pointer's record");
        p.release();
        p.pointer(Some((1, Kind::Held)));
        assert_eq!((0..200).map(|_| p.tick().count()).sum::<usize>(), 0);
    }

    #[test]
    fn a_delayed_button_shows_the_pressed_frame_first_and_acts_afterwards() {
        let mut p = Press::new();
        p.press_delayed(0);
        assert!(p.is_pressed(0), "the gauntlet goes down at once");
        assert!(p.busy());
        for t in 1..DELAYED_FRAMES as u32 {
            assert_eq!(p.tick().next(), None, "nothing has happened yet at tick {t}");
            assert!(p.is_pressed(0), "and it is still down");
        }
        assert_eq!(p.tick().collect::<Vec<_>>(), vec![0], "the handler runs on the twentieth tick");
        assert!(!p.is_pressed(0), "and the button comes back up");
        assert!(!p.busy());
    }

    #[test]
    fn a_repeating_button_shows_the_pressed_frame_for_three_frames() {
        let mut p = Press::new();
        p.press(7);
        assert!(p.is_pressed(7));
        p.release();
        let _ = p.tick();
        let _ = p.tick();
        assert!(p.is_pressed(7), "still down two frames after the release");
        let _ = p.tick();
        assert!(!p.is_pressed(7), "and up on the third");
    }

    fn run(p: &mut Press, ticks: u32, presses: &[(u32, usize)]) -> Vec<(u32, usize)> {
        let mut out = Vec::new();
        for t in 1..=ticks {
            for &(at, w) in presses {
                if at == t {
                    p.press_delayed(w);
                }
            }
            out.extend(p.tick().map(|w| (t, w)));
        }
        out
    }

    /// **Two gauntlets pressed five ticks apart both act, each on its own
    /// twentieth tick** — `+0x0D` is a byte of each record, and the countdown
    /// loop walks every record.
    ///
    /// **Ablation, run:** make `press_delayed` zero every other record's timer
    /// — the one-pending-press model this replaced — and this goes red with
    /// only the second widget firing.
    #[test]
    fn two_delayed_presses_five_ticks_apart_both_fire_each_on_its_own_twentieth_tick() {
        let mut p = Press::new();
        let fired = run(&mut p, 40, &[(1, 0), (6, 1)]);
        assert_eq!(fired, vec![(20, 0), (25, 1)]);
    }

    #[test]
    fn two_waiting_gauntlets_are_both_drawn_down() {
        let mut p = Press::new();
        p.press_delayed(0);
        for _ in 0..5 {
            let _ = p.tick();
        }
        p.press_delayed(1);
        assert!(p.is_pressed(0) && p.is_pressed(1));
    }

    #[test]
    fn the_same_delayed_widget_pressed_twice_restarts_and_acts_once() {
        let mut p = Press::new();
        let fired = run(&mut p, 60, &[(1, 2), (8, 2)]);
        assert_eq!(fired, vec![(27, 2)]);
    }

    #[test]
    fn a_kind_four_press_does_not_cancel_a_waiting_kind_five_press() {
        let mut p = Press::new();
        p.press_delayed(6);
        let _ = p.tick();
        assert!(p.press(0), "the spinner fires on its press");
        p.release();
        let fired: Vec<usize> = (0..40).flat_map(|_| p.tick().collect::<Vec<_>>()).collect();
        assert_eq!(fired, vec![6], "and the thumb still acts");
    }

    #[test]
    fn a_countdown_and_a_repeat_on_one_tick_both_fire_countdown_first() {
        let mut p = Press::new();
        let mut on = Vec::new();
        for t in 1..=20u32 {
            if t == 1 {
                p.press_delayed(5);
            }
            if t == 5 {
                assert!(p.press(0));
            }
            on.push(p.tick().collect::<Vec<_>>());
        }
        assert_eq!(on[19], vec![5, 0], "tick 20: {:?}", on);
        assert!(on[..19].iter().all(Vec::is_empty), "{on:?}");
    }

    /// **`DAT_00591554` is 0 on the press and the clamped counter on each repeat
    /// fire** — the value the trade arrows read to decide between one and ten.
    #[test]
    fn the_published_repeat_step_is_zero_on_the_press_and_the_counter_on_a_repeat() {
        let mut p = Press::new();
        p.press(0);
        assert_eq!(p.repeat_step(), 0, "published as 0 before the press's call");
        let mut seen = Vec::new();
        for _ in 0..200 {
            if p.tick().next().is_some() {
                seen.push(p.repeat_step());
            }
        }
        assert_eq!(seen.first(), Some(&8), "the first repeat is step 8");
        assert_eq!(seen.last(), Some(&0x2F), "and the counter clamps at 0x2F");
        p.release();
        assert_eq!(p.repeat_step(), 0);
    }

    #[test]
    #[should_panic(expected = "MAX_WIDGETS")]
    fn a_widget_past_the_bound_is_refused() {
        Press::new().press_delayed(MAX_WIDGETS);
    }
}

