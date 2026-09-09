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
/// does — reproduced rather than "corrected" to 0.
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
    /// `g_battleApproachLane`. **[I]** (`Battlefield_BuildCastle` unread.)
    pub castle_approach: [[(i16, i16); 4]; 6],
    /// `0x0055CDD0` — the castle's reference cell, which several orders offset
    /// from. **[I]**
    pub castle_ref: (i16, i16),
    /// `0x0055CDF0`, by lane — the secondary staging table. **[I]**
    pub staging: [(i16, i16); 4],
    /// `0x00554180`, `[group][slot]`: **sixteen** slots per group, empties
    /// skipped by scanning forward and wrapping. `docs/battle-ai.md` describes
    /// nine/four/one *used*; the table itself is sixteen wide. **[D]**
    pub wall_slot: [[(i16, i16); 16]; 3],
    /// `0x00553274` and `0x00553EE4`, as cell indices (`y * 80 + x`). **[I]**
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
    /// `0x0055307C`, the moat flag. **[I]**
    pub moat_flag: i32,
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
            moat_flag: 0,
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
    /// Supplied here rather than invented.
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
    /// It lives on the AI state rather than on the runner because the AI is
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
    /// Reproduced rather than centred, and the reason is worth stating: the AI
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

/// Everything a handler can see, borrowed for one dispatch.
pub struct World<'a> {
    pub units: &'a mut Units,
    pub figures: &'a mut [Figure],
    /// Where each figure stands, indexed by figure. `l2-sim` does not own
    /// positions; whoever does supplies them here.
    pub positions: &'a [(u8, u8)],
    pub field: &'a AiField,
    pub ai: &'a mut Ai,
}

// The `to_*` methods below take `&mut self` rather than `self`, which trips
// `wrong_self_convention`. They are named for the original's `Order_To*`
// routines — `Order_ToCell`, `Order_ToWallSlot`, `Order_ToCastleObjective` —
// and keeping that correspondence is worth more than the Rust naming idiom,
// because it is what lets a reader check each one against the binary.
#[allow(clippy::wrong_self_convention)]
impl World<'_> {
    fn unit_pos(&self, u: usize) -> (i16, i16) {
        let u = self.units.get(u);
        (u.x, u.y)
    }

    fn set_target(&mut self, cur: usize, x: i16, y: i16) {
        let u = self.units.get_mut(cur);
        u.target_x = x;
        u.target_y = y;
    }

    /// The three destination-writing orders all clear the withdrawing flag
    /// first. `Order_StepAwayFromUnit` is the one that sets it.
    fn clear_withdrawing(&mut self, cur: usize) {
        self.units.get_mut(cur).withdrawing = false;
    }

    fn side_index(&self, cur: usize) -> usize {
        if self.units.get(cur).side == 0 {
            0
        } else {
            1
        }
    }

    /// `Enemy_NearestUnit` (`0x0048EF45`): nearest **enemy unit** by Chebyshev
    /// distance between the two units' centres, within `max_dist`, holding at
    /// least `min_figures` figures. `0` when nothing qualifies.
    ///
    /// "Enemy" is *a different owner byte*, not a different side. No weighting
    /// of any kind — no troop type, no strength, no facing, no threat.
    pub fn nearest_enemy_unit(&self, cur: usize, max_dist: i32, min_figures: u8) -> usize {
        let mine = self.units.get(cur).owner;
        let (x, y) = self.unit_pos(cur);
        let mut best = 0usize;
        // 160000 in the original: larger than any distance on an 80-cell map.
        let mut best_dist = 160_000i32;
        for i in 1..=MAX_UNITS {
            let u = self.units.get(i);
            if !u.is_live() || u.owner == mine || u.figures < min_figures {
                continue;
            }
            let d = chebyshev(x, y, u.x, u.y);
            if d <= max_dist && d < best_dist {
                best = i;
                best_dist = d;
            }
        }
        best
    }

    // --- the action vocabulary ---------------------------------------------
    //
    // "There is no formation change, no facing order, no fire-at-will toggle,
    // no reserve, and no withdrawal from the field. The AI's entire vocabulary
    // is *go here*, *shoot that unit* and *everybody charge*."

    /// `Order_HoldPosition`: destination = own position.
    fn hold_position(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        let (x, y) = self.unit_pos(cur);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::HoldPosition);
    }

    /// `Order_OntoUnit`: walk straight at another unit.
    fn onto_unit(&mut self, cur: usize, other: usize) {
        self.clear_withdrawing(cur);
        let (x, y) = self.unit_pos(other);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::OntoUnit(other));
    }

    /// `Order_HalfwayToUnit`: halve the separation per axis.
    ///
    /// It does **nothing at all** unless one axis is separated by eight cells
    /// or more, and then moves only the axes separated by six or more. That,
    /// plus `Order_StopShortOfTarget` pulling a missile unit's destination back
    /// to `range/8 − 3`, is why AI archers converge on a standoff distance
    /// rather than closing.
    fn halfway_to_unit(&mut self, cur: usize, other: usize) {
        self.clear_withdrawing(cur);
        let (sx, sy) = self.unit_pos(cur);
        let (ox, oy) = self.unit_pos(other);
        let hx = ((ox as i32 - sx as i32).abs()) / 2;
        let hy = ((oy as i32 - sy as i32).abs()) / 2;
        if hx <= 3 && hy <= 3 {
            return;
        }
        let (mut tx, mut ty) = (sx, sy);
        if hx > 2 {
            tx = if sx < ox { sx + hx as i16 } else { sx - hx as i16 };
        }
        if hy > 2 {
            ty = if sy < oy { sy + hy as i16 } else { sy - hy as i16 };
        }
        self.set_target(cur, tx, ty);
        self.ai.record(cur, Action::HalfwayToUnit(other));
    }

    /// `Order_StepAwayFromUnit`: shift the **destination** two cells away.
    ///
    /// Not the position — the destination, which is why repeated withdrawals
    /// compound. Along the longer axis, and along both once an axis separation
    /// exceeds five. This is the whole of the AI's retreat behaviour.
    fn step_away_from_unit(&mut self, cur: usize, other: usize) {
        let (ox, oy) = self.unit_pos(other);
        let (sx, sy) = self.unit_pos(cur);
        let (adx, ady) = (
            (ox as i32 - sx as i32).abs(),
            (oy as i32 - sy as i32).abs(),
        );
        let (mut dx, mut dy) = (0i16, 0i16);
        if ady < adx {
            dx = if ox < sx { 2 } else { -2 };
        } else {
            dy = if oy < sy { 2 } else { -2 };
        }
        if adx > 5 {
            dx = if ox < sx { 2 } else { -2 };
        }
        if ady > 5 {
            dy = if oy < sy { 2 } else { -2 };
        }
        {
            let u = self.units.get_mut(cur);
            u.withdrawing = true;
            u.withdrawals = u.withdrawals.wrapping_add(1);
            u.target_x += dx;
            u.target_y += dy;
        }
        self.ai.record(cur, Action::StepAwayFromUnit(other));
    }

    /// `Order_StepTowardRallyPoint`: the mirror of the above, toward
    /// `rally_x/y`, **and it clears the request** — so the first missile unit
    /// to think answers the call and the rest do not.
    fn step_toward_rally_point(&mut self, cur: usize) {
        let (rx, ry) = (self.ai.rally_x, self.ai.rally_y);
        self.ai.rally_request = false;
        let (sx, sy) = self.unit_pos(cur);
        let (adx, ady) = ((rx - sx as i32).abs(), (ry - sy as i32).abs());
        let (mut dx, mut dy) = (0i16, 0i16);
        if ady < adx {
            dx = if rx < sx as i32 { -2 } else { 2 };
        } else {
            dy = if ry < sy as i32 { -2 } else { 2 };
        }
        if adx > 5 {
            dx = if rx < sx as i32 { -2 } else { 2 };
        }
        if ady > 5 {
            dy = if ry < sy as i32 { -2 } else { 2 };
        }
        {
            let u = self.units.get_mut(cur);
            u.target_x += dx;
            u.target_y += dy;
        }
        self.ai.record(cur, Action::StepTowardRallyPoint);
    }

    /// `Order_ToRallyWaypoint`: one of three waypoints for this side, in the
    /// group `Battle_Start` picked.
    fn to_rally_waypoint(&mut self, cur: usize, index: usize) {
        let side = self.side_index(cur);
        let (x, y) = self.field.rally[side][self.ai.rally_group][index];
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToRallyWaypoint(index));
    }

    /// `BattleUnit_OrderToEnemyEnd`: a slot of the *opposing* side's
    /// deployment marker.
    fn to_enemy_end(&mut self, cur: usize, slot: usize) {
        self.clear_withdrawing(cur);
        let side = self.side_index(cur);
        let (x, y) = self.field.enemy_end[side][slot & 3];
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToEnemyEnd(slot & 3));
    }

    /// `Order_ChargeNearest` (`0x0048C8AF`) — the most consequential action.
    ///
    /// It sets the unit's halted flag, which switches the every-500-frame
    /// reform off, and puts **every** figure of the unit into free pursuit.
    /// From then on each figure picks its own victim and ignores the unit's
    /// destination: *a charged unit stops being a formation*.
    fn charge_nearest(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        self.units.get_mut(cur).halted = true;
        for f in self.figures.iter_mut() {
            if f.is_alive() && f.unit as usize == cur {
                f.state = State::Chasing;
                f.target = None;
            }
        }
        self.ai.record(cur, Action::Charge);
    }

    /// `Order_ShootAtUnit` (`0x0048C974`): every figure of this unit takes a
    /// target within forty cells, restricted to `unit` when that is non-zero,
    /// and enters state 17.
    ///
    /// Returns false if **any** figure found nothing, which the siege missile
    /// handler reads as "that unit is out of reach". Note the odd exclusion the
    /// original carries and this reproduces: a shooter standing on surface 1
    /// will not take a target standing on surface 5.
    fn shoot_at_unit(&mut self, cur: usize, unit: usize) -> bool {
        let mut all_found = true;
        for f in 0..self.figures.len() {
            if !self.figures[f].is_alive() || self.figures[f].unit as usize != cur {
                continue;
            }
            let Some(target) = self.find_missile_target(f, 40, unit) else {
                all_found = false;
                continue;
            };
            let here = self.positions.get(f).copied().unwrap_or((0, 0));
            let there = self.positions.get(target).copied().unwrap_or((0, 0));
            let blocked = self.field.surface_at(here.0 as i32, here.1 as i32) == Some(1)
                && self.field.surface_at(there.0 as i32, there.1 as i32) == Some(5);
            if !blocked {
                self.figures[f].state = State::Shooting;
                self.figures[f].target = Some(target);
            }
        }
        self.ai.record(cur, Action::ShootAtUnit(unit));
        all_found
    }

    /// `Missile_FindTarget` (`0x004956CC`), reduced to what the order handlers
    /// need of it.
    ///
    /// Score is Manhattan distance capped at 160; a siege-engine target costs
    /// +35 and is invisible to a melee figure; and the *range* test is a square
    /// box even though the *score* is Manhattan. `restrict` limits the search
    /// to one unit when non-zero.
    fn find_missile_target(&self, shooter: usize, range: i32, restrict: usize) -> Option<usize> {
        let me = &self.figures[shooter];
        let mine = me.owner;
        let melee_only = !matches!(me.troop, Troop::Crossbowmen | Troop::Archers | Troop::Catapults);
        let &(sx, sy) = self.positions.get(shooter)?;
        let (sx, sy) = (sx as i32, sy as i32);
        let mut best: Option<(i32, usize)> = None;
        for (i, f) in self.figures.iter().enumerate() {
            if !f.is_alive() || f.owner == mine || f.owner == 0 {
                continue;
            }
            if restrict != 0 && f.unit as usize != restrict {
                continue;
            }
            if f.troop.is_siege() && melee_only {
                continue;
            }
            let Some(&(x, y)) = self.positions.get(i) else { continue };
            let (dx, dy) = ((x as i32 - sx).abs(), (y as i32 - sy).abs());
            if dx > range || dy > range {
                continue;
            }
            let mut score = (dx + dy).min(160);
            if f.troop.is_siege() {
                score += 35;
            }
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, i));
            }
        }
        best.map(|(_, i)| i)
    }

    // --- the siege position vocabulary -------------------------------------

    /// `Order_ToCell`: a cell index straight into the destination.
    fn to_cell(&mut self, cur: usize, cell: usize) {
        let (x, y) = ((cell % DIM) as i16, (cell / DIM) as i16);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToCell);
    }

    /// `Order_ToFieldCorner`: `(6, 74)` or `(74, 74)` by castle layout, and a
    /// castle approach point when the layout is neither 1 nor 2.
    fn to_field_corner(&mut self, cur: usize, index: usize) {
        self.clear_withdrawing(cur);
        match self.field.layout {
            1 => {
                self.set_target(cur, 6, 74);
                self.ai.record(cur, Action::ToFieldCorner);
            }
            2 => {
                self.set_target(cur, 74, 74);
                self.ai.record(cur, Action::ToFieldCorner);
            }
            _ => self.to_castle_approach(cur, index),
        }
    }

    /// `Order_ToCastleApproach`: one of four approach points per index, by
    /// lane. Rams (category 7) always use the primary table; everyone else
    /// switches on the castle orientation flag.
    fn to_castle_approach(&mut self, cur: usize, index: usize) {
        self.clear_withdrawing(cur);
        let category = self.units.get(cur).category;
        let lane = self.ai.approach_lane & 3;
        let (x, y) = if category == 7 || self.field.orientation == 0 {
            self.field.castle_approach[index.min(5)][lane]
        } else if category == 1 {
            (self.field.castle_ref.0 + 10, self.field.castle_ref.1 + 10)
        } else {
            (self.field.castle_ref.0 + 2, self.field.castle_ref.1 + 15)
        };
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToCastleApproach(index));
    }

    /// `Order_ToSiegeStaging`: the primary approach table below approach score
    /// 16 or above 399, the secondary five cells further in between, or a fixed
    /// offset from the castle when the orientation flag is set.
    fn to_siege_staging(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        let lane = self.ai.approach_lane & 3;
        let (x, y) = if self.field.orientation == 0 {
            if self.ai.approach_score < 16 || self.ai.approach_score > 399 {
                self.field.castle_approach[0][lane]
            } else {
                let p = self.field.staging[lane];
                (p.0, p.1 + 5)
            }
        } else {
            (self.field.castle_ref.0 + 2, self.field.castle_ref.1 + 12)
        };
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToSiegeStaging);
    }

    /// `Order_ToBreachOrStaging`: below approach score 16 it looks for the
    /// nearest **moat** cell at radii 12…19 and walks onto it; between 16 and
    /// 400 it falls back on the secondary staging table; above 400 it does
    /// nothing at all.
    ///
    /// The name in `docs/battle-ai.md` reads it as a breach. The cell it
    /// searches for is surface **2**, which `docs/battle.md` §7 and the
    /// state-9 moat handler both give as water — so the early phase of a siege
    /// is an order to go and fill the moat in.
    fn to_breach_or_staging(&mut self, cur: usize) {
        if self.ai.approach_score >= 401 {
            self.ai.record(cur, Action::DoNothing);
            return;
        }
        if self.ai.approach_score < 16 {
            let (sx, sy) = self.unit_pos(cur);
            for radius in 12..20 {
                if let Some((x, y)) = self.nearest_surface(sx as i32, sy as i32, radius, 2, false) {
                    self.set_target(cur, x as i16, y as i16);
                    self.ai.record(cur, Action::ToMoatCell);
                    return;
                }
            }
            self.ai.record(cur, Action::DoNothing);
            return;
        }
        let (x, y) = self.field.staging[self.ai.approach_lane & 3];
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToSiegeStaging);
    }

    /// `Order_ToCastleObjective`: one of the two cells the castle builder
    /// recorded.
    fn to_castle_objective(&mut self, cur: usize, mode: usize) {
        let cell = if mode == 1 || self.field.moat_flag >= 1 || self.field.castle_index == 13 {
            self.field.castle_objective[0]
        } else {
            self.field.castle_objective[1]
        };
        self.to_cell(cur, cell);
        self.ai.record(cur, Action::ToCastleObjective(mode));
    }

    /// `Order_ToWallSlot`: one of sixteen positions in a group, **skipping
    /// empty entries and wrapping**, offset one cell west and two north of the
    /// recorded position.
    fn to_wall_slot(&mut self, cur: usize, group: usize, slot: usize) {
        self.clear_withdrawing(cur);
        let mut slot = slot % 16;
        let mut guard = 17;
        while guard > 0 {
            let (x, y) = self.field.wall_slot[group][slot];
            if x != 0 && y != 0 {
                break;
            }
            slot = (slot + 1) % 16;
            guard -= 1;
        }
        let (x, y) = self.field.wall_slot[group][slot];
        self.set_target(cur, x - 1, y - 2);
        self.ai.record(cur, Action::ToWallSlot(group, slot));
    }

    /// `Order_ToNearestWallCell`: the nearest surface-4 cell within forty.
    fn to_nearest_wall_cell(&mut self, cur: usize) {
        let (sx, sy) = self.unit_pos(cur);
        if let Some((x, y)) = self.nearest_surface(sx as i32, sy as i32, 40, 4, true) {
            self.set_target(cur, x as i16, y as i16);
            self.ai.record(cur, Action::ToNearestWallCell);
        } else {
            self.ai.record(cur, Action::DoNothing);
        }
    }

    /// `Order_ToWallBelowKeep`: the primary castle cell, four cells south, then
    /// the nearest surface-4 cell within five of *that*.
    fn to_wall_below_keep(&mut self, cur: usize) {
        let cell = self.field.castle_objective[0];
        let (x, y) = ((cell % DIM) as i32, (cell / DIM) as i32 + 4);
        if let Some((wx, wy)) = self.nearest_surface(x, y, 5, 4, true) {
            self.set_target(cur, wx as i16, wy as i16);
            self.ai.record(cur, Action::ToWallBelowKeep);
        } else {
            // The original writes the offset destination *before* the search
            // and only re-issues the order when the search succeeds, so a
            // failed search leaves the destination changed but unordered.
            self.ai.record(cur, Action::DoNothing);
        }
    }

    /// `Order_ToSurface5Near`: the nearest surface-5 cell within five of a
    /// given cell.
    ///
    /// **Reproduces an original bug.** `Siege_FindCellSurface5` measures its
    /// distance from the *clipped top-left corner of the search box*, not from
    /// the cell it was asked about — the local holding the query point is
    /// overwritten by the clipping arithmetic before the distance call. Its
    /// sibling `Siege_FindCellSurface4` saves the query point first and does
    /// not have the fault. Left as written: the original's choice of cell is
    /// the specification, and "fixing" it moves defenders somewhere else.
    fn to_surface5_near(&mut self, cur: usize, cell: usize) {
        let (x, y) = ((cell % DIM) as i32, (cell / DIM) as i32);
        if let Some((wx, wy)) = self.nearest_surface_from_box_corner(x, y, 5, 5) {
            self.set_target(cur, wx as i16, wy as i16);
            self.ai.record(cur, Action::ToSurface5Near);
        } else {
            self.ai.record(cur, Action::DoNothing);
        }
    }

    /// `Order_ToSecondTarget`: the unit's stored second destination. We have no
    /// second destination pair, so this holds the destination it already has —
    /// which is what copying `+0x26/+0x28` onto `+0x22/+0x24` does on a unit
    /// that has never been given one.
    fn to_second_target(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        self.ai.record(cur, Action::ToSecondTarget);
    }

    /// `Order_ToWallNearPreferredTarget` / `Order_ToWallNearAvoidedTarget`.
    ///
    /// The two weighted variants of `Missile_FindTarget` choose *where to
    /// stand*, not what to shoot: one halves the score of an enemy carrying a
    /// missile weapon and the other doubles it, and the chosen enemy's nearest
    /// wall cell becomes the destination. **[I]** on the wall step — the
    /// weighting is read from the binary, the "then stand beside him" is how
    /// the callers use the result.
    fn to_wall_near_target(&mut self, cur: usize, prefer_shooters: bool) {
        let (sx, sy) = self.unit_pos(cur);
        let mine = self.units.get(cur).owner;
        let mut best: Option<(i32, usize)> = None;
        for (i, f) in self.figures.iter().enumerate() {
            if !f.is_alive() || f.owner == mine || f.owner == 0 {
                continue;
            }
            let Some(&(x, y)) = self.positions.get(i) else { continue };
            let mut score =
                ((x as i32 - sx as i32).abs() + (y as i32 - sy as i32).abs()).min(160);
            if f.state == State::FillingMoat {
                score /= 4;
            }
            let shooter = matches!(f.troop, Troop::Crossbowmen | Troop::Archers);
            if shooter {
                if prefer_shooters {
                    score /= 2;
                } else {
                    score *= 2;
                }
            }
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, i));
            }
        }
        let action = if prefer_shooters {
            Action::ToWallNearPreferredTarget
        } else {
            Action::ToWallNearAvoidedTarget
        };
        let Some((_, target)) = best else {
            self.ai.record(cur, Action::DoNothing);
            return;
        };
        let (tx, ty) = self.positions[target];
        if let Some((wx, wy)) = self.nearest_surface(tx as i32, ty as i32, 5, 4, true) {
            self.set_target(cur, wx as i16, wy as i16);
            self.ai.record(cur, action);
        } else {
            self.ai.record(cur, Action::DoNothing);
        }
    }

    /// `Siege_FindCellSurface4` / `Siege_FindCellSurface5`: nearest cell of a
    /// given surface inside a square box, clipped to the map.
    ///
    /// `manhattan` picks the metric — the original uses Manhattan distance when
    /// its `maxElevation` argument is 4 and `min(|dx|, |dy|)` otherwise.
    fn nearest_surface(
        &self,
        x: i32,
        y: i32,
        radius: i32,
        surface: u8,
        manhattan: bool,
    ) -> Option<(i32, i32)> {
        if self.field.surface.is_empty() {
            return None;
        }
        let mut best: Option<(i32, i32, i32)> = None;
        for cy in (y - radius).max(0)..=(y + radius).min(DIM as i32 - 1) {
            for cx in (x - radius).max(0)..=(x + radius).min(DIM as i32 - 1) {
                if self.field.surface_at(cx, cy) != Some(surface) {
                    continue;
                }
                let (dx, dy) = ((cx - x).abs(), (cy - y).abs());
                let d = if manhattan { dx + dy } else { dx.min(dy) };
                // `<=` rather than `<`: the original keeps the *last* equally
                // close cell it finds, which fixes the tie-break to scan order.
                if best.is_none_or(|(b, _, _)| d <= b) {
                    best = Some((d, cx, cy));
                }
            }
        }
        best.map(|(_, cx, cy)| (cx, cy))
    }

    /// The faulty sibling — see [`to_surface5_near`](Self::to_surface5_near).
    fn nearest_surface_from_box_corner(
        &self,
        x: i32,
        y: i32,
        radius: i32,
        surface: u8,
    ) -> Option<(i32, i32)> {
        if self.field.surface.is_empty() {
            return None;
        }
        let (ox, oy) = ((x - radius).max(0), (y - radius).max(0));
        let mut best: Option<(i32, i32, i32)> = None;
        for cy in oy..=(y + radius).min(DIM as i32 - 1) {
            for cx in ox..=(x + radius).min(DIM as i32 - 1) {
                if self.field.surface_at(cx, cy) != Some(surface) {
                    continue;
                }
                let d = (cx - ox).abs() + (cy - oy).abs();
                if best.is_none_or(|(b, _, _)| d < b) {
                    best = Some((d, cx, cy));
                }
            }
        }
        best.map(|(_, cx, cy)| (cx, cy))
    }

    /// `Siege_ClaimDefencePost` (`0x0048ED95`): a twenty-entry reservation
    /// table that stops two defending units posting to the same place. Returns
    /// the cell already reserved for this unit, or reserves the first free
    /// non-empty entry. `0` when the table is full.
    fn claim_defence_post(&mut self, cur: usize) -> usize {
        for i in 0..20 {
            if self.ai.defence_post_owner[i] as usize == cur && cur != 0 {
                return self.field.defence_posts[i];
            }
        }
        for i in 0..20 {
            if self.field.defence_posts[i] != 0 && self.ai.defence_post_owner[i] == 0 {
                self.ai.defence_post_owner[i] = cur as u16;
                return self.field.defence_posts[i];
            }
        }
        0
    }

    /// `Oil_IsOnHighWall`: elevation 2 or more on a cell of surface 6 or above.
    fn oil_is_on_high_wall(&self, cur: usize) -> bool {
        let (x, y) = self.unit_pos(cur);
        self.field.elevation_at(x as i32, y as i32) >= 2
            && self.field.surface_at(x as i32, y as i32).unwrap_or(0) >= 6
    }

    /// `Oil_FindPourTarget` → `Enemy_FindCluster`: nothing below elevation 2;
    /// otherwise the densest enemy cluster within six cells needing three
    /// enemies, or within four needing two once the unit is on the rampart
    /// proper. A genuine "wait until they bunch up under you" rule.
    fn oil_find_pour_target(&self, cur: usize) -> Option<(i16, i16)> {
        let (ux, uy) = self.unit_pos(cur);
        if self.field.elevation_at(ux as i32, uy as i32) < 2 {
            return None;
        }
        let on_rampart = self.field.surface_at(ux as i32, uy as i32).unwrap_or(0) >= 6;
        let (radius, needed) = if on_rampart { (4i32, 2usize) } else { (6i32, 3usize) };
        let mine = self.units.get(cur).owner;
        let mut best: Option<(usize, i32, i32)> = None;
        for cy in (uy as i32 - radius).max(0)..=(uy as i32 + radius).min(DIM as i32 - 1) {
            for cx in (ux as i32 - radius).max(0)..=(ux as i32 + radius).min(DIM as i32 - 1) {
                let mut count = 0usize;
                for (i, f) in self.figures.iter().enumerate() {
                    if !f.is_alive() || f.owner == mine || f.owner == 0 {
                        continue;
                    }
                    let Some(&(x, y)) = self.positions.get(i) else { continue };
                    if (x as i32 - cx).abs() <= 1 && (y as i32 - cy).abs() <= 1 {
                        count += 1;
                    }
                }
                if count >= needed && best.is_none_or(|(b, _, _)| count > b) {
                    best = Some((count, cx, cy));
                }
            }
        }
        best.map(|(_, x, y)| (x as i16, y as i16))
    }

    // --- the think gate ----------------------------------------------------

    /// The skeleton every handler opens with: count the think timer up, and run
    /// only when it passes its interval and (in 13 of the 17) no figure of the
    /// unit is in melee.
    fn may_think(&mut self, cur: usize, interval: i16, melee_gates: bool) -> bool {
        let u = self.units.get_mut(cur);
        u.think = u.think.saturating_add(1);
        if u.think < interval || (melee_gates && u.in_melee) {
            return false;
        }
        u.think = 0;
        true
    }

    /// The tail: `orders += 1`. Two handlers return before reaching it.
    fn advance_script(&mut self, cur: usize) {
        let u = self.units.get_mut(cur);
        u.orders = u.orders.wrapping_add(1);
    }

    /// The liveness test every handler makes on its remembered attacker:
    /// the grudge must be live *and* the attacker must still exist.
    fn live_attacker(&self, cur: usize) -> Option<usize> {
        let u = self.units.get(cur);
        if u.hit_memory == 0 {
            return None;
        }
        let a = u.last_attacker as usize;
        if self.units.owner_of(a) == 0 {
            return None;
        }
        Some(a)
    }
}

// ---------------------------------------------------------------------------
// The seventeen handlers, plus the stub
// ---------------------------------------------------------------------------

type Handler = fn(&mut World, usize);

/// One dispatch slot.
///
/// The name and address are carried rather than inferred so that a slot can be
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

/// `UnitOrder_None` (`0x0048A9C7`): a bare `return`, filling **seven** of the
/// twenty-five slots.
///
/// Which seven is not arbitrary, and it is the strongest cross-check in the
/// whole document: the siege **defender**'s catapult, tower and ram slots are
/// empty, and `g_raiseOrderSiege` independently says a garrison never holds
/// those three troop types. Two unrelated structures agreeing. **[V]**
fn order_none(w: &mut World, cur: usize) {
    // Not even the think timer. The body is `return`.
    w.ai.record(cur, Action::NoThink);
}

/// `UnitOrder_FieldMissile` (`0x0048A9D2`) — category 1, archers and
/// crossbowmen.
///
/// `nearest` is the whole 80-cell field; §2.3's melee units look 8 or 9 cells.
/// The last branch is a **second, independent decision** taken after the first,
/// and it exists only on the cautious side: an AI archer unit that thinks it is
/// winning never backs away.
fn field_missile(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let attacker = w.units.get(cur).last_attacker as usize;
    let nearest = w.nearest_enemy_unit(cur, 80, 1);

    if w.ai.strength_advantage > AGGRESSION_THRESHOLD {
        if w.units.get(cur).hit_memory != 0 && w.units.owner_of(nearest) != 0 {
            w.shoot_at_unit(cur, nearest);
        } else {
            let orders = w.units.get(cur).orders;
            if orders < 10 {
                // Order_DoNothing.
            } else if orders < 0x12 {
                w.halfway_to_unit(cur, nearest);
            } else if orders < 0x15 {
                w.hold_position(cur);
            } else if w.units.owner_of(nearest) != 0 {
                w.halfway_to_unit(cur, nearest);
            }
        }
    } else {
        if w.ai.commit_counter == 0 {
            if w.units.get(cur).hit_memory != 0 && w.units.owner_of(attacker) != 0 {
                w.shoot_at_unit(cur, attacker);
            } else if w.ai.rally_request {
                w.step_toward_rally_point(cur);
            } else if w.units.get(cur).orders % 4 == 0 {
                w.to_rally_waypoint(cur, 0);
            }
        } else {
            w.halfway_to_unit(cur, nearest);
        }
        let u = *w.units.get(cur);
        if u.times_hit > 10
            && w.units.owner_of(attacker) != 0
            && u.firing == 0
            && !u.withdrawing
        {
            w.step_away_from_unit(cur, attacker);
        }
    }
    w.advance_script(cur);
}

/// The shape `UnitOrder_FieldFoot` (`0x0048ACD2`) and `UnitOrder_FieldMelee`
/// (`0x0048B02B`) share. They differ only in four constants.
///
/// **The cautious branch is where the only inter-unit coordination in the game
/// lives.** A melee unit that is losing men backs two cells away from its
/// attacker *and* publishes that attacker's position; the field missile handler
/// reads the request and shifts two cells toward it. That fire-support call is
/// the only message any unit sends to any other.
#[allow(clippy::too_many_arguments)]
fn field_foot_or_melee(
    w: &mut World,
    cur: usize,
    silent_until: i16,
    march_until: i16,
    charge_radius: i32,
    rally_every: i16,
    rally_waypoint: usize,
    withdrawal_limit: u8,
) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let attacker = w.units.get(cur).last_attacker as usize;
    let has_attacker =
        w.units.get(cur).hit_memory != 0 && w.units.owner_of(attacker) != 0;

    if w.ai.strength_advantage > AGGRESSION_THRESHOLD {
        if has_attacker {
            w.onto_unit(cur, attacker);
        } else {
            let orders = w.units.get(cur).orders;
            if orders < march_until {
                if orders >= silent_until {
                    // The march slot is chosen with the **kingdom's season
                    // byte**. In even seasons alternate units take alternate
                    // slots; in odd seasons they all take the same one. This
                    // looks like a free coin flip that happened to be lying
                    // around, and nothing establishes intent.
                    let slot = if w.ai.season & 1 == 0 {
                        (cur & 1) + w.ai.approach_lane
                    } else {
                        w.ai.approach_lane
                    };
                    w.to_enemy_end(cur, slot);
                }
            } else {
                w.charge_nearest(cur);
            }
        }
    } else {
        if has_attacker {
            w.ai.engagement_count += 1;
            if w.ai.engagement_count > w.ai.engagement_budget {
                w.ai.commit_counter += 3;
            } else if w.ai.men_missile < w.ai.men_total / 8 {
                // "We have run out of archers, so go in."
                w.ai.commit_counter += 20;
            } else {
                w.ai.rally_request = true;
                let (ax, ay) = w.unit_pos(attacker);
                w.ai.rally_x = ax as i32;
                w.ai.rally_y = ay as i32;
                w.step_away_from_unit(cur, attacker);
            }
        } else if w.ai.commit_counter != 0 {
            w.ai.commit_counter -= 1;
        }
        if w.ai.commit_counter < 1 {
            w.units.get_mut(cur).halted = false;
        }
        // Charge radius is tiny, and a worn-down enemy unit of fewer than three
        // figures is invisible to the melee AI entirely.
        let nearest = w.nearest_enemy_unit(cur, charge_radius, 3);
        if w.ai.commit_counter != 0 || nearest != 0 {
            w.charge_nearest(cur);
        } else if w.units.get(cur).orders % rally_every == 0
            && w.units.get(cur).withdrawals < withdrawal_limit
        {
            w.to_rally_waypoint(cur, rally_waypoint);
        }
    }
    w.advance_script(cur);
}

/// `UnitOrder_FieldFoot` — category 2, peasants and pikemen.
fn field_foot(w: &mut World, cur: usize) {
    field_foot_or_melee(w, cur, 2, 10, 8, 5, 2, 1);
}

/// `UnitOrder_FieldMelee` — categories 3 **and** 4; both slots hold this one
/// function.
///
/// Note the rally waypoint: `FieldFoot` uses waypoint 2 and `FieldMelee`
/// waypoint 1. `docs/battle-ai.md` §2.3's pseudocode gives both as 2.
fn field_melee(w: &mut World, cur: usize) {
    field_foot_or_melee(w, cur, 4, 13, 9, 8, 1, 2);
}

/// `UnitOrder_SiegeAttMissile` (`0x0048D16E`) — siege attacker, category 1.
fn siege_att_missile(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.ai.rot_missile3 = (w.ai.rot_missile3 + 1) % 3;

    if w.ai.approach_score < 3 || w.ai.breach_score == 0 {
        let orders = w.units.get(cur).orders;
        if orders == 0 {
            w.to_field_corner(cur, 1);
        } else if orders % 15 == 0 {
            w.to_castle_approach(cur, 1);
        } else if orders > 10 {
            if w.ai.approach_score < 10 && w.ai.men_total - w.ai.men_knight <= w.ai.men_missile {
                if w.ai.rot_missile16 < 11 {
                    w.to_breach_or_staging(cur);
                } else {
                    w.to_siege_staging(cur);
                }
                w.ai.rot_missile16 = (w.ai.rot_missile16 + 1) % 16;
            } else if !w.shoot_at_unit(cur, 0) {
                w.to_castle_approach(cur, 1);
            }
        }
    } else if w.ai.men_missile < w.ai.men_total {
        if w.ai.tick_missile < 4 || w.ai.rot_missile3 != 0 {
            if w.ai.rot_missile3 == 0 {
                w.ai.tick_missile += 1;
            } else if !w.shoot_at_unit(cur, 0) {
                w.to_nearest_wall_cell(cur);
            }
        } else if w.field.moat_flag < 1 {
            w.ai.rot_missile_objective += 1;
            if w.ai.rot_missile_objective < 6 {
                w.to_castle_objective(cur, 0);
            } else {
                w.to_nearest_wall_cell(cur);
            }
        } else {
            w.to_castle_objective(cur, 0);
        }
    } else {
        w.to_castle_objective(cur, 0);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttFoot` (`0x0048D412`) — the longest of the order scripts.
///
/// The ladder is 8, 18, 25, 31, 40, 51, 60, 71, alternating two staging
/// positions, with a jump that sets `orders` to **100** outright when the
/// castle layout flag is set — skipping the rest of the approach script.
fn siege_att_foot(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.ai.rot_foot3 = (w.ai.rot_foot3 + 1) % 3;

    if w.ai.approach_score < 3 || w.ai.breach_score == 0 {
        let orders = w.units.get(cur).orders;
        if orders < 8 {
            w.to_field_corner(cur, 0);
        } else if orders < 0x12 {
            w.to_siege_staging(cur);
        } else if orders < 0x19 {
            if w.field.castle_layout_flag {
                w.units.get_mut(cur).orders = 100;
            }
            w.to_breach_or_staging(cur);
        } else if orders < 0x1f {
            w.to_siege_staging(cur);
        } else if orders < 0x28 {
            w.to_breach_or_staging(cur);
        } else if orders < 0x33 {
            w.to_siege_staging(cur);
        } else if orders < 0x3c {
            w.to_breach_or_staging(cur);
        } else if orders < 0x47 {
            w.to_siege_staging(cur);
        } else {
            w.to_breach_or_staging(cur);
        }
    } else if w.ai.strength_advantage < SIEGE_CHARGE_ADVANTAGE {
        if w.ai.tick_foot < 7 || w.ai.rot_foot3 != 0 {
            if w.ai.rot_foot3 == 0 {
                w.ai.tick_foot += 1;
            } else if w.ai.tick_foot < 6 {
                w.to_siege_staging(cur);
            }
        } else if w.ai.breach_score >= 1 || w.ai.siege_engine_count == 0 {
            w.to_castle_objective(cur, 0);
        }
    } else {
        w.charge_nearest(cur);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttMelee` (`0x0048D6FC`) — the same ladder at 15, 25, 30,
/// 41, 50, 61, 70, 81, gated on `orders > 5`, plus a six-phase rotation.
fn siege_att_melee(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.ai.rot_melee6 = (w.ai.rot_melee6 + 1) % 6;

    if w.ai.approach_score < 3 || w.ai.breach_score == 0 {
        let orders = w.units.get(cur).orders;
        if orders > 5 {
            if orders < 0xf {
                w.to_field_corner(cur, 0);
            } else if orders < 0x19 {
                w.to_siege_staging(cur);
            } else if orders < 0x1e {
                if w.field.castle_layout_flag {
                    w.units.get_mut(cur).orders = 100;
                }
                w.to_breach_or_staging(cur);
            } else if orders < 0x29 {
                w.to_siege_staging(cur);
            } else if orders < 0x32 {
                w.to_breach_or_staging(cur);
            } else if orders < 0x3d {
                w.to_siege_staging(cur);
            } else if orders < 0x46 {
                w.to_breach_or_staging(cur);
            } else if orders < 0x51 {
                w.to_siege_staging(cur);
            } else {
                w.to_breach_or_staging(cur);
            }
        }
    } else if w.ai.rot_melee6 == 0 || w.ai.rot_melee6 == 3 {
        if w.ai.breach_score >= 1 || w.ai.siege_engine_count == 0 {
            w.to_castle_objective(cur, 0);
        }
    } else if w.ai.rot_melee6 == 5 {
        w.hold_position(cur);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttKnight` (`0x0048D9CE`).
///
/// # It is also **the only thing in the binary that ends a battle by giving up**
///
/// ```c
/// if ((g_aiMenTotal <= g_aiMenKnight) && (g_siegeBreachScore == 0)) {
///     g_battleWithdrawal = 1;
///     DAT_005656f8 = g_battleUnits[g_curBattleUnit].owner;
/// }
/// ```
///
/// — three statements at the top of the think, above the movement ladder and
/// **not** in an `else`, so the unit raises the flag and then goes on issuing
/// its order for the frame. `g_aiMenTotal` and `g_aiMenKnight` are
/// [`Ai::count_men`]'s totals over the AI's own living troops of type 0…6, so
/// `total <= knights` is *"every man I have left is a knight"* — an all-cavalry
/// besieger in front of an unbreached wall, which is a siege it cannot win.
///
/// `Battle_CheckOutcome` tests [`Ai::withdrawal`] **before** either men
/// counter, so this outranks annihilation; and `Battle_ReturnToCampaign` then
/// charges the loser [`Army_WithdrawCasualties`](../../l2_kingdom/battle/fn.withdraw_casualties.html)
/// — half of every troop line — before deciding whether fifty men are left.
///
/// **This clause was missing, and its absence was load-bearing.** Nothing else
/// raises the flag, so with it absent `End::Withdrawal` could not arise in a
/// played game, the whole withdrawal half of the campaign seam was unreachable,
/// and the campaign's own missing `Army_WithdrawCasualties` could not be
/// noticed. `docs/decisions.md` CNEW-withdrawal.
fn siege_att_knight(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if w.ai.men_total <= w.ai.men_knight && w.ai.breach_score == 0 {
        w.ai.withdrawal = Some(w.units.get(cur).side);
    }
    w.ai.rot_knight3 = (w.ai.rot_knight3 + 1) % 3;

    if w.ai.approach_score < 3 || w.ai.breach_score == 0 {
        let orders = w.units.get(cur).orders;
        if orders < 10 {
            w.to_field_corner(cur, 3);
        } else if orders < 0x1f {
            w.to_castle_approach(cur, 3);
        }
    } else if w.ai.rot_knight3 == 0 && (w.ai.breach_score >= 1 || w.ai.siege_engine_count == 0) {
        w.to_castle_objective(cur, 0);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttCatapult` (`0x0048DB84`) — **no in-melee gate**, so it
/// keeps thinking while engaged.
///
/// The search radius is 20 cells, which is *exactly* the catapult's firing
/// range in `g_missileStats` (160 eighths of a cell). Two unrelated constants
/// in the binary agreeing is the strongest check available here. **[V]**
///
/// Note it `return`s on the wall-found path *before* the `orders` increment, so
/// a catapult doing its job stops advancing its script.
// Two ladder rungs issue the same order from different conditions, as the
// original does. Collapsing them would lose the correspondence.
#[allow(clippy::if_same_then_else)]
fn siege_att_catapult(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, false) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let orders = w.units.get(cur).orders;
    if orders > 3 {
        if orders < 0xd {
            w.to_field_corner(cur, 5);
        } else if orders < 0x15 {
            w.to_castle_approach(cur, 5);
        } else if orders % 100 == 0 {
            w.to_castle_approach(cur, 5);
        } else if orders > 0x1e {
            let (first, last) = {
                let u = w.units.get(cur);
                (u.first as usize, u.last as usize)
            };
            for f in first..=last.min(w.figures.len().saturating_sub(1)) {
                if !w.figures[f].is_alive() || w.figures[f].unit as usize != cur {
                    continue;
                }
                let Some(&(x, y)) = w.positions.get(f) else { continue };
                match w.nearest_surface(x as i32, y as i32, 20, 4, false) {
                    Some((cx, cy)) => w.to_cell(cur, cy as usize * DIM + cx as usize),
                    None => w.to_castle_approach(cur, 5),
                }
                return;
            }
        }
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttTower` (`0x0048DDC7`) — no in-melee gate either.
///
/// Once `approach_score` passes 6 after think 60, or passes 10 at any time, it
/// widens a search from radius **10 to 30** for a surface-4 cell. `L2.eng`
/// 214/2 is the other half of the catapult's sentence: *"Battering rams and
/// siege towers must be moved right up to the castle wall to work."* **[V]**
fn siege_att_tower(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, false) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let orders = w.units.get(cur).orders;
    if orders < 0xb {
        w.to_field_corner(cur, 3);
    } else if (orders > 0x3c && w.ai.approach_score > 6) || w.ai.approach_score > 10 {
        let (first, last) = {
            let u = w.units.get(cur);
            (u.first as usize, u.last as usize)
        };
        for f in first..=last.min(w.figures.len().saturating_sub(1)) {
            if !w.figures[f].is_alive() || w.figures[f].unit as usize != cur {
                continue;
            }
            let Some(&(x, y)) = w.positions.get(f) else { continue };
            for radius in 10..=30 {
                if let Some((cx, cy)) = w.nearest_surface(x as i32, y as i32, radius, 4, false) {
                    w.to_cell(cur, cy as usize * DIM + cx as usize);
                    return;
                }
            }
            w.to_castle_approach(cur, 3);
            return;
        }
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeAttRam` (`0x0048DFBB`) — ignores `orders` entirely and picks
/// between its stored second destination and a castle approach point from three
/// flags. No in-melee gate.
fn siege_att_ram(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, false) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    if !w.field.castle_layout_flag && w.ai.approach_score < 8 {
        w.to_second_target(cur);
    } else if !w.ai.drawbridge_down {
        w.to_castle_approach(cur, 2);
    } else {
        w.to_second_target(cur);
    }
    w.advance_script(cur);
}

/// The opening every "real" siege defender shares: above the sortie threshold
/// it lowers the drawbridge. Returns true when the sortie is on.
///
/// The threshold is 260, so a garrison sallies out only when it believes it is
/// 3.6× the attacker's strength. **[V]** on the number; the drawbridge routine
/// demonstrably paints 7 × 4 cells and bumps both progress counters — whether
/// that is a drawbridge, a sally port or a siege ramp is **[I]**.
fn sortie(w: &mut World, _cur: usize) -> bool {
    if w.ai.strength_advantage > SORTIE_THRESHOLD {
        if !w.ai.drawbridge_down {
            w.ai.drawbridge_down = true;
            w.ai.approach_score += 4;
            w.ai.breach_score += 4;
        }
        true
    } else {
        false
    }
}

/// `UnitOrder_SiegeDefMissile` (`0x0048E097`) — nine wall slots, every other
/// think, while fewer than three attackers stand on the wall.
///
/// **Its sortie branch does nothing at all**, and that is faithful, not a stub:
/// the original discards the `Enemy_NearestUnit` result and then calls
/// `Order_HalfwayToUnit` on the unit's *own* index, so both axis separations
/// are zero and it returns without writing anything. It reads as a missing
/// assignment. Reproduced, because "fixing" it would change observable
/// behaviour — and it is unreachable in practice anyway, behind a 260 % gate.
fn siege_def_missile(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if sortie(w, cur) {
        let _discarded = w.nearest_enemy_unit(cur, 80, 1);
        w.halfway_to_unit(cur, cur);
    } else if !w.field.castle_layout_flag || w.ai.approach_score == 0 {
        if w.ai.attackers_on_wall < 3 {
            if w.units.get(cur).orders % 2 == 0 && w.units.get(cur).firing == 0 {
                let slot = w.ai.wall_slot_cursor;
                w.to_wall_slot(cur, 1, slot);
                w.ai.wall_slot_cursor = if slot >= 8 { 0 } else { slot + 1 };
            }
        } else {
            w.to_castle_objective(cur, 0);
        }
    } else {
        w.to_castle_objective(cur, 1);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefWallMissileA` (`0x0048E234`) — category 9, the first
/// missile unit a garrison raises.
///
/// It does **nothing but count `orders` up**, so that unit holds whatever wall
/// slot it was deployed on for the entire battle. Not a stub: the original body
/// is the think gate and the increment, and nothing else.
fn siege_def_wall_missile_a(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefWallMissileB` (`0x0048E2C8`) — category 10, the second.
fn siege_def_wall_missile_b(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if !w.field.castle_layout_flag || w.ai.approach_score < 4 {
        if w.ai.attackers_on_wall < 3 {
            w.to_wall_near_target(cur, true);
        } else {
            w.to_wall_below_keep(cur);
        }
    } else {
        w.to_wall_below_keep(cur);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefFoot` (`0x0048E39C`) — thinks every **100** frames and
/// has **no** in-melee gate, so it keeps re-deciding while fighting.
///
/// Its `Enemy_NearestUnit(unit, 10, 0)` result is discarded, the same missing
/// assignment as in `SiegeDefMissile`. Reproduced for the same reason.
fn siege_def_foot(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL_FAST, false) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if w.live_attacker(cur).is_none() {
        let _discarded = w.nearest_enemy_unit(cur, 10, 0);
    }
    if sortie(w, cur) {
        w.charge_nearest(cur);
    } else {
        w.to_castle_objective(cur, 1);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefMelee` (`0x0048E4CC`) — thinks every 100 frames.
///
/// Reserves a defence post, moves onto an attacking unit that has reached the
/// wall, and otherwise rotates four inner slots every fifteenth think —
/// abandoning all of it for the castle objective once four attackers are up.
fn siege_def_melee(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL_FAST, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    let attacker = w.units.get(cur).last_attacker as usize;
    // 0 = no live attacker, 1 = attacker on low ground, 2 = attacker on the
    // wall. The threshold is the attacker unit's own cell surface.
    let mut attacker_state = 0u8;
    if w.live_attacker(cur).is_some() {
        let (ax, ay) = w.unit_pos(attacker);
        attacker_state = if w.field.surface_at(ax as i32, ay as i32).unwrap_or(0) < 4 {
            1
        } else {
            2
        };
    }
    let post = w.claim_defence_post(cur);

    if sortie(w, cur) {
        w.charge_nearest(cur);
    } else if attacker_state == 2 {
        w.onto_unit(cur, attacker);
    } else if !w.field.castle_layout_flag || w.ai.approach_score == 0 {
        if w.ai.attackers_on_wall >= 6 {
            w.to_castle_objective(cur, 1);
        } else if w.ai.attackers_on_wall >= 4 {
            w.to_wall_slot(cur, 2, 0);
        } else if post != 0 && attacker_state == 1 {
            w.to_surface5_near(cur, post);
        } else if w.units.get(cur).orders % 15 == 0 {
            if post == 0 {
                let slot = w.ai.inner_slot_cursor;
                w.to_wall_slot(cur, 0, slot);
                w.ai.inner_slot_cursor = if slot >= 3 { 0 } else { slot + 1 };
            } else {
                w.to_cell(cur, post);
            }
        }
    } else {
        w.to_castle_objective(cur, 1);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefKnight` (`0x0048E774`) — the most passive of the
/// defenders: wall-slot group 2 every twentieth think until a single attacker
/// reaches the wall.
fn siege_def_knight(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if sortie(w, cur) {
        w.charge_nearest(cur);
    } else if !w.field.castle_layout_flag || w.ai.approach_score == 0 {
        if w.ai.attackers_on_wall < 1 {
            if w.units.get(cur).orders % 20 == 0 {
                w.to_wall_slot(cur, 2, 0);
            }
        } else {
            w.to_castle_objective(cur, 1);
        }
    } else {
        w.to_castle_objective(cur, 1);
    }
    w.advance_script(cur);
}

/// `UnitOrder_SiegeDefOil` (`0x0048E8B8`) — the most specific handler in the
/// set. It moves only if its unit is at elevation ≥ 2, and then only to the
/// densest cluster of enemies within six cells (needing three) or four (needing
/// two, on the rampart proper).
// Three of the fall-back branches end in the same order, as the original does.
#[allow(clippy::if_same_then_else)]
fn siege_def_oil(w: &mut World, cur: usize) {
    if !w.may_think(cur, THINK_INTERVAL, true) {
        w.ai.record(cur, Action::NoThink);
        return;
    }
    w.ai.record(cur, Action::DoNothing);
    if let Some((x, y)) = w.oil_find_pour_target(cur) {
        w.set_target(cur, x, y);
        w.ai.record(cur, Action::PourOil);
    } else if w.ai.attackers_on_wall >= 2 {
        w.to_wall_below_keep(cur);
    } else if w.field.castle_layout_flag && w.ai.approach_score != 0 {
        w.to_wall_below_keep(cur);
    } else if !w.oil_is_on_high_wall(cur) {
        if w.units.get(cur).orders % 3 == 0 && w.units.get(cur).hit_memory != 0 {
            w.to_wall_below_keep(cur);
        } else {
            w.to_wall_near_target(cur, false);
        }
    }
    w.advance_script(cur);
}

// ---------------------------------------------------------------------------
// The three dispatch tables
// ---------------------------------------------------------------------------

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
/// **outside** the human-control guard — which is why a player's units reform
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{SIDE_A, SIDE_B};
    use crate::unit::CATEGORY_OF_TROOP;

    /// A two-unit field battle: one AI unit of `ai_troop`, one human unit of
    /// `human_troop`, twenty cells apart.
    struct Fixture {
        units: Units,
        figures: Vec<Figure>,
        positions: Vec<(u8, u8)>,
        field: AiField,
        ai: Ai,
        pub ai_unit: usize,
        pub human_unit: usize,
    }

    impl Fixture {
        fn new(ai_troop: Troop, human_troop: Troop, n: usize) -> Self {
            let mut units = Units::new();
            let mut figures = Vec::new();
            let mut positions = Vec::new();
            let ai_unit = units
                .create(1, false, SIDE_B, CATEGORY_OF_TROOP[ai_troop.index()])
                .unwrap();
            let human_unit = units
                .create(2, true, SIDE_A, CATEGORY_OF_TROOP[human_troop.index()])
                .unwrap();
            for i in 0..n {
                let mut f = Figure::new(ai_troop, SIDE_B, 8);
                f.unit = ai_unit as u16;
                f.owner = 1;
                figures.push(f);
                positions.push((40 + i as u8, 50));
            }
            for i in 0..n {
                let mut f = Figure::new(human_troop, SIDE_A, 8);
                f.unit = human_unit as u16;
                f.owner = 2;
                f.owner_is_human = true;
                figures.push(f);
                positions.push((40 + i as u8, 30));
            }
            let mut fx = Fixture {
                units,
                figures,
                positions,
                field: AiField::field((40, 20), (40, 60)),
                ai: Ai::new(0x5EED),
                ai_unit,
                human_unit,
            };
            fx.units.rebuild_from_figures(&mut fx.figures);
            fx.units.recentre(ai_unit, &fx.figures, &fx.positions);
            fx.units.recentre(human_unit, &fx.figures, &fx.positions);
            fx
        }

        /// Run one whole think cycle with a chosen strength advantage.
        ///
        /// The number is written on the last frame before the handler reads it,
        /// which is the only way to hold it steady: the recompute would
        /// otherwise overwrite it every 101 frames. `multiplayer` is set so
        /// the jitter cannot move it either.
        fn think_at(&mut self, advantage: i32) -> Action {
            self.ai.multiplayer = true;
            for _ in 0..(THINK_INTERVAL - 1) {
                self.frame();
            }
            self.ai.strength_advantage = advantage;
            self.frame();
            self.action()
        }

        fn think_at_n(&mut self, advantage: i32, n: usize) -> Vec<Action> {
            (0..n).map(|_| self.think_at(advantage)).collect()
        }

        fn frame(&mut self) -> Vec<usize> {
            self.units.rebuild_from_figures(&mut self.figures);
            update_all_units(
                &mut self.units,
                &mut self.figures,
                &self.positions,
                &self.field,
                &mut self.ai,
            )
        }

        fn frames(&mut self, n: usize) {
            for _ in 0..n {
                self.frame();
            }
        }

        fn action(&self) -> Action {
            self.ai.last_action[self.ai_unit]
        }
    }

    // --- the think gate ----------------------------------------------------

    /// A unit decides once every two hundred frames. Nothing else in the battle
    /// is that slow.
    #[test]
    fn a_unit_thinks_once_every_two_hundred_frames_and_not_before() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        for _ in 0..199 {
            fx.frame();
            assert_eq!(fx.action(), Action::NoThink, "thought early");
            assert_eq!(fx.units.get(fx.ai_unit).orders, 0);
        }
        fx.frame();
        assert_ne!(fx.action(), Action::NoThink, "should have thought on frame 200");
        assert_eq!(fx.units.get(fx.ai_unit).orders, 1, "one think, one script step");
        for _ in 0..199 {
            fx.frame();
            assert_eq!(fx.units.get(fx.ai_unit).orders, 1, "and not again until 400");
        }
        fx.frame();
        assert_eq!(fx.units.get(fx.ai_unit).orders, 2);
    }

    /// Thirteen of the seventeen handlers refuse to run while any figure of the
    /// unit is in melee. Once the AI's line makes contact, most of the AI stops
    /// manoeuvring.
    #[test]
    fn a_unit_in_melee_stops_thinking_entirely() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        fx.figures[0].state = State::Melee;
        for _ in 0..1_000 {
            fx.frame();
            assert_eq!(fx.action(), Action::NoThink);
        }
        assert_eq!(fx.units.get(fx.ai_unit).orders, 0, "five thinks' worth of frames, none taken");
        // And it resumes the moment the melee ends.
        fx.figures[0].state = State::Idle;
        fx.frames(200);
        assert!(fx.units.get(fx.ai_unit).orders > 0);
    }

    /// The four exceptions, which is why "13 of 17" is worth pinning: the two
    /// siege engines and the ram keep thinking while engaged, and so does the
    /// siege defender's foot.
    #[test]
    fn four_handlers_do_not_gate_on_melee() {
        for (troop, side) in [
            (Troop::Catapults, SIDE_B),
            (Troop::SiegeTowers, SIDE_B),
            (Troop::BatteringRams, SIDE_B),
            (Troop::Pikemen, SIDE_A),
        ] {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
            fx.ai.is_siege = true;
            fx.ai.multiplayer = true;
            let u = fx
                .units
                .create(3, false, side, CATEGORY_OF_TROOP[troop.index()])
                .unwrap();
            let mut f = Figure::new(troop, side, 4);
            f.unit = u as u16;
            f.owner = 3;
            f.state = State::Melee;
            fx.figures.push(f);
            fx.positions.push((30, 40));
            fx.frames(250);
            assert!(
                fx.units.get(u).orders > 0,
                "{troop:?} should keep thinking in melee"
            );
        }
    }

    /// `SiegeDefFoot` and `SiegeDefMelee` think twice as often as everybody
    /// else.
    #[test]
    fn the_two_fast_siege_defenders_think_every_hundred_frames() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        let foot = fx.units.create(3, false, SIDE_A, 2).unwrap();
        let melee = fx.units.create(3, false, SIDE_A, 3).unwrap();
        for (i, u) in [foot, melee].into_iter().enumerate() {
            let mut f = Figure::new(Troop::Pikemen, SIDE_A, 4);
            f.unit = u as u16;
            f.owner = 3;
            fx.figures.push(f);
            fx.positions.push((30 + i as u8, 40));
        }
        fx.frames(100);
        assert_eq!(fx.units.get(foot).orders, 1);
        assert_eq!(fx.units.get(melee).orders, 1);
        // The 200-frame handlers have not moved at all yet.
        assert_eq!(fx.units.get(fx.ai_unit).orders, 0);
    }

    #[test]
    fn a_human_controlled_unit_is_never_given_an_order() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        fx.frames(2_000);
        assert_eq!(fx.units.get(fx.human_unit).orders, 0);
        assert_eq!(fx.ai.last_action[fx.human_unit], Action::NoThink);
        assert!(fx.units.get(fx.ai_unit).orders > 0, "but the AI's did");
    }

    // --- the one number ----------------------------------------------------

    #[test]
    fn the_strength_advantage_is_weighted_ai_men_over_weighted_human_men() {
        let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
        fx.ai.multiplayer = true;
        fx.ai.update_strength_advantage(&fx.figures);
        // 4 figures x 8 men: knights weigh 4, peasants 1, so 128 v 32 = 400 %.
        assert_eq!(fx.ai.strength_advantage, 300);

        let mut even = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        even.ai.multiplayer = true;
        even.ai.update_strength_advantage(&even.figures);
        assert_eq!(even.ai.strength_advantage, 0, "identical armies are even");
    }

    /// An army with no living enemy reads −100 rather than dividing by zero.
    #[test]
    fn an_unopposed_army_reads_minus_one_hundred() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        for f in fx.figures.iter_mut().filter(|f| f.owner_is_human) {
            f.take_hits(60_000);
        }
        fx.ai.update_strength_advantage(&fx.figures);
        assert_eq!(fx.ai.strength_advantage, -100);
    }

    /// The jitter is **−10 to +21**, not ±10 and not ±16. Asymmetric because
    /// the original is `(rand & 0x1F) - 10`, and reproduced rather than centred:
    /// with the threshold at 5 the half-point bias toward attacking is a real
    /// behaviour.
    #[test]
    fn the_jitter_spans_minus_ten_to_plus_twenty_one() {
        let mut ai = Ai::new(0x1234_5678);
        let figures: Vec<Figure> = Vec::new();
        let (mut low, mut high) = (i32::MAX, i32::MIN);
        let mut seen = [false; 32];
        for _ in 0..5_000 {
            ai.update_strength_advantage(&figures);
            // No figures at all: pct_of(0, 0) is 0, so the base is -100 and
            // everything after it is the jitter.
            let j = ai.strength_advantage + 100;
            low = low.min(j);
            high = high.max(j);
            seen[(j + 10) as usize] = true;
        }
        assert_eq!(low, -10);
        assert_eq!(high, 21);
        assert!(seen.iter().all(|s| *s), "every one of the 32 values should occur");
    }

    /// Determinism is the property lockstep depends on, and the jitter is the
    /// only place a battle draws a random number at all.
    #[test]
    fn the_same_seed_gives_the_same_jitter_and_a_different_seed_does_not() {
        let roll = |seed: u64| {
            let mut ai = Ai::new(seed);
            let figures: Vec<Figure> = Vec::new();
            (0..50)
                .map(|_| {
                    ai.update_strength_advantage(&figures);
                    ai.strength_advantage
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(roll(7), roll(7));
        assert_ne!(roll(7), roll(8));
    }

    /// The multiplayer flag drops the jitter entirely, which is the
    /// original's own answer to "this is a networked game".
    #[test]
    fn the_multiplayer_flag_removes_the_jitter_and_the_draw() {
        let mut ai = Ai::new(99);
        ai.multiplayer = true;
        let before = ai.rng.clone();
        let figures: Vec<Figure> = Vec::new();
        for _ in 0..20 {
            ai.update_strength_advantage(&figures);
            assert_eq!(ai.strength_advantage, -100, "no jitter at all");
        }
        assert_eq!(ai.rng, before, "and the generator was not advanced");
    }

    #[test]
    fn the_advantage_is_recomputed_every_hundred_and_first_frame() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        let start = fx.ai.rng.clone();
        for _ in 0..100 {
            fx.frame();
        }
        assert_eq!(fx.ai.rng, start, "not yet");
        fx.frame();
        assert_ne!(fx.ai.rng, start, "the hundred-and-first frame draws");
    }

    // --- the field handlers ------------------------------------------------

    /// Above the aggression threshold a foot unit marches on the enemy's
    /// deployment marker for ten thinks, and then charges — which drops every
    /// figure into free pursuit and *stops the unit being a formation*.
    #[test]
    fn an_aggressive_foot_unit_marches_for_ten_thinks_and_then_charges() {
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        let actions = fx.think_at_n(50, 12); // comfortably above 5
        assert_eq!(actions[0], Action::DoNothing, "the first two thinks are silent");
        assert_eq!(actions[1], Action::DoNothing);
        assert!(
            matches!(actions[2], Action::ToEnemyEnd(_)),
            "think 2 starts the march: {:?}",
            actions[2]
        );
        assert!(matches!(actions[9], Action::ToEnemyEnd(_)), "still marching at think 9");
        assert_eq!(actions[10], Action::Charge, "think 10 charges");
        assert!(fx.units.get(fx.ai_unit).halted, "and the unit is halted");
        assert!(fx
            .figures
            .iter()
            .filter(|f| f.unit as usize == fx.ai_unit)
            .all(|f| f.state == State::Chasing));
    }

    /// Melee units march three thinks longer than foot units and are silent for
    /// four rather than two. The two functions differ only in constants.
    #[test]
    fn a_melee_unit_marches_thirteen_thinks_rather_than_ten() {
        let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
        let actions = fx.think_at_n(50, 15);
        assert_eq!(actions[3], Action::DoNothing, "silent through think 3");
        assert!(matches!(actions[4], Action::ToEnemyEnd(_)));
        assert!(matches!(actions[12], Action::ToEnemyEnd(_)), "still marching at 12");
        assert_eq!(actions[13], Action::Charge);
    }

    /// **The aggression threshold is 5, and it is the whole field AI.** The
    /// same unit in the same position attacks or does not, on one number.
    #[test]
    fn five_is_the_line_between_marching_on_the_enemy_and_falling_back() {
        let run = |advantage: i32| {
            let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
            fx.think_at_n(advantage, 6)
        };
        let bold = run(AGGRESSION_THRESHOLD + 1);
        let timid = run(AGGRESSION_THRESHOLD);
        assert!(
            bold.iter().any(|a| matches!(a, Action::ToEnemyEnd(_))),
            "at 6 it marches on the enemy: {bold:?}"
        );
        assert!(
            !timid.iter().any(|a| matches!(a, Action::ToEnemyEnd(_))),
            "at exactly 5 it does not: {timid:?}"
        );
        assert!(
            timid.iter().any(|a| matches!(a, Action::ToRallyWaypoint(_))),
            "it falls back on a rally waypoint instead: {timid:?}"
        );
    }

    /// The field handlers' rally waypoints differ, which
    /// `docs/battle-ai.md` §2.3's pseudocode does not say: foot units go to
    /// waypoint 2 and melee units to waypoint 1.
    #[test]
    fn foot_and_melee_units_rally_on_different_waypoints() {
        let waypoint = |troop: Troop| {
            let mut fx = Fixture::new(troop, Troop::Peasants, 4);
            for _ in 0..40 {
                if let Action::ToRallyWaypoint(k) = fx.think_at(0) {
                    return Some(k);
                }
            }
            None
        };
        assert_eq!(waypoint(Troop::Peasants), Some(2));
        assert_eq!(waypoint(Troop::Knights), Some(1));
    }

    /// The charge radius is tiny, and a unit of fewer than three figures is
    /// invisible to it: `Enemy_NearestUnit(unit, 8, 3)`.
    #[test]
    fn a_worn_down_enemy_unit_becomes_invisible_to_the_melee_ai() {
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        // Put the human unit right on top of the AI unit.
        let human: Vec<usize> = fx
            .figures
            .iter()
            .enumerate()
            .filter(|(_, f)| f.owner == 2)
            .map(|(i, _)| i)
            .collect();
        for i in &human {
            fx.positions[*i] = (41, 50);
        }
        assert_eq!(fx.think_at(0), Action::Charge, "four figures at one cell is a target");

        let mut worn = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        for i in &human {
            worn.positions[*i] = (41, 50);
        }
        // Two figures left of four: below the three-figure floor.
        let doomed: Vec<usize> = worn
            .figures
            .iter()
            .enumerate()
            .filter(|(_, f)| f.owner == 2)
            .map(|(i, _)| i)
            .take(2)
            .collect();
        for i in doomed {
            worn.figures[i].take_hits(60_000);
        }
        assert_ne!(worn.think_at(0), Action::Charge, "two figures is not a target");
    }

    /// The only inter-unit message in the game: a melee unit that is being hit
    /// backs off *and* publishes its attacker's position, and a missile unit
    /// answers by shifting two cells toward it.
    #[test]
    fn a_melee_unit_under_attack_calls_the_archers_onto_its_attacker() {
        // Peasants, so this is `UnitOrder_FieldFoot`. The choice matters: the
        // cautious branch takes a *second* decision after the withdrawal, and
        // its rally is gated on `withdrawals == 0` — which the withdrawal has
        // just made false. `UnitOrder_FieldMelee` allows two withdrawals, so
        // there the rally runs and overwrites the destination the step-away
        // wrote. Both are the original; only the constant differs.
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        fx.ai.engagement_budget = 100; // stay out of the two escalation branches
        fx.ai.men_missile = 100;
        fx.ai.men_total = 100;
        fx.ai.multiplayer = true;

        // The grudge lasts 50 frames, so it has to be created inside the last
        // 50 of the 200 the unit waits.
        for _ in 0..(THINK_INTERVAL - 10) {
            fx.frame();
        }
        fx.figures[0].was_hit = true;
        fx.figures[0].hit_by = Some(4);
        fx.ai.strength_advantage = 0;
        for _ in 0..10 {
            fx.frame();
        }

        assert_eq!(fx.action(), Action::StepAwayFromUnit(fx.human_unit));
        assert!(fx.ai.rally_request, "and the request went out");
        assert_eq!((fx.ai.rally_x, fx.ai.rally_y), (41, 30));
        assert!(fx.units.get(fx.ai_unit).withdrawing);
        assert_eq!(fx.units.get(fx.ai_unit).withdrawals, 1);
    }

    /// `g_aiCommitCounter` is an escalation, not a hold: while it is non-zero
    /// **every** AI melee unit charges regardless of what is near it.
    #[test]
    fn the_commit_counter_makes_every_melee_unit_charge_regardless() {
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        // Twenty cells apart, so nothing is within the eight-cell radius.
        assert_ne!(fx.think_at(0), Action::Charge, "nothing in reach");

        // Two, not one: an unengaged unit decays the counter *before* reading
        // it, so a counter of one is already spent by the time it is tested.
        fx.ai.commit_counter = 2;
        assert_eq!(fx.think_at(0), Action::Charge, "the counter overrides distance");
        assert_eq!(fx.ai.commit_counter, 1, "and it decays by one per unengaged think");
    }

    /// Running out of archers raises the counter by twenty — "we have run out
    /// of archers, so go in" — and being over the engagement budget by three.
    #[test]
    fn the_two_escalations_raise_the_counter_by_three_and_by_twenty() {
        let escalate = |budget: i32, missile: i32| {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Peasants, 4);
            fx.ai.multiplayer = true;
            fx.ai.engagement_budget = budget;
            fx.ai.men_total = 800;
            fx.ai.men_missile = missile;
            for _ in 0..(THINK_INTERVAL - 10) {
                fx.frame();
            }
            fx.figures[0].was_hit = true;
            fx.figures[0].hit_by = Some(4);
            fx.ai.strength_advantage = 0;
            for _ in 0..10 {
                fx.frame();
            }
            fx.ai.commit_counter
        };
        assert_eq!(escalate(0, 800), 3, "over the engagement budget");
        assert_eq!(escalate(100, 0), 20, "no archers left");
    }

    /// An AI archer unit that thinks it is winning never backs away: the
    /// retreat is a second decision that exists only on the cautious branch.
    #[test]
    fn a_winning_archer_unit_never_backs_away_however_often_it_is_hit() {
        let hit_ten_times = |advantage: i32| {
            let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 4);
            fx.ai.multiplayer = true;
            for _ in 0..(THINK_INTERVAL - 20) {
                fx.frame();
            }
            for _ in 0..12 {
                fx.figures[0].was_hit = true;
                fx.figures[0].hit_by = Some(4);
                fx.frame();
            }
            assert!(fx.units.get(fx.ai_unit).times_hit > 10);
            fx.ai.strength_advantage = advantage;
            for _ in 0..8 {
                fx.frame();
            }
            fx.action()
        };
        assert_eq!(
            hit_ten_times(0),
            Action::StepAwayFromUnit(2),
            "the cautious archer withdraws"
        );
        assert_ne!(
            hit_ten_times(50),
            Action::StepAwayFromUnit(2),
            "the confident one does not"
        );
    }

    /// A missile unit looks over the whole eighty-cell field; a melee unit
    /// looks eight or nine cells.
    #[test]
    fn the_search_radius_is_the_whole_field_for_missiles_and_a_few_cells_for_melee() {
        let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 4);
        let world = World {
            units: &mut fx.units,
            figures: &mut fx.figures,
            positions: &fx.positions,
            field: &fx.field,
            ai: &mut fx.ai,
        };
        assert_eq!(world.nearest_enemy_unit(1, 80, 1), 2, "twenty cells is within eighty");
        assert_eq!(world.nearest_enemy_unit(1, 9, 3), 0, "and outside nine");
        assert_eq!(world.nearest_enemy_unit(1, 20, 1), 2, "exactly twenty is inside");
        assert_eq!(world.nearest_enemy_unit(1, 19, 1), 0);
    }

    /// `Order_HalfwayToUnit` does nothing until an axis is separated by eight,
    /// and then moves only the axes separated by six. This is why AI archers
    /// advance in stages and then stop.
    #[test]
    fn halfway_to_unit_does_nothing_below_eight_cells_of_separation() {
        let close = |gap: u8| {
            let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 1);
            fx.positions[0] = (40, 50);
            fx.positions[1] = (40, 50 - gap);
            fx.units.rebuild_from_figures(&mut fx.figures);
            fx.units.recentre(1, &fx.figures, &fx.positions);
            fx.units.recentre(2, &fx.figures, &fx.positions);
            fx.units.get_mut(1).target_x = 40;
            fx.units.get_mut(1).target_y = 50;
            let mut world = World {
                units: &mut fx.units,
                figures: &mut fx.figures,
                positions: &fx.positions,
                field: &fx.field,
                ai: &mut fx.ai,
            };
            world.halfway_to_unit(1, 2);
            (fx.units.get(1).target_x, fx.units.get(1).target_y)
        };
        assert_eq!(close(7), (40, 50), "seven cells: nothing happens at all");
        assert_eq!(close(8), (40, 46), "eight: halve it");
        assert_eq!(close(20), (40, 40));
    }

    // --- the siege handlers ------------------------------------------------

    /// **The one thing in `Lords2.exe` that ends a battle by giving up.**
    ///
    /// `UnitOrder_SiegeAttKnight` is the only writer of `g_battleWithdrawal`
    /// (`0x0056D5C8`) in the whole binary, and the clause was missing here —
    /// which made [`crate::End::Withdrawal`] unreachable in a played game and
    /// with it the whole withdrawal half of the campaign seam, including
    /// `Army_WithdrawCasualties`, which `l2-kingdom` had never implemented
    /// because nothing could reach it. `docs/decisions.md` CNEW-withdrawal.
    ///
    /// Three cases, and the third is the one the two conditions are for:
    /// `g_aiMenTotal <= g_aiMenKnight` is *"every AI man still standing is a
    /// knight"*, over `Battle_CountMenByType`'s census of **both** sides'
    /// non-human figures.
    #[test]
    fn an_all_knight_besieger_at_an_unbreached_wall_leaves_the_field() {
        let raised = |breach: i32, dismount: bool| {
            let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
            fx.ai.is_siege = true;
            if dismount {
                // One man of the besieging force who is not a knight, which is
                // the whole of what `total <= knights` asks about.
                fx.figures[0].troop = Troop::Peasants;
            }
            fx.ai.count_men(&fx.figures);
            assert!(fx.ai.men_total > 0, "the census sees the AI's own men");
            fx.ai.breach_score = breach;
            assert_eq!(fx.ai.withdrawal, None, "nothing before the unit has thought");
            fx.think_at(0);
            fx.ai.withdrawal
        };
        assert_eq!(raised(0, false), Some(SIDE_B), "all knights, nothing breached");
        assert_eq!(raised(4, false), None, "a breach is a reason to stay");
        assert_eq!(raised(0, true), None, "and so is one man who can climb");
    }

    /// The three defender stubs at categories 5, 6 and 7 are the catapult, the
    /// siege tower and the ram — precisely the three troop types
    /// `g_raiseOrderSiege` says a garrison never has.
    #[test]
    fn a_garrison_has_no_handler_for_the_three_troops_it_never_raises() {
        for category in [5usize, 6, 7] {
            assert_eq!(
                TABLE_SIEGE_DEF[category].name, "UnitOrder_None",
                "defender category {category} should be the empty handler"
            );
        }
        // And the mirror: oil is a defender's weapon, so the attacker's slot is
        // empty too.
        assert_eq!(TABLE_SIEGE_ATT[8].name, "UnitOrder_None");

        let all = || {
            TABLE_FIELD
                .iter()
                .chain(TABLE_SIEGE_ATT.iter())
                .chain(TABLE_SIEGE_DEF.iter())
        };
        assert_eq!(all().count(), 25, "twenty-five slots");
        assert_eq!(
            all().filter(|s| s.name == "UnitOrder_None").count(),
            7,
            "UnitOrder_None fills seven of them"
        );
        let mut distinct: Vec<u32> = all().map(|s| s.addr).collect();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 18, "eighteen distinct functions");
        assert_eq!(distinct.len() - 1, 17, "seventeen of which do something");
        // The last real handler is UnitOrder_SiegeDefOil, not the helper at
        // 0x0048ECA9 that `battle.md` §11's range overshoots to.
        assert_eq!(*distinct.last().unwrap(), 0x0048_E8B8);
    }

    /// A garrison sallies out only above 260, which is close to never — and
    /// below it the defenders sit on the castle instead.
    #[test]
    fn a_garrison_only_sorties_when_it_believes_it_is_three_and_a_half_times_stronger() {
        let sortied = |advantage: i32| {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
            fx.ai.is_siege = true;
            fx.ai.multiplayer = true;
            let def = fx.units.create(3, false, SIDE_A, 4).unwrap(); // knight
            let mut f = Figure::new(Troop::Knights, SIDE_A, 4);
            f.unit = def as u16;
            f.owner = 3;
            fx.figures.push(f);
            fx.positions.push((40, 40));
            fx.think_at(advantage);
            (fx.ai.last_action[def], fx.ai.drawbridge_down)
        };
        let (held, bridge) = sortied(SORTIE_THRESHOLD);
        assert_ne!(held, Action::Charge, "260 is not above 260");
        assert!(!bridge, "and the drawbridge stayed up");
        assert_eq!(sortied(SORTIE_THRESHOLD + 1), (Action::Charge, true));
    }

    /// Category 9's handler does nothing at all except count, so that unit
    /// holds the wall slot it deployed on for the whole battle. A test that
    /// only checked "it issued no order" would pass on a broken dispatch too,
    /// so check the counter moved.
    #[test]
    fn the_first_wall_missile_unit_holds_its_slot_for_the_entire_battle() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        let u = fx.units.create(3, false, SIDE_A, 9).unwrap();
        let mut f = Figure::new(Troop::Archers, SIDE_A, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 44));
        fx.frames(2_000);
        assert_eq!(fx.units.get(u).orders, 10, "it thought ten times");
        assert_eq!(fx.ai.last_action[u], Action::DoNothing, "and never ordered anything");
        assert_eq!(
            (fx.units.get(u).target_x, fx.units.get(u).target_y),
            (30, 44),
            "its destination is still where it deployed"
        );
    }

    /// In an open field, categories 5 to 8 have no handler at all: the table is
    /// five entries and the bound is `< 5`. An AI catapult in a field battle is
    /// never repositioned.
    #[test]
    fn a_catapult_in_a_field_battle_is_never_given_an_order() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.multiplayer = true;
        let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
        let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 55));
        fx.frames(2_000);
        assert_eq!(fx.units.get(u).orders, 0, "it never even thinks");
        assert_eq!(fx.ai.last_action[u], Action::NoThink);
        // The siege table, by contrast, does have a handler for it.
        fx.ai.is_siege = true;
        fx.frames(200);
        assert!(fx.units.get(u).orders > 0);
    }

    /// The catapult searches exactly its own firing range: `Siege_FindCellSurface4`
    /// is called with radius 20, and `g_missileStats` gives the catapult 160
    /// eighths of a cell = 20. Two unrelated constants agreeing.
    #[test]
    fn the_catapult_looks_for_a_wall_within_exactly_its_firing_range() {
        let found = |wall_distance: i32| {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
            fx.ai.is_siege = true;
            fx.ai.multiplayer = true;
            fx.field.surface = vec![0u8; CELLS];
            let (cx, cy) = (30usize, 55usize);
            fx.field.surface[(cy as i32 - wall_distance) as usize * DIM + cx] = 4;
            let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
            let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
            f.unit = u as u16;
            f.owner = 3;
            fx.figures.push(f);
            fx.positions.push((cx as u8, cy as u8));
            // Think 31 is the first that searches.
            for _ in 0..32 {
                fx.frames(200);
            }
            fx.ai.last_action[u]
        };
        assert_eq!(found(20), Action::ToCell, "twenty cells is in range");
        assert_eq!(found(21), Action::ToCastleApproach(5), "twenty-one is not");
    }

    /// Two handlers return before the `orders` increment on their wall-found
    /// path, so a unit that is doing its job stops advancing its script.
    #[test]
    fn a_catapult_that_has_found_its_wall_stops_advancing_its_script() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        fx.field.surface = vec![0u8; CELLS];
        fx.field.surface[50 * DIM + 30] = 4;
        let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
        let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 55));
        for _ in 0..32 {
            fx.frames(200);
        }
        let pinned = fx.units.get(u).orders;
        assert_eq!(fx.ai.last_action[u], Action::ToCell);
        for _ in 0..10 {
            fx.frames(200);
        }
        assert_eq!(fx.units.get(u).orders, pinned, "the script counter is stuck");
    }

    /// `SiegeAttFoot` abandons its whole approach script and charges once the
    /// advantage reaches 151.
    #[test]
    fn a_siege_attacker_on_foot_charges_outright_above_a_hundred_and_fifty_one() {
        let outcome = |advantage: i32| {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
            fx.ai.is_siege = true;
            fx.ai.multiplayer = true;
            fx.ai.approach_score = 10;
            fx.ai.breach_score = 5;
            let u = fx.units.create(3, false, SIDE_B, 2).unwrap();
            let mut f = Figure::new(Troop::Pikemen, SIDE_B, 4);
            f.unit = u as u16;
            f.owner = 3;
            fx.figures.push(f);
            fx.positions.push((30, 55));
            for _ in 0..3 {
                fx.think_at(advantage);
            }
            fx.ai.last_action[u]
        };
        assert_ne!(outcome(SIEGE_CHARGE_ADVANTAGE - 1), Action::Charge);
        assert_eq!(outcome(SIEGE_CHARGE_ADVANTAGE), Action::Charge);
    }

    /// The castle layout flag makes two handlers jump `orders` to 100 outright,
    /// skipping the rest of the approach script.
    #[test]
    fn the_castle_layout_flag_jumps_the_approach_script_to_a_hundred() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        fx.field.castle_layout_flag = true;
        let u = fx.units.create(3, false, SIDE_B, 2).unwrap();
        let mut f = Figure::new(Troop::Pikemen, SIDE_B, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 55));
        // Think 18 is the first in the 18..25 band that carries the jump.
        for _ in 0..19 {
            fx.frames(200);
        }
        assert_eq!(fx.units.get(u).orders, 101, "jumped to 100, then incremented");
    }

    // --- reforming ---------------------------------------------------------

    /// `re targ` counts 500 down and then reforms the unit's **own figures**;
    /// it does not look for an enemy. And it runs for human-controlled units
    /// too, which is why the exemption for small human units exists at all.
    #[test]
    fn the_five_hundred_frame_countdown_reforms_and_runs_for_human_units_too() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        let mut who = Vec::new();
        for _ in 0..500 {
            who = fx.frame();
        }
        assert_eq!(who, vec![fx.ai_unit, fx.human_unit], "both sides reform");
        assert_eq!(fx.units.get(fx.human_unit).reform, REFORM_INTERVAL);
    }

    /// Two exemptions, and they are different in kind. A human unit of fewer
    /// than four figures is spared the tidy-up; a unit that has been ordered to
    /// charge is spared it because *it is no longer a formation*.
    #[test]
    fn a_small_human_unit_is_exempt_and_so_is_one_that_has_charged() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 3);
        fx.ai.multiplayer = true;
        let mut who = Vec::new();
        for _ in 0..500 {
            who = fx.frame();
        }
        assert!(
            !who.contains(&fx.human_unit),
            "three figures and human-controlled: exempt"
        );
        assert!(who.contains(&fx.ai_unit), "the AI's unit of three is not");

        // The charge exemption, checked on the flag the charge sets rather than
        // by waiting fourteen thinks for one.
        let mut charged = *fx.units.get(fx.ai_unit);
        assert!(needs_reform(&charged));
        charged.halted = true;
        assert!(!needs_reform(&charged));
        // And an emptied unit is never reformed either.
        charged.halted = false;
        charged.figures = 0;
        assert!(!needs_reform(&charged));
    }

    // --- determinism -------------------------------------------------------

    /// The property lockstep depends on, at the level of the whole AI: two
    /// battles from the same seed must stay bit-identical, jitter and all.
    #[test]
    fn two_ai_battles_from_the_same_seed_stay_identical() {
        let build = || Fixture::new(Troop::Swordsmen, Troop::Archers, 6);
        let (mut a, mut b) = (build(), build());
        for _ in 0..3_000 {
            a.frame();
            b.frame();
            assert_eq!(a.units, b.units, "units diverged");
            assert_eq!(a.ai, b.ai, "ai state diverged");
            assert_eq!(a.figures, b.figures, "figures diverged");
        }
        // And the jitter really was in play, or this would prove nothing.
        assert_ne!(a.ai.rng, Ai::new(0x5EED).rng, "the generator never advanced");
    }
}
