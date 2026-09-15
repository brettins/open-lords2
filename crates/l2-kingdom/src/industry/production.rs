#![allow(unused_imports)]
use super::*;
use super::wages::*;
use super::castles::*;
use super::ui::*;
use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::report::Message;
use crate::tables::{Commodity, Tables, RESOURCE_LIMIT_UNLIMITED, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

/// `PctOf(a, b) = a * 100 / b` — `FUN_00404DC1`, the companion to
/// [`crate::math::pct`]. Zero denominator gives zero.
#[inline]
pub fn pct_of(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        ((a as i64 * 100) / b as i64) as i32
    }
}

/// `FUN_0044F248` — the efficiency ramp `docs/kingdom.md` §12 lists as unknown.
///
/// `[V]` on all of it; the `advanced_farming` global is `0x0053F25C`, the same
/// one `Grain_Sow` and `Fertility_Update` branch on.
pub fn efficiency_ramp(
    t: &Tables,
    last_efficiency: i32,
    workers: i32,
    capacity: i32,
    base: i32,
    advanced_farming: bool,
) -> i32 {
    if !advanced_farming {
        return t.efficiency.without_advanced_farming;
    }
    if workers == 0 {
        return 0;
    }
    let mut increment = base;
    if capacity < workers {
        increment = pct(base, pct_of(capacity, workers));
    }
    let mut efficiency = last_efficiency + increment;
    if efficiency > t.efficiency.max {
        efficiency = t.efficiency.max;
    }
    if efficiency < base {
        efficiency = base;
    }
    efficiency
}

/// `FUN_0044EF4E` — the `resourceLimit` term.
///
/// For wood, iron and stone it is a **flag test, not a quantity**: the industry
/// must be enabled (`+0x297`), must have its resource (`+0x295`) and must not
/// be counting down a disablement (`+0x296`); if all three hold the limit is a
/// literal [`RESOURCE_LIMIT_UNLIMITED`] and otherwise it is zero. So the only
/// thing bounding a mine is the county's workers — up to 999 units a season,
/// which no plausible workforce reaches.
///
/// first county emptying it. Those two denominators are [`WeaponShare`], and
/// they are no longer `[D]`.
pub fn resource_limit(
    t: &Tables,
    county: &County,
    c: Commodity,
    realm: &Realm,
    weapon_share: WeaponShare,
) -> i32 {
    let record = &county.industry[c.index()];
    if !record.enabled {
        return 0;
    }
    if c == Commodity::Weapons {
        let weapon = county.weapon_type.min(WEAPON_TYPE_COUNT - 1);
        let (wood, iron) = (t.weapon[weapon].wood, t.weapon[weapon].iron);
        let quota = |stock: i32, cost: i32, share: i32| {
            ((stock as i64 * cost as i64 / share.max(1) as i64) / cost as i64) as i32
        };
        let mut limit = RESOURCE_LIMIT_UNLIMITED;
        if wood != 0 {
            limit = limit.min(quota(realm.wood, wood, weapon_share.wood));
        }
        if iron != 0 {
            limit = limit.min(quota(realm.iron, iron, weapon_share.iron));
        }
        return limit.max(0);
    }
    if !record.has_resource || record.disabled_seasons != 0 {
        return 0;
    }
    RESOURCE_LIMIT_UNLIMITED
}

/// `FUN_0044F15B` — [`WeaponShare`] for one realm, summed over the counties
/// whose blacksmith is switched on **and staffed**. An idle smithy takes no
/// share, so switching one off gives the rest of the realm more iron.
pub fn weapon_shares(
    t: &Tables,
    counties: &[County],
    county_count: usize,
    realm: u8,
) -> WeaponShare {
    let mut share = WeaponShare { wood: 0, iron: 0 };
    for c in counties.iter().take(county_count + 1).skip(1) {
        if c.owner != realm
            || !c.industry[Commodity::Weapons.index()].enabled
            || c.labour[t.commodity[Commodity::Weapons.index()].job] <= 0
        {
            continue;
        }
        let weapon = c.weapon_type.min(WEAPON_TYPE_COUNT - 1);
        share.wood += t.weapon[weapon].wood;
        share.iron += t.weapon[weapon].iron;
    }
    WeaponShare { wood: share.wood.max(1), iron: share.iron.max(1) }
}

pub fn output(
    t: &Tables,
    county: &County,
    c: Commodity,
    realm: &Realm,
    weapon_share: WeaponShare,
) -> i32 {
    let record = &county.industry[c.index()];
    let workers = county.labour[t.commodity[c.index()].job].max(0);
    let raw = pct(workers / t.commodity[c.index()].divisor, record.efficiency);
    raw.min(resource_limit(t, county, c, realm, weapon_share)).max(0)
}

/// `Industry_LabourEstimate` (`0x0044F318`) — **one industry's labour
/// ceiling.**
///
/// `weapon_share` is [`weapon_shares`] for the owning realm; the original calls
/// `FUN_0044F15B` afresh inside every `resourceLimit`, so it is the *current*
/// state of the realm's smithies each time.
pub fn labour_estimate(
    t: &Tables,
    county: &County,
    c: Commodity,
    realm: &Realm,
    weapon_share: WeaponShare,
    advanced_farming: bool,
) -> (i32, i32) {
    const NONE: (i32, i32) = (crate::county::LABOUR_NO_FLOOR, 0);
    if county.owner == 0 || county.pop_band == 0 {
        return NONE;
    }
    let limit = resource_limit(t, county, c, realm, weapon_share);
    if limit <= 0 {
        return NONE;
    }
    if c != Commodity::Weapons {
        return (crate::county::LABOUR_NO_FLOOR, crate::county::LABOUR_UNBOUNDED);
    }

    let row = t.commodity[c.index()];
    let record = &county.industry[c.index()];
    let band = county.pop_band.max(1);
    let mut best = -1;
    let mut ceiling = 0;
    let mut trial = 0;
    while trial < county.population + band {
        let workers = trial.min(county.population);
        let efficiency = efficiency_ramp(
            t,
            // `FUN_0044F248` reads `+0x29C`, never `+0x294`.
            record.last_efficiency,
            workers,
            record.capacity,
            row.base_efficiency,
            advanced_farming,
        );
        let made = pct(workers / row.divisor, efficiency).min(limit);
        if best < made {
            best = made;
            ceiling = workers;
        }
        trial += band;
    }
    (crate::county::LABOUR_NO_FLOOR, ceiling)
}

/// ```c
/// county[0x2A8 + industry*0x18] = 0;                    /* before the guard */
/// if (owner == 0) return;
/// limit = resourceLimit(county, industry, owner, 0);
/// if (limit <= 0 || popBand == 0) return;
/// for (...) { local_28 = efficiencyRamp(county, industry, n, base); ... }   /* the loop */
/// iVar3   = efficiencyRamp(county, industry, labour[slot].workers, base);
/// county.industry[industry].efficiency = (char)iVar3;                       /* county +0x294 */
/// made    = Pct(labour[slot].workers / divisor, local_28);
/// if (made > limit) made = limit;
/// county[0x2A8 + industry*0x18] = made;                 /* [County::next_season] */
/// ```
///
/// C136 held it back because `County_RefreshEstimates` runs **four times**
/// inside `Industry_ToggleFromMap` alone. It is idempotent: the ramp reads
/// county `+0x29C` ([`crate::county::Industry::last_efficiency`]) and the
/// write-back writes `+0x294`, and only [`produce`] copies the one to the
/// other. Four refreshes ramp from the same season-old number and land on the
/// same answer. With *Advanced Farming* off the ramp is a flat 80 for every
/// staffing, so the write moves nothing.
///
/// * **County `+0x280` … `+0x28C`.** This said *"nothing reads them and no draw
/// call does either"*,
///   (`0x00412E6B`) draws all four under `L2.eng` group 76. They are not stored
///   here because they are pure functions of fields the county already holds —
///   [`panel_figures`] is them, and `docs/stored-fields.json` holds it to the
///   file's bytes in every county of every save.
pub fn preview(
    t: &Tables,
    county: &mut County,
    c: Commodity,
    realm: &Realm,
    weapon_share: WeaponShare,
    advanced_farming: bool,
) {
    let index = c.index();
    // `*(undefined4 *)(county * 0x300 + 0x53fc58 + industry * 0x18) = 0;` is
    // the function's **first** statement, outside every guard —
// that fails one of the three tests below forecasts nothing.
    county.industry[index].next_season = 0;
    if county.owner == 0 || county.pop_band == 0 {
        return;
    }
    let limit = resource_limit(t, county, c, realm, weapon_share);
    if limit <= 0 {
        return;
    }
    let row = t.commodity[index];
    let loop_efficiency = efficiency_ramp(
        t,
        county.industry[index].last_efficiency,
        county.population,
        county.industry[index].capacity,
        row.base_efficiency,
        advanced_farming,
    );
    let workers = county.labour[row.job].max(0);
    // `(&DAT_0053fc44)[industry*0x18 + county*0x300] = (char)iVar3;` — county
    // `+0x294` takes the ramp at the **real** staffing, computed on the line
    // before the forecast and used for nothing else. The `(char)` truncation
    // cannot bite: the ramp returns 0…100.
    county.industry[index].efficiency = efficiency_ramp(
        t,
        county.industry[index].last_efficiency,
        workers,
        county.industry[index].capacity,
        row.base_efficiency,
        advanced_farming,
    );
    county.industry[index].next_season = pct(workers / row.divisor, loop_efficiency).min(limit);
}

/// **`Industry_LabourEstimate` (`0x0044F318`), whole** — the search loop's two
/// words into the job's labour record,
///
/// [`labour_estimate`] and [`preview`] are the function's two halves, and they
/// have more than one caller in the original — `County_RefreshEstimates`
/// (`0x004485A5`) four times, `Industry_ProduceAll` (`0x0044E852`) after every
/// production pass, and `FUN_00448648` (the realm's blacksmiths, after a drop
/// or a switch) — so the pair is named once here
/// each of them. The efficiency write-back is in [`preview`].
pub fn refresh(
    t: &Tables,
    county: &mut County,
    c: Commodity,
    realm: &Realm,
    weapon_share: WeaponShare,
    advanced_farming: bool,
) {
    let job = t.commodity[c.index()].job;
    let (wanted, useful) = labour_estimate(t, county, c, realm, weapon_share, advanced_farming);
    county.labour_wanted[job] = wanted;
    county.labour_useful[job] = useful;
    preview(t, county, c, realm, weapon_share, advanced_farming);
}

pub fn produce(
    t: &Tables,
    county: &mut County,
    realm: &mut Realm,
    c: Commodity,
    advanced_farming: bool,
) {
    let share = WeaponShare::single_smith(t, county.weapon_type);
    produce_with_share(t, county, realm, c, advanced_farming, share)
}

pub fn produce_with_share(
    t: &Tables,
    county: &mut County,
    realm: &mut Realm,
    c: Commodity,
    advanced_farming: bool,
    weapon_share: WeaponShare,
) {
    let index = c.index();
    let previous_total = county.industry[index].total;

    if county.industry[index].disabled_seasons != 0 {
        county.industry[index].total = 0;
        county.industry[index].output = 0;
        county.industry[index].disabled_seasons -= 1;
        if county.industry[index].disabled_seasons < 1 {
            county.industry[index].enabled = true;
        }
        return;
    }

    let workers = county.labour[t.commodity[c.index()].job].max(0);
    county.industry[index].efficiency = efficiency_ramp(
        t,
        county.industry[index].last_efficiency,
        workers,
        county.industry[index].capacity,
        t.commodity[c.index()].base_efficiency,
        advanced_farming,
    );
    // `(&DAT_0053fc4c)[...] = (&DAT_0053fc44)[...];` — the season, and only the
    // season, advances the ramp's own input. `Industry_Produce` `0x0044EA92`.
    county.industry[index].last_efficiency = county.industry[index].efficiency;

    let mut made = output(t, county, c, realm, weapon_share);
    match c {
        Commodity::Wood => realm.wood += made,
        Commodity::Iron => realm.iron += made,
        Commodity::Stone => realm.stone += made,
        Commodity::Weapons => {
            let weapon = county.weapon_type.min(WEAPON_TYPE_COUNT - 1);
            let (wood, iron) = (t.weapon[weapon].wood, t.weapon[weapon].iron);
            if wood > 0 {
                made = made.min(realm.wood / wood);
            }
            if iron > 0 {
                made = made.min(realm.iron / iron);
            }
            made = made.max(0);
            realm.wood -= made * wood;
            realm.iron -= made * iron;
            realm.weapons[weapon] += made;
        }
    }
    county.industry[index].total = previous_total + made;
    county.industry[index].output = made;
}

