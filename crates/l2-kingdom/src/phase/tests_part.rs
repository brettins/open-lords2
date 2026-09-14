#![allow(unused_imports)]
use super::*;

use crate::unit::UnitKind;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Commodity;

    #[test]
    fn the_phases_are_numbered_one_to_seven_and_wrap() {
        for (i, p) in PHASE_ORDER.iter().enumerate() {
            assert_eq!(p.index() as usize, i + 1);
            assert_eq!(Phase::from_index(p.index()), Some(*p));
        }
        assert_eq!(Phase::from_index(0), None);
        assert_eq!(Phase::from_index(8), None);
        assert_eq!(Phase::SeasonEnd.next(), Phase::NeutralCounties, "7 wraps to 1");
    }

    /// The cycle, walked with everything settling immediately. Seven phases,
    /// and back to where it started.
    #[test]
    fn a_full_cycle_visits_every_phase_in_order_exactly_once() {
        let mut m = TurnMachine::new();
        let mut visited = Vec::new();
        for _ in 0..64 {
            let t = m.tick(true);
            if t.started {
                visited.push(t.phase);
            }
            if visited.len() == PHASE_ORDER.len() + 1 {
                break;
            }
        }
        assert_eq!(&visited[..7], &PHASE_ORDER[..]);
        assert_eq!(visited[7], Phase::NeutralCounties, "and round again");
    }

    #[test]
    fn phase_one_takes_exactly_three_steps_regardless_of_anything_settling() {
        let mut m = TurnMachine::new();
        assert_eq!(m.tick(false).advanced_to, None);
        assert_eq!(m.tick(false).advanced_to, None);
        assert_eq!(m.tick(false).advanced_to, Some(Phase::ArmyMovement));
        assert_eq!(m.step, 0, "the step counter resets on a phase change");
    }

    #[test]
    fn a_unit_phase_starts_its_work_before_it_can_end() {
        let mut m = TurnMachine { phase: Phase::ArmyMovement, step: 0 };
        // Even with the units already settled, the first call is the one that
        // kicks off the work.
        let first = m.tick(true);
        assert!(first.started);
        assert_eq!(first.advanced_to, None, "must not skip its own handler");
        assert_eq!(m.tick(true).advanced_to, Some(Phase::SupplyTransports));
    }

    #[test]
    fn a_unit_phase_waits_indefinitely_while_units_are_moving() {
        let mut m = TurnMachine { phase: Phase::Merchants, step: 0 };
        for _ in 0..1000 {
            assert_eq!(m.tick(false).advanced_to, None);
        }
        assert_eq!(m.tick(true).advanced_to, Some(Phase::SeasonEnd));
    }

    #[test]
    fn the_end_of_season_phase_runs_once_and_goes_straight_to_phase_one() {
        let mut m = TurnMachine { phase: Phase::SeasonEnd, step: 0 };
        let t = m.tick(false);
        assert!(t.started);
        assert_eq!(t.advanced_to, Some(Phase::NeutralCounties));
    }

    #[test]
    fn every_phase_waits_on_something_and_only_one_waits_on_a_step_count() {
        let step_waiters: Vec<Phase> = PHASE_ORDER
            .into_iter()
            .filter(|p| matches!(p.wait(), PhaseWait::Steps(_)))
            .collect();
        assert_eq!(step_waiters, vec![Phase::NeutralCounties]);
        assert_eq!(Phase::PlayersTurn.wait(), PhaseWait::AllRealmsDone);
        assert_eq!(Phase::Merchants.wait(), PhaseWait::Units(UnitKind::Merchant));
    }

    /// The four ordering dependencies the economy silently relies on.
    /// `docs/kingdom.md` §3.4: **the order is the rule**.
    #[test]
    fn the_pipeline_order_encodes_every_dependency_the_formulas_need() {
        let before = |a: Pass, b: Pass| a.order() < b.order();
        assert!(before(Pass::RationApply, Pass::HealthUpdate), "health reads the ration level");
        assert!(before(Pass::HealthUpdate, Pass::HappinessUpdate), "happiness reads the new band");
        assert!(before(Pass::TaxCollect, Pass::HappinessUpdate), "happiness sums the tax term");
        assert!(before(Pass::HappinessUpdate, Pass::MigrationUpdate));
        assert!(before(Pass::HappinessUpdate, Pass::PopulationUpdate));
        assert!(before(Pass::MigrationUpdate, Pass::PopulationUpdate), "pop applies the flows");
        assert!(before(Pass::EventRoll, Pass::PopulationUpdate), "events modify births");
        assert!(before(Pass::EventRoll, Pass::GrainSeasonTick), "... and the grain store");
        assert!(before(Pass::EventRoll, Pass::HerdSeasonTick), "... and the herd");
        assert!(before(Pass::Weather, Pass::GrainSeasonTick), "crops scale by this season's band");
        assert!(before(Pass::Weather, Pass::HerdSeasonTick));
        assert!(before(Pass::FertilityUpdate, Pass::FieldReclaim), "fertility, *then* reclamation");
        assert!(before(Pass::PopulationUpdate, Pass::ScoreRank), "the score is scored last");
        assert!(before(Pass::RationApply, Pass::RationPreview), "and the preview really is second");
    }

    /// **The blacksmith runs before the mines.** The obvious reading — mine the
    /// ore, then forge it — is wrong,
    /// runs weapons over every county first, so a season's weapons are paid for
    /// out of the *previous* season's ore.
    #[test]
    fn the_industry_passes_run_in_the_order_the_driver_calls_them() {
        let runs: Vec<Commodity> = SEASON_PIPELINE
            .into_iter()
            .filter_map(|p| match p {
                Pass::Industry(c) => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(runs, crate::tables::INDUSTRY_ORDER.to_vec());
        assert_eq!(runs[0], Commodity::Weapons);
        let order = |c: Commodity| runs.iter().position(|x| *x == c).unwrap();
        assert!(order(Commodity::Weapons) < order(Commodity::Iron), "forge, then mine");
        assert!(order(Commodity::Weapons) < order(Commodity::Wood));
    }

    /// **Four passes here are not `Season_Advance`'s calls**, whatever
    /// `docs/kingdom.md` §3.4 says of the first: `Score_RankRealms` has five
    /// callers and none of them is the season,
    /// belong to `Turn_Tick`'s seventh phase alongside it. All four are kept
    /// in the pipeline because the work has to happen somewhere, and flagged so
    /// nobody reads the array as a transcription.
    #[test]
    fn four_passes_here_are_not_things_season_advance_calls() {
        let extra: Vec<Pass> =
            SEASON_PIPELINE.into_iter().filter(|p| !is_in_season_advance(*p)).collect();
        assert_eq!(
            extra,
            vec![
                Pass::ScoreRank,
                Pass::MercenaryAdvance,
                Pass::UnitsResetMoves,
                Pass::ReconcileAlliances
            ]
        );
        assert!(is_in_season_advance(Pass::History), "the ring is the real second-to-last");
        assert!(is_in_season_advance(Pass::RationPreview), "and the preview really is last");
    }

    /// The correction to `docs/armies.md` §2.1, as an ordering assertion: phase
    /// 7 walks the mercenaries **before** it gives the armies their moves back.
    #[test]
    fn the_mercenaries_walk_before_the_armies_get_their_moves_back() {
        assert!(Pass::MercenaryAdvance.order() < Pass::UnitsResetMoves.order());
        // …and both are after everything the season itself does, because the
        // season runs inside the same phase.
        assert!(Pass::RationPreview.order() < Pass::MercenaryAdvance.order());
    }

    /// **The clock is not first, and this test said it was.** `Season_Advance`
    /// (`0x00448440`) opens `Ai_ManageFarmsAll(); Rand_Advance();` and only then
    /// reads `g_seasonNext` — so the AI farms the season that is *ending*, and a
    /// Winter re-sow happens on the Winter turn. The assertion used to be
    /// `SEASON_PIPELINE[0] == Pass::Clock`, which described our array
    /// the function.
    #[test]
    fn the_pipeline_holds_no_duplicates_and_farms_before_the_clock() {
        assert_eq!(SEASON_PIPELINE[0], Pass::AiManageFarms);
        assert_eq!(SEASON_PIPELINE[1], Pass::Clock);
        assert!(is_in_season_advance(Pass::AiManageFarms), "it is the function's first call");
        for (i, p) in SEASON_PIPELINE.iter().enumerate() {
            assert_eq!(p.order(), i, "{p:?} should be unique and at {i}");
        }
    }

    #[test]
    fn industry_runs_once_per_commodity() {
        let runs: Vec<Pass> =
            SEASON_PIPELINE.into_iter().filter(|p| matches!(p, Pass::Industry(_))).collect();
        assert_eq!(runs.len(), 4);
        for c in Commodity::ALL {
            assert!(runs.contains(&Pass::Industry(c)), "{c:?} should have a run");
        }
    }
}

