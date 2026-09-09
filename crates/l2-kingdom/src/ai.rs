//! The AI realm's turn, its advantages, and scoring — `docs/kingdom.md` §3.2
//! and §8.
//!
//! # The fourteen handlers
//!
//! `AI_RunTurnStep` (`0x0049A581`) picks the next in-play realm round-robin and
//! dispatches on that realm's `+0x00`. `docs/kingdom.md` §12 calls the handlers
//! *"the single largest remaining piece of the kingdom layer"* and records that
//! **none was decompiled**. All fourteen are decompiled now, and [`AiStep`]
//! names them with their addresses.
//!
//! | step | address | what it does | here? |
//! |---:|---|---|---|
//! | *0* | `0x0049B42B` | recount realm strength, check elimination, rank realms | [`begin_realm_turn`] |
//! | 1 | `0x004A277D` | `Diplo_AnswerInbox` — answer the five pending diplomatic messages | no — **blocked**: there is no inbox and no reply handlers |
//! | 2 | `0x004A0C1D` | `AI_Diplomacy` — accumulate grudges; break or offer an alliance | no — **blocked**: needs `form_alliance` / `break_alliance` / `offend` |
//! | 3 | `0x0049D638` | **`AI_SetTaxRates`** — set tax rates, grant resources | [`set_tax_rates`], [`grant_resources`] |
//! | 4 | `0x0049E1BF` | work out what the realm wants to buy | no — the four `want` fields have no consumer until the merchant lands |
//! | 5 | `0x0049DD01` | **`Ai_ManageCountyFarms`** — order fields, then the lord's farming style | [`crate::ai_farm::manage_county_farms`] |
//! | 6 | `0x0049EDC7` | **`AI_BuildCastles`** — order the biggest castle the treasury clears | [`build_castles`] |
//! | 7 | `0x0049F93D` | three army-management passes | partly — pass 3 is **blocked** on transports and merchant trade |
//! | 8 | `0x0049F96C` | **nothing: the function is empty** | [`AiStep::is_empty`] |
//! | 9 | `0x0049F977` | raise and move the main army | no — needs eight new fields and `Diplo_ActionAllowed` |
//! | 10 | `0x004A0015` | send out the raiding force | no — needs the unit *mission* byte |
//! | 11 | `0x004A5667` | walk every army towards its target | no — **blocked** on the six unit-mission handlers behind `FUN_004A57AC` |
//! | 12 | `0x0049E77D` | **`AI_ChooseIndustry`** — the weapon rota, and the industry switches | [`choose_industry`] |
//! | 13 | `0x004A13A6` | **`AI_Taunt`** — gloat at the human when winning | [`taunt`] |
//! | 14 | `0x0049D1E0` | **recompute the realm's totals** | [`update_realm_totals`] |
//!
//! Step 8 being empty is worth stating as a finding rather than an omission:
//! one of the fourteen slots does nothing in the shipped binary.
//!
//! # The two names that were one name
//!
//! This table used to give step 5 as *"`AI_ManageFields` (`0x0049DD01`)"*.
//! **Those are two different functions and the mistake hid two whole
//! allocators.** `Ai_ManageCountyFarms` (`0x0049DD01`) is step 5 and dispatches
//! styles 0, 1 and 9 into three allocators; `AI_ManageFields` (`0x0049DFC6`) has
//! exactly one caller, `AI_ManageFields(0)` in turn phase 1, and dispatches
//! styles 0 and 1 into **two others**. There are five allocators in all, and the
//! one previously described as *"the AI's farming style"* is the one no AI realm
//! can reach. [`crate::ai_farm`] carries the correction and all five.
//!
//! # The turn's shape, corrected
//!
//! `docs/kingdom.md` §3.2 says the counter runs into fourteen handlers and that
//! *"when it passes `15 + 2 * realmIndex` the realm is marked 999"*. Three
//! details it does not have, all of which change the behaviour:
//!
//! 1. **Step 0 is not a handler.** It is a one-off initialisation — the `if
//!    (aiStep == 0)` branch above the dispatch — which recounts the realm's
//!    strength, eliminates it if it has nothing left, ranks every realm and
//!    then sets `aiStep = 1`. The dispatch covers 1..=14.
//! 2. **The finish test is `>=`, not `>`, and it is not the only condition.**
//!    The realm finishes when `aiStep >= 15 + 2 * realmIndex` *and*
//!    `FUN_004A4E3D(1, realm)` returns 0 — a call that sets every one of the
//!    realm's idle armies moving and reports whether it found any. So a realm
//!    with armies still to launch keeps stepping past the threshold, doing
//!    nothing at each stop, until they are all away.
//! 3. **The stored sentinel ends up 1000, not 999.** The increment at the
//!    bottom of the function is outside the `if` that writes 999, so the value
//!    saved is one more. Nothing breaks, because every later test is
//!    `aiStep < 999`; but a reader comparing `== 999` against a save would find
//!    nothing. Reproduced — see [`run_step`].
//!
//! Points 2 and 3 are read off the **disassembly**, not the decompiler:
//! `0049A933 CMP [aiStep],ECX / JL` with `ECX = 2*realm + 15`, then
//! `CALL 0x004A4E3D / TEST EAX,EAX / JNZ`, then `MOV [aiStep],0x3E7` — and
//! `0049A985 INC dword ptr [aiStep]` sits at the target both jumps skip to, so
//! it runs either way. `docs/decisions.md` C13: the artefact is the evidence.
//!
//! # `AI_SetTaxRates`
//!
//! `AI_SetTaxRates` (`0x0049D638`) does two things and `docs/kingdom.md` §8.2
//! gives neither in full. Its **four tax ladders** are
//! [`crate::tables::AiTable::tax_ladder_neutral`] and
//! [`crate::tables::AiTable::tax_ladders`]; its **resource grants** are tiered
//! by the realm's county count, which the document does not mention. Both are
//! implemented, and both are read out of the [`Tables`] the caller hands in
//! rather than out of a constant — the ladders are `if`/`else if` chains in the
//! original, so a ruleset is the first time they have been data at all.

use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

/// The number of handlers `AI_RunTurnStep` dispatches into: `aiStep` 1..=14.
pub const AI_HANDLER_COUNT: i32 = 14;

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
    pub const ALL: [AiStep; 14] = [
        AiStep::Diplomacy,
        AiStep::ConsiderWar,
        AiStep::SetTaxRates,
        AiStep::ResourceWants,
        AiStep::ManageFields,
        AiStep::BuildCastles,
        AiStep::ManageArmies,
        AiStep::Nothing,
        AiStep::RaiseArmy,
        AiStep::SendUnit,
        AiStep::MoveArmies,
        AiStep::ChooseIndustry,
        AiStep::Taunt,
        AiStep::UpdateTotals,
    ];

    pub fn from_counter(step: i32) -> Option<AiStep> {
        AiStep::ALL.get(usize::try_from(step.checked_sub(1)?).ok()?).copied()
    }

    pub fn counter(self) -> i32 {
        self as i32
    }

    /// The address of the handler in `Lords2.exe`, so a future differential
    /// test knows where to look.
    pub fn address(self) -> u32 {
        match self {
            AiStep::Diplomacy => 0x004A277D,
            AiStep::ConsiderWar => 0x004A0C1D,
            AiStep::SetTaxRates => 0x0049D638,
            AiStep::ResourceWants => 0x0049E1BF,
            AiStep::ManageFields => 0x0049DD01,
            AiStep::BuildCastles => 0x0049EDC7,
            AiStep::ManageArmies => 0x0049F93D,
            AiStep::Nothing => 0x0049F96C,
            AiStep::RaiseArmy => 0x0049F977,
            AiStep::SendUnit => 0x004A0015,
            AiStep::MoveArmies => 0x004A5667,
            AiStep::ChooseIndustry => 0x0049E77D,
            AiStep::Taunt => 0x004A13A6,
            AiStep::UpdateTotals => 0x0049D1E0,
        }
    }

    /// True for the one step whose handler is an empty function in the shipped
    /// binary — a fact, not a gap in this crate.
    pub fn is_empty(self) -> bool {
        self == AiStep::Nothing
    }

    /// True where this crate actually runs the step. The rest are named and
    /// described and do nothing, because what they drive — armies, merchants,
    /// diplomacy, map tiles — is not this crate's state.
    pub fn is_implemented(self) -> bool {
        matches!(
            self,
            AiStep::SetTaxRates
                | AiStep::ManageFields
                | AiStep::BuildCastles
                | AiStep::ChooseIndustry
                | AiStep::Taunt
                | AiStep::UpdateTotals
                | AiStep::Nothing
        )
    }
}

/// A realm is finished once its step counter **reaches** `15 + 2 * realmIndex`
/// — the original's test is `15 + 2*realm <= aiStep`, not `<`.
///
/// The `2 * realmIndex` term is reproduced rather than tidied: it means the
/// later realms in the array get two extra idle steps each. Since steps above
/// 14 dispatch nowhere, what those extra steps actually buy is extra chances
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
        // `if` that writes the sentinel, so the value that actually lands in
        // the record is 1000. Every later test is `< 999`, so nothing breaks —
        // but a reader comparing a save against `== 999` finds nothing, which
        // is worth having in the model rather than tidying away.
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
pub fn begin_turn(realms: &mut [Realm]) {
    for realm in realms.iter_mut() {
        realm.ai_step = if realm.in_play && !realm.is_human { 0 } else { AI_STEP_DONE };
    }
}

/// Step 0 — `FUN_0049B42B`, the initialisation above the dispatch.
///
/// ```c
/// strength = 3 * ownedCounties + 1 * armies;      /* realm +0x04 */
/// if (strength == 0) the realm is eliminated and everyone is told;
/// Score_RankRealms();
/// ```
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

/// `AI_SetTaxRates`' first half — set every county the realm owns to the rate
/// its happiness earns, on one of four ladders.
///
/// `realm_lord` is the owning realm's `lord` byte, and `realm` **0 means the
/// unowned counties**, which phase 1 (`docs/kingdom.md` §3.1) runs once a turn
/// with [`crate::tables::AiTable::tax_ladder_neutral`]. An AI realm uses the
/// ladder its lord's personality names — see [`Tables::ai_tax_ladder`].
///
/// The four ladders are the piece `docs/kingdom.md` §8.2 says exists and does
/// not give. What they say, in one line each:
///
/// * **neutral** — eight rungs from 0% below 20 happiness up to 12% at 90;
/// * **ladder 0** — the greediest, 15% on anything at 80 or above;
/// * **ladder 1** — ladder 0 softened, topping out at 12%;
/// * **ladder 2** — the gentlest and the one three of the four lords use:
///   nothing at all below 60 happiness, and 10% only above 95.
///
/// Note that a *neutral* county is taxed harder at low happiness than any AI
/// taxes its own — 1% at 20 happiness where every lord's ladder charges
/// nothing below 30. Nobody is collecting it, though: `Tax_CollectAll` banks an
/// unowned county's take into the county itself rather than into a treasury.
pub fn set_tax_rates(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    realm: u8,
    realm_lord: u8,
) {
    let ladder = if realm == 0 {
        Some(&t.ai.tax_ladder_neutral)
    } else {
        t.ai_tax_ladder(realm_lord)
    };
    // A lord with no personality record sets no rates at all, rather than
    // falling back to a ladder that was never established. See
    // `crate::tables::AI_PERSONALITY_COUNT`.
    let Some(ladder) = ladder else { return };
    for id in 1..=county_count {
        if counties[id].owner != realm {
            continue;
        }
        counties[id].tax_rate = tax_rate_for(ladder, counties[id].happiness);
    }
}

/// Step 14 — `FUN_0049D1E0`, which recomputes the realm-wide totals every other
/// pass reads.
///
/// This is where realm `+0x29` (the county count the grant tiers turn on) comes
/// from, and **five of the six score inputs `docs/kingdom.md` §8.3 could not
/// identify** — see [`crate::tables::SCORE_INPUT_OFFSETS`].
///
/// ```c
/// countyCount = 0; totalPopulation = 0; sumHappiness = 0; sumHealth = 0;
/// for each owned county { countyCount++; totalPopulation += pop;
///                         sumHappiness += happiness; sumHealth += healthMeter; }
/// meanPopulation = totalPopulation / countyCount;      /* +0x14 */
/// shareOfMap     = PctOf(countyCount, g_countyCount);  /* +0x60 */
/// meanHappiness  = sumHappiness / countyCount;         /* +0x0C */
/// meanHealth     = sumHealth / countyCount;            /* +0x58 */
/// ```
///
/// Every division is guarded on `countyCount != 0` and writes 0 instead, so a
/// realm about to be eliminated does not divide by zero.
///
/// `armies` and `total_men` come from the unit array, which is not this
/// crate's; the caller supplies them and they land in `+0x2C` and `+0x54`.
pub fn update_realm_totals(
    realm: &mut Realm,
    counties: &[County],
    county_count: usize,
    realm_id: u8,
    armies: u8,
    total_men: i32,
) {
    realm.population_last = realm.population_total;
    realm.county_count = 0;
    realm.population_total = 0;
    let mut sum_happiness: i64 = 0;
    let mut sum_health: i64 = 0;
    for id in 1..=county_count {
        if counties[id].owner != realm_id {
            continue;
        }
        realm.county_count += 1;
        realm.population_total += counties[id].population;
        sum_happiness += counties[id].happiness as i64;
        sum_health += counties[id].health_meter as i64;
    }
    let n = realm.county_count as i64;
    if n == 0 {
        realm.population_mean = 0;
        realm.share_of_map_pct = 0;
        realm.mean_happiness = 0;
        realm.mean_health = 0;
    } else {
        realm.population_mean = (realm.population_total as i64 / n) as i32;
        realm.share_of_map_pct =
            crate::industry::pct_of(realm.county_count as i32, county_count as i32);
        realm.mean_happiness = (sum_happiness / n) as i32;
        realm.mean_health = (sum_health / n) as i32;
    }
    realm.army_count = armies;
    realm.total_men = total_men;
    realm.sync_score_inputs();
}

/// `AI_SetTaxRates`' second half — the AI's free resources, per county, per
/// season.
///
/// **Three corrections to `docs/kingdom.md` §8.2**, all from the same function:
///
/// 1. **The goods grant is tiered by the realm's county count**, not flat. §8.2
///    quotes `difficulty * 20` people, `* 5` head and `* 40` sacks; those are
///    the figures for a realm holding **one or two** counties. Three or four
///    counties halve them; **five or more get nothing at all.** See
///    [`crate::tables::AI_GRANT_TIERS`].
/// 2. **The gold grant has two tables**, and the smaller
///    ([`crate::tables::AI_GOLD_GRANT_SMALL`]) is the one a realm below three
///    counties draws from. §8.2 mentions the second table; the crate did not
///    have it.
/// 3. **The whole grant is gated on the realm holding at least one county.**
///    `if (realm.countyCount != 0)` wraps both halves, so a realm reduced to
///    armies alone gets neither gold nor goods.
///
/// Taken together the grants **reward a realm that is already ahead** and
/// abandon one that is losing — the opposite of the rubber-banding the phrase
/// "the AI's advantages" suggests.
///
/// Each county's share is still gated on the county already having some, so it
/// compounds rather than rescues.
///
/// The realm's `county_count` must be current: [`update_realm_totals`] is what
/// sets it, and it is step 14 of the *previous* turn.
pub fn grant_resources(
    t: &Tables,
    counties: &mut [County],
    realms: &mut [Realm],
    county_count: usize,
    difficulty: u8,
) {
    let d = difficulty as i32;
    for id in 1..=county_count {
        let owner = counties[id].owner as usize;
        if owner == 0 || owner >= realms.len() {
            continue;
        }
        let realm = &realms[owner];
        if realm.is_human || !realm.in_play || realm.county_count == 0 {
            continue;
        }
        let (people_per, herd_per, grain_per) = ai_grant_tier(realm.county_count);
        let _ = t;
        let c = &mut counties[id];
        if c.population > t.ai.grant_min_population {
            let people = d * people_per;
            c.population += people;
            c.births += people;
        }
        if c.herd > t.ai.grant_min_herd {
            c.herd += d * herd_per;
        }
        if c.grain > t.ai.grant_min_grain {
            c.grain += d * grain_per;
        }
    }
    for realm in realms.iter_mut().skip(1) {
        if realm.in_play && !realm.is_human && realm.county_count != 0 {
            realm.gold += realm.gold_grant(t, difficulty);
        }
    }
}

/// Step 6 — `AI_BuildCastles` (`0x0049EDC7`).
///
/// For every county the realm holds that has **no castle**, is at or above the
/// lord's population floor, and while the realm has fewer builds running than
/// the lord's concurrency limit, order the **largest** castle type whose gold
/// threshold the treasury clears.
///
/// The ladder is walked from type 5 down, and a **zero threshold means the type
/// is not offered to that lord at all** rather than meaning free. Read against
/// [`crate::tables::AI_PERSONALITY_CASTLE_GOLD`], the four lords are sharply
/// different: two of them have a non-zero entry in the top slot and two do not,
/// so **two of the four can never build the largest castle however rich they
/// get**.
///
/// The concurrency count is realm `+0x4D`, which `Castle_BuildTick`
/// (`0x004508DE`) rebuilds every season as *the number of the realm's counties
/// with a build in progress*. It is derived here rather than stored, for the
/// same reason the original derives it: a stored copy would be a second source
/// of truth for something one loop already answers. `[V]` on what `+0x4D`
/// counts — `Castle_BuildTick` zeroes both `+0x4C` and `+0x4D` and increments
/// `+0x4C` for a finished castle and `+0x4D` for one under construction.
///
/// The limit is tested **inside** the county loop and the count is not
/// refreshed as builds are ordered, so a lord with a limit of 4 and five
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
/// entirely, so the search continues past it rather than treating it as free.
pub fn largest_castle_affordable(ladder: &[i32; AI_CASTLE_LADDER_LEN], gold: i32) -> Option<u8> {
    for slot in (0..AI_CASTLE_LADDER_LEN).rev() {
        if ladder[slot] != 0 && gold >= ladder[slot] {
            return Some(slot as u8 + 1);
        }
    }
    None
}

/// Step 12 — `FUN_0049E77D`, which this crate names `AI_ChooseIndustry`.
///
/// Three loops over the realm's counties, and the first is **the weapon rota**:
/// each county takes the weapon type at the realm's cursor and the cursor
/// advances. Because the cursor is a *realm* field advanced inside a loop over
/// counties, a realm of four counties makes four different weapons at once and
/// the pattern rotates from wherever it stopped last turn — which is why
/// [`Realm::weapon_rota`] has to be saved.
///
/// The second loop is **the industry switchboard**, and it is the interesting
/// one:
///
/// ```c
/// enabled = 0;
/// if (hasResource && disabledSeasons == 0 &&
///     (!castleBuilding
///      || (slot != iron && slot != weapons
///          && (slot != wood  || woodStillNeeded  > 0)
///          && (slot != stone || stoneStillNeeded > 0))))
///     enabled = 1;
/// ```
///
/// With the four slots in [`crate::tables::Commodity`] order that reads:
/// **while a castle is going up, the AI switches iron and the blacksmith off
/// outright**, and keeps forestry and quarrying on only while the build still
/// wants wood or stone. An AI at war stops making weapons the moment it starts
/// a castle, which is a real strategic quirk and not an obvious one.
///
/// `docs/kingdom.md` calls county `+0x1B0` untraced; this loop sets it to **1
/// on every county the realm holds, unconditionally**, and
/// `AI_ManageFields(0)` sets it to 0 on the unowned ones. That is
/// [`County::castle_switch`], and the pair of writes says what it is: *this
/// county's castle-building job slot is live*.
///
/// **The departure this used to record is closed.** It said the original reads
/// county `+0x1D4`/`+0x1D0` — the wood and stone a build still owes — and that
/// this crate had no such counter because it debited the whole cost up front.
/// The up-front debit was ours and it was wrong; the counters exist
/// ([`County::castle_wood_owed`]), and [`castle_allows`] reads them. So an AI
/// realm that has already delivered all the stone for its keep switches its
/// quarries off and leaves the forests running, which the constant could not
/// say.
///
/// The third loop is the labour re-allocation, which the caller does — this
/// crate's [`crate::labour::allocate`] is county-local and the caller already
/// walks the counties.
pub fn choose_industry(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    realm: &mut Realm,
    realm_id: u8,
) {
    let Some(p) = t.ai_personality(realm.lord) else { return };
    let rota = p.weapon_rota;
    for id in 1..=county_count.min(counties.len() - 1) {
        if counties[id].owner != realm_id {
            continue;
        }
        let cursor = realm.weapon_rota.clamp(0, AI_WEAPON_ROTA_ORDER.len() as i32 - 1) as usize;
        counties[id].weapon_type = rota[AI_WEAPON_ROTA_ORDER[cursor]];
        realm.weapon_rota += 1;
        if realm.weapon_rota > 9 {
            realm.weapon_rota = 0;
        }
        // `FUN_0049ED13`: the blacksmith's "has resource" is not geology, it is
        // **affordability** — can the realm pay this county's chosen weapon out
        // of its wood and iron?
        let row = t.weapon[counties[id].weapon_type.min(t.weapon.len() - 1)];
        counties[id].industry[crate::tables::Commodity::Weapons as usize].has_resource =
            row.wood <= realm.wood && row.iron <= realm.iron;
    }
    for id in 1..=county_count.min(counties.len() - 1) {
        if counties[id].owner != realm_id {
            continue;
        }
        let building = counties[id].castle_degraded != 0;
        for slot in 0..counties[id].industry.len() {
            let record = &counties[id].industry[slot];
            let free = record.has_resource && record.disabled_seasons == 0;
            counties[id].industry[slot].enabled =
                free && (!building || castle_allows(slot, &counties[id]));
        }
        counties[id].castle_switch = true;
    }
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
/// this is the original rather than a stand-in for it, and it is the third
/// independent confirmation that `+0x1D4` is wood and `+0x1D0` stone: the two
/// tests are keyed on industry slots 0 and 3, which `Industry_Produce` fixes as
/// wood and stone. `[V]`
fn castle_allows(slot: usize, county: &County) -> bool {
    use crate::tables::Commodity;
    match slot {
        s if s == Commodity::Wood as usize => county.castle_wood_owed > 0,
        s if s == Commodity::Stone as usize => county.castle_stone_owed > 0,
        _ => false,
    }
}

/// Step 13 — `AI_Taunt` (`0x004A13A6`).
///
/// **`docs/kingdom.md` §3.2 has this step as *"offer or break an alliance"*. It
/// is not.** Alliances are step 2's business; step 13 is a two-stage gloat, and
/// the only thing it changes about the game state is a timer, a stage byte and
/// the voice rotation.
///
/// * Only a realm ranked **first** taunts at all (`rank < 2`).
/// * **Stage 0** — above 39% of the map, count to 8, then send *"How are you
///   doing?"* (`L2.eng` group 193) to **every** live human realm, reset the
///   timer and go to stage 1.
/// * **Stage 1** — above 27% of the map, and only if the realm in **last
///   place** is human and is not this realm's ally, count to 8, then send
///   *"Helpful advice."* (group 192) to that realm and go back to stage 0.
///
/// The two thresholds are `>` on 0x27 and 0x1B, and the timer test is `7 <
/// timer`, so it is the **ninth** consecutive qualifying turn that sends. The
/// timer only advances on a turn the share threshold is met, so a realm that
/// slips below 39% pauses rather than restarting.
///
/// `trailer` is the last-placed realm — `g_rankTrailer`, which the original
/// keeps as a global and which is derived by the caller from
/// [`rank_realms`]. `out` collects the letters; the caller decides what to do
/// with them, because a realm-to-realm letter is not a season message.
pub fn taunt(realm: &mut Realm, realm_id: u8, realms_snapshot: &[Realm], trailer: u8) -> Vec<Taunt> {
    let mut sent = Vec::new();
    if realm.rank >= 2 {
        return sent;
    }
    if realm.taunt_stage == 0 {
        if realm.share_of_map_pct <= 39 {
            return sent;
        }
        realm.taunt_timer = realm.taunt_timer.saturating_add(1);
        if realm.taunt_timer <= 7 {
            return sent;
        }
        for (id, other) in realms_snapshot.iter().enumerate().take(6).skip(1) {
            if id as u8 != realm_id && other.in_play && other.is_human {
                sent.push(Taunt {
                    from: realm_id,
                    to: id as u8,
                    group: TAUNT_HOW_ARE_YOU_DOING,
                    variant: realm.message_variant(),
                });
                realm.advance_voice();
            }
        }
        realm.taunt_timer = 0;
        realm.taunt_stage = 1;
        return sent;
    }
    let Some(target) = realms_snapshot.get(trailer as usize) else { return sent };
    if realm.share_of_map_pct <= 27 || !target.is_human || realm.ally == trailer {
        return sent;
    }
    realm.taunt_timer = realm.taunt_timer.saturating_add(1);
    if realm.taunt_timer <= 7 {
        return sent;
    }
    realm.taunt_timer = 0;
    realm.taunt_stage = 0;
    sent.push(Taunt {
        from: realm_id,
        to: trailer,
        group: TAUNT_HELPFUL_ADVICE,
        variant: realm.message_variant(),
    });
    realm.advance_voice();
    sent
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



#[cfg(test)]
mod tests {
    use super::*;

    /// The stock ruleset. Every rule below takes it as an argument now.
    const T: &Tables = &Tables::DEFAULT;
    use crate::realm::MAX_REALMS;
    use crate::tables::{AI_GOLD_GRANT, AI_GOLD_GRANT_SMALL};

    fn ai_realms() -> Vec<Realm> {
        let mut realms = vec![Realm::new(); MAX_REALMS];
        for i in 1..=3 {
            realms[i].in_play = true;
            realms[i].lord = i as u8;
            realms[i].county_count = 4;
        }
        realms
    }

    // --- the turn machine --------------------------------------------------

    /// **All fourteen dispatch targets are named**, each with the address it
    /// was decompiled from, and the counter maps onto them one for one.
    #[test]
    fn the_fourteen_handlers_are_named_and_numbered_one_to_fourteen() {
        assert_eq!(AiStep::ALL.len(), AI_HANDLER_COUNT as usize);
        for (i, step) in AiStep::ALL.iter().enumerate() {
            assert_eq!(step.counter(), i as i32 + 1);
            assert_eq!(AiStep::from_counter(step.counter()), Some(*step));
            assert!(step.address() >= 0x0049_0000, "{step:?} has no address");
        }
        assert_eq!(AiStep::from_counter(0), None, "step 0 is the initialisation");
        assert_eq!(AiStep::from_counter(15), None, "and 15 upwards dispatch nowhere");
        assert_eq!(AiStep::from_counter(-1), None);
    }

    /// One of the fourteen slots is an **empty function** in the shipped
    /// binary. Stated as a finding, so nobody later mistakes it for a gap here.
    #[test]
    fn step_eight_does_nothing_because_the_original_does_nothing() {
        assert!(AiStep::Nothing.is_empty());
        for step in AiStep::ALL {
            assert_eq!(step.is_empty(), step == AiStep::Nothing, "{step:?}");
        }
        assert_eq!(AiStep::Nothing.address(), 0x0049_F96C);
    }

    /// **Seven of the fourteen run.** The other seven are the army, merchant and
    /// diplomacy steps; the module table says which are blocked on a missing
    /// subsystem and which only want fields.
    #[test]
    fn the_implemented_steps_are_the_ones_that_touch_only_counties_and_realms() {
        let done: Vec<AiStep> =
            AiStep::ALL.iter().copied().filter(|s| s.is_implemented()).collect();
        assert_eq!(
            done,
            vec![
                AiStep::SetTaxRates,
                AiStep::ManageFields,
                AiStep::BuildCastles,
                AiStep::Nothing,
                AiStep::ChooseIndustry,
                AiStep::Taunt,
                AiStep::UpdateTotals,
            ]
        );
        let blocked: Vec<AiStep> =
            AiStep::ALL.iter().copied().filter(|s| !s.is_implemented()).collect();
        assert_eq!(
            blocked,
            vec![
                AiStep::Diplomacy,
                AiStep::ConsiderWar,
                AiStep::ResourceWants,
                AiStep::ManageArmies,
                AiStep::RaiseArmy,
                AiStep::SendUnit,
                AiStep::MoveArmies,
            ],
            "diplomacy, the merchant's wants, and the four army steps"
        );
    }

    #[test]
    fn an_ai_realm_walks_its_program_counter_through_every_handler() {
        let mut realms = ai_realms();
        begin_turn(&mut realms);
        assert!(!all_realms_done(&realms));

        begin_realm_turn(&mut realms[1], 4, 0);
        let mut visited = Vec::new();
        let mut guard = 0;
        loop {
            if let Some(step) = pending_step(&realms[1], 1) {
                visited.push(step);
            }
            if run_step(&mut realms[1], 1, false) {
                break;
            }
            guard += 1;
            assert!(guard < 100, "the turn should terminate");
        }
        assert_eq!(visited, AiStep::ALL.to_vec(), "every handler, in order, once");
        assert!(realms[1].turn_done());
    }

    /// **The sentinel written is 999 and the value stored is 1000** — the
    /// increment below the dispatch is outside the `if`. Reproduced.
    #[test]
    fn the_finished_counter_overshoots_the_sentinel_by_one() {
        let mut realms = ai_realms();
        begin_turn(&mut realms);
        while !run_step(&mut realms[1], 1, false) {}
        assert_eq!(realms[1].ai_step, AI_STEP_DONE + 1, "999, then incremented");
        assert!(realms[1].turn_done(), "which is why the test has to be >=");
    }

    /// **A realm with armies still to launch does not finish.** The threshold
    /// is necessary and not sufficient: `FUN_004A4E3D` has to come back empty
    /// too.
    #[test]
    fn a_realm_with_armies_left_to_launch_keeps_stepping() {
        let mut realms = ai_realms();
        begin_turn(&mut realms);
        // Well past the threshold, and still launching.
        for _ in 0..40 {
            assert!(!run_step(&mut realms[1], 1, true), "still has armies to send");
        }
        assert!(realms[1].ai_step > done_threshold(1));
        assert!(run_step(&mut realms[1], 1, false), "and now it is done");
    }

    /// The later realms in the array really do get more steps. Reproduced, not
    /// tidied.
    #[test]
    fn later_realms_get_two_extra_steps_each() {
        for index in 1..MAX_REALMS {
            assert_eq!(done_threshold(index), 15 + 2 * index as i32);
        }
        assert!(done_threshold(5) > done_threshold(1));
    }

    #[test]
    fn a_human_realm_is_skipped_entirely() {
        let mut realms = ai_realms();
        realms[2].is_human = true;
        begin_turn(&mut realms);
        assert_eq!(realms[2].ai_step, AI_STEP_DONE);
        assert!(run_step(&mut realms[2], 2, false), "and stays done");
    }

    #[test]
    fn phase_four_ends_only_when_every_realm_is_done() {
        let mut realms = ai_realms();
        begin_turn(&mut realms);
        for index in 1..=3 {
            while !run_step(&mut realms[index], index, false) {}
            let expected = index == 3;
            assert_eq!(all_realms_done(&realms), expected, "after realm {index}");
        }
    }

    /// **`+0x04` is a strength count, not a flag**: three per county plus one
    /// per army, and elimination is that count reaching zero.
    #[test]
    fn a_realm_is_eliminated_when_it_has_neither_a_county_nor_an_army() {
        let mut r = Realm::new();
        r.in_play = true;
        begin_realm_turn(&mut r, 4, 2);
        assert_eq!(r.strength, 14, "3*4 + 2");
        assert!(r.in_play);
        assert_eq!(r.ai_step, 1, "step 0 hands over to step 1");

        begin_realm_turn(&mut r, 0, 1);
        assert_eq!(r.strength, 1, "one army is enough to stay alive");
        assert!(r.in_play);

        begin_realm_turn(&mut r, 0, 0);
        assert_eq!(r.strength, 0);
        assert!(!r.in_play);
    }

    /// The count cannot overflow its byte on any legal map: sixteen counties
    /// and every one of the 150 units.
    #[test]
    fn the_strength_count_fits_a_byte_on_the_largest_possible_realm() {
        let mut r = Realm::new();
        begin_realm_turn(&mut r, 16, 150);
        assert_eq!(r.strength, 198);
        assert!(r.in_play);
    }

    // --- the four tax ladders ----------------------------------------------

    /// The neutral ladder, every rung, as `AI_SetTaxRates(0)` walks it.
    #[test]
    fn the_neutral_ladder_taxes_an_unowned_county_by_its_happiness() {
        let rate = |h: i32| tax_rate_for(&T.ai.tax_ladder_neutral, h);
        assert_eq!(rate(0), 0);
        assert_eq!(rate(19), 0);
        assert_eq!(rate(20), 1, "the first rung is at 20");
        assert_eq!(rate(39), 1);
        assert_eq!(rate(40), 2);
        assert_eq!(rate(49), 2);
        assert_eq!(rate(50), 3);
        assert_eq!(rate(60), 4);
        assert_eq!(rate(70), 6, "the ladder skips 5");
        assert_eq!(rate(80), 8, "and 7");
        assert_eq!(rate(90), 12, "and 9, 10 and 11");
        assert_eq!(rate(100), 12);
    }

    /// The three AI ladders. Ladder 2 is the gentlest and is what three of the
    /// four lords use.
    #[test]
    fn the_three_ai_ladders_run_from_greedy_to_gentle() {
        let rate = |l: usize, h: i32| tax_rate_for(&T.ai.tax_ladders[l], h);
        // Ladder 0: 0 / 2 / 4 / 10 / 15
        assert_eq!(
            [rate(0, 29), rate(0, 30), rate(0, 50), rate(0, 65), rate(0, 80)],
            [0, 2, 4, 10, 15]
        );
        // Ladder 1: the same thresholds, softer rates.
        assert_eq!(
            [rate(1, 29), rate(1, 30), rate(1, 50), rate(1, 65), rate(1, 80)],
            [0, 1, 3, 7, 12]
        );
        // Ladder 2: different thresholds entirely, and nothing below 60.
        assert_eq!(
            [rate(2, 59), rate(2, 60), rate(2, 70), rate(2, 80), rate(2, 90), rate(2, 95)],
            [0, 1, 2, 3, 8, 10]
        );
        // At every happiness, ladder 0 charges at least what ladder 1 does.
        for h in 0..=100 {
            assert!(rate(0, h) >= rate(1, h), "at {h}");
        }
    }

    /// Every AI lord has a ladder; the human and an eliminated realm do not.
    #[test]
    fn only_the_four_ai_lords_have_a_personality_record() {
        for lord in 1..=crate::tables::AI_PERSONALITY_COUNT as u8 {
            assert!(T.ai_tax_ladder(lord).is_some(), "lord {lord}");
        }
        assert!(T.ai_tax_ladder(crate::realm::LORD_HUMAN).is_none(), "the human");
        assert!(T.ai_tax_ladder(5).is_none(), "lord 5 is not established");
        assert!(T.ai_tax_ladder(crate::realm::LORD_ELIMINATED).is_none());
    }

    /// The unowned counties get their rates in phase 1, and nothing else does.
    #[test]
    fn phase_one_sets_the_neutral_counties_and_leaves_owned_ones_alone() {
        let mut counties = vec![County::new(); 5];
        counties[1].happiness = 95; // unowned
        counties[2].owner = 1;
        counties[2].happiness = 95;
        counties[2].tax_rate = 3;
        set_tax_rates(T, &mut counties, 4, 0, 0);
        assert_eq!(counties[1].tax_rate, 12);
        assert_eq!(counties[2].tax_rate, 3, "realm 1 county untouched");
    }

    #[test]
    fn an_ai_realm_taxes_its_counties_on_its_lords_ladder() {
        let mut counties = vec![County::new(); 4];
        for id in 1..=3 {
            counties[id].owner = 2;
            counties[id].happiness = 70;
        }
        // Lord 4 uses ladder 1: 70 happiness -> 7%.
        set_tax_rates(T, &mut counties, 3, 2, 4);
        assert_eq!(counties[1].tax_rate, 7);
        // Lord 1 uses ladder 2: 70 happiness -> 2%.
        set_tax_rates(T, &mut counties, 3, 2, 1);
        assert_eq!(counties[1].tax_rate, 2);
    }

    /// A lord with no personality record sets no rates at all, rather than
    /// falling back to a ladder nothing established.
    #[test]
    fn an_unestablished_lord_changes_nothing() {
        let mut counties = vec![County::new(); 3];
        counties[1].owner = 2;
        counties[1].happiness = 90;
        counties[1].tax_rate = 6;
        set_tax_rates(T, &mut counties, 2, 2, 5);
        assert_eq!(counties[1].tax_rate, 6);
    }

    // --- the grants --------------------------------------------------------

    /// **The human gets nothing from either mechanism.**
    #[test]
    fn the_human_realm_receives_no_grant_of_any_kind() {
        let mut counties = vec![County::new(); 4];
        let mut realms = vec![Realm::new(); MAX_REALMS];
        realms[1].in_play = true;
        realms[1].is_human = true;
        realms[1].county_count = 3;
        for id in 1..=3 {
            counties[id].owner = 1;
            counties[id].population = 400;
            counties[id].herd = 100;
            counties[id].grain = 500;
        }
        grant_resources(T, &mut counties, &mut realms, 3, 2);
        assert_eq!(realms[1].gold, 0);
        for id in 1..=3 {
            assert_eq!(counties[id].population, 400);
            assert_eq!(counties[id].herd, 100);
            assert_eq!(counties[id].grain, 500);
        }
    }

    fn granted(county_count: u8, difficulty: u8) -> (County, Realm) {
        let mut counties = vec![County::new(); 2];
        let mut realms = vec![Realm::new(); MAX_REALMS];
        realms[2].in_play = true;
        realms[2].lord = 4;
        realms[2].county_count = county_count;
        counties[1].owner = 2;
        counties[1].population = 400;
        counties[1].herd = 100;
        counties[1].grain = 500;
        grant_resources(T, &mut counties, &mut realms, 1, difficulty);
        (counties[1].clone(), realms[2].clone())
    }

    /// **The goods grant is tiered by county count, and `docs/kingdom.md` §8.2
    /// quotes only the top tier.** A realm with five counties gets nothing.
    #[test]
    fn the_goods_grant_shrinks_as_the_realm_grows_and_then_stops() {
        let (small, _) = granted(2, 3);
        assert_eq!((small.population, small.births, small.herd, small.grain), (460, 60, 115, 620));

        let (middling, _) = granted(4, 3);
        assert_eq!(
            (middling.population, middling.births, middling.herd, middling.grain),
            (430, 30, 106, 560),
            "half rates from three counties"
        );

        let (large, _) = granted(5, 3);
        assert_eq!(
            (large.population, large.births, large.herd, large.grain),
            (400, 0, 100, 500),
            "nothing at all from five"
        );
    }

    /// The two gold tables, and which side of three counties picks which.
    #[test]
    fn a_realm_below_three_counties_draws_from_the_smaller_gold_table() {
        assert!(uses_small_gold_table(2));
        assert!(!uses_small_gold_table(3));

        let (_, small) = granted(2, 3);
        assert_eq!(small.gold, AI_GOLD_GRANT_SMALL[4][3]);
        assert_eq!(small.gold, 600);

        let (_, big) = granted(3, 3);
        assert_eq!(big.gold, AI_GOLD_GRANT[4][3]);
        assert_eq!(big.gold, 1800, "three times as much for one more county");
    }

    /// The grant table's rows 1..=3, which this crate used to zero.
    #[test]
    fn every_ai_lord_now_has_a_gold_row() {
        assert_eq!(AI_GOLD_GRANT[0], [0; 4], "the human row is still all zeros");
        for lord in 1..5 {
            assert!(
                AI_GOLD_GRANT[lord].iter().any(|g| *g > 0),
                "lord {lord} should draw something at some difficulty"
            );
            for d in 0..4 {
                assert!(
                    AI_GOLD_GRANT_SMALL[lord][d] <= AI_GOLD_GRANT[lord][d],
                    "the small table is never the larger one"
                );
            }
        }
        assert_eq!(AI_GOLD_GRANT[1], AI_GOLD_GRANT[3], "lords 1 and 3 share a row");
    }

    /// A realm reduced to armies alone gets neither gold nor goods — the
    /// `countyCount != 0` gate around the whole thing.
    #[test]
    fn a_realm_with_no_counties_left_gets_nothing() {
        let (county, realm) = granted(0, 3);
        assert_eq!((county.population, county.herd, county.grain), (400, 100, 500));
        assert_eq!(realm.gold, 0);
    }

    /// **The grant compounds rather than rescues**: it is gated on the county
    /// already having some of each.
    #[test]
    fn a_ruined_ai_county_gets_nothing() {
        let mut counties = vec![County::new(); 2];
        let mut realms = vec![Realm::new(); MAX_REALMS];
        realms[2].in_play = true;
        realms[2].lord = 4;
        realms[2].county_count = 1;
        counties[1].owner = 2;
        counties[1].population = T.ai.grant_min_population;
        counties[1].herd = T.ai.grant_min_herd;
        counties[1].grain = T.ai.grant_min_grain;

        grant_resources(T, &mut counties, &mut realms, 1, 3);
        assert_eq!(counties[1].population, T.ai.grant_min_population);
        assert_eq!(counties[1].herd, T.ai.grant_min_herd);
        assert_eq!(counties[1].grain, T.ai.grant_min_grain);
    }

    #[test]
    fn a_grant_at_difficulty_zero_is_no_goods_grant_at_all() {
        let (county, realm) = granted(4, 0);
        assert_eq!(county.population, 400);
        assert_eq!(realm.gold, 250, "except the gold, 250 for lord 4");
    }

    // The field ladder moved to `crate::ai_farm` with the rest of step 5; it is
    // no longer a bare counter bump but a reclamation order, and the tests moved
    // with it.

    // --- step 6: the castles -----------------------------------------------

    /// **A zero threshold takes a castle type out of a lord's ladder**, and two
    /// of the four lords can never build the largest castle at any treasury.
    #[test]
    fn a_zero_gold_threshold_means_the_type_is_not_offered_rather_than_free() {
        let ladder = &crate::tables::AI_PERSONALITY_CASTLE_GOLD[1]; // [0, 500, 0, 4000, 0]
        assert_eq!(largest_castle_affordable(ladder, 0), None, "nothing at all with no gold");
        assert_eq!(largest_castle_affordable(ladder, 499), None);
        assert_eq!(largest_castle_affordable(ladder, 500), Some(2));
        assert_eq!(largest_castle_affordable(ladder, 3_999), Some(2), "type 3 is not offered");
        assert_eq!(largest_castle_affordable(ladder, 4_000), Some(4));
        assert_eq!(
            largest_castle_affordable(ladder, 1_000_000),
            Some(4),
            "and type 5 never, however rich"
        );

        let reaches_the_top = (0..crate::tables::AI_PERSONALITY_COUNT)
            .filter(|&l| {
                largest_castle_affordable(&crate::tables::AI_PERSONALITY_CASTLE_GOLD[l], 1_000_000)
                    == Some(5)
            })
            .count();
        assert_eq!(reaches_the_top, 2, "two of the four lords can reach a royal castle");
    }

    fn castle_realm(lord: u8, gold: i32) -> (Vec<County>, Vec<Realm>) {
        let mut counties = vec![County::new(); 4];
        let mut realms = vec![Realm::new(); MAX_REALMS];
        realms[2].in_play = true;
        realms[2].lord = lord;
        realms[2].gold = gold;
        realms[2].wood = 100_000;
        realms[2].stone = 100_000;
        for id in 1..=3 {
            counties[id].owner = 2;
            counties[id].population = 5_000;
        }
        (counties, realms)
    }

    /// The lord's population floor and his concurrency limit both gate the
    /// order, and a county that already has a castle is skipped.
    #[test]
    fn a_lord_builds_only_where_the_county_is_big_enough_and_has_no_castle() {
        let (mut counties, mut realms) = castle_realm(1, 100_000);
        counties[2].castle_type = 3; // already has one
        counties[3].population = 699; // below lord 1's floor of 700
        let started = build_castles(T, &mut counties, 3, &mut realms, 2);
        assert_eq!(started, vec![1]);
        // **`castle_type`, not `castle_building`.** The order moves the type at
        // once and leaves `castle_building` holding what stood there before —
        // nothing, on a bare plot. See [`County::castle_building`].
        assert_eq!(counties[1].castle_type, 5, "lord 1 reaches the royal castle");
        assert_eq!(counties[1].castle_building, 0, "and there was no castle before it");
        assert_eq!(counties[1].castle_degraded, 1, "the work is under way");
    }

    /// **The concurrency limit is read once, before the loop.** The Countess
    /// allows one build at a time — and starts three in one pass if she began
    /// it with none. Reproduced, not tidied.
    #[test]
    fn the_concurrency_limit_is_tested_before_the_pass_and_not_during_it() {
        let (mut counties, mut realms) = castle_realm(4, 100_000);
        assert_eq!(T.ai_personality(4).unwrap().castle_concurrent, 1);
        let started = build_castles(T, &mut counties, 3, &mut realms, 2);
        assert_eq!(started, vec![1, 2, 3], "one at a time, three at once");

        // But next pass, with those three running, she starts nothing.
        let started = build_castles(T, &mut counties, 3, &mut realms, 2);
        assert!(started.is_empty());
    }

    #[test]
    fn a_lord_with_no_personality_record_builds_nothing() {
        let (mut counties, mut realms) = castle_realm(5, 100_000);
        assert!(build_castles(T, &mut counties, 3, &mut realms, 2).is_empty());
    }

    // --- step 12: the weapon rota ------------------------------------------

    /// **The rota is a realm counter walked inside a loop over counties**, so
    /// one realm's counties make different weapons in the same step and the
    /// pattern carries across turns.
    #[test]
    fn one_realms_counties_each_take_the_next_weapon_on_the_lords_rota() {
        let mut counties = vec![County::new(); 5];
        let mut realm = Realm::new();
        realm.lord = 1; // rota [0, 1, 4, 2, 5, 2]
        realm.wood = 100_000;
        realm.iron = 100_000;
        for id in 1..=4 {
            counties[id].owner = 2;
        }
        choose_industry(T, &mut counties, 4, &mut realm, 2);
        assert_eq!(
            (1..=4).map(|i| counties[i].weapon_type).collect::<Vec<_>>(),
            vec![0, 1, 4, 2],
            "cursor 0..3 selects rota slots 0..3"
        );
        assert_eq!(realm.weapon_rota, 4);

        // The next turn picks up where it left off, and slots 0..3 come round
        // a second time before slots 4 and 5 are ever seen.
        choose_industry(T, &mut counties, 4, &mut realm, 2);
        assert_eq!(
            (1..=4).map(|i| counties[i].weapon_type).collect::<Vec<_>>(),
            vec![0, 1, 4, 2],
            "cursor 4..7 selects rota slots 0..3 again"
        );
        choose_industry(T, &mut counties, 4, &mut realm, 2);
        assert_eq!(
            (1..=4).map(|i| counties[i].weapon_type).collect::<Vec<_>>(),
            vec![5, 2, 0, 1],
            "cursor 8, 9 reach the last two slots, then it wraps"
        );
        assert_eq!(realm.weapon_rota, 2);
    }

    /// The blacksmith's *has resource* is **affordability**, not geology.
    #[test]
    fn the_blacksmith_has_a_resource_only_when_the_realm_can_pay_for_the_weapon() {
        let mut counties = vec![County::new(); 2];
        let mut realm = Realm::new();
        realm.lord = 1;
        counties[1].owner = 2;
        let weapons = crate::tables::Commodity::Weapons as usize;

        realm.wood = 0;
        realm.iron = 0;
        choose_industry(T, &mut counties, 1, &mut realm, 2);
        assert!(!counties[1].industry[weapons].has_resource, "no stock, no smithing");

        realm.wood = 100_000;
        realm.iron = 100_000;
        realm.weapon_rota = 0;
        choose_industry(T, &mut counties, 1, &mut realm, 2);
        assert!(counties[1].industry[weapons].has_resource);
    }

    /// **A castle going up switches iron and the blacksmith off**, and leaves
    /// forestry and quarrying running.
    #[test]
    fn a_castle_under_construction_switches_the_mine_and_the_smithy_off() {
        use crate::tables::Commodity;
        let mut counties = vec![County::new(); 2];
        let mut realm = Realm::new();
        realm.lord = 1;
        realm.wood = 100_000;
        realm.iron = 100_000;
        counties[1].owner = 2;
        for slot in 0..4 {
            counties[1].industry[slot].has_resource = true;
        }

        choose_industry(T, &mut counties, 1, &mut realm, 2);
        assert!(counties[1].industry.iter().all(|i| i.enabled), "no castle: all four run");
        assert!(counties[1].castle_switch, "and the castle job slot is switched on regardless");

        counties[1].castle_degraded = 1;
        counties[1].castle_wood_owed = 100;
        counties[1].castle_stone_owed = 100;
        for slot in 0..4 {
            counties[1].industry[slot].has_resource = true;
        }
        realm.weapon_rota = 0;
        choose_industry(T, &mut counties, 1, &mut realm, 2);
        assert!(counties[1].industry[Commodity::Wood as usize].enabled, "the build wants wood");
        assert!(counties[1].industry[Commodity::Stone as usize].enabled, "and stone");
        assert!(!counties[1].industry[Commodity::Iron as usize].enabled, "iron is off");
        assert!(!counties[1].industry[Commodity::Weapons as usize].enabled, "and so is the smithy");

        // **And once the stone has all been delivered the quarries go off too.**
        // That is the half the old constant could not express, and it is the
        // literal `slot != 3 || county.stoneOwed > 0`.
        counties[1].castle_stone_owed = 0;
        for slot in 0..4 {
            counties[1].industry[slot].has_resource = true;
        }
        realm.weapon_rota = 0;
        choose_industry(T, &mut counties, 1, &mut realm, 2);
        assert!(counties[1].industry[Commodity::Wood as usize].enabled, "still owes wood");
        assert!(!counties[1].industry[Commodity::Stone as usize].enabled, "the stone is all there");
    }

    /// A seasonally disabled industry stays off whatever else is true.
    #[test]
    fn a_seasonally_disabled_industry_is_not_switched_back_on() {
        let mut counties = vec![County::new(); 2];
        let mut realm = Realm::new();
        realm.lord = 1;
        counties[1].owner = 2;
        counties[1].industry[0].has_resource = true;
        counties[1].industry[0].disabled_seasons = 2;
        choose_industry(T, &mut counties, 1, &mut realm, 2);
        assert!(!counties[1].industry[0].enabled);
    }

    // --- step 13: the taunt -------------------------------------------------

    fn taunting_realm(share: i32) -> (Realm, Vec<Realm>) {
        let mut realms = vec![Realm::new(); MAX_REALMS];
        for i in 1..=3 {
            realms[i].in_play = true;
            realms[i].rank = i as u8;
        }
        realms[3].is_human = true;
        let mut me = realms[1].clone();
        me.rank = 1;
        me.lord = 2;
        me.share_of_map_pct = share;
        (me, realms)
    }

    /// **Only the leader taunts, and only on the ninth qualifying turn.**
    #[test]
    fn the_leader_taunts_every_human_after_eight_turns_above_two_fifths_of_the_map() {
        let (mut me, realms) = taunting_realm(40);
        for turn in 1..=7 {
            assert!(taunt(&mut me, 1, &realms, 3).is_empty(), "turn {turn}");
            assert_eq!(me.taunt_timer, turn as u8);
        }
        let sent = taunt(&mut me, 1, &realms, 3);
        assert_eq!(sent.len(), 1, "one human realm, one letter");
        assert_eq!(sent[0].group, TAUNT_HOW_ARE_YOU_DOING);
        assert_eq!(sent[0].to, 3);
        assert_eq!(me.taunt_stage, 1, "and it moves to the second stage");
        assert_eq!(me.taunt_timer, 0);
    }

    /// The share threshold is `> 39`, and falling below it **pauses** the timer
    /// rather than resetting it.
    #[test]
    fn slipping_below_the_share_threshold_pauses_the_count_rather_than_restarting_it() {
        let (mut me, realms) = taunting_realm(40);
        for _ in 0..5 {
            taunt(&mut me, 1, &realms, 3);
        }
        assert_eq!(me.taunt_timer, 5);
        me.share_of_map_pct = 39;
        taunt(&mut me, 1, &realms, 3);
        assert_eq!(me.taunt_timer, 5, "39 is not above 39");
        me.share_of_map_pct = 40;
        taunt(&mut me, 1, &realms, 3);
        assert_eq!(me.taunt_timer, 6, "and it carries on from where it was");
    }

    /// A realm that is not first says nothing at all.
    #[test]
    fn only_the_realm_in_first_place_taunts() {
        let (mut me, realms) = taunting_realm(90);
        me.rank = 2;
        for _ in 0..20 {
            assert!(taunt(&mut me, 1, &realms, 3).is_empty());
        }
        assert_eq!(me.taunt_timer, 0, "the timer does not even run");
    }

    /// Stage 1 writes only to the **last-placed** realm, and only if that realm
    /// is human and not this one's ally.
    #[test]
    fn the_second_taunt_goes_to_the_trailer_and_never_to_an_ally() {
        let (mut me, realms) = taunting_realm(30);
        me.taunt_stage = 1;
        for _ in 0..8 {
            taunt(&mut me, 1, &realms, 3);
        }
        assert_eq!(me.taunt_stage, 0, "it went, and the cycle restarts");

        // An allied trailer is spared.
        let (mut me, realms) = taunting_realm(30);
        me.taunt_stage = 1;
        me.ally = 3;
        for _ in 0..20 {
            assert!(taunt(&mut me, 1, &realms, 3).is_empty());
        }

        // So is an AI trailer.
        let (mut me, mut realms) = taunting_realm(30);
        realms[3].is_human = false;
        me.taunt_stage = 1;
        for _ in 0..20 {
            assert!(taunt(&mut me, 1, &realms, 3).is_empty());
        }
    }

    /// Every letter advances the voice rotation, which is half of the variant
    /// index the message screen plays.
    #[test]
    fn every_taunt_advances_the_lords_voice_rotation() {
        let mut realms = vec![Realm::new(); MAX_REALMS];
        for i in 1..=4 {
            realms[i].in_play = true;
            realms[i].is_human = i >= 2;
        }
        let mut me = Realm::new();
        me.in_play = true;
        me.rank = 1;
        me.lord = 3;
        me.share_of_map_pct = 90;
        me.taunt_timer = 7;
        let sent = taunt(&mut me, 1, &realms, 4);
        assert_eq!(sent.len(), 3, "three human realms, three letters");
        assert_eq!(
            sent.iter().map(|t| t.variant).collect::<Vec<_>>(),
            vec![8, 9, 10],
            "lord 3: 3*4 + rot - 4"
        );
        assert_eq!(me.voice_rotation, 3);
    }

    /// The trailer is the worst-ranked realm still in play.
    #[test]
    fn the_trailer_is_the_worst_ranked_realm_in_play() {
        let mut realms = vec![Realm::new(); MAX_REALMS];
        for i in 1..=4 {
            realms[i].in_play = true;
            realms[i].rank = i as u8;
        }
        assert_eq!(rank_trailer(&realms), 4);
        realms[4].in_play = false;
        assert_eq!(rank_trailer(&realms), 3);
        assert_eq!(rank_trailer(&vec![Realm::new(); MAX_REALMS]), 0, "nobody in play");
    }

    // --- step 14: the realm totals -----------------------------------------

    /// **Five of the six score inputs `docs/kingdom.md` §8.3 could not
    /// identify** are what this step writes.
    #[test]
    fn the_realm_totals_are_five_of_the_six_score_inputs() {
        let mut counties = vec![County::new(); 9];
        for id in 1..=4 {
            counties[id].owner = 2;
            counties[id].population = 100 * id as i32;
            counties[id].happiness = 40 + 10 * id as i32;
            counties[id].health_meter = 50 + id as i32;
        }
        let mut realm = Realm::new();
        update_realm_totals(&mut realm, &counties, 8, 2, 3, 600);

        assert_eq!(realm.county_count, 4);
        assert_eq!(realm.population_total, 1000, "100+200+300+400");
        assert_eq!(realm.population_mean, 250);
        assert_eq!(realm.share_of_map_pct, 50, "four counties of eight");
        assert_eq!(realm.mean_happiness, 65, "(50+60+70+80)/4");
        assert_eq!(realm.mean_health, 52, "(51+52+53+54)/4 truncated");
        assert_eq!(realm.army_count, 3);
        assert_eq!(realm.total_men, 600);

        assert_eq!(realm.score_inputs[..5], [50, 1000, 65, 52, 600]);
        assert_eq!(realm.score_inputs[5], 0, "the sixth is still unidentified");

        // And those feed the score with the documented weights.
        assert_eq!(realm.compute_score(T), 50 * 10 + 1000 / 10 + 65 * 2 + 52 * 2 + 600 / 5);
    }

    /// Every division is guarded, so a realm losing its last county does not
    /// divide by zero.
    #[test]
    fn a_realm_with_no_counties_totals_to_zero_rather_than_dividing_by_it() {
        let counties = vec![County::new(); 5];
        let mut realm = Realm::new();
        update_realm_totals(&mut realm, &counties, 4, 3, 0, 0);
        assert_eq!(realm.county_count, 0);
        assert_eq!(
            (
                realm.population_mean,
                realm.share_of_map_pct,
                realm.mean_happiness,
                realm.mean_health
            ),
            (0, 0, 0, 0)
        );
    }

    /// The county count this writes is what the grant tier reads next turn.
    #[test]
    fn the_county_count_the_grant_tiers_turn_on_comes_from_step_fourteen() {
        let mut counties = vec![County::new(); 9];
        for id in 1..=2 {
            counties[id].owner = 1;
        }
        let mut realm = Realm::new();
        update_realm_totals(&mut realm, &counties, 8, 1, 0, 0);
        assert_eq!(realm.county_count, 2);
        assert!(uses_small_gold_table(realm.county_count));
    }

    // --- scoring -----------------------------------------------------------

    #[test]
    fn ranking_orders_realms_by_score_and_breaks_ties_by_index() {
        let mut realms = vec![Realm::new(); MAX_REALMS];
        for i in 1..=4 {
            realms[i].in_play = true;
        }
        realms[1].score_inputs[0] = 1; // 10
        realms[2].score_inputs[0] = 5; // 50
        realms[3].score_inputs[0] = 5; // 50, ties with 2
        realms[4].score_inputs[0] = 0; // 0

        rank_realms(T, &mut realms);
        assert_eq!(realms[2].rank, 1, "equal scores: the lower index wins");
        assert_eq!(realms[3].rank, 2);
        assert_eq!(realms[1].rank, 3);
        assert_eq!(realms[4].rank, 4);
        assert_eq!(realms[5].rank, 0, "a realm not in play is unranked");
    }

    #[test]
    fn ranking_does_not_depend_on_the_order_the_realms_were_set_up_in() {
        let build = |scores: [i32; 4]| {
            let mut realms = vec![Realm::new(); MAX_REALMS];
            for (i, s) in scores.iter().enumerate() {
                realms[i + 1].in_play = true;
                realms[i + 1].score_inputs[0] = *s;
            }
            rank_realms(T, &mut realms);
            (1..=4).map(|i| realms[i].rank).collect::<Vec<u8>>()
        };
        assert_eq!(build([4, 3, 2, 1]), vec![1, 2, 3, 4]);
        assert_eq!(build([1, 2, 3, 4]), vec![4, 3, 2, 1]);
        assert_eq!(build([2, 2, 2, 2]), vec![1, 2, 3, 4]);
    }

    #[test]
    fn every_in_play_realm_gets_a_distinct_rank() {
        let mut realms = vec![Realm::new(); MAX_REALMS];
        for i in 1..=5 {
            realms[i].in_play = true;
            realms[i].gold = (i as i32) * 3000;
        }
        rank_realms(T, &mut realms);
        let mut ranks: Vec<u8> = (1..=5).map(|i| realms[i].rank).collect();
        ranks.sort_unstable();
        assert_eq!(ranks, vec![1, 2, 3, 4, 5]);
    }
}
