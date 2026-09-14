#![allow(unused_imports)]
use super::*;
use super::steps::*;
use super::taxes::*;
use super::construction::*;
use super::industry::*;
use super::diplomacy::*;
use super::*;
use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

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

