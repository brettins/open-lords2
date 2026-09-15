#![allow(unused_imports)]
use super::*;

use super::*;
use super::military::*;
use l2_kingdom::phase::Phase;
use l2_kingdom::realm::AI_STEP_DONE;
use l2_kingdom::tables::Tables;
use l2_game::turn;
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::units_tick::Contact;
use l2_kingdom::merchant::MerchantRoutes;
use l2_kingdom::unit::{Unit, UnitKind};

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

/// `Turn_BeginPlayersTurn` (`0x0049B6D3`) zeroes every realm's counter and
/// `AI_RunTurnStep`'s (`0x0049A581`) `isHuman` test guards the fourteen handlers
/// and the increment, not the prologue above them:
///
/// `Realm_UpdateTotals` (`0x0049D1E0`) is the **only** thing in the binary that
/// fills the score inputs, and its other caller here is AI step 14, which a human
/// realm never reaches. Before `l2_game::turn::step_zero` ran the whole prologue,
/// a human's six inputs stayed at zero for the life of the game and
/// `Score_RankRealms` scored the person on `score_gold_bracket` alone — a flat
/// **50** against the original's 576, 590, 1333 and 1334 across the four pairs of
/// `tests/differential.rs`, with the rank inverted out of the same hole.
///
/// **Ablation** — delete the `update_totals(&mut game.kingdom, realm)` line from
/// `turn::step_zero` and this goes red on the first assertion: every input drops
/// to zero and the score falls back to the bracket.
#[test]
fn the_human_realms_score_inputs_are_rebuilt_on_the_humans_own_turn() {
    let mut g = five_realms();
    assert_eq!(g.kingdom.realms[1].score_inputs, [0; 6], "nothing has scored yet");
    assert!(g.kingdom.realms[1].is_human);

    turn::end_turn(&mut g).unwrap();

    let r = &g.kingdom.realms[1];
    assert_eq!(r.county_count, 1, "the human holds county 1");
    assert_eq!(r.score_inputs[0], r.share_of_map_pct, "+0x60, one county of fourteen");
    assert_eq!(r.score_inputs[1], r.population_total, "+0x10");
    assert_eq!(r.score_inputs[2], r.mean_happiness, "+0x0C");
    assert_eq!(r.score_inputs[3], r.mean_health, "+0x58");
    assert_eq!(r.score_inputs[4], r.total_men, "+0x54");
    assert_ne!(r.share_of_map_pct, 0, "a realm holding a county holds some of the map");
    assert_ne!(r.population_total, 0, "and some people");

    let bracket = Tables::DEFAULT.score_gold_bracket(r.gold);
    assert!(
        r.score > bracket,
        "the human scored {} where the gold bracket alone pays {bracket} - the five weighted \
         inputs are still not reaching Score_RankRealms",
        r.score
    );
    assert_eq!(r.score, r.compute_score(&Tables::DEFAULT), "and it is that expression");
}

/// The prologue's third line, `g_realms[r].offerPending = 0` (realm `+0x1C`).
#[test]
fn a_realms_outstanding_alliance_offer_is_cleared_at_the_top_of_its_own_turn() {
    let mut g = five_realms();
    g.kingdom.realms[2].offer_pending = true;
    g.kingdom.realms[1].offer_pending = true;
    turn::end_turn(&mut g).unwrap();
    assert!(!g.kingdom.realms[2].offer_pending, "the AI's offer was never cleared");
    assert!(!g.kingdom.realms[1].offer_pending, "nor the human's - the prologue is not gated");
}

/// **Realm `+0x4C` is the castle count, and it is the sixth score input.**
///
/// `Castle_BuildTick` (`0x004508DE`) is its only writer — clears realms 1..=5 and
/// increments the owner's for each county with `castleType != 0` and
/// `castleDegraded == 0`. Verified exhaustively
/// instruction in `Lords2.exe` whose operand mentions `g_realms + 0x4C` is one of
/// seven, and they are that clear, that increment,
/// `Game_SetupRealmsAndCounties`' initial clear, `Score_RankRealms` three times
/// and one painter.
///
/// **Two ablations, and they fail differently on purpose.** Delete the increment
/// in `Kingdom::castle_build_tick` and the first assertion goes red. Delete the
/// *clear* above it and only the second does,
/// added to is right the first season and wrong every season after.
#[test]
fn a_realms_finished_castles_are_counted_every_season_and_not_accumulated() {
    let mut g = five_realms();
    g.kingdom.counties[6].owner = 1;
    g.kingdom.counties[7].owner = 1;
    g.kingdom.counties[1].castle_type = 1;
    g.kingdom.counties[6].castle_type = 2;
    g.kingdom.counties[7].castle_type = 3;
    g.kingdom.counties[7].castle_degraded = 1;
    g.kingdom.counties[7].castle_work_left = 100_000;
    g.kingdom.counties[7].castle_work_total = 100_000;

    turn::end_turn(&mut g).unwrap();
    assert_eq!(
        g.kingdom.realms[1].score_inputs[l2_kingdom::tables::SCORE_INPUT_CASTLES],
        2,
        "two finished castles; county 7 is still building and belongs to +0x4D"
    );

    turn::end_turn(&mut g).unwrap();
    assert_eq!(
        g.kingdom.realms[1].score_inputs[l2_kingdom::tables::SCORE_INPUT_CASTLES],
        2,
        "the count is rebuilt each season, not accumulated"
    );
    assert_eq!(
        g.kingdom.realms[2].score_inputs[l2_kingdom::tables::SCORE_INPUT_CASTLES],
        0,
        "realm 2 has no castle"
    );
}

/// `docs/kingdom.md` §3.1: phase 1 sets the *unowned* counties' tax rates on
/// the neutral ladder, which `AI_SetTaxRates(0)` reads out of a chain of
/// comparisons — `< 20 -> 0`, `< 40 -> 1`, `< 50 -> 2`, `< 60 -> 3`,
/// `< 70 -> 4`, `< 80 -> 6`, `< 90 -> 8`, else 12 (`docs/decisions.md` C19).
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

    turn::end_turn(&mut a).unwrap();
    turn::end_turn(&mut b).unwrap();
    assert_eq!(a.kingdom, b.kingdom);
}

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



