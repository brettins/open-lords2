#![allow(unused_imports)]
use super::*;

use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

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

    /// The address of the handler in `Lords2.exe`
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

/// True where this crate runs the step — **all fourteen**, and
    /// the one that does nothing does nothing because the original's does.
    ///
    /// > This used to exempt the diplomacy pair
    /// > seven reply handlers"*. [`crate::diplomacy`] is that inbox and those
    /// > seven handlers.
    pub fn is_implemented(self) -> bool {
        true
    }
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
/// A *neutral* county is taxed harder at low happiness than any AI
/// taxes its own — 1% at 20 happiness where every lord's ladder charges
/// nothing below 30. Nobody is collecting it, though: `Tax_CollectAll` banks an
/// unowned county's take into the county itself.
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
// A lord with no personality record sets no rates at all.
    // falling back to a ladder that. See
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
/// Every division is guarded on `countyCount != 0` and writes 0 instead
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
/// 2. **The gold grant has two tables**
///    ([`crate::tables::AI_GOLD_GRANT_SMALL`]) is the one a realm below three
///    counties draws from. §8.2 mentions the second table; the crate did not
///    have it.
/// 3. **The whole grant is gated on the realm holding at least one county.**
/// `if (realm.countyCount != 0)` wraps both halves
///    armies alone gets neither gold nor goods.
///
/// Taken together the grants **reward a realm that is already ahead** and
/// abandon one that is losing — the opposite of the rubber-banding the phrase
/// "the AI's advantages" suggests.
///
/// Each county's share is still gated on the county already having some, so it
/// compounds.
///
/// The realm's `county_count` must be current: [`update_realm_totals`] is what
/// sets it.
///
/// > This used to end *"and it is step 14 of the **previous** turn"*. It is not.
/// > `AI_RunTurnStep`'s step-0 prologue calls `Realm_UpdateTotals`
/// > (`0x0049D1E0`) for **every** realm at the top of phase 4, before any
/// > handler runs, so the count step 3 reads is this turn's. See
/// > `l2_game::turn::step_zero`.
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

/// Step 12 — `FUN_0049E77D`, which this crate names `AI_ChooseIndustry`.
///
/// Three loops over the realm's counties
/// each county takes the weapon type at the realm's cursor and the cursor
/// advances. Because the cursor is a *realm* field advanced inside a loop over
/// counties, a realm of four counties makes four different weapons at once and
/// the pattern rotates from wherever it stopped last turn — so
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
/// **while a castle is going up
/// outright**, and keeps forestry and quarrying on only while the build still
/// wants wood or stone. An AI at war stops making weapons the moment it starts
/// a castle, which is a real strategic quirk and not an obvious one.
///
/// `docs/kingdom.md` calls county `+0x1B0` untraced; this loop sets it to **1
/// on every county the realm holds, unconditionally**, and
/// `AI_ManageFields(0)` sets it to 0 on the unowned ones. That is
/// [`County::castle_switch`]
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
/// this is the original, and it is the third
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
/// The two thresholds are `>` on 0x27 and 0x1B
/// timer`, so it is the **ninth** consecutive qualifying turn that sends. The
/// timer only advances on a turn the share threshold is met
/// slips below 39% pauses.
///
/// `trailer` is the last-placed realm — `g_rankTrailer`, which the original
/// keeps as a global and which is derived by the caller from
/// [`rank_realms`]. `out` collects the letters; the caller decides what to do
/// with them
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

