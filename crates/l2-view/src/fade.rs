//! **`FUN_004B0CB4` — the end-of-turn screen fade.**
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
//! [`FIRST`] … [`LAST`] **only** —
//! map dims. A channel of 255 has 191 to travel,
//! [`STEPS`] steps ≈ 320 ms each way.

use l2_formats::Palette;

pub const FIRST: usize = 10;

pub const LAST: usize = 245;

/// The most one channel moves in one step — `0x004B0E03`'s clamp.
pub const STEP: u8 = 12;

pub const STEPS: u8 = 16;

pub const PHASES: u8 = STEPS * 2;

#[inline]
fn quarter(v: u8) -> u8 {
    v / 4
}

pub fn at(full: &Palette, phase: u8) -> Palette {
    let mut entries = *full.entries();
    for entry in entries.iter_mut().take(LAST + 1).skip(FIRST) {
        for channel in entry.iter_mut() {
            *channel = channel_at(*channel, phase);
        }
    }
    Palette::from_entries(entries)
}

fn channel_at(v: u8, phase: u8) -> u8 {
    let dark = quarter(v);
    if phase <= STEPS {
        v.saturating_sub(STEP.saturating_mul(phase)).max(dark)
    } else {
        let s = (phase - STEPS).min(STEPS);
        dark.saturating_add(STEP.saturating_mul(s)).min(v)
    }
}

#[inline]
pub fn is_darkest(phase: u8) -> bool {
    phase == STEPS
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_bottom_of_the_fade_is_one_quarter_brightness() {
        let p = at(&flat(255), STEPS);
        assert_eq!(p.rgb(128), [63, 63, 63], "255 / 4");
        assert_eq!(at(&flat(255), STEPS - 1).rgb(128), [75, 75, 75]);
    }

    /// No channel ever moves by more than [`STEP`] between two phases — the
    /// clamp at `0x004B0E03`, asserted over every starting value
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
