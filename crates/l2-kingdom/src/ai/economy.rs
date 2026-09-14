#![allow(unused_imports)]
use super::*;

use economy::*;
use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

/// The fourteen dispatch targets, named from the decompilation, with the
/// address of each. See the module documentation for the table and for which
/// are implemented.
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
    /// [`choose_industry`].
    ChooseIndustry = 12,
    /// `0x004A13A6` — `AI_Taunt`: gloat at the human when winning.
    /// **`docs/kingdom.md` §3.2's "offer or break an alliance" is wrong** —
    /// alliances are step 2's business. [`taunt`].
    Taunt = 13,
    /// `0x0049D1E0` — [`update_realm_totals`]. Also called by step 0.
    UpdateTotals = 14,
}

impl AiStep {
}
/// A realm is finished once its step counter **reaches** `15 + 2 * realmIndex`
/// — the original's test is `15 + 2*realm <= aiStep`, not `<`.
///
/// The `2 * realmIndex` term is reproduced: it means the
/// later realms in the array get two extra idle steps each. Since steps above
/// 14 dispatch nowhere, what those extra steps buy is extra chances
/// for the army-launch condition below to come good.
pub fn done_threshold(realm_index: usize) -> i32 {
    15 + 2 * realm_index as i32
}

/// One step of one realm's turn. Returns `true` once the realm is finished.
///
/// `launched_army` is `FUN_004A4E3D(1, realm)`: it starts every idle army of
/// the realm moving and returns whether it found any. A realm at or past its
/// threshold is **not** finished while that keeps returning true, which is the
/// condition `docs/kingdom.md` §3.2 omits. Armies are not this crate's, so the
/// caller answers; passing `false` gives the pure-economy turn.
///
/// The step the caller should run is [`AiStep::from_counter`] of the counter
/// *before* the increment.
pub fn run_step(realm: &mut Realm, realm_index: usize, launched_army: bool) -> bool {
    if !realm.in_play || realm.is_human || realm.ai_step >= AI_STEP_DONE {
        return true;
    }
    if realm.ai_step >= done_threshold(realm_index) && !launched_army {
        realm.ai_step = AI_STEP_DONE;
        // **Reproduced bug.** The increment below the dispatch is outside the
// `if` that writes the sentinel, so the value that lands in
        // the record is 1000. Every later test is `< 999`, so nothing breaks —
        // but a reader comparing a save against `== 999` finds nothing, which
// is worth having in the model.
        realm.ai_step += 1;
        return true;
    }
    realm.ai_step += 1;
    false
}

/// The step [`run_step`] is about to run, or `None` for the idle steps above
/// 14 and for a realm that is done.
pub fn pending_step(realm: &Realm, realm_index: usize) -> Option<AiStep> {
    if realm.turn_done() || realm.ai_step >= done_threshold(realm_index) {
        return None;
    }
    AiStep::from_counter(realm.ai_step)
}

/// `Turn_AllRealmsDone` — what phase 4 waits on.
///
/// The original's test is `aiStep == 999`; because of the increment bug the
/// stored value is 1000, so [`Realm::turn_done`] tests `>= 999`.
///
/// **A realm that is not in play is not waited on**, which is the original's own
/// condition and not a softening of it:
///
/// ```c
/// if ((g_realms[i].strength != 0) && (g_realms[i].aiStep < 999)) g_realmsActive++;
/// ```
///
/// It used to read `all(turn_done)`, which is the same answer as long as
/// [`begin_turn`] is the only thing that ever writes `in_play` — it sets the
/// sentinel on every realm it takes out. **A realm eliminated *during* phase 4
/// breaks that**: it is out of play with its counter still at 0, and phase 4
/// would then wait on a realm that will never take another step. That is exactly
/// what happens the turn somebody loses their last county, so it is the turn the
/// game ends that would have hung.
pub fn all_realms_done(realms: &[Realm]) -> bool {
    realms.iter().all(|r| !r.in_play || r.turn_done())
}

/// Reset every realm's program counter at the start of phase 4.
///
/// # This is `Turn_BeginPlayersTurn` (`0x0049B6D3`) with one deliberate difference
///
/// ```c
/// for (r = 1; r < 6; r++) {
///     g_realms[r].aiStep = 0;
///     if (g_realms[r].strength == 0) g_realms[r].aiStep = 999;
///     else g_realmsActive++;
/// }
/// ```
///
/// **The original resets a human realm's counter to 0 like any other**, because
/// `AI_RunTurnStep`'s step-0 prologue runs for every realm; the human then parks
/// at 1 — the counter's increment is inside the `isHuman == 0` guard — until
/// `Turn_End` (`0x0043AC23`) writes 999 when the person presses the button.
///
/// We write [`AI_STEP_DONE`] straight away instead, because `end_turn` **is** the
/// button: a headless turn has no person to wait for, so the human's counter is
/// 999 the moment phase 4 opens.
///
/// **What that used to cost, and no longer does.** This was read as the reason
/// the human's score never moved — a human takes no AI step, so step 14's
/// `Realm_UpdateTotals` never runs on one. It is not: the prologue is not a
/// *step*, it is the initialisation above the dispatch, and
/// `l2_game::turn::step_zero` runs it for every realm regardless of this
/// counter. `realm.ai_step` is the only field this divergence touches, and
/// `crates/l2-game/tests/differential.rs` excludes it for an unrelated reason
/// (the original's autosave samples it mid-phase).
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
/// **This function is only the first of those four lines**
/// player-visible: the second line is the only thing in the original that fills
/// the score inputs
/// scored 50 for ever. The whole prologue, at the call site the original puts it,
/// is `l2_game::turn::step_zero`; this has no production caller and is the
/// crate's own statement of the strength half.
///
/// **This corrects `docs/kingdom.md` §2's `+0x04`.** The document calls it
/// `inPlay` and marks it `[V]`; it is a *weighted strength count*, three per
/// county and one per army, and "in play" is "that count is not zero". Every
/// other reader in the binary only tests it against zero, so the difference is
/// invisible until you try to write the field — which is exactly what a
/// reimplementation does. The count cannot overflow its byte: 16 counties and
/// 150 units is 198.
///
/// It also fixes where `Score_RankRealms` runs. `docs/kingdom.md` §3.4 lists it
/// in the `Season_Advance` pipeline; it is **not in that call list at all**.
/// Its five callers are `Turn_Tick`, `Game_NewGame`, `Turn_AdvancePhase`, this
/// function, and one UI path.
pub fn begin_realm_turn(realm: &mut Realm, owned_counties: u8, armies: u8) {
    realm.strength = 3u8.wrapping_mul(owned_counties).wrapping_add(armies);
    realm.in_play = realm.strength != 0;
    realm.ai_step = 1;
}

/// `L2.eng` group 193 — *"How are you doing?"*, the stage-0 taunt.
pub const TAUNT_HOW_ARE_YOU_DOING: u16 = 0xC1;
/// `L2.eng` group 192 — *"Helpful advice."*, the stage-1 taunt.
pub const TAUNT_HELPFUL_ADVICE: u16 = 0xC0;

/// One letter [`taunt`] sends. `variant` is `lord * 4 + rotation - 4`, which
/// picks which of the lord's four recorded takes plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Taunt {
    pub from: u8,
    pub to: u8,
    pub group: u16,
    pub variant: u8,
}

/// `g_rankTrailer` — the in-play realm with the **worst** rank, or 0.
///
/// The original keeps it as a global that `Score_RankRealms` writes; deriving it
/// from the ranks [`rank_realms`] just wrote is the same answer with one fewer
/// thing to keep in step. Ties go to the lower index, which is the ordering
/// [`rank_realms`] already guarantees is unique among in-play realms.
/// `g_rankLeader` (`0x00553D24`) — the in-play realm with the **best** rank, or
/// 0.
///
/// The mirror of [`rank_trailer`], derived the same way and for the same
/// reason. `AI_Diplomacy`'s envy rung reads it: an alliance with the realm that
/// is winning accumulates grudge, so the leader's friend eventually stops being
/// one.
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

/// Whether a realm draws from the small gold table — fewer than three counties.
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

