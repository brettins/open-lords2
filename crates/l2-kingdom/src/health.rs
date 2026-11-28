
use crate::county::County;
use crate::math::clamp;
use crate::tables::Tables;
use l2_net::{Quirk, Quirks};

pub const HEALTH_METER_MIN: i32 = 0;
pub const HEALTH_METER_MAX: i32 = 100;

pub fn delta(t: &Tables, ration_level: usize, band: u8) -> i32 {
    let row = &t.ration[ration_level.min(t.ration.len() - 1)].health_delta;
    row[(band as usize).min(row.len() - 1)]
}

pub fn happiness(t: &Tables, band: u8) -> i32 {
    t.health[(band as usize).min(t.health.len() - 1)].happiness
}

/// `Health_UpdateAll` (`0x0044BD9F`). `ai` is `g_realms[owner].isHuman == 0`.
pub fn update(t: &Tables, county: &mut County, ai: bool, quirks: Quirks) {
    let d = delta(t, county.ration_index(), county.health_band);
    county.health_meter = clamp(county.health_meter + d, HEALTH_METER_MIN, HEALTH_METER_MAX);
    // 00440000.c:8143 `if (population < 1 && isHuman == 0) healthMeter = 0x32;`
    if county.population < 1 && ai {
        county.health_meter = EMPTY_AI_COUNTY_METER;
    }
    county.health_band = band(t, county.health_meter, quirks);
    county.d_hap_health = happiness(t, county.health_band);
}

pub const EMPTY_AI_COUNTY_METER: i32 = 0x32;

/// B274: `Table_Lookup` gives 100 the fallback band 3; fixed, 100 is Perfect.
pub fn band(t: &Tables, meter: i32, quirks: Quirks) -> u8 {
    if meter >= HEALTH_METER_MAX && quirks.is_fixed(Quirk::FullHealthBandsGood) {
        return (t.health_band_ladder.len() - 1) as u8;
    }
    t.health_band(meter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::{health_band, NEW_GAME_HEALTH_BAND};

    const T: &Tables = &Tables::DEFAULT;
    const F: Quirks = Quirks::FAITHFUL;

    fn county(meter: i32, band: u8, ration: i32) -> County {
        let mut c = County::new();
        c.population = 100;
        (c.health_meter, c.health_band, c.ration_achieved) = (meter, band, ration);
        c
    }

    fn run(c: &mut County, seasons: usize, quirks: Quirks) {
        for _ in 0..seasons {
            update(T, c, false, quirks);
        }
    }

    /// C273: 65 -> 67 is band 2's +2 because a new game stores band 2, not because 65 bands 2.
    #[test]
    fn the_health_chain_from_the_shipped_save_reproduces() {
        assert_eq!(health_band(65), 3, "Table_Lookup is strict: 65 is Good");
        let mut c = county(65, NEW_GAME_HEALTH_BAND, 3);
        run(&mut c, 1, F);
        assert_eq!((c.health_meter, c.health_band, c.d_hap_health), (67, 3, 1));
    }

    /// C273. Ablation: `x < pairs[i].0` -> `<=` in `table_lookup`.
    #[test]
    fn every_threshold_belongs_to_the_band_above_it() {
        let got: Vec<u8> = [9, 10, 34, 35, 64, 65, 89, 90, 99, 100].map(health_band).into();
        assert_eq!(got, [0, 1, 1, 2, 2, 3, 3, 4, 4, 3]);
    }

    /// B274. Ablation: `band` ignoring `FullHealthBandsGood`.
    #[test]
    fn b274_full_health_is_good_unless_fixed() {
        let mut fixed = Quirks::FAITHFUL;
        fixed.set_reproduced(Quirk::FullHealthBandsGood, false);
        let mut c = county(99, 4, 5);
        run(&mut c, 1, F);
        assert_eq!((c.health_meter, c.health_band, c.d_hap_health), (100, 3, 1));
        let mut c = county(99, 4, 5);
        run(&mut c, 1, fixed);
        assert_eq!((c.health_meter, c.health_band, c.d_hap_health), (100, 4, 2));
    }

    /// B274: band 3 is +1/+2/+3 on Normal/Double/Triple, so 100 never leaves Good.
    #[test]
    fn b274_a_well_fed_county_at_full_health_stays_good() {
        for ration in 3..=5 {
            let mut c = county(100, health_band(100), ration);
            run(&mut c, 40, F);
            assert_eq!((c.health_meter, c.health_band), (100, 3), "ration {ration}");
        }
    }

    #[test]
    fn the_meter_is_clamped_at_both_ends() {
        let mut c = county(2, 0, 0);
        run(&mut c, 1, F);
        assert_eq!((c.health_meter, c.health_band), (0, 0));
        let mut c = county(100, 4, 5);
        run(&mut c, 1, F);
        assert_eq!(c.health_meter, 100);
    }

    /// 00440000.c:8143. Ablation: drop the `EMPTY_AI_COUNTY_METER` reset.
    #[test]
    fn an_empty_ai_county_resets_to_fifty_and_an_empty_human_one_does_not() {
        let mut c = county(0, 0, 0);
        c.population = 0;
        update(T, &mut c, true, F);
        assert_eq!((c.health_meter, c.health_band), (50, 2));
        let mut c = county(0, 0, 0);
        c.population = 0;
        update(T, &mut c, false, F);
        assert_eq!(c.health_meter, 0);
    }

    #[test]
    fn perfect_health_slips_on_normal_rations_and_holds_on_double() {
        let after = |ration: i32| {
            let mut c = county(100, 4, ration);
            run(&mut c, 1, F);
            c.health_meter
        };
        assert_eq!((after(3), after(4), after(5)), (99, 100, 100));
    }

    #[test]
    fn a_diseased_county_on_triple_rations_recovers_fast_then_slowly() {
        let mut c = county(0, 0, 5);
        run(&mut c, 1, F);
        assert_eq!(c.health_meter, 20, "the first season is worth 20");
        let mut seasons = 1;
        while c.health_band < 4 && seasons < 100 {
            run(&mut c, 1, F);
            seasons += 1;
        }
        assert_eq!((c.health_band, seasons), (4, 15), "20, 32, 44, +6 to 68, +3 to 92");
    }

    #[test]
    fn a_county_on_no_rations_reaches_diseased_and_stays_there() {
        let mut c = county(100, 4, 0);
        run(&mut c, 40, F);
        assert_eq!((c.health_meter, c.health_band, c.d_hap_health), (0, 0, -10));
    }

    /// Band 3 is +1 and band 4 is -1 on Normal, so the meter rocks across 90.
    #[test]
    fn normal_rations_converge_on_the_good_perfect_boundary() {
        for start in 0..100 {
            let mut c = county(start, health_band(start), 3);
            run(&mut c, 500, F);
            assert!((89..=90).contains(&c.health_meter), "from {start}: {}", c.health_meter);
        }
    }

    #[test]
    fn an_out_of_range_band_or_ration_clamps_rather_than_panicking() {
        assert_eq!(delta(T, 99, 99), crate::tables::HEALTH_DELTA[5][4]);
        assert_eq!(happiness(T, 99), crate::tables::HEALTH_HAPPINESS[4]);
    }
}
