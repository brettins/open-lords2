#![allow(unused_imports)]
use super::*;
use super::logic::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

/// `Map_ScrollThrottle` (`0x004320D1`) in whole simulation ticks.
pub fn scroll_interval_ticks(speed: i32) -> u32 {
    pub(crate) const TICK_MS: u32 = 16;
    let q = (100 - speed.clamp(0, 100)) / 10;
    if q >= 10 {
        return u32::MAX;
    }
    let ms = (q * 12 + 2) as u32;
    ((ms + TICK_MS / 2) / TICK_MS).max(1)
}

pub(crate) fn group_slot(digit: u8) -> Option<usize> {
    if (b'1'..=b'9').contains(&digit) {
        Some((digit - b'1') as usize)
    } else {
        None
    }
}


