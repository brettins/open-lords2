#![allow(unused_imports)]
use super::*;
use super::report::*;
use super::fight::*;
use super::siege::*;
use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

/// `docs/battle.md` §4.1: the campaign record's `+0x16C`, `TROOPS*.ENG`'s
/// columns and the `.skr` army record all agree on it — three independent
/// sources, so it is safe to cross between the crates by index at
/// all.
#[cfg(test)]
const COLUMN_ORDER: [(&str, &str); TROOP_TYPES] = [
    ("Peasant", "Peasants"),
    ("Crossbowman", "Crossbowmen"),
    ("Maceman", "Macemen"),
    ("Swordsman", "Swordsmen"),
    ("Pikeman", "Pikemen"),
    ("Archer", "Archers"),
    ("Knight", "Knights"),
];

#[test]
fn the_two_crates_number_the_troop_types_identically() {
    for (t, (kingdom, sim)) in COLUMN_ORDER.iter().enumerate() {
        assert_eq!(l2_kingdom::unit::ALL_TROOP_TYPES[t].index(), t);
        assert_eq!(l2_sim::ALL_TROOPS[t].index(), t);
        assert_eq!(
            format!("{:?}", l2_kingdom::unit::ALL_TROOP_TYPES[t]),
            *kingdom,
            "l2-kingdom column {t}"
        );
        assert_eq!(format!("{:?}", l2_sim::ALL_TROOPS[t]), *sim, "l2-sim column {t}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_kingdom::unit::{TroopType, Unit, UnitKind};

    fn kingdom_with(
        attacker: &[(TroopType, i32)],
        defender: &[(TroopType, i32)],
        defender_owner: u8,
        defender_human: bool,
    ) -> (Kingdom, usize, usize) {
        let mut k = Kingdom::new(1);
        k.counties[3].owner = 0;
        k.counties[3].population = 546;
        k.counties[3].happiness = 79;
        k.realms[1].in_play = true;
        k.realms[1].is_human = true;

        let mut a = Unit::new(UnitKind::Army, 1, 33, 17);
        a.owner_is_human = true;
        for &(t, n) in attacker {
            a.troops[t.index()] = n;
        }
        a.men = a.troops.iter().sum();
        a.moves_used = 8;
        let ai = k.campaign.units.spawn(a).unwrap();

        let mut d = Unit::new(UnitKind::Army, defender_owner, 36, 17);
        d.owner_is_human = defender_human;
        d.home_county = 3;
        d.defence_mark = l2_kingdom::conquest::RAISED;
        d.move_allowance = 0;
        for &(t, n) in defender {
            d.troops[t.index()] = n;
        }
        d.men = d.troops.iter().sum();
        let di = k.campaign.units.spawn(d).unwrap();
        (k, ai, di)
    }

    #[test]
    fn the_fixture_battle_runs_from_the_campaign_and_returns_the_saves_numbers() {
        let (mut k, a, d) = kingdom_with(
            &[(TroopType::Peasant, 128), (TroopType::Swordsman, 25), (TroopType::Archer, 25)],
            &[(TroopType::Peasant, 122), (TroopType::Archer, 60)],
            l2_kingdom::levy::OWNERLESS,
            false,
        );

        let report = resolve(
            &mut k,
            Attack::Battle { attacker: a, defender: d },
            3,
            Answer::Decline,
            1,
        )
        .expect("a battle");

        assert_eq!(report.settlement, Settlement::Prompt);
        assert_eq!(report.resolution, Resolution::Autocalc, "declining is the autocalc");

        assert!(!report.verdict.attacker_won);
        assert_eq!(report.verdict.winner(), d);
        assert_eq!(report.attacker_men, (178, 0));
        assert_eq!(report.defender_men, (182, 36), "the ladder's second rung, 20%");

        assert_eq!(report.aftermath.county_taken_by, None);
        assert_eq!(k.counties[3].owner, 0, "county 3 is still neutral");
        assert!(k.campaign.units.get(a).is_none(), "the attacker was destroyed");
        assert!(k.campaign.units.get(d).is_none(), "the defence was dissolved");

        assert_eq!(report.defenders_returned, 36);
        assert_eq!(k.counties[3].population, 582);
    }

    #[test]
    fn taking_the_field_runs_the_real_simulation_and_hands_the_result_back() {
        let (mut k, a, d) = kingdom_with(
            &[(TroopType::Peasant, 128), (TroopType::Swordsman, 25), (TroopType::Archer, 25)],
            &[(TroopType::Peasant, 122), (TroopType::Archer, 60)],
            l2_kingdom::levy::OWNERLESS,
            false,
        );

        let report = resolve(
            &mut k,
            Attack::Battle { attacker: a, defender: d },
            3,
            Answer::TakeTheField,
            l2_sim::runner::DEFAULT_SEED,
        )
        .expect("a battle");

        let Resolution::Fought { ticks, cause } = report.resolution else {
            panic!("taking the field must fight it to a conclusion: {:?}", report.resolution);
        };
        assert!(ticks > 0);
        assert_eq!(cause, End::Annihilation);
        eprintln!(
            "fought: {ticks} ticks, {cause:?}, attacker {:?}, defender {:?}, winner {}",
            report.attacker_men,
            report.defender_men,
            if report.verdict.attacker_won { "attacker" } else { "defender" }
        );

        let (a0, a1) = report.attacker_men;
        let (d0, d1) = report.defender_men;
        assert_eq!((a0, d0), (178, 182));
        assert!(a1 < a0 || d1 < d0, "a battle in which nobody died is not a battle");
        assert!(a1 >= 0 && d1 >= 0 && a1 <= a0 && d1 <= d0);

        let loser = report.verdict.loser();
        assert!(k.campaign.units.get(loser).is_none());

        if report.verdict.attacker_won {
            assert_eq!(report.defenders_returned, 0);
            assert_eq!(k.counties[3].population, 546);
            assert_eq!(k.counties[3].owner, 1, "a marked defence loses the county");
        } else {
            assert_eq!(k.counties[3].population, 546 + report.defenders_returned);
            assert_eq!(k.counties[3].owner, 0);
        }
    }

    #[test]
    fn an_ai_battle_is_settled_silently_whatever_the_player_would_have_answered() {
        for answer in [Answer::TakeTheField, Answer::Decline] {
            let (mut k, a, d) = kingdom_with(
                &[(TroopType::Knight, 100)],
                &[(TroopType::Peasant, 40)],
                2,
                false,
            );
            k.campaign.units.get_mut(a).unwrap().owner = 3;
            k.campaign.units.get_mut(a).unwrap().owner_is_human = false;

            let report =
                resolve(&mut k, Attack::Battle { attacker: a, defender: d }, 3, answer, 1)
                    .unwrap();
            assert_eq!(report.settlement, Settlement::Silently);
            assert_eq!(report.resolution, Resolution::Autocalc);
            assert!(report.verdict.attacker_won);
            assert_eq!(k.counties[3].owner, 3, "and the AI took the county");
        }
    }

    #[test]
    fn the_same_battle_fought_twice_gives_the_same_answer() {
        let run = || {
            let (mut k, a, d) = kingdom_with(
                &[(TroopType::Swordsman, 90), (TroopType::Peasant, 60)],
                &[(TroopType::Pikeman, 80), (TroopType::Maceman, 40)],
                2,
                false,
            );
            k.campaign.units.get_mut(d).unwrap().defence_mark = 0;
            let r = resolve(
                &mut k,
                Attack::Battle { attacker: a, defender: d },
                3,
                Answer::TakeTheField,
                0xC0FF_EE01,
            )
            .unwrap();
            (r.attacker_men, r.defender_men, r.verdict.attacker_won, r.resolution)
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_campaign_army_is_raised_at_the_scale_the_two_totals_choose() {
        let (k, a, d) = kingdom_with(
            &[(TroopType::Peasant, 128), (TroopType::Swordsman, 25), (TroopType::Archer, 25)],
            &[(TroopType::Peasant, 122), (TroopType::Archer, 60)],
            6,
            false,
        );
        let at = muster_of(&k, a).unwrap();
        let dt = muster_of(&k, d).unwrap();
        let runner = BattleRunner::deploy_muster(
            blank_field(),
            1,
            Muster { troops: &at, owner: 1, human: true },
            Muster { troops: &dt, owner: 6, human: false },
        );
        assert_eq!(runner.men_per_figure(SIDE_B), 8);
        assert_eq!(runner.men_per_figure(SIDE_A), 8);
        assert_eq!(runner.survivors(SIDE_B)[TroopType::Peasant.index()], 128);
        assert_eq!(runner.survivors(SIDE_B)[TroopType::Swordsman.index()], 25);
        assert_eq!(runner.survivors(SIDE_A)[TroopType::Archer.index()], 60);
        assert_eq!(runner.men(SIDE_B) + runner.men(SIDE_A), 360, "nobody lost in the raising");
    }


    fn siege_kingdom(castle_type: u8, engines: [i16; 3]) -> (Kingdom, usize, usize) {
        let (mut k, a, d) = kingdom_with(
            &[(TroopType::Peasant, 200), (TroopType::Knight, 150)],
            &[(TroopType::Archer, 100), (TroopType::Pikeman, 50)],
            2,
            false,
        );
        k.counties[3].owner = 2;
        k.counties[3].castle_type = castle_type;
        k.counties[3].garrison_unit = d;
        k.realms[2].in_play = true;

        let du = k.campaign.units.get_mut(d).unwrap();
        du.garrison_county = 3;
        du.defence_mark = 0;
        du.besieged_by = a as u8;
        let au = k.campaign.units.get_mut(a).unwrap();
        au.besieging_county = 3;
        for (slot, ordered) in au.engines.iter_mut().zip(engines) {
            slot.ordered = ordered;
            slot.percent = 100;
        }
        (k, a, d)
    }

    #[test]
    fn the_castle_level_reaches_the_autocalc_and_decides_the_siege() {
        let mut won = Vec::new();
        for castle_type in 1..=5u8 {
            let (mut k, a, d) = siege_kingdom(castle_type, [2, 0, 0]);
            let assault = l2_kingdom::siege::assault(&k.counties, &mut k.campaign.units, a);
            assert!(matches!(
                assault,
                l2_kingdom::siege::Assault::Battle { castle_level, .. }
                    if castle_level == castle_type - 1
            ));
            let report = resolve_siege(&mut k, assault, Answer::Decline, 1).expect("a siege");
            assert_eq!(report.resolution, Resolution::Autocalc);
            won.push(report.verdict.attacker_won);
            if report.verdict.attacker_won {
                assert_eq!(k.counties[3].owner, 1, "the castle fell and the county with it");
                assert_eq!(k.counties[3].garrison_unit, 0);
                assert!(k.campaign.units.get(d).is_none());
                assert_eq!(k.campaign.units.get(a).unwrap().besieging_county, 0);
            } else {
                assert_eq!(k.counties[3].owner, 2, "a castle that holds keeps its county");
                assert_eq!(k.campaign.units.get(d).unwrap().besieged_by, 0, "the siege is over");
            }
        }
        assert_eq!(
            won,
            vec![true, true, false, false, false],
            "the same army takes the two smallest castles and no others"
        );
    }

    #[test]
    fn the_besiegers_engines_and_the_garrisons_oil_are_raised_and_then_gone() {
        let (k, a, d) = siege_kingdom(5, [2, 1, 1]);
        let engines = l2_kingdom::siege::prepare_besieger(k.campaign.units.get(a).unwrap());
        let oil = l2_kingdom::siege::prepare_garrison(4);
        assert_eq!(engines.counts(), [2, 1, 1, 0]);
        assert_eq!(oil.counts(), [0, 0, 0, 6], "a royal castle gets six pots");

        let at = muster_with(&k, a, engines).unwrap();
        let dt = muster_with(&k, d, oil).unwrap();
        assert!(at.iter().any(|&(t, n)| t == l2_sim::Troop::Catapults && n == 2));
        assert!(dt.iter().any(|&(t, n)| t == l2_sim::Troop::Oil && n == 6));

        assert_eq!(k.campaign.units.get(a).unwrap().troops.len(), TROOP_TYPES);
    }

    #[test]
    fn a_fought_siege_puts_a_castle_on_the_field_and_returns_a_result() {
        let (mut k, a, d) = siege_kingdom(4, [2, 2, 1]);
        k.campaign.units.get_mut(d).unwrap().owner_is_human = true;
        let assault = l2_kingdom::siege::assault(&k.counties, &mut k.campaign.units, a);
        let report =
            resolve_siege(&mut k, assault, Answer::TakeTheField, 0xB01D).expect("a siege");
        assert!(
            matches!(report.resolution, Resolution::Fought { .. } | Resolution::Stalled { .. }),
            "taking the field runs l2-sim: {:?}",
            report.resolution
        );
        for id in [a, d] {
            if let Some(u) = k.campaign.units.get(id) {
                assert_eq!(u.besieging_county, 0);
                assert_eq!(u.besieged_by, 0);
            }
        }
    }

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
}

