#![allow(unused_imports)]
use super::*;

use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum AiStep {
    /// `0x004A277D` — walk the realm's five-slot diplomatic inbox at
    /// `0x0053F0F0` and dispatch on each message's type byte, then clear the
    /// inbox and the realm's per-rival message counters.
    Diplomacy = 1,
    /// `0x004A0C1D` — age every rival's grudge counter, add to it for a rival
    /// who is winning or who holds a coveted county, and declare war when it
    /// passes the lord's personality threshold.
    ConsiderWar = 2,
    /// `0x0049D638` — `AI_SetTaxRates`: [`set_tax_rates`] and
    /// [`grant_resources`].
    SetTaxRates = 3,
    /// `0x0049E1BF` — total up what the realm's counties can sell and what it
    /// needs to buy, into realm `+0x70 … +0x7C`.
    ResourceWants = 4,
    /// `0x0049DD01` — `Ai_ManageCountyFarms`: order fields, then run the lord's
    /// farming style. **Not `AI_ManageFields`, which is `0x0049DFC6` and a
    /// different pass** — see the module documentation and
    /// [`crate::ai_farm::manage_county_farms`].
    ManageFields = 5,
    /// `0x0049EDC7` — `AI_BuildCastles`: order the largest castle the realm's
    /// gold clears, from five per-lord thresholds. [`build_castles`].
    BuildCastles = 6,
    /// `0x0049F93D` — three sub-passes over the realm's armies.
    ManageArmies = 7,
    /// `0x0049F96C` — **empty**. `void f(void) { return; }`.
    Nothing = 8,
    /// `0x0049F977` — raise men in the realm's chosen county and march them.
    RaiseArmy = 9,
    /// `0x004A0015` — create a type-7 unit and send it somewhere.
    SendUnit = 10,
    /// `0x004A5667` — walk every army towards its target tile.
    MoveArmies = 11,
    /// `0x0049E77D` — set every county's weapon type from a ten-step per-lord
    /// rota, switch the four industries on or off, and reallocate labour.
    ChooseIndustry = 12,
    /// `0x004A13A6` — `AI_Taunt`: gloat at the human when winning.
    Taunt = 13,
    /// `0x0049D1E0` — [`update_realm_totals`]. Also called by step 0.
    UpdateTotals = 14,
}

impl AiStep {
}
pub fn done_threshold(realm_index: usize) -> i32 {
    15 + 2 * realm_index as i32
}

/// `launched_army` is `FUN_004A4E3D(1, realm)`: it starts every idle army of
/// the realm moving and returns whether it found any. A realm at or past its
/// threshold is **not** finished while that keeps returning true, which is the
/// condition `docs/kingdom.md` §3.2 omits. Armies are not this crate's, so the
/// caller answers; passing `false` gives the pure-economy turn.
pub fn run_step(realm: &mut Realm, realm_index: usize, launched_army: bool) -> bool {
    if !realm.in_play || realm.is_human || realm.ai_step >= AI_STEP_DONE {
        return true;
    }
    if realm.ai_step >= done_threshold(realm_index) && !launched_army {
        realm.ai_step = AI_STEP_DONE;
        realm.ai_step += 1;
        return true;
    }
    realm.ai_step += 1;
    false
}

pub fn pending_step(realm: &Realm, realm_index: usize) -> Option<AiStep> {
    if realm.turn_done() || realm.ai_step >= done_threshold(realm_index) {
        return None;
    }
    AiStep::from_counter(realm.ai_step)
}

pub fn all_realms_done(realms: &[Realm]) -> bool {
    realms.iter().all(|r| !r.in_play || r.turn_done())
}

/// # This is `Turn_BeginPlayersTurn` (`0x0049B6D3`) with one deliberate difference
///
/// **The original resets a human realm's counter to 0 like any other**, because
/// `AI_RunTurnStep`'s step-0 prologue runs for every realm; the human then parks
/// at 1 — the counter's increment is inside the `isHuman == 0` guard — until
/// `Turn_End` (`0x0043AC23`) writes 999 when the person presses the button.
pub fn begin_turn(realms: &mut [Realm]) {
    for realm in realms.iter_mut() {
        realm.ai_step = if realm.in_play && !realm.is_human { 0 } else { AI_STEP_DONE };
    }
}

/// Step 0 — `FUN_0049B42B`, the initialisation above the dispatch.
///
/// ```c
/// Realm_RecountStrength(r);      /* 0x0049B42B: strength = 3*counties + armies,
///                                   elimination, then Score_RankRealms() */
/// Realm_UpdateTotals(r);         /* 0x0049D1E0: the six score inputs */
/// g_realms[r].offerPending = 0;  /* +0x1C */
/// g_realms[r].aiStep = 1;
/// ```
///
/// **This corrects `docs/kingdom.md` §2's `+0x04`.** The document calls it
/// `inPlay` and marks it `[V]`; it is a *weighted strength count*, three per
/// county and one per army, and "in play" is "that count is not zero". Every
/// other reader in the binary only tests it against zero, so the difference is
/// invisible until you try to write the field — which is exactly what a
/// reimplementation does. The count cannot overflow its byte: 16 counties and
/// 150 units is 198.
pub fn begin_realm_turn(realm: &mut Realm, owned_counties: u8, armies: u8) {
    realm.strength = 3u8.wrapping_mul(owned_counties).wrapping_add(armies);
    realm.in_play = realm.strength != 0;
    realm.ai_step = 1;
}

/// `L2.eng` group 193 — *"How are you doing?"*, the stage-0 taunt.
pub const TAUNT_HOW_ARE_YOU_DOING: u16 = 0xC1;
/// `L2.eng` group 192 — *"Helpful advice."*, the stage-1 taunt.
pub const TAUNT_HELPFUL_ADVICE: u16 = 0xC0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Taunt {
    pub from: u8,
    pub to: u8,
    pub group: u16,
    pub variant: u8,
}

/// `g_rankLeader` (`0x00553D24`) — the in-play realm with the **best** rank, or
/// 0.
pub fn rank_leader(realms: &[Realm]) -> u8 {
    let mut best = 0u8;
    let mut best_rank = u8::MAX;
    for (id, realm) in realms.iter().enumerate().take(6).skip(1) {
        if realm.in_play && realm.rank != 0 && realm.rank < best_rank {
            best_rank = realm.rank;
            best = id as u8;
        }
    }
    best
}

pub fn rank_trailer(realms: &[Realm]) -> u8 {
    let mut worst = 0u8;
    let mut worst_rank = 0u8;
    for (id, realm) in realms.iter().enumerate().take(6).skip(1) {
        if realm.in_play && realm.rank > worst_rank {
            worst_rank = realm.rank;
            worst = id as u8;
        }
    }
    worst
}

pub fn uses_small_gold_table(county_count: u8) -> bool {
    county_count < AI_GOLD_GRANT_SMALL_COUNTIES
}

/// `Score_RankRealms` (`0x0049AA0E`) — rebuild every realm's score, then rank
/// them 1..=5.
///
/// The original bubble-sorts realms 1..=5 into a table and writes the rank back
/// to `+0x2B`. Ranking is done here without sorting anything: for each realm,
/// its rank is one plus the number of realms that beat it, where "beats" is a
/// strictly higher score or an equal score at a lower realm index. That is the
/// same ordering a stable bubble sort produces and it cannot depend on the
/// starting arrangement, which a sort can.
pub fn rank_realms(t: &Tables, realms: &mut [Realm]) {
    for realm in realms.iter_mut() {
        realm.score = realm.compute_score(t);
    }
    let scores: Vec<(bool, i32)> = realms.iter().map(|r| (r.in_play, r.score)).collect();
    for i in 1..realms.len() {
        if !scores[i].0 {
            realms[i].rank = 0;
            continue;
        }
        let mut ahead = 0;
        for j in 1..scores.len() {
            if i == j || !scores[j].0 {
                continue;
            }
            if scores[j].1 > scores[i].1 || (scores[j].1 == scores[i].1 && j < i) {
                ahead += 1;
            }
        }
        realms[i].rank = (ahead + 1) as u8;
    }
}

