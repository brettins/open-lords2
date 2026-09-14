#![allow(unused_imports)]
use super::*;
use super::basket::*;
use super::tests::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use crate::unit::{ArmyNames, TroopType, Unit, UnitKind, Units, TROOP_TYPES};

/// `Levy_SetPercent` (`0x00435EBC`) — what taking `percent` of a county costs.
///
/// ```c
/// men  = Pct(county.population, pct);
/// cost = g_armyHappinessCost[pct];
/// if (pct != 0) cost += county.levySurcharge;
/// if (cost > 100) cost = 100;
/// if (cost < 0)   cost = 0;
/// if (county.happiness < 1) { men = 0; cost = 0; }
/// else while (county.happiness - cost < 1) { pct--; ...recompute... }
/// ```
///
/// > **Three corrections to `docs/armies.md` §6.1.**
/// >
/// > 1. Its bare `clamp 0 .. 100;` is ambiguous and reads as if the
/// >    *percentage* is clamped. **The clamp is on the happiness cost, and it
/// >    is applied after the surcharge is added.** The percentage is clamped by
/// >    the slider handler, not here.
/// > 2. The pseudocode **omits the `happiness < 1` early-out**, which is the
/// >    walk-back's only termination guard. Without it a county at zero
/// >    happiness would walk `pct` negative and index the table at −1 forever.
/// > 3. It says *"in a county at happiness 100 with no surcharge the largest
/// >    levy is 59 % (cost 98)"*. The percentage is right and **the cost is
/// > 99** — the table's index 58 is 98 and index 59 is 99,
/// >    `happiness - cost >= 1`, so 99 is exactly affordable at happiness 100.
/// >
/// > Its reasoning about the saturation is also wrong even though the
/// > conclusion survives: *"101 … exceeds any possible happiness, so the
/// > walk-back loop always fires"*. 101 is clamped to 100 before the
/// > comparison, and `100 - 100 = 0 < 1` is what fires the loop. `[V]`
///
/// County `+0x2F4`, the surcharge, is set to 15 by [`create_army`] and **decays
/// by 5 a season** in `Happiness_UpdateAll` — so it is gone after three
/// seasons. §6.1 records that as untraced; see
/// [`crate::happiness::decay_levy_surcharge`].
pub fn set_percent(t: &Tables, county: &County, percent: i32) -> Levy {
    let cost_of = |p: i32| {
        let mut cost = t.army_happiness_cost(p);
        if p != 0 {
            cost += county.levy_surcharge;
        }
        cost.clamp(0, crate::tables::LEVY_COST_MAX)
    };

    let mut pct_taken = percent;
    let mut men = pct(county.population, pct_taken);
    let mut cost = cost_of(pct_taken);

    if county.happiness < 1 {
        return Levy { men: 0, happiness_cost: 0, settled: pct_taken };
    }
    while county.happiness - cost < 1 {
        pct_taken -= 1;
        men = pct(county.population, pct_taken);
        cost = cost_of(pct_taken);
    }
    Levy { men, happiness_cost: cost, settled: pct_taken }
}

/// The two size refusals, which a **mercenary hire bypasses entirely** because
/// the band supplies the men.
///
/// `[V]` — `hireMercs` is `&&`-ed into both guards.
pub fn refuse_levy(men: i32, hiring_mercenaries: bool) -> Option<LevyRefusal> {
    if hiring_mercenaries {
        return None;
    }
    if men == 0 {
        Some(LevyRefusal::NoMen)
    } else if men < crate::tables::ARMY_MIN_MEN {
        Some(LevyRefusal::TooFew)
    } else {
        None
    }
}

/// `County_FindFreeRoadTile` (`0x00428007`), then `County_FindFreeOpenTile`
/// (`0x00428078`) — where a new army is put.
///
/// **Correction C47.** This used to scan the whole 64 × 64 map row-major for
/// the first free road tile *of the county*, and that is not what the original
/// does. Both finders are a **box search around the county's anchor tile**, at
/// radius 1, then 2, then 3, and neither of them looks at the county id at all:
///
/// ```c
/// County_FindFreeRoadTile(county):
///     for r in 1..=3: if Map_FindFreeRoadTileNear(county.anchorX, county.anchorY, r) return 1;
///     return 0;
/// Map_FindFreeRoadTileNear(x, y, r):          /* 0x0046CFBD */
///     scan the (2r+1)² box from (x−r, y−r), clipped to [0, 64), row-major;
///     accept the first tile with  tile.unit == 0  &&  (tile.flags & 0x01);
/// Map_FindFreeOpenTileNear(x, y, r):          /* 0x0046D130 */
///     the same box; accept  tile.unit == 0  &&  (tile.flags & 0xFD) == 0;
/// ```
///
/// The difference is not academic and it is the whole of the bug a player
/// reported as *"I raised an army and nothing appeared on the map"*. A county
/// is tens of tiles across, the near zoom shows **eight lattice columns**, and
/// the old scan handed back the county's lowest-indexed road tile — which on
/// the England fixture is fourteen columns from the county's anchor and off
/// the side of the screen. The original never places an army more than three
/// tiles from the county's centre, so it is always in shot.
///
/// Two further facts fall out of the quoted code and both are the original's:
///
/// * **The county is never tested.** A county whose anchor sits near a border
/// can raise its army onto a *neighbour's* tile.
/// * **The open-ground fallback is stricter than "passable"**: `& 0xFD == 0`
/// admits bare ground
///   not farmland, not a settlement, not rough ground, not a road. The road
///   pass is the only way an army lands on a road.
///
/// `anchor` is `County +0x6C`/`+0x6D`.
pub fn muster_tile(map: &CampaignMap, units: &Units, anchor: (u8, u8)) -> Option<(u8, u8)> {
    // A road tile: plane-0 bit 0x01 set, whatever else it carries.
    if let Some(at) = search_near(map, units, anchor, |f| f & flags::ROAD != 0) {
        return Some(at);
    }
    // Open ground: nothing set but, at most, the county boundary.
    search_near(map, units, anchor, |f| f & !flags::BOUNDARY == 0)
}

/// The `for r in 1..=3` box walk both finders share, with the predicate that
/// tells them apart. Row-major inside each box and radius-ascending between
/// them, which is the original's order and therefore reproducible.
fn search_near(
    map: &CampaignMap,
    units: &Units,
    (ax, ay): (u8, u8),
    accept: impl Fn(u8) -> bool,
) -> Option<(u8, u8)> {
    let dim = crate::map::MAP_DIM as i32;
    for r in 1..=MUSTER_RADIUS {
        let (x0, y0) = ((ax as i32 - r).max(0), (ay as i32 - r).max(0));
        let (x1, y1) = ((ax as i32 + r).min(dim - 1), (ay as i32 + r).min(dim - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (x, y) = (x as u8, y as u8);
                if units.at(x, y).is_none() && accept(map.flags_at(x, y)) {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

/// `Army_Create` (`0x004A9A9A`) — put a levy on the map.
///
/// The order of operations, which is the rule:
///
/// ```c
/// tile = County_FindFreeRoadTile(county) || County_FindFreeOpenTile(county);
/// if (!tile) return 0;                                  /* message 0xDD */
/// Unit_Spawn(1, tile.x, tile.y, realm);                 /* zeroes the record */
/// u.isPlayerDriven = u.needsDestination = 1;
/// u.ownerIsHuman   = realm.isHuman;
/// u.county = u.homeCounty = county;
/// u.yearFormed = g_year;
/// u.morale     = county.happiness;                      /* BEFORE the debit */
/// if (realm == 0) { u.shield = 0; u.owner = 6; } else u.shield = realm.shield;
/// Levy_DebitPopulation(county);                         /* pop -= total; army -= total */
/// u.men = basket[7].chosen;  u.troops[t] = basket[t].chosen;
/// u.nameIndex = Army_PickName(realm);
/// Levy_ConsumeWeapons(realm);
/// if (hireMercs) Mercenary_Hire(unit, county.mercOffer);
/// Army_RecountCountyTroops();  Wages_ForUnit(unit);
/// ...the county's food passes, twice...
/// if (county.happiness < cost) { shownArmy -= happiness; happiness = 0; }
/// else                        { happiness -= cost; shownArmy -= cost; }
/// county.levySurcharge = 15;
/// realm.wages = Wages_ForRealm(realm);
/// ```
///
/// > **Three corrections to `docs/armies.md` §6.3, all `[V]`.**
/// >
/// > 1. **`morale` is the county's happiness *before* the levy cost is
/// > deducted.** It is the fifth write after the spawn
/// > nearly last,
/// >    morale 80. The document's pseudocode lists them in the other order.
/// > 2. **The happiness debit is clamped.** §6.3 renders it as a flat
/// >    `happiness -= cost`. When the county cannot afford the full cost its
/// > happiness goes to 0
/// >    taken, so the two always agree. This is reachable: the AI paths pass an
/// >    unclamped cost.
/// > 3. **`Army_Create` sets neither `moveAllowance` nor `movesUsed`.**
/// > `Unit_Spawn` memsets the whole `0x1A4`-byte record,
/// >    an allowance of **0** until the next tick, when `Army_Tick` writes 15.
/// >    Reproduced by [`crate::unit::Unit::new`] setting the allowance from the
/// >    kind — the one-frame stale 0 is a rendering artefact of the original's
/// >    frame loop and not a rule.
///
/// The **two food passes really do run twice**
/// slip: `Food_Available` is recomputed from the ration pass's per-season caps,
/// so the army's own foraging has to be settled before the happiness is
/// charged.
///
/// Returns the new unit's slot.
#[allow(clippy::too_many_arguments)]
pub fn create_army(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    basket: &LevyBasket,
    muster: Muster,
    explored: &mut crate::explore::Explored,
) -> Result<usize, LevyRefusal> {
    // `County_FindFree*Tile` searches around the county's **anchor**, not over
    // the county — C47, and see [`muster_tile`].
    let anchor = counties
        .get(muster.county as usize)
        .map_or((0, 0), |c| (c.anchor_x, c.anchor_y));
    let (x, y) = muster_tile(map, units, anchor).ok_or(LevyRefusal::NowhereToStand)?;
    // **`Unit_Spawn`'s own precondition**, which is stricter than the road
    // finder's: `(tile.flags & 0xFC) == 0`. A road tile that also carries
    // farmland, rough ground, a plot or a settlement passes
    // `Map_FindFreeRoadTileNear` and then fails the spawn, and `Army_Create`
    // returns 0 — message `0xDD`, the same refusal as nowhere to stand.
    if map.flags_at(x, y) & !(flags::ROAD | flags::BOUNDARY) != 0 {
        return Err(LevyRefusal::NowhereToStand);
    }

    let (is_human, shield) = realms
        .get(muster.realm as usize)
        .map_or((false, 0), |r| (r.is_human, r.shield_index));
    let county_happiness = counties
        .get(muster.county as usize)
        .map_or(0, |c| c.happiness);

    let mut unit = Unit::new(UnitKind::Army, muster.realm, x, y);
    unit.player_driven = true;
    unit.needs_destination = true;
    unit.owner_is_human = is_human;
    unit.county = muster.county;
    unit.home_county = muster.county;
    unit.year_formed = muster.year;
    unit.morale = county_happiness;
    if muster.realm == 0 {
        unit.owner = OWNERLESS;
        unit.shield = 0;
    } else {
        unit.shield = shield;
    }
    unit.men = basket.total();
    unit.troops = basket.troops();
    unit.name_index = names.pick(muster.realm);

    let id = units.spawn(unit).ok_or(LevyRefusal::NowhereToStand)?;

    if let Some(county) = counties.get_mut(muster.county as usize) {
        // `Levy_DebitPopulation` (`0x004A9F18`). County `+0x38` is the
        // population panel's *"Army"* line, a display accumulator that
        // `Population_UpdateAll` zeroes every season, so this half is
        // transient; `+0x24` is the real population.
        county.population -= basket.total();
        county.army -= basket.total();

        // The clamp §6.3 leaves out.
        if county.happiness < muster.happiness_cost {
            county.shown_army -= county.happiness;
            county.happiness = 0;
        } else {
            county.happiness -= muster.happiness_cost;
            county.shown_army -= muster.happiness_cost;
        }
        county.levy_surcharge = crate::tables::LEVY_SURCHARGE;
    }
    if let Some(realm) = realms.get_mut(muster.realm as usize) {
        basket.consume_weapons(realm);
    }
    let realms_snapshot: [Realm; MAX_REALMS] = realms.clone();
    units.recount_county_troops(counties, &realms_snapshot);
    crate::unit::refresh_wages(t, units, realms, muster.realm, 0);
    // `Army_Create`'s last act before it returns 1:
    //
    // ```c
    // if (g_localPlayer == realm) { FUN_0046E067(unit.x, unit.y, 6); Gfx_MarkAllDirty(); }
    // ```
    //
    // The new army sees its 13 × 13 square — for the realm that raised it, and
    // for nobody when `realm == 0` raised an ownerless militia.
    // `crate::explore`.
    explored.reveal_square(muster.realm, x as i32, y as i32, crate::explore::ARMY_SIGHT);
    Ok(id)
}

/// `FUN_004A50AE` — raise a county's defence force in the face of an invader.
///
/// This is the function `docs/armies.md` never names, and it is what turns
/// walking onto a county into a fight
/// percentage of the county's population, equips it according to who owns the
/// county, and calls `Army_Create`.
///
/// Returns the new unit, or `None` when the county has fewer than
/// [`DEFENCE_MIN_POPULATION`] people — in which case
/// [`crate::conquest::attack_county`] captures outright.
#[allow(clippy::too_many_arguments)]
pub fn raise_defence(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    county: u8,
    percent: i32,
    mode: Defence,
    year: i32,
    explored: &mut crate::explore::Explored,
) -> Option<usize> {
    let c = counties.get(county as usize)?;
    if c.population < DEFENCE_MIN_POPULATION {
        return None;
    }
    let realm = c.owner;
    let men = pct(c.population, percent);
    let mut happiness_cost = t.army_happiness_cost(percent);

    let mut basket = LevyBasket::seed(realms.get(realm as usize)?, men);
    match mode {
        Defence::HumanCounty => {}
        Defence::AiCounty => basket.auto_equip(),
        Defence::Militia => {
            // The grant. Realm 0 is nobody's, so this is bookkeeping rather
            // than a gift to a player, but it is what the original writes and
            // it is what the ladder spends.
            for troop in [TroopType::Pikeman, TroopType::Archer, TroopType::Maceman] {
                let slot = troop.weapon_slot().expect("all three are equipped types") + 1;
                basket.slots[slot].available = MILITIA_GRANT;
                basket.slots[slot].remaining = MILITIA_GRANT;
            }
            happiness_cost = 0;
            if let Some(&(_, archers, pikes, maces)) =
                MILITIA_LADDER.iter().find(|&&(floor, ..)| men >= floor)
            {
                basket.slots[TroopType::Archer.index()].chosen = archers;
                basket.slots[TroopType::Pikeman.index()].chosen = pikes;
                basket.slots[TroopType::Maceman.index()].chosen = maces;
                basket.slots[0].chosen -= archers + pikes + maces;
            }
        }
    }

    create_army(
        t,
        map,
        counties,
        realms,
        units,
        names,
        &basket,
        Muster { realm, county, happiness_cost, year },
        explored,
    )
    .ok()
}

