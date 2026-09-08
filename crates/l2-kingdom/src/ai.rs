//! The AI realm's turn, its advantages, and scoring — `docs/kingdom.md` §3.2
//! and §8.
//!
//! # What is here and what is not
//!
//! `AI_RunTurnStep` (`0x0049A581`) picks the next in-play realm round-robin and
//! dispatches on that realm's `+0x00` into **fourteen handlers** — tax, fields,
//! food, industry, castles, armies, diplomacy — incrementing it each time. When
//! it passes `15 + 2 * realmIndex` the realm is marked 999 and phase 4 ends for
//! it.
//!
//! `docs/kingdom.md` §12 calls the fourteen handlers *"the single largest
//! remaining piece of the kingdom layer"* and records that **none was
//! decompiled**. So the program counter, the sentinel and the round-robin are
//! implemented here and the handlers are not: [`run_step`] advances a realm
//! through its turn and does nothing at each stop. That is the honest shape —
//! an AI that does nothing is visibly missing, where an AI invented from
//! nothing would be indistinguishable from one that was reverse engineered.
//!
//! `AI_SetTaxRates` (`0x0049D638`) does two things, and only one of them is
//! documented well enough to write down. Its four tax-rate ladders — one for
//! unowned counties and three chosen by the AI lord's personality — are not
//! given at all, so there is no tax-rate function here; its **resource
//! grants** are given, and are [`grant_resources`]. Phase 1
//! (`docs/kingdom.md` §3.1) is where the unowned ladder would run.

use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    AI_GRANT_GRAIN_PER_DIFFICULTY, AI_GRANT_HERD_PER_DIFFICULTY, AI_GRANT_MIN_GRAIN,
    AI_GRANT_MIN_HERD, AI_GRANT_MIN_POPULATION, AI_GRANT_POPULATION_PER_DIFFICULTY,
};

/// The number of handlers `AI_RunTurnStep` dispatches into: `aiStep` 0..=14.
pub const AI_HANDLER_COUNT: i32 = 14;

/// A realm is finished once its step counter passes `15 + 2 * realmIndex`.
///
/// The `2 * realmIndex` term is odd and is reproduced rather than tidied: it
/// means the later realms in the array get two extra steps each, which is
/// either a staggering trick or a bug, and `docs/kingdom.md` §3.2 marks the
/// structure **`[V]`** without explaining it.
pub fn done_threshold(realm_index: usize) -> i32 {
    15 + 2 * realm_index as i32
}

/// One step of one realm's turn. Returns `true` once the realm is finished.
///
/// The fourteen handlers are deliberately absent — see the module
/// documentation.
pub fn run_step(realm: &mut Realm, realm_index: usize) -> bool {
    if !realm.in_play || realm.is_human || realm.ai_step == AI_STEP_DONE {
        return true;
    }
    realm.ai_step += 1;
    if realm.ai_step > done_threshold(realm_index) {
        realm.ai_step = AI_STEP_DONE;
        return true;
    }
    false
}

/// `Turn_AllRealmsDone` — what phase 4 waits on.
pub fn all_realms_done(realms: &[Realm]) -> bool {
    realms.iter().all(|r| r.turn_done())
}

/// Reset every realm's program counter at the start of phase 4.
pub fn begin_turn(realms: &mut [Realm]) {
    for realm in realms.iter_mut() {
        realm.ai_step = if realm.in_play && !realm.is_human { 0 } else { AI_STEP_DONE };
    }
}

/// The AI's free resources, per county, per season — `docs/kingdom.md` §8.2.
///
/// `difficulty * 20` people (and the same again booked as births),
/// `difficulty * 5` head and `difficulty * 40` sacks. Each is gated on the
/// county already having some, so the grant **compounds rather than rescues**.
///
/// The gold grant comes from [`Realm::gold_grant`], whose table is only
/// partially documented.
pub fn grant_resources(
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
        if realms[owner].is_human || !realms[owner].in_play {
            continue;
        }
        let c = &mut counties[id];
        if c.population > AI_GRANT_MIN_POPULATION {
            let people = d * AI_GRANT_POPULATION_PER_DIFFICULTY;
            c.population += people;
            c.births += people;
        }
        if c.herd > AI_GRANT_MIN_HERD {
            c.herd += d * AI_GRANT_HERD_PER_DIFFICULTY;
        }
        if c.grain > AI_GRANT_MIN_GRAIN {
            c.grain += d * AI_GRANT_GRAIN_PER_DIFFICULTY;
        }
    }
    for realm in realms.iter_mut().skip(1) {
        if realm.in_play && !realm.is_human {
            realm.gold += realm.gold_grant(difficulty);
        }
    }
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
pub fn rank_realms(realms: &mut [Realm]) {
    for realm in realms.iter_mut() {
        realm.score = realm.compute_score();
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
    use crate::realm::MAX_REALMS;

    fn ai_realms() -> Vec<Realm> {
        let mut realms = vec![Realm::new(); MAX_REALMS];
        for i in 1..=3 {
            realms[i].in_play = true;
            realms[i].lord = i as u8;
        }
        realms
    }

    #[test]
    fn an_ai_realm_walks_its_program_counter_to_the_sentinel() {
        let mut realms = ai_realms();
        begin_turn(&mut realms);
        assert!(!all_realms_done(&realms));

        let mut steps = 0;
        while !run_step(&mut realms[1], 1) {
            steps += 1;
            assert!(steps < 100, "the turn should terminate");
        }
        assert_eq!(realms[1].ai_step, AI_STEP_DONE);
        // 15 + 2*1 = 17, so the counter runs 1..=17 and then trips on 18.
        assert_eq!(steps, done_threshold(1) as usize);
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
        assert!(run_step(&mut realms[2], 2), "and stays done");
    }

    #[test]
    fn phase_four_ends_only_when_every_realm_is_done() {
        let mut realms = ai_realms();
        begin_turn(&mut realms);
        for index in 1..=3 {
            while !run_step(&mut realms[index], index) {}
            let expected = index == 3;
            assert_eq!(all_realms_done(&realms), expected, "after realm {index}");
        }
    }

    /// **The human gets nothing from either mechanism.**
    #[test]
    fn the_human_realm_receives_no_grant_of_any_kind() {
        let mut counties = vec![County::new(); 4];
        let mut realms = vec![Realm::new(); MAX_REALMS];
        realms[1].in_play = true;
        realms[1].is_human = true;
        for id in 1..=3 {
            counties[id].owner = 1;
            counties[id].population = 400;
            counties[id].herd = 100;
            counties[id].grain = 500;
        }
        grant_resources(&mut counties, &mut realms, 3, 2);
        assert_eq!(realms[1].gold, 0);
        for id in 1..=3 {
            assert_eq!(counties[id].population, 400);
            assert_eq!(counties[id].herd, 100);
            assert_eq!(counties[id].grain, 500);
        }
    }

    #[test]
    fn an_ai_county_is_topped_up_in_proportion_to_the_difficulty() {
        let mut counties = vec![County::new(); 2];
        let mut realms = vec![Realm::new(); MAX_REALMS];
        realms[2].in_play = true;
        realms[2].lord = 4;
        counties[1].owner = 2;
        counties[1].population = 400;
        counties[1].herd = 100;
        counties[1].grain = 500;

        grant_resources(&mut counties, &mut realms, 1, 3);
        assert_eq!(counties[1].population, 460, "3 x 20");
        assert_eq!(counties[1].births, 60, "booked as births as well");
        assert_eq!(counties[1].herd, 115, "3 x 5");
        assert_eq!(counties[1].grain, 620, "3 x 40");
        assert_eq!(realms[2].gold, 1800, "the top row of g_aiGoldGrant");
    }

    /// **The grant compounds rather than rescues**: it is gated on the county
    /// already having some of each.
    #[test]
    fn a_ruined_ai_county_gets_nothing() {
        let mut counties = vec![County::new(); 2];
        let mut realms = vec![Realm::new(); MAX_REALMS];
        realms[2].in_play = true;
        realms[2].lord = 4;
        counties[1].owner = 2;
        counties[1].population = AI_GRANT_MIN_POPULATION;
        counties[1].herd = AI_GRANT_MIN_HERD;
        counties[1].grain = AI_GRANT_MIN_GRAIN;

        grant_resources(&mut counties, &mut realms, 1, 3);
        assert_eq!(counties[1].population, AI_GRANT_MIN_POPULATION);
        assert_eq!(counties[1].herd, AI_GRANT_MIN_HERD);
        assert_eq!(counties[1].grain, AI_GRANT_MIN_GRAIN);
    }

    #[test]
    fn a_grant_at_difficulty_zero_is_no_grant_at_all() {
        let mut counties = vec![County::new(); 2];
        let mut realms = vec![Realm::new(); MAX_REALMS];
        realms[2].in_play = true;
        realms[2].lord = 4;
        counties[1].owner = 2;
        counties[1].population = 400;
        counties[1].herd = 100;
        counties[1].grain = 500;
        grant_resources(&mut counties, &mut realms, 1, 0);
        assert_eq!(counties[1].population, 400);
        assert_eq!(realms[2].gold, 250, "except the gold, which starts at 250");
    }

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

        rank_realms(&mut realms);
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
            rank_realms(&mut realms);
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
        rank_realms(&mut realms);
        let mut ranks: Vec<u8> = (1..=5).map(|i| realms[i].rank).collect();
        ranks.sort_unstable();
        assert_eq!(ranks, vec![1, 2, 3, 4, 5]);
    }
}
