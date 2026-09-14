#![allow(unused_imports)]
use super::*;
use super::logic::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

/// `Map_ScrollThrottle` (`0x004320D1`) in whole simulation ticks.
///
/// `((100 − speed) / 10) × 12 + 2` milliseconds, rounded to the nearest tick and
/// never below one; speed 0 never scrolls. Identical to `screens/map/mod.rs`'s,
/// without the `+= 2` that function adds for `g_screenId == 0x10`.
pub fn scroll_interval_ticks(speed: i32) -> u32 {
    pub(crate) const TICK_MS: u32 = 16;
    let q = (100 - speed.clamp(0, 100)) / 10;
    if q >= 10 {
        return u32::MAX;
    }
    let ms = (q * 12 + 2) as u32;
    ((ms + TICK_MS / 2) / TICK_MS).max(1)
}

/// Virtual keys `0x31` … `0x39` are groups 0 … 8. `0x30` is not one: the
/// original tests `0x30 < key && key < 0x3A`.
pub(crate) fn group_slot(digit: u8) -> Option<usize> {
    if (b'1'..=b'9').contains(&digit) {
        Some((digit - b'1') as usize)
    } else {
        None
    }
}


