//! The battle AI: what an *unordered* unit does.
//!
//! Reimplemented from `docs/battle-ai.md`, which reads the whole of it out of
//! `Lords2.exe`. `Battle_UpdateAllUnits` (`0x00489401`) dispatches one order
//! handler per unit unless that unit's owner is human; there are **25 dispatch
//! slots holding 18 distinct functions, one of which is an empty `return`
//! filling seven slots**, so **17 real handlers**. All 18 are here.
//!
//! # The headline
//!
//! The whole AI turns on one number.
//!
//! ```text
//! strength_advantage = (weighted AI men * 100 / weighted human men) - 100  ± jitter
//! unit.orders  (+0x1A)  = how many times this unit has thought
//! unit.last_attacker    = the unit that last hit me, remembered for 50 frames
//! ```
//!
//! Every handler is a variation on one skeleton, and three consequences fall
//! straight out of it:
//!
//! * **a unit decides once every 200 frames** (100 for two of the siege
//!   defenders) — nothing else in the battle is that slow;
//! * **a unit in melee stops thinking**: 13 of the 17 refuse to run while any
//!   of their figures is fighting, so once the line makes contact most of the
//!   AI stops manoeuvring;
//! * **aggression is global, not per unit and not per side.** One word,
//!   recomputed every 101 frames from the whole battlefield, read by every
//!   handler that has a mood at all.
//!
//! # Two things that read wrong until you know them
//!
//! `orders` (`+0x1A`) is a **script program counter**, not an order: nothing a
//! player clicks ever writes it, and the handlers read it as `orders < 10`,
//! `orders % 5 == 0`. And `re targ` (`+0x14`) does **not** re-target — at zero
//! the unit reforms *its own figures* onto formation slots.
//!
//! # Where the numbers come from, and where they don't
//!
//! `AGGRESSION_THRESHOLD` = 5 and `SORTIE_THRESHOLD` = 260 are **[V]**: both
//! are `MOV imm32` writes in `Rules_InitConstants`, recovered statically by
//! `tools/oracle/initconsts.ps1` *and* read out of a live process
//! (`docs/decisions.md` C15, C16).
//!
//! Everything positional — castle approach points, wall slots, rally waypoints,
//! the castle objective — lives in [`AiField`], which this crate takes as plain
//! data and never loads. That is deliberate twice over. It is the same seam
//! [`crate::TroopTable`] uses, so a ruleset or a battlefield builder supplies
//! the numbers and the simulation stays unable to read a file. And it is
//! honest: those tables are filled by `Battlefield_BuildCastle`, which has
//! **not** been decompiled, so their *contents* are not ours to assert.
//! `docs/battle-ai.md` §9 says so; the seam keeps our ignorance visible rather
//! than inventing a castle.
//!
//! # Determinism
//!
//! The strength advantage carries a **−10…+21 jitter**, and with the threshold
//! at 5 that jitter alone decides whether the field AI attacks in any battle
//! within about thirty points of even. It therefore has to be part of the
//! lockstep state: it comes from [`Pcg32`], the generator frozen in-tree
//! (`docs/netcode.md` D-3), advanced only by simulation code and carried in
//! [`Ai`] like any other field. **No system source, no clock, no thread-local.**
//!
//! The original draws it from `Rand_Advance`, a pair of 31-bit Fibonacci LFSRs.
//! We reproduce the *distribution and the cadence*, not the stream: replicating
//! their seeding would buy nothing, since our battles are not their battles,
//! and a second generator is a second value stream to freeze forever.
//!
//! Beyond that: integer arithmetic only, every sweep by ascending index, no
//! iteration in hash order, nothing branching on an address. The handler tables
//! are `const` arrays of `fn` pointers indexed by category — the *index*
//! decides, never the pointer value.

mod world;
pub use world::*;
mod handlers;
pub use handlers::*;
mod tests;
pub use tests::*;

use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

/// The battlefield is 80 × 80 cells. `docs/formats/skr.md`, and `battle.md` §3.
pub const DIM: usize = 80;
pub const CELLS: usize = DIM * DIM;

/// `g_aiAggressionThreshold` (`0x0057C8B4`) = **5**. **[V]**
///
/// Every field handler attacks above this. Read by the three field handlers and
/// by nothing else.
pub const AGGRESSION_THRESHOLD: i32 = 5;

/// `g_aiSortieThreshold` (`0x00552FF0`) = **260**. **[V]**
///
/// A garrison sallies out only when it believes it is 3.6× the attacker's
/// strength, which is close to never.
pub const SORTIE_THRESHOLD: i32 = 260;

/// Frames between one unit's decisions, in 15 of the 17 handlers. **[D]**
pub const THINK_INTERVAL: i16 = 200;

/// The two exceptions: `SiegeDefFoot` and `SiegeDefMelee` think twice as often.
pub const THINK_INTERVAL_FAST: i16 = 100;

/// Frames between recomputations of the strength advantage. **[D]**
///
/// `Battle_UpdateAllUnits` counts up and fires at `> 100`, which is every 101st
/// frame, not every 100th.
pub const ADVANTAGE_INTERVAL: i32 = 101;

/// `UnitOrder_SiegeAttFoot` abandons its script and charges at or above this.
/// **[D]**
pub const SIEGE_CHARGE_ADVANTAGE: i32 = 151;

/// The strength weighting `Battle_UpdateStrengthAdvantage` applies per troop
/// type, indexed by [`Troop::index`]. **[D]**
///
/// A knight counts four men and a peasant one. Siege engines fall off the end
/// of the ladder and count 1, which is what the missing `else` in the original
/// does — reproduced.
pub const STRENGTH_WEIGHT: [i32; 11] = [1, 2, 3, 3, 2, 2, 4, 1, 1, 1, 1];

/// Everything positional the handlers need, as plain data.
///
/// A caller with no castle can use [`AiField::field`], which leaves the siege
/// tables empty; the siege handlers then find nothing to move to and issue no
/// order, which is exactly what the original does with an unbuilt castle.
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
    /// **[V]** from the structure layer, **[I]** from the stand-in.
    pub castle_ref: (i16, i16),
    /// `0x0055CDF0`, by lane — the secondary staging table, which is
    /// `castle_approach[3]` in the original: the same memory.
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
    /// Cell surface bytes, `y * 80 + x`. Empty means "no battlefield supplied",
    /// and every surface search then fails.
    pub surface: Vec<u8>,
    /// Cell elevation bytes, same indexing.
    pub elevation: Vec<u8>,
}

impl AiField {
    /// A field battlefield: rally waypoints and the two deployment markers,
    /// with no castle at all.
    ///
    /// `home` is each side's own marker; on a `.skr` map all three rally
    /// waypoints sit on it, which is what both field battlefield builders do.
    ///
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
        // The *enemy's* end: side 0 marches on side 4's marker and vice versa.
        for slot in 0..4 {
            let spread = slot as i16 * 2 - 3;
            f.enemy_end[0][slot] = (home_side4.0 + spread, home_side4.1);
            f.enemy_end[1][slot] = (home_side0.0 + spread, home_side0.1);
        }
        f
    }

    fn surface_at(&self, x: i32, y: i32) -> Option<u8> {
        if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
            return None;
        }
        self.surface.get(y as usize * DIM + x as usize).copied()
    }

    fn elevation_at(&self, x: i32, y: i32) -> u8 {
        if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
            return 0;
        }
        self.elevation
            .get(y as usize * DIM + x as usize)
            .copied()
            .unwrap_or(0)
    }
}

/// What a handler decided, recorded per unit.
///
/// The handlers *apply* their decision as well — they write the unit's
/// destination and, for two of them, figure states. This enum exists so a test
/// can assert which branch ran, which is the difference between checking that
/// the AI did something and checking that it did the documented thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The unit did not think this frame: the timer had not elapsed, or it is
    /// in melee, or its owner is human.
    NoThink,
    /// It thought and chose to do nothing — `Order_DoNothing`, or a branch of
    /// the script that falls through without issuing anything.
    DoNothing,
    HoldPosition,
    OntoUnit(usize),
    HalfwayToUnit(usize),
    StepAwayFromUnit(usize),
    StepTowardRallyPoint,
    ToRallyWaypoint(usize),
    ToEnemyEnd(usize),
    /// `Order_ChargeNearest`: every figure into free pursuit.
    Charge,
    /// `Order_ShootAtUnit(u)`; `u == 0` means "anything in range".
    ShootAtUnit(usize),
    ToFieldCorner,
    ToCastleApproach(usize),
    ToSiegeStaging,
    /// `Order_ToBreachOrStaging` found a moat cell to fill.
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
    /// Boiling oil found a cluster of enemies to pour on.
    PourOil,
}

/// The AI's globals, and the per-handler rotation counters that in the original
/// are module statics.
///
/// One `Ai` is one battle. Everything here is part of the lockstep state: two
/// peers with equal `Ai` values and equal inputs stay equal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ai {
    /// `g_aiStrengthAdvantage`. The one number.
    pub strength_advantage: i32,
    /// `g_aiAdvantageTimer`, counted up every frame.
    pub advantage_timer: i32,
    /// `g_aiCommitCounter` — an **escalation, not a hold**. While it is
    /// non-zero every AI melee unit charges regardless of what is near it.
    pub commit_counter: i32,
    /// `g_aiEngagementCount`, never reset within a battle.
    pub engagement_count: i32,
    /// `g_engagementBudget[battlefield]` (`0x00553080`), which
    /// `docs/battle-ai.md` §9 records as never traced to where it is filled.
/// Supplied here.
    pub engagement_budget: i32,
    /// `g_aiRallyRequest`/`X`/`Y` — the only message any unit sends any other:
    /// "archers, onto this man".
    pub rally_request: bool,
    pub rally_x: i32,
    pub rally_y: i32,
    /// `Battle_CountMenByType`'s three AI totals: men of troop types 0…6 owned
    /// by a non-human player, the missile share (crossbowmen + archers), and
    /// the knights.
    pub men_total: i32,
    pub men_missile: i32,
    pub men_knight: i32,
    /// `g_season`, the **strategic-layer** season. The field handlers key their
    /// march slot off its low bit, which looks like a free coin flip that
    /// happened to be lying around. Nothing establishes intent.
    pub season: u8,
    /// `g_battleApproachLane` (0…3) and `g_battleRallyGroup` (0 or 1), chosen
    /// once per battle by `Battle_Start`.
    pub approach_lane: usize,
    pub rally_group: usize,
    pub is_siege: bool,
    /// `g_multiplayer` (`0x00553030`) — **a network game is in progress**. When
    /// set, the jitter is dropped entirely and the two selectors above advance
    /// cyclically instead of at random: the original's own answer to "this is a
    /// networked game, so nothing may diverge". `[V]`.
    ///
    /// It was called `deterministic` here, after the name the global carried in
    /// `symbols.json`, and that name asserted the wrong thing — the flag is
    /// about the wire and the determinism is what the wire costs. See
    /// `docs/battle.md` §14.9.
    pub multiplayer: bool,
    /// The frozen generator. Advanced only by
    /// [`update_strength_advantage`](Ai::update_strength_advantage).
    pub rng: Pcg32,
    /// `g_siegeApproachScore` and `g_siegeBreachScore` — "how far in are we".
    /// The mechanism is read; the names are **[I]**.
    pub approach_score: i32,
    pub breach_score: i32,
    /// **`_DAT_0055307C` — how many ways through the outer wall there are**,
    /// mirrored from [`crate::SiegeState::ramparts_breached`] every frame.
    ///
    /// # It is not the moat flag, and this field used to say it was
    ///
    /// `AiField` carried it as `moat_flag`, `[I]`, on the strength of nothing.
    /// The binary is unambiguous and there are only six sites:
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
    /// The pair has **exactly one writer in the whole binary**, and it is
    /// [`siege_att_knight`]: an AI besieger whose entire remaining force is
    /// knights, facing a wall nothing has breached. `Battle_CheckOutcome` tests
    /// the flag *before* either men counter, so a withdrawal outranks
    /// annihilation.
    ///
/// It lives on the AI state because the AI is
    /// what raises it. [`crate::runner::BattleRunner::step`] copies it into the
    /// runner's own flag each tick; [`crate::runner::BattleRunner::withdraw`]
    /// is the same lever pulled from outside, for a caller that has a retreat
    /// of its own.
    ///
    /// The original stores the withdrawing unit's **owner** in `DAT_005656F8`
    /// and `Battle_CheckOutcome` turns it back into a side by comparing it with
    /// `g_units[g_battleArmyA].owner`; this keeps the side, which is the same
    /// answer with the round trip left out.
    ///
    /// **[V]** — one write, in one handler, against the exhaustive search for
    /// the global.
    pub withdrawal: Option<crate::figure::Side>,

    // --- per-handler rotations. Module statics in the original, so they are
    // shared by every unit that runs that handler, not per unit. ---
    rot_missile3: i32,
    rot_missile16: i32,
    rot_missile_objective: i32,
    tick_missile: i32,
    rot_foot3: i32,
    tick_foot: i32,
    rot_melee6: i32,
    rot_knight3: i32,
    wall_slot_cursor: usize,
    inner_slot_cursor: usize,
    /// `Siege_ClaimDefencePost`'s claims, parallel to
    /// [`AiField::defence_posts`].
    defence_post_owner: [u16; 20],

    /// What each unit last decided, for tests and for a debug panel. Index 0 is
    /// the unused slot.
    pub last_action: Vec<Action>,
}

impl Ai {
    /// A field battle with a seeded generator.
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
    /// this is the AI taking stock of itself, not a census of the battlefield.
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
    ///
    /// Weighted AI men as a percentage of weighted human men, minus 100, plus a
    /// **−10…+21** jitter. The asymmetric range is not a typo: the original is
    /// `(rand & 0x1F) - 10`, which is 0…31 shifted down by ten.
    ///
/// Reproduced, and the reason is worth stating: the AI
    /// is biased half a point *toward* attacking, and with the threshold at 5
    /// that bias is a real behaviour, not a rounding artefact.
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

    fn record(&mut self, unit: usize, action: Action) {
        self.last_action[unit] = action;
    }
}

// ---------------------------------------------------------------------------
// The world a handler acts on
// ---------------------------------------------------------------------------

type Handler = fn(&mut World, usize);

/// One dispatch slot.
///
/// The name and address are carried so that a slot can be
/// checked against `docs/battle-ai.md` §1.3 — and so a test can say "these
/// seven slots are the empty handler" without comparing function pointers,
/// which `docs/netcode.md` forbids anywhere a decision is made and which is a
/// bad habit to start in a test.
#[derive(Clone, Copy)]
pub struct Slot {
    pub name: &'static str,
    /// Address in the GOG Windows build, `ImageBase 0x400000`, no ASLR.
    pub addr: u32,
    run: Handler,
}

impl Slot {
    const fn new(name: &'static str, addr: u32, run: Handler) -> Slot {
        Slot { name, addr, run }
    }
}

impl core::fmt::Debug for Slot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} (0x{:08X})", self.name, self.addr)
    }
}

/// `g_unitOrderTableField` (`0x004D91B8`), bound `category < 5`.
///
/// **In a field battle, categories 5 to 8 have no handler at all** — the table
/// has five entries and the code tests `< 5`, so an AI catapult in an open
/// field is never given an order. Its figures still shoot; the unit never
/// repositions. Not observed in a running game.
pub const TABLE_FIELD: [Slot; 5] = [
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_FieldMissile", 0x0048_A9D2, field_missile),
    Slot::new("UnitOrder_FieldFoot", 0x0048_ACD2, field_foot),
    Slot::new("UnitOrder_FieldMelee", 0x0048_B02B, field_melee),
    Slot::new("UnitOrder_FieldMelee", 0x0048_B02B, field_melee),
];

/// `g_unitOrderTableSiegeAtt` (`0x004D91D0`), bound `category < 9`.
pub const TABLE_SIEGE_ATT: [Slot; 9] = [
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_SiegeAttMissile", 0x0048_D16E, siege_att_missile),
    Slot::new("UnitOrder_SiegeAttFoot", 0x0048_D412, siege_att_foot),
    Slot::new("UnitOrder_SiegeAttMelee", 0x0048_D6FC, siege_att_melee),
    Slot::new("UnitOrder_SiegeAttKnight", 0x0048_D9CE, siege_att_knight),
    Slot::new("UnitOrder_SiegeAttCatapult", 0x0048_DB84, siege_att_catapult),
    Slot::new("UnitOrder_SiegeAttTower", 0x0048_DDC7, siege_att_tower),
    Slot::new("UnitOrder_SiegeAttRam", 0x0048_DFBB, siege_att_ram),
    // Oil is a defender's weapon, so the attacker's slot for it is empty.
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
];

/// `g_unitOrderTableSiegeDef` (`0x004D91F8`), bound `category < 11`.
///
/// The three stubs at 5, 6 and 7 line up exactly with `g_raiseOrderSiege`: a
/// castle garrison holds no catapult, siege tower or ram. Two unrelated
/// structures in the binary agreeing about which troops a garrison never has.
/// **[V]**
pub const TABLE_SIEGE_DEF: [Slot; 11] = [
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_SiegeDefMissile", 0x0048_E097, siege_def_missile),
    Slot::new("UnitOrder_SiegeDefFoot", 0x0048_E39C, siege_def_foot),
    Slot::new("UnitOrder_SiegeDefMelee", 0x0048_E4CC, siege_def_melee),
    Slot::new("UnitOrder_SiegeDefKnight", 0x0048_E774, siege_def_knight),
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_SiegeDefOil", 0x0048_E8B8, siege_def_oil),
    Slot::new("UnitOrder_SiegeDefWallMissileA", 0x0048_E234, siege_def_wall_missile_a),
    Slot::new("UnitOrder_SiegeDefWallMissileB", 0x0048_E2C8, siege_def_wall_missile_b),
];

/// **Which of the eighteen handlers a unit dispatches to**, or `None` when its
/// category is past its table's bound and it is given no order at all.
///
/// One function so that `update_all_units` and anything asking *"is this
/// handler reachable?"* cannot disagree — which matters, because fourteen of
/// the seventeen were unreachable for as long as nothing could produce a siege,
/// and the only honest way to say they are reachable now is to name the
/// position that reaches each one.
pub fn handler_for(is_siege: bool, side: crate::Side, category: u8) -> Option<Slot> {
    let category = category as usize;
    if !is_siege {
        TABLE_FIELD.get(category).copied()
    } else if side == crate::SIDE_A {
        TABLE_SIEGE_DEF.get(category).copied()
    } else {
        TABLE_SIEGE_ATT.get(category).copied()
    }
}

/// `Battle_UpdateAllUnits` (`0x00489401`), once per frame.
///
/// The order of operations is the original's and it matters: the strength
/// advantage is recomputed *before* any unit thinks, each unit is recentred on
/// its figures *before* its handler runs, and the reform countdown runs
/// **outside** the human-control guard — so a player's units reform
/// too.
///
/// Returns the units whose reform countdown reached zero and which
/// `BattleUnit_NeedsReform` accepts. Reforming assigns figures to formation
/// slots on a battlefield, so it belongs to whoever owns positions; this crate
/// says *which* units want it and stops there.
pub fn update_all_units(
    units: &mut Units,
    figures: &mut [Figure],
    positions: &[(u8, u8)],
    field: &AiField,
    ai: &mut Ai,
) -> Vec<usize> {
    ai.advantage_timer += 1;
    if ai.advantage_timer > ADVANTAGE_INTERVAL - 1 {
        ai.update_strength_advantage(figures);
        ai.advantage_timer = 0;
    }

    let mut reform = Vec::new();
    for cur in 1..=MAX_UNITS {
        if !units.get(cur).is_live() {
            continue;
        }
        units.recentre(cur, figures, positions);
        if !units.get(cur).human {
            let slot = handler_for(ai.is_siege, units.get(cur).side, units.get(cur).category);
            if let Some(slot) = slot {
                let mut world = World { units, figures, positions, field, ai };
                (slot.run)(&mut world, cur);
            } else {
                // Out of the table's bound: no order at all. In a field battle
                // that is every category from 5 to 8.
                ai.record(cur, Action::NoThink);
            }
        } else {
            ai.record(cur, Action::NoThink);
        }

        let u = units.get_mut(cur);
        u.reform -= 1;
        if u.reform < 1 {
            u.reform = REFORM_INTERVAL;
            if needs_reform(units.get(cur)) {
                reform.push(cur);
            }
        }
    }
    reform
}

/// `BattleUnit_NeedsReform` (`0x00489654`): false for an empty unit, false for
/// one that has been ordered to charge, and false for a **human-controlled**
/// unit of fewer than four figures.
pub fn needs_reform(u: &crate::unit::BattleUnit) -> bool {
    if u.figures == 0 || u.halted {
        return false;
    }
    !(u.figures < 4 && !u.reform_gate && u.human)
}

