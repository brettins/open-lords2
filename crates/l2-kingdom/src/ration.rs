//! Rations — `docs/kingdom.md` §4.2 and §4.3, `Ration_Apply` (`0x0044DF5F`).
//!
//! **Neither call spends.** §3.4 has `Ration_Apply` running twice per season,
//! the second time as next season's preview, and §4.3 records that the stored
//! `rationAchieved` is consequently the *next* season's level
//! one that was applied. `Ration_Apply` (`0x0044DF5F`) debits no store at all:
//!
//! [`apply`] is `Ration_ApplyAll`'s body and shadows the cost into
//! `+0x18C`/`+0x190`, [`preview`] does not, and `Grain_SeasonTick` /
//! `Herd_SeasonTick` take the food out. That is a choice, not a finding:

use crate::county::County;
use crate::math::{div_ceil, pct};
use crate::tables::{Season, Tables, RATION_LEVEL_COUNT};

/// Every call in the binary passes `g_season` except `Ration_ApplyAll`
/// (`0x0044BF04`), which passes `g_seasonPrev` — so the season here is the one
/// *that call* was handed, not always the turn's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sowing {
    pub season: Option<Season>,
    pub advanced_farming: bool,
}

impl Sowing {
    pub const NONE: Sowing = Sowing { season: None, advanced_farming: false };

    pub fn new(season: Season, advanced_farming: bool) -> Sowing {
        Sowing { season: Some(season), advanced_farming }
    }

    pub fn from_index(season: u8, advanced_farming: bool) -> Sowing {
        Sowing { season: Season::from_index(season), advanced_farming }
    }
}

/// **The seed corn is not food.** `Ration_Apply` (`0x0044DF5F`) opens with
///
/// Season 4 is Winter and the spending pass is handed `g_seasonPrev`, so the
/// turn the gate fires on is the turn `Grain_SeasonTick` (`0x0044C8AE`) sows,
/// which is Spring. Without it a Spring county is offered its own seed as a
/// meal, eats it, and sows what the ration pass left.
///
/// `Grain_Sow` (`0x0044CFE1`) is called for its return value and is otherwise
/// pure but for [`County::sow_shortfall`] (`+0x1A7`), which it writes here as
/// from every other caller. Nothing reads that flag between this write and the
/// sowing that overwrites it, so the write is faithful rather than load-bearing
/// — the argument [`crate::land::grain::sow_seed`] records for the labour
/// estimate. **`[V]`** on the reservation, **`[D]`** on the flag being
/// unobservable.
pub fn seed_reserved(t: &Tables, county: &mut County, sowing: Sowing) -> i32 {
    if sowing.season != Some(Season::Winter) {
        return 0;
    }
    let labour = crate::land::grain_labour(t, county);
    let store = county.grain;
    crate::land::sow_sacks(t, county, store, labour, sowing.advanced_farming)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub level: i32,
    pub requirement: i32,
    pub dairy: i32,
    pub heads: i32,
    pub sacks: i32,
}

/// **`[V]`**, and unusually well corroborated: a strategy guide states *"each
/// portion of cheese being enough to feed 5 people"*, and a player measuring a
/// save reported 80 cows feeding 400 people.
pub fn food_from_dairy(t: &Tables, herd: i32) -> i32 {
    herd.max(0).saturating_mul(t.food.dairy_per_head)
}

/// ```c
/// county[+0x16C] = county.herd      * g_dairyPerHead;   /* the standing herd  */
/// county[+0x170] = county.grainEaten * g_foodPerSack;   /* the grain eaten    */
/// county[+0x174] = county.herdEaten  * g_foodPerHead;   /* the beasts killed  */
/// ```
///
/// `00440000.c:8019-8021`, the tail of `Ration_Apply` (`0x0044DF5F`): three bare
/// multiplications. **`+0x16C` is not `Food_FromDairy`** (`0x0044E5BB`), which
/// guards with `if (0 < g_counties[county].herd)` at `00440000.c:8040`. `[V]`
pub fn people_fed(t: &Tables, county: &County) -> (i32, i32, i32) {
    (
        county.grain_eaten.saturating_mul(t.food.food_per_sack),
        county.herd_eaten.saturating_mul(t.food.food_per_head),
        county.herd.saturating_mul(t.food.dairy_per_head),
    )
}

pub fn heads_for_people(t: &Tables, people: i32) -> i32 {
    div_ceil(people, t.food.food_per_head)
}

pub fn sacks_for_people(t: &Tables, people: i32) -> i32 {
    div_ceil(people, t.food.food_per_sack)
}

pub fn requirement(t: &Tables, people: i32, level: i32) -> i32 {
    let row = t.ration[clamp_level(level) as usize];
    let (divisor, multiplier) = (row.divisor, row.multiplier);
    div_ceil(people, divisor) * multiplier
}

// [D] ceiling 5: Ration_Apply 00440000.c:7942 clamps rationAchieved to 6, one row past
// g_rationTable; unreachable from any rationWanted <= 5 (oracle check 2026-09-17).
fn clamp_level(level: i32) -> i32 {
    level.clamp(0, RATION_LEVEL_COUNT as i32 - 1)
}

pub fn plan(t: &Tables, people: i32, level: i32, herd: i32, split: i32) -> Plan {
    let level = clamp_level(level);
    split_plan(t, level, requirement(t, people, level), herd, split)
}

/// The tail both of `Ration_Apply` (`0x0044DF5F`)'s two passes share:
/// `local_2c = local_2c - Food_FromDairy(county); if (local_2c < 1) local_2c = 0;`
/// then `iVar3 = Pct(local_2c, sVar1)`, `Food_HeadsForPeople(county, iVar3)`
/// and `Food_SacksForPeople(county, local_2c - iVar3)`.
fn split_plan(t: &Tables, level: i32, want: i32, herd: i32, split: i32) -> Plan {
    let dairy = food_from_dairy(t, herd);
    let remainder = (want - dairy).max(0);
    let from_livestock = pct(remainder, split.clamp(0, 100));
    Plan {
        level,
        requirement: want,
        dairy: dairy.min(want),
        heads: heads_for_people(t, from_livestock),
        sacks: sacks_for_people(t, remainder - from_livestock),
    }
}

/// `local_2c = DAT_004d673c[l] * DivCeil(iVar6, g_rationTable[l]);
/// if (g_optArmiesEat == 1) local_2c = local_2c + iVar2;` — `iVar6` is
/// `population` alone and `iVar2` is `+0x198 + +0x19C`, so the garrisons are
/// added **after** the level has scaled the population, not through the
/// divisor. `Ration_Apply` (`0x0044DF5F`).
fn demand(t: &Tables, county: &County, level: i32, armies_eat: bool) -> i32 {
    let mut want = requirement(t, county.population, level);
    if armies_eat {
        want += county.friendly_troops + county.enemy_troops;
    }
    want
}

/// **`[I]` on the enemy half.** `docs/kingdom.md` §1.3 lists `+0x198` and `+0x19C`
/// together as *"friendly / enemy troops … added to the food requirement when
/// Armies Eat is on"* and does not say whether both are added. Both are added
/// here; an occupying army foraging the county it stands in is the reading that
/// makes the field pair worth storing. `Ration_Apply` (`0x0044DF5F`) adds the
/// pair as [`demand`]'s `iVar2`; this sum is `FUN_004A4782`'s ladder input.
pub fn people_to_feed(county: &County, armies_eat: bool) -> i32 {
    if armies_eat {
        county.population + county.friendly_troops + county.enemy_troops
    } else {
        county.population
    }
}

/// `Food_Available` (`0x0044E7B4`) — the number [`crate::unit::starve`] tests
/// an army's size against.
///
/// There are two sibling functions computing superficially similar sums from
/// *different* county fields (`+0x178`/`+0x17C`, the food eaten). Any
/// reading that pattern-matches on "the food function" is `docs/decisions.md`
/// C3 waiting to happen.
pub fn food_available(t: &Tables, county: &County) -> i32 {
    available(t, county.herd, county.herd_available, county.grain_available)
}

fn available(t: &Tables, herd: i32, herd_available: i32, grain_available: i32) -> i32 {
    let mut food = 0i64;
    if herd > 0 {
        food += herd as i64 * t.food.dairy_per_head as i64;
    }
    if herd_available > 0 {
        food += herd_available as i64 * t.food.food_per_head as i64;
    }
    if grain_available > 0 {
        food += grain_available as i64 * t.food.food_per_sack as i64;
    }
    food.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

pub fn choose(t: &Tables, county: &County, armies_eat: bool) -> Plan {
    choose_within(t, county, armies_eat, 0)
}

/// `Ration_Apply` (`0x0044DF5F`) — **two** descents, not one.
///
/// The first tests the requirement against `Food_Available` (`0x0044E7B4`)'s
/// *total* — `iVar3 = Food_Available(county); ... if (local_2c <= iVar3) break;`
/// — so a larder that is large enough in sum passes the level even when
/// `rationSplit` aims the demand at the empty side. The split bites in the
/// second: `if (local_2c <= iVar5 + iVar4 + iVar3) break;` where those three are
/// the dairy plus the `herdEaten`/`grainEaten` the first pass could actually
/// fetch, each already capped at `herdAvailable`/`grainAvailable`.
fn choose_within(t: &Tables, county: &County, armies_eat: bool, seed: i32) -> Plan {
    // `herdAvailable = herd; grainAvailable = grain - local_24;` before the
    // `Food_Available(county)` call reads them.
    let herd_available = county.herd;
    let grain_available = county.grain - seed;
    let total = available(t, county.herd, herd_available, grain_available);
    let split = county.ration_split;

    // `rationAchieved = rationWanted + 1;` and the loop's leading
    // `if (0 < rationAchieved) rationAchieved--` — six iterations, so the
    // first test is at `rationWanted`.
    let mut level = county.ration_wanted + 1;
    for _ in 0..RATION_LEVEL_COUNT {
        if level > 0 {
            level -= 1;
        }
        if demand(t, county, level, armies_eat) <= total {
            break;
        }
    }
    // `if (6 < rationAchieved) rationAchieved = 6; if (rationAchieved < 0)
    // rationAchieved = 0;` — 6 would index `g_rationTable` one row past its
    // end, and the loop's first decrement makes it unreachable from any
    // `rationWanted` the game writes.
    level = clamp_level(level);

    // `herdEaten = min(iVar4, herdAvailable); grainEaten = min(iVar3, grainAvailable);`
    let first = split_plan(t, level, demand(t, county, level, armies_eat), county.herd, split);
    let got = (food_from_dairy(t, county.herd) as i64
        + first.heads.min(herd_available) as i64 * t.food.food_per_head as i64
        + first.sacks.min(grain_available) as i64 * t.food.food_per_sack as i64)
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32;

    for _ in 0..RATION_LEVEL_COUNT {
        if demand(t, county, level, armies_eat) <= got {
            break;
        }
        if level > 0 {
            level -= 1;
        }
    }
    level = clamp_level(level);
    split_plan(t, level, demand(t, county, level, armies_eat), county.herd, split)
}

/// `Ration_Apply` (`0x0044DF5F`) proper — everything the original's function
/// writes. Shared by [`apply`] and [`preview`]; the only difference between
/// them is whether the shadow pair is written after it.
fn record(t: &Tables, county: &mut County, p: Plan, seed: i32) {
    county.ration_achieved = p.level;
    county.herd_available = county.herd;
    county.grain_available = county.grain - seed;
    county.herd_eaten = p.heads.min(county.herd_available);
    county.grain_eaten = p.sacks.min(county.grain_available);
    county.d_hap_ration = t.ration_happiness(p.level);
}

/// `Ration_ApplyAll` (`0x0044BF04`) for one county: choose a level, set the
/// ration happiness term, and **shadow** what was priced.
///
/// `Ration_Apply` (`0x0044DF5F`) itself never debits a store — its whole loop
/// reads `Food_Available` and writes display fields. `Ration_ApplyAll` follows
/// it with `+0x18C = grainEaten; +0x190 = herdEaten;`, and the debit happens
/// two passes later, at the top of `Grain_SeasonTick` (`0x0044C8AE`) and
/// `Herd_SeasonTick` (`0x0044D60D`). The shadow is not redundant with
/// `+0x178`/`+0x17C`: [`preview`] overwrites those with next season's forecast
/// before either tick runs (`docs/decisions.md` C20), and in the fixture every
/// unowned county stores `herdEaten` 13 against a shadow of 0.
pub fn apply(t: &Tables, county: &mut County, armies_eat: bool, sowing: Sowing) -> Plan {
    let seed = seed_reserved(t, county, sowing);
    let p = choose_within(t, county, armies_eat, seed);
    record(t, county, p, seed);
    county.grain_eaten_shadow = county.grain_eaten;
    county.herd_eaten_shadow = county.herd_eaten;
    p
}

pub fn preview(t: &Tables, county: &mut County, armies_eat: bool, sowing: Sowing) -> Plan {
    let seed = seed_reserved(t, county, sowing);
    let p = choose_within(t, county, armies_eat, seed);
    record(t, county, p, seed);
    p
}

#[cfg(test)]
mod tests;
