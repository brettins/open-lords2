#![allow(unused_imports)]
use super::*;
use super::engine::*;
use super::assault::*;
use crate::county::{County, MAX_COUNTIES};
use crate::math::pct_of;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{Unit, UnitKind, Units, MAX_UNITS};

/// `Army_BeginSiege` (`0x004A7CA2`) — an army lays siege to a county's castle.
///
/// ```c
/// if (county.garrisonUnit != 0 && county.castleRuined == 0
///     && (county.castleDegraded != 1 || county.castleBuilding != 0)
///     && units[county.garrisonUnit].besiegedBy == 0)
///     Siege_Link(unit, county);
/// ```
///
/// **The guard does not check whose garrison it is.** Besieging your own
/// castle is not refused here; the map's hover test is what never offers the
/// order, and `Army_Garrison` is the sibling call for a castle that is yours
/// (`docs/armies.md` §9's target table: *your* county → `Army_Garrison`,
/// *theirs* → `Army_BeginSiege`). Reproduced as the original has it, with the
/// ownership left to the caller that decides which of the two to call.
pub fn begin_siege(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    county: u8,
    season: u8,
) -> Result<(), SiegeRefusal> {
    match units.get(army) {
        Some(u) if u.kind == UnitKind::Army => {}
        _ => return Err(SiegeRefusal::NotAnArmy),
    }
    let Some(c) = counties.get(county as usize) else { return Err(SiegeRefusal::NotAnArmy) };
    if c.garrison_unit == 0 {
        return Err(SiegeRefusal::NoGarrison);
    }
    if c.castle_ruined {
        return Err(SiegeRefusal::CastleRuined);
    }
    if c.castle_degraded == CASTLE_DEGRADED_BUILDING && c.castle_building == 0 {
        return Err(SiegeRefusal::CastleUnderConstruction);
    }
    if units.get(c.garrison_unit).is_some_and(|g| g.besieged_by != 0) {
        return Err(SiegeRefusal::AlreadyBesieged);
    }
    link(t, counties, realms, units, army, county, season);
    Ok(())
}

/// `Siege_Link` (`0x004A7E0A`) — the two back-pointers, then [`prepare`].
///
/// The garrison's `+0x19A` and the besieger's `+0x199` are the pair
/// [`crate::unit::destroy`] already knows how to unpick, and
/// [`validate_link`] is what notices when they stop agreeing.
pub fn link(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    county: u8,
    season: u8,
) {
    let garrison = counties[county as usize].garrison_unit;
    if let Some(u) = units.get_mut(army) {
        u.needs_destination = true;
        u.besieging_county = county;
    }
    if let Some(g) = units.get_mut(garrison) {
        g.besieged_by = army as u8;
    }
    prepare(t, counties, realms, units, army, season);
}

/// `Siege_Prepare` (`0x004A7EB5`) — clear the three build records, and for an
/// **AI** owner write the order its lord's doctrine names.
///
/// # The doctrine field is `personality +0xA0`, and it was untraced until now
///
/// `docs/diplomacy.md` §8.4 lists eleven fields of the 240-byte personality
/// record that *"hold plausible per-lord values and were never traced"*, and
/// `+0xA0` is one of them. This function is its only reader. Read out of
/// `Lords2.exe` at `0x004D8A58 + (lord-1)*0xF0 + 0xA0`, the four values are
///
/// | lord | `+0xA0` | orders |
/// |---|---:|---|
/// | 1 the Knight | **8** | 4 siege towers |
/// | 2 the Baron | **9** | 1 battering ram, and the default 2 towers |
/// | 3 the Countess | **7** | 3 catapults, the default 2 towers, and a ram against a stone or royal castle after season 2 |
/// | 4 the Bishop | **7** | the same |
///
/// **[V]** — three of the four values are exactly the three constants the
/// function tests and the fourth repeats one of them, which a field that meant
/// something else would not do. Two consequences fall out and both are worth
/// stating: the *"default: 2 towers"* arm is **unreachable for every shipped
/// lord**
///
/// # The orders are cumulative, not alternative
///
/// The tower count is written **before** the personality tests and only the
/// `== 8` arm overwrites it, so the Countess builds 3 catapults *and* 2 towers
/// — 600 + 400 = **1,000 man-seasons**, not 600. `docs/armies.md` §4 already
/// said so; this is where it is enforced.
pub fn prepare(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    season: u8,
) {
    let Some(u) = units.get_mut(army) else { return };
    u.siege_seasons_left = 0;
    u.engines = [EngineBuild::default(); 3];
    if u.owner_is_human {
        return;
    }
    let county = u.besieging_county;
    let owner = u.owner;
    let castle_type = counties.get(county as usize).map_or(0, |c| c.castle_type);
    let doctrine = realms
        .get(owner as usize)
        .and_then(|r| siege_doctrine(t, r.lord))
        .unwrap_or(0);

    u.engines[Engine::SiegeTower.index()].ordered = 2;
    if doctrine == 8 {
        u.engines[Engine::SiegeTower.index()].ordered = 4;
    }
    if doctrine == 9 {
        u.engines[Engine::BatteringRam.index()].ordered = 1;
    }
    if doctrine == 7 {
        u.engines[Engine::Catapult.index()].ordered = 3;
        if castle_type > 3 && season > 2 {
            u.engines[Engine::BatteringRam.index()].ordered = 1;
        }
    }
    recompute_build_time(units, army);
}

/// `Siege_ValidateLink` (`0x004A8426`) — break a besieger's link when the
/// castle it is sitting outside no longer has the garrison it was pointed at.
///
/// Two clauses
/// garrison slot must still hold a unit *and* that unit must still name this
/// county as the castle it is inside. A garrison that marched out leaves the
/// first test passing and the second failing.
pub fn validate_link(counties: &[County; MAX_COUNTIES], units: &mut Units, army: usize) {
    let Some(county) = units.get(army).map(|u| u.besieging_county) else { return };
    let garrison = counties.get(county as usize).map_or(0, |c| c.garrison_unit);
    let still_inside = units.get(garrison).is_some_and(|g| g.garrison_county == county);
    if garrison == 0 || !still_inside {
        if let Some(u) = units.get_mut(army) {
            u.besieging_county = 0;
        }
    }
}

/// `Siege_StartPhase` (`0x004A82B9`) — the first call of turn phase 2.
///
/// Breaks every garrison/besieger pair that no longer agrees, runs
/// [`validate_link`] over every besieging army, and seeds the cursor at 1.
///
/// **It does not call [`prepare`].** `docs/armies.md` §4 used to say it
/// *"re-prepares every besieging army"*; it breaks stale links, and re-preparing
/// would wipe the player's engine order every season.
pub fn start_phase(counties: &[County; MAX_COUNTIES], units: &mut Units) -> SiegeCursor {
    // The county sweep runs 1..=16 in the original — `MAX_COUNTY_ID`, not the
    // map's county count.
    for county in 1..=crate::county::MAX_COUNTY_ID {
        let garrison = counties[county as usize].garrison_unit;
        if garrison == 0 {
            continue;
        }
        let besieger = units.get(garrison).map_or(0, |g| g.besieged_by) as usize;
        if besieger == 0 {
            continue;
        }
        let agrees = units.get(besieger).is_some_and(|b| b.besieging_county == county);
        if !agrees {
            if let Some(g) = units.get_mut(garrison) {
                g.besieged_by = 0;
            }
            if let Some(b) = units.get_mut(besieger) {
                b.besieging_county = 0;
            }
        }
    }

    let mut count = 0;
    for id in 1..=MAX_UNITS {
        let besieging = units
            .get(id)
            .is_some_and(|u| u.owner != 0 && u.kind == UnitKind::Army && u.besieging_county != 0);
        if besieging {
            validate_link(counties, units, id);
            // `g_siegeCount` is incremented unconditionally inside
            // `Siege_ValidateLink` — **including for the link it just broke**.
            // Reproduced: the count is "armies that were besieging when the
            // phase began"
            // above 1.
            count += 1;
        }
    }
    SiegeCursor { at: 1, count }
}

/// `Siege_TickPhase` (`0x004A84BA`) — the pump.
///
/// Walks the cursor from where it stands to [`MAX_UNITS`], calling
/// [`build_tick`], and **stops on the first army whose engines came in**,
/// leaving the cursor on it. Returns that army; `None` means the sweep is
/// exhausted and the phase is over.
///
/// The caller runs [`assault`] on the army returned and calls this again. The
/// cursor is deliberately *not* advanced past it: the assault clears the siege
/// link one way or another, so the next call's [`build_tick`] on the same slot
/// returns false and the cursor moves on by itself.
pub fn tick_phase(cursor: &mut SiegeCursor, units: &mut Units) -> Option<usize> {
    while cursor.at <= MAX_UNITS {
        if build_tick(units, cursor.at) {
            return Some(cursor.at);
        }
        cursor.at += 1;
    }
    None
}

/// `Siege_Break` (`0x0043B917`) — lift a siege.
///
/// Three callers, and together they are the whole answer to *"the lift-siege
/// handler was not traced"*:
///
/// * **any successful move order** on a type-1 unit (`Unit_OrderMove`), so
///   giving a besieging army anywhere to go lifts its siege — `L2.eng` 10/13
///   *"Lift the siege?"* is a confirmation, not a veto;
/// * the siege-preparation screen's *"Lift siege"* button, `L2.eng` 83/6;
/// * [`assault`], when the castle is too strong to storm without engines.
pub fn break_siege(counties: &[County; MAX_COUNTIES], units: &mut Units, army: usize) {
    let Some(county) = units.get(army).map(|u| u.besieging_county) else { return };
    if county == 0 {
        return;
    }
    let garrison = counties.get(county as usize).map_or(0, |c| c.garrison_unit);
    if let Some(u) = units.get_mut(army) {
        u.besieging_county = 0;
    }
    if let Some(g) = units.get_mut(garrison) {
        g.besieged_by = 0;
    }
}

/// The siege-preparation screen's `+` and `−` buttons — `0x0043B681` and
/// `0x0043B741`, both of which end in `0x0043B7C4`.
///
/// `delta` is `+1` or `−1`; anything else is refused. Returns true if the order
/// changed, in which case [`recompute_build_time`] has already run and the
/// caller should redraw. The caps are [`ENGINE_ORDER_CAP`].
pub fn order_engine(units: &mut Units, army: usize, engine: Engine, delta: i16) -> bool {
    let cap = ENGINE_ORDER_CAP[engine.index()];
    let Some(u) = units.get_mut(army) else { return false };
    let current = u.engines[engine.index()].ordered;
    let ok = match delta {
        1 => current < cap,
        -1 => current > 0,
        _ => false,
    };
    if !ok {
        return false;
    }
    u.engines[engine.index()].ordered = current + delta;
    recompute_build_time(units, army);
    true
}

/// **Is this county's garrison under siege?** — the Readme's *Besieged Castles
/// (p.87)* rule, and the guard `Army_Garrison` needs.
///
/// *"When one of your castles is under siege, you may only leave the castle to
/// engage the sieging force, and you may not enter the castle or strengthen the
/// garrison until the siege is lifted."* `L2.eng` 289 is the refusal, and
/// `docs/armies.md` §9's target table already paired that string with this
/// condition; the Readme is the second source that makes it **[V]**.
pub fn garrison_is_besieged(counties: &[County; MAX_COUNTIES], units: &Units, county: u8) -> bool {
    counties
        .get(county as usize)
        .map(|c| c.garrison_unit)
        .and_then(|g| units.get(g))
        .is_some_and(|g| g.besieged_by != 0)
}

/// **May this garrison move at all?** The other half of *Besieged Castles
/// (p.87)*: a besieged garrison *"may only leave the castle to engage the
/// sieging force"*.
///
/// Returns the besieger's slot when the garrison is pinned — the one
/// destination a sortie may have. `None` means the garrison is not besieged and
/// is free to march.
///
/// **`[I]`, and marked so.** The Readme states the rule and nothing in the
/// decompiled mover was found to enforce it: `Unit_OrderMove` calls
/// [`break_siege`] on the *besieger*, not on the garrison, and no guard on
/// `garrison_county` with a live `besieged_by` was located. So this is the
/// game's own documentation implemented in the absence of the code that does
/// it, and it is a function a caller must choose to call
/// wired into movement.
pub fn sortie_target(units: &Units, garrison: usize) -> Option<usize> {
    let besieger = units.get(garrison)?.besieged_by;
    (besieger != 0).then_some(besieger as usize)
}

