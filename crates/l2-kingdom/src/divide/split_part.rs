#![allow(unused_imports)]
use super::*;
use super::disband_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::mercenary::MercenaryBands;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{
    ArmyNames, Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNIT_ID, TROOP_TYPES,
};

// ------------------------------------------------------------------ the split

/// `FUN_0046733C` — a free tile within five of `(x, y)`, road preferred.
///
/// The original's shape exactly: for `r` = 1…5 it asks
/// `Map_FindFreeRoadTileNear(x, y, r + 1)` and then
/// `Map_FindFreeOpenTileNear(x, y, r)`, so at every radius a road one ring
/// wider beats open ground one ring narrower. Each of those scans the clipped
/// `(2r+1)²` box **row-major, ascending y then x**, which is what makes the
/// answer reproducible; `docs/netcode.md` §3 is why that is written down rather
/// than left to whatever order a search happened to have.
pub fn free_tile_near(map: &CampaignMap, units: &Units, x: u8, y: u8) -> Option<(u8, u8)> {
    let cost = map.cost_map();
    let scan = |r: i32, road: bool| -> Option<(u8, u8)> {
        let dim = crate::map::MAP_DIM as i32;
        let (x0, y0) = ((x as i32 - r).max(0), (y as i32 - r).max(0));
        let (x1, y1) = ((x as i32 + r + 1).min(dim), (y as i32 + r + 1).min(dim));
        for ty in y0..y1 {
            for tx in x0..x1 {
                let (tx, ty) = (tx as u8, ty as u8);
                if units.at(tx, ty).is_some() {
                    continue;
                }
                if road {
                    if map.has(tx, ty, flags::ROAD) {
                        return Some((tx, ty));
                    }
                } else if cost.at(tx, ty) != 0 {
                    return Some((tx, ty));
                }
            }
        }
        None
    };
    for r in 1..=SPLIT_SEARCH_RADIUS {
        if let Some(t) = scan(r + 1, true) {
            return Some(t);
        }
        if let Some(t) = scan(r, false) {
            return Some(t);
        }
    }
    None
}

/// The gate `FUN_00437AFB` applies before it calls `Army_Split`, on its own.
///
/// Split out so the screen can grey a button with the same rule the confirm
/// enforces
pub fn refuse_split(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    units: &Units,
    basket: &SplitBasket,
    into: SplitInto,
) -> Option<SplitRefusal> {
    let (parent, daughter) = (basket.parent_total(), basket.daughter_total());
    if parent == 0 || daughter == 0 {
        return Some(SplitRefusal::Empty);
    }
    let (min, cap) = match into {
        SplitInto::Field => (crate::tables::ARMY_MIN_MEN, SPLIT_NO_CASTLE_CAP),
        SplitInto::Castle { county, .. } => {
            let c = counties.get(county as usize);
            let room = c.map_or(0, |c| {
                let cap = crate::industry::garrison_cap(t, c.castle_type);
                match units.get(c.garrison_unit) {
                    Some(g) => cap - g.men,
                    None => cap,
                }
            });
            (0, room)
        }
    };
    if parent < min || daughter < min {
        return Some(SplitRefusal::TooFew);
    }
    if cap < daughter {
        return Some(SplitRefusal::GarrisonFull(cap));
    }
    None
}

/// **`Army_Split` (`0x00437FD7`)** — the daughter army leaves.
///
/// ```c
/// if (!FindFreeTileNear(parent.x, parent.y)) return;
/// d = Unit_Spawn(1, foundX, foundY, realm);   if (!d) return;
/// d.needsDestination = 1;  d.ownerIsHuman = realm.isHuman;
/// d.county = parent.county;  d.yearFormed = g_year;
/// d.morale = parent.morale;  d.shield = parent.shield;
/// d.nameIndex = Army_PickName(realm);
/// d.troops[0..7] = daughterBasket;  parent.troops[0..7] = parentBasket;
/// d.men = sum(daughterBasket[0..8]);  parent.men = sum(parentBasket[0..8]);
/// if (parent.besiegingCounty) Siege_RecomputeBuildTime(parent);
/// d.moving = parent.moving = 2;  d.playerDriven = 1;
/// if (destCounty == 0) { d.movesUsed += 5; parent.movesUsed += 5; }
/// else                 { d.dest = tile; FloodFill; ExtractPath; CopyToUnit; }
/// if (parent has a band && the band is in the daughter's column) the band moves whole;
/// ```
///
/// Four things a reimplementation gets wrong by default.
/// instruction stream:
///
/// 1. **`men` is the sum of all eight slots**, so an army whose whole strength
///    is its mercenary band still has the right total.
/// 2. **The daughter inherits the parent's morale**, not the county's happiness
/// — this is not `Army_Create`.
/// 3. **Both halves are set walking** (`moving = 2`) even on the plain split,
///    with no path; the stepper finds nothing to do and they stand still. It is
///    the same value `Unit_OrderMove` writes, and reproducing it matters because
///    the phase waits read it.
/// 4. **The daughter does not inherit the home county.** `Army_Split` writes
///    `county` and never writes `homeCounty`, and `Unit_Spawn` has just zeroed
/// the record — so a split army's county of origin is **0**. That is not
///    lost men: [`disband_county`] falls through to the county the army is
///    standing in whenever the home county is not the owner's, and county 0
///    never is. The visible effect is that **a daughter army always disbands
///    where it stands**, never to the county its parent was raised in.
/// Reproduced
///    the fallback makes it harmless. `[D]`
///
/// The parent keeps its siege and garrison links: neither is copied.
#[allow(clippy::too_many_arguments)]
pub fn split(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    bands: &mut MercenaryBands,
    army: usize,
    basket: &SplitBasket,
    into: SplitInto,
    year: i32,
) -> Result<usize, SplitRefusal> {
    let parent = units.get(army).filter(|u| u.kind == UnitKind::Army).ok_or(SplitRefusal::NotAnArmy)?;
    if parent.moves_used >= 1 {
        return Err(SplitRefusal::AlreadyMoved);
    }
    if let Some(no) = refuse_split(t, counties, units, basket, into) {
        return Err(no);
    }
    let parent = units.get(army).expect("checked just above").clone();
    let (x, y) = free_tile_near(map, units, parent.x, parent.y).ok_or(SplitRefusal::NowhereToStand)?;

    let is_human = realms.get(parent.owner as usize).is_some_and(|r| r.is_human);
    let mut daughter = Unit::new(UnitKind::Army, parent.owner, x, y);
    daughter.needs_destination = true;
    daughter.owner_is_human = is_human;
    daughter.player_driven = true;
    daughter.county = parent.county;
    daughter.year_formed = year;
    daughter.morale = parent.morale;
    daughter.shield = parent.shield;
    daughter.name_index = names.pick(parent.owner);
    daughter.troops = basket.daughter;
    daughter.men = basket.daughter_total();
    daughter.moving = true;
    let id = units.spawn(daughter).ok_or(SplitRefusal::NowhereToStand)?;

    {
        let parent = units.get_mut(army).expect("still there");
        parent.troops = basket.parent;
        parent.men = basket.parent_total();
        parent.moving = true;
    }
    // The band crosses whole.
    if let (Some(band), true) = (basket.mercenaries, basket.mercenaries_leave) {
        if let Some(p) = units.get_mut(army) {
            p.mercenaries = None;
        }
        if let Some(d) = units.get_mut(id) {
            d.mercenaries = Some(band);
        }
        let mut live = bands.band_raw(band.band as usize);
        live.hired_by = id as u16;
        bands.set_band_raw(band.band as usize, live);
    }
    if units.get(army).is_some_and(|u| u.besieging_county != 0) {
        crate::siege::recompute_build_time(units, army);
    }

    match into {
        SplitInto::Field => {
            for slot in [army, id] {
                if let Some(u) = units.get_mut(slot) {
                    u.moves_used += SPLIT_MOVE_COST;
                }
            }
        }
        SplitInto::Castle { tile, .. } => {
            // The daughter walks in. A failed extraction leaves it standing,
            // which is `Unit_OrderMove`'s own behaviour — every write is inside
            // the `if`. `docs/armies.md` §2.3.
            crate::movement::order_move(map, units, id, tile, crate::movement::Routing::Direct);
        }
    }
    let snapshot: [Realm; MAX_REALMS] = realms.clone();
    units.recount_county_troops(counties, &snapshot);
    crate::unit::refresh_wages(t, units, realms, parent.owner, 0);
    Ok(id)
}

