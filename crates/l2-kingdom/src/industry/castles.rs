#![allow(unused_imports)]
use super::*;
use super::production::*;
use super::wages::*;
use super::ui::*;
use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::report::Message;
use crate::tables::{Commodity, Tables, RESOURCE_LIMIT_UNLIMITED, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

// ---------------------------------------------------------------------------
// Castles
// ---------------------------------------------------------------------------

/// The garrison a completed castle can hold.
pub fn garrison_cap(t: &Tables, castle_type: u8) -> i32 {
    if castle_type == 0 {
        0
    } else {
        t.castle.garrison_cap[(castle_type as usize - 1).min(t.castle.garrison_cap.len() - 1)]
    }
}

/// The archers a new castle comes with. The manual: *"A new castle will
/// automatically include a garrison. Its size will vary according to the size
/// of the castle."*
pub fn free_archers(t: &Tables, castle_type: u8) -> i32 {
    if castle_type == 0 {
        0
    } else {
        t.castle.free_archers[(castle_type as usize - 1).min(t.castle.free_archers.len() - 1)]
    }
}

/// `g_castleMaterial` (`0x004D89C0`) — `(wood, stone)` to build a castle type
/// 1..=5, from scratch.
///
/// **The column order is read from code, not assumed.** `Castle_Order` takes
/// the `+0` word out of `realm.wood` and the `+4` word out of `realm.stone`,
/// a wooden palisade really is 400 wood and 40 stone and a royal castle 800
/// wood and 3,000 stone. `[V]`
pub fn castle_cost(t: &Tables, castle_type: u8) -> (i32, i32) {
    t.castle.cost[(castle_type.max(1) as usize - 1).min(t.castle.cost.len() - 1)]
}

/// The workforce a castle type consumes.
///
/// The table holds two ints per level and both carry the same number. What the
/// second column is for is not established,
/// table keeps the pair.
pub fn castle_workforce(t: &Tables, castle_type: u8) -> i32 {
    t.castle.workforce[(castle_type.max(1) as usize - 1).min(t.castle.workforce.len() - 1)].0
}

/// The chooser's OK guard, separated from the act
/// right refusal and the AI can ask before ordering.
pub fn castle_refusal(t: &Tables, county: &County, castle_type: u8) -> Option<CastleRefusal> {
    if castle_type == 0 || castle_type as usize > t.castle.cost.len() {
        return Some(CastleRefusal::NoSuchType);
    }
    if county.castle_type == castle_type {
        return Some(CastleRefusal::AlreadyBuilt);
    }
    if castle_type < county.castle_type {
        return Some(CastleRefusal::Downgrade);
    }
    None
}

/// `Castle_Order` (`0x00436D02`) — **order a castle**, which is the only thing
/// that ever sets [`County::castle_degraded`] to
/// [`crate::siege::CASTLE_DEGRADED_BUILDING`].
///
/// ```c
/// wood = g_castleMaterial[p].wood;  stone = g_castleMaterial[p].stone;
/// if (county.castleType != 0) {                      /* an upgrade */
///     county.castleBuilding = county.castleType;     /* remember what stands */
///     wood  -= g_castleMaterial[castleType - 1].wood;
///     stone -= g_castleMaterial[castleType - 1].stone;
///     if (wood  < 0) { realm.wood  -= wood;  wood  = 0; }   /* a refund */
///     if (stone < 0) { realm.stone -= stone; stone = 0; }
/// }
/// county.castleType     = p + 1;
/// county.castleRuined   = 0;
/// county.castleDegraded = 1;
/// county.castleSwitch   = 1;
/// county.workLeft = county.workTotal = g_castleWorkforce[p];
/// county.stoneTotal = stone;  county.woodTotal = wood;  county.percent = 0;
/// Castle_StampTile(county, p);  Castle_EvictTile(county);
/// Labour_ToggleIndustryShare(county, 3, 1);
/// county.stoneOwed = take(realm.stone, stone);      /* what the realm cannot
/// county.woodOwed  = take(realm.wood,  wood);          pay now stays owing */
/// ```
///
/// Three things here contradict what this module used to say,
/// read from the function:
///
/// * You may order a royal castle with an
///   empty treasury; it takes forever, because
///   [`castle_labour_estimate`]'s ceiling is zero until the materials are all
///   delivered. The old *"returns `false` if the realm cannot pay"* was ours.
/// * **The cost is a difference, and a *lower* castle refunds.** Upgrading a
///   Norman keep (200 wood, 1,000 stone) to a stone castle (400, 2,000) costs
///   200 wood and 1,000 stone. Upgrading a motte and bailey (800 wood, 80
///   stone) to a Norman keep costs **1,000 stone and gives 600 wood back** —
///   the negative arm is reachable on the shipped table and is not an overflow
///   guard.
/// * **`castleType` moves immediately.** See [`County::castle_building`].
///
/// Returns `false`, changing nothing, for either of [`castle_refusal`]'s two
/// guards. It does **not** stamp the map: that is
/// [`crate::map::castle_tile_stamp`], which the caller applies because the tile
/// planes are not the county's to write.
pub fn order_castle(t: &Tables, county: &mut County, realm: &mut Realm, castle_type: u8) -> bool {
    if castle_refusal(t, county, castle_type).is_some() {
        return false;
    }
    let (mut wood, mut stone) = castle_cost(t, castle_type);
    if county.castle_type != 0 {
        county.castle_building = county.castle_type;
        let (had_wood, had_stone) = castle_cost(t, county.castle_type);
        wood -= had_wood;
        stone -= had_stone;
        // A negative difference is paid back into the store. `realm.wood -=
        // wood` with `wood` negative.
        if wood < 0 {
            realm.wood -= wood;
            wood = 0;
        }
        if stone < 0 {
            realm.stone -= stone;
            stone = 0;
        }
    }
    county.castle_type = castle_type;
    county.castle_ruined = false;
    // `+0x1C3` — **a castle is under construction.** Five independent readers
    // all mean that: `Labour_Allocate` will not staff castle building without
    // it, [`castle_labour_estimate`] computes nothing without it,
    // `Industry_LabourEstimate` shows the outstanding wood and stone only with
    // it, `Tax_CollectAll` charges the *standing* castle while it is set, and
// [`crate::map::castle_tile_stamp`] draws scaffolding.
    // **Nothing a player could reach used to write it**, so the castle-building
    // job had a ceiling of zero for ever and no county could build anything.
    county.castle_degraded = crate::siege::CASTLE_DEGRADED_BUILDING;
    // `+0x1B0` — the map's castle-building switch, thrown **on** by the order.
    // It is the third of its three writers in the original and the only one
    // that is not a click.
    county.castle_switch = true;
    county.castle_work_left = castle_workforce(t, castle_type);
    county.castle_work_total = county.castle_work_left;
    county.castle_stone_total = stone;
    county.castle_wood_total = wood;
    county.castle_percent = 0;
    // `Labour_ToggleIndustryShare(county, 3, 1)` — `Castle_Order`
    // (`0x00436D02`) calls it between `Castle_EvictTile` and the take, so the
    // order itself puts builders on the castle. Without it the job's share
    // stayed 0 at any industry split. `[V]`
    crate::labour::toggle_industry_share(county, crate::tables::JOB_CASTLE_BUILDING, true);
    county.castle_stone_owed = take_from(&mut realm.stone, stone);
    county.castle_wood_owed = take_from(&mut realm.wood, wood);
    true
}

/// Take as much of `want` out of `store` as it holds, and answer what is still
/// owed. The shape `Castle_Order` and `Castle_DeliverMaterials` both use.
fn take_from(store: &mut i32, want: i32) -> i32 {
    if *store < want {
        let owed = want - *store;
        *store = 0;
        owed
    } else {
        *store -= want;
        0
    }
}

/// `Castle_DeliverMaterials` (`0x00450CCD`) — **the season's cart of wood and
/// stone**, taken off the realm and set against what the county still owes.
///
/// Runs before the labour is spent, and takes whatever is there: a realm with
/// 10 stone in the store delivers 10. This is why a castle ordered on an empty
/// treasury crawls.
pub fn deliver_castle_materials(county: &mut County, realm: &mut Realm) {
    if county.castle_degraded == 0 {
        return;
    }
    county.castle_wood_owed = take_from(&mut realm.wood, county.castle_wood_owed);
    county.castle_stone_owed = take_from(&mut realm.stone, county.castle_stone_owed);
}

/// `FUN_00450FB4` — **how much of the materials bill has arrived**, as
/// a percentage,
///
/// ```c
/// min(100 - Pct(woodOwed, woodTotal), 100 - Pct(stoneOwed, stoneTotal))
/// ```
///
/// Zero when nothing is under construction. Note the `Pct(0, 0) == 0`
/// convention: a castle that costs no wood at all is 100% delivered in wood.
pub fn castle_materials_percent(county: &County) -> i32 {
    if county.castle_degraded == 0 {
        return 0;
    }
    let wood = 100 - crate::math::pct_of(county.castle_wood_owed, county.castle_wood_total);
    let stone = 100 - crate::math::pct_of(county.castle_stone_owed, county.castle_stone_total);
    wood.min(stone)
}

/// `Castle_BuildTick` (`0x004508DE`) — **one season of castle work in one
/// county**, season pass 3.4.
///
/// ```c
/// if (castleDegraded == 0) { Castle_BuildEstimate(county); return; }
/// if (county.owner == 0) return;                 /* not even the estimate */
/// workers = labour[3].workers;
/// Castle_DeliverMaterials(county);
/// percent = 100 - Pct(workLeft, workTotal);
/// if (materialsPercent > 99) {
///     spend = min(workers, workLeft);  workLeft -= spend;
///     percent = 100 - Pct(workLeft, workTotal);
///     if (percent > 99 && workLeft != 0) percent = 99;   /* never round up to done */
/// }
/// county.percent = percent;
/// if (percent > 99) { ...complete... }
/// Castle_StampTile(county, castleType - 1);
/// Castle_BuildEstimate(county);
/// ```
///
/// Returns `Some` on the season the castle is finished. **`castleType` is not
/// promoted here** — it moved when the castle was ordered — so all completion
/// does is clear [`County::castle_degraded`], hand out the free archers and
/// switch the castle job off.
pub fn build_tick(
    t: &Tables,
    county: &mut County,
    realm: &mut Realm,
    id: u8,
    out: &mut Vec<Message>,
) -> Option<CastleComplete> {
    if county.castle_degraded == 0 || county.owner == 0 {
        return None;
    }
    let workers = county.labour[t.job.castle_building].max(0);
    deliver_castle_materials(county, realm);
    let mut percent = 100 - crate::math::pct_of(county.castle_work_left, county.castle_work_total);
    if castle_materials_percent(county) > 99 {
        let spend = workers.min(county.castle_work_left);
        county.castle_work_left -= spend;
        percent = 100 - crate::math::pct_of(county.castle_work_left, county.castle_work_total);
        // **Never let the rounding finish a castle that still has work in it.**
        if percent > 99 && county.castle_work_left != 0 {
            percent = 99;
        }
    }
    county.castle_percent = percent.clamp(0, 100) as u8;
    if county.castle_percent <= 99 {
        return None;
    }

    let repaired = county.castle_degraded == crate::siege::CASTLE_DEGRADED_DAMAGED;
    let free_archers = if repaired { 0 } else { free_garrison_archers(t, county) };
    county.castle_degraded = 0;
    // `Castle_BuildTick` (`0x004508DE`) ends the degraded branch with
    // `castleDegraded = 0; Labour_ToggleIndustryShare(county, 3, 0)` — the
    // builders go back to the fields the moment the castle tops out. `[V]`
    // It does not write `+0x1B0`; we clear the switch with it so the flag and
    // the share agree, which `Industry_ToggleFromMap` assumes. `[I]`
    crate::labour::toggle_industry_share(county, crate::tables::JOB_CASTLE_BUILDING, false);
    county.castle_switch = false;
    out.push(Message::CastleBuilt { county: id, castle_type: county.castle_type });
    Some(CastleComplete { free_archers, repaired })
}

/// `Castle_RaiseFreeGarrison` (`0x004A551B`) — how many archers a finished
/// castle comes with.
///
/// ```c
/// archers = g_castleFreeArchers[castleType - 1];
/// if (castleBuilding != 0) archers -= g_castleFreeArchers[castleBuilding - 1];
/// if (archers <= 0) return 0;
/// if (archers > county.population / 2) return 0;
/// ```
///
/// So an **upgrade** yields only the difference — a motte and bailey (150) to a
/// Norman keep (150) yields nothing at all — and a county that would have to
/// give up more than half its people gives up none. The men are equipped as
/// archers out of a bow stock the function tops up itself, which is what makes
/// them free.
pub fn free_garrison_archers(t: &Tables, county: &County) -> i32 {
    if county.castle_type == 0 {
        return 0;
    }
    let free = |ty: u8| t.castle.free_archers[(ty as usize - 1).min(t.castle.free_archers.len() - 1)];
    let mut archers = free(county.castle_type);
    if county.castle_building != 0 {
        archers -= free(county.castle_building);
    }
    if archers <= 0 || archers > county.population / 2 {
        return 0;
    }
    archers
}

/// `Castle_BuildEstimate` (`0x00450E46`) — the **castle** labour ceiling.
///
/// ```c
/// wanted[3] = -1; useful[3] = 0;
/// if (castleDegraded == 0) return;
/// useful[3] = (materialsPercent < 100) ? 0 : workLeft;
/// ```
///
/// **The materials clause is real and it bites.** This module used to say it
/// *"cannot be reproduced and does not need to be"*, on the reading that the
/// whole cost was taken up front; it is not, so the gate is shut for every
/// season the county is still owed a stick of wood,
/// idle. That is the difference between a castle you can order and a castle you
/// can order *and not build*.
///
/// The ceiling is a *cumulative* figure — the whole remaining work, not a
/// per-season share —
/// one that cannot puts everybody it has on the walls.
pub fn castle_labour_estimate(_t: &Tables, county: &County) -> (i32, i32) {
    if county.castle_degraded == 0 || castle_materials_percent(county) < 100 {
        return (crate::county::LABOUR_NO_FLOOR, 0);
    }
    (crate::county::LABOUR_NO_FLOOR, county.castle_work_left.max(0))
}

/// County `+0x280`, `+0x284`, `+0x288` and `+0x28C` — **the four figures
/// `Panel_JobIndustry` (`0x00412E6B`) prints beside an industry's forecast**,
/// in that order: the wood and the iron next season's weapons will use (76/2
/// *"will be used by the blacksmiths."* on the wood and iron rows),
/// and stone the castle builders still need (76/3 *"needed by castle
/// builders."* on the wood and stone rows).
///
/// `Industry_LabourEstimate` (`0x0044F318`) writes them as it goes: every call
/// zeroes the first two and the weapons call fills them, inside its guard,
/// `g_weaponCost[weaponType]` times the weapons forecast it has just made; the
/// wood and stone calls copy the castle's outstanding wood and stone when a
/// build is in progress, and zero otherwise. Since the weapons forecast is zero
/// whenever that guard fails, the four are a **pure function** of
/// [`Industry::next_season`], [`County::weapon_type`] and the castle record —
/// so this is a function.
///
/// **`[V]` for the first two**: `weaponCost[type] × made` is the stored value in
/// every county of every save, sixteen of them non-zero for the wood and seven
/// for the iron. The castle pair is zero in every save, because no save on this
/// machine was taken mid-build.
///
/// **Nothing draws them yet.** The job popup's body is a stub in `l2-game`, so
/// this is what that painter will call.
pub fn panel_figures(t: &Tables, county: &County) -> [i32; 4] {
    let weapon = county.weapon_type.min(WEAPON_TYPE_COUNT - 1);
    let made = county.industry[Commodity::Weapons.index()].next_season;
    let (wood_owed, stone_owed) = if county.castle_degraded != 0 {
        (county.castle_wood_owed, county.castle_stone_owed)
    } else {
        (0, 0)
    };
    [
        t.weapon[weapon].wood.saturating_mul(made),
        t.weapon[weapon].iron.saturating_mul(made),
        wood_owed,
        stone_owed,
    ]
}

/// County `+0x1A6` — the seasons the castle panel says the work will take.
/// **100** stands for *"not in this lifetime"*: no materials, or nobody on it.
pub fn castle_seasons_left(t: &Tables, county: &County) -> i32 {
    if county.castle_degraded == 0 {
        return 0;
    }
    let workers = county.labour[t.job.castle_building];
    if castle_materials_percent(county) < 100 || workers < 1 {
        return 100;
    }
    crate::math::div_ceil(county.castle_work_left, workers)
}

