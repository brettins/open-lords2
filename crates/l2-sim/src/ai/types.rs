#![allow(unused_imports)]
use super::*;
use super::handlers_part::*;
use world::*;
use handlers::*;
use tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiField {
    /// `g_rallyWaypoints`, `[side][group][index]`. `side` is 0 for side 0 and 1
    /// for side 4; `group` is `g_battleRallyGroup`. **[D]**
    pub rally: [[[(i16, i16); 3]; 2]; 2],
    /// `BattleUnit_OrderToEnemyEnd`'s two tables: the four slots of the
    /// **opposing** side's deployment marker, `[side][slot]`. **[D]**
    pub enemy_end: [[(i16, i16); 4]; 2],
    /// `0x0055CD90`, `[index][lane]` — the castle approach points, chosen by
    /// `g_battleApproachLane`. **[V]** on the [`crate::castle::ai_field`] path,
    /// where they are the structure layer's own markers
    /// (`Battlefield_ReadStructureLayer`); **[I]** only on
    /// [`crate::siege::our_castle_ai_field`]'s, which invents them.
    pub castle_approach: [[(i16, i16); 4]; 6],
    /// `0x0055CDD0` — the castle's reference cell, which several orders offset
    /// from. `castle_approach[2][0]` in the original: the same memory.
    ///
    /// **[V]** from the structure layer, **[I]** from the stand-in.
    pub castle_ref: (i16, i16),
    /// `0x0055CDF0`, by lane — the secondary staging table, which is
    /// `castle_approach[3]` in the original: the same memory.
    ///
    /// **[V]** from the structure layer, **[I]** from the stand-in.
    pub staging: [(i16, i16); 4],
    /// `0x00554180`, `[group][slot]`: **sixteen** slots per group, empties
    /// skipped by scanning forward and wrapping. `docs/battle-ai.md` describes
    /// nine/four/one *used*; the table itself is sixteen wide. **[D]**
    pub wall_slot: [[(i16, i16); 16]; 3],
    /// `0x00553274` and `0x00553EE4`, as cell indices (`y * 80 + x`). The
    /// builder's code ladder seeds them — the first code-6 cell one row north,
    /// the first code-8 cell at `off − 0x278` — so **[V]** on the
    /// [`crate::castle::ai_field`] path and **[I]** on the stand-in's.
    pub castle_objective: [usize; 2],
    /// `Siege_ClaimDefencePost`'s twenty candidate cells. A zero entry is
    /// empty and never claimed. **[D]** for the mechanism, **[I]** for the
    /// contents.
    pub defence_posts: [usize; 20],
    /// `0x004EEB64`: 1 and 2 name the two fixed field corners `(6, 74)` and
    /// `(74, 74)`; anything else sends the unit to a castle approach point
    /// instead. **[D]**
    pub layout: u8,
    /// `0x004EEB6C`, the castle orientation flag. **[I]**
    pub orientation: u8,
    /// `0x00542CD4`, the castle layout flag two handlers jump their `orders` to
    /// 100 on. **[I]**
    pub castle_layout_flag: bool,
    /// `0x0056D590`, the castle index; 13 picks the primary objective. **[I]**
    pub castle_index: u8,
    pub surface: Vec<u8>,
    pub elevation: Vec<u8>,
}

impl AiField {
    /// **[V]**, and it decides how a field battle opens. `Battlefield_BuildRandom`
    /// writes `g_rallyWaypoints[i] = g_foundTileX` for `i` in `0..3`, for **both**
    /// rally groups, out of the same tile it just read the deployment marker
    /// from — in the terrain-`0x14` arm and again in the terrain-`0x1E` one. So
    /// `Order_ToRallyWaypoint` sends a unit to where it already is, and the
    /// cautious branch of `UnitOrder_FieldFoot` / `UnitOrder_FieldMelee` — the
    /// branch every handler takes while `g_aiStrengthAdvantage` is under 5 — has
    /// no way to advance at all. **An outnumbered AI stands on its marker and
    /// waits**, and a player who gives no orders gets a battle in which nothing
    /// happens. `docs/decisions.md` C186.
    pub fn field(home_side0: (i16, i16), home_side4: (i16, i16)) -> Self {
        let mut f = AiField {
            rally: [[[(0, 0); 3]; 2]; 2],
            enemy_end: [[(0, 0); 4]; 2],
            castle_approach: [[(0, 0); 4]; 6],
            castle_ref: (0, 0),
            staging: [(0, 0); 4],
            wall_slot: [[(0, 0); 16]; 3],
            castle_objective: [0; 2],
            defence_posts: [0; 20],
            layout: 0,
            orientation: 0,
            castle_layout_flag: false,
            castle_index: 0,
            surface: Vec::new(),
            elevation: Vec::new(),
        };
        for group in 0..2 {
            for i in 0..3 {
                f.rally[0][group][i] = home_side0;
                f.rally[1][group][i] = home_side4;
            }
        }
        for slot in 0..4 {
            let spread = slot as i16 * 2 - 3;
            f.enemy_end[0][slot] = (home_side4.0 + spread, home_side4.1);
            f.enemy_end[1][slot] = (home_side0.0 + spread, home_side0.1);
        }
        f
    }

    pub(super) fn surface_at(&self, x: i32, y: i32) -> Option<u8> {
        if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
            return None;
        }
        self.surface.get(y as usize * DIM + x as usize).copied()
    }

    pub(super) fn elevation_at(&self, x: i32, y: i32) -> u8 {
        if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
            return 0;
        }
        self.elevation
            .get(y as usize * DIM + x as usize)
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NoThink,
    DoNothing,
    HoldPosition,
    OntoUnit(usize),
    HalfwayToUnit(usize),
    StepAwayFromUnit(usize),
    StepTowardRallyPoint,
    ToRallyWaypoint(usize),
    ToEnemyEnd(usize),
    Charge,
    ShootAtUnit(usize),
    ToFieldCorner,
    ToCastleApproach(usize),
    ToSiegeStaging,
    ToMoatCell,
    ToCastleObjective(usize),
    ToWallSlot(usize, usize),
    ToWallBelowKeep,
    ToNearestWallCell,
    ToWallNearPreferredTarget,
    ToWallNearAvoidedTarget,
    ToSurface5Near,
    ToSecondTarget,
    ToCell,
    PourOil,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ai {
    pub strength_advantage: i32,
    pub advantage_timer: i32,
    pub commit_counter: i32,
    pub engagement_count: i32,
    /// `g_engagementBudget[battlefield]` (`0x00553080`), which
    /// `docs/battle-ai.md` §9 records as never traced to where it is filled.
    pub engagement_budget: i32,
    pub rally_request: bool,
    pub rally_x: i32,
    pub rally_y: i32,
    pub men_total: i32,
    pub men_missile: i32,
    pub men_knight: i32,
    pub season: u8,
    pub approach_lane: usize,
    pub rally_group: usize,
    pub is_siege: bool,
    /// `g_multiplayer` (`0x00553030`) — **a network game is in progress**. When
    /// set, the jitter is dropped entirely and the two selectors above advance
    /// cyclically instead of at random: the original's own answer to "this is a
    /// networked game, so nothing may diverge". `[V]`.
    pub multiplayer: bool,
    pub rng: Pcg32,
    /// The mechanism is read; the names are **[I]**.
    pub approach_score: i32,
    pub breach_score: i32,
    /// `AiField` carried it as `moat_flag`, `[I]`, on the strength of nothing.
    ///
    /// * **initialised** by `Battlefield_BuildCastle` — hard-coded to 1 for
    ///   campaign castle levels **0 and 3** and 0 for the rest, or, on the
    ///   skirmish path, from a twenty-entry table at `0x004D4A98` whose values
    ///   are `1 0 0 0 0 1 0 0 1 0 1 0 0 0 0 0 0 1 1 1` — a per-layout flag,
    ///   one per castle raster;
    /// * **incremented** by `BattleMan_StateAttackWall` and
    ///   `BattleMan_StateRamGate`, in each case at the 5,000-hit rampart
    /// threshold, beside `Wall_Smash` and the counter reset. Nothing else
    ///   writes it — `Wall_Collapse` does **not**;
    /// * **read** three times: `BattleMan_StateRamGate` sends a ram away
    ///   (`1 < it`), and `Order_ToCastleObjective` and
    ///   `UnitOrder_SiegeDefMissile` branch on `it < 1`.
    ///
    /// So it counts gaps in the wall, it is seeded because two of the five
    /// castle layouts ship with one, and no moat function in the binary
    /// touches it. `[V]` on the writers and readers, `[I]` on *"a gap"* as
    /// against some other per-layout property that a rampart breach also
    /// creates.
    pub ramparts_breached: i32,
    /// `g_attackersOnWall`, recounted every frame. **[I]**
    pub attackers_on_wall: i32,
    /// `g_siegeEngineCount`: live figures of troop type 7, 8 or 9. **[D]**
    pub siege_engine_count: i32,
    /// `0x00569588` — whether the drawbridge patch has been laid. **[I]**
    pub drawbridge_down: bool,
    /// **`g_battleWithdrawal` (`0x0056D5C8`) and `DAT_005656F8`** — a side has
    /// given up and left the field, and which side it was.
    ///
    /// The original stores the withdrawing unit's **owner** in `DAT_005656F8`
    /// and `Battle_CheckOutcome` turns it back into a side by comparing it with
    /// `g_units[g_battleArmyA].owner`; this keeps the side, which is the same
    /// answer with the round trip left out.
    ///
    /// **[V]** — one write, in one handler, against the exhaustive search for
    /// the global.
    pub withdrawal: Option<crate::figure::Side>,

    pub(super) rot_missile3: i32,
    pub(super) rot_missile16: i32,
    pub(super) rot_missile_objective: i32,
    pub(super) tick_missile: i32,
    pub(super) rot_foot3: i32,
    pub(super) tick_foot: i32,
    pub(super) rot_melee6: i32,
    pub(super) rot_knight3: i32,
    pub(super) wall_slot_cursor: usize,
    pub(super) inner_slot_cursor: usize,
    pub(super) defence_post_owner: [u16; 20],

    pub last_action: Vec<Action>,
}

impl Ai {
    pub fn new(seed: u64) -> Self {
        Ai {
            strength_advantage: 0,
            advantage_timer: 0,
            commit_counter: 0,
            engagement_count: 0,
            engagement_budget: 6,
            rally_request: false,
            rally_x: 0,
            rally_y: 0,
            men_total: 0,
            men_missile: 0,
            men_knight: 0,
            season: 0,
            approach_lane: 0,
            rally_group: 0,
            is_siege: false,
            multiplayer: false,
            rng: Pcg32::from_seed(seed),
            approach_score: 0,
            breach_score: 0,
            ramparts_breached: 0,
            attackers_on_wall: 0,
            siege_engine_count: 0,
            drawbridge_down: false,
            withdrawal: None,
            rot_missile3: 0,
            rot_missile16: 0,
            rot_missile_objective: 0,
            tick_missile: 0,
            rot_foot3: 0,
            tick_foot: 0,
            rot_melee6: 0,
            rot_knight3: 0,
            wall_slot_cursor: 0,
            inner_slot_cursor: 0,
            defence_post_owner: [0; 20],
            last_action: vec![Action::NoThink; MAX_UNITS + 1],
        }
    }

    /// `Battle_CountMenByType` (`0x00481B9A`), the three totals the handlers
    /// read. Only **non-human-owned** figures of troop type 0…6 are counted:
    pub fn count_men(&mut self, figures: &[Figure]) {
        self.men_total = 0;
        self.men_missile = 0;
        self.men_knight = 0;
        self.siege_engine_count = 0;
        for f in figures {
            if !f.is_alive() {
                continue;
            }
            match f.troop {
                Troop::Catapults | Troop::SiegeTowers | Troop::BatteringRams => {
                    self.siege_engine_count += 1;
                }
                _ => {}
            }
            if f.troop.index() >= 7 || f.owner_is_human {
                continue;
            }
            let men = f.men as i32;
            self.men_total += men;
            match f.troop {
                Troop::Crossbowmen | Troop::Archers => self.men_missile += men,
                Troop::Knights => self.men_knight += men,
                _ => {}
            }
        }
    }

    /// `Battle_UpdateStrengthAdvantage` (`0x0047FC01`).
    pub fn update_strength_advantage(&mut self, figures: &[Figure]) {
        let (mut ai_men, mut human_men) = (0i32, 0i32);
        for f in figures {
            if !f.is_alive() {
                continue;
            }
            let weighted = f.men as i32 * STRENGTH_WEIGHT[f.troop.index()];
            if f.owner_is_human {
                human_men += weighted;
            } else {
                ai_men += weighted;
            }
        }
        self.strength_advantage = pct_of(ai_men, human_men) - 100;
        if !self.multiplayer {
            self.strength_advantage += (self.rng.next_u32() & 0x1F) as i32 - 10;
        }
    }

    pub(super) fn record(&mut self, unit: usize, action: Action) {
        self.last_action[unit] = action;
    }
}


