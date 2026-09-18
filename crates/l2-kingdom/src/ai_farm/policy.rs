use super::*;

/// `0x004A3C67`'s opening line: `if (grain < 100) county.+0x1F4 += 100`.
///
/// County `+0x1F4` is the **unowned county's own purse** — the thing
/// `Merchant_Trade` pays out of when the realm is 0.
///
/// **`[V]`**,
/// *first* statement
/// follow it, so the hundred crowns are available to the cascade that spends
/// them in the same pass.
pub const NEUTRAL_PURSE_TOP_UP: i32 = 100;

pub fn neutral_purse_top_up(style: FarmStyle, county: &County) -> i32 {
    if style == FarmStyle::NeutralArable && county.grain < 100 {
        NEUTRAL_PURSE_TOP_UP
    } else {
        0
    }
}

fn stock(county: &County, good: Good) -> i32 {
    match good {
        Good::Grain => county.grain,
        Good::Cattle => county.herd,
    }
}

pub fn run_buys(
    style: FarmStyle,
    id: usize,
    county: &mut County,
    map: &mut CampaignMap,
    market: &mut dyn Market,
) -> i32 {
    let mut bought = 0;
    for line in style.buys() {
        if stock(county, line.good) < line.floor
            && market.buy(id, county, map, line.lot, line.good)
        {
            bought += 1;
        }
    }
    bought
}

// ---------------------------------------------------------------------------
// FUN_004A4782 — the AI's ration setter
// ---------------------------------------------------------------------------

/// `FUN_0044E6A3` — **not** `Food_Available`,
/// point of `crate::ration::food_available`'s warning.
///
/// Three functions in the binary compute a superficially similar sum from
/// different fields: this one from the **stores** (`herd`, `grain`),
/// `Food_Available` (`0x0044E7B4`) from the per-season *caps*
/// `herdAvailable`/`grainAvailable`, and `FUN_0044E741` from the stores with no
/// slaughter term at all. Picking the wrong one is `docs/decisions.md` C3
/// waiting to happen; the AI's ration ladder reads **this** one.
pub fn food_in_store(t: &Tables, county: &County) -> i32 {
    let mut food = 0i64;
    if county.herd > 0 {
        food += county.herd as i64 * t.food.dairy_per_head as i64;
        food += county.herd as i64 * t.food.food_per_head as i64;
    }
    if county.grain > 0 {
        food += county.grain as i64 * t.food.food_per_sack as i64;
    }
    food.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// How the style searches for a ration split — the `param_2` of `FUN_004A4782`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitSearch {
    PreferGrain,
    PreferHerd,
    Fixed(i32),
}

pub fn split_search(style: FarmStyle, county: &County) -> SplitSearch {
    match style {
        FarmStyle::NeutralArable | FarmStyle::RealmArable => {
            if county.herd < 101 {
                SplitSearch::PreferGrain
            } else {
                SplitSearch::PreferHerd
            }
        }
        _ => SplitSearch::PreferGrain,
    }
}

/// The ration level the AI *asks* for, from what is in store —
/// `FUN_004A4782`'s first half.
///
/// `[V]` on the arithmetic, `[D]` on calling it a bug. It is the only place in
/// the ration rules where the *composition* of the larder changes the level
///, and it costs the grazing lords a ration point.
pub fn ration_wanted(t: &Tables, county: &County, armies_eat: bool) -> i32 {
    let store = food_in_store(t, county);
    let dairy = ration::food_from_dairy(t, county.herd);
    let people = ration::people_to_feed(county, armies_eat);
    if store < people / 2 {
        0
    } else if store < people {
        1
    } else if store < people * 2 {
        2
    } else if dairy > people * 3 {
        5
    } else if dairy > people * 2 {
        4
    } else if store > people * 8 {
        5
    } else if store > people * 6 {
        4
    } else {
        3
    }
}

/// `FUN_004A4782` — set `rationWanted`, then find the `rationSplit` that feeds
/// the county best.
///
/// It makes no difference — the next iteration overwrites it,
/// iteration's write is the one that survives — so this is written the
/// straightforward way. `[D]`, and
/// the reason it is safe to tidy is that `Ration_Apply` reads `rationSplit`
/// only at the top of each iteration, after it has already been set to the
/// candidate.
pub fn set_rations(
    t: &Tables,
    county: &mut County,
    search: SplitSearch,
    armies_eat: bool,
    sowing: crate::ration::Sowing,
) {
    county.ration_wanted = ration_wanted(t, county, armies_eat);
    let score = |county: &County, split: i32| -> i32 {
        let mut probe = county.clone();
        probe.ration_split = split;
        ration::choose(t, &probe, armies_eat).level
    };
    let chosen = match search {
        SplitSearch::Fixed(split) => split,
        SplitSearch::PreferGrain => {
            let mut best = 0;
            let mut chosen = 0;
            for split in 0..=100 {
                let achieved = score(county, split);
                if best < achieved {
                    best = achieved;
                    chosen = split;
                }
            }
            chosen
        }
        SplitSearch::PreferHerd => {
            let mut best = 0;
            let mut chosen = 100;
            for split in (0..=100).rev() {
                let achieved = score(county, split);
                if best < achieved {
                    best = achieved;
                    chosen = split;
                }
            }
            chosen
        }
    };
    county.ration_split = chosen;
    ration::preview(t, county, armies_eat, sowing);
}


pub fn winter_grain_quota(fertility: i32, base: i32) -> i32 {
    if fertility < -20 {
        base - 1
    } else if fertility < -50 {
        base - 2
    } else {
        base
    }
}

/// `Labour_DefaultSharesBuilt` (`0x0045158B`) — the setter every farming style
/// uses. Farm 33/50/17, industry 40/15/15/15/15.
pub fn default_shares_built(county: &mut County) {
    use crate::tables::{
        JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_FIELD_RECLAMATION,
        JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
    };
    county.labour_share[JOB_GRAIN_FARMING] = 33;
    county.labour_share[JOB_CATTLE_FARMING] = 50;
    county.labour_share[JOB_FIELD_RECLAMATION] = 17;
    county.labour_share[JOB_CASTLE_BUILDING] = 40;
    county.labour_share[JOB_IRON_MINING] = 15;
    county.labour_share[JOB_STONE_QUARRYING] = 15;
    county.labour_share[JOB_WOOD_CUTTING] = 15;
    county.labour_share[JOB_BLACKSMITH] = 15;
}

/// `FUN_004A4694` — nudge the pasture count by one, and only by one.
pub fn fit_cattle_fields(t: &Tables, county: &mut County, map: &mut CampaignMap, cap: i32) {
    if county.herd_crowding < 11 {
        if county.fields_cattle > 2 && county.herd < 10 {
            field::clear_type(county, map, FieldType::Pasture);
            field::set_count(county, map, FieldType::Pasture, county.fields_cattle - 1);
        }
    } else if county.fields_cattle < cap {
        field::clear_type(county, map, FieldType::Pasture);
        field::set_count(county, map, FieldType::Pasture, county.fields_cattle + 1);
    }
    field::recount(county, map);
    field::herd_update_crowding(t, county, map);
}

