#![allow(unused_imports)]
use super::*;



    /// `UnitOrder_SiegeAttKnight` (`0x0048D9CE`) is the only writer of
    /// `g_battleWithdrawal` in the binary: an AI besieger whose whole force is
    /// knights, in front of a wall nothing has breached, stops trying. It is
    /// the only lever in the game that reaches
    /// [`l2_kingdom::battle::withdraw_casualties`], and until the clause was
/// added to `l2-sim` neither existed — so the campaign had never
    /// implemented the half of `Battle_ReturnToCampaign` behind it.
    #[test]
    fn an_all_knight_ai_besieger_withdraws_and_is_charged_for_it() {
        let (mut k, a, d) = siege_kingdom(2, [1, 0, 0]);
        {
            let au = k.campaign.units.get_mut(a).unwrap();
            au.owner = 2;
            au.owner_is_human = false;
            au.troops = [0; TROOP_TYPES];
            au.troops[TroopType::Knight.index()] = 400;
            au.men = 400;
            let du = k.campaign.units.get_mut(d).unwrap();
            du.owner = 1;
            du.owner_is_human = true;
        }
        k.counties[3].owner = 1;

        let assault = l2_kingdom::siege::assault(&k.counties, &mut k.campaign.units, a);
        let report = resolve_siege(&mut k, assault, Answer::TakeTheField, 0x5A11).expect("a siege");

        assert_eq!(
            report.resolution,
            Resolution::Fought { ticks: report_ticks(&report), cause: End::Withdrawal },
            "the knights gave up: {:?}",
            report.resolution
        );
        assert!(!report.verdict.attacker_won, "a withdrawal hands the field to the other side");

        assert_eq!(report.aftermath.withdrawal_casualties, Some(200));
        assert!(!report.aftermath.loser_destroyed, "two hundred knights is over fifty");
        assert!(report.aftermath.loser_siege_lifted);
        let survivor = k.campaign.units.get(a).expect("it walked off the field");
        assert_eq!(survivor.troops[TroopType::Knight.index()], 200);
        assert_eq!(survivor.men, 200);
        assert_eq!(survivor.besieging_county, 0, "and it lost the siege, not its life");
        assert_eq!(k.counties[3].owner, 1, "the castle held");
        assert_eq!(report.attacker_men, (400, 200));
    }

    fn report_ticks(r: &BattleReport) -> u32 {
        match r.resolution {
            Resolution::Fought { ticks, .. } | Resolution::Stalled { ticks } => ticks,
            Resolution::Autocalc => 0,
        }
    }

    #[test]
    fn one_turn_phase_two_builds_the_engines_assaults_and_takes_the_county() {
        let (mut k, a, d) = siege_kingdom(1, [0, 0, 0]);
        l2_kingdom::siege::order_engine(
            &mut k.campaign.units,
            a,
            l2_kingdom::siege::Engine::Catapult,
            1,
        );
        assert_eq!(k.campaign.units.get(a).unwrap().siege_seasons_left, 1);

        let reports = run_siege_phase(&mut k, Answer::Decline, 7);
        assert_eq!(reports.len(), 1, "one siege, one assault");
        let report = &reports[0];
        assert!(report.verdict.attacker_won, "a palisade against 350 men");
        assert_eq!(k.counties[3].owner, 1, "the county changed hands");
        assert_eq!(k.counties[3].garrison_unit, 0);
        assert!(k.campaign.units.get(d).is_none());
        assert_eq!(k.campaign.units.get(a).unwrap().engines[0].percent, 100);

        assert!(run_siege_phase(&mut k, Answer::Decline, 8).is_empty());
    }

    #[test]
    fn a_siege_still_building_survives_the_phase_untouched() {
        let (mut k, a, d) = siege_kingdom(5, [0, 0, 0]);
        for _ in 0..2 {
            l2_kingdom::siege::order_engine(
                &mut k.campaign.units,
                a,
                l2_kingdom::siege::Engine::BatteringRam,
                1,
            );
        }
        assert_eq!(k.campaign.units.get(a).unwrap().siege_seasons_left, 3);

        for expected in [2u8, 1, 0] {
            let reports = run_siege_phase(&mut k, Answer::Decline, 3);
            if expected == 0 {
                assert_eq!(reports.len(), 1, "the last season assaults");
            } else {
                assert!(reports.is_empty(), "still building");
                assert_eq!(k.campaign.units.get(a).unwrap().siege_seasons_left, expected);
                assert_eq!(k.counties[3].owner, 2, "the castle still stands");
                assert!(k.campaign.units.get(d).is_some());
            }
        }
    }

    #[test]
    fn a_besieger_with_no_engines_against_a_big_castle_is_released_rather_than_stalled() {
        let (mut k, a, d) = siege_kingdom(4, [0, 0, 0]);
        assert_eq!(k.campaign.units.get(a).unwrap().siege_seasons_left, 0, "nothing to build");
        let reports = run_siege_phase(&mut k, Answer::Decline, 1);
        assert!(reports.is_empty(), "no battle was fought");
        assert_eq!(k.campaign.units.get(a).unwrap().besieging_county, 0, "the siege was lifted");
        assert_eq!(k.campaign.units.get(d).unwrap().besieged_by, 0);
        assert_eq!(k.counties[3].owner, 2);
    }

    #[test]
    fn a_siege_picks_one_of_the_four_siege_banners_and_not_a_field_one() {
        let (mut k, a, _d) = siege_kingdom(1, [1, 0, 0]);
        let assault = l2_kingdom::siege::assault(&k.counties, &mut k.campaign.units, a);
        let report = resolve_siege(&mut k, assault, Answer::Decline, 1).unwrap();
        let (winner_owner, loser_owner) =
            if report.verdict.attacker_won { (1, 2) } else { (2, 1) };
        let banner = battle::outcome(report.verdict, true, 1, winner_owner, loser_owner);
        assert!(
            matches!(
                banner,
                battle::Outcome::SiegeWon
                    | battle::Outcome::SiegeLost
                    | battle::Outcome::SiegeLifted
                    | battle::Outcome::CastleLost
            ),
            "a siege never shows a field banner: {banner:?}"
        );
    }

