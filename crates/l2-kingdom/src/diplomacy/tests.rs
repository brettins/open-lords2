#![allow(unused_imports)]
use super::*;
use super::standing::*;
use super::inbox::*;
use super::replies::*;
use super::ai::*;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use l2_net::Pcg32;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realm::{LORD_BISHOP, LORD_ELIMINATED};

    fn world() -> ([Realm; MAX_REALMS], Diplomacy) {
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        for (id, r) in realms.iter_mut().enumerate().take(MAX_REALMS).skip(1) {
            r.in_play = true;
            r.strength = 3;
            r.lord = ((id - 1) % 4 + 1) as u8;
            r.rank = id as u8;
            r.gold = 5000;
            r.population_mean = 4000;
        }
        realms[1].is_human = true;
        realms[1].lord = crate::realm::LORD_HUMAN;
        let mut d = Diplomacy::new(7);
        init(&mut realms, &mut d);
        (realms, d)
    }

    /// `Diplo_Init` gives an in-play AI realm 5 towards **everyone**, a human 0
    /// towards everyone, and opens the help multiple at 1.
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

    /// The whole hook is skipped when the offended realm is a person — the
    /// `isHuman == 0` clause is in `Diplo_Offend`'s entry guard.
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

    /// Two warnings, then war — and only against a person.
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

    /// The betrayal branch, and the Bishop's exemption from **both** writes.
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

    /// Betrayal is the only act that costs reputation with third parties.
    #[test]
    fn betraying_an_ally_costs_fifteen_with_everybody_else() {
        let (mut realms, _) = world();
        form_alliance(&mut realms, 2, 3);
        offend(&mut realms, 2, 3, offence::BATTLE);
        assert_eq!(realms[4].pair(3).standing, 5 - 15, "realm 4 heard about it");
        assert_eq!(realms[5].pair(3).standing, 5 - 15);
        assert_eq!(realms[2].pair(3).standing, STANDING_MIN, "20 + 15 from 5, clamped");
    }

    /// The gift ratchet: the second gift is judged against the first.
    #[test]
    fn a_gift_is_judged_against_the_largest_you_have_ever_sent() {
        let (mut realms, _) = world();
        let t = Tables::DEFAULT;
        realms[2].lord = 1; // Knight, increment 100
        assert_eq!(reply_gift(&mut realms, &t, 2, 1, 100)[0].group, group::GIFT_PLEASED);
        assert_eq!(realms[2].pair(1).standing, 15);
        assert_eq!(realms[2].pair(1).best_gift, 100);
        // The same gift again is now under best + T/2 and costs eight.
        assert_eq!(reply_gift(&mut realms, &t, 2, 1, 100)[0].group, group::GIFT_CONTEMPTUOUS);
        assert_eq!(realms[2].pair(1).standing, 7);
        assert_eq!(reply_gift(&mut realms, &t, 2, 1, 200)[0].group, group::GIFT_PLEASED);
    }

    /// Three compliments and you have overdone it, permanently.
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

    /// A gift is spent when it is posted, not when it is answered — and it is
/// clamped to what the sender holds.
    #[test]
    fn the_gold_moves_at_the_post_office() {
        let (mut realms, mut d) = world();
        realms[1].gold = 300;
        post(&mut realms, &mut d, 1, 2, Kind::Gift, 1000, 0);
        assert_eq!(realms[1].gold, 0, "clamped to what was there");
        assert_eq!(realms[2].gold, 5000 + 300);
        assert!(realms[2].pair(1).has_mail);
    }

    /// The inbox is five deep, dense, and emptied every turn whether or not it
    /// was full.
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

    /// The alliance offer's ladder, at the two break points the lord card
    /// draws.
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

    /// At war is a permanent bar, and it is answered with a different group
    /// from the ordinary refusal.
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

    /// **Step 2's heal is asymmetric**, and it is the sharpest single fact in
    /// the subsystem: damage a person does never heals.
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

    /// Courtship: not before 1269, not while ranked first, and only after the
    /// lord's own interval of turns.
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

    /// An eliminated ally drops the alliance and **returns**, so the courtship
    /// below never runs that turn.
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

    /// The grudge break is much cheaper than a betrayal, because the alliance
    /// is already gone by the time `offend` looks at it.
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
        // −20, healed to −19 by the step's own first pass, then −5.
        assert_eq!(realms[2].pair(3).standing, -24, "five, not five plus fifteen");
        assert_eq!(realms[4].pair(3).standing, before, "and nobody else hears about it");
    }

    /// `Diplo_ActionAllowed` charges the asker a point of grudge every time it
    /// says no. `docs/bugs.md` B74, reproduced.
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

    /// A one-sided alliance is **repaired**, not dropped — which is the
    /// opposite of what `docs/diplomacy.md` §4.1 said.
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

    /// Help is a request, and the price of it doubles every time.
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

    /// An ally with too few people refuses whatever the standing — and the
    /// floor is a population, not a treasury.
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

    /// Asking an ally to attack is twice as annoying as asking for help.
    #[test]
    fn an_attack_request_refused_costs_two_grudge_where_help_costs_one() {
        let (mut realms, mut d) = world();
        let t = Tables::DEFAULT;
        reply_help_request(&mut realms, &mut d, &t, 2, 1, 4);
        assert_eq!(realms[2].pair(1).grudge, 1, "not even my ally");
        reply_attack_request(&mut realms, &mut d, &t, 2, 1, 4);
        assert_eq!(realms[2].pair(1).grudge, 3);
    }

    /// Ending an alliance from the screen is silent and costs fifteen.
    #[test]
    fn terminating_an_alliance_sends_nothing_at_all() {
        let (mut realms, _) = world();
        form_alliance(&mut realms, 2, 1);
        let letters = reply_alliance_end(&mut realms, 2, 1);
        assert!(letters.is_empty(), "\"Broken alliance.\" is the other path");
        assert_eq!(realms[2].ally, 0);
        assert_eq!(realms[2].pair(1).standing, 5 - 15);
    }

    /// Every message advances the sender's voice rotation, so the lord's four
/// recorded takes cycle.
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

    /// The whole point of the module: a played turn writes the four fields the
    /// raid handler starves without.
    #[test]
    fn step_two_writes_a_standing_a_war_target_and_an_ally() {
        let (mut realms, _) = world();
        let t = Tables::DEFAULT;
        // A standing, from the heal.
        realms[2].pair_mut(3).standing = 0;
        ai_diplomacy(&mut realms, &t, 2, 1268, 0);
        assert_eq!(realms[2].pair(3).standing, 1);
        // An ally, from the courtship.
        realms[2].rank = 3;
        realms[2].lord = LORD_BISHOP;
        for _ in 0..4 {
            ai_diplomacy(&mut realms, &t, 2, 1269, 0);
        }
        assert_ne!(realms[2].ally, 0);
        // A war target, from an act.
        let (mut realms, _) = world();
        form_alliance(&mut realms, 2, 3);
        offend(&mut realms, 2, 3, offence::BATTLE);
        assert_eq!(realms[2].war_target, 3);
        // And a target county, from a paid request.
        pay_for_help(&mut realms, 2, 3, 9, 0);
        assert_eq!(realms[2].target_county, 9);
    }
}

