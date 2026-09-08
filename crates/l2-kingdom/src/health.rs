//! Health — `docs/kingdom.md` §4.2, `Health_UpdateAll`.
//!
//! One meter, one ladder, one table. Each season the meter moves by
//! `g_healthDeltaTable[rationLevel][healthBand]`, is clamped to 0..=100, and is
//! re-banded; the new band then produces the happiness term and the death rate.
//!
//! The ordering matters and is asserted in [`crate::phase`]: the delta is
//! indexed by the band the county had *before* the move, and the happiness term
//! reads the band it has *after*. `docs/kingdom.md` §9's chain is exactly that
//! — `65 -> band 2 -> +2 -> 67 -> band 3` — and it is the observation that pins
//! the ladder's comparison sense and the delta table's index order at once.

use crate::county::County;
use crate::math::clamp;
use crate::tables::{health_band, HEALTH_DELTA, HEALTH_HAPPINESS};

/// The meter's range.
pub const HEALTH_METER_MIN: i32 = 0;
pub const HEALTH_METER_MAX: i32 = 100;

/// The seasonal move for a ration level and the band the county is currently
/// in.
pub fn delta(ration_level: usize, band: u8) -> i32 {
    HEALTH_DELTA[ration_level.min(HEALTH_DELTA.len() - 1)]
        [(band as usize).min(HEALTH_DELTA[0].len() - 1)]
}

/// The happiness a band is worth per season.
pub fn happiness(band: u8) -> i32 {
    HEALTH_HAPPINESS[(band as usize).min(HEALTH_HAPPINESS.len() - 1)]
}

/// One county's health pass.
///
/// Reads [`County::ration_achieved`] — so [`crate::ration::apply`] must already
/// have run this season — and writes the meter, the band and the health
/// happiness term.
pub fn update(county: &mut County) {
    let d = delta(county.ration_index(), county.health_band);
    county.health_meter = clamp(county.health_meter + d, HEALTH_METER_MIN, HEALTH_METER_MAX);
    county.health_band = health_band(county.health_meter);
    county.d_hap_health = happiness(county.health_band);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`docs/kingdom.md` §9 point 7, the whole chain.** A published dump of
    /// the new-game presets gives a starting health of 65 for a medium county,
    /// which is band 2 on the ladder since `65 <= 65`;
    /// `g_healthDeltaTable[Normal][band 2]` is +2, giving 67; and 67 bands as 3
    /// since `67 <= 90`. The shipped save stores meter **67** and band **3**.
    #[test]
    fn the_health_chain_from_the_shipped_save_reproduces() {
        let mut c = County::new();
        c.health_meter = 65;
        c.health_band = crate::tables::health_band(65);
        assert_eq!(c.health_band, 2, "65 is Average");
        c.ration_achieved = 3; // Normal

        update(&mut c);

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
        update(&mut c);
        assert_eq!(c.health_meter, 0);
        assert_eq!(c.health_band, 0);

        let mut c = County::new();
        c.health_meter = 99;
        c.health_band = 4;
        c.ration_achieved = 5; // Triple: +1 at band 4
        update(&mut c);
        assert_eq!(c.health_meter, 100);
        assert_eq!(c.d_hap_health, 2, "Perfect health is worth +2");
    }

    /// **Perfect health decays under anything less than Double rations** — the
    /// single most quoted consequence of the delta table.
    #[test]
    fn perfect_health_slips_on_normal_rations_and_holds_on_double() {
        let run = |ration: i32| {
            let mut c = County::new();
            c.health_meter = 100;
            c.health_band = 4;
            c.ration_achieved = ration;
            update(&mut c);
            c.health_meter
        };
        assert_eq!(run(3), 99, "Normal is -1 at Perfect");
        assert_eq!(run(4), 100, "Double holds");
        assert_eq!(run(5), 100, "Triple holds, clamped");
    }

    /// Recovering a starved county is fast at the bottom and slow at the top:
    /// a Diseased county put back on Triple rations gains 20 in its first
    /// season and 3 a season for the last third of the climb.
    #[test]
    fn a_diseased_county_on_triple_rations_recovers_fast_then_slowly() {
        let mut c = County::new();
        c.health_meter = 0;
        c.health_band = 0;
        c.ration_achieved = 5;

        update(&mut c);
        assert_eq!(c.health_meter, 20, "the first season is worth 20");

        let mut seasons = 1;
        while c.health_band < 4 && seasons < 100 {
            update(&mut c);
            seasons += 1;
        }
        assert_eq!(c.health_band, 4);
        assert_eq!(seasons, 15, "20, 32, 44, then +6 a season, then +3 a season");
    }

    /// Starvation is a one-way street until the rations come back.
    #[test]
    fn a_county_on_no_rations_reaches_diseased_and_stays_there() {
        let mut c = County::new();
        c.health_meter = 100;
        c.health_band = 4;
        c.ration_achieved = 0;
        for _ in 0..40 {
            update(&mut c);
        }
        assert_eq!(c.health_meter, 0);
        assert_eq!(c.health_band, 0);
        assert_eq!(c.d_hap_health, -10, "Diseased is -10 happiness a season");
    }

    /// The steady state of the meter under Normal rations, from every possible
    /// starting value: it converges on the Good/Perfect boundary and then
    /// oscillates across it one point at a time, because `Normal` is +1 at band
    /// 3 and -1 at band 4. A county on Normal rations cannot *hold* Perfect
    /// health, and cannot fall below Good either.
    #[test]
    fn normal_rations_converge_on_the_good_perfect_boundary() {
        for start in 0..=100 {
            let mut c = County::new();
            c.health_meter = start;
            c.health_band = crate::tables::health_band(start);
            c.ration_achieved = 3;
            for _ in 0..500 {
                update(&mut c);
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
        assert_eq!(delta(99, 99), HEALTH_DELTA[5][4]);
        assert_eq!(happiness(99), HEALTH_HAPPINESS[4]);
    }
}
