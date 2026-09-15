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


pub fn garrison_cap(t: &Tables, castle_type: u8) -> i32 {
    if castle_type == 0 {
        0
    } else {
        t.castle.garrison_cap[(castle_type as usize - 1).min(t.castle.garrison_cap.len() - 1)]
    }
}

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

pub fn castle_workforce(t: &Tables, castle_type: u8) -> i32 {
    t.castle.workforce[(castle_type.max(1) as usize - 1).min(t.castle.workforce.len() - 1)].0
}

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
    county.castle_degraded = crate::siege::CASTLE_DEGRADED_BUILDING;
    // `+0x1B0` — the map's castle-building switch, thrown **on** by the order.
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
pub fn deliver_castle_materials(county: &mut County, realm: &mut Realm) {
    if county.castle_degraded == 0 {
        return;
    }
    county.castle_wood_owed = take_from(&mut realm.wood, county.castle_wood_owed);
    county.castle_stone_owed = take_from(&mut realm.stone, county.castle_stone_owed);
}

/// `FUN_00450FB4` — **how much of the materials bill has arrived**, as
/// a percentage,
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

