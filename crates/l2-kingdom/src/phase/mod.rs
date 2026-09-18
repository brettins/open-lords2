
mod tests_part;
pub use tests_part::*;

/// The seven phases `Turn_Tick` (`0x0049A010`) dispatches on `g_turnPhase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Phase {
    NeutralCounties = 1,
    ArmyMovement = 2,
    SupplyTransports = 3,
    PlayersTurn = 4,
    PeasantMobs = 5,
    Merchants = 6,
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

pub use crate::unit::UnitKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PhaseWait {
    Steps(u32),
    Units(UnitKind),
    AllRealmsDone,
    /// `Siege_TickPhase` (`0x004A84BA`) walks `g_siegeCursor` from 1 to 150
    /// calling `Siege_BuildTick` and returns as soon as one army's engines are
    /// ready, *leaving the cursor where it is*; `Turn_Tick` then runs
    /// `Siege_LaunchAssault` and comes back to the same cursor next call.
    Sieges,
    Immediate,
}

impl Phase {
    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Option<Phase> {
        PHASE_ORDER.get(i.checked_sub(1)? as usize).copied()
    }

    pub fn next(self) -> Phase {
        match self {
            Phase::SeasonEnd => Phase::NeutralCounties,
            other => Phase::from_index(other.index() + 1).expect("phases 1..6 all have a successor"),
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseTick {
    pub phase: Phase,
    pub started: bool,
    pub advanced_to: Option<Phase>,
}

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
    pub fn new() -> TurnMachine {
        TurnMachine { phase: Phase::NeutralCounties, step: 0, players_turn_open: false }
    }

    pub fn tick(&mut self, settled: bool) -> PhaseTick {
        let phase = self.phase;
        let started = self.step == 0;
        let step = self.step;
        self.step += 1;

        let done = match phase.wait() {
            PhaseWait::Steps(n) => step + 1 >= n,
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

/// `Season_Advance` (`0x00448440`) calls 29 functions in a fixed order.
///
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
    Clock,
    EventRoll,
    Weather,
    TaxCollect,
    WagesPay,
    RationApply,
    HealthUpdate,
    HappinessUpdate,
    UnrestUpdate,
    SecedeIsolatedCounties,
    /// `County_RecountFieldsAll` (`FUN_00469B51`) — every county's five field
    /// counts, rebuilt from the map. **The first of the estimate inputs**, and
    /// the reason it is a pass of its own: the counts are a cache,
    /// reclamation that runs two passes later turns a finished field into
    /// fallow on the map without touching them.
    CountyRecountFields,
    FertilityUpdate,
    /// `Field_ReclaimTick`,
    /// call. Together with [`Pass::FertilityUpdate`] this is
    /// `Fields_SeasonTick` (`0x0044BF7A`).
    FieldReclaim,
    GrainSeasonTick,
    HerdSeasonTick,
    Industry(crate::tables::Commodity),
    CastleBuildTick,
    /// `Labour_AllocateAll` (`0x0044F699`) — **reassign every peasant, from
    /// scratch**, the first of the season's two.
    LabourAllocate,
    MigrationUpdate,
    PopulationUpdate,
    /// `County_RecountMerchants` (`0x00451061`) — **which counties have a stall
    /// this season**, and therefore which of them can trade at all.
    CountyRecountMerchants,
    ScoreRank,
    /// `Army_RecountCountyTroops` (`0x004AD6C0`) — the friendly/enemy men in
    /// every county, **and the tail that reprices the county on them**:
    ///
    /// `[V]`: a new England's nine unowned counties open the season on herd 95,
    /// eat no animal (dairy covers them), come out of the tick on 67 and here
    /// eat 13 — ceiling 386 without this pass, 323 with it, and 323 is
    /// `england-turn1.sav`'s.
    ArmyRecountTroops,
    LabourAllocateAgain,
    History,
    RationPreview,
    RefreshEstimates,
    MercenaryAdvance,
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
    ReconcileAlliances,
}

/// 1. **The industry order is weapons, iron, stone, wood** — the driver
///    (`FUN_0044E852`) runs the blacksmith over every county in its own loop
///    and only then mines per county. §3.4 says *"wood, iron, stone,
///    weapons"*; §7.4's table indexes them wood, iron, weapons, stone; the
///    binary is neither, and it is [`crate::tables::INDUSTRY_ORDER`]. It
///    matters: the blacksmith spends the *previous* season's ore, because this
///    season's has not been mined yet.
///.
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
    // The position is `Season_Advance`'s: `Population_UpdateAll();
    // County_RecountMerchants(); FUN_00428471(); Army_RecountCountyTroops();`.
    Pass::CountyRecountMerchants,
    Pass::ScoreRank,
    // `FUN_00428471(); Army_RecountCountyTroops(); Event_ClearCountyModifiers();
    // Labour_AllocateAll();`.
    Pass::ArmyRecountTroops,
    Pass::LabourAllocateAgain,
    Pass::History,
    Pass::RationPreview,
    Pass::RefreshEstimates,
    Pass::MercenaryAdvance,
    Pass::UnitsResetMoves,
    Pass::ReconcileAlliances,
];

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
    pub fn order(self) -> usize {
        SEASON_PIPELINE
            .iter()
            .position(|p| *p == self)
            .expect("every Pass appears in SEASON_PIPELINE")
    }
}

