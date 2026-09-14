#![allow(unused_imports)]
use super::*;
use super::geometry::*;
use super::interaction::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::nobles::{self, NoblesScreen};
use l2_game::Game;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;

/// **`FUN_00415E42`, category by category**, and it is undocumented anywhere
/// else: the seven rules were read out of the decompilation for this branch.
///
/// Every category takes a different realm field, so one realm carrying a
/// distinct number in each is enough to prove none of the seven is reading its
/// neighbour's.
#[test]
fn each_category_reads_its_own_realm_field() {
    let mut realms = five_realms();
    let r = &mut realms[1];
    r.county_count = 7;
    r.score_inputs[SCORE_INPUT_CASTLES] = 3;
    r.total_men = 900;
    r.gold = 4_000;
    r.mean_happiness = 55;
    r.population_total = 12_345;
    r.rank = 2;

    let at = |c: usize| nobles::value(&realms[1], c, 1300);
    assert_eq!(at(nobles::COUNTIES), 7, "+0x29");
    assert_eq!(at(nobles::CASTLES), 3, "+0x4C");
    assert_eq!(at(nobles::TROOPS), 900, "+0x54");
    assert_eq!(at(nobles::CROWNS), 4_000, "+0x118");
    assert_eq!(at(nobles::HAPPINESS), 55, "+0x0C");
    assert_eq!(at(nobles::PEOPLE), 12_345, "+0x10");
    // `6 - rank`, so the top of the table scores 5 and the bottom 1.
    assert_eq!(at(nobles::GREATEST_NOBLE), 4, "6 - rank");
}

/// **A realm out of play scores zero in every category**, which is the
/// function's first statement and the reason a dead realm's pole is not drawn.
#[test]
fn a_realm_out_of_play_scores_nothing_anywhere() {
    let mut realms = five_realms();
    realms[1].in_play = false;
    realms[1].gold = 9_999;
    realms[1].county_count = 12;
    for c in 0..nobles::CATEGORIES {
        assert_eq!(nobles::value(&realms[1], c, 1300), 0, "category {c}");
    }
}

/// **The castle count is read as a byte** — `(uint)(byte)field_0x4c`. A 256th
/// castle reads as none.
///
/// Ablation, run: drop the `as u8` in [`nobles::value`] and this returns 256.
#[test]
fn the_castle_count_is_truncated_to_a_byte_as_the_original_truncates_it() {
    let mut realms = five_realms();
    realms[1].score_inputs[SCORE_INPUT_CASTLES] = 256;
    assert_eq!(nobles::value(&realms[1], nobles::CASTLES, 1300), 0);
    realms[1].score_inputs[SCORE_INPUT_CASTLES] = 257;
    assert_eq!(nobles::value(&realms[1], nobles::CASTLES, 1300), 1);
}

/// **The overall standing is dead until 1270** — `if (g_year < 0x4F6) return
/// 2;`, for every realm, so the page reads *"Greatest noble, undecided."* for
/// the first two years of every game whatever the ranks say.
#[test]
fn the_greatest_noble_is_undecided_until_1270() {
    let mut realms = five_realms();
    for (i, r) in realms.iter_mut().enumerate().skip(1) {
        r.rank = i as u8;
    }

    for year in [1268, 1269] {
        for r in 1..l2_kingdom::MAX_REALMS {
            assert_eq!(
                nobles::value(&realms[r], nobles::GREATEST_NOBLE, year),
                nobles::GREATEST_NOBLE_EARLY,
                "year {year}, realm {r}",
            );
        }
        let s = nobles::rank(&realms, nobles::GREATEST_NOBLE, year);
        assert!(s.all_level, "year {year}: every realm is level, so nothing leads");
        assert!(!s.decided(), "year {year}: the line reads `undecided.`");
    }

    // And the year it comes alive, with ranks that are not all equal.
    let s = nobles::rank(&realms, nobles::GREATEST_NOBLE, 1270);
    assert!(!s.all_level, "1270: the ranks differ, so the bars differ");
    assert!(s.decided(), "1270: rank 1 leads outright");
    assert_eq!(s.leader, 1, "realm 1 holds rank 1");
}

/// **The bars are a percentage of the leader**, clamped 0…100 — not of a
/// maximum and not of the sum. The leader's pole is always full height.
#[test]
fn the_bars_are_each_realms_share_of_the_leaders_score() {
    let mut realms = five_realms();
    realms[1].gold = 1_000;
    realms[2].gold = 500;
    realms[3].gold = 250;
    realms[4].gold = 0;
    realms[5].gold = 1;

    let s = nobles::rank(&realms, nobles::CROWNS, 1300);
    assert_eq!(s.leader, 1);
    assert!(s.decided(), "one realm is ahead on its own");
    assert_eq!(s.pct[1], 100);
    assert_eq!(s.pct[2], 50);
    assert_eq!(s.pct[3], 25);
    assert_eq!(s.pct[4], 0);
    // `PctOf(1, 1000)` is `1 * 100 / 1000`, which is integer zero — a realm
    // with a crown in the bank is drawn
    assert_eq!(s.pct[5], 0, "integer division, and the original's too");
}

/// **A tie for the lead is won by the highest realm index** — the loop is
/// `if (best <= v)`, not `<`.
///
/// It never shows on screen, because the same function then calls the category
/// tied and the line prints *"undecided."* instead of a name. It is reproduced
/// because [`nobles::Standings::leader`] is what *would* be printed, and a
/// leader chosen the other way would be a different name the day either flag
/// stops being set.
#[test]
fn a_tie_for_the_lead_goes_to_the_highest_realm_index() {
    let mut realms = five_realms();
    realms[1].gold = 100;
    realms[4].gold = 100;
    realms[2].gold = 10;

    let s = nobles::rank(&realms, nobles::CROWNS, 1300);
    assert_eq!(s.leader, 4, "`best <= v` keeps the later realm");
    assert!(s.tied_at_top, "and the page says so rather than naming it");
    assert!(!s.all_level, "realm 2 is behind, so this is not the level case");
    assert!(!s.decided());
}

/// **Level is not the same as tied**, and the original draws them differently:
/// a level category sets every in-play bar to **50**, a tie at the top leaves
/// the real percentages standing. Both print *"undecided."*.
#[test]
fn a_level_category_draws_every_bar_at_fifty() {
    let mut realms = five_realms();
    for r in realms.iter_mut().skip(1) {
        r.county_count = 3;
    }

    let s = nobles::rank(&realms, nobles::COUNTIES, 1300);
    assert!(s.all_level);
    assert!(!s.tied_at_top, "the tie test is only reached when the category is NOT level");
    for r in 1..l2_kingdom::MAX_REALMS {
        assert_eq!(s.pct[r], nobles::LEVEL_BAR_PCT, "realm {r}");
    }

    // The ablation the arrangement is about: make one realm differ and the
    // bars stop being 50 even though two of the others still match.
    realms[5].county_count = 4;
    let s = nobles::rank(&realms, nobles::COUNTIES, 1300);
    assert!(!s.all_level);
    assert_eq!(s.pct[5], 100);
    assert_eq!(s.pct[1], 75);
    assert!(!s.tied_at_top, "nobody else matches the leader");
    assert!(s.decided(), "so the page names realm 5");
}

/// **A realm out of play does not make a category level**, and does not tie
/// it: both loops skip it. Four realms all on 3 counties with a dead fifth is
/// still level.
#[test]
fn a_dead_realm_is_not_counted_level_or_tied() {
    let mut realms = five_realms();
    for r in realms.iter_mut().skip(1) {
        r.county_count = 3;
    }
    realms[5].in_play = false;
    realms[5].county_count = 0;

    let s = nobles::rank(&realms, nobles::COUNTIES, 1300);
    assert!(s.all_level, "the four live realms agree; the dead one is not asked");
}

