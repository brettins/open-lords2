#![allow(unused_imports)]
use super::*;
use super::tests_part::*;
use crate::figure::{Figure, Side, State};

/// `+0x14` is reset to this whenever it reaches zero. **[D]**
pub const REFORM_INTERVAL: i16 = 500;

/// The countdown `+0x0C` is set to when a figure of the unit is hit. **[D]**
pub const HIT_MEMORY: u8 = 50;

/// The order lock `BattleUnit_Order` sets when it re-orders a unit that is
/// already in melee, buying it that many passes of immunity from
/// `BattleUnit_JoinMelee`. **[D]** `docs/battle-ai.md` §4.2.
pub const ORDER_LOCK: u8 = 64;

/// What `+0x14` is set to when a unit is **ordered**.
///
/// Three separate sites write it — `BattleUnit_Order` (`0x00479E90`),
/// `FUN_00478987`'s regroup, and both halves of the split it can perform — and
/// all three write the same twenty. So an ordered unit reforms almost at once
/// instead of waiting out the rest of its five hundred. **[V]**
pub const REFORM_ON_ORDER: i16 = 0x14;

/// Which dispatch category a troop type is given by `BattleUnit_Create`
/// (`0x00480662`), read from the eleven-way ladder at `0x00480743`. **[D]**
pub const CATEGORY_OF_TROOP: [u8; 11] = [2, 1, 3, 3, 2, 1, 4, 5, 6, 7, 8];

/// Chebyshev distance, the metric `Enemy_NearestUnit` uses. **[D]**
pub fn chebyshev(ax: i16, ay: i16, bx: i16, by: i16) -> i32 {
    let dx = (ax as i32 - bx as i32).abs();
    let dy = (ay as i32 - by as i32).abs();
    dx.max(dy)
}

/// `PctOf` (`0x00404DC1`): `a * 100 / b`, and **0 when `b` is 0**. **[D]**
pub fn pct_of(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        a * 100 / b
    }
}

