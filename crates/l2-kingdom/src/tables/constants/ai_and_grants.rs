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
///
/// Uniformly smaller than [`AI_GOLD_GRANT`], which reads the opposite way round
/// to what "help the loser" would suggest: a cornered AI is given *less*, not
/// more. The same direction as the population/herd/grain grant, which stops
/// entirely once a realm has five counties - the grants reward a realm that is
/// already doing well.
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

/// The AI's free population, herd and grain per county per season, as
/// `(population, herd, grain)` multipliers on the difficulty.
///
/// **`[V]`, and `docs/kingdom.md` §8.2 is wrong to state one tier.** It gives
/// *"`difficulty * 20` people, `difficulty * 5` head and `difficulty * 40`
/// sacks"* flatly. `AI_SetTaxRates` picks the tier from the realm's county
/// count (`+0x29`):
///
/// | realm counties | people | head | sacks |
/// |---|---:|---:|---:|
/// | 1 … 2 | `d * 20` | `d * 5` | `d * 40` |
/// | 3 … 4 | `d * 10` | `d * 2` | `d * 20` |
/// | 5 or more | **0** | **0** | **0** |
///
/// So the figures the document quotes are the *smallest* realm's tier, and a
/// realm with five counties gets no goods grant at all. A realm with zero
/// counties is gated out one level up and gets nothing either, gold included.
pub const AI_GRANT_TIERS: [(u8, i32, i32, i32); 2] = [(3, 20, 5, 40), (5, 10, 2, 20)];

/// The tier's per-difficulty multipliers for a realm holding `counties`
/// counties, or `(0, 0, 0)` above the last tier.
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

/// Kept for the documentation's own figures, which are the first tier's.
pub const AI_GRANT_POPULATION_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].1;
pub const AI_GRANT_HERD_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].2;
pub const AI_GRANT_GRAIN_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].3;

/// The grant is gated on the county *already having some*, so it compounds
///.
pub const AI_GRANT_MIN_POPULATION: i32 = 20;
pub const AI_GRANT_MIN_HERD: i32 = 10;
pub const AI_GRANT_MIN_GRAIN: i32 = 50;

