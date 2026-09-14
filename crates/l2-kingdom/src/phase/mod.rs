//! The turn machine — `docs/kingdom.md` §3.
//!
//! Two ordered things live here, and both are ordered *data*
//! implicit control flow, because `docs/kingdom.md` §3.4 is explicit that
//! **the order is the rule**:
//!
//! * [`Phase`] and [`TurnMachine`] — the seven per-frame phases `Turn_Tick`
//!   dispatches on, wrapping 7 back to 1.
//! * [`Pass`] and [`SEASON_PIPELINE`] — the end-of-season pipeline, as an array
//!   the season driver walks. A test can therefore assert the order directly
//! instead of inferring it from which function calls which.
//!
//! Making the pipeline an array is the whole point. Taxation reads the
//! happiness migration has not yet changed; population growth reads the
//! happiness this turn's update has already written. Those are properties of
//! the *sequence*, and a sequence buried in a 300-line function is a sequence
//! nobody can test.

mod tests_part;
pub use tests_part::*;

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
/// now, and two identical `UnitKind`s in one crate is a trap
/// separation — a phase that waits on `phase::UnitKind::Merchant` and a unit
/// that is a `unit::UnitKind::Merchant` would not have compared equal,
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
/// rule — testable
/// without a unit simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnMachine {
    /// `g_turnPhase` (`0x00569584`).
    pub phase: Phase,
    /// `g_turnPhaseStep` (`0x0053F658`), counted up on every call and reset on
    /// every phase change.
    pub step: u32,
    /// **Phase 4 has already been opened and run for this turn.** No original
    /// field: the original is in phase 4 *while the person deliberates*, so
    /// `Turn_BeginPlayersTurn` (`0x0049B6D3`) and `AI_RunTurnStep`
    /// (`0x0049A581`) both run on the frames before End Turn. Our phase machine
    /// parks between turns instead — `g_turnPhase = 1`, where the original's
    /// autosave is taken — so the interactive frames run phase 4's arm ahead of
    /// the machine and this says so, and `Kingdom::tick` then does not open the
    /// phase a second time when the wind-on reaches it.
    pub players_turn_open: bool,
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
        TurnMachine { phase: Phase::NeutralCounties, step: 0, players_turn_open: false }
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
/// **`[V]`** in that section is on the call list and its order,
/// one-line descriptions are **`[D]`/`[I]`**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pass {
    /// `Ai_ManageFarmsAll` (`0x0049A990`) — **`Season_Advance`'s first call,
    /// ahead of the clock**: `Ai_ManageCountyFarms` for every realm whose
    /// strength is non-zero and which no person drives. It is the AI's farming
    /// pass a second time in the same turn — step 5 already ran it in phase 4
    /// — and it runs before tax, rations and industry read the fields
    /// labour split. See [`crate::Kingdom::ai_manage_farms_all`].
    AiManageFarms,
    /// season, year, turn counter.
    Clock,
    /// `Event_RollAll` — random events per county.
    EventRoll,
    /// `Weather_UpdateAll` — dryness -> weather band.
    Weather,
    /// `Tax_CollectAll` — gold,
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
    /// the reason it is a pass of its own: the counts are a cache,
    /// reclamation that runs two passes later turns a finished field into
    /// fallow on the map without touching them.
    CountyRecountFields,
    /// `Fertility_Update` — fertility.
    FertilityUpdate,
    /// `Field_ReclaimTick`,
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
    /// `County_RecountMerchants` (`0x00451061`) — **which counties have a stall
    /// this season**, and therefore which of them can trade at all.
    ///
    /// Its position is `Season_Advance`'s own: immediately after
    /// `Population_UpdateAll` and before `Army_RecountCountyTroops`. Nothing
    /// later in the pipeline reads what it writes, so the *ordering* is inert
    /// today — but the pass is not, because **phase 1 of the next turn reads it
    /// as a gate.** `Ai_BuyGood` does nothing whatever for a county whose
    /// `merchantCount` is zero, so with this pass absent every unowned county
    /// in the game is permanently unable to buy food. See
    /// [`crate::merchant::recount_all`].
    CountyRecountMerchants,
    /// `Score_RankRealms` — scores and the ranking.
    ScoreRank,
    /// `Army_RecountCountyTroops` (`0x004AD6C0`) — the friendly/enemy men in
    /// every county, **and the tail that reprices the county on them**:
    ///
    /// ```c
    /// for (c = 1; c < 0x11; c++) {
    ///     Ration_Apply(c, g_seasonNext);
    ///     Grain_LabourEstimate(c, g_seasonNext);
    ///     Herd_LabourEstimate(c, g_seasonNext);
    /// }
    /// ```
    ///
    /// That tail is why [`Pass::LabourAllocateAgain`] deals against a **fresh**
    /// cattle ceiling. `Ration_ApplyAll` priced the season against the herd as
    /// it stood before `Herd_SeasonTick`; this re-prices it against the herd
    /// that survived, and `Herd_LabourEstimate` searches `herd - herdEaten`.
    /// `[V]`: a new England's nine unowned counties open the season on herd 95,
    /// eat no animal (dairy covers them), come out of the tick on 67 and here
    /// eat 13 — ceiling 386 without this pass, 323 with it, and 323 is
    /// `england-turn1.sav`'s.
    ArmyRecountTroops,
    /// `Labour_AllocateAll` a **second** time, after the population pass and
    /// the army recount have changed how many people a county holds.
    ///
    /// This one is what keeps [`crate::labour::allocate`]'s invariant true at
    /// the *end* of the season: the nine
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
    /// `Diplo_ReconcileAlliances` (`0x004A1847`) — turn phase 7, and **the last
    /// thing it does before `Turn_AdvancePhase`**.
    ///
    /// **It was implemented and never called.** `crate::diplomacy` has it,
    /// `Kingdom::reconcile_alliances` wraps it, and until this pass existed
    /// nothing in the workspace invoked either — so the `allied` matrix
    /// `ally` bytes were only ever written by the two functions that form and
    /// break an alliance, and an alliance with a realm that had just been
    /// eliminated. `tests/long_game.rs` catches
    /// that as a broken invariant, and it caught it only once
    /// `docs/decisions.md` **C134** slowed the world down enough for
    /// realm 2 to die while realm 5 was still allied to it.
    ///
    /// A producer that is complete and a consumer that is absent look identical
    /// from the producer's end (`docs/agents.md`), and this is that shape with
    /// the consumer being *the game itself*.
    ReconcileAlliances,
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
/// and the birth factor reads it.
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
///.
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
pub const SEASON_PIPELINE: [Pass; 34] = [
    // **`Season_Advance`'s first call, ahead of `Rand_Advance` and the clock**
    // (`0x00448440`: `Ai_ManageFarmsAll(); Rand_Advance(); ...`). Inserted at
    // position 0, so every later pass index moved by one — which
    // `l2_game::save` writes a season report's pass as, and which
    // `l2_kingdom::save::VERSION` 20 records.
    Pass::AiManageFarms,
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
    // **Inserted, not appended, and that shifts every later pass index** — which
    // `l2_game::save` writes a pass as. `l2_game::save::VERSION` moves with it.
    // The position is `Season_Advance`'s: `Population_UpdateAll();
    // County_RecountMerchants(); FUN_00428471(); Army_RecountCountyTroops();`.
    Pass::CountyRecountMerchants,
    Pass::ScoreRank,
    // **Inserted, and that shifts every later pass index** — both save
    // `VERSION`s move with it. The position is `Season_Advance`'s:
    // `FUN_00428471(); Army_RecountCountyTroops(); Event_ClearCountyModifiers();
    // Labour_AllocateAll();`.
    Pass::ArmyRecountTroops,
    Pass::LabourAllocateAgain,
    Pass::History,
    Pass::RationPreview,
    Pass::RefreshEstimates,
    Pass::MercenaryAdvance,
    Pass::UnitsResetMoves,
    // **Appended,** `Turn_Tick`'s
    // phase-7 arm is `Season_Advance(); Mercenary_AdvanceAll();
    // Units_ResetMoves(); Move_BuildCostMap(); Score_RankRealms();
    // Diplo_ReconcileAlliances(); Turn_AdvancePhase();` — so this really is
    // the last work of the phase. Appending also leaves every existing pass
    // index unmoved, which matters because `l2_game::save` writes a pass as
    // its position in this array.
    Pass::ReconcileAlliances,
];

/// The passes that are in `Season_Advance`'s call list, as against the ones
/// this crate runs there for want of anywhere better.
///
/// Four are in the second group: [`Pass::ScoreRank`], which has five callers
/// and none of them is `Season_Advance`,
/// belong to `Turn_Tick`'s seventh phase alongside it. See [`SEASON_PIPELINE`].
pub fn is_in_season_advance(pass: Pass) -> bool {
    !matches!(
        pass,
        Pass::ScoreRank
            | Pass::MercenaryAdvance
            | Pass::UnitsResetMoves
            | Pass::ReconcileAlliances
    )
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

