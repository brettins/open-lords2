#![allow(unused_imports)]
use super::*;
use super::allies::*;
use super::super::*;
use crate::realm::{LORD_BISHOP, LORD_ELIMINATED};

#[test]
fn the_opening_standing_is_the_realms_own_and_not_the_pairs() {
    let (realms, _) = world();
    for other in 1..MAX_REALMS {
        assert_eq!(realms[1].pair(other as u8).standing, 0, "the human's row is all zeros");
        assert_eq!(realms[2].pair(other as u8).standing, 5, "an AI opens at 5 on everyone");
        assert_eq!(realms[2].pair(other as u8).help_price_multiple, 1);
    }
    assert_eq!(realms[2].pair(2).standing, 5, "including itself: no other != me guard");
}

#[test]
fn a_person_takes_no_offence_because_the_guard_refuses_to_let_them() {
    let (mut realms, _) = world();
    let letters = offend(&mut realms, 1, 2, offence::BATTLE);
    assert!(letters.is_empty());
    assert_eq!(realms[1].pair(2).standing, 0, "unchanged, and it was 0 to begin with");

    let letters = offend(&mut realms, 2, 1, offence::BATTLE);
    assert!(letters.is_empty(), "no warning yet: standing has not bottomed out");
    assert_eq!(realms[2].pair(1).standing, 5 - 20);
}

#[test]
fn the_warning_ladder_runs_out_at_the_third_and_declares_war() {
    let (mut realms, _) = world();
    realms[2].pair_mut(1).standing = STANDING_MIN;
    let groups = |ls: Vec<Letter>| ls.iter().map(|l| l.group).collect::<Vec<_>>();
    assert_eq!(groups(offend(&mut realms, 2, 1, 1)), vec![group::WARNING_FIRST]);
    assert_eq!(groups(offend(&mut realms, 2, 1, 1)), vec![group::WARNING_SECOND]);
    assert!(!realms[2].pair(1).at_war);
    assert_eq!(groups(offend(&mut realms, 2, 1, 1)), vec![group::NOTICE_OF_REVENGE]);
    assert!(realms[2].pair(1).at_war, "the third is war");
    assert_eq!(realms[2].war_target, 1, "and it names the target");
    assert!(offend(&mut realms, 2, 1, 1).is_empty(), "and there is no fourth");
}

#[test]
fn a_betrayed_bishop_declares_neither_war_nor_a_target() {
    let (mut realms, _) = world();
    realms[2].lord = LORD_BISHOP;
    form_alliance(&mut realms, 2, 3);
    let letters = offend(&mut realms, 2, 3, offence::DWELLING);
    assert_eq!(letters[0].group, group::ALLIANCE_BROKEN);
    assert!(!realms[2].pair(3).at_war, "the Bishop stays technically at peace");
    assert_eq!(realms[2].war_target, 0, "and does not even name a target");
    assert_eq!(realms[2].ally, 0, "but the alliance is gone");

    let (mut realms, _) = world();
    realms[2].lord = 1;
    form_alliance(&mut realms, 2, 3);
    offend(&mut realms, 2, 3, offence::DWELLING);
    assert!(realms[2].pair(3).at_war, "every other lord flags a war");
    assert_eq!(realms[2].war_target, 3);
}

#[test]
fn betraying_an_ally_costs_fifteen_with_everybody_else() {
    let (mut realms, _) = world();
    form_alliance(&mut realms, 2, 3);
    offend(&mut realms, 2, 3, offence::BATTLE);
    assert_eq!(realms[4].pair(3).standing, 5 - 15, "realm 4 heard about it");
    assert_eq!(realms[5].pair(3).standing, 5 - 15);
    assert_eq!(realms[2].pair(3).standing, STANDING_MIN, "20 + 15 from 5, clamped");
}

#[test]
fn a_gift_is_judged_against_the_largest_you_have_ever_sent() {
    let (mut realms, _) = world();
    let t = Tables::DEFAULT;
    realms[2].lord = 1; // Knight, increment 100
    assert_eq!(reply_gift(&mut realms, &t, 2, 1, 100)[0].group, group::GIFT_PLEASED);
    assert_eq!(realms[2].pair(1).standing, 15);
    assert_eq!(realms[2].pair(1).best_gift, 100);
    assert_eq!(reply_gift(&mut realms, &t, 2, 1, 100)[0].group, group::GIFT_CONTEMPTUOUS);
    assert_eq!(realms[2].pair(1).standing, 7);
    assert_eq!(reply_gift(&mut realms, &t, 2, 1, 200)[0].group, group::GIFT_PLEASED);
}

#[test]
fn the_third_compliment_and_every_one_after_it_costs_four() {
    let (mut realms, mut d) = world();
    let t = Tables::DEFAULT;
    for expected in [group::COMPLIMENT_FIRST, group::COMPLIMENT_SECOND] {
        post(&mut realms, &mut d, 1, 2, Kind::Compliment, 0, 0);
        let letters = answer_inbox(&mut realms, &mut d, &t, 2);
        assert_eq!(letters[0].group, expected);
    }
    assert_eq!(realms[2].pair(1).standing, 5 + 15 + 8);
    for _ in 0..3 {
        post(&mut realms, &mut d, 1, 2, Kind::Compliment, 0, 0);
        let letters = answer_inbox(&mut realms, &mut d, &t, 2);
        assert_eq!(letters[0].group, group::COMPLIMENT_ENOUGH);
    }
    assert_eq!(realms[2].pair(1).standing, 5 + 15 + 8 - 12, "and it never resets");
}

#[test]
fn the_gold_moves_at_the_post_office() {
    let (mut realms, mut d) = world();
    realms[1].gold = 300;
    post(&mut realms, &mut d, 1, 2, Kind::Gift, 1000, 0);
    assert_eq!(realms[1].gold, 0, "clamped to what was there");
    assert_eq!(realms[2].gold, 5000 + 300);
    assert!(realms[2].pair(1).has_mail);
}

#[test]
fn the_inbox_holds_five_and_the_sixth_letter_is_lost() {
    let (mut realms, mut d) = world();
    for _ in 0..6 {
        post(&mut realms, &mut d, 1, 2, Kind::Compliment, 0, 0);
    }
    assert_eq!(d.pending(2).count(), 5);
    assert_eq!(
        realms[2].pair(1).compliments_from,
        5,
        "and the sixth is lost outright: `Diplo_Post` returns off the end of \
         the slot walk before it counts the compliment, sets `has_mail` or \
         moves any gold — so a gift into a full inbox is not even spent"
    );
    answer_inbox(&mut realms, &mut d, &Tables::DEFAULT, 2);
    assert_eq!(d.pending(2).count(), 0);
    assert!(!realms[2].pair(1).has_mail);
}

#[test]
fn eleven_accepts_outright_and_minus_eleven_refuses_outright() {
    let (mut realms, mut d) = world();
    realms[2].pair_mut(1).standing = 11;
    assert_eq!(
        reply_alliance_offer(&mut realms, &mut d, 2, 1, 5)[0].group,
        group::ALLIANCE_ACCEPTED
    );
    assert_eq!(realms[2].ally, 1);
    assert_eq!(realms[2].pair(1).standing, 15);

    let (mut realms, mut d) = world();
    realms[2].pair_mut(1).standing = -11;
    assert_eq!(
        reply_alliance_offer(&mut realms, &mut d, 2, 1, 5)[0].group,
        group::ALLIANCE_REFUSED
    );
    assert_eq!(realms[2].ally, 0);
}

#[test]
fn war_blocks_an_alliance_at_any_standing_at_all() {
    let (mut realms, mut d) = world();
    realms[2].pair_mut(1).standing = STANDING_MAX;
    realms[2].pair_mut(1).at_war = true;
    let l = reply_alliance_offer(&mut realms, &mut d, 2, 1, 5);
    assert_eq!(l[0].group, group::ALLIANCE_AT_WAR, "\"Retort to alliance offer.\"");
    assert_eq!(realms[2].ally, 0);
    assert_eq!(realms[2].pair(1).standing, STANDING_MAX - 1, "a retort costs one, not two");
}

#[test]
fn an_ai_forgives_another_ai_and_never_forgives_a_person() {
    let (mut realms, _) = world();
    let t = Tables::DEFAULT;
    realms[2].pair_mut(1).standing = -20;
    realms[2].pair_mut(3).standing = -20;
    for _ in 0..10 {
        ai_diplomacy(&mut realms, &t, 2, 1268, 0);
    }
    assert_eq!(realms[2].pair(1).standing, -20, "the human is realm 1");
    assert_eq!(realms[2].pair(3).standing, -10, "and realm 3 is not");
}

