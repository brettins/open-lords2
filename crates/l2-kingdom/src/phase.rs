//! The turn machine — `docs/kingdom.md` §3.
//!
//! Two ordered things live here, and both are ordered *data* rather than
//! implicit control flow, because `docs/kingdom.md` §3.4 is explicit that
//! **the order is the rule**:
//!
//! * [`Phase`] and [`TurnMachine`] — the seven per-frame phases `Turn_Tick`
//!   dispatches on, wrapping 7 back to 1.
//! * [`Pass`] and [`SEASON_PIPELINE`] — the end-of-season pipeline, as an array
//!   the season driver walks. A test can therefore assert the order directly
//!   instead of inferring it from which function calls which.
//!
//! Making the pipeline an array is the whole point. Taxation reads the
//! happiness migration has not yet changed; population growth reads the
//! happiness this turn's update has already written. Those are properties of
//! the *sequence*, and a sequence buried in a 300-line function is a sequence
//! nobody can test.

/// The seven phases `Turn_Tick` (`0x0049A010`) dispatches on `g_turnPhase`.
/// `docs/kingdom.md` §3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Phase {
    /// Realm 0 — the unowned counties — get their tax rates and fields set.
    NeutralCounties = 1,
    /// **Sieges** — validate, build, assault. *Not* army movement.
    ///
    /// The variant keeps its name because five files spell it and renaming it
    /// is a separate change; what it does has been corrected. See
    /// [`Phase::wait`] and `crate::units_tick`.
    ArmyMovement = 2,
    /// Supply transports re-target their **cargo** county's anchor and walk.
    /// Not their `dest_county`, which is where the current path ends.
    SupplyTransports = 3,
    /// The players' turn. `AI_RunTurnStep` drives the AI realms; the human
    /// realm is driven by the UI. Also where the turn timer runs.
    PlayersTurn = 4,
    /// Revolting peasants move.
    PeasantMobs = 5,
    /// Merchants — already documented in `docs/formats/plane4.md` §2.3.
    Merchants = 6,
    /// End of season. Runs once and advances straight to phase 1.
    SeasonEnd = 7,
}

/// The phases in dispatch order. `Turn_AdvancePhase` (`0x0049CE51`) walks this
/// and wraps.
pub const PHASE_ORDER: [Phase; 7] = [
    Phase::NeutralCounties,
    Phase::ArmyMovement,
    Phase::SupplyTransports,
    Phase::PlayersTurn,
    Phase::PeasantMobs,
    Phase::Merchants,
    Phase::SeasonEnd,
];

/// The campaign unit types phases 2, 3, 5 and 6 wait on.
///
/// **This used to be a second enum with the same name and the same four
/// values.** It was declared here while `crate::unit` did not exist; it does
/// now, and two identical `UnitKind`s in one crate is a trap rather than a
/// separation — a phase that waits on `phase::UnitKind::Merchant` and a unit
/// that is a `unit::UnitKind::Merchant` would not have compared equal, and the
/// compiler would have said nothing useful about why. There is one, and it is
/// the unit record's own.
pub use crate::unit::UnitKind;

/// What a phase waits for before `Turn_AdvancePhase` moves on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PhaseWait {
    /// A fixed number of calls. Phase 1 is the only one, and it waits 3.
    Steps(u32),
    /// Every unit of this type has stopped moving.
    Units(UnitKind),
    /// Every realm's `aiStep` is [`crate::realm::AI_STEP_DONE`].
    AllRealmsDone,
    /// **The siege cursor sweep has come up empty** — phase 2, and only phase
    /// 2.
    ///
    /// `Siege_TickPhase` (`0x004A84BA`) walks `g_siegeCursor` from 1 to 150
    /// calling `Siege_BuildTick` and returns as soon as one army's engines are
    /// ready, *leaving the cursor where it is*; `Turn_Tick` then runs
    /// `Siege_LaunchAssault` and comes back to the same cursor next call.
    /// Nothing left to build is a return of 0, and that is what advances the
    /// phase.
    ///
    /// With no sieges in play — which is every game this crate can currently
    /// produce — the first sweep is empty and the phase advances immediately.
    Sieges,
    /// Runs once and advances on the same call.
    Immediate,
}

impl Phase {
    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Option<Phase> {
        PHASE_ORDER.get(i.checked_sub(1)? as usize).copied()
    }

    /// The next phase, wrapping 7 -> 1.
    pub fn next(self) -> Phase {
        match self {
            Phase::SeasonEnd => Phase::NeutralCounties,
            other => Phase::from_index(other.index() + 1).expect("phases 1..6 all have a successor"),
        }
    }

    /// What has to be true before `Turn_AdvancePhase` fires.
    ///
    /// > **Phase 2 does not wait on armies, and this said it did.** The row was
    /// > taken from `docs/kingdom.md` §3.1, which derives every unit phase from
    /// > "the type each phase waits on" — a derivation that is right for 3, 5
    /// > and 6 and wrong for 2. `Turn_Tick`'s phase-2 arm is
    /// >
    /// > ```c
    /// > if (g_turnPhaseStep % 100 == 2) {
    /// >     if (Siege_TickPhase() == 0) Turn_AdvancePhase();
    /// >     else { DAT_0055403C = 0; Siege_LaunchAssault(g_siegeCursor); }
    /// > }
    /// > ```
    /// >
    /// > — no unit sweep, no type-1 predicate, nothing that reads `moving`. The
    /// > three phases that *do* wait on units call `FUN_004A4F5B(type)` (3 and
    /// > 6) or `FUN_004A4E3D(2, 6)` (5), and phase 2 calls neither. Armies move
    /// > outside the phase machine entirely — `crate::units_tick` has the
    /// > reading. `docs/decisions.md` C35, and `docs/kingdom.md` §3.1's phase-2
    /// > row is corrected with it. `[V]`
    pub fn wait(self) -> PhaseWait {
        match self {
            Phase::NeutralCounties => PhaseWait::Steps(3),
            Phase::ArmyMovement => PhaseWait::Sieges,
            Phase::SupplyTransports => PhaseWait::Units(UnitKind::Transport),
            Phase::PlayersTurn => PhaseWait::AllRealmsDone,
            Phase::PeasantMobs => PhaseWait::Units(UnitKind::PeasantMob),
            Phase::Merchants => PhaseWait::Units(UnitKind::Merchant),
            Phase::SeasonEnd => PhaseWait::Immediate,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Phase::NeutralCounties => "Neutral counties",
            Phase::ArmyMovement => "Army movement",
            Phase::SupplyTransports => "Supply transports",
            Phase::PlayersTurn => "Players' turn",
            Phase::PeasantMobs => "Revolting peasants",
            Phase::Merchants => "Merchants",
            Phase::SeasonEnd => "End of season",
        }
    }
}

/// What one call to [`TurnMachine::tick`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseTick {
    /// The phase that was current for this call.
    pub phase: Phase,
    /// True on the first call after entering a phase — the call on which the
    /// phase "kicks off work".
    pub started: bool,
    /// Set when this call ended the phase, naming the phase now current.
    pub advanced_to: Option<Phase>,
}

/// `g_turnPhase` and `g_turnPhaseStep`, as a machine.
///
/// The machine deliberately does **not** own the units it waits on. `Turn_Tick`
/// waits for movement to stop, and movement is not this crate's business; the
/// caller passes `settled` and the machine decides. That keeps the ordering
/// rule — the part `docs/kingdom.md` §3.1 actually establishes — testable
/// without a unit simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnMachine {
    /// `g_turnPhase` (`0x00569584`).
    pub phase: Phase,
    /// `g_turnPhaseStep` (`0x0053F658`), counted up on every call and reset on
    /// every phase change.
    pub step: u32,
}

impl Default for TurnMachine {
    fn default() -> Self {
        TurnMachine::new()
    }
}

impl TurnMachine {
    /// A machine sitting at the start of phase 1 — which is where a freshly
    /// loaded `lastturn.sav` sits: `g_turnPhase = 1` (`docs/kingdom.md` §9).
    pub fn new() -> TurnMachine {
        TurnMachine { phase: Phase::NeutralCounties, step: 0 }
    }

    /// One call of `Turn_Tick`.
    ///
    /// `settled` answers the current phase's [`PhaseWait`]: "every unit of the
    /// type this phase started has stopped moving", or "every realm has
    /// finished its turn". It is ignored by [`PhaseWait::Steps`] and
    /// [`PhaseWait::Immediate`], which do not wait on anything external.
    pub fn tick(&mut self, settled: bool) -> PhaseTick {
        let phase = self.phase;
        let started = self.step == 0;
        let step = self.step;
        self.step += 1;

        let done = match phase.wait() {
            PhaseWait::Steps(n) => step + 1 >= n,
            // Work is kicked off on the first call, so the earliest a phase can
            // end is the call after it started. Without this a phase whose
            // units happen to be idle already would be skipped before its
            // handler ever ran.
            PhaseWait::Units(_) | PhaseWait::AllRealmsDone | PhaseWait::Sieges => {
                settled && !started
            }
            PhaseWait::Immediate => true,
        };

        if done {
            self.phase = phase.next();
            self.step = 0;
            PhaseTick { phase, started, advanced_to: Some(self.phase) }
        } else {
            PhaseTick { phase, started, advanced_to: None }
        }
    }
}

/// One pass of the end-of-season pipeline.
///
/// `Season_Advance` (`0x00448440`) calls 29 functions in a fixed order.
/// `docs/kingdom.md` §3.4 identifies the ones below and abridges the rest; the
/// **`[V]`** in that section is on the call list and its order, and the
/// one-line descriptions are **`[D]`/`[I]`**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pass {
    /// season, year, turn counter.
    Clock,
    /// `Event_RollAll` — random events per county.
    EventRoll,
    /// `Weather_UpdateAll` — dryness -> weather band.
    Weather,
    /// `Tax_CollectAll` — gold, and the tax happiness term.
    TaxCollect,
    /// `Wages_PayAll` — army wages, bankruptcy.
    WagesPay,
    /// The food demand recomputation — `Ration_Apply` per county.
    RationApply,
    /// `Health_UpdateAll` — health meter and band.
    HealthUpdate,
    /// `Happiness_UpdateAll` — sum the four terms.
    HappinessUpdate,
    /// `Unrest_UpdateAll` — revolt counter.
    UnrestUpdate,
    /// `Realm_SecedeIsolatedCounties` — every realm keeps only its most
    /// populous contiguous block of counties; everything cut off declares
    /// independence. `docs/kingdom.md` §6.1, [`crate::territory`].
    SecedeIsolatedCounties,
    /// `County_RecountFieldsAll` (`FUN_00469B51`) — every county's five field
    /// counts, rebuilt from the map. **The first of the estimate inputs**, and
    /// the reason it is a pass of its own: the counts are a cache, and the
    /// reclamation that runs two passes later turns a finished field into
    /// fallow on the map without touching them.
    CountyRecountFields,
    /// `Fertility_Update` — fertility.
    FertilityUpdate,
    /// `Field_ReclaimTick`, and the `Field_ReclaimEstimate` that is its tail
    /// call. Together with [`Pass::FertilityUpdate`] this is
    /// `Fields_SeasonTick` (`0x0044BF7A`).
    FieldReclaim,
    /// `Grain_SeasonTick` — sow / grow / harvest.
    GrainSeasonTick,
    /// `Herd_SeasonTick` — livestock.
    HerdSeasonTick,
    /// One of `Industry_Produce`'s four runs.
    Industry(crate::tables::Commodity),
    /// `Castle_BuildTick` — castle construction, and `Castle_BuildEstimate`
    /// behind it.
    CastleBuildTick,
    /// `Labour_AllocateAll` (`0x0044F699`) — **reassign every peasant, from
    /// scratch**, the first of the season's two.
    ///
    /// It sits here because everything above it has just moved a ceiling: the
    /// field recount, reclamation, the grain and herd ticks, the four
    /// industries and the castle each refresh their own estimate as their last
    /// act, and this is the first pass that reads them. `docs/kingdom.md` §14.
    LabourAllocate,
    /// `Migration_UpdateAll` — emigrants and immigrants.
    MigrationUpdate,
    /// `Population_UpdateAll` — births, deaths, new population.
    PopulationUpdate,
    /// `Score_RankRealms` — scores and the ranking.
    ScoreRank,
    /// `Labour_AllocateAll` a **second** time, after the population pass and
    /// the army recount have changed how many people a county holds.
    ///
    /// This one is what keeps [`crate::labour::allocate`]'s invariant true at
    /// the *end* of the season rather than only in the middle of it: the nine
    /// job records sum to the population, exactly, every season. A pipeline
    /// with only the first allocation leaves every newborn in no job at all.
    LabourAllocateAgain,
    /// The history ring.
    History,
    /// `Ration_Apply` again, as next season's preview.
    ///
    /// This second call is why `docs/kingdom.md` §4.3's food-split fields do
    /// not reproduce for player-owned counties: the stored `rationAchieved` is
    /// **the next season's** level, not the one that was applied.
    RationPreview,
    /// `Panels_RefreshAll`'s middle statement — `County_RefreshEstimates` for
    /// every county, on `g_seasonNext`.
    ///
    /// **This is the pass that made wiring the allocator possible.**
    /// `Season_Advance` never calls `County_RefreshEstimates` directly, and for
    /// a long time this crate read that as "it does not run in the season". It
    /// does: `Panels_RefreshAll` is `Season_Advance`'s *last* call and this is
    /// the middle of its three. `docs/kingdom.md` §3.4.
    RefreshEstimates,
    /// `Mercenary_AdvanceAll` — every band walks one county and may offer
    /// itself. **Turn phase 7, not `Season_Advance`**, and it runs *before*
    /// [`Pass::UnitsResetMoves`]. See [`is_in_season_advance`].
    MercenaryAdvance,
    /// `Units_ResetMoves` — every unit's move counter back to zero. Turn phase
    /// 7, immediately after the mercenary walk.
    UnitsResetMoves,
}

/// The end-of-season pipeline, in order.
///
/// The order encodes four dependencies that the formulas silently rely on and
/// that [`crate::Kingdom`]'s tests assert directly:
///
/// 1. [`Pass::RationApply`] before [`Pass::HealthUpdate`] — the health delta is
///    indexed by the ration level achieved *this* season.
/// 2. [`Pass::HealthUpdate`] before [`Pass::HappinessUpdate`] — the happiness
///    health term reads the *new* band. `docs/kingdom.md` §9's chain
///    `65 -> band 2 -> +2 -> 67 -> band 3` then `shownHealth = +1` only works
///    this way round.
/// 3. [`Pass::HappinessUpdate`] before [`Pass::MigrationUpdate`] and
///    [`Pass::PopulationUpdate`] — migration compares this season's happiness,
///    and the birth factor reads it.
/// 4. [`Pass::MigrationUpdate`] before [`Pass::PopulationUpdate`] — population
///    applies `pop -= emigrants; pop += immigrants` at the end of its own pass.
///
/// **Two corrections against `docs/kingdom.md` §3.4, both from the call list
/// itself.**
///
/// 1. **The industry order is weapons, iron, stone, wood** — the driver
///    (`FUN_0044E852`) runs the blacksmith over every county in its own loop
///    and only then mines per county. §3.4 says *"wood, iron, stone,
///    weapons"*; §7.4's table indexes them wood, iron, weapons, stone; the
///    binary is neither, and it is [`crate::tables::INDUSTRY_ORDER`]. It
///    matters: the blacksmith spends the *previous* season's ore, because this
///    season's has not been mined yet.
/// 2. **`Score_RankRealms` is not in the pipeline at all.** §3.4 lists it;
///    `Season_Advance`'s twenty-eight calls do not include it. Its callers are
///    `Turn_Tick`, `Game_NewGame`, `Turn_AdvancePhase`, the AI turn's step 0
///    ([`crate::ai::begin_realm_turn`]) and one UI path. [`Pass::ScoreRank`] is
///    kept, because the ranking still has to happen somewhere and a kingdom
///    with no turn machine driving it would otherwise never rank at all; it is
///    flagged by [`is_in_season_advance`] and asserted in this module's tests
///    rather than quietly presented as the original's order.
/// 3. **The two campaign passes at the end are phase 7's, not
///    `Season_Advance`'s.** `Turn_Tick`'s seventh phase runs
///    `Mercenary_AdvanceAll(); Units_ResetMoves(); Move_BuildCostMap();` and
///    *then* the season. They are carried here, after
///    [`Pass::RationPreview`], for the same reason [`Pass::ScoreRank`] is: the
///    work still has to happen somewhere and a pipeline that omitted it would
///    leave every army permanently out of moves. [`is_in_season_advance`]
///    keeps them distinguishable. `Move_BuildCostMap` is *not* a pass —
///    [`crate::map::CampaignMap::cost_map`] is recomputed on demand, which is
///    what the original does on every move order anyway.
///
///    **`docs/armies.md` §2.1 has these two the wrong way round**, giving
///    `Units_ResetMoves` first. Corrected there.
pub const SEASON_PIPELINE: [Pass; 30] = [
    Pass::Clock,
    Pass::EventRoll,
    Pass::Weather,
    Pass::TaxCollect,
    Pass::WagesPay,
    Pass::RationApply,
    Pass::HealthUpdate,
    Pass::HappinessUpdate,
    Pass::UnrestUpdate,
    Pass::SecedeIsolatedCounties,
    Pass::CountyRecountFields,
    Pass::FertilityUpdate,
    Pass::FieldReclaim,
    Pass::GrainSeasonTick,
    Pass::HerdSeasonTick,
    Pass::Industry(crate::tables::Commodity::Weapons),
    Pass::Industry(crate::tables::Commodity::Iron),
    Pass::Industry(crate::tables::Commodity::Stone),
    Pass::Industry(crate::tables::Commodity::Wood),
    Pass::CastleBuildTick,
    Pass::LabourAllocate,
    Pass::MigrationUpdate,
    Pass::PopulationUpdate,
    Pass::ScoreRank,
    Pass::LabourAllocateAgain,
    Pass::History,
    Pass::RationPreview,
    Pass::RefreshEstimates,
    Pass::MercenaryAdvance,
    Pass::UnitsResetMoves,
];

/// The passes that are in `Season_Advance`'s call list, as against the ones
/// this crate runs there for want of anywhere better.
///
/// Three are in the second group: [`Pass::ScoreRank`], which has five callers
/// and none of them is `Season_Advance`, and the two campaign passes, which
/// belong to `Turn_Tick`'s seventh phase alongside it. See [`SEASON_PIPELINE`].
pub fn is_in_season_advance(pass: Pass) -> bool {
    !matches!(pass, Pass::ScoreRank | Pass::MercenaryAdvance | Pass::UnitsResetMoves)
}

impl Pass {
    /// The position of a pass in [`SEASON_PIPELINE`], for ordering assertions.
    pub fn order(self) -> usize {
        SEASON_PIPELINE
            .iter()
            .position(|p| *p == self)
            .expect("every Pass appears in SEASON_PIPELINE")
    }
}

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
    /// ore, then forge it — is wrong, and the pipeline now says so: the driver
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

    /// **Three passes here are not `Season_Advance`'s calls**, whatever
    /// `docs/kingdom.md` §3.4 says of the first: `Score_RankRealms` has five
    /// callers and none of them is the season, and the two campaign passes
    /// belong to `Turn_Tick`'s seventh phase alongside it. All three are kept
    /// in the pipeline because the work has to happen somewhere, and flagged so
    /// nobody reads the array as a transcription.
    #[test]
    fn three_passes_here_are_not_things_season_advance_calls() {
        let extra: Vec<Pass> =
            SEASON_PIPELINE.into_iter().filter(|p| !is_in_season_advance(*p)).collect();
        assert_eq!(extra, vec![Pass::ScoreRank, Pass::MercenaryAdvance, Pass::UnitsResetMoves]);
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

    #[test]
    fn the_pipeline_holds_no_duplicates_and_starts_with_the_clock() {
        assert_eq!(SEASON_PIPELINE[0], Pass::Clock);
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
