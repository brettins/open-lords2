#![allow(unused_imports)]
use super::*;
use super::lifecycle::*;
use super::engine::*;
use crate::county::{County, MAX_COUNTIES};
use crate::math::pct_of;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{Unit, UnitKind, Units, MAX_UNITS};

impl SiegeScars {
    pub fn any(&self) -> bool {
        self.moat_filled != 0 || self.wall_damage != 0
    }
}

pub const REPAIR_WOOD_PER_WALL: i32 = 10;
pub const REPAIR_STONE_PER_WALL: i32 = 15;
pub const REPAIR_WORK_PER_WALL: i32 = 15;
pub const REPAIR_WORK_PER_MOAT: i32 = 5;

/// **Bill the repair** — `Siege_RecordCastleDamage` (`0x004784CA`), called from
/// `Battle_ReturnToCampaign`'s siege arm and the **only** writer of
/// [`CASTLE_DEGRADED_DAMAGED`] in the whole binary.
///
/// ```c
/// if (!g_battleIsSiege || (DAT_0057A0D8 == 0 && DAT_0056D648 == 0)) return;
/// county[+0x1E4 .. +0x1F1] = the six numbers the battle finished with;
/// if (g_castleLevel < 2) { woodOwed  += wallDamage * 10; woodTotal  += same; }
/// else                   { stoneOwed += wallDamage * 15; stoneTotal += same; }
/// workLeft += moatFilled * 5 + wallDamage * 15;  workTotal += same;
/// county.castleLevelLeft = g_castleLevel;
/// county.castleDegraded  = 2;
/// county.castlePercent   = 0;
/// ```
///
/// > **`docs/symbols.md` calls `DAT_0057A0D8` `breachDamage` and that is a
/// > misnomer** — the whole binary holds three writers of it and the only one
/// > that adds is the moat fill. The parameter is named for what writes it.
pub fn record_castle_damage(county: &mut County, castle_level: u8, scars: SiegeScars) -> bool {
    if !scars.any() {
        return false;
    }
    let already_building = county.castle_degraded == CASTLE_DEGRADED_BUILDING;
    let add = |slot: &mut i32, amount: i32| {
        *slot = if already_building { *slot + amount } else { amount };
    };

    county.siege_scars = scars;

    let wall = scars.wall_damage as i32;
    if castle_level < 2 {
        let bill = wall * REPAIR_WOOD_PER_WALL;
        add(&mut county.castle_wood_owed, bill);
        add(&mut county.castle_wood_total, bill);
    } else {
        let bill = wall * REPAIR_STONE_PER_WALL;
        add(&mut county.castle_stone_owed, bill);
        add(&mut county.castle_stone_total, bill);
    }
    let work = scars.moat_filled as i32 * REPAIR_WORK_PER_MOAT + wall * REPAIR_WORK_PER_WALL;
    add(&mut county.castle_work_left, work);
    add(&mut county.castle_work_total, work);

    county.castle_level_left = castle_level;
    county.castle_degraded = CASTLE_DEGRADED_DAMAGED;
    county.castle_percent = 0;
    true
}

/// **The other half of the round trip** — `FUN_004787A4` (`0x004787A4`), which
/// `Battle_Start` runs on the way *into* an assault.
///
/// ```c
/// if (county.castleDegraded == 2) { the six globals = county[+0x1E4 .. +0x1F1]; }
/// else                            { county[+0x1E4 .. +0x1F1] = 0; }
/// ```
pub fn scars_for_assault(county: &mut County) -> SiegeScars {
    if county.castle_degraded == CASTLE_DEGRADED_DAMAGED {
        county.siege_scars
    } else {
        county.siege_scars = SiegeScars::default();
        SiegeScars::default()
    }
}

/// ```c
/// if (degraded == 1 && castleBuilding != 0) level = castleBuilding - 1;
/// else if (degraded == 2)                   level = county.castleLevelLeft;   /* +0x1F9 */
/// else                                      level = castleType - 1;
/// ```
pub fn assault_castle_level(county: &County) -> u8 {
    if county.castle_degraded == CASTLE_DEGRADED_BUILDING && county.castle_building != 0 {
        county.castle_building.saturating_sub(1)
    } else if county.castle_degraded == CASTLE_DEGRADED_DAMAGED {
        county.castle_level_left
    } else {
        county.castle_type.saturating_sub(1)
    }
}

/// The gate: `level < 3 || engines > 0`. `L2.eng` 281 is this sentence in the
/// game's own words. `[V]`
pub fn can_assault(castle_level: u8, engines_built: i32) -> bool {
    castle_level < ENGINES_REQUIRED_FROM_LEVEL || engines_built > 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assault {
    Battle { attacker: usize, defender: usize, castle_level: u8 },
    /// `level >= 3` with no engines: message `0x119` (`L2.eng` 281) and the
    /// siege is **lifted**. The army does not stall waiting; it gives up.
    NoEngines,
    NoSiege,
}

/// `Siege_LaunchAssault` (`0x004A8AAB`) — turn phase 2's payoff.
pub fn assault(counties: &[County; MAX_COUNTIES], units: &mut Units, army: usize) -> Assault {
    let Some(county) = units.get(army).map(|u| u.besieging_county) else { return Assault::NoSiege };
    if county == 0 {
        return Assault::NoSiege;
    }
    let Some(c) = counties.get(county as usize) else { return Assault::NoSiege };
    let garrison = c.garrison_unit;
    if garrison == 0 {
        return Assault::NoSiege;
    }
    let level = assault_castle_level(c);
    let engines: i32 =
        units.get(army).map_or(0, |u| u.engines.iter().map(|r| r.ordered as i32).sum());
    if can_assault(level, engines) {
        Assault::Battle { attacker: army, defender: garrison, castle_level: level }
    } else {
        break_siege(counties, units, army);
        Assault::NoEngines
    }
}

