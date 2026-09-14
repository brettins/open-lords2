#![allow(unused_imports)]
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

// ---------------------------------------------------------------------------
// The passes, over a whole kingdom
// ---------------------------------------------------------------------------

/// What one turn of the AI's army handling did, for a caller that wants to
/// show it or a test that wants to assert on it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ArmyReport {
    /// Armies raised to fill a castle garrison — step 7's second pass.
    pub garrisons_raised: Vec<usize>,
    /// Armies raised on a threatened frontier county — step 7's third pass.
    pub frontier_raised: Vec<usize>,
    /// Counties written off, and what the pass wanted to carry out of them.
    pub evacuations: Vec<Evacuation>,
}

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

// ---------------------------------------------------------------------------
// Step 11 — run every army's mission and re-path it
// ---------------------------------------------------------------------------

/// A garrison [`Mission::GARRISON`] turned out of a castle whose county has
/// changed hands, and — where it had one — the besieger that was waiting for
/// it.
///
/// `FUN_00437535` starts the battle itself. This crate reports it, for the
/// same reason [`crate::ai::taunt`] returns its letters
/// them: a battle is not this crate's to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eviction {
    pub unit: usize,
    pub county: u8,
    /// The besieger's slot, if the garrison was under siege when it was
    /// turned out.
    pub besieger: Option<usize>,
    /// True when there was nowhere to put the garrison and it was destroyed.
    pub destroyed: bool,
}

/// What [`Kingdom::run_ai_move_armies`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MoveReport {
    /// The armies that were given a fresh path this step.
    pub marching: Vec<usize>,
    /// Garrisons turned out of castles in counties the realm has lost.
    pub evictions: Vec<Eviction>,
    /// Armies disbanded because [`Mission::JOIN_GARRISON`] found no castle
    /// with room for them.
    pub disbanded: Vec<usize>,
}

/// `FUN_004673B8` — the nearest **enemy** army anywhere on the map, by
/// Chebyshev distance, as `(distance, slot)`. Distance is 1000 when there is
/// none.
///
/// Its filter is `owner != 0 && owner != mine && kind == 1 && !garrisoned`,
/// and it makes **no diplomacy check at all** — so an *allied* army is a
/// candidate here and is thrown out afterwards by
/// [`Mission::SEEK_ENEMY`]'s own test. The consequence is not cosmetic: a
/// nearer ally **masks** a slightly farther enemy, and the intercept is
/// skipped that turn. `[D]`
pub fn nearest_enemy_army(units: &Units, unit: usize) -> (i32, Option<usize>) {
    let Some(me) = units.get(unit) else { return (1000, None) };
    let mut best = 1000;
    let mut found = None;
    for (id, u) in units.iter() {
        if u.owner == 0 || u.owner == me.owner || u.kind != UnitKind::Army || u.garrison_county != 0
        {
            continue;
        }
        let d = chebyshev(me.tile(), u.tile());
        if d < best {
            best = d;
            found = Some(id);
        }
    }
    (best, found)
}

/// `FUN_00467532` — the nearest army **in one county** that
/// [`action_allowed`] lets this one attack.
///
/// It takes `&mut` realms because `Diplo_ActionAllowed` is not a predicate:
/// every allied unit it steps over bumps the searcher's own grudge. This
/// function is called once per turn per army on missions 3 and 6, over every
/// unit slot, so **an AI hemmed in by its ally corrodes the alliance by
/// looking around**. Whether that is intended is not established;
/// [`action_allowed`] carries the note.
pub fn nearest_attackable_army_in_county(
    units: &Units,
    realms: &mut [Realm; MAX_REALMS],
    unit: usize,
    county: u8,
) -> (i32, Option<usize>) {
    let Some(me) = units.get(unit) else { return (1000, None) };
    let (mine, my_tile) = (me.owner, me.tile());
    let mut best = 1000;
    let mut found = None;
    for (id, u) in units.iter() {
        if u.owner == 0 || u.kind != UnitKind::Army {
            continue;
        }
        if !action_allowed(realms, mine, u.owner) {
            continue;
        }
        if u.garrison_county != 0 || u.county != county {
            continue;
        }
        let d = chebyshev(my_tile, u.tile());
        if d < best {
            best = d;
            found = Some(id);
        }
    }
    (best, found)
}

/// `FUN_004A0649` — the target an army on [`Mission::SEEK_ENEMY`] picks for
/// itself when its standing order has run out.
///
/// The same scoring as [`Kingdom::pick_attack_county`] with the **narrow**
/// grain ladder ([`target_score`]), and the same two gates: the county must be
/// one [`action_allowed`] permits and must border the realm.
pub fn pick_next_target(
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    realms: &mut [Realm; MAX_REALMS],
    realm_id: u8,
    from: u8,
) -> u8 {
    let mut best = TARGET_SCORE_CEILING;
    let mut found = 0u8;
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        let owner = counties[id].owner;
        if owner == realm_id
            || !action_allowed(realms, realm_id, owner)
            || !county_borders_realm(counties, id as u8, realm_id)
        {
            continue;
        }
        let Some(from_county) = counties.get(from as usize) else { continue };
        let score = target_score(from_county, &counties[id], false);
        if score < best {
            best = score;
            found = id as u8;
        }
    }
    found
}

/// `FUN_004A07F2` — the **lowest-numbered** county of this realm whose castle
/// has room for `men`.
///
/// Lowest-numbered, not nearest: an army that cannot get into the castle it
/// was raised for walks to the realm's first castle instead, however far that
/// is.
pub fn first_castle_with_room(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    units: &Units,
    county_count: usize,
    realm: u8,
    men: i32,
) -> u8 {
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        let c = &counties[id];
        if c.owner != realm
            || c.castle_type == 0
            || c.castle_degraded == crate::siege::CASTLE_DEGRADED_BUILDING
            || c.castle_ruined
        {
            continue;
        }
        let sitting = units.get(c.garrison_unit).map_or(0, |u| u.men);
        if men <= crate::industry::garrison_cap(t, c.castle_type) - sitting {
            return id as u8;
        }
    }
    0
}

impl Kingdom {
    /// AI step 11 — `FUN_004A5667`, which this crate names `Ai_AdvanceArmies`.
    ///
    /// ```c
    /// for (i = 1; i <= 150; i++)
    ///   if (units[i].kind == 1 && units[i].owner == realm && mission_handler(i) != 0) {
    ///       units[i].castlePassThrough = 0;          /* +0x14E */
    ///       Move_FloodFill(0, units[i].x, units[i].y, 0);
    ///       if (Move_ExtractPath(0, units[i].destX, units[i].destY))
    ///           { Path_CopyToUnit(0, i); units[i].moving = 1; }
    ///   }
    /// ```
    ///
    /// It is the same shape as `PeasantMob_AdvanceAll`, which is already
    /// implemented as [`Kingdom::begin_unit_phase`]'s phase-5 arm — the AI's
    /// armies are the mobs' mechanism with a six-way mission dispatch in front
    /// of it.
    ///
    /// **The routing is `Direct`, not `PreferRoads`.** `Unit_OrderMove` — the
    /// player's path — tries roads first and falls back; this call passes
    /// mode 0 outright. So the claim in [`crate::movement::Routing`] that *"AI
    /// armies prefer roads and the player's do not"* is true of the
    /// *order-move* entry point and **false of the AI's own driver**, which is
/// the one that moves AI armies every turn. Both readings are of
    /// the binary; they are about two different functions.
    ///
    /// Unit `+0x14E`, cleared before each re-path, is a *pass through the
    /// castle site* flag: `Unit_TryEnterTile` masks off the settlement bit
    /// while it is set, which is what stops a garrison just turned out of a
    /// castle immediately walking back into it. This crate has no such field
    /// and no path that sets one, so there is nothing to clear. Named rather
    /// than omitted. `[D]`
    pub fn run_ai_move_armies(&mut self, realm_id: u8) -> MoveReport {
        let mut report = MoveReport::default();
        let ids: Vec<usize> = self
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == UnitKind::Army && u.owner == realm_id)
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            if !self.run_mission(id, &mut report) {
                continue;
            }
            let Some(dest) = self.campaign.units.get(id).and_then(|u| u.dest) else { continue };
            if crate::movement::order_move(
                &self.campaign.map,
                &mut self.campaign.units,
                id,
                dest,
                crate::movement::Routing::Direct,
            )
            .is_some()
            {
                report.marching.push(id);
            }
        }
        report
    }

    /// `FUN_004A57AC` — dispatch one army's mission.
    ///
    /// Returns whether the unit wants a fresh path. **An unrecognised mission
    /// byte is rewritten to [`Mission::SEEK_ENEMY`] and the unit is not moved
    /// this turn** — so a freshly spawned army, whose byte is 0, loses exactly
    /// one turn and then behaves as an attacker.
    fn run_mission(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(mission) = self.campaign.units.get(unit).map(|u| u.mission) else { return false };
        match mission {
            Mission::RAID => self.mission_raid(unit),
            Mission::ASSIST_ALLY => self.mission_assist_ally(unit),
            Mission::SEEK_ENEMY => self.mission_seek_enemy(unit),
            Mission::HOLD_HOME => self.mission_hold_home(unit),
            Mission::JOIN_GARRISON => self.mission_join_garrison(unit, report),
            Mission::GARRISON => self.mission_garrison(unit, report),
            _ => {
                if let Some(u) = self.campaign.units.get_mut(unit) {
                    u.mission = Mission::SEEK_ENEMY;
                }
                false
            }
        }
    }

    /// [`Mission::RAID`] — `FUN_004A58F4`. Re-aim at the target county's
    /// nearest standing crop, every turn, unless the unit is camped outside a
    /// castle.
    ///
    /// Re-aiming *every* turn is what makes a raid destructive out of
    /// proportion to its size: the party is fifty men who cannot fight, but it
    /// walks a fresh line to the nearest surviving crop each season and
    /// tramples everything it crosses.
    fn mission_raid(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (from, target) = (u.tile(), u.dest_county);
        let tile = aim_tile(&self.campaign.map, &self.counties, from, target, Aim::StandingCrop);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.dest = Some(tile);
            u.needs_destination = false;
        }
        true
    }

    /// [`Mission::ASSIST_ALLY`] — `FUN_004A599D`.
    ///
    /// Re-validate the ally's request; a request that no longer stands demotes
    /// the unit to [`Mission::SEEK_ENEMY`] and **runs that handler in the same
    /// tick**, so no turn is lost. Otherwise close on the nearest enemy in the
    /// ally's county, but only within [`ASSIST_ALLY_RADIUS`] — an army too far
/// away does nothing at all this turn, which
    /// is a real difference from [`Mission::HOLD_HOME`].
    fn mission_assist_ally(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, mut county) = (u.owner, u.mission_county);
        if county != 0 {
            county = ally_request_still_stands(&self.counties, &self.realms, county, owner);
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission_county = county;
            }
        }
        if county == 0 {
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = Mission::SEEK_ENEMY;
            }
            return self.mission_seek_enemy(unit);
        }
        let (d, other) = nearest_attackable_army_in_county(
            &self.campaign.units,
            &mut self.realms,
            unit,
            county,
        );
        let Some(other) = other.filter(|_| d < ASSIST_ALLY_RADIUS) else { return false };
        let tile = self.campaign.units.get(other).map(|u| u.tile());
        if let (Some(tile), Some(u)) = (tile, self.campaign.units.get_mut(unit)) {
            u.needs_destination = false;
            u.dest = Some(tile);
            return true;
        }
        false
    }

    /// [`Mission::SEEK_ENEMY`] — `FUN_004A5B1F`, the default war mission and
    /// the largest of the six.
    ///
    /// Four things in order:
    ///
    /// 1. **Intercept.** An enemy army within [`SEEK_ENEMY_RADIUS`] that is
    ///    not this realm's ally, and that is either *not itself on this
    ///    mission* or is to the east. The `x` test is a tie-break that stops
    ///    two attacking armies chasing each other for ever: whichever is
    ///    further west gives chase. `[I]` on the intent, `[D]` on the code.
    /// 2. **Keep the standing order** if the county it names is still somebody
    ///    else's and still borders this realm.
    /// 3. Otherwise, if the army is standing somewhere it cannot attack —
    ///    its own county, or one that does not touch its realm — **pick a new
    ///    target** and go.
    /// 4. Otherwise it is standing in a hostile county that borders its realm:
    ///    **attack where it stands**, unless that county belongs to its ally,
    ///    in which case pick another.
    ///
    /// **Two defects are reproduced.** In arm 4 the unit's
    /// [`crate::unit::Unit::dest_county`] is *not* updated, so it walks into the county it
    /// is in while its stored order still names the county arm 2 rejected; and
    /// if the ally case finds nothing, the aim is taken at **county 0**, which
    /// falls through to county 0's anchor. Both are in the decompilation and
    /// neither is tidied here.
    ///
    /// A third is *not* reproduced because it cannot be: the original calls
    /// `FUN_00467F2E` between arms 3 and 4 and **discards the result** — the
    /// two branches it was meant to choose between are identical. A dead call
    /// with no side effect is nothing to reproduce.
    fn mission_seek_enemy(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, my_x, order, here) = (u.owner, u.x, u.dest_county, u.county);
        let ally = self.realms.get(owner as usize).map_or(0, |r| r.ally);

        // 1 — intercept.
        let (d, other) = nearest_enemy_army(&self.campaign.units, unit);
        if d < SEEK_ENEMY_RADIUS {
            if let Some(o) = other.and_then(|o| self.campaign.units.get(o)) {
                if (o.mission != Mission::SEEK_ENEMY || my_x < o.x) && ally != o.owner {
                    let tile = o.tile();
                    if let Some(u) = self.campaign.units.get_mut(unit) {
                        u.needs_destination = false;
                        u.dest = Some(tile);
                    }
                    return true;
                }
            }
        }

        // 2 — the standing order still stands.
        if order != 0
            && self.counties.get(order as usize).is_some_and(|c| c.owner != owner)
            && county_borders_realm(&self.counties, order, owner)
        {
            self.aim_at_county(unit, order);
            return true;
        }

        // 3 — nowhere to attack from here: choose again.
        let here_owner = self.counties.get(here as usize).map_or(0, |c| c.owner);
        if here_owner == owner || !county_borders_realm(&self.counties, here, owner) {
            let next = pick_next_target(
                &self.counties,
                self.county_count,
                &mut self.realms,
                owner,
                here,
            );
            if next == 0 {
                return false;
            }
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.dest_county = next;
            }
            self.aim_at_county(unit, next);
            return true;
        }

        // 4 — attack where it stands. `dest_county` is deliberately left alone.
        let mut target = here;
        if ally == here_owner {
            target = pick_next_target(
                &self.counties,
                self.county_count,
                &mut self.realms,
                owner,
                here,
            );
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.dest_county = target;
            }
        }
        self.aim_at_county(unit, target);
        true
    }

    /// [`Mission::HOLD_HOME`] — `FUN_004A5F0A`.
    ///
    /// The same intercept test as [`Mission::SEEK_ENEMY`] but restricted to
    /// [`crate::unit::Unit::home_county`] and with five times the radius
    /// ([`HOLD_HOME_RADIUS`]) — and, unlike the ally mission, **it always
    /// returns something to do**: nothing to intercept means walk back to the
    /// county it is posted to.
    ///
    /// "Home county" is not always where the army was raised: step 7's
    /// abandonment branch overwrites it with the realm's muster county, which
    /// is what turns a written-off county's levy into a retreat.
    fn mission_hold_home(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, my_x, home) = (u.owner, u.x, u.home_county);
        let ally = self.realms.get(owner as usize).map_or(0, |r| r.ally);
        let (d, other) =
            nearest_attackable_army_in_county(&self.campaign.units, &mut self.realms, unit, home);
        if d < HOLD_HOME_RADIUS {
            if let Some(o) = other.and_then(|o| self.campaign.units.get(o)) {
                if (o.mission != Mission::SEEK_ENEMY || my_x < o.x) && ally != o.owner {
                    let tile = o.tile();
                    if let Some(u) = self.campaign.units.get_mut(unit) {
                        u.needs_destination = false;
                        u.dest = Some(tile);
                    }
                    return true;
                }
            }
        }
        self.aim_at_county(unit, home);
        true
    }

    /// [`Mission::JOIN_GARRISON`] — `FUN_004A6270`.
    ///
    /// Re-check every turn that the castle this army was sent to is still
    /// worth walking to: it must still have room for **all** of the army, be
    /// standing, not be under construction, and still belong to the realm. Any
    /// of those failing sends it to
    /// [`first_castle_with_room`] instead.
    /// army is **disbanded** — its men go back into a county and its weapons
    /// back into the armoury.
    ///
    /// **The original returns 1 even after disbanding**, so its caller then
    /// flood-fills from a record `Army_Destroy` has already cleared. Here the
/// slot is empty and the re-path finds nothing to do.
    /// same observable outcome by a route that cannot read freed memory.
    fn mission_join_garrison(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        let (owner, men, target) = (u.owner, u.men, u.dest_county);
        let ok = self.counties.get(target as usize).is_some_and(|c| {
            let sitting = self.campaign.units.get(c.garrison_unit).map_or(0, |g| g.men);
            c.castle_type != 0
                && !c.castle_ruined
                && c.castle_degraded != crate::siege::CASTLE_DEGRADED_BUILDING
                && c.owner == owner
                && men <= crate::industry::garrison_cap(&self.tables, c.castle_type) - sitting
        });
        let county = if ok {
            target
        } else {
            first_castle_with_room(
                &self.tables,
                &self.counties,
                &self.campaign.units,
                self.county_count,
                owner,
                men,
            )
        };
        if county == 0 {
            let Kingdom { tables, counties, realms, campaign, options, .. } = self;
            let _ = crate::divide::disband(
                tables,
                counties,
                realms,
                &mut campaign.units,
                &mut campaign.names,
                &mut campaign.mercenaries,
                unit,
                options.difficulty,
            );
            report.disbanded.push(unit);
            return true;
        }
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, Aim::Castle);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.dest_county = county;
            u.needs_destination = false;
            u.dest = Some(tile);
        }
        true
    }

    /// [`Mission::GARRISON`] — `FUN_004A60B9`, *"should I still be in here?"*
    ///
    /// A garrison whose county still belongs to its realm does **nothing at
    /// all** — the common case.
    /// nearly always. Once the county is lost, the garrison is turned out
    /// ([`Kingdom::evict_garrison`]) and demoted to [`Mission::SEEK_ENEMY`],
    /// and in the one case where the castle is still standing and unruined it
    /// is also sent straight at the **county town** — i.e. it walks out of the
    /// castle to take the county back.
    fn mission_garrison(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        let (owner, county) = (u.owner, u.dest_county);
        let Some(c) = self.counties.get(county as usize) else { return false };
        if c.owner == owner {
            return false;
        }
        let retake = c.castle_type != 0
            && c.castle_degraded != crate::siege::CASTLE_DEGRADED_BUILDING
            && !c.castle_ruined;
        if !retake {
            self.evict_garrison(unit, county, report);
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = Mission::SEEK_ENEMY;
            }
            return false;
        }
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.mission = Mission::SEEK_ENEMY;
        }
        self.evict_garrison(unit, county, report);
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, Aim::Town);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.needs_destination = false;
            u.dest = Some(tile);
            return true;
        }
        false
    }

    /// `FUN_00437535` — turn a garrison out of its castle onto the nearest
    /// free tile, or destroy it if there is nowhere to stand.
    ///
    /// The link is cleared on **both** sides — the unit's
    /// [`crate::unit::Unit::garrison_county`] and the county's
    /// [`crate::county::County::garrison_unit`] — and a besieger waiting outside is reported
    ///; see [`Eviction`].
    ///
    /// **`[I]` on the search.** The original calls `Map_FindFreeTileNear`,
    /// whose radius this crate has not read; the box walk here is the one
    /// [`crate::levy::muster_tile`] and [`crate::merchant::find_free_road_tile`]
    /// both use, growing 1, 2, 3 around the unit's own tile and taking any
    /// passable unoccupied ground. The choice of tile is not observable in any
    /// rule; whether one is *found* is, and at radius 3 around a castle it
    /// always is on a real map.
    pub fn evict_garrison(&mut self, unit: usize, county: u8, report: &mut MoveReport) {
        let besieger = self.campaign.units.get(unit).and_then(|u| {
            let b = u.besieged_by as usize;
            if b == 0 {
                None
            } else {
                Some(b)
            }
        });
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let spot = free_tile_near(&self.campaign.map, &self.campaign.units, from);
        if let Some(c) = self.counties.get_mut(county as usize) {
            if c.garrison_unit == unit {
                c.garrison_unit = 0;
            }
        }
        match spot {
            Some((x, y)) => {
                if let Some(u) = self.campaign.units.get_mut(unit) {
                    u.x = x;
                    u.y = y;
                    u.garrison_county = 0;
                    u.besieged_by = 0;
                }
                report.evictions.push(Eviction { unit, county, besieger, destroyed: false });
            }
            None => {
                self.campaign.units.remove(unit);
                report.evictions.push(Eviction { unit, county, besieger, destroyed: true });
            }
        }
    }

    /// `FUN_004A64CA` — aim an army at a county, at its castle when that
    /// castle is held and at its town otherwise. See [`aim_for_county`].
    fn aim_at_county(&mut self, unit: usize, county: u8) {
        let aim = aim_for_county(&self.counties, county);
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, aim);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.needs_destination = false;
            u.dest = Some(tile);
        }
    }
}

/// Any free, passable tile within three of `from`. See
/// [`Kingdom::evict_garrison`].
fn free_tile_near(map: &CampaignMap, units: &Units, (ax, ay): (u8, u8)) -> Option<(u8, u8)> {
    for r in 1..=3i32 {
        let (x0, y0) = ((ax as i32 - r).max(0), (ay as i32 - r).max(0));
        let (x1, y1) = ((ax as i32 + r).min(MAP_DIM as i32 - 1), (ay as i32 + r).min(MAP_DIM as i32 - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (x, y) = (x as u8, y as u8);
                if units.at(x, y).is_none() && map.flags_at(x, y) & flags::IMPASSABLE == 0 {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

