#![allow(unused_imports)]
use super::*;
use super::agriculture::*;
use super::castle_and_tax::*;
use super::jobs_and_goods::*;
use super::ai_and_grants::*;
use super::military_and_movement::*;
use super::scoring::*;
use super::*;

pub const EVENT_POPULATION_CAP_PCT: i32 = 20;

pub const EVENT_FIRST_YEAR: i32 = 1268;


pub const DEATH_RATE_BY_SEASON: [i32; 5] = [0, 4, 0, 2, 8];

pub const DRYNESS_BY_SEASON: [i32; 5] = [0, 8, 24, 12, -12];


/// The six ration levels, `L2.eng` group 21. `docs/kingdom.md` §4.2.
pub const RATION_LEVEL_COUNT: usize = 6;

/// `g_rationTable` (`0x004D6738`) - six `{divisor, multiplier}` pairs. The
/// requirement is `DivCeil(people, divisor) * multiplier`.
///
/// | level | pair | food needed | `L2.eng` group 21 |
/// |---:|---|---|---|
/// | 0 | (1, 0) | none    | None |
/// | 1 | (4, 1) | pop / 4 | Quarter |
/// | 2 | (2, 1) | pop / 2 | Half |
/// | 3 | (1, 1) | pop     | Normal |
/// | 4 | (1, 2) | pop x 2 | Double |
/// | 5 | (1, 3) | pop x 3 | Triple |
pub const RATION_TABLE: [(i32, i32); RATION_LEVEL_COUNT] =
    [(1, 0), (4, 1), (2, 1), (1, 1), (1, 2), (1, 3)];

pub const RATION_NAMES: [&str; RATION_LEVEL_COUNT] =
    ["None", "Quarter", "Half", "Normal", "Double", "Triple"];

#[inline]
pub const fn ration_happiness(level: i32) -> i32 {
    3 * level - 8
}

/// The five health bands, `L2.eng` group 20: `Diseased, Sick, Average, Good,
/// Perfect`.
pub const HEALTH_BAND_COUNT: usize = 5;

pub const HEALTH_BAND_NAMES: [&str; HEALTH_BAND_COUNT] =
    ["Diseased", "Sick", "Average", "Good", "Perfect"];

/// `g_healthDeltaTable` (`0x004D64A8`) - `int[6][5]`, indexed
/// `[rationLevel][healthBand]`, added to the health meter each season.
pub const HEALTH_DELTA: [[i32; HEALTH_BAND_COUNT]; RATION_LEVEL_COUNT] = [
    [-8, -10, -13, -16, -20], // None
    [-4, -6, -9, -12, -15],   // Quarter
    [-2, -4, -6, -8, -12],    // Half
    [8, 4, 2, 1, -1],         // Normal
    [12, 8, 4, 2, 0],         // Double
    [20, 12, 6, 3, 1],        // Triple
];

/// `g_healthBandLadder` (`0x004D6520`) - **five `{threshold, band}` pairs**,
/// `(10,0) (35,1) (65,2) (90,3) (100,4)`.
///
/// **`[V]` The pair layout is read out of the bytes, and `docs/kingdom.md`
/// §4.2's "else 4" is wrong**: the last case is an explicit `(100, 4)` row, not
/// a fallthrough. Two independent checks pin it. Five pairs of `int` is 40
/// bytes and `0x004D6520 + 40` is exactly `0x004D6548`, where
/// `g_healthHappiness` begins; and the interleaved 0,1,2,3,4 cannot be a
/// threshold array, because thresholds do not decrease.
pub const HEALTH_BAND_LADDER: [(i32, i32); HEALTH_BAND_COUNT] =
    [(10, 0), (35, 1), (65, 2), (90, 3), (100, 4)];

#[inline]
pub fn health_band(meter: i32) -> u8 {
    let mut i = 0;
    while i < HEALTH_BAND_LADDER.len() {
        if meter <= HEALTH_BAND_LADDER[i].0 {
            return HEALTH_BAND_LADDER[i].1 as u8;
        }
        i += 1;
    }
    HEALTH_BAND_LADDER[HEALTH_BAND_LADDER.len() - 1].1 as u8
}

/// `g_healthHappiness` (`0x004D6548`) - the happiness a health band is worth
/// per season, indexed by band 0..=4.
///
/// The array in the binary has **six** slots, the sixth zero: `0x004D6548 + 24`
/// is exactly `0x004D6560`, where `g_herdWeatherPct` begins. The same
/// five-used-plus-a-trailing-zero shape carries the three castle tables. Only
/// the five used slots are carried here.
pub const HEALTH_HAPPINESS: [i32; HEALTH_BAND_COUNT] = [-10, -5, 0, 1, 2];

pub const DEATH_RATE_BY_HEALTH: [i32; HEALTH_BAND_COUNT] = [35, 20, 8, 3, 0];


/// `g_birthRateLadder` (`0x004D6308`) - twenty `{population, percent}` pairs.
pub const BIRTH_RATE_LADDER: [(i32, i32); 20] = [
    (40, 100),
    (80, 70),
    (100, 50),
    (250, 30),
    (500, 20),
    (700, 15),
    (800, 14),
    (900, 13),
    (1000, 12),
    (1100, 11),
    (1200, 10),
    (1300, 9),
    (1400, 8),
    (1500, 7),
    (1600, 6),
    (1700, 5),
    (1800, 4),
    (1900, 3),
    (2000, 2),
    (3000, 1),
];

#[inline]
pub fn birth_rate(population: i32) -> i32 {
    let mut i = 0;
    while i < BIRTH_RATE_LADDER.len() {
        if population <= BIRTH_RATE_LADDER[i].0 {
            return BIRTH_RATE_LADDER[i].1;
        }
        i += 1;
    }
    BIRTH_RATE_LADDER[BIRTH_RATE_LADDER.len() - 1].1
}

#[inline]
pub fn happiness_birth_factor(happiness: i32) -> i32 {
    if happiness < 26 {
        25
    } else if happiness < 51 {
        50
    } else if happiness < 76 {
        75
    } else if happiness < 100 {
        100
    } else {
        120
    }
}

