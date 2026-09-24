#![allow(unused_imports)]
use super::*;
use super::conquest_part::*;
use super::realm::*;
use l2_formats::save::Save;
use l2_game::engagement::{self, Answer, Resolution};
use l2_kingdom::conquest::{self, Attack};
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM};
use l2_kingdom::unit::{TroopType, Unit, UnitKind, TROOP_TYPES};

/// The player declined the field — which is the autocalc, `FUN_0043B622` — and
/// lost. This runs the same path and compares every number the last save holds.
#[test]
fn the_battle_runs_from_the_campaign_and_lands_on_the_saved_aftermath() {
    let before_save: Save = l2_testkit::fixture!("battle-before.sav");
    let (mut k, attacker) = before(&before_save);
    let after: Save = l2_testkit::fixture!("battle-after.sav");

    let outcome = {
        let restore = k.restore();
        let Kingdom { counties, realms, campaign, options, tables, year, .. } = &mut k;
        conquest::attack_county(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            attacker,
            COUNTY,
            options.difficulty,
            *year,
            &mut campaign.explored,
            restore,
        )
    };
    let Attack::Battle { defender, .. } = outcome else { panic!("{outcome:?}") };

    let report = engagement::resolve(
        &mut k,
        outcome,
        COUNTY,
        Answer::Decline,
        1,
    )
    .expect("a battle");

    assert_eq!(report.resolution, Resolution::Autocalc);

    assert!(!report.verdict.attacker_won, "the save has both armies gone and the county neutral");
    assert_eq!(report.verdict.winner(), defender);

    assert!(unit_at(&after, ATTACKER_SLOT).is_none(), "the fixture's slot 5 is empty");
    assert!(unit_at(&after, DEFENCE_SLOT).is_none(), "and so is slot 6");
    assert!(k.campaign.units.get(attacker).is_none(), "the attacker was destroyed as the loser");
    assert!(k.campaign.units.get(defender).is_none(), "the defence was dissolved as the winner");

    let after_county = after.county(COUNTY as usize).unwrap();
    assert_eq!(after_county.owner, 0, "the fixture's county 3 is still neutral");
    assert_eq!(report.aftermath.county_taken_by, None);
    assert_eq!(k.counties[COUNTY as usize].owner, 0);

    assert_eq!(report.defenders_returned, 36);
    assert_eq!(
        k.counties[COUNTY as usize].population,
        after_county.population,
        "county 3's population after the battle"
    );
    assert_eq!(after_county.population, 582);
    assert_eq!(report.defender_men, (182, 36));
    assert_eq!(report.attacker_men, (178, 0));
}

/// **The same position, taken to the field.** The save's verdict is the
/// *autocalc*'s — `FUN_0043B622`, which the player reached by declining — and
/// it weighs troop types where a fought battle counts men, so it is no
/// yardstick for a fought one: Q2, settled 2026-09-17, and the floor this test
/// held on the militia went with it. What is left is the seam itself — every
/// seed runs to a [`Resolution::Fought`] and comes back with the numbers it
/// went in with.
#[test]
fn the_same_position_can_be_fought_for_real_and_still_comes_back() {
    let before_save: Save = l2_testkit::fixture!("battle-before.sav");
    let fight = |seed: u64| {
        let (mut k, attacker) = before(&before_save);
        let outcome = {
            let restore = k.restore();
            let Kingdom { counties, realms, campaign, options, tables, year, .. } = &mut k;
            conquest::attack_county(
                tables,
                &campaign.map,
                counties,
                realms,
                &mut campaign.units,
                &mut campaign.names,
                attacker,
                COUNTY,
                options.difficulty,
                *year,
                &mut campaign.explored,
                restore,
            )
        };
        let Attack::Battle { defender, .. } = outcome else { panic!("{outcome:?}") };
        let levied = k.campaign.units.get(defender).unwrap().men;
        let after_levy = k.counties[COUNTY as usize].population;
        let report = engagement::resolve(&mut k, outcome, COUNTY, Answer::TakeTheField, seed)
            .expect("a battle");
        (k, defender, levied, after_levy, report)
    };

    const SEEDS: u64 = 8;
    let fights: Vec<_> = (0..SEEDS)
        .map(|i| fight(l2_sim::runner::DEFAULT_SEED.wrapping_add(i.wrapping_mul(0x9E37_79B9))))
        .collect();
    let held = fights.iter().filter(|(_, _, _, _, r)| !r.verdict.attacker_won).count() as u64;
    eprintln!("the militia held {held} of {SEEDS} fights");

    for (k, defender, levied, after_levy, report) in &fights {
        let (levied, after_levy) = (*levied, *after_levy);
        let Resolution::Fought { ticks, cause } = report.resolution else {
            panic!("taking the field must fight it to a conclusion: {:?}", report.resolution)
        };
        assert!(ticks > 0 && ticks < engagement::MAX_TICKS, "{ticks} ticks");
        assert_eq!(cause, l2_sim::End::Annihilation, "the only way a field battle ends by itself");

        assert_eq!(report.attacker_men.0, 178);
        assert_eq!(report.defender_men.0, levied);
        assert!(report.attacker_men.1 <= 178 && report.defender_men.1 <= levied);
        let (won, lost) = match report.verdict.attacker_won {
            true => (report.attacker_men, report.defender_men),
            false => (report.defender_men, report.attacker_men),
        };
        assert_eq!(lost.1, 0, "annihilation leaves the loser none");
        // [I] no oracle bound. Melee_Tick 0x00494908 run inside 0x004831D8, one man a tick,
        // leaves the winner 28-94 of 178 over the 8 seeds (battle-4; 20-83 before).
        assert!(won.1 > 0 && won.1 * 3 < won.0 * 2, "an even fight costs the winner a third: {won:?}");
        assert_eq!(report.verdict.winner() != *defender, report.verdict.attacker_won);

        assert!(k.campaign.units.get(report.verdict.loser()).is_none(), "the loser is gone");
        assert_eq!(
            k.counties[COUNTY as usize].population,
            after_levy + report.defenders_returned,
            "the militia that lived walks back into county 3"
        );
        eprintln!(
            "fought: {ticks} ticks, attacker {:?}, defender {:?}, {} held the field",
            report.attacker_men,
            report.defender_men,
            if report.verdict.attacker_won { "the player" } else { "the militia" }
        );
    }
}

