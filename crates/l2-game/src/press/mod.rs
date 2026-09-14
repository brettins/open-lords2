//! **Holding a button down**, which the original does in exactly one place and
//! we did in none.
//!
//! A player reported it as two separate things and they are one thing:
//!
//! > *"Clicking yes/no (gauntlet thumbs up and down) is instant, whereas the
//! > game waited on mouse-up, and the gauntlet would go down slightly when
//! > clicked."*
//! >
//! > *"Holding on a button doesn't seem to make it go up faster. I recall you
//! > could click an up arrow and after a few seconds the number would go up
//! > fast."*
//!
//! Both are the **kind byte** of the original's widget record, and
//! `docs/input.md` is the whole model. The short version, because this module
//! is one half of it:
//!
//! `Widget_Test` (`0x0040DA1E`) walks a table of 24-byte records and reads a
//! kind at `+0x0F`. Kind **4** — every `+`/`−` in the game — fires on the
//! **press**, shows the pressed frame for three frames, and **auto-repeats with
//! acceleration** while the button stays down on it. Kind **5** — the yes/no
//! box's two gauntlets, the divide screen's confirm, diplomacy's send — fires
//! **twenty frames after the press**, with the pressed frame up the whole time.
//! That delay is what reads as *"the game waited on mouse-up"*.
//!
//! # What is here and what is not
//!
//! Here: the acceleration table, the schedule it produces, and the two state
//! machines. **Not** here: the artwork. The pressed frame is `base + 1` on the
//! same sheet — `Widget_Draw` (`0x0040CFD2`) adds one to the frame at `+0x04`
//! whenever the press timer at `+0x0D` is non-zero — so it is a fact about a
//! *sprite index*, and the painters own it. [`Press::is_pressed`] is what a
//! painter asks, one record at a time.
//!
//! # Determinism
//!
//! `docs/netcode.md`: an auto-repeat is a timer, and a timer that feeds the
//! simulation is a lockstep surface. **This one does not feed it.** It sits
//! entirely on the input side: it consumes ticks and produces *discrete fires*,
//! and a fire becomes an ordinary command. Two peers
//! running at different frame rates therefore produce different *numbers* of
//! commands, which is correct — a player who holds an arrow longer steps the
//! number further — and never a different *result* from the same commands.
//!
//! Nothing here reads a clock. It counts [`TICK_MS`] ticks, because
//! `crate::input`'s rule is that no screen may be told how much time passed.

mod kind;
pub use kind::*;
mod press;
pub use press::*;

use crate::input::{Event, Rect};

/// One fixed simulation tick, in milliseconds.
///
/// Duplicated from `crate::battlefield` deliberately: this
/// module must not depend on the battle, and the number is the machine's, not
/// either module's. If they ever disagree the test below says so.
pub const TICK_MS: u32 = 16;

/// **The auto-repeat gate table at `0x004D2748`**, read out of `Lords2.exe`.
///
/// `Widget_Test`'s repeat is a **48-byte
/// hand-authored ramp**, indexed by how many 30 ms steps the button has been
/// held for, and the button fires on a step whose entry is non-zero:
///
/// ```text
/// idx:  0  1  2  3  4  5  6  7 | 8  9 10 11 12 13 14 15 16 17 18 19 20 21 22 23
/// val:  8  8  8  8  8  8  8  8 | 1  0  0  0  0  0  1  0  0  0  0  1  0  0  0  1
/// idx: 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47
/// val:  0  0  1  0  0  1  0  0  1  0  1  0  1  0  0  1  1  1  1  1  1  1  1  0
/// ```
///
/// **The first eight entries are never read** — the code returns before
/// touching the table while the counter is under 8 — and neither is entry 47,
/// because the counter clamps *to* 47 on the step that would have made it 48
/// and that branch fires without consulting the table at all. They are kept in
/// the constant because the constant is a copy of the bytes, and a copy with
/// the dead entries edited out is a copy somebody has already interpreted.
///
/// The wobble at 36 → 39 is in the game. It is not smoothed here.
pub const REPEAT_GATE: [u8; 48] = [
    8, 8, 8, 8, 8, 8, 8, 8, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0,
    0, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0,
];

/// Address of [`REPEAT_GATE`] in `Lords2.exe`, for the check that pins it.
pub const REPEAT_GATE_ADDR: u32 = 0x004D_2748;

/// Steps below this fire nothing: `if (rec[0x0E] < 8) return 0;`
pub const REPEAT_FIRST_STEP: u8 = 8;

/// The counter clamps here, and the clamped branch **skips the table** —
/// from this step onward the button fires on every step, which is the *"and
/// then it goes fast"* the player remembers.
pub const REPEAT_CLAMP: u8 = 0x2F;

/// **30 ms**, the gate `FUN_004B20ED` puts on the repeat counter.
///
/// It is a `timeGetTime()` difference tested against `0x1E`, evaluated once per
/// frame in the whole-game frame function and stored in `DAT_004EA128`, and the
/// timestamp is **not advanced** when the difference is under 30. So the
/// counter advances at most once per 30 ms and at most once per frame,
/// whichever is slower.
pub const REPEAT_STEP_MS: u32 = 30;

/// How long the pressed frame stays up for a kind-4 button after the press:
/// `rec[0x0D] = 3`, refreshed to 3 on every frame the button is still held over
/// it, so in practice it is *three frames after the release*.
pub const PRESS_FRAMES: u8 = 3;

/// **Twenty**, and this is the number the player felt. `Widget_Test`'s kind-5
/// branch sets `rec[0x0D] = 0x14` and returns **without calling the handler**;
/// the handler runs in the next call's countdown loop, on the frame the timer
/// reaches zero.
pub const DELAYED_FRAMES: u8 = 20;

/// **320 ms**, `Hotspot_Test`'s kind-2 repeat, which is a flat pulse and not a
/// ramp: `DAT_0057D3C8` is one of `Tick_Pulses`' eight dividers — every four of
/// the 80 ms pulses. The two repeats are different mechanisms and conflating
/// so [`Kind::Held`] is a separate
/// branch of [`Press::tick`].
pub const HELD_PULSE_MS: u32 = 320;

/// **The marker and the declaration, as one token.**
///
/// `arm!("0x00437AFB/divide-confirm", Delayed)` *is* `Kind::Delayed` — it
/// expands to nothing else — and it is also the `docs/arms.json` marker for
/// that arm. `crates/l2-game/tests/arms.rs` reads the id out of the first
/// argument and the gesture out of the second, through [`Kind::gesture`],
/// the word the marker claims and the kind the widget is answered with are
/// the same identifier and cannot drift apart.
///
/// That check used to compare a comment's word with a record's word and never
/// look at the `Kind` beside it: every options row was once declared
/// `Kind::Press` under a `left-press-delayed` comment and `arms.rs` stayed
/// green. **So a comment marker may not name the three kinds only this module
/// answers** — `left-press-repeat`, `left-press-delayed`, `left-press-held` —
/// and `arms.rs` refuses one that does. A plain press or a release can still
/// be marked by comment, because hand-rolled tests answer those.
#[macro_export]
macro_rules! arm {
    ($id:literal, $kind:ident $(,)?) => {
        $crate::press::Kind::$kind
    };
}

/// **How many records one [`Press`] can time.** An index into a screen's
/// table must be below this.
///
/// The largest table a screen of ours hands to [`Press`] is the army-division
/// screen's eighteen (`g_splitWidgets`). The bound is a bitmask's width
/// ([`Fired`]), and it is asserted on every press,
/// bigger table fails its first test
pub const MAX_WIDGETS: usize = 32;

/// **The handlers one tick owes a screen**, in the order `Widget_Test` calls
/// them.
///
/// First every kind-5 record whose countdown reached zero, in record order —
/// the loop at the top of `Widget_Test` (`0x0040DA1E`) walks the whole table
/// and **does not return after a fire**, so two can expire in one call. Then
/// the held record's repeat, which is the hit test below that loop.
///
/// In the original two kind-5 records cannot expire on the same frame, because
/// the hit test returns on its first hit and so arms one record per frame. Ours
/// can receive two `Event::Click`s between ticks, and would lose the second if
/// this were an `Option`.
#[must_use = "a fire that is not run is a button that did nothing"]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fired {
    /// Bit `i` set: record `i`'s twenty frames ran out on this tick.
    delayed: u32,
    /// The held record's auto-repeat step or flat pulse.
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

    /// **The schedule, spelled out**, because the table is easy to copy and
    /// hard to read.
    ///
    /// These are the steps the button fires on, and they are the acceleration:
    /// six steps between the first two, then five, four, three, three, three,
    /// two, two, three, and then every step. At 30 ms a step that is a first
    /// repeat 240 ms after the press and full speed from 1.44 s — *"click an up
    /// arrow and after a few seconds the number would go up fast."*
    #[test]
    fn the_repeat_accelerates_on_the_schedule_the_table_encodes() {
        let firing: Vec<u8> = (0..=REPEAT_CLAMP).filter(|&s| fires_on_step(s)).collect();
        assert_eq!(
            firing,
            vec![8, 14, 19, 23, 26, 29, 32, 34, 36, 39, 40, 41, 42, 43, 44, 45, 46],
            "the ramp"
        );
        // Nothing at all for the first seven steps: 210 ms of holding before
        // the button does anything a second time.
        assert!((0..8).all(|s| !fires_on_step(s)));
        // And past the clamp, every step.
        assert!((REPEAT_CLAMP + 1..=u8::MAX).all(fires_on_step));
    }

    /// A press fires once and once only, however long you wait, unless the
    /// button is *held*.
    #[test]
    fn a_press_and_release_fires_exactly_once() {
        let mut p = Press::new();
        assert!(p.press(3));
        p.release();
        assert_eq!((0..200).map(|_| p.tick().count()).sum::<usize>(), 0, "no repeat after the release");
    }

    /// The whole gesture, in ticks, with the numbers a player would feel.
    #[test]
    fn holding_a_button_repeats_slowly_then_quickly() {
        let mut p = Press::new();
        assert!(p.press(3));
        // 16 ms ticks against a 30 ms step,
        let mut fires = Vec::new();
        for t in 1..=120 {
            if p.tick().any(|w| w == 3) {
                fires.push(t);
            }
        }
        assert_eq!(fires[0], 16, "the first repeat is step 8, and a step is two ticks");
        assert_eq!(&fires[..4], &[16, 28, 38, 46], "steps 8, 14, 19, 23");
        // By the end of the run it is firing every other tick, which is as fast
        // as a 30 ms gate can go.
        let tail = &fires[fires.len() - 5..];
        assert!(
            tail.windows(2).all(|w| w[1] - w[0] == 2),
            "past the clamp every step fires: {tail:?}",
        );
    }

    /// **The pointer leaving the button stops the repeat**, because the
/// original re-hit-tests every frame and stops matching.
    #[test]
    fn sliding_off_the_button_stops_it_repeating() {
        let mut p = Press::new();
        p.press(1);
        for _ in 0..40 {
            let _ = p.tick();
        }
        p.pointer(Some(2));
        assert_eq!((0..200).map(|_| p.tick().count()).sum::<usize>(), 0);
    }

    /// **Kind 5: the press does not fire, and twenty ticks later it does.**
    /// This is the gauntlet.
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

    /// A kind-4 press shows the pressed frame too — for three frames, not
    /// twenty — and it comes back up on its own after the release.
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

    /// The tick each widget fires on, over `ticks` ticks, with an optional
    /// press of a kind-5 widget injected before a given tick.
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
        // Pressed before ticks 1 and 6, so their twentieth ticks are 20 and 25.
        let fired = run(&mut p, 40, &[(1, 0), (6, 1)]);
        assert_eq!(fired, vec![(20, 0), (25, 1)]);
    }

    /// **Both are drawn down while both wait**, which is `Widget_Draw` reading
    /// each record's own timer.
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

    /// **The same gauntlet pressed twice acts once, twenty ticks after the
    /// second press**: the kind-5 arm writes `rec[0x0D] = 0x14` whatever the
    /// timer held.
    #[test]
    fn the_same_delayed_widget_pressed_twice_restarts_and_acts_once() {
        let mut p = Press::new();
        let fired = run(&mut p, 60, &[(1, 2), (8, 2)]);
        assert_eq!(fired, vec![(27, 2)]);
    }

    /// **A spinner pressed while a thumb is waiting does not cancel the thumb.**
    /// `Widget_Test`'s kind-4 arm writes its own record's bytes and no other.
    ///
    /// **Ablation, run:** zero `self.delayed` in `Press::press` and this goes
    /// red — the thumb never acts.
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

    /// **The countdown and the repeat can fall on one tick**, and both come
    /// back — the delayed fire first, because the countdown loop runs before the
    /// hit test.
    #[test]
    fn a_countdown_and_a_repeat_on_one_tick_both_fire_countdown_first() {
        // The thumb before tick 1 acts on tick 20. The spinner pressed before
        // tick 5 and held makes its first repeat on the sixteenth tick after
        // its press — step 8 of 30 ms steps at 16 ms a tick — which is also
        // tick 20.
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
    /// The two numbers are pinned from `Widget_Test`'s body, not from the
    /// constants: the first repeat is step 8 and the clamp writes `0x2F`.
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

    /// Every record past the table's end is refused, loudly.
    #[test]
    #[should_panic(expected = "MAX_WIDGETS")]
    fn a_widget_past_the_bound_is_refused() {
        Press::new().press_delayed(MAX_WIDGETS);
    }
}

