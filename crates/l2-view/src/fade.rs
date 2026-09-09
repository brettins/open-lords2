//! **`FUN_004B0CB4` — the end-of-turn screen fade.**
//!
//! The one visual effect in the game that is entirely in the palette. There is
//! no dither table and no 50 % blit anywhere on this path: the canvas of
//! indices is untouched and only the colours those indices resolve to move, so
//! this module produces a [`Palette`] and [`Canvas::to_rgba`] does the rest.
//!
//! [`Canvas::to_rgba`]: crate::Canvas::to_rgba
//!
//! # What was traced
//!
//! `FUN_004B0CB4(restoreScreen, rawFlag, palPtr)` has **exactly two call sites
//! in the whole binary, both on the turn boundary**:
//!
//! * `Turn_Tick`'s phase 7, with `rawFlag = 1`, immediately after
//!   `Season_Advance`;
//! * `FUN_0049A3E6`, with `rawFlag = 0`, after reloading the seasonal art.
//!
//! The two branches differ by a factor of four, so it is a fade **to one
//! quarter brightness and back**. The stepper at `0x004B0E03` moves each
//! channel by at most [`STEP`] every ~20 ms, over palette entries
//! [`FIRST`] … [`LAST`] **only** — which is why the chrome stays lit while the
//! map dims. A channel of 255 has 191 to travel, so a full fade is
//! [`STEPS`] steps ≈ 320 ms each way.
//!
//! Inferred, from where the two calls sit rather than from anything the
//! function says: the dark window is **cover for the seasonal art reload and
//! the autosave**, both of which run inside it.
//!
//! # The rate is ours, the shape is the original's
//!
//! The original steps on a `GetTickCount` interval. Nothing below `main.rs` may
//! read a clock (`docs/netcode.md`), so ours steps once per fixed 16 ms tick —
//! the same trade `MapScreen::flag_tick` and `VillageScreen::CLICK_SETTLE_TICKS`
//! already make, and the same one `docs/decisions.md` C49 records for the flag
//! wave. Sixteen steps at 16 ms is 256 ms rather than 320; the *sequence of
//! palettes* is the original's, the interval between them is ours.
//!
//! # Nothing here is simulation
//!
//! A fade is a reader of state that produces colour. It cannot be observed by
//! the simulation, it takes no argument from it, and a peer that skipped the
//! whole animation would compute the same turn — which is the property that
//! lets it be paced by a screen at all.

use l2_formats::Palette;

/// The first palette entry the fade touches.
pub const FIRST: usize = 10;

/// The last palette entry the fade touches, inclusive. Entries outside
/// `FIRST ..= LAST` are left at full brightness, and that is deliberate rather
/// than an optimisation: it is what keeps the interface chrome lit while the
/// map goes dark.
pub const LAST: usize = 245;

/// The most one channel moves in one step — `0x004B0E03`'s clamp.
pub const STEP: u8 = 12;

/// How many steps a full fade takes.
///
/// The brightest possible channel, 255, has `255 - 255/4 = 192` to travel, and
/// `192 / 12` is exactly 16. Every dimmer channel arrives sooner and then sits
/// on its target, which is what a *clamped* step means and why one constant
/// covers the whole palette.
pub const STEPS: u8 = 16;

/// How many steps the whole animation takes: down, then back up.
pub const PHASES: u8 = STEPS * 2;

/// Where a channel is heading on the way down: one quarter.
#[inline]
fn quarter(v: u8) -> u8 {
    v / 4
}

/// The palette at `phase` of the fade.
///
/// `0` and [`PHASES`] are both `full` exactly; [`STEPS`] is the bottom of the
/// dark window. Phases below `STEPS` are the descent and phases above it are
/// the climb, and they are **not** mirror images of each other — the original
/// steps *toward a target* rather than replaying a sequence backwards, so a
/// channel that reaches its target early sits there on the way down and leaves
/// late on the way up. Reproduced rather than smoothed, because it is one
/// `min`/`max` either way and a smoothed version would be a guess.
///
/// A phase past [`PHASES`] is clamped to it, so a caller that overruns gets the
/// full palette rather than a panic.
pub fn at(full: &Palette, phase: u8) -> Palette {
    let mut entries = *full.entries();
    for entry in entries.iter_mut().take(LAST + 1).skip(FIRST) {
        for channel in entry.iter_mut() {
            *channel = channel_at(*channel, phase);
        }
    }
    Palette::from_entries(entries)
}

/// One channel at one phase. See [`at`].
fn channel_at(v: u8, phase: u8) -> u8 {
    let dark = quarter(v);
    if phase <= STEPS {
        // Descending: move down by at most STEP a step, never past the target.
        v.saturating_sub(STEP.saturating_mul(phase)).max(dark)
    } else {
        // Climbing, from the target back to where it started.
        let s = (phase - STEPS).min(STEPS);
        dark.saturating_add(STEP.saturating_mul(s)).min(v)
    }
}

/// Whether `phase` is the bottom of the fade — the frame at which the original
/// reloads the seasonal art and writes the autosave, and therefore the frame at
/// which anything we want hidden has to happen.
#[inline]
pub fn is_darkest(phase: u8) -> bool {
    phase == STEPS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A palette whose every entry is the same triple, so a test can talk about
    /// "the value" without indexing.
    fn flat(v: u8) -> Palette {
        Palette::from_entries([[v, v, v]; 256])
    }

    #[test]
    fn phase_zero_and_the_last_phase_are_the_palette_itself() {
        let full = flat(255);
        for phase in [0, PHASES] {
            let p = at(&full, phase);
            for i in 0..=255u8 {
                assert_eq!(p.rgb(i), [255, 255, 255], "entry {i} at phase {phase}");
            }
        }
    }

    /// **The chrome stays lit.** This is the whole reason the range exists, and
    /// it is the assertion that would fail if somebody "simplified" the loop to
    /// the whole palette.
    #[test]
    fn only_entries_10_through_245_ever_move() {
        let full = flat(200);
        for phase in 0..=PHASES {
            let p = at(&full, phase);
            for i in 0..=255usize {
                let moved = p.rgb(i as u8) != [200, 200, 200];
                let in_range = (FIRST..=LAST).contains(&i);
                assert!(
                    !moved || in_range,
                    "entry {i} moved at phase {phase} and is outside {FIRST}..={LAST}",
                );
            }
        }
    }

    /// One quarter at the bottom, and it takes exactly [`STEPS`] to get there
    /// from the brightest channel there is.
    #[test]
    fn the_bottom_of_the_fade_is_one_quarter_brightness() {
        let p = at(&flat(255), STEPS);
        assert_eq!(p.rgb(128), [63, 63, 63], "255 / 4");
        // And not one step sooner: 255 - 12*15 = 75.
        assert_eq!(at(&flat(255), STEPS - 1).rgb(128), [75, 75, 75]);
    }

    /// No channel ever moves by more than [`STEP`] between two phases — the
    /// clamp at `0x004B0E03`, asserted over every starting value rather than a
    /// chosen one.
    #[test]
    fn no_channel_moves_by_more_than_twelve_in_one_step() {
        for v in 0..=255u8 {
            let mut prev = channel_at(v, 0);
            for phase in 1..=PHASES {
                let now = channel_at(v, phase);
                assert!(
                    prev.abs_diff(now) <= STEP,
                    "value {v} jumped {} between phase {} and {phase}",
                    prev.abs_diff(now),
                    phase - 1,
                );
                prev = now;
            }
        }
    }

    /// Monotone down then monotone up, for every value. A fade that brightened
    /// mid-descent would be visible and would mean the two branches had been
    /// mixed up.
    #[test]
    fn the_descent_never_brightens_and_the_climb_never_dims() {
        for v in 0..=255u8 {
            for phase in 1..=STEPS {
                assert!(channel_at(v, phase) <= channel_at(v, phase - 1), "value {v}");
            }
            for phase in STEPS + 1..=PHASES {
                assert!(channel_at(v, phase) >= channel_at(v, phase - 1), "value {v}");
            }
        }
    }
}
