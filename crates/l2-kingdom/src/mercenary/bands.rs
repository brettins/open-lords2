#![allow(unused_imports)]
use super::*;

use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{Mercenaries, TroopType, UnitKind, Units};
use l2_net::{Quirk, Quirks};

impl MercenaryBands {
    /// No bands at all — the state a kingdom with no map is in.
    pub fn none() -> MercenaryBands {
        MercenaryBands { bands: [Band::default(); BAND_SLOTS], in_play: 0 }
    }

    /// `Mercenary_Init` (`0x004AC904`), once from `Game_NewGame`: decide how
    /// many bands the map supports and seed each from the roster.
    ///
    /// The band's *start* county is copied into both `start` and `next`, and
    /// the period into both `countdown` and `reload`.
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
    /// file carries all thirteen.
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
    /// `[D]`, and one detail worth reproducing: the second
    /// `nextCounty++` has **no wrap guard**.
    /// offer sits at `countyCount + 1` for one season until the next call wraps
    /// it. Reproduce the sequence, not the invariant.
    ///
    /// **Switchable** — [`Quirk::MercenaryBandOvershoots`], `docs/bugs.md` B42.
    /// The fixed path gives the second increment the wrap guard the first one
    /// has.
    /// county after it.
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
    /// position are left alone.
    /// `[D]`
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

