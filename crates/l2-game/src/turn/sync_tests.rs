use super::*;
use crate::engagement::{BattleReport, Resolution};
use l2_kingdom::battle::{Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::{Attack, Capture, CastleArrival, LeftCastle};
use l2_kingdom::siege::{Assault, SiegeRefusal};
use l2_kingdom::units_tick::{Contact, Encounter};
use l2_net::{Canonical, Reader};

fn report(n: i32) -> BattleReport {
    let capture = Capture {
        new_owner: 1,
        old_owner: 2,
        county: 3,
        held_before: 4,
        peak_before: 5,
        governable: true,
        penalty: -n,
        garrison: Some(LeftCastle::Marched { tile: (7, 8), sortie: Some(9) }),
    };
    BattleReport {
        settlement: Settlement::Prompt,
        resolution: Resolution::Fought { ticks: 400 + n as u32, cause: l2_sim::End::BrokeIn },
        verdict: Verdict { attacker: 3, defender: 4, attacker_won: true, survival_percent: Some(n) },
        aftermath: Aftermath {
            county_taken_by: Some(2),
            loser_destroyed: true,
            loser_siege_lifted: true,
            offence: Some((1, 2)),
            winner_moves_used: 8,
            withdrawal_casualties: Some(11),
            loser_owner: 2,
            captures: [Some(capture), None],
        },
        defenders_returned: 6,
        attacker_men: (100, 60),
        defender_men: (80, 0),
        county: 5,
        is_siege: true,
        attacker_owner: 1,
        defender_owner: 2,
        attacker_roster: ([1; 7], [2; 7]),
        defender_roster: ([3; 7], [n; 7]),
        castle_damage: l2_sim::CastleDamage {
            moat_filled: 1,
            wall_damage: 2,
            breach_score: 3,
            approach_score: 4,
            ramparts_breached: 5,
            gate_open: true,
        },
    }
}

/// Ablation: any field dropped from `encode` or read out of order in `decode`.
#[test]
fn a_turn_in_flight_round_trips_every_field() {
    let meet = Encounter { mover: 4, occupant: 9, county: 6 };
    let mut season = l2_kingdom::SeasonReport::new();
    season.revolts = vec![3];
    let question = Question {
        attacker: 4,
        defender: 9,
        county: 6,
        is_siege: true,
        attacker_owner: 1,
        defender_owner: 2,
        attacker_men: 120,
        defender_men: 80,
        attacker_roster: [5; 7],
        defender_roster: [6; 7],
        choice_owner: 1,
        castle_level: Some(3),
    };
    let progress = TurnProgress {
        idle: true,
        opening: true,
        ticks: 77,
        steps: 12,
        contacts: vec![
            Contact::Battle(meet),
            Contact::Castle { unit: 2, county: 3, outcome: Attack::Captured(report(1).aftermath.captures[0].unwrap()) },
            Contact::CastleBuilding { unit: 2, county: 3, arrival: CastleArrival::Siege(Err(SiegeRefusal::CastleRuined)) },
            Contact::Merged { mover: 1, into: 2 },
            Contact::Blocked { mover: 3, occupant: 4 },
        ],
        pending_battles: vec![meet],
        granted: true,
        stage: Stage::Tail,
        siege: None,
        question: Some(question),
        pending_assault: Some(Assault::Battle { attacker: 4, defender: 9, castle_level: 2 }),
        tail: Some(Tail { contacts: vec![Contact::Battle(meet)], report: Some(season), battle: Some(meet) }),
        reports: vec![report(1), report(2)],
        unseen: Some(report(3)),
    };
    let bytes = {
        let mut out = Canonical::recording();
        sync::encode(&mut out, &progress);
        out.finish().bytes.unwrap()
    };
    let mut r = Reader::new(&bytes);
    assert_eq!(sync::decode(&mut r).unwrap(), progress);
    r.finish().unwrap();
}
