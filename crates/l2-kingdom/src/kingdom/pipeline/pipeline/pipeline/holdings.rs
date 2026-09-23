#![allow(unused_imports)]
use super::*;
use super::setup::*;
use super::passes::*;
use super::seasonal::*;
use super::super::*;
use super::super::*;
use super::super::*;
use crate::ai;
use crate::county::{County, MAX_COUNTIES, MAX_COUNTY_ID};
use crate::event;
use crate::happiness;
use crate::health;
use crate::industry;
use crate::land;
use crate::phase::{Pass, Phase, PhaseTick, TurnMachine, SEASON_PIPELINE};
use crate::population;
use crate::ration;
use crate::realm::{Realm, MAX_REALMS};
use crate::report::{Message, SeasonReport};
use crate::tables::{Commodity, Season, Tables};
use crate::tax;
use crate::unrest;
use crate::weather;
use l2_net::Pcg32;

impl Kingdom {
    pub fn restore(&self) -> crate::conquest::Restore {
        crate::conquest::Restore {
            sowing: crate::ration::Sowing::from_index(self.season, self.options.advanced_farming),
            season_next: Season::from_index(self.season_next).unwrap_or(Season::Spring),
            advanced_farming: self.options.advanced_farming,
            armies_eat: self.options.armies_eat,
            county_count: self.county_count,
        }
    }

    /// `County_MakeIndependent` (`0x004AC3C6`) — secession, revolt, the
    /// elimination of a realm and a conquest the taker cannot reach all end
    /// here. The function is [`crate::conquest::make_independent`]; this is the
    /// kingdom's arguments for it.
    pub fn make_county_independent(&mut self, county: usize) {
        if county == 0 || county > self.county_count {
            return;
        }
        let r = self.restore();
        let Kingdom { counties, realms, campaign, tables, .. } = self;
        crate::conquest::make_independent(tables, counties, realms, county as u8, &campaign.map, r);
    }

    /// The blacksmith's stockpile share ([`industry::weapon_shares`],
    /// `FUN_0044F15B`) is computed **per realm, before the loop**. The original
    /// recomputes it inside every `resourceLimit` call, which is the same
    /// answer: its inputs are which smithies are switched on and staffed, and
    /// the pass changes neither.
    ///
    /// They run in the pass that ends each of the original's loops — weapons'
    /// own, and wood's for the other four — so the order they see the stock in
    /// is the original's **except in one place, `[D]`**: this crate runs iron
    /// and stone over *every* county before wood runs over any, where the
    /// original's second loop does iron, stone and wood county by county. So
    /// the weapons estimate in county `c` sees the iron every county of its realm
    /// mined this season, not only counties `1 ..= c`. It moves a blacksmith's
    /// ceiling in a realm where a later county mines iron, for the season's two
    /// allocations only — `Panels_RefreshAll` recomputes it from the final stock
    /// before anything is drawn. Folding the passes together would move
    /// `l2_game::save`'s pass indices.
    pub(super) fn industry(&mut self, commodity: Commodity) {
        let shares: [industry::WeaponShare; MAX_REALMS] = core::array::from_fn(|realm| {
            industry::weapon_shares(&self.tables, &self.counties, self.county_count, realm as u8)
        });
        let advanced = self.options.advanced_farming;
        let count = self.county_count;
        let neutral = Realm::new();
        let estimated: &[Commodity] = match commodity {
            Commodity::Weapons => &[Commodity::Weapons],
            Commodity::Wood => &crate::tables::INDUSTRY_ESTIMATE_ORDER,
            Commodity::Iron | Commodity::Stone => &[],
        };
        let (counties, realms, tables) = (&mut self.counties, &mut self.realms, &self.tables);
        for id in 1..=count {
            let owner = counties[id].owner as usize;
            if owner >= MAX_REALMS {
                continue;
            }
            if owner != 0 {
                industry::produce_with_share(
                    tables,
                    &mut counties[id],
                    &mut realms[owner],
                    commodity,
                    advanced,
                    shares[owner],
                );
            }
            let realm = realms.get(owner).unwrap_or(&neutral);
            for &c in estimated {
                industry::refresh(tables, &mut counties[id], c, realm, shares[owner], advanced);
            }
        }
        // **`Industry_ProduceAll` (`0x0044E852`) has no owner test here, and
        // this used to.** Its loop is `for (c = 1; c <= g_countyCount; c++)`
        // with the update unconditional; the owner test is one level down, in
        // `Industry_Produce` (`0x0044EA92`), where `if (realm != 0)` guards the
        // *production* and nothing else. A county that leaves a realm still has
        // its site tiles repainted the next season.
        //
        // The visible defect was a seceded county's mine going on turning for
        // the rest of the game. `County_MakeIndependent` (`0x004AC3C6`) clears
        // all four `enabled` flags and repaints nothing — `Industry_Produce`
        // and `Industry_ProduceAll` hold every call site of
        // `Industry_UpdateSiteTile` there is — so the terrain byte stays on
        // `base + 1`, the *working* value `Sprite_TopIt` (`0x004071A0`)
        // animates, and this pass is the only thing that ever writes
        // `base + 0` over it.
        for id in 1..=self.county_count {
            self.update_industry_site(id, commodity);
        }
    }

    /// `Castle_BuildTick` (`0x004508DE`) over every county, plus the two things
    /// it does that [`industry::build_tick`] cannot reach: the free garrison a
    /// finished castle comes with
    pub(super) fn castle_build_tick(&mut self, report: &mut SeasonReport) {
        // `crate::tables::SCORE_INPUT_OFFSETS` has read `+0x4C` as *castles
        // held* for as long as it has existed, with the C for it in the doc
        // comment — and **nothing in this workspace ever wrote it**, so
        // `score_inputs[5]`
        // heaviest-weighted of the six terms (x50, more than the other five
        // combined) contributed nothing to anybody's score. That is
        // `docs/agents.md`'s *a correct explanation sitting directly above the
        // omission it describes*, and `crates/l2-game/tests/differential.rs` is
        // what found it: with the score's other five inputs repaired, every
        // realm came in exactly 50 short, once per castle.
        for realm in self.realms.iter_mut().take(MAX_REALMS).skip(1) {
            realm.score_inputs[crate::tables::SCORE_INPUT_CASTLES] = 0;
        }
        for id in 1..=self.county_count {
            let owner = self.counties[id].owner as usize;
            // **Counted before the work, not after**, which is the original's
            // order and decides a real case: the season a castle tops out, the
            // county is still `castle_degraded != 0` here, so it is counted into
            // `+0x4D` (the concurrency count, derived in `ai::build_castles`)
            // and does not score its 50 until the *next* season.
            if self.counties[id].castle_degraded == 0 && self.counties[id].castle_type != 0 {
                if let Some(r) = self.realms.get_mut(owner) {
                    r.score_inputs[crate::tables::SCORE_INPUT_CASTLES] += 1;
                }
            }
            let mut messages = Vec::new();
            let (mut realm, has_realm) = match self.realms.get(owner) {
                Some(r) => (r.clone(), true),
                None => (Realm::new(), false),
            };
            let done = industry::build_tick(
                &self.tables,
                &mut self.counties[id],
                &mut realm,
                id as u8,
                &mut messages,
            );
            if has_realm {
                self.realms[owner] = realm;
            }
            for m in messages {
                report.message(m);
            }
            if self.counties[id].castle_degraded != 0 || done.is_some() {
                crate::map::stamp_castle_terrain(
                    &mut self.campaign.map,
                    id as u8,
                    self.counties[id].castle_type,
                );
            }
            let Some(done) = done else { continue };
            if done.free_archers > 0 {
                self.raise_free_garrison(id as u8, done.free_archers);
            }
        }
    }

}



