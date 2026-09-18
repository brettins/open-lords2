//! `docs/decisions.md` C3 is the standing hazard: a table that *looks* like the
//! thing you were hoping for is not evidence. The roster is held in place by an
//! arithmetic invariant instead. `Mercenary_Init` (`0x004AC904`) copies six
//! parallel arrays into the live table, and they lie end to end with nothing
//! left over:
//!
//! | table | address | element | length |
//! |---|---|---|---|
//! | `g_mercMen` | `0x004DE760` | `i32` | 48 bytes |
//! | `g_mercTroopType` | `0x004DE790` | `u8` | 12 + 4 pad |
//! | `g_mercStartCounty` | `0x004DE7A0` | `u8` | 12 + 4 pad |
//! | `g_mercPeriod` | `0x004DE7B0` | `u8` | 12 + 4 pad |
//! | `g_mercPrice` | `0x004DE7C0` | `i32` | 48 bytes |
//! | `g_mercWage` | `0x004DE7F0` | `i32` | 48 bytes |
//! | `g_mercBandCount` | `0x004DE820` | `i32` | 20 entries |
//!
//! Three `i32` arrays of exactly 48 bytes and three `u8` arrays of exactly 12
//! bytes plus alignment, and all twelve pad bytes are zero. `[V]` — read out of
//! `Lords2.exe` twice, independently, byte for byte.
//!
//! **And the index is 1…12, with index 0 not a slot at all.** That is the C3
//! check, made explicitly: `Mercenary_Init`'s loop is `for (i = 1; i <=
//! bandsInPlay; i++)` and each base pointer is biased back one element, so
//! nothing ever reads index 0. Read anyway, the "index 0" values are spillover
//! from whatever sits before each array — `men[0]` is the tail of the preceding
//! `i32`, `wage[0]` is `price[12]` — which is what they should be if the
//! reading is right, and would not be if it were not.

mod bands;
pub use bands::*;

use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{Mercenaries, TroopType, UnitKind, Units};
use l2_net::{Quirk, Quirks};

pub const MERCENARY_BANDS: usize = 12;

pub const BAND_SLOTS: usize = MERCENARY_BANDS + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandRules {
    /// `L2.eng` group 16, which begins *"No mercenaries in the army."* and then
    /// names the twelve nationalities.
    pub nationality: &'static str,
    pub men: i32,
    pub troop: TroopType,
    pub price: i32,
    pub listed_wage: i32,
    pub start_county: u8,
    pub period: u8,
}

pub const ROSTER: [BandRules; BAND_SLOTS] = [
    BandRules { nationality: "-", men: 0, troop: TroopType::Peasant, price: 0, listed_wage: 0, start_county: 0, period: 0 },
    BandRules { nationality: "Scottish", men: 100, troop: TroopType::Pikeman, price: 1800, listed_wage: 180, start_county: 1, period: 5 },
    BandRules { nationality: "Irish", men: 200, troop: TroopType::Pikeman, price: 3500, listed_wage: 350, start_county: 2, period: 3 },
    BandRules { nationality: "Moorish", men: 150, troop: TroopType::Archer, price: 3000, listed_wage: 300, start_county: 4, period: 6 },
    BandRules { nationality: "Welsh", men: 200, troop: TroopType::Archer, price: 4000, listed_wage: 400, start_county: 5, period: 4 },
    BandRules { nationality: "Danish", men: 200, troop: TroopType::Swordsman, price: 5500, listed_wage: 550, start_county: 7, period: 2 },
    BandRules { nationality: "Swedish", men: 100, troop: TroopType::Swordsman, price: 2700, listed_wage: 270, start_county: 8, period: 7 },
    BandRules { nationality: "Flemish", men: 100, troop: TroopType::Crossbowman, price: 3000, listed_wage: 300, start_county: 10, period: 5 },
    BandRules { nationality: "Norman", men: 200, troop: TroopType::Crossbowman, price: 6000, listed_wage: 600, start_county: 11, period: 2 },
    BandRules { nationality: "Saxon", men: 150, troop: TroopType::Maceman, price: 1900, listed_wage: 190, start_county: 13, period: 1 },
    BandRules { nationality: "Burgundian", men: 250, troop: TroopType::Maceman, price: 3100, listed_wage: 310, start_county: 14, period: 3 },
    BandRules { nationality: "Spanish", men: 50, troop: TroopType::Knight, price: 2700, listed_wage: 270, start_county: 16, period: 6 },
    BandRules { nationality: "Angevin", men: 100, troop: TroopType::Knight, price: 5500, listed_wage: 550, start_county: 17, period: 7 },
];

/// `g_mercBandCount` (`0x004DE820`) — how many bands a map gets, indexed by its
/// county count.
///
/// `0x004DE820 + 20 * 4 = 0x004DE870`, which is where the string table begins
/// (`"ff_batl.wav"`). England has 14 counties, so all twelve bands exist there.
///
/// `[V]`
pub const BAND_COUNT_BY_COUNTIES: [i32; 20] =
    [0, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 12, 12, 12, 12, 12, 12];

/// > **`docs/armies.md` §5.2 and `docs/symbols.json` both say *"clamped to
/// > 1 … 12"*. That is wrong at both ends.** The code is
/// >
/// > ```c
/// > n = g_mercBandCount[g_countyCount];
/// > if (0xc < n) n = 1;      /* above twelve collapses to ONE, not to twelve */
/// > if (n < 0)   n = 1;      /* and zero is not < 0, so zero stays zero      */
/// > ```
/// >
/// > With the shipped table neither branch can fire, so nothing in the original
/// > behaves differently — but a modded county-count table would hit both, and
/// > "clamped" is exactly the plausible-sounding description that hides what
/// > the code does. Corrected in the document. `[D]`
pub fn bands_in_play(county_count: usize) -> usize {
    let n = BAND_COUNT_BY_COUNTIES
        .get(county_count)
        .copied()
        .unwrap_or(*BAND_COUNT_BY_COUNTIES.last().expect("the table is not empty"));
    if n > MERCENARY_BANDS as i32 {
        1
    } else if n < 0 {
        1
    } else {
        n as usize
    }
}

/// One live band — `g_mercBands` (`0x00568DC0`), stride `0x14`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Band {
    /// `+0x00` — the unit slot that hired this band; 0 means available.
    pub hired_by: u16,
    /// `+0x03` — the county it is **currently offered in**; 0 = nowhere.
    pub offered_in: u8,
    /// `+0x04` — the next county in its walk.
    pub next_county: u8,
    /// `+0x06` — counts down to the next offer.
    pub countdown: i8,
    /// `+0x07` — what the countdown reloads to, from `g_mercPeriod`.
    pub reload: i8,
}

impl Band {
    pub fn is_available(&self) -> bool {
        self.hired_by == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MercenaryBands {
    bands: [Band; BAND_SLOTS],
    in_play: usize,
}

impl Default for MercenaryBands {
    fn default() -> Self {
        MercenaryBands::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;
    use crate::unit::Unit;

    fn blank() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        (core::array::from_fn(|_| County::new()), core::array::from_fn(|_| Realm::new()))
    }

    #[test]
    fn the_roster_is_the_shipped_one() {
        assert_eq!(ROSTER[11].nationality, "Spanish");
        assert_eq!((ROSTER[11].men, ROSTER[11].troop), (50, TroopType::Knight));
        assert_eq!(ROSTER[11].price, 2700);
        assert_eq!(ROSTER[12].nationality, "Angevin");
        assert_eq!((ROSTER[12].men, ROSTER[12].troop), (100, TroopType::Knight));
        assert_eq!(ROSTER[9].nationality, "Saxon");
        assert_eq!((ROSTER[9].men, ROSTER[9].troop, ROSTER[9].price), (150, TroopType::Maceman, 1900));
        assert_eq!(ROSTER[10].men, 250, "Burgundian, the other maceman band");
        assert!(
            !ROSTER[1..].iter().any(|b| b.troop == TroopType::Maceman && b.men == 200),
            "there is no 200-man maceman band"
        );
    }

    #[test]
    fn the_listed_wage_is_a_tenth_of_the_price_for_every_band() {
        for band in &ROSTER[1..] {
            assert_eq!(band.listed_wage, band.price / 10, "{}", band.nationality);
        }
    }

    #[test]
    fn the_twelve_bands_are_six_pairs_at_a_flat_price_per_man() {
        for pair in 0..6 {
            let (a, b) = (&ROSTER[1 + pair * 2], &ROSTER[2 + pair * 2]);
            assert_eq!(a.troop, b.troop, "{} and {}", a.nationality, b.nationality);
            assert_ne!(a.men, b.men, "a small band and a large one");
            let (pa, pb) = (a.price * 100 / a.men, b.price * 100 / b.men);
            assert!((pa - pb).abs() <= 200, "{} {pa} vs {} {pb}", a.nationality, b.nationality);
        }
        assert_eq!(ROSTER[0].men, 0, "slot 0 is not a band");
    }

    #[test]
    fn a_fourteen_county_map_gets_all_twelve_bands() {
        assert_eq!(bands_in_play(14), 12);
        assert_eq!(bands_in_play(1), 1);
        assert_eq!(bands_in_play(0), 0, "no counties, no bands");
        assert_eq!(bands_in_play(2), 2);
        assert_eq!(bands_in_play(3), 2, "the table repeats 2");
        assert_eq!(BAND_COUNT_BY_COUNTIES.len(), 20, "the address arithmetic says twenty");
    }

    #[test]
    fn the_band_count_clamp_collapses_to_one_rather_than_to_twelve() {
        let mut modded = BAND_COUNT_BY_COUNTIES;
        modded[5] = 30;
        let clamp = |n: i32| if n > 12 { 1 } else if n < 0 { 1 } else { n };
        assert_eq!(clamp(modded[5]), 1, "not 12");
        assert_eq!(clamp(0), 0, "zero is not less than zero");
        assert_eq!(clamp(-3), 1);
    }

    #[test]
    fn a_band_starts_at_its_own_county_with_its_period_on_the_clock() {
        let bands = MercenaryBands::init(14);
        assert_eq!(bands.in_play(), 12);
        for (id, band) in bands.iter() {
            let rules = ROSTER[id as usize];
            assert_eq!(band.next_county, rules.start_county, "{}", rules.nationality);
            assert_eq!(band.countdown, rules.period as i8);
            assert_eq!(band.reload, rules.period as i8);
            assert!(band.is_available());
            assert_eq!(band.offered_in, 0, "nothing is on offer at the start");
        }
        assert!(bands.get(13).is_none(), "there is no thirteenth band");
        assert!(bands.get(0).is_none(), "and slot 0 is not one either");
    }

    #[test]
    fn the_saxon_band_offers_itself_every_season_and_keeps_walking() {
        let (mut counties, _) = blank();
        let mut bands = MercenaryBands::init(14);
        for i in 1..=12u8 {
            if i != 9 {
                bands.bands[i as usize].hired_by = 99;
            }
        }
        let mut offers = Vec::new();
        for _ in 0..4 {
            bands.advance(&mut counties, 14, Q);
            offers.push(bands.get(9).unwrap().offered_in);
        }
        assert_eq!(offers, vec![14, 1, 3, 5]);
    }

    #[test]
    fn a_long_period_band_is_silent_until_its_countdown_runs_out() {
        let (mut counties, _) = blank();
        let mut bands = MercenaryBands::init(14);
        for i in 1..=12u8 {
            if i != 6 {
                bands.bands[i as usize].hired_by = 99;
            }
        }
        for season in 1..=6 {
            bands.advance(&mut counties, 14, Q);
            assert_eq!(bands.get(6).unwrap().offered_in, 0, "season {season}");
        }
        bands.advance(&mut counties, 14, Q);
        assert_ne!(bands.get(6).unwrap().offered_in, 0, "the seventh season");
    }

    #[test]
    fn a_hired_band_stops_walking_altogether() {
        let (mut counties, _) = blank();
        let mut bands = MercenaryBands::init(14);
        bands.bands[3].hired_by = 7;
        let before = *bands.get(3).unwrap();
        for _ in 0..5 {
            bands.advance(&mut counties, 14, Q);
        }
        assert_eq!(*bands.get(3).unwrap(), before, "a hired band is frozen");
    }

    #[test]
    fn a_county_holds_one_offer_and_the_lower_numbered_band_wins() {
        let mut bands = MercenaryBands::init(14);
        for i in 1..=12u8 {
            bands.bands[i as usize].hired_by = 99;
        }
        bands.bands[4].hired_by = 0;
        bands.bands[9].hired_by = 0;
        bands.bands[4].offered_in = 3;
        bands.bands[9].offered_in = 3;
        assert_eq!(bands.offer_in(3), 4);

        bands.bands[4].hired_by = 1;
        assert_eq!(bands.offer_in(3), 9);
    }

    #[test]
    fn the_walk_wraps_at_the_maps_county_count_rather_than_at_sixteen() {
        let (mut counties, _) = blank();
        let mut bands = MercenaryBands::init(6);
        assert_eq!(bands.in_play(), 5);
        for _ in 0..20 {
            bands.advance(&mut counties, 6, Q);
            for (_, band) in bands.iter() {
                assert!(band.next_county <= 7, "walked past the map: {}", band.next_county);
                assert!(band.offered_in <= 6);
            }
        }
    }


    fn army(units: &mut Units, owner: u8, home: u8) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, 5, 5);
        u.men = 100;
        u.troops[TroopType::Peasant.index()] = 100;
        u.home_county = home;
        u.county = home;
        units.spawn(u).unwrap()
    }

    #[test]
    fn hiring_a_band_adds_its_men_debits_the_treasury_and_takes_it_off_the_map() {
        let (mut counties, mut realms) = blank();
        realms[1].gold = 10_000;
        counties[2].mercenary_offer = 9;
        let mut bands = MercenaryBands::init(14);
        bands.bands[9].offered_in = 2;
        let mut units = Units::new();
        let id = army(&mut units, 1, 2);

        assert!(bands.hire(&mut units, &mut counties, &mut realms, id, 9));
        let u = units.get(id).unwrap();
        assert_eq!(u.men, 100 + 150, "the Saxon band is 150 macemen");
        assert_eq!(
            u.mercenaries,
            Some(Mercenaries { band: 9, troop: TroopType::Maceman, men: 150 })
        );
        assert_eq!(realms[1].gold, 10_000 - 1900);
        assert_eq!(counties[2].mercenary_offer, 0);
        assert!(!bands.get(9).unwrap().is_available());
        assert_eq!(bands.offer_in(2), 0);

        let second = army(&mut units, 1, 2);
        assert!(!bands.hire(&mut units, &mut counties, &mut realms, second, 9));
    }

    #[test]
    fn a_mercenary_band_is_not_in_the_troop_counts_and_never_deserts() {
        let (mut counties, mut realms) = blank();
        let mut bands = MercenaryBands::init(14);
        bands.bands[10].offered_in = 1;
        let mut units = Units::new();
        let id = army(&mut units, 1, 1);
        bands.hire(&mut units, &mut counties, &mut realms, id, 10);

        let u = units.get_mut(id).unwrap();
        assert_eq!(u.men, 100 + 250);
        assert_eq!(u.troops[TroopType::Maceman.index()], 0, "not in the troop counts");
        assert_eq!(u.troop_total(), 350, "but it is in the total");

        let lost = u.desert();
        assert_eq!(lost, 10, "a tenth of the hundred peasants and nothing else");
        assert_eq!(u.mercenary_men(), 250, "the band is untouched");
    }

    #[test]
    fn releasing_a_band_takes_its_men_and_frees_it_mid_walk() {
        let (mut counties, mut realms) = blank();
        let mut bands = MercenaryBands::init(14);
        bands.bands[5].offered_in = 1;
        let mut units = Units::new();
        let id = army(&mut units, 1, 1);
        bands.hire(&mut units, &mut counties, &mut realms, id, 5);
        let walk = bands.get(5).unwrap().next_county;

        assert_eq!(bands.release(&mut units, id), 200, "the Danish band is 200 swordsmen");
        assert_eq!(units.get(id).unwrap().men, 100);
        assert!(units.get(id).unwrap().mercenaries.is_none());
        assert!(bands.get(5).unwrap().is_available());
        assert_eq!(bands.get(5).unwrap().next_county, walk, "it rejoins its walk mid-cycle");
        assert_eq!(bands.release(&mut units, id), 0, "and releasing twice does nothing");
    }

    /// Bankruptcy stage 1 — `L2.eng` 160, *"Mercenaries desert!"*
    #[test]
    fn bankruptcy_walks_every_mercenary_in_a_realm_out_at_once() {
        let (mut counties, mut realms) = blank();
        let mut bands = MercenaryBands::init(14);
        let mut units = Units::new();
        let a = army(&mut units, 1, 1);
        let b = army(&mut units, 1, 1);
        let other = army(&mut units, 2, 1);
        bands.bands[1].offered_in = 1;
        bands.bands[2].offered_in = 1;
        bands.bands[3].offered_in = 1;
        bands.hire(&mut units, &mut counties, &mut realms, a, 1);
        bands.hire(&mut units, &mut counties, &mut realms, b, 2);
        bands.hire(&mut units, &mut counties, &mut realms, other, 3);

        assert_eq!(bands.release_realm(&mut units, 1), 100 + 200);
        assert!(units.get(a).unwrap().mercenaries.is_none());
        assert!(units.get(b).unwrap().mercenaries.is_none());
        assert!(units.get(other).unwrap().mercenaries.is_some(), "another realm's band stays");
    }
}

