#![allow(unused_imports)]
use super::*;
use super::demographics::*;
use super::agriculture::*;
use super::castle_and_tax::*;
use super::jobs_and_goods::*;
use super::military_and_movement::*;
use super::scoring::*;

/// `g_aiGoldGrantSmall` (`0x004DC230`) - the same shape, used instead when the
/// realm holds **fewer than three** counties. `[V]`
pub const AI_GOLD_GRANT_SMALL: [[i32; 4]; 5] = [
    [0, 0, 0, 0],         // lord 0 - the human
    [0, 160, 250, 400],   // lord 1
    [40, 180, 300, 500],  // lord 2
    [0, 160, 250, 400],   // lord 3
    [100, 240, 400, 600], // lord 4
];

/// A realm with fewer than this many counties draws from
/// [`AI_GOLD_GRANT_SMALL`]. `[V]` - `docs/kingdom.md` §8.2 states it.
pub const AI_GOLD_GRANT_SMALL_COUNTIES: u8 = 3;

/// **`[V]`, and `docs/kingdom.md` §8.2 is wrong to state one tier.** It gives
/// *"`difficulty * 20` people, `difficulty * 5` head and `difficulty * 40`
/// sacks"* flatly. `AI_SetTaxRates` picks the tier from the realm's county
/// count (`+0x29`):
pub const AI_GRANT_TIERS: [(u8, i32, i32, i32); 2] = [(3, 20, 5, 40), (5, 10, 2, 20)];

#[inline]
pub fn ai_grant_tier(counties: u8) -> (i32, i32, i32) {
    let mut i = 0;
    while i < AI_GRANT_TIERS.len() {
        let (below, p, h, g) = AI_GRANT_TIERS[i];
        if counties < below {
            return (p, h, g);
        }
        i += 1;
    }
    (0, 0, 0)
}

pub const AI_GRANT_POPULATION_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].1;
pub const AI_GRANT_HERD_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].2;
pub const AI_GRANT_GRAIN_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].3;

///.
pub const AI_GRANT_MIN_POPULATION: i32 = 20;
pub const AI_GRANT_MIN_HERD: i32 = 10;
pub const AI_GRANT_MIN_GRAIN: i32 = 50;

