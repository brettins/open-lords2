
use crate::county::County;
use crate::math::clamp;
use crate::tables::Tables;

pub const HEALTH_METER_MIN: i32 = 0;
pub const HEALTH_METER_MAX: i32 = 100;

pub fn delta(t: &Tables, ration_level: usize, band: u8) -> i32 {
    let row = &t.ration[ration_level.min(t.ration.len() - 1)].health_delta;
    row[(band as usize).min(row.len() - 1)]
}

pub fn happiness(t: &Tables, band: u8) -> i32 {
    t.health[(band as usize).min(t.health.len() - 1)].happiness
}

pub fn update(t: &Tables, county: &mut County) {
    let d = delta(t, county.ration_index(), county.health_band);
    county.health_meter = clamp(county.health_meter + d, HEALTH_METER_MIN, HEALTH_METER_MAX);
    county.health_band = t.health_band(county.health_meter);
    county.d_hap_health = happiness(t, county.health_band);
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &Tables = &Tables::DEFAULT;

    #[test]
    fn the_health_chain_from_the_shipped_save_reproduces() {
        let mut c = County::new();
        c.health_meter = 65;
        c.health_band = crate::tables::health_band(65);
        assert_eq!(c.health_band, 2, "65 is Average");
        c.ration_achieved = 3; // Normal

        update(T, &mut c);

        assert_eq!(c.health_meter, 67, "65 + 2");
        assert_eq!(c.health_band, 3, "Good");
        assert_eq!(c.d_hap_health, 1, "the save's shownHealth = +1");
    }

    #[test]
    fn the_meter_is_clamped_at_both_ends() {
        let mut c = County::new();
        c.health_meter = 2;
        c.health_band = 0;
        c.ration_achieved = 0; // None: -8 at band 0
        update(T, &mut c);
        assert_eq!(c.health_meter, 0);
        assert_eq!(c.health_band, 0);

        let mut c = County::new();
        c.health_meter = 99;
        c.health_band = 4;
        c.ration_achieved = 5; // Triple: +1 at band 4
        update(T, &mut c);
        assert_eq!(c.health_meter, 100);
        assert_eq!(c.d_hap_health, 2, "Perfect health is worth +2");
    }

    #[test]
    fn perfect_health_slips_on_normal_rations_and_holds_on_double() {
        let run = |ration: i32| {
            let mut c = County::new();
            c.health_meter = 100;
            c.health_band = 4;
            c.ration_achieved = ration;
            update(T, &mut c);
            c.health_meter
        };
        assert_eq!(run(3), 99, "Normal is -1 at Perfect");
        assert_eq!(run(4), 100, "Double holds");
        assert_eq!(run(5), 100, "Triple holds, clamped");
    }

    #[test]
    fn a_diseased_county_on_triple_rations_recovers_fast_then_slowly() {
        let mut c = County::new();
        c.health_meter = 0;
        c.health_band = 0;
        c.ration_achieved = 5;

        update(T, &mut c);
        assert_eq!(c.health_meter, 20, "the first season is worth 20");

        let mut seasons = 1;
        while c.health_band < 4 && seasons < 100 {
            update(T, &mut c);
            seasons += 1;
        }
        assert_eq!(c.health_band, 4);
        assert_eq!(seasons, 15, "20, 32, 44, then +6 a season, then +3 a season");
    }

    #[test]
    fn a_county_on_no_rations_reaches_diseased_and_stays_there() {
        let mut c = County::new();
        c.health_meter = 100;
        c.health_band = 4;
        c.ration_achieved = 0;
        for _ in 0..40 {
            update(T, &mut c);
        }
        assert_eq!(c.health_meter, 0);
        assert_eq!(c.health_band, 0);
        assert_eq!(c.d_hap_health, -10, "Diseased is -10 happiness a season");
    }

    #[test]
    fn normal_rations_converge_on_the_good_perfect_boundary() {
        for start in 0..=100 {
            let mut c = County::new();
            c.health_meter = start;
            c.health_band = crate::tables::health_band(start);
            c.ration_achieved = 3;
            for _ in 0..500 {
                update(T, &mut c);
            }
            assert!(
                (90..=91).contains(&c.health_meter),
                "starting from {start} settled at {}",
                c.health_meter
            );
            assert!((3..=4).contains(&c.health_band));
        }
    }

    #[test]
    fn an_out_of_range_band_or_ration_clamps_rather_than_panicking() {
        assert_eq!(delta(T, 99, 99), crate::tables::HEALTH_DELTA[5][4]);
        assert_eq!(happiness(T, 99), crate::tables::HEALTH_HAPPINESS[4]);
    }
}
