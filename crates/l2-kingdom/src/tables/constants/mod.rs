
mod demographics;
pub use demographics::*;
mod agriculture;
pub use agriculture::*;
mod castle_and_tax;
pub use castle_and_tax::*;
mod jobs_and_goods;
pub use jobs_and_goods::*;
mod ai_and_grants;
pub use ai_and_grants::*;
mod military_and_movement;
pub use military_and_movement::*;
mod scoring;
pub use scoring::*;

use super::*;

/// `g_dairyPerHead` (`0x00553F60`) - the standing herd feeds five people per
/// head per season for free, without being slaughtered. `docs/kingdom.md`
/// §4.3, corroborated by a strategy guide and by a player's measurement of
/// 80 cows feeding 400 people.
pub const DAIRY_PER_HEAD: i32 = 5;

/// `g_foodPerHead` (`docs/kingdom.md` §4.3) - one slaughtered animal feeds ten.
pub const FOOD_PER_HEAD: i32 = 10;

/// `g_foodPerSack` (`docs/kingdom.md` §4.3) - one sack of grain feeds six.
pub const FOOD_PER_SACK: i32 = 6;

/// `g_grainYieldPerSack` (`0x0057C8E0`) = 12. Confirmed verbatim by the game's
/// own FAQ page, `L2.eng` group 292 index 4: *"Each sack planted will grow into
/// 12 sacks"*. `docs/kingdom.md` §7.1.
pub const GRAIN_YIELD_PER_SACK: i32 = 12;

/// `g_grainMaxSacksPerField` (`0x00552FFC`) = 10.
///
/// **The printed manual says 5, twice
/// `docs/decisions.md` C10 and `docs/kingdom.md` §7.1 and §11. Two independent
/// players measuring their own saves reported 6 fields sowing 60 sacks and 9
/// fields sowing 90.
pub const GRAIN_MAX_SACKS_PER_FIELD: i32 = 10;

/// The labour divisor in `Grain_Sow`'s "can this many sacks be tended?" test:
/// 5 with *Advanced Farming* on, 2 with it off. `docs/kingdom.md` §7.1.
///
/// Note the direction: the *smaller* divisor demands *more* labour, so turning
/// Advanced Farming off makes sowing harder, not easier.
pub const GRAIN_LABOUR_DIVISOR_ADVANCED: i32 = 5;
pub const GRAIN_LABOUR_DIVISOR_BASIC: i32 = 2;

/// `Grain_Grow` (`0x0044D15A`) and `Grain_Harvest` (`0x0044D1E5`) cap the crop
/// at `labour * this`, and their multiplier is **not** the sowing divisor.
///
/// `Rules_InitConstants` (`0x004983B7`) writes four numbers where this crate
/// had two: `g_grainLabourDivisorAdv = 5` (`0x005533BC`), `0x00553538 = 10`,
/// `0x00553218 = 3` and `g_grainLabourDivisor = 2` (`0x0057D34C`). Each of the
/// three grain steps picks between *its own* advanced value and the **same**
/// basic value, so with *Advanced Farming* off all three read
/// [`GRAIN_LABOUR_DIVISOR_BASIC`] — once as a divisor and twice as a
/// multiplier. That coincidence is the original's, and it is why there are two
/// new constants here and not four. **`[V]`**
pub const GRAIN_GROW_PER_WORKER_ADVANCED: i32 = 10;

/// …and the harvest's, which is far tighter: **three sacks a reaper**, out of a
/// workforce `Grain_Harvest` has already halved. `docs/kingdom.md` §7.1.
pub const GRAIN_HARVEST_PER_WORKER_ADVANCED: i32 = 3;

/// A field is fully reclaimed at 800, and `Field_ReclaimTick` (`0x0044C093`)
/// moves it by at most 200 a season. 200/800 is exactly the manual's *"you
/// will never be able to reclaim more than a quarter of a field in a single
/// season"*. `docs/kingdom.md` §7.2.
pub const FIELD_PROGRESS_MAX: i32 = 800;
pub const FIELD_RECLAIM_PER_SEASON: i32 = 200;

/// The flat efficiency every industry runs at when *Advanced Farming* is off.
///
/// **`[V]`** - `FUN_0044F248`'s first line is
/// `if (g_optAdvancedFarming == 0) return 80;`. The 15% and 20% bases, and the
/// whole ramp, only exist in an Advanced Farming game. Nothing in
/// `docs/kingdom.md` §7.4 says so, and the England turn-one fixture has the option **off**,
/// so the FAQ's *"30 serfs working at 15% efficiency"* describes the advanced
/// game only.
pub const EFFICIENCY_WITHOUT_ADVANCED_FARMING: i32 = 80;

/// The efficiency ceiling in `FUN_0044F248`.
///
/// **`[V]`, and in the instruction stream**: the ramp
/// ends `CMP dword ptr [ebp-0x0C], 0x64` / `MOV dword ptr [ebp-0x0C], 0x64` at
/// `0x0044F2E8`, and the flat 80 above is the `MOV EAX, 0x50` at `0x0044F278`.
/// `tools/oracle/kingdom.ps1` recovers both immediates out of `.text` — the
/// technique `docs/decisions.md` C16 established, applied to a rule rather
/// than to a `Rules_InitConstants` global.
pub const EFFICIENCY_MAX: i32 = 100;

/// What `FUN_0044EF4E` returns as "no limit" for an enabled non-weapon
/// industry. It is a literal 999,
/// with enough workers really is capped at 999 units a season.
pub const RESOURCE_LIMIT_UNLIMITED: i32 = 999;

// ---------------------------------------------------------------------------
// Wages
// ---------------------------------------------------------------------------

/// `Wages_ForUnit` (`0x004AD52B`): a human pays `men / 4`; an AI pays
/// `men / 3`, `men / 5` or `men / 10` by difficulty.
///
/// **Troop type does not enter it: a knight and a peasant cost the same.** A
/// player measured 250 men -> 62 crowns, 252 -> 63, 254 -> 63, across armies of
/// knights, of peasants and of mixed troops.
pub const WAGE_DIVISOR_HUMAN: i32 = 4;
pub const WAGE_DIVISOR_AI: [i32; 3] = [3, 5, 10];

/// The bankruptcy escalation in `Wages_PayAll` runs 0..=5. What each stage
/// *does* was not traced (`docs/kingdom.md` §2, `+0x158`, marked `[D]`).
pub const BANKRUPT_STAGE_MAX: u8 = 5;

/// The history ring `Season_Advance`'s second-to-last pass (`FUN_004AE7DD`)
/// writes: **400 seasons x 16 counties x `{i32 population, i8 happiness}`**.
///
/// **`[V]`, and the length invariant closes it.** Save block 10 is
/// `0x0056D8C0` for **51,200** bytes, and `400 * 16 * 8` is exactly 51,200.
/// The write is `[head << 7 + county * 8 + 0x0056D8B8]`, so county `k` lands at
/// slot `k - 1` of the entry — the eight-byte offset between the block's base
/// and the instruction's is what makes the 1-based county array fit a 0-based
/// ring without spilling.
///
/// `docs/kingdom.md` §3.4 names this pass and nothing more.
pub const HISTORY_SEASONS: usize = 400;

/// The ring stores 16 county slots per season, which is [`MAX_COUNTY_ID`]
/// counties — the loop runs `for (c = 1; c < 0x11; c++)`.
///
/// [`MAX_COUNTY_ID`]: crate::county::MAX_COUNTY_ID
pub const HISTORY_COUNTIES: usize = 16;

];

#[cfg(test)]
mod tests {
    use super::*;

    /// The health ladder's comparison sense, pinned by `docs/kingdom.md` §9:
    /// a starting meter of 65 must band as 2, and 67 must band as 3.
    #[test]
    fn the_health_ladder_is_inclusive_at_every_step() {
        assert_eq!(health_band(65), 2, "65 <= 65 is band 2 - the save says so");
        assert_eq!(health_band(66), 3);
        assert_eq!(health_band(67), 3, "the save's stored healthBand");
        assert_eq!(health_band(0), 0);
        assert_eq!(health_band(10), 0);
        assert_eq!(health_band(11), 1);
        assert_eq!(health_band(35), 1);
        assert_eq!(health_band(36), 2);
        assert_eq!(health_band(90), 3);
        assert_eq!(health_band(91), 4);
        assert_eq!(health_band(100), 4);
    }

    #[test]
    fn the_health_ladder_is_monotonic_over_its_whole_range() {
        let mut last = 0;
        for m in 0..=100 {
            let b = health_band(m);
            assert!(b >= last, "band went backwards at meter {m}");
            assert!(b < HEALTH_BAND_COUNT as u8);
            last = b;
        }
    }

    /// Every documented ration ratio, checked against its name.
    #[test]
    fn the_ration_table_matches_the_names_in_l2_eng_group_21() {
        let need = |level: usize, pop: i32| {
            let (d, m) = RATION_TABLE[level];
            crate::math::div_ceil(pop, d) * m
        };
        assert_eq!(need(0, 400), 0, "None");
        assert_eq!(need(1, 400), 100, "Quarter");
        assert_eq!(need(2, 400), 200, "Half");
        assert_eq!(need(3, 400), 400, "Normal");
        assert_eq!(need(4, 400), 800, "Double");
        assert_eq!(need(5, 400), 1200, "Triple");
    }

    /// The manual: *"A ration of Normal or above will improve happiness while
    /// half or quarter rations will decrease happiness"*. Normal is the first
/// positive row.
    #[test]
    fn normal_is_the_first_ration_level_worth_happiness() {
        for l in 0..3 {
            assert!(ration_happiness(l as i32) < 0, "level {l} should hurt");
        }
        for l in 3..RATION_LEVEL_COUNT {
            assert!(ration_happiness(l as i32) > 0, "level {l} should help");
        }
        assert_eq!(ration_happiness(3), 1);
        assert_eq!(ration_happiness(0), -8);
        assert_eq!(ration_happiness(5), 7);
    }

    /// The published FAQ, quoted in `docs/kingdom.md` §4.4: *"a county in good
    /// health (+1 happiness) with normal rations (+1) and 100 happiness can pay
    /// 7% taxes (-2) and remain at 100%. At perfect health, they can pay 8%."*
    ///
    /// This is the steady state of the whole layer in one assertion.
    #[test]
    fn the_break_even_tax_rate_is_seven_at_good_health_and_eight_at_perfect() {
        let steady = |band: usize, ration: i32, rate: i32| {
            (5 - rate) + HEALTH_HAPPINESS[band] + ration_happiness(ration)
        };
        assert_eq!(steady(3, 3, 7), 0, "Good health, Normal rations, 7%");
        assert_eq!(steady(4, 3, 8), 0, "Perfect health, Normal rations, 8%");
        assert!(steady(3, 3, 8) < 0, "8% is one too many at Good health");
        assert!(steady(4, 3, 9) < 0, "9% is one too many at Perfect health");
    }

    /// **Perfect health decays under anything less than Double rations** - the
    /// sign pattern in the delta table's last column.
    #[test]
    fn perfect_health_needs_double_rations_to_hold() {
        for level in 0..RATION_LEVEL_COUNT {
            let d = HEALTH_DELTA[level][4];
            if level < 4 {
                assert!(d < 0, "ration {level} should let Perfect decay, got {d}");
            } else {
                assert!(d >= 0, "ration {level} should hold Perfect, got {d}");
            }
        }
    }

    /// Recovering a starved county is fast at the bottom and slow at the top,
    /// in both directions: every row of the delta table is non-increasing.
    #[test]
    fn every_health_delta_row_is_non_increasing_across_the_bands() {
        for level in 0..RATION_LEVEL_COUNT {
            let row = HEALTH_DELTA[level];
            for b in 1..HEALTH_BAND_COUNT {
                assert!(
                    row[b] <= row[b - 1],
                    "ration {level} band {b}: {} should not exceed {}",
                    row[b],
                    row[b - 1]
                );
            }
        }
    }

    #[test]
    fn the_castle_tax_bases_are_the_published_bonus_percentages() {
        for t in 1..CASTLE_TYPE_COUNT {
            let bonus = CASTLE_TAX_BONUS_PCT[t - 1];
            assert_eq!(
                CASTLE_TAX_BASE[t] * 100,
                CASTLE_TAX_BASE[0] * (100 + bonus),
                "castle {t}: {} should be {}% above {}",
                CASTLE_TAX_BASE[t],
                bonus,
                CASTLE_TAX_BASE[0]
            );
        }
    }

    #[test]
    fn the_birth_ladder_is_a_decreasing_staircase() {
        for i in 1..BIRTH_RATE_LADDER.len() {
            assert!(BIRTH_RATE_LADDER[i].0 > BIRTH_RATE_LADDER[i - 1].0);
            assert!(BIRTH_RATE_LADDER[i].1 < BIRTH_RATE_LADDER[i - 1].1);
        }
        assert_eq!(birth_rate(1), 100);
        assert_eq!(birth_rate(40), 100);
        assert_eq!(birth_rate(41), 70);
        assert_eq!(birth_rate(417), 20, "docs/kingdom.md 9: pop 417 -> 20%");
        assert_eq!(birth_rate(500), 20);
        assert_eq!(birth_rate(501), 15);
        assert_eq!(birth_rate(2000), 2);
        assert_eq!(birth_rate(3000), 1);
        assert_eq!(birth_rate(50_000), 1, "above the last row the last row holds");
    }

    /// The soft cap players describe: at 2,000 people a Winter birth rate
    /// cannot keep up with a Winter death rate even at Perfect health.
    #[test]
    fn two_thousand_people_cannot_outbreed_winter() {
        let winter_deaths = DEATH_RATE_BY_HEALTH[4] + DEATH_RATE_BY_SEASON[Season::Winter as usize];
        assert_eq!(winter_deaths, 8);
        assert!(birth_rate(2000) < winter_deaths);
        assert!(birth_rate(500) > winter_deaths, "a small county still grows");
    }

    #[test]
    fn only_sunny_helps_the_herd_and_only_cloudy_is_neutral() {
        assert!(HERD_WEATHER_PCT[Weather::Sunny as usize] > 0);
        assert_eq!(HERD_WEATHER_PCT[Weather::Cloudy as usize], 0);
        for w in Weather::ALL {
            if w != Weather::Sunny && w != Weather::Cloudy {
                assert!(HERD_WEATHER_PCT[w as usize] < 0, "{} should hurt", w.name());
            }
        }
    }

    #[test]
    fn a_bow_costs_no_iron_and_armour_costs_the_most() {
        assert_eq!(WEAPON_COST[4], (13, 0), "bows need no iron");
        let dearest = WEAPON_COST.iter().map(|(w, i)| w + i).max().unwrap();
        assert_eq!(WEAPON_COST[5].0 + WEAPON_COST[5].1, dearest, "armour");
    }

    #[test]
    fn sheep_and_wool_are_the_two_goods_priced_zero() {
        let zeros: Vec<usize> =
            (1..GOOD_SELL_PRICE.len()).filter(|&i| GOOD_SELL_PRICE[i] == 0).collect();
        assert_eq!(zeros, vec![3, 5]);
        assert_eq!(GOOD_NAMES[3], "Sheep");
        assert_eq!(GOOD_NAMES[5], "Wool");
    }

    /// The human's `lord` byte is 0, and row 0 is all zeros.
    #[test]
    fn the_human_gets_no_gold_grant() {
        assert_eq!(AI_GOLD_GRANT[0], [0; 4]);
    }

    #[test]
    fn reclaiming_a_field_takes_exactly_four_seasons() {
        assert_eq!(FIELD_PROGRESS_MAX / FIELD_RECLAIM_PER_SEASON, 4);
    }
}

// ---------------------------------------------------------------------------
// The whole thing, as one value
// ---------------------------------------------------------------------------


