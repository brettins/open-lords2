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
    /// `Siege_RecordCastleDamage`'s opening `if`: it does nothing at all unless
    /// one of the two accumulators is non-zero.
    pub fn any(&self) -> bool {
        self.moat_filled != 0 || self.wall_damage != 0
    }
}

/// Wood owed per point of [`SiegeScars::wall_damage`], **below castle level
/// 2** — a palisade or a motte and bailey, which are made of wood.
pub const REPAIR_WOOD_PER_WALL: i32 = 10;
/// Stone owed per point, at level 2 and above.
pub const REPAIR_STONE_PER_WALL: i32 = 15;
/// Man-seasons per point of [`SiegeScars::wall_damage`], either way.
pub const REPAIR_WORK_PER_WALL: i32 = 15;
/// Man-seasons per moat cell filled in — the digging
/// moat costs.
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
/// Four things follow
///
/// * **A castle is repaired in the material it is made of.** Wood below level
///   2, stone at 2 and above, and never both. `docs/bugs.md` B69.
/// * **The `+=` is real.** The original writes plain `=` when
///   `castleDegraded != 1` and `x = x + y` when it is 1, so besieging a castle
///   that is *already being built* makes the job bigger than the castle was —
/// the scaffolding's bill and the siege's are added together and paid once.
/// * **Filling in the moat costs work and no materials.** `moatFilled` is only
///   ever multiplied by 5 into the work total; it never reaches the wood or
/// stone line. So a besieger who shovels the ditch full and is then thrown
///   off has cost the defender labour and nothing else.
/// * **`castlePercent` is reset to 0**, which is what puts the scaffolding
///   back on the map tile: `Castle_StampTile` reads `< 50` as scaffolding.
///
/// > **`docs/symbols.md` calls `DAT_0057A0D8` `breachDamage` and that is a
/// > misnomer** — the whole binary holds three writers of it and the only one
/// > that adds is the moat fill. The parameter is named for what writes it.
/// >
///
/// Answers whether anything was billed.
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
///
/// So a castle carries its scars into the next assault
/// *not* mid-repair has them cleared — which is what stops a rebuilt castle
/// inheriting the last siege's open gate. Both arms matter and only the first
/// one is obvious.
pub fn scars_for_assault(county: &mut County) -> SiegeScars {
    if county.castle_degraded == CASTLE_DEGRADED_DAMAGED {
        county.siege_scars
    } else {
        county.siege_scars = SiegeScars::default();
        SiegeScars::default()
    }
}

/// **Which castle is fought** — `Siege_LaunchAssault`'s opening, and
/// not `castleType`.
///
/// ```c
/// if (degraded == 1 && castleBuilding != 0) level = castleBuilding - 1;
/// else if (degraded == 2)                   level = county.castleLevelLeft;   /* +0x1F9 */
/// else                                      level = castleType - 1;
/// ```
///
/// The level is **zero-based**: type 1 (palisade) is level 0 and type 5 (royal)
/// is level 4, which is the indexing [`OIL_BY_CASTLE_LEVEL`] and
/// `crate::battle`'s castle-strength bonus both use. That is why this returns a
/// level and not a type, and why the two tables are five long.
///
/// `castleType == 0` — no castle at all — would give `-1`; the assault path
/// cannot reach it because [`begin_siege`] requires a garrison and a garrison
/// requires a castle, and it is clamped here.
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

/// What `Siege_LaunchAssault` decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assault {
    /// The battle is on: these two units, at this castle level. `attacker` is
    /// always the besieger and `defender` always the garrison — the mapping
    /// `crate::battle::outcome`'s four siege banners depend on.
    Battle { attacker: usize, defender: usize, castle_level: u8 },
    /// `level >= 3` with no engines: message `0x119` (`L2.eng` 281) and the
    /// siege is **lifted**. The army does not stall waiting; it gives up.
    NoEngines,
    /// The link had already gone — nothing to assault.
    NoSiege,
}

/// `Siege_LaunchAssault` (`0x004A8AAB`) — turn phase 2's payoff.
///
/// Works out the level, sums the three engine counts, applies the gate, and
/// either names the battle or lifts the siege. **It does not fight the
/// battle**: `Battle_ChooseSettlement` decides how that happens and the caller
/// owns it,
/// [`crate::conquest::Attack::Battle`].
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

