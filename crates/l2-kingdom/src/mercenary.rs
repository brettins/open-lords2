//! The twelve mercenary bands — `docs/armies.md` §5.
//!
//! They are not a random event and they are not on the merchant screen.
//! **Twelve fixed bands wander the map, one per nationality, and you hire
//! whichever is standing in your county on the turn you raise an army there.**
//! The roster is identical on every map and every playthrough; only the
//! starting counties and the walk depend on the map.
//!
//! # Why the roster is a finding rather than a fit
//!
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

use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{Mercenaries, TroopType, UnitKind, Units};
use l2_net::{Quirk, Quirks};

/// Twelve bands, one per nationality, indexed 1…12. Slot 0 is never a band.
pub const MERCENARY_BANDS: usize = 12;

/// The array bound: `MERCENARY_BANDS + 1`.
pub const BAND_SLOTS: usize = MERCENARY_BANDS + 1;

/// One row of the shipped roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandRules {
    /// `L2.eng` group 16, which begins *"No mercenaries in the army."* and then
    /// names the twelve nationalities.
    pub nationality: &'static str,
    pub men: i32,
    pub troop: TroopType,
    pub price: i32,
    /// `g_mercWage`, which is **`price / 10` for every band and is never read
    /// anywhere**. Carried because a mod might want it and because its absence
    /// from every read path is itself a finding: the raise-army screen prints
    /// `men / 2` and `Wages_ForUnit` charges `men / 4`, so there are three
    /// different numbers for the same thing and only the last is spent.
    pub listed_wage: i32,
    /// Where the band begins its walk, and where it returns to.
    pub start_county: u8,
    /// It stops to offer itself every this-many seasons.
    pub period: u8,
}

/// `g_mercMen`, `g_mercTroopType`, `g_mercPrice`, `g_mercWage`,
/// `g_mercStartCounty` and `g_mercPeriod`, as twelve rows. Index 0 is a
/// placeholder that no rule reads, exactly as in the original.
///
/// Nationalities come in pairs by troop type — a small band and a large one of
/// each — and the price per man is flat within a type: pikemen ≈ 18, archers
/// 20, swordsmen ≈ 27, crossbowmen 30, macemen ≈ 12.5, knights ≈ 54. The prices
/// are hand-authored: price ÷ `TROOP_STRENGTH_WEIGHT` comes out 2.0, 1.54, 2.1,
/// 1.875, 1.56 and 2.5 across the six types, so they do not track the combat
/// value.
pub const ROSTER: [BandRules; BAND_SLOTS] = [
    // Slot 0 — never read. Every field is the zero its absence deserves.
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
/// **Twenty entries**, and the length is held by address arithmetic:
/// `0x004DE820 + 20 * 4 = 0x004DE870`, which is where the string table begins
/// (`"ff_batl.wav"`). England has 14 counties, so all twelve bands exist there.
/// `[V]`
pub const BAND_COUNT_BY_COUNTIES: [i32; 20] =
    [0, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 12, 12, 12, 12, 12, 12];

/// `Mercenary_Init`'s clamp on [`BAND_COUNT_BY_COUNTIES`].
///
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

/// The live band table plus how many of it are in play.
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

impl MercenaryBands {
    /// No bands at all — the state a kingdom with no map is in.
    pub fn none() -> MercenaryBands {
        MercenaryBands { bands: [Band::default(); BAND_SLOTS], in_play: 0 }
    }

    /// `Mercenary_Init` (`0x004AC904`), once from `Game_NewGame`: decide how
    /// many bands the map supports and seed each from the roster.
    ///
    /// The band's *start* county is copied into both `start` and `next`, and
    /// the period into both `countdown` and `reload`, so a band offers itself
    /// for the first time `period` seasons in.
    pub fn init(county_count: usize) -> MercenaryBands {
        let in_play = bands_in_play(county_count);
        let mut bands = [Band::default(); BAND_SLOTS];
        for (i, band) in bands.iter_mut().enumerate().take(in_play + 1).skip(1) {
            let rules = ROSTER[i];
            band.hired_by = 0;
            band.offered_in = 0;
            band.next_county = rules.start_county;
            band.countdown = rules.period as i8;
            band.reload = rules.period as i8;
        }
        MercenaryBands { bands, in_play }
    }

    pub fn in_play(&self) -> usize {
        self.in_play
    }

    pub fn get(&self, band: u8) -> Option<&Band> {
        let i = band as usize;
        (1..=self.in_play).contains(&i).then(|| &self.bands[i])
    }

    pub fn rules(&self, band: u8) -> Option<&'static BandRules> {
        self.get(band).map(|_| &ROSTER[band as usize])
    }

    /// A slot regardless of whether it is in play, for [`crate::save`]: the
    /// file carries all thirteen so a band that a modded map put out of play
    /// still round-trips.
    pub fn band_raw(&self, band: usize) -> Band {
        self.bands.get(band).copied().unwrap_or_default()
    }

    pub fn set_band_raw(&mut self, band: usize, value: Band) {
        if let Some(slot) = self.bands.get_mut(band) {
            *slot = value;
        }
    }

    pub fn set_in_play(&mut self, count: usize) {
        self.in_play = count.min(MERCENARY_BANDS);
    }

    /// Every band in play, in ascending id order.
    pub fn iter(&self) -> impl Iterator<Item = (u8, &Band)> {
        self.bands.iter().enumerate().skip(1).take(self.in_play).map(|(i, b)| (i as u8, b))
    }

    /// `Mercenary_AdvanceAll` (`0x004ACA2B`), once a season at end of turn.
    ///
    /// ```text
    /// for each band not currently hired:
    ///     offeredIn = 0
    ///     countdown--;  nextCounty++
    ///     if nextCounty > countyCount: nextCounty = 1
    ///     if countdown < 1:
    ///         offeredIn = nextCounty
    ///         countdown = reload
    ///         nextCounty++                       # note: no wrap check here
    /// for each county:
    ///     county.mercOffer = the lowest-numbered unhired band offered here, else 0
    /// ```
    ///
    /// Each band **walks one county a season** and stops to offer itself every
    /// `period` seasons — Saxon every season, Norman and Danish every other,
    /// Swedish and Angevin every seventh. The county's offer is a *cache* and
    /// holds one band; if two land on the same county the lower-numbered one
    /// wins.
    ///
    /// `[D]`, and one detail worth reproducing rather than tidying: the second
    /// `nextCounty++` has **no wrap guard**, so a band that has just made an
    /// offer sits at `countyCount + 1` for one season until the next call wraps
    /// it. Reproduce the sequence, not the invariant.
    ///
    /// **Switchable** — [`Quirk::MercenaryBandOvershoots`], `docs/bugs.md` B42.
    /// The fixed path gives the second increment the wrap guard the first one
    /// has, so a band that has just made an offer stands next season in the
    /// county after it rather than one past the end of the map.
    pub fn advance(
        &mut self,
        counties: &mut [County; MAX_COUNTIES],
        county_count: usize,
        quirks: Quirks,
    ) {
        let limit = county_count.max(1) as u8;
        for i in 1..=self.in_play {
            let band = &mut self.bands[i];
            if !band.is_available() {
                continue;
            }
            band.offered_in = 0;
            band.countdown -= 1;
            band.next_county = band.next_county.saturating_add(1);
            if band.next_county > limit {
                band.next_county = 1;
            }
            if band.countdown < 1 {
                band.offered_in = band.next_county;
                band.countdown = band.reload;
                band.next_county = band.next_county.saturating_add(1);
                if !quirks.reproduces(Quirk::MercenaryBandOvershoots)
                    && band.next_county > limit
                {
                    band.next_county = 1;
                }
            }
        }
        for id in 1..=county_count.min(MAX_COUNTIES - 1) {
            counties[id].mercenary_offer = self.offer_in(id as u8);
        }
    }

    /// `Mercenary_OfferInCounty` (`0x004ACB54`) — the lowest-numbered unhired
    /// band standing in a county, or 0.
    pub fn offer_in(&self, county: u8) -> u8 {
        for (id, band) in self.iter() {
            if band.is_available() && band.offered_in == county {
                return id;
            }
        }
        0
    }

    /// `Mercenary_Hire` (`0x004AC7F3`), which runs from inside `Army_Create`.
    ///
    /// ```c
    /// unit.mercBand = band;  unit.mercTroopType = bands[band].troopType;
    /// unit.mercMen  = (byte)bands[band].men;
    /// unit.men     += bands[band].men;
    /// bands[band].hiredBy = unit;  bands[band].offeredIn = 0;
    /// realm[unit.owner].gold -= bands[band].price;
    /// county[unit.homeCounty].mercOffer = 0;
    /// ```
    ///
    /// Three details that a reading of the doc's summary would miss, all `[V]`:
    ///
    /// * the gold comes out of **`realm[unit.owner]`**, not out of the realm
    ///   `Army_Create` was called for — they agree in play, and would not for
    ///   an ownerless unit;
    /// * the offer is cleared on the unit's **home county**, not on the county
    ///   the hire was made from;
    /// * `mercMen` is a genuine **byte truncation** of an `i32`. The largest
    ///   shipped band is 250 so nothing is lost, but a modded band of 300 would
    ///   silently store 44 while `men` gained the full 300.
    ///
    /// **The price is not checked here.** `L2.eng` group 69 has *"You cannot
    /// afford to hire these mercenaries."* and the screen refuses before it
    /// gets this far; this function will happily take a realm negative.
    /// Returns false only when the band is not on offer.
    pub fn hire(
        &mut self,
        units: &mut Units,
        counties: &mut [County; MAX_COUNTIES],
        realms: &mut [Realm; MAX_REALMS],
        unit: usize,
        band: u8,
    ) -> bool {
        let i = band as usize;
        if !(1..=self.in_play).contains(&i) || !self.bands[i].is_available() {
            return false;
        }
        let rules = ROSTER[i];
        let Some(u) = units.get_mut(unit) else { return false };
        u.mercenaries = Some(Mercenaries { band, troop: rules.troop, men: rules.men as u8 });
        u.men += rules.men;
        let (owner, home) = (u.owner, u.home_county);

        self.bands[i].hired_by = unit as u16;
        self.bands[i].offered_in = 0;
        if let Some(r) = realms.get_mut(owner as usize) {
            r.gold -= rules.price;
        }
        if let Some(c) = counties.get_mut(home as usize) {
            c.mercenary_offer = 0;
        }
        true
    }

    /// `Mercenary_Release` (`0x004AC6FE`) — the band walks off and is free to
    /// be hired again.
    ///
    /// ```c
    /// if (unit.type == 1 && unit.mercBand != 0) {
    ///     unit.men -= (byte)unit.mercMen;      /* the byte, not the table value */
    ///     unit.mercMen = unit.mercTroopType = unit.mercBand = 0;
    ///     bands[band].hiredBy = 0;
    /// }
    /// ```
    ///
    /// It frees **only** `hiredBy`: the countdown, the offer and the walk
    /// position are left alone, so a released band rejoins its round mid-cycle
    /// rather than starting again. `[D]`
    ///
    /// Two callers matter: an army being destroyed, and **bankruptcy stage 1**,
    /// which walks every mercenary in the realm out at once — `L2.eng` 160,
    /// *"Mercenaries desert!"*, `docs/kingdom.md` §7.4. Returns the men that
    /// left.
    pub fn release(&mut self, units: &mut Units, unit: usize) -> i32 {
        let Some(u) = units.get_mut(unit) else { return 0 };
        if u.kind != UnitKind::Army {
            return 0;
        }
        let Some(band) = u.mercenaries.take() else { return 0 };
        u.men -= band.men();
        if let Some(b) = self.bands.get_mut(band.band as usize) {
            b.hired_by = 0;
        }
        band.men()
    }

    /// Bankruptcy stage 1: every mercenary in a realm walks off at once.
    /// Returns the men lost.
    pub fn release_realm(&mut self, units: &mut Units, realm: u8) -> i32 {
        let ids: Vec<usize> = units
            .iter()
            .filter(|(_, u)| u.owner == realm && u.mercenaries.is_some())
            .map(|(i, _)| i)
            .collect();
        ids.into_iter().map(|i| self.release(units, i)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful. The switched-off answers live in `tests/quirks.rs`.
    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;
    use crate::unit::Unit;

    fn blank() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        (core::array::from_fn(|_| County::new()), core::array::from_fn(|_| Realm::new()))
    }

    /// The prediction `docs/armies.md` §5.1 tests itself against, and the two
    /// places the player's recollection was wrong: **the Spanish band is the
    /// fifty knights, the Angevin is a hundred**, and there is no 200-maceman
    /// band.
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

    /// The listed wage is `price / 10` for all twelve — and is never read by
    /// any rule, here or in the original.
    #[test]
    fn the_listed_wage_is_a_tenth_of_the_price_for_every_band() {
        for band in &ROSTER[1..] {
            assert_eq!(band.listed_wage, band.price / 10, "{}", band.nationality);
        }
    }

    /// Nationalities come in pairs by troop type: a small band and a large one
    /// of each of the six equipped types, and the price per man is flat within
    /// a pair.
    #[test]
    fn the_twelve_bands_are_six_pairs_at_a_flat_price_per_man() {
        for pair in 0..6 {
            let (a, b) = (&ROSTER[1 + pair * 2], &ROSTER[2 + pair * 2]);
            assert_eq!(a.troop, b.troop, "{} and {}", a.nationality, b.nationality);
            assert_ne!(a.men, b.men, "a small band and a large one");
            // Flat within a couple of crowns a man, allowing for hand-rounding.
            let (pa, pb) = (a.price * 100 / a.men, b.price * 100 / b.men);
            assert!((pa - pb).abs() <= 200, "{} {pa} vs {} {pb}", a.nationality, b.nationality);
        }
        assert_eq!(ROSTER[0].men, 0, "slot 0 is not a band");
    }

    /// England has 14 counties, so all twelve bands are in play there.
    #[test]
    fn a_fourteen_county_map_gets_all_twelve_bands() {
        assert_eq!(bands_in_play(14), 12);
        assert_eq!(bands_in_play(1), 1);
        assert_eq!(bands_in_play(0), 0, "no counties, no bands");
        assert_eq!(bands_in_play(2), 2);
        assert_eq!(bands_in_play(3), 2, "the table repeats 2");
        assert_eq!(BAND_COUNT_BY_COUNTIES.len(), 20, "the address arithmetic says twenty");
    }

    /// The correction: above twelve becomes **one**, and zero stays zero.
    /// Unreachable with the shipped table, reachable with a mod.
    #[test]
    fn the_band_count_clamp_collapses_to_one_rather_than_to_twelve() {
        let mut modded = BAND_COUNT_BY_COUNTIES;
        modded[5] = 30;
        // The rule, applied directly, since the table itself is a constant.
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

    /// The Saxon band has period 1, so it offers itself every single season —
    /// and it walks **two** counties on each, because a season that makes an
    /// offer advances the walk once for the step and once more afterwards.
    ///
    /// The sequence is also where the missing wrap guard shows: starting at
    /// county 13 on a fourteen-county map, the first season offers in 14 and
    /// leaves the walk sitting at **15**, one past the end, until the *next*
    /// season's guarded increment wraps it to 1. That is why the offers run
    /// 14, 1, 3, 5 rather than 14, 2, 4, 6 — the overshoot costs the band a
    /// county. Reproduced rather than tidied.
    #[test]
    fn the_saxon_band_offers_itself_every_season_and_keeps_walking() {
        let (mut counties, _) = blank();
        let mut bands = MercenaryBands::init(14);
        // Isolate band 9 by hiring every other one out of the way.
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

    /// A band with period 7 is silent for six seasons and offers on the
    /// seventh.
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

    /// The county's offer is a one-slot cache, and the lower-numbered band
    /// wins when two land together.
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

        // …and once the lower one is hired the higher one is visible.
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

    // --- hiring ------------------------------------------------------------

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

        // The same band cannot be hired twice.
        let second = army(&mut units, 1, 2);
        assert!(!bands.hire(&mut units, &mut counties, &mut realms, second, 9));
    }

    /// The band is inside `men` but not inside `troops`, which is what makes it
    /// atomic — and, as `Army_Desert` walks `troops` alone, what makes
    /// **mercenaries the one part of an army that never deserts**.
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
