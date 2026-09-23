#![allow(unused_imports)]
use super::*;
use super::standing::*;
use super::super::*;
use crate::realm::{LORD_BISHOP, LORD_ELIMINATED};

#[test]
fn an_ai_courts_the_best_ranked_realm_it_can_reach_after_its_lords_interval() {
    let (mut realms, _) = world();
    let t = Tables::DEFAULT;
    realms[2].lord = LORD_BISHOP; // interval 4, the shortest
    realms[2].rank = 3;
    for _ in 0..8 {
        ai_diplomacy(&mut realms, &t, 2, 1268, 0);
    }
    assert_eq!(realms[2].ally, 0, "1268 is too early");
    for turn in 1..=4 {
        ai_diplomacy(&mut realms, &t, 2, 1269, 0);
        if turn < 4 {
            assert_eq!(realms[2].ally, 0, "the interval has not run out");
        }
    }
    assert_ne!(realms[2].ally, 0, "and on the fourth it acts");
    assert_ne!(realms[2].ally, 1, "never the person: that goes through group 180");
}

#[test]
fn a_dead_ally_ends_the_step_as_well_as_the_alliance() {
    let (mut realms, _) = world();
    let t = Tables::DEFAULT;
    realms[2].rank = 3;
    form_alliance(&mut realms, 2, 3);
    realms[3].strength = 0;
    realms[3].lord = LORD_ELIMINATED;
    ai_diplomacy(&mut realms, &t, 2, 1300, 0);
    assert_eq!(realms[2].ally, 0);
    assert_eq!(realms[2].ally_candidate, 0, "the courtship did not run");
}

#[test]
fn a_grudge_break_costs_five_and_not_twenty() {
    let (mut realms, _) = world();
    let t = Tables::DEFAULT;
    realms[2].lord = 1; // Knight, tolerance 5
    realms[2].rank = 3;
    form_alliance(&mut realms, 2, 3);
    realms[2].pair_mut(3).standing = -20;
    let before = realms[4].pair(3).standing;
    let letters = ai_diplomacy(&mut realms, &t, 2, 1300, 0);
    assert_eq!(realms[2].ally, 0);
    assert_eq!(letters.last().unwrap().group, group::ALLIANCE_ENDED);
    assert_eq!(realms[2].pair(3).standing, -24, "five, not five plus fifteen");
    assert_eq!(realms[4].pair(3).standing, before, "and nobody else hears about it");
}

#[test]
fn asking_whether_an_act_against_an_ally_counts_corrodes_the_alliance() {
    let (mut realms, _) = world();
    form_alliance(&mut realms, 2, 3);
    for _ in 0..6 {
        assert!(!action_allowed(&mut realms, 2, 3));
    }
    assert_eq!(realms[2].pair(3).grudge, 6, "six looks at the map, six grudge");
    assert!(action_allowed(&mut realms, 2, 4), "a non-ally is allowed and costs nothing");
    assert!(action_allowed(&mut realms, 2, 0), "and target 0 always is");
    assert!(!action_allowed(&mut realms, 2, 2), "acting on yourself is not");
}

#[test]
fn reconciling_repairs_a_one_sided_pairing_and_drops_a_contested_one() {
    let (mut realms, _) = world();
    realms[2].ally = 3;
    reconcile_alliances(&mut realms);
    assert_eq!(realms[3].ally, 2, "repaired");
    assert!(realms[2].pair(3).allied && realms[3].pair(2).allied);

    let (mut realms, _) = world();
    realms[2].ally = 3;
    realms[3].ally = 4;
    realms[4].ally = 3;
    reconcile_alliances(&mut realms);
    assert_eq!(realms[2].ally, 0, "realm 3 was already spoken for");
    assert_eq!(realms[3].ally, 4);
}

#[test]
fn the_price_of_help_ratchets_and_the_asking_costs_standing() {
    let (mut realms, _) = world();
    realms[2].lord = 1; // Knight, base price 500
    form_alliance(&mut realms, 2, 1);
    realms[2].pair_mut(1).standing = 20;
    realms[1].gold = 10_000;
    pay_for_help(&mut realms, 2, 1, 4, 500);
    assert_eq!(realms[1].gold, 9500);
    assert_eq!(realms[2].gold, 5500);
    assert_eq!(realms[2].target_county, 4, "and the ally marches on it");
    assert_eq!(realms[2].pair(1).help_price_multiple, 2);
    assert_eq!(realms[2].pair(1).standing, 16, "minus four for having been made to");

    realms[1].gold = 100;
    pay_for_help(&mut realms, 2, 1, 5, 1000);
    assert_eq!(realms[1].gold, 100, "cannot afford it, so nothing happens at all");
    assert_eq!(realms[2].target_county, 4);
    assert_eq!(realms[2].pair(1).help_price_multiple, 2);
}

#[test]
fn a_thinly_peopled_ally_will_not_march_at_any_standing() {
    let (mut realms, mut d) = world();
    let t = Tables::DEFAULT;
    realms[2].lord = 1; // Knight, floor 750
    form_alliance(&mut realms, 2, 1);
    realms[2].pair_mut(1).standing = STANDING_MAX;
    realms[2].population_mean = 749;
    realms[2].gold = 1_000_000;
    let l = reply_help_request(&mut realms, &mut d, &t, 2, 1, 4);
    assert_eq!(l[0].group, group::HELP_REFUSED, "gold is not what is asked about");
    assert_eq!(realms[2].pair(1).grudge, 1);
    realms[2].population_mean = 750;
    let l = reply_help_request(&mut realms, &mut d, &t, 2, 1, 4);
    assert_ne!(l[0].group, group::HELP_REFUSED);
}

#[test]
fn an_attack_request_refused_costs_two_grudge_where_help_costs_one() {
    let (mut realms, mut d) = world();
    let t = Tables::DEFAULT;
    reply_help_request(&mut realms, &mut d, &t, 2, 1, 4);
    assert_eq!(realms[2].pair(1).grudge, 1, "not even my ally");
    reply_attack_request(&mut realms, &mut d, &t, 2, 1, 4);
    assert_eq!(realms[2].pair(1).grudge, 3);
}

#[test]
fn terminating_an_alliance_sends_nothing_at_all() {
    let (mut realms, _) = world();
    form_alliance(&mut realms, 2, 1);
    let letters = reply_alliance_end(&mut realms, 2, 1);
    assert!(letters.is_empty(), "\"Broken alliance.\" is the other path");
    assert_eq!(realms[2].ally, 0);
    assert_eq!(realms[2].pair(1).standing, 5 - 15);
}

#[test]
fn the_four_recorded_takes_cycle() {
    let (mut realms, _) = world();
    realms[2].lord = 2; // Baron: variants 4..=7
    let mut seen = Vec::new();
    for _ in 0..5 {
        seen.push(reply_insult(&mut realms, 2, 1)[0].variant);
    }
    assert_eq!(seen, vec![4, 5, 6, 7, 4]);
}

#[test]
fn step_two_writes_a_standing_a_war_target_and_an_ally() {
    let (mut realms, _) = world();
    let t = Tables::DEFAULT;
    realms[2].pair_mut(3).standing = 0;
    ai_diplomacy(&mut realms, &t, 2, 1268, 0);
    assert_eq!(realms[2].pair(3).standing, 1);
    realms[2].rank = 3;
    realms[2].lord = LORD_BISHOP;
    for _ in 0..4 {
        ai_diplomacy(&mut realms, &t, 2, 1269, 0);
    }
    assert_ne!(realms[2].ally, 0);
    let (mut realms, _) = world();
    form_alliance(&mut realms, 2, 3);
    offend(&mut realms, 2, 3, offence::BATTLE);
    assert_eq!(realms[2].war_target, 3);
    pay_for_help(&mut realms, 2, 3, 9, 0);
    assert_eq!(realms[2].target_county, 9);
}

