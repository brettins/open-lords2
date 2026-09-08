//! Ending a turn: the phase machine goes round, the AI finishes, and the same
//! kingdom ended twice produces the same numbers.
//!
//! No install and no window. The kingdoms here are built by hand so that each
//! assertion is about *one* thing the spine does — the tests that run the
//! England turn-one scenario are in `tests/scenario.rs`.

use l2_kingdom::phase::Phase;
use l2_kingdom::realm::AI_STEP_DONE;
use l2_kingdom::tables::Tables;
use l2_game::turn;
use l2_game::Game;

/// One human realm holding county 1, four AI realms holding one each, and nine
/// unowned — the shape of the England turn-one scenario, without needing the file.
fn five_realms() -> Game {
    let mut g = Game::new(0x51EED);
    g.player = 1;
    g.kingdom.set_county_count(14);
    for id in 1..=14 {
        let c = &mut g.kingdom.counties[id];
        c.population = 417;
        c.pop_last = 417;
        c.happiness = 65;
        c.happiness_last = 65;
        c.health_meter = 65;
        c.health_band = l2_kingdom::tables::health_band(65);
        c.herd = 67;
        c.ration_wanted = 3;
        c.ration_split = 100;
        for n in 1..=14u8 {
            if n as usize != id {
                c.add_neighbour(n);
            }
        }
    }
    for realm in 1..=5usize {
        g.kingdom.counties[realm].owner = realm as u8;
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 3;
        g.kingdom.realms[realm].county_count = 1;
        g.kingdom.realms[realm].gold = 1000;
        g.kingdom.realms[realm].lord = realm as u8 - 1;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 0;
    g
}

#[test]
fn a_turn_walks_the_whole_phase_machine_and_runs_the_season_once() {
    let mut g = five_realms();
    let before = g.kingdom.turn_count;
    let outcome = turn::end_turn(&mut g).expect("the machine comes round");

    assert_eq!(
        outcome.report.passes,
        l2_kingdom::SEASON_PIPELINE.to_vec(),
        "every pass, in the order the pipeline is written in"
    );
    assert_eq!(g.kingdom.turn_count, before + 1, "exactly one season ran");
    assert_eq!(g.kingdom.turn.phase, Phase::NeutralCounties, "and phase 7 wrapped to 1");
    assert!(outcome.ticks < turn::MAX_TICKS, "{} ticks", outcome.ticks);
    assert_eq!(g.turns_played, 1);
    assert!(g.last_report.is_some());
}

/// Phase 4 does not end until every realm's `aiStep` reaches its threshold, and
/// nothing in `l2-kingdom` was calling `ai::run_step` at all. If the driver in
/// `turn.rs` stopped working, this test would not fail with a wrong number: the
/// turn would never finish, and `end_turn` would return `None`.
#[test]
fn every_ai_realm_finishes_its_turn_and_the_human_realm_never_starts_one() {
    let mut g = five_realms();
    turn::end_turn(&mut g).expect("the AI must finish or the turn cannot end");
    for realm in 1..=5usize {
        assert!(
            g.kingdom.realms[realm].ai_step >= AI_STEP_DONE,
            "realm {realm} did not finish its turn"
        );
    }
}

/// `docs/kingdom.md` §3.1: phase 1 sets the *unowned* counties' tax rates on
/// the neutral ladder, which `AI_SetTaxRates(0)` reads out of a chain of
/// comparisons — `< 20 -> 0`, `< 40 -> 1`, `< 50 -> 2`, `< 60 -> 3`,
/// `< 70 -> 4`, `< 80 -> 6`, `< 90 -> 8`, else 12 (`docs/decisions.md` C19).
/// A county at 65 lands on 4.
#[test]
fn the_unowned_counties_are_taxed_on_the_neutral_ladder_every_turn() {
    let mut g = five_realms();
    assert_eq!(g.kingdom.counties[14].owner, 0, "county 14 is unowned");
    assert_eq!(g.kingdom.counties[14].tax_rate, 0, "and starts untaxed");
    assert_eq!(g.kingdom.counties[14].happiness, 65);

    turn::end_turn(&mut g).unwrap();
    assert_eq!(g.kingdom.counties[14].tax_rate, 4, "65 is on the fifth rung");
    assert_eq!(g.kingdom.counties[1].tax_rate, 0, "the player's county is not touched");
}

/// The one place the spine deliberately differs from the original's call
/// structure, and the reason it has to: `AI_SetTaxRates(realm)` grants that one
/// realm its gold, while `l2_kingdom::ai::grant_resources` walks every AI realm
/// in a single pass. Running it inside each realm's step 3 would pay every
/// realm four times over. The check is arithmetic: exactly one grant.
#[test]
fn the_ai_gold_grant_is_paid_once_a_turn_and_not_once_per_ai_realm() {
    let mut g = five_realms();
    g.kingdom.options.difficulty = 2;
    let tables = Tables::DEFAULT;

    let expected: Vec<i32> = (0..6)
        .map(|r| g.kingdom.realms[r].gold_grant(&tables, 2))
        .collect();
    let before: Vec<i32> = (0..6).map(|r| g.kingdom.realms[r].gold).collect();

    turn::end_turn(&mut g).unwrap();

    for realm in 2..=5usize {
        let paid = g.kingdom.realms[realm].gold - before[realm] - g.kingdom.counties[realm].tax_collected;
        assert_eq!(
            paid, expected[realm],
            "realm {realm} was paid {paid} where one grant is {}",
            expected[realm]
        );
    }
    assert_eq!(
        g.kingdom.realms[1].gold - before[1],
        g.kingdom.counties[1].tax_collected,
        "the human realm is granted nothing at all"
    );
}

/// Determinism, which is not negotiable: a turn is a pure function of the
/// kingdom it started from. Two identical kingdoms, ended independently, must
/// be bit-identical afterwards — no clock, no hash order, no address.
#[test]
fn the_same_kingdom_ended_twice_lands_on_the_same_numbers() {
    let mut a = five_realms();
    let mut b = five_realms();
    assert_eq!(a.kingdom, b.kingdom, "the two start equal");

    let ra = turn::end_turn(&mut a).unwrap();
    let rb = turn::end_turn(&mut b).unwrap();
    assert_eq!(ra.ticks, rb.ticks);
    assert_eq!(ra.report, rb.report);
    assert_eq!(a.kingdom, b.kingdom);

    // And again, because a difference that only appears on the second turn is
    // the kind a single-turn test misses.
    turn::end_turn(&mut a).unwrap();
    turn::end_turn(&mut b).unwrap();
    assert_eq!(a.kingdom, b.kingdom);
}

/// The seasons turn, and the year rolls where the code says rather than where
/// the prose does.
///
/// `Season_Advance` rolls the year when the season that *ended* was 4, and
/// `ended` is the old `g_seasonNext` — so starting from the England turn-one fixture's
/// Winter 1268 the year does not move until the turn that brings Winter round
/// again. The year label therefore runs **Winter, Spring, Summer, Autumn**,
/// which `l2-kingdom`'s errata note 3 records and §3.3's prose contradicts.
/// This test was written expecting the prose and corrected by the code.
#[test]
fn four_turns_walk_the_year_round() {
    let mut g = five_realms();
    g.kingdom.season = 4;
    g.kingdom.season_next = 1;
    g.kingdom.year = 1268;
    g.kingdom.year_next = 1269;

    let seasons: Vec<(u8, i32)> = (0..4)
        .map(|_| {
            turn::end_turn(&mut g).unwrap();
            (g.kingdom.season, g.kingdom.year)
        })
        .collect();
    assert_eq!(seasons, vec![(1, 1268), (2, 1268), (3, 1268), (4, 1269)]);
}

/// The numbers actually move, which is the point of the button. Population,
/// health and the treasury all change on a turn that nobody gave any orders.
#[test]
fn a_turn_moves_the_population_and_the_treasury() {
    let mut g = five_realms();
    g.kingdom.counties[1].tax_rate = 10;
    g.kingdom.counties[1].castle_type = 3;
    let pop = g.kingdom.counties[1].population;
    let gold = g.kingdom.realms[1].gold;

    turn::end_turn(&mut g).unwrap();

    assert_ne!(g.kingdom.counties[1].population, pop, "a season with births and deaths");
    assert_eq!(g.kingdom.counties[1].pop_last, pop, "and last season is remembered");
    assert!(
        g.kingdom.realms[1].gold > gold,
        "a taxed county pays: {} -> {}",
        gold,
        g.kingdom.realms[1].gold
    );
    assert_eq!(g.gold_last[1], gold, "the interface can show which way it went");
}
