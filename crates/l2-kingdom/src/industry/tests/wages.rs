#![allow(unused_imports)]
use super::*;
use super::output::*;
use super::harvest::*;
use super::castles::*;
use super::super::*;
use crate::tables::{INDUSTRY_ORDER, JOB_BLACKSMITH, JOB_NAMES};

#[test]
fn the_wage_bill_is_the_sum_over_units_and_ignores_troop_type() {
    let mut r = Realm::new();
    r.is_human = true;
    assert_eq!(compute_wages(T, &r, &[250, 252, 254], 0), 62 + 63 + 63);
    assert_eq!(compute_wages(T, &r, &[], 0), 0);
}

#[test]
fn a_realm_that_can_pay_clears_its_bankruptcy_counter() {
    let mut r = Realm::new();
    r.gold = 1000;
    r.wages = 400;
    r.bankrupt_stage = 3;
    let mut out = Vec::new();
    assert_eq!(pay(&mut r, 1, false, &mut out), BankruptcyAction::None);
    assert_eq!(r.gold, 600);
    assert_eq!(r.bankrupt_stage, 0);
    assert!(out.is_empty());
}

/// **The five stages, in order, each with the `L2.eng` group that says in
/// prose what its handler does.**
#[test]
fn the_escalation_runs_warning_desertion_desertion_desertion_mutiny() {
    let mut r = Realm::new();
    r.gold = 10;
    r.wages = 400;
    let mut out = Vec::new();
    let mut actions = Vec::new();
    for _ in 0..6 {
        actions.push(pay(&mut r, 2, false, &mut out));
    }
    assert_eq!(
        actions,
        vec![
            BankruptcyAction::Warned,      // stage 1 - group 270
            BankruptcyAction::Desertion,   // stage 2 - group 287
            BankruptcyAction::Desertion,   // stage 3
            BankruptcyAction::Desertion,   // stage 4
            BankruptcyAction::LastWarning, // stage 5 - group 271
            BankruptcyAction::Mutiny,      // and back to 0 - group 272
        ]
    );
    assert_eq!(actions[4], BankruptcyAction::LastWarning);
    assert_eq!(r.bankrupt_stage, 0, "the counter wraps rather than sticking");
    assert_eq!(out.len(), 6);
}

#[test]
fn an_unpayable_bill_costs_the_treasury_nothing_at_all() {
    let mut r = Realm::new();
    r.gold = 399;
    r.wages = 400;
    let mut out = Vec::new();
    pay(&mut r, 1, false, &mut out);
    assert_eq!(r.gold, 399, "one crown short, and it keeps all 399");
}

#[test]
fn the_first_unpaid_season_dismisses_mercenaries_if_there_are_any() {
    let mut with = Realm::new();
    with.wages = 1;
    let mut out = Vec::new();
    assert_eq!(pay(&mut with, 1, true, &mut out), BankruptcyAction::MercenariesDesert);
    assert_eq!(with.bankrupt_stage, 1);

    let mut without = Realm::new();
    without.wages = 1;
    assert_eq!(pay(&mut without, 1, false, &mut out), BankruptcyAction::Warned);
    assert_eq!(without.bankrupt_stage, 1);
}

#[test]
fn every_stage_names_the_l2_eng_group_it_raises() {
    assert_eq!(BankruptcyAction::None.message_id(), None);
    assert_eq!(BankruptcyAction::MercenariesDesert.message_id(), Some(0xA0));
    assert_eq!(BankruptcyAction::Warned.message_id(), Some(0x10E));
    assert_eq!(BankruptcyAction::Desertion.message_id(), Some(0x11F));
    assert_eq!(BankruptcyAction::LastWarning.message_id(), Some(0x10F));
    assert_eq!(BankruptcyAction::Mutiny.message_id(), Some(0x110));
    assert_eq!(BankruptcyAction::Mutiny.rival_message_id(), Some(0x111));
    assert_eq!(BankruptcyAction::Desertion.rival_message_id(), None);
}

#[test]
fn paying_once_clears_the_whole_escalation() {
    let mut r = Realm::new();
    r.wages = 100;
    let mut out = Vec::new();
    pay(&mut r, 1, false, &mut out);
    pay(&mut r, 1, false, &mut out);
    assert_eq!(r.bankrupt_stage, 2);
    r.gold = 100;
    assert_eq!(pay(&mut r, 1, false, &mut out), BankruptcyAction::None);
    assert_eq!(r.bankrupt_stage, 0);
}


