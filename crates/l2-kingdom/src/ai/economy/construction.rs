#![allow(unused_imports)]
use super::*;
use super::steps::*;
use super::taxes::*;
use super::grants::*;
use super::industry::*;
use super::diplomacy::*;
use super::*;
use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

/// Step 6 — `AI_BuildCastles` (`0x0049EDC7`).
///
/// For every county the realm holds that has **no castle**, is at or above the
/// lord's population floor, and while the realm has fewer builds running than
/// the lord's concurrency limit, order the **largest** castle type whose gold
/// threshold the treasury clears.
///
/// The ladder is walked from type 5 down, and a **zero threshold means the type
/// is not offered to that lord at all**. Read against
/// [`crate::tables::AI_PERSONALITY_CASTLE_GOLD`], the four lords are sharply
/// different: two of them have a non-zero entry in the top slot and two do not,
/// so **two of the four can never build the largest castle however rich they
/// get**.
///
/// The concurrency count is realm `+0x4D`, which `Castle_BuildTick`
/// (`0x004508DE`) rebuilds every season as *the number of the realm's counties
/// with a build in progress*. It is derived here, for the
/// same reason the original derives it: a stored copy would be a second source
/// of truth for something one loop already answers. `[V]` on what `+0x4D`
/// counts — `Castle_BuildTick` zeroes both `+0x4C` and `+0x4D` and increments
/// `+0x4C` for a finished castle and `+0x4D` for one under construction.
///
/// The limit is tested **inside** the county loop and the count is not
/// refreshed as builds are ordered
/// castle-less counties can start five builds in one pass if he began the pass
/// with none. Reproduced.
///
/// Returns the counties a build was started in.
pub fn build_castles(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    realms: &mut [Realm],
    realm_id: u8,
) -> Vec<u8> {
    let mut started = Vec::new();
    let Some(realm) = realms.get(realm_id as usize) else { return started };
    let Some(p) = t.ai_personality(realm.lord) else { return started };
    let (min_population, concurrent) = (p.castle_min_population, p.castle_concurrent);
    let gold_ladder = p.castle_gold;
    let in_progress = counties[1..=county_count.min(counties.len() - 1)]
        .iter()
        .filter(|c| c.owner == realm_id && c.castle_degraded != 0)
        .count() as i32;
    if in_progress >= concurrent {
        return started;
    }
    for id in 1..=county_count.min(counties.len() - 1) {
        if counties[id].owner != realm_id
            || counties[id].population < min_population
            || counties[id].castle_type != 0
        {
            continue;
        }
        let gold = realms[realm_id as usize].gold;
        let Some(castle_type) = largest_castle_affordable(&gold_ladder, gold) else { continue };
        let (county, realm) = (&mut counties[id], &mut realms[realm_id as usize]);
        if crate::industry::order_castle(t, county, realm, castle_type) {
            started.push(id as u8);
        }
    }
    started
}

/// The castle type an AI lord orders at `gold`, or `None`.
///
/// Walked from the top down; a zero threshold takes the type out of the ladder
/// entirely, so the search continues past it.
pub fn largest_castle_affordable(ladder: &[i32; AI_CASTLE_LADDER_LEN], gold: i32) -> Option<u8> {
    for slot in (0..AI_CASTLE_LADDER_LEN).rev() {
        if ladder[slot] != 0 && gold >= ladder[slot] {
            return Some(slot as u8 + 1);
        }
    }
    None
}

/// The industry slots a county with a castle going up may still run.
///
/// Iron and weapons are switched off outright; wood and stone survive **only
/// while the build still owes some**, which is county `+0x1D4` and `+0x1D0`
/// read literally:
///
/// ```c
/// castleDegraded == 0
///   || (slot != 1 && slot != 2
///       && (slot != 0 || county.woodOwed  > 0)
///       && (slot != 3 || county.stoneOwed > 0))
/// ```
///
/// The old comment here recorded a departure — this crate had no owed-materials
/// counters, so both tests were evaluated as constants. It has them now, so
/// this is the original, and it is the third
/// independent confirmation that `+0x1D4` is wood and `+0x1D0` stone: the two
/// tests are keyed on industry slots 0 and 3, which `Industry_Produce` fixes as
/// wood and stone. `[V]`
pub(super) fn castle_allows(slot: usize, county: &County) -> bool {
    use crate::tables::Commodity;
    match slot {
        s if s == Commodity::Wood as usize => county.castle_wood_owed > 0,
        s if s == Commodity::Stone as usize => county.castle_stone_owed > 0,
        _ => false,
    }
}

