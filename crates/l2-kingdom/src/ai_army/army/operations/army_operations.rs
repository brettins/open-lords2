#![allow(unused_imports)]
use super::*;

use super::*;
use super::targeting::*;
use super::movement::*;
use super::*;
use super::wants::*;
use super::muster::*;
use super::raid::*;
use super::aim::*;
use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};

impl Kingdom {
    /// AI step 4 — [`resource_wants`].
    pub fn run_ai_resource_wants(&mut self, realm_id: u8) {
        resource_wants(
            &self.tables,
            &self.counties,
            self.county_count,
            &mut self.realms,
            realm_id,
        );
    }

    /// AI step 7 — `FUN_0049F93D`, which is three passes in a row.
    pub fn run_ai_armies(&mut self, realm_id: u8) -> ArmyReport {
        let (muster, raid) = choose_muster_counties(&self.tables, &self.counties, self.county_count, realm_id);
        if let Some(r) = self.realms.get_mut(realm_id as usize) {
            r.muster_county = muster;
            r.raid_county = raid;
        }
        let garrisons_raised = self.run_ai_garrisons(realm_id);
        let (frontier_raised, evacuations) = self.run_ai_frontier(realm_id);
        ArmyReport { garrisons_raised, frontier_raised, evacuations }
    }

    /// Step 7's **second** pass — `FUN_0049F12F`, the castle garrisons.
    ///
    /// For every county the realm holds that has a castle which is standing,
    /// not ruined and not under construction, is at or above the lord's
    /// [`crate::tables::AI_PERSONALITY_GARRISON_MIN_POPULATION`], **and** while
    /// the realm holds more than [`GARRISON_MIN_MISSILE_STOCK`] bows plus
    /// crossbows, raise a slice of the garrison's shortfall
    /// ([`garrison_levy_share`]) and send it to the castle on
    /// [`Mission::JOIN_GARRISON`].
    ///
    /// The missile-stock test is the interesting gate: it is
    /// `weapons[4] + weapons[0]`, **bows and crossbows only**, and it is read
/// once per county — so a realm that spends its
    /// last bows on the first castle stops garrisoning at the second.
    pub fn run_ai_garrisons(&mut self, realm_id: u8) -> Vec<usize> {
        let mut raised = Vec::new();
        let Some(lord) = self.realms.get(realm_id as usize).map(|r| r.lord) else { return raised };
        let Some(p) = self.tables.ai_personality(lord) else { return raised };
        let (floor, year) = (p.garrison_min_population, self.year);
        for id in 1..=self.county_count.min(MAX_COUNTIES - 1) {
            let c = &self.counties[id];
            if c.owner != realm_id
                || c.castle_type == 0
                || c.castle_degraded == crate::siege::CASTLE_DEGRADED_BUILDING
                || c.castle_ruined
                || c.population < floor
            {
                continue;
            }
            let realm = &self.realms[realm_id as usize];
            if realm.weapons[4] + realm.weapons[0] <= GARRISON_MIN_MISSILE_STOCK {
                continue;
            }
            let sitting = self
                .campaign
                .units
                .get(c.garrison_unit)
                .map_or(0, |u| u.men);
            let gap = crate::industry::garrison_cap(&self.tables, c.castle_type) - sitting;
            if gap <= GARRISON_GAP_MIN {
                continue;
            }
            let Some(want) = garrison_levy_share(gap, c.population) else { continue };
            let percent = crate::industry::pct_of(want, c.population);
            let Some(unit) = self.raise_garrison_levy(id as u8, percent, year) else { continue };
            let tile = aim_tile(
                &self.campaign.map,
                &self.counties,
                self.campaign.units.get(unit).map_or((0, 0), |u| u.tile()),
                id as u8,
                Aim::Castle,
            );
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = Mission::JOIN_GARRISON;
                u.needs_destination = false;
                u.dest_county = id as u8;
                u.dest = Some(tile);
            }
            raised.push(unit);
        }
        raised
    }

    /// `FUN_004A5389` — the AI's garrison levy, which is **not**
    /// `FUN_004A50AE` and does not equip the same way.
    ///
    /// [`GARRISON_EQUIP_ORDER`] is the difference: a greedy fill in a fixed
    /// priority, where the general raiser round-robins ten at a time.
    fn raise_garrison_levy(&mut self, county: u8, percent: i32, year: i32) -> Option<usize> {
        let realm = self.counties.get(county as usize)?.owner;
        let population = self.counties.get(county as usize)?.population;
        let happiness_cost = self.tables.army_happiness_cost(percent);
        let men = pct(population, percent);
        let mut basket = LevyBasket::seed(self.realms.get(realm as usize)?, men);
        for troop in GARRISON_EQUIP_ORDER {
            basket.equip(troop, i32::MAX);
        }
        let Kingdom { tables, counties, realms, campaign, .. } = self;
        levy::create_army(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            &basket,
            Muster { realm, county, happiness_cost, year },
            &mut campaign.explored,
        )
        .ok()
    }

    /// Step 7's **third** pass — `FUN_0049F431`, hold the frontier or write it
    /// off.
    ///
    /// For every county the realm holds that has **no castle or no garrison**,
    /// compare the hostile men standing in it against the realm's own. When
    /// the enemy is above [`FRONTIER_ENEMY_MIN`] and beats the defenders by
    /// more than [`FRONTIER_DEFICIT`], the realm decides:
    ///
    /// * **hold** — if the enemy force is smaller than the realm's total
    ///   weapon stock plus two thirds of the county's people, *or* the county
    ///   is the muster county. A levy of [`FRONTIER_LEVY_PCT`] goes up, **armed**
    ///   from the realm's stores, on [`Mission::HOLD_HOME`].
    /// * **write it off** — the same levy goes up **carrying nothing at all**,
    ///   with its [`crate::unit::Unit::home_county`] set to the *muster* county so it walks
///   away; the county is taxed at the
    ///   lord's [`crate::tables::AI_PERSONALITY_ABANDON_TAX_RATE`], its whole
    ///   workforce is thrown at industry, and its larder is shipped out.
    ///
    /// **The two branches differ by one argument** — the equip mode — and that
    /// single bit is the whole difference between a defence and an evacuation.
    /// A county below [`FRONTIER_MIN_POPULATION`] people forces the comparison
    /// threshold to zero and is therefore **always** written off.
    ///
    /// The larder is the seam; see [`Evacuation`].
    pub fn run_ai_frontier(&mut self, realm_id: u8) -> (Vec<usize>, Vec<Evacuation>) {
        let (mut raised, mut evacuations) = (Vec::new(), Vec::new());
        let Some(lord) = self.realms.get(realm_id as usize).map(|r| r.lord) else {
            return (raised, evacuations);
        };
        let Some(p) = self.tables.ai_personality(lord) else { return (raised, evacuations) };
        let abandon_tax_rate = p.abandon_tax_rate;
        let year = self.year;
        for id in 1..=self.county_count.min(MAX_COUNTIES - 1) {
            let c = &self.counties[id];
            if c.owner != realm_id || (c.castle_type != 0 && c.garrison_unit != 0) {
                continue;
            }
            let enemy = men_in_county(&self.campaign.units, &self.realms, id as u8, realm_id, true);
            let mine = men_in_county(&self.campaign.units, &self.realms, id as u8, realm_id, false);
            if enemy <= FRONTIER_ENEMY_MIN || mine >= enemy - FRONTIER_DEFICIT {
                continue;
            }
            let population = self.counties[id].population;
            let muster = self.realms[realm_id as usize].muster_county;
            let mut threshold = self.realms[realm_id as usize].weapons_total() + (population * 2) / 3;
            if population < FRONTIER_MIN_POPULATION {
                threshold = 0;
            }
            let hold = enemy < threshold || muster == id as u8;
            let unit = self.raise_frontier_levy(id as u8, hold, year);
            if let Some(unit) = unit {
                if let Some(u) = self.campaign.units.get_mut(unit) {
                    u.mission = Mission::HOLD_HOME;
                    if !hold {
                        u.home_county = muster;
                    }
                }
                raised.push(unit);
            }
            if hold {
                continue;
            }
            let c = &mut self.counties[id];
            c.tax_rate = abandon_tax_rate;
            c.industry_share = 100;
            if worth_evacuating(c) {
                evacuations.push(Evacuation {
                    from: id as u8,
                    to: muster,
                    grain: c.grain,
                    herd: c.herd,
                });
            }
        }
        (raised, evacuations)
    }

    /// `FUN_004A50AE` with the frontier pass's arguments: 50 % of the county,
    /// a population floor of [`FRONTIER_LEVY_MIN_POPULATION`], and the equip
    /// mode that decides whether this is a defence or a retreat.
    fn raise_frontier_levy(&mut self, county: u8, armed: bool, year: i32) -> Option<usize> {
        let c = self.counties.get(county as usize)?;
        if c.population < FRONTIER_LEVY_MIN_POPULATION {
            return None;
        }
        let (realm, population) = (c.owner, c.population);
        let happiness_cost = self.tables.army_happiness_cost(FRONTIER_LEVY_PCT);
        let men = pct(population, FRONTIER_LEVY_PCT);
        let mut basket = LevyBasket::seed(self.realms.get(realm as usize)?, men);
        if armed {
            basket.auto_equip();
        }
        let Kingdom { tables, counties, realms, campaign, .. } = self;
        levy::create_army(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            &basket,
            Muster { realm, county, happiness_cost, year },
            &mut campaign.explored,
        )
        .ok()
    }

    /// AI step 9 — `FUN_0049F977`, **raise the main army and point it at
    /// somebody**.
    ///
    /// The shape, which is more interesting than any one of its numbers:
    ///
    /// 1. An **ally's request** outranks everything. If one stands and still
    ///    holds ([`ally_request_still_stands`]), that county is the target and
    ///    the lord's population floor is halved.
    /// 2. Otherwise, with a **declared war target**, the floor is halved and
    ///    the realm looks for somewhere every turn.
    /// 3. Otherwise the realm counts to its lord's
    ///    [`crate::tables::AI_PERSONALITY_MUSTER_PATIENCE`] first, and only
    ///    then looks. Two attempts: **the threat's counties**, and failing
    ///    that anybody's. Nothing found means nothing raised.
    /// 4. If the muster county has more people than the floor, levy
    ///    [`crate::tables::AI_PERSONALITY_MUSTER_PCT`] of it — gated on the
    ///    realm's weapon stock unless [`emergency_weapons`] fires. If it has
    ///    **fewer**, no levy at all: an existing idle army of at least
    ///    [`DIVERT_MIN_MEN`] is diverted instead.
    /// 5. Whatever came out is aimed by [`Kingdom::aim_army`].
    ///
    /// Point 4 is the one worth reading twice. A realm whose best county is
    /// too small to conscript **does not stop making war** — it re-tasks the
    /// army it already has. That is why a cornered AI keeps coming.
    ///
    /// Returns the army it raised or diverted.
    pub fn run_ai_raise_army(&mut self, realm_id: u8) -> Option<usize> {
        let lord = self.realms.get(realm_id as usize)?.lord;
        let p = *self.tables.ai_personality(lord)?;
        let mut floor = p.help_population_floor;

        let target_county = self.realms[realm_id as usize].target_county;
        if target_county == 0 {
            if self.realms[realm_id as usize].war_target == 0 {
                let r = &mut self.realms[realm_id as usize];
                r.muster_timer = r.muster_timer.saturating_add(1);
                if (r.muster_timer as i32) < p.muster_patience {
                    return None;
                }
                r.muster_timer = 0;
            } else {
                floor /= 2;
            }
        } else {
            let kept = ally_request_still_stands(&self.counties, &self.realms, target_county, realm_id);
            self.realms[realm_id as usize].target_county = kept;
        }

        let muster = self.realms[realm_id as usize].muster_county;
        if self.realms[realm_id as usize].target_county == 0 {
            let threat = pick_threat(&self.realms, realm_id);
            self.realms[realm_id as usize].threat_realm = threat;
            if !self.pick_attack_county(realm_id, muster, true, 0)
                && !self.pick_attack_county(realm_id, muster, false, 0)
            {
                return None;
            }
        } else {
            let t = self.realms[realm_id as usize].target_county;
            self.realms[realm_id as usize].attack_county = t;
            floor /= 2;
        }

        let population = self.counties.get(muster as usize).map_or(0, |c| c.population);
        let year = self.year;
        let unit = if population > floor {
            let emergency = emergency_weapons(&mut self.realms[realm_id as usize], year);
            if !emergency && self.realms[realm_id as usize].weapons_total() < p.muster_arms {
                return None;
            }
            self.raise_muster_levy(muster, p.muster_pct, year)?
        } else {
            let target = self.realms[realm_id as usize].attack_county;
            self.pick_army_to_divert(realm_id, target)?
        };

        self.aim_army(realm_id, unit);
        self.realms[realm_id as usize].target_county = 0;
        Some(unit)
    }

    /// `FUN_004A50AE(muster, musterPct, 100, 1)` — the main levy, auto-equipped
    /// from the realm's stores.
    fn raise_muster_levy(&mut self, county: u8, percent: i32, year: i32) -> Option<usize> {
        let c = self.counties.get(county as usize)?;
        if c.population < FRONTIER_LEVY_MIN_POPULATION {
            return None;
        }
        let (realm, population) = (c.owner, c.population);
        let happiness_cost = self.tables.army_happiness_cost(percent);
        let men = pct(population, percent);
        let mut basket = LevyBasket::seed(self.realms.get(realm as usize)?, men);
        basket.auto_equip();
        let Kingdom { tables, counties, realms, campaign, .. } = self;
        levy::create_army(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            &basket,
            Muster { realm, county, happiness_cost, year },
            &mut campaign.explored,
        )
        .ok()
    }

    /// `FUN_004A03F2` — pick the county to march on, into realm `+0x4B`.
    ///
    /// `prefer_threat` restricts the search to [`crate::realm::Realm::threat_realm`]'s
    /// counties; `owner_filter` restricts it to one realm's and — the part
    /// that matters — **skips the diplomacy and adjacency gates entirely**.
/// Step 10 uses the filter, so a raid can be sent at a county
    /// nowhere near the raider's border, and at an ally's.
    ///
    /// Returns whether anything was found.
    pub fn pick_attack_county(
        &mut self,
        realm_id: u8,
        from: u8,
        prefer_threat: bool,
        owner_filter: u8,
    ) -> bool {
        let mut best = TARGET_SCORE_CEILING;
        let mut found = 0u8;
        let threat = self.realms[realm_id as usize].threat_realm;
        for id in 1..=self.county_count.min(MAX_COUNTIES - 1) {
            let owner = self.counties[id].owner;
            let eligible = if owner_filter != 0 {
                owner == owner_filter
            } else {
                (!prefer_threat || threat == owner)
                    && action_allowed(&mut self.realms, realm_id, owner)
                    && county_borders_realm(&self.counties, id as u8, realm_id)
            };
            if !eligible {
                continue;
            }
            let Some(from_county) = self.counties.get(from as usize) else { continue };
            let score = target_score(from_county, &self.counties[id], true);
            if score < best {
                best = score;
                found = id as u8;
            }
        }
        self.realms[realm_id as usize].attack_county = found;
        found != 0
    }

    /// `FUN_004A0917` — the best **existing** army to send instead of raising
    /// one.
    ///
    /// Only an army with no orders (`needs_destination`), out of any garrison
    /// and any siege, and of at least [`DIVERT_MIN_MEN`] men qualifies. It is
    /// scored [`DIVERT_IN_COUNTY`] for already standing in the target county,
    /// [`DIVERT_NEXT_DOOR`] for standing next door, nothing otherwise, plus an
    /// eighth of its [`crate::unit::Unit::strength_score`] — so **position dominates
    /// strength**: an eighth of the strongest plausible army is worth far less
    /// than the 200 that being adjacent is worth.
    pub fn pick_army_to_divert(&self, realm_id: u8, county: u8) -> Option<usize> {
        let mut best = 0;
        let mut chosen = None;
        for (id, u) in self.campaign.units.iter() {
            if u.owner != realm_id
                || u.garrison_county != 0
                || u.besieging_county != 0
                || !u.needs_destination
                || u.men < DIVERT_MIN_MEN
            {
                continue;
            }
            let mut score = if u.county == county {
                DIVERT_IN_COUNTY
            } else if self
                .counties
                .get(u.county as usize)
                .is_some_and(|c| c.neighbours().contains(&county))
            {
                DIVERT_NEXT_DOOR
            } else {
                0
            };
            score += u.strength_score() / 8;
            if score > best {
                best = score;
                chosen = Some(id);
            }
        }
        chosen
    }

    /// `FUN_0049FDA5` — give the raised or diverted army its mission and its
    /// destination tile.
    ///
    /// A plain campaign is [`Mission::SEEK_ENEMY`]. An **ally's request**
    /// against a county the ally itself holds is [`Mission::ASSIST_ALLY`] —
/// relief — and against anybody else's it is an
    /// ordinary attack. The tile is [`aim_for_county`]'s choice, so a
    /// garrisoned castle is approached at the castle and a siege begins.
    pub fn aim_army(&mut self, realm_id: u8, unit: usize) {
        let (target, request, ally) = {
            let r = &self.realms[realm_id as usize];
            (r.attack_county, r.target_county, r.ally)
        };
        let target_owner = self.counties.get(target as usize).map_or(0, |c| c.owner);
        let aim = aim_for_county(&self.counties, target);
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, target, aim);
        let Some(u) = self.campaign.units.get_mut(unit) else { return };
        if request == 0 {
            u.mission = Mission::SEEK_ENEMY;
        } else {
            u.mission_county = request;
            u.mission =
                if target_owner == ally { Mission::ASSIST_ALLY } else { Mission::SEEK_ENEMY };
        }
        u.needs_destination = false;
        u.dest_county = target;
        u.dest = Some(tile);
    }

    /// AI step 10 — `FUN_004A0015`, **the raid**.
    ///
    /// One unit, every [`crate::tables::AI_PERSONALITY_RAID_INTERVAL`] turns,
    /// out of the muster county, aimed at the standing crops of a county
    /// belonging to the realm this one thinks worst of. It is
    /// [`RAID_MEN`]-ish men carrying **nothing** — `FUN_004A5003` never opens
    /// the armoury — so it cannot fight and is not meant to: the damage is
    /// [`crate::movement::destroy_field`], done on the way in.
    ///
    /// Three gates, all on the muster county: more than
    /// [`RAID_MIN_POPULATION`] people, more than [`RAID_MIN_HAPPINESS`]
    /// happiness, and a rival below [`RAID_STANDING_THRESHOLD`] to send it at.
    ///
    /// **The cooldown is only loaded on success.** A realm that wants to raid
    /// and cannot — no victim, an unhappy muster county — tries again every
    /// single turn.
    pub fn run_ai_raid(&mut self, realm_id: u8) -> Option<usize> {
        if self.realms.get(realm_id as usize)?.raid_timer != 0 {
            self.realms[realm_id as usize].raid_timer -= 1;
            return None;
        }
        let victim = pick_raid_victim(&self.realms, realm_id);
        if victim == 0 {
            return None;
        }
        let muster = self.realms[realm_id as usize].muster_county;
        if !self.pick_attack_county(realm_id, muster, true, victim) {
            return None;
        }
        let c = self.counties.get(muster as usize)?;
        if c.population <= RAID_MIN_POPULATION || c.happiness <= RAID_MIN_HAPPINESS {
            return None;
        }
        let year = self.year;
        let unit = self.raise_raiding_party(muster, year)?;
        let target = self.realms[realm_id as usize].attack_county;
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, target, Aim::StandingCrop);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.mission = Mission::RAID;
            u.needs_destination = false;
            u.dest_county = target;
            u.dest = Some(tile);
        }
        let lord = self.realms[realm_id as usize].lord;
        if let Some(p) = self.tables.ai_personality(lord) {
            self.realms[realm_id as usize].raid_timer = p.raid_interval.clamp(0, 255) as u8;
        }
        Some(unit)
    }

    /// `FUN_004A5003` — [`RAID_MEN`] men, unequipped.
    ///
    /// The percentage is `PctOf(50, population)` and the men are
    /// `Pct(population, that)`, so the party is fifty men rounded by integer
    /// percent: 48 out of a county of 400, 50 out of 500, 45 out of 900. The
    /// happiness the county is charged is the table entry for that same
    /// percentage, so a **big** county pays almost nothing for a raid and a
    /// small one pays a lot.
    fn raise_raiding_party(&mut self, county: u8, year: i32) -> Option<usize> {
        let c = self.counties.get(county as usize)?;
        let (realm, population) = (c.owner, c.population);
        let percent = crate::industry::pct_of(RAID_MEN, population);
        let happiness_cost = self.tables.army_happiness_cost(percent);
        let men = pct(population, percent);
        let basket = LevyBasket::seed(self.realms.get(realm as usize)?, men);
        let Kingdom { tables, counties, realms, campaign, .. } = self;
        levy::create_army(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            &basket,
            Muster { realm, county, happiness_cost, year },
            &mut campaign.explored,
        )
        .ok()
    }
}


