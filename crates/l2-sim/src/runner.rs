//! A battle that runs: units, figures, positions, orders and the tick loop.
//!
//! The rest of this crate is rules. [`melee`](crate::melee),
//! [`missile`](crate::missile), [`movement`](crate::movement),
//! [`pathfind`](crate::pathfind), [`unit`](crate::unit) and [`ai`](crate::ai)
//! each answer one question and none of them owns a coordinate. This module is
//! the composition of all of them into a battle — the thing the original's
//! `Battle_Update` is — and it is where positions live.
//!
//! It used to live in the renderer, which is how it came to be the one part of
//! the simulation sitting behind `winit` and `pixels`. `docs/plan-review.md` §5
//! is the argument for the move; `docs/netcode.md` D-3 is the reason it matters.
//!
//! # One frame, in the original's order
//!
//! ```text
//! BattleUnits_RebuildFromFigures   units mirror their figures; grudges age
//! Battle_UpdateAllUnits            recentre, dispatch one order handler per
//!                                  AI unit, count the reform timer down
//! BattleUnit_Reform                units whose destination moved, and units
//!                                  whose 500-frame timer expired, re-issue
//!                                  every figure a destination
//! Battle_UpdateAllMen              `targeted` decays; each figure moves,
//!                                  swings or stands
//! melee::tick                      damage, casualties, death
//! ```
//!
//! Order matters and is the original's: the strength advantage is recomputed
//! before any unit thinks, a unit is recentred on its figures before its handler
//! runs, and the reform countdown runs **outside** the human-control guard so a
//! player's units tidy their formation too (`docs/battle-ai.md` §5).
//!
//! # Determinism
//!
//! Integer arithmetic only, every sweep by ascending index, no hashing anywhere
//! a decision is made, nothing read from the clock. The one random draw in the
//! whole model is the AI's strength jitter, and it comes from the [`Ai`]'s own
//! `Pcg32` — carried in this struct like any other state.
//! `two_runs_of_the_same_battle_stay_identical` holds the property locally and
//! `tests/lockstep.rs` holds it across a socket.
//!
//! # What is faithful and what is not
//!
//! * Unit sizes, footprints, formation geometry and the deployment slot per
//!   unit: [`crate::formation`]. **[V]** from `g_troopBattleStats`.
//! * The seventeen order handlers, the 200-frame think, the 101-frame strength
//!   advantage: [`crate::ai`]. **[D]**
//! * Movement timing, pathfinding, melee: the modules named above.
//! * **Missile fire is not driven from here.** [`crate::missile`] resolves a
//!   hit and nothing calls it: there is no reload counter, no flight and no
//!   arrow. A unit ordered to shoot enters state 17 and stands still, which is
//!   what the original does — but in the original it is also shooting.
//! * **A figure already locked in a melee is not re-tasked by a reform.** The
//!   original re-tasks it and lets its next tick tear the duel down; we do not
//!   model that teardown, so pulling one side out here would leave a
//!   half-broken duel that is *less* like the original than leaving it alone.
//! * The original's figure states 10 (siege engine moving) and 12 (catapult
//!   aiming) are not modelled; those figures walk.

use crate::ai::{self, Ai, AiField};
use crate::facing::{facing_from_delta, FACING_DELTA};
use crate::formation::{self, FOOTPRINT, MAX_FIGURES_PER_UNIT, ROW_MAX, TYPE_PRIORITY, WEAPON_CLASS};
use crate::movement::Progress;
use crate::pathfind::{self, Grid, Outcome, Pos};
use crate::terrain::{self, Battlefield, DIM};
use crate::unit::{chebyshev, Units, CATEGORY_OF_TROOP, MAX_UNITS};
use crate::{Battle, Motion, Side, State, Troop, SIDE_A, SIDE_B};

/// One drawn man: a [`crate::Figure`] plus everywhere it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fighter {
    /// Index into [`BattleRunner::sim`]'s figures. Stable for the whole battle.
    pub sim: usize,
    pub troop: Troop,
    pub side: Side,
    pub x: u8,
    pub y: u8,
    /// Where this figure is walking to — the original's `tg x` / `tg y`, figure
    /// record `+0x24`/`+0x26`, which is what the developers' own debug panel
    /// calls them.
    ///
    /// **Corrected.** This comment used to say `+0x144`/`+0x146`, "written by
    /// `Formation_SendFigure` and by nothing else". Both halves were wrong and
    /// nothing tested either. No instruction in `Lords2.exe` references
    /// `+0x144` or `+0x146`; both fall inside the 300-byte path array at
    /// `+0x38`, which runs to `+0x164` (`docs/records.json`). And eleven
    /// functions write `tg x` / `tg y`, not one: `BattleMan_Create`,
    /// `BattleMan_Step`, `BattleMan_StateChase`, `BattleMan_StateEngineFire`,
    /// `BattleMan_StateCloseToAttack`, `Formation_SendFigure` and five
    /// unnamed functions in `0x00492000`. `BattleMan_StateChase` in particular
    /// rewrites it *every tick* from the chased figure's position, which is
    /// the behaviour "written by nothing else" would have ruled out.
    pub target: (u8, u8),
    pub facing: u8,
    /// Sub-cell progress, 0 … 16 in twos, from [`crate::movement`].
    pub progress: Progress,
    pub anim: Motion,
    /// The figure's own animation counter. Seeded per figure so that identical
    /// men do not march in lockstep — the original seeds `+0x0E` the same way.
    pub phase: u8,
    /// Waypoints from the pathfinder, consumed from the end.
    pub path: Vec<Pos>,
    /// Consecutive pathfinding failures. At four the figure stops trying —
    /// the original's `barred`, `+0x176`.
    pub barred: u8,
    /// Ticks to wait before asking the pathfinder again — `hold it`, `+0x165`.
    pub hold: u8,
    /// How many times this figure has been re-routed — `routed`, `+0x166`.
    /// Not morale; see `docs/battle.md` §8.3.
    pub reroutes: u16,
}

impl Fighter {
    fn pos(&self) -> Pos {
        Pos::new(self.x, self.y)
    }
    fn at_target(&self) -> bool {
        (self.x, self.y) == self.target
    }
}

/// One side's army as it is raised: troop counts in figures, plus who owns it.
#[derive(Debug, Clone, Copy)]
pub struct Army<'a> {
    /// `(troop, figures)` in raise order — what [`army_from_counts`] produces.
    pub troops: &'a [(Troop, u16)],
    /// The realm this army belongs to. **Zero means a free slot**, so an owner
    /// of 0 would make every unit read as dead; [`BattleRunner::deploy_armies`]
    /// refuses it.
    pub owner: u8,
    /// Controlled by a human. No order handler runs for a human unit
    /// (`Battle_UpdateAllUnits` guards on it), and
    /// `Battle_UpdateStrengthAdvantage` weighs the human side against the AI
    /// side — so **a battle with no human side reads a strength advantage of
    /// −100 and every AI unit takes its cautious branch.** That is the
    /// original's arithmetic, not a limitation here, and it is why
    /// [`BattleRunner::deploy`] makes one side human.
    pub human: bool,
}

/// One side's army as the **campaign** hands it over: real men per troop type,
/// not figures.
///
/// [`Army`] is the skirmish shape — a fixed number of figures of four men each,
/// which is what a `.skr` map and the shell want. A campaign army is a row of
/// eleven counts out of a `g_units` record (`docs/armies.md` §1.4), and how
/// many men one drawn figure stands for is **not** the caller's choice: it is
/// derived from the two armies together by `Battle_InitArmies`
/// (`docs/battle.md` §5.1). So this carries men and
/// [`BattleRunner::deploy_muster`] picks the scale.
#[derive(Debug, Clone, Copy)]
pub struct Muster<'a> {
    /// `(troop, men)` — real men, in any order; [`RAISE_ORDER`] is applied here.
    pub troops: &'a [(Troop, u32)],
    /// The realm this army belongs to. Zero is the free-slot marker and is
    /// refused, exactly as in [`BattleRunner::deploy_armies`].
    pub owner: u8,
    pub human: bool,
}

impl Muster<'_> {
    pub fn men(&self) -> u32 {
        self.troops.iter().map(|(_, n)| *n).sum()
    }
}

/// `g_menPerFigureTable` (`0x004D9658`) — men one drawn figure stands for, by
/// battlefield size class. **[V]** `docs/battle.md` §5.1.
pub const MEN_PER_FIGURE_TABLE: [u32; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

/// `g_sizeClassLadder` (`0x004D95D8`) — the totals at which the size class
/// steps up. **[V]** `docs/battle.md` §5.1.
pub const SIZE_CLASS_LADDER: [u32; 8] = [305, 609, 1217, 2433, 4865, 9729, 19457, 38913];

/// `Table_Lookup(total, g_sizeClassLadder, 8)`: the first threshold the total
/// does not reach, or 8.
///
/// ```
/// # use l2_sim::runner::{size_class, MEN_PER_FIGURE_TABLE};
/// // docs/battle.md §5.4 derives USER.SKR map 0 independently: 600 v 450 men,
/// // size class 2, 16 men a figure.
/// assert_eq!(size_class(600 + 450), 2);
/// assert_eq!(MEN_PER_FIGURE_TABLE[size_class(1050)], 16);
/// // …and the blank template, 150 v 150, is class 0 at four men a figure.
/// assert_eq!(MEN_PER_FIGURE_TABLE[size_class(300)], 4);
/// ```
pub fn size_class(total_men: u32) -> usize {
    SIZE_CLASS_LADDER.iter().position(|&b| total_men < b).unwrap_or(8)
}

/// The **per-side** refinement of §5.1: a side that would draw fewer than nine
/// figures halves the scale, so a small army is not nearly invisible beside a
/// large one. Floor of four men, the smallest figure the ladder produces.
///
/// The original's condition is `sideTotal / menPerFigure < 9 && menPerFigure > 7`.
/// It is applied **once**; `docs/battle.md` lists the reachable results as
/// 4, 8, 16, 32 or 64, which is one halving from 8 … 128 and no more. **[D]**
/// on the once.
pub fn side_scale(side_men: u32, men_per_figure: u32) -> u32 {
    if men_per_figure > 7 && side_men / men_per_figure < 9 {
        (men_per_figure / 2).max(4)
    } else {
        men_per_figure
    }
}

/// Why a battle ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// One side has no men left. The only way a field battle ends by itself.
    Annihilation,
    /// A side left the field — [`BattleRunner::withdraw`].
    Withdrawal,
    /// **Siege only.** A besieging figure reached the castle's `0x08` cell —
    /// `DAT_00553F3C`. The garrison is still standing and the siege is over
    /// anyway.
    BrokeIn,
    /// **Siege only.** No breach and no siege engines left, against a castle
    /// of [`ASSAULT_REPEATS_BELOW_LEVEL`] or above: the besieger has no way in
    /// and loses. Below that level the same position resets the two progress
    /// scores and the battle carries on instead.
    AssaultFailed,
}

/// The castle level at and above which running out of engines with no breach
/// **ends** the battle rather than restarting the assault.
///
/// The same 3 as `crate::siege`'s campaign-side gate, from a different
/// function: `Siege_LaunchAssault` refuses to *start* an engineless assault at
/// level 3, and `Battle_CheckOutcome` refuses to *continue* one. Two
/// independent statements of the same rule, which is what makes it `[V]`.
pub const ASSAULT_REPEATS_BELOW_LEVEL: u8 = 3;

/// What *assault repulsed, repeat* resets both progress scores to.
pub const ASSAULT_REPEAT_SCORE: i32 = 4;

/// **The battle is over.** [`BattleRunner::conclusion`]'s answer.
///
/// A conclusion is not a boolean and never has been: `FUN_00477DFC` reaches
/// this point six ways and `FUN_00478419` turns the result into one of `L2.eng`
/// group 82's seven heading/body pairs. Two of those ways are in scope here;
/// the rest are sieges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conclusion {
    /// The side left holding the field.
    pub winner: Side,
    pub cause: End,
}

/// How long the original leaves the outcome banner up before
/// `Battle_ReturnToCampaign` runs — `DAT_00568470` counting past 5000 in
/// `FUN_00477DFC`.
///
/// **Nothing about the result changes while it counts**, so this is presentation
/// timing rather than a rule; it is here because the write-back is on the far
/// side of it, and a caller reproducing the original's pacing needs the number.
pub const SETTLE_TICKS: u32 = 5000;

/// The other of the two sides.
pub fn other_side(side: Side) -> Side {
    if side == SIDE_A {
        SIDE_B
    } else {
        SIDE_A
    }
}

/// The battle: rules, battlefield, units, figures and the clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleRunner {
    /// The rules. Melee, damage and death happen in here.
    pub sim: Battle,
    pub field: Battlefield,
    pub fighters: Vec<Fighter>,
    /// The unit array the order handlers think with — `g_battleUnits`.
    pub units: Units,
    /// The AI's globals: the strength advantage, the commit counter, the
    /// frozen generator.
    pub ai: Ai,
    /// Everything positional the handlers read. Built from the battlefield.
    pub ai_field: AiField,
    /// Where each figure stands, indexed by figure — the slice
    /// [`crate::unit::Units::recentre`] and [`ai::update_all_units`] take.
    /// Rebuilt at the top of every tick from [`Self::fighters`].
    positions: Vec<(u8, u8)>,
    /// Which fighter stands on each cell, mirroring the original's cell byte
    /// `+5`. `None` is the original's zero.
    occupant: Vec<Option<u16>>,
    /// Impassable terrain, built once from the battlefield flags.
    blocked: Vec<bool>,
    /// The castle, the two damage accumulators and the way in.
    /// [`crate::siege::SiegeState::field`] on a field battle, and every rule it
    /// carries is then inert.
    pub siege: crate::siege::SiegeState,
    /// One-shot latches for the garrison's first two missile units, which take
    /// dispatch categories **9** and **10** rather than 1 — `BattleUnit_Create`,
    /// siege only, side 0 only. `docs/battle-ai.md` §1.2.
    wall_missile_latch: u8,
    /// The side that has left the field, if any — `DAT_0056D5C8` and
    /// `DAT_005656F8` folded into one. Outranks annihilation.
    withdrawn: Option<Side>,
    /// Men one figure of each side stands for, indexed `[side 0, side 4]` —
    /// `docs/battle.md` §5.1. Four for a skirmish; the campaign's ladder picks
    /// it in [`Self::deploy_muster`], and each side may differ.
    men_per_figure: [u16; 2],
    pub tick: u32,
}

/// Men one figure stands for. The size ladder that picks this from the two
/// armies' totals (`docs/battle.md` §5.1) belongs to whoever builds the armies;
/// this is the value the previous driver used and it is kept.
const MEN_PER_FIGURE: u16 = 4;

/// Seed for the AI's generator when the caller does not supply one.
///
/// Fixed, because a battle that seeded itself from the clock would not be
/// reproducible and could not be run in lockstep. `Battle_Start` seeds the
/// original's pair of LFSRs from a global we have not traced.
pub const DEFAULT_SEED: u64 = 0x004D_9870;

impl BattleRunner {
    /// Deploy two armies onto a battlefield: army A is the AI, army B is the
    /// player.
    ///
    /// `army_a` is side 4 and `army_b` is side 0, matching `Battle_InitArmies`:
    /// army A is raised with side 4, army B with side 0, and side 0 is the one
    /// that deploys at the `0x04` marker. **[V]**
    ///
    /// Army B is marked human-controlled, and that is not cosmetic. It decides
    /// two things the whole AI turns on: which units get an order handler at
    /// all, and the denominator of the strength advantage — see [`Army::human`].
    pub fn deploy(field: Battlefield, army_a: &[(Troop, u16)], army_b: &[(Troop, u16)]) -> Self {
        BattleRunner::deploy_armies(
            field,
            DEFAULT_SEED,
            Army { troops: army_a, owner: 1, human: false },
            Army { troops: army_b, owner: 2, human: true },
        )
    }

    /// **The campaign's entry point.** Deploy two armies given as real men,
    /// choosing the men-per-figure scale the way `Battle_InitArmies` does.
    ///
    /// `docs/battle.md` §5.1: the size class comes from the two totals
    /// *together*, then each side may halve it again if it would otherwise draw
    /// fewer than nine figures. Army A takes side 4 and army B side 0, and in a
    /// campaign battle **army B is the defender** (§4.3).
    ///
    /// Unlike [`Self::deploy_armies`], every figure carries its real share of
    /// men and the **last figure of each unit carries the remainder**, which is
    /// `BattleUnit_Create`'s own rounding. That is what makes the survivors
    /// readable back out as campaign troop counts: the totals start exactly at
    /// the counts that went in.
    ///
    /// ```
    /// # use l2_sim::runner::{blank_field, BattleRunner, Muster};
    /// # use l2_sim::{Troop, SIDE_A, SIDE_B};
    /// let a = [(Troop::Peasants, 128u32), (Troop::Swordsmen, 25), (Troop::Archers, 25)];
    /// let b = [(Troop::Peasants, 122u32), (Troop::Archers, 60)];
    /// let r = BattleRunner::deploy_muster(
    ///     blank_field(),
    ///     l2_sim::runner::DEFAULT_SEED,
    ///     Muster { troops: &a, owner: 1, human: true },
    ///     Muster { troops: &b, owner: 6, human: false },
    /// );
    /// // 178 + 182 = 360, which is size class 1: eight men a figure.
    /// assert_eq!(r.men_per_figure(SIDE_B), 8);
    /// // And no man is lost or invented in the raising.
    /// assert_eq!(r.men(SIDE_B), 178);
    /// assert_eq!(r.men(SIDE_A), 182);
    /// ```
    pub fn deploy_muster(field: Battlefield, seed: u64, army_a: Muster, army_b: Muster) -> Self {
        BattleRunner::deploy_muster_on(field, seed, army_a, army_b, None)
    }

    /// **Deploy a siege** — army A besieging a castle of `castle_level` held by
    /// army B.
    ///
    /// Three things change, and they are the three the original changes:
    /// `g_battleIsSiege`, the castle level `Battle_CheckOutcome` reads, and the
    /// two one-shot category latches that give a garrison's first two missile
    /// units categories **9 and 10** instead of 1. Everything else — the size
    /// ladder, the raise order, the deployment slots — is what a field battle
    /// does.
    ///
    /// The battlefield is the caller's. [`crate::siege::our_castle`] builds one
    /// and says in its name that the layout is ours rather than the original's.
    pub fn deploy_siege(
        field: Battlefield,
        seed: u64,
        army_a: Muster,
        army_b: Muster,
        castle_level: u8,
    ) -> Self {
        BattleRunner::deploy_muster_on(field, seed, army_a, army_b, Some(castle_level))
    }

    fn deploy_muster_on(
        field: Battlefield,
        seed: u64,
        army_a: Muster,
        army_b: Muster,
        castle_level: Option<u8>,
    ) -> Self {
        let total = army_a.men() + army_b.men();
        let class = MEN_PER_FIGURE_TABLE[size_class(total)];
        let mpf_a = side_scale(army_a.men(), class);
        let mpf_b = side_scale(army_b.men(), class);
        let mut runner = BattleRunner::empty(field, seed);
        runner.men_per_figure = [mpf_b as u16, mpf_a as u16];
        if let Some(level) = castle_level {
            runner.siege = crate::siege::SiegeState::castle(level);
            runner.ai.is_siege = true;
            runner.ai_field = crate::siege::our_castle_ai_field(&runner.field, level);
        }
        // Side 4 first, then side 0 — the order fixes figure indices, and figure
        // indices are the simulation order.
        runner.raise_men(&army_a, mpf_a, SIDE_B);
        runner.raise_men(&army_b, mpf_b, SIDE_A);
        runner.settle();
        runner
    }

    /// Deploy with full control over owners, control and the AI's seed.
    pub fn deploy_armies(field: Battlefield, seed: u64, army_a: Army, army_b: Army) -> Self {
        assert_ne!(army_a.owner, 0, "owner 0 is the original's free-slot marker");
        assert_ne!(army_b.owner, 0, "owner 0 is the original's free-slot marker");
        let mut runner = BattleRunner::empty(field, seed);
        // Side 4 first, then side 0. The order fixes figure indices, and figure
        // indices are the simulation order.
        runner.raise(army_a, SIDE_B);
        runner.raise(army_b, SIDE_A);
        runner.settle();
        runner
    }

    /// A battlefield with the arrays allocated and nothing on it.
    fn empty(field: Battlefield, seed: u64) -> Self {
        let blocked: Vec<bool> = field.cells.iter().map(|c| c.impassable()).collect();
        let ai_field = ai_field_for(&field);
        BattleRunner {
            sim: Battle::new(),
            field,
            fighters: Vec::new(),
            units: Units::new(),
            ai: Ai::new(seed),
            ai_field,
            positions: Vec::new(),
            occupant: vec![None; DIM * DIM],
            blocked,
            withdrawn: None,
            men_per_figure: [MEN_PER_FIGURE, MEN_PER_FIGURE],
            siege: crate::siege::SiegeState::field(),
            wall_missile_latch: 0,
            tick: 0,
        }
    }

    /// `Battle_InitArmies`'s tail: the census and the rebuild, before the first
    /// frame runs.
    fn settle(&mut self) {
        self.sync_positions();
        self.units.rebuild_from_figures(&mut self.sim.figures);
        self.ai.count_men(&self.sim.figures);
        for u in 1..=MAX_UNITS {
            if self.units.get(u).is_live() {
                let BattleRunner { units, sim, positions, .. } = self;
                units.recentre(u, &sim.figures, positions);
            }
        }
    }

    /// The player's order: send a unit to a cell.
    ///
    /// `BattleUnit_Order` (`0x00479E90`) is the entry point for a click. Only
    /// the part this crate can honour is here — the destination and the
    /// re-issue to the figures. The original also runs `Dest_FindReachableNear`
    /// on the cell and `Order_StopShortOfTarget` for a missile unit, and sets a
    /// 64-frame order lock when the unit is already in melee; the lock is
    /// reproduced because `BattleUnit_JoinMelee` reads it.
    pub fn order_unit(&mut self, unit: usize, x: u8, y: u8) {
        if unit == 0 || unit > MAX_UNITS || !self.units.get(unit).is_live() {
            return;
        }
        {
            let u = self.units.get_mut(unit);
            u.target_x = x as i16;
            u.target_y = y as i16;
            u.withdrawing = false;
            if u.in_melee {
                u.order_lock = crate::unit::ORDER_LOCK;
            }
        }
        self.reform_unit(unit);
    }

    /// Order every unit of a side at a cell. What a player would do with a
    /// select-all and a click, and what the viewer does so a skirmish is
    /// watchable from the first frame.
    pub fn order_side(&mut self, side: Side, x: u8, y: u8) {
        for u in 1..=MAX_UNITS {
            if self.units.get(u).is_live() && self.units.get(u).side == side {
                self.order_unit(u, x, y);
            }
        }
    }

    /// The unit a fighter belongs to, or 0.
    pub fn unit_of(&self, fighter: usize) -> usize {
        self.sim.figures[self.fighters[fighter].sim].unit as usize
    }

    /// A side's own deployment marker: the `0x04` cell for side 0, `0x0F` for
    /// side 4.
    pub fn home(&self, side: Side) -> (u8, u8) {
        if side == SIDE_A {
            self.field.home_side0
        } else {
            self.field.home_side4
        }
    }

    /// `Deploy_SlotForUnit` (`0x0048169E`): the `n`-th unit of a side takes the
    /// `n`-th of that side's twelve marker slots.
    ///
    /// The two tables are adjacent in the original's `.data` — side 0's twelve
    /// at `0x00553150` and side 4's twelve at `0x005531B0` — and the routine
    /// **does not bound-check the ordinal**. So a side-0 army of more than
    /// twelve units deploys its thirteenth unit on the *enemy's* first
    /// deployment slot, which is reachable: oil holds one figure per unit.
    /// Reproduced. Past the twenty-fourth entry the original reads memory we
    /// have not identified, so there we clamp instead of inventing bytes.
    fn deploy_slot(&self, ordinal: usize, side: Side) -> (u8, u8) {
        let base = if side == SIDE_A { 0 } else { 12 };
        let idx = (base + ordinal).min(23);
        if idx < 12 {
            self.field.deploy_side0[idx]
        } else {
            self.field.deploy_side4[idx - 12]
        }
    }

    /// `Battle_RaiseSide` (`0x0047FEA7`) and `BattleUnit_Create` (`0x00480662`).
    ///
    /// Each troop type is split into units of at most
    /// [`MAX_FIGURES_PER_UNIT`] figures, one unit per deployment slot, and each
    /// unit's figures are laid out in a rectangle around that slot by
    /// [`formation::offset_x`] / [`formation::offset_y`].
    fn raise(&mut self, army: Army, side: Side) {
        let men: Vec<(Troop, u32)> = army
            .troops
            .iter()
            .map(|&(t, figures)| (t, figures as u32 * MEN_PER_FIGURE as u32))
            .collect();
        self.raise_men(
            &Muster { troops: &men, owner: army.owner, human: army.human },
            MEN_PER_FIGURE as u32,
            side,
        );
    }

    /// `Battle_RaiseSide` proper: men in, units and figures out.
    ///
    /// `docs/battle.md` §5.2. The eleven troop types are walked in
    /// [`RAISE_ORDER`]; each is cut into units of at most
    /// `MAX_FIGURES_PER_UNIT[t] * men_per_figure` **men**, and each unit into
    /// `ceil(unitMen / men_per_figure)` figures with the last figure taking the
    /// remainder rather than a full complement.
    fn raise_men(&mut self, army: &Muster, men_per_figure: u32, side: Side) {
        assert_ne!(army.owner, 0, "owner 0 is the original's free-slot marker");
        let mpf = men_per_figure.max(1);
        let mut counts = [0u32; 11];
        for &(t, n) in army.troops {
            counts[t.index()] += n;
        }
        let mut ordinal = 0usize;
        for &troop in RAISE_ORDER.iter() {
            let count = counts[troop.index()];
            if count == 0 {
                continue;
            }
            // A siege engine is one figure whatever the scale, so its count is
            // multiplied up before the split and divided back out by it.
            let mut left = if troop.index() > 6 { count * mpf } else { count };
            let per_unit = MAX_FIGURES_PER_UNIT[troop.index()] as u32;
            let footprint = FOOTPRINT[troop.index()];
            while left > 0 {
                let unit_men = left.min(per_unit * mpf);
                let figures = unit_men.div_ceil(mpf) as usize;
                // `BattleUnit_Create`'s eleven-way ladder, plus the two
                // one-shot latches: in a **siege**, on side **0**, the first
                // missile unit raised takes category 9 and the second takes
                // category 10. Those two categories exist nowhere else, and
                // their handlers — one of which does nothing but count — are
                // two of the fourteen this makes reachable.
                let mut category = CATEGORY_OF_TROOP[troop.index()];
                if self.siege.is_siege && side == SIDE_A && category == 1 && self.wall_missile_latch < 2
                {
                    self.wall_missile_latch += 1;
                    category = 8 + self.wall_missile_latch;
                }
                let Some(unit) = self.units.create(army.owner, army.human, side, category) else {
                    return;
                };
                let slot = self.deploy_slot(ordinal, side);
                let rows = formation::rows_for(troop, figures);
                for i in 0..figures {
                    // The last figure takes what is left rather than a full
                    // complement — `BattleUnit_Create`'s own rounding.
                    let men = if i + 1 == figures {
                        (unit_men - (figures as u32 - 1) * mpf) as u16
                    } else {
                        mpf as u16
                    };
                    let Some(sim) = self.sim.add(troop, side, men) else {
                        // The original truncates at 80 figures and keeps
                        // allocating empty units for the rest of the army; we
                        // stop, because an empty unit is a unit the AI would
                        // then dispatch on.
                        return;
                    };
                    let dx = formation::offset_x(footprint, rows, i as i32);
                    let dy = formation::offset_y(footprint, rows, i as i32, side);
                    let Some((x, y)) =
                        self.free_cell_near(slot.0 as i32 + dx, slot.1 as i32 + dy)
                    else {
                        return;
                    };
                    {
                        let f = &mut self.sim.figures[sim];
                        f.unit = unit as u16;
                        f.owner = army.owner;
                        f.owner_is_human = army.human;
                    }
                    // `BattleMan_Create` sets the facing from the *row*, not
                    // from the side: north of the halfway line a figure faces
                    // south and vice versa. **[D]**
                    let facing = if y < 41 { 4 } else { 0 };
                    self.occupant[y as usize * DIM + x as usize] = Some(self.fighters.len() as u16);
                    self.fighters.push(Fighter {
                        sim,
                        troop,
                        side,
                        x,
                        y,
                        target: (x, y),
                        facing,
                        progress: Progress::default(),
                        anim: Motion::Idle,
                        // The original's seed is `(index * 9 + x * 16) & 0x3F`.
                        phase: ((sim as u32 * 9 + x as u32 * 16) & 0x3F) as u8,
                        path: Vec::new(),
                        barred: 0,
                        hold: 0,
                        reroutes: 0,
                    });
                }
                left -= unit_men;
                ordinal += 1;
            }
        }
    }

    /// `FUN_0046E70E` — the placement search `BattleMan_Create` uses.
    ///
    /// Not a spiral: it scans the whole `(2r+1)²` box clipped to the map, rows
    /// top to bottom and columns left to right, and takes the first cell with
    /// no occupant and no `0x90` flag. Since radius `r-1` has already been
    /// scanned and rejected, that is the topmost-then-leftmost free cell in the
    /// box. Radii 0…19; `None` when the whole neighbourhood is full.
    fn free_cell_near(&self, x: i32, y: i32) -> Option<(u8, u8)> {
        for radius in 0..20i32 {
            let x0 = (x - radius).max(0);
            let y0 = (y - radius).max(0);
            let x1 = (x + radius + 1).min(DIM as i32);
            let y1 = (y + radius + 1).min(DIM as i32);
            for cy in y0..y1 {
                for cx in x0..x1 {
                    let i = cy as usize * DIM + cx as usize;
                    if self.occupant[i].is_none() && !self.blocked[i] {
                        return Some((cx as u8, cy as u8));
                    }
                }
            }
        }
        None
    }

    fn sync_positions(&mut self) {
        self.positions.clear();
        self.positions
            .extend(self.fighters.iter().map(|f| (f.x, f.y)));
    }

    /// `DAT_0053F028` / `DAT_00553C58` — **a side's living men, and the number
    /// the player is looking at.**
    ///
    /// `Battle_CountMenByType` recomputes both every frame, and
    /// `00420000.c:1072` draws them side by side on the battle HUD with
    /// `Ui_DrawNumberRight`. So the two counters that decide the battle are the
    /// two numbers on the screen, which is as good a confirmation as this layer
    /// offers. **[V]**
    ///
    /// **Troop types 7 … 10 are excluded** — the original's loop is
    /// `if (troopType < 7)`, so catapults, towers, rams and oil are worth no
    /// men and a side reduced to siege engines has already lost.
    pub fn men_of_side(&self, side: Side) -> u32 {
        self.sim
            .figures
            .iter()
            .filter(|f| f.side == side && f.is_alive() && f.troop.index() < 7)
            .map(|f| f.men as u32)
            .sum()
    }

    /// **Is the battle over, and who holds the field** — `FUN_00477DFC`
    /// (`0x00477DFC`), the per-frame outcome test.
    ///
    /// `None` while it continues. The whole of the non-siege rule is the first
    /// two arms of that function:
    ///
    /// ```c
    /// if (DAT_0056d5c8 == 0) {
    ///     if      (menA < 1) { g_battleLoser = g_battleArmyB; }  /* B holds the field */
    ///     else if (menB < 1) { g_battleLoser = g_battleArmyA; }
    ///     else if (siege)    { ...three more arms... }
    /// } else {                                    /* a side withdrew */
    ///     g_battleLoser = (A.owner == withdrawer) ? B : A;
    /// }
    /// ```
    ///
    /// — remembering that `g_battleLoser` holds the **winner**, which this is a
    /// fourth site to confirm: `FUN_00478419` maps
    /// `g_localPlayer == g_units[g_battleLoser].owner` onto `L2.eng` group 82's
    /// *"won"* pair.
    ///
    /// **A field battle ends only when one side is annihilated or withdraws.**
    /// There is no morale break, no rout threshold and no clock. The three
    /// siege arms — the escape tile, *assault repulsed, repeat*, and *siege
    /// lifted* — are deliberately not here: sieges are out of scope, and
    /// `Battlefield_BuildCastle` has not been implemented, so a battle in this
    /// crate cannot be one.
    pub fn conclusion(&self) -> Option<Conclusion> {
        if let Some(side) = self.withdrawn {
            return Some(Conclusion { winner: other_side(side), cause: End::Withdrawal });
        }
        // The original tests A first, so a battle that wipes both sides out on
        // the same frame is won by B. Reproduced.
        if self.men_of_side(SIDE_B) < 1 {
            return Some(Conclusion { winner: SIDE_A, cause: End::Annihilation });
        }
        if self.men_of_side(SIDE_A) < 1 {
            return Some(Conclusion { winner: SIDE_B, cause: End::Annihilation });
        }
        if self.siege.is_siege {
            // **Arm three: the besieger got in.** A side-4 figure reached a
            // `0x08` cell, and that alone wins the siege — the garrison need
            // not be touched.
            if self.siege.broke_in {
                return Some(Conclusion { winner: SIDE_B, cause: End::BrokeIn });
            }
            // **Arms four and five, which are one test with two answers.** No
            // breach and no engines left: a small castle can still be stormed,
            // so the scores are reset and the battle carries on; a big one
            // cannot, and the besieger has lost.
            if self.ai.breach_score == 0 && self.ai.siege_engine_count == 0 {
                if self.siege.castle_level >= ASSAULT_REPEATS_BELOW_LEVEL {
                    return Some(Conclusion { winner: SIDE_A, cause: End::AssaultFailed });
                }
                // *Assault repulsed, repeat* is handled in [`Self::step`],
                // which is where a state change belongs; this test is `&self`.
            }
        }
        None
    }

    /// **Assault repulsed, repeat.** `Battle_CheckOutcome`'s arm four, which is
    /// the only place in the whole outcome test that *changes* something
    /// instead of ending the battle: with no breach and no engines left, a
    /// castle below level 3 has the breach and approach scores **reset to 4**
    /// and the fighting goes on.
    ///
    /// It reads as the besiegers regrouping for another go, and it is why a
    /// palisade cannot be defended by simply destroying the siege engines.
    fn assault_repulsed(&mut self) {
        if !self.siege.is_siege
            || self.siege.castle_level >= ASSAULT_REPEATS_BELOW_LEVEL
            || self.ai.breach_score != 0
            || self.ai.siege_engine_count != 0
        {
            return;
        }
        self.ai.breach_score = ASSAULT_REPEAT_SCORE;
        self.ai.approach_score = ASSAULT_REPEAT_SCORE;
    }

    /// A side leaves the field — `DAT_0056D5C8` and `DAT_005656F8`.
    ///
    /// The original raises this in exactly one place,
    /// `UnitOrder_SiegeAttKnight`: an all-knight AI besieger facing an
    /// unbreached wall gives up. It is a lever rather than a rule here because
    /// the only *rule* that pulls it is a siege one, and because a player's
    /// withdrawal has to enter the model somewhere.
    ///
    /// It outranks annihilation: the original tests the flag before it looks at
    /// either men counter.
    pub fn withdraw(&mut self, side: Side) {
        self.withdrawn = Some(side);
    }

    /// Men one figure of `side` stands for — [`size_class`]'s answer for this
    /// battle, or four for a skirmish.
    pub fn men_per_figure(&self, side: Side) -> u16 {
        self.men_per_figure[usize::from(side != SIDE_A)]
    }

    /// **The casualty readback.** Living men of `side`, by
    /// [`Troop::index`] — the eleven counts a `g_units` record carries at
    /// `+0x16C`.
    ///
    /// This is the whole of "the battle hands its result back": the campaign
    /// wrote eleven counts in, the battle killed some of the men standing for
    /// them, and this reads what is left in the same eleven slots. It is exact
    /// rather than proportional because [`Self::deploy_muster`] gives every
    /// figure its real share of men.
    pub fn survivors(&self, side: Side) -> [u32; 11] {
        let mut out = [0u32; 11];
        for f in &self.sim.figures {
            if f.side == side && f.is_alive() {
                out[f.troop.index()] += f.men as u32;
            }
        }
        out
    }

    /// Living men of `side` — the sum of [`Self::survivors`].
    pub fn men(&self, side: Side) -> u32 {
        self.sim.men(side)
    }

    pub fn is_alive(&self, i: usize) -> bool {
        self.sim.figures[self.fighters[i].sim].is_alive()
    }

    pub fn living(&self, side: Side) -> usize {
        self.sim.living(side)
    }

    pub fn is_decided(&self) -> bool {
        self.sim.is_decided()
    }

    /// Advance one tick. See the module doc for the order and why it is that
    /// order.
    pub fn step(&mut self) {
        self.sync_positions();
        self.units.rebuild_from_figures(&mut self.sim.figures);
        if self.siege.is_siege {
            self.recount_siege();
            self.assault_repulsed();
        }

        // A destination the handlers are about to overwrite. Comparing before
        // and after is how this driver notices an order: the handlers in
        // `l2-sim::ai` write `target_x/target_y` where the original's action
        // routines write them *and* call `BattleUnit_Order`, which is what
        // re-issues the figures.
        let mut before = [(0i16, 0i16); MAX_UNITS + 1];
        for (u, slot) in before.iter_mut().enumerate().skip(1) {
            let unit = self.units.get(u);
            *slot = (unit.target_x, unit.target_y);
        }

        let reform = {
            let BattleRunner { units, sim, positions, ai_field, ai, .. } = self;
            ai::update_all_units(units, &mut sim.figures, positions, ai_field, ai)
        };

        let ordered: Vec<usize> = before
            .iter()
            .enumerate()
            .skip(1)
            .filter(|&(u, &was)| {
                let unit = self.units.get(u);
                unit.is_live() && (unit.target_x, unit.target_y) != was
            })
            .map(|(u, _)| u)
            .collect();
        for u in ordered {
            self.reform_unit(u);
        }
        for u in reform {
            self.reform_unit(u);
        }

        // `Battle_UpdateAllMen` ages `targeted` by one per frame, which is what
        // makes free pursuit spread out rather than dogpile.
        for f in self.sim.figures.iter_mut() {
            f.targeted = f.targeted.saturating_sub(1);
        }

        for i in 0..self.fighters.len() {
            self.step_one(i);
        }
        // Damage, casualties and death all happen here, in melee.rs.
        self.sim.step();
        // A figure that died this tick lets go of its cell and starts falling.
        for i in 0..self.fighters.len() {
            if !self.is_alive(i) && self.fighters[i].anim != Motion::Dying {
                let f = &self.fighters[i];
                let cell = f.y as usize * DIM + f.x as usize;
                if self.occupant[cell] == Some(i as u16) {
                    self.occupant[cell] = None;
                }
                self.fighters[i].anim = Motion::Dying;
                self.fighters[i].phase = 0;
                self.fighters[i].path.clear();
            }
        }
        self.tick += 1;
    }

    pub fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.step();
        }
    }

    /// The two counters `Battle_UpdateAllMen` recounts **every frame**, and
    /// which every siege handler branches on.
    ///
    /// * `g_attackersOnWall` (`0x00553E64`) — live side-4 figures standing on
    ///   surface 5. Every defender handler tests it, at 1, 2, 3, 4 and 6.
    /// * `g_siegeEngineCount` (`0x00553FF0`) — live figures of troop type 7, 8
    ///   or 9. Three attacker handlers will not move onto the castle objective
    ///   while it is zero, and `Battle_CheckOutcome` ends the battle when it
    ///   and the breach score are both zero.
    ///
    /// The other two — the approach and breach scores — are *accumulators*
    /// rather than counts and are raised where the wall comes down.
    fn recount_siege(&mut self) {
        let mut on_wall = 0;
        let mut engines = 0;
        for f in self.fighters.iter() {
            if !self.sim.figures[f.sim].is_alive() {
                continue;
            }
            if f.troop.index() >= 7 && f.troop.index() <= 9 {
                engines += 1;
            }
            if f.side == SIDE_B
                && self.field.cells[f.y as usize * DIM + f.x as usize].surface
                    == crate::siege::SURFACE_RAMPART
            {
                on_wall += 1;
            }
        }
        self.ai.attackers_on_wall = on_wall;
        self.ai.siege_engine_count = engines;
    }

    // -- reforming ----------------------------------------------------------

    /// Live fighter indices belonging to a unit, ascending.
    fn members(&self, unit: usize) -> Vec<usize> {
        (0..self.fighters.len())
            .filter(|&i| {
                let f = &self.sim.figures[self.fighters[i].sim];
                f.is_alive() && f.unit as usize == unit
            })
            .collect()
    }

    /// `Formation_PickGeometry` (`0x004819CC`): the member with the highest
    /// [`TYPE_PRIORITY`] dictates footprint and figures per row.
    fn pick_geometry(&self, members: &[usize]) -> (i32, i32) {
        let mut best = (0i32, Troop::Peasants);
        for &m in members {
            let t = self.fighters[m].troop;
            if TYPE_PRIORITY[t.index()] > best.0 {
                best = (TYPE_PRIORITY[t.index()], t);
            }
        }
        (FOOTPRINT[best.1.index()], ROW_MAX[best.1.index()])
    }

    /// The unit's destination as a cell.
    ///
    /// `Order_StepAwayFromUnit` can push a destination off the map; in the
    /// original `BattleUnit_Order` runs `Dest_FindReachableNear` on it, which
    /// clamps. We clamp to the playable border and leave the reachability
    /// search to the pathfinder.
    fn unit_dest(&self, unit: usize) -> (i16, i16) {
        let u = self.units.get(unit);
        (u.target_x.clamp(1, 78), u.target_y.clamp(1, 78))
    }

    /// `BattleUnit_Reform` (`0x0048970E`): re-issue every figure a destination,
    /// from the formation rectangle if it is clear and by search if it is not.
    fn reform_unit(&mut self, unit: usize) {
        let members = self.members(unit);
        if members.is_empty() {
            return;
        }
        let (footprint, cols) = self.pick_geometry(&members);
        let target = self.unit_dest(unit);
        if self.rect_is_clear(unit, target, members.len(), footprint, cols) {
            let rect = formation::compute_rect(target, members.len(), footprint, cols);
            let mut assigned = vec![false; members.len()];
            for i in 0..members.len() {
                let (x, y) = rect.slot(i);
                let Some(k) = self.nearest_free_figure(&members, &assigned, x, y) else {
                    continue;
                };
                assigned[k] = true;
                self.send_figure(unit, members[k], x, y);
            }
        } else {
            self.assign_searched_slots(unit, &members, target);
        }
        self.units.get_mut(unit).reform_gate = false;
    }

    /// `Formation_RectIsClear` (`0x00489F9D`): every slot on the map, at the
    /// destination's elevation, holding no other unit's figure, not impassable.
    /// A destination on water fails outright.
    fn rect_is_clear(
        &self,
        unit: usize,
        target: (i16, i16),
        figures: usize,
        footprint: i32,
        cols: i32,
    ) -> bool {
        let dest = self.field.at(target.0 as usize, target.1 as usize);
        if dest.surface == 2 {
            return false;
        }
        let rect = formation::compute_rect(target, figures, footprint, cols);
        for i in 0..figures {
            let (x, y) = rect.slot(i);
            // The original's own bounds, which are the playable border rather
            // than the array: 1 ..= 78.
            if !(1..=0x4E).contains(&x) || !(1..=0x4E).contains(&y) {
                return false;
            }
            let cell = self.field.at(x as usize, y as usize);
            if cell.elevation != dest.elevation {
                return false;
            }
            if let Some(o) = self.occupant[y as usize * DIM + x as usize] {
                if self.unit_of(o as usize) != unit {
                    return false;
                }
            }
            if cell.impassable() {
                return false;
            }
        }
        true
    }

    /// `Formation_NearestFreeFigure` (`0x00489A62`): the member nearest `(x, y)`
    /// by Manhattan distance that has not been given a slot this pass. Greedy
    /// nearest-first, so a unit does not fold through itself.
    fn nearest_free_figure(
        &self,
        members: &[usize],
        assigned: &[bool],
        x: i32,
        y: i32,
    ) -> Option<usize> {
        let mut best: Option<(i32, usize)> = None;
        for (k, &m) in members.iter().enumerate() {
            if assigned[k] {
                continue;
            }
            let f = &self.fighters[m];
            let d = (f.x as i32 - x).abs() + (f.y as i32 - y).abs();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, k));
            }
        }
        best.map(|(_, k)| k)
    }

    /// `Formation_AssignSearchedSlots` (`0x00489798`): the destination's
    /// rectangle is blocked, so search outward from it once per figure and
    /// claim cells as they are taken.
    ///
    /// The claim list is the original's — it exists so two figures never get
    /// the same cell. The acceptance test is `Formation_SlotIsUsable`
    /// (`0x0048A672`), whose field-battle half is reproduced: a cell holding an
    /// enemy figure is usable, a cell holding another friendly unit's figure is
    /// not, and an impassable empty cell is not.
    fn assign_searched_slots(&mut self, unit: usize, members: &[usize], target: (i16, i16)) {
        let mut assigned = vec![false; members.len()];
        let mut claimed: Vec<(i32, i32)> = Vec::with_capacity(members.len());
        for _ in 0..members.len() {
            let Some((x, y)) = self.find_nearby_slot(unit, target, &claimed) else {
                break;
            };
            claimed.push((x, y));
            let Some(k) = self.nearest_free_figure(members, &assigned, x, y) else {
                break;
            };
            assigned[k] = true;
            self.send_figure(unit, members[k], x, y);
        }
    }

    /// `Formation_FindNearbySlot` (`0x0048A38E`), reduced to the field case:
    /// radii 0…19 around the unit's destination for a usable, unclaimed cell.
    fn find_nearby_slot(
        &self,
        unit: usize,
        target: (i16, i16),
        claimed: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        let dest = self.field.at(target.0 as usize, target.1 as usize);
        for radius in 0..20i32 {
            let x0 = (target.0 as i32 - radius).max(0);
            let y0 = (target.1 as i32 - radius).max(0);
            let x1 = (target.0 as i32 + radius + 1).min(DIM as i32);
            let y1 = (target.1 as i32 + radius + 1).min(DIM as i32);
            for y in y0..y1 {
                for x in x0..x1 {
                    if claimed.contains(&(x, y)) {
                        continue;
                    }
                    if self.slot_is_usable(unit, x, y, dest.elevation) {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }

    /// `Formation_SlotIsUsable` (`0x0048A672`), field-battle branches only.
    fn slot_is_usable(&self, unit: usize, x: i32, y: i32, elevation: u8) -> bool {
        let cell = self.field.at(x as usize, y as usize);
        let occupant = self.occupant[y as usize * DIM + x as usize];
        // Impassable *and* occupied is accepted, which reads like a mistake and
        // is what the code does — the two tests are `(flags & 0x90) == 0 ||
        // occupant == 0`, then a fall-through `return 1`.
        if cell.impassable() && occupant.is_some() {
            return true;
        }
        if let Some(o) = occupant {
            let other = self.unit_of(o as usize);
            if self.units.owner_of(other) != self.units.owner_of(unit) {
                return true;
            }
            if other != unit {
                return false;
            }
        }
        if (cell.elevation as i32) < elevation as i32 - 1 {
            return false;
        }
        !cell.impassable()
    }

    /// `Formation_SendFigure` (`0x00489B8D`): one figure's destination and the
    /// state it walks there in.
    fn send_figure(&mut self, unit: usize, fighter: usize, x: i32, y: i32) {
        let sim = self.fighters[fighter].sim;
        let gate = self.units.get(unit).reform_gate;
        let state = self.sim.figures[sim].state;
        // A figure already filling in the moat is left alone unless the unit's
        // `+0x13` gate is set.
        if state == State::FillingMoat && !gate {
            return;
        }
        // Deviation, and the module doc says why: a figure locked in a duel is
        // left in it.
        if state == State::Melee {
            return;
        }
        let x = x.clamp(0, DIM as i32 - 1);
        let y = y.clamp(0, DIM as i32 - 1);
        let cell = self.field.at(x as usize, y as usize);
        let troop = self.fighters[fighter].troop;
        let owner = self.sim.figures[sim].owner;

        // The occupant of the destination, unless it is one of ours —
        // `g_otherBattleMan` is zeroed when the owners match.
        let enemy_there = self.occupant[y as usize * DIM + x as usize].and_then(|o| {
            let o = o as usize;
            (self.sim.figures[self.fighters[o].sim].owner != owner).then_some(o)
        });

        let mut dest = (x as u8, y as u8);
        let mut new_state = State::Idle;
        if !troop.is_siege() {
            if cell.surface == 2 {
                // Water: fill the moat in. Knights alone are returned unchanged
                // — `docs/battle-ai.md` §5, and `L2.eng` group 214 index 3 is
                // the corroborating help text.
                if troop == Troop::Knights {
                    return;
                }
                new_state = State::FillingMoat;
            }
        } else if self.sim.figures[sim].owner_is_human && !gate {
            // A human's siege engine is not repositioned by a reform.
            return;
        }

        let weapon = WEAPON_CLASS[troop.index()];
        if (1..3).contains(&weapon) && enemy_there.is_some() {
            // A bow or a crossbow with somebody on the destination closes to
            // shoot instead of walking on: its destination becomes its own cell.
            self.sim.figures[sim].target = enemy_there.map(|o| self.fighters[o].sim);
            new_state = State::Shooting;
            dest = (self.fighters[fighter].x, self.fighters[fighter].y);
        }
        if weapon > 2 && (1..4).contains(&cell.elevation) {
            // A catapult aiming at low ground: the original's state 12, which
            // is not modelled. It stands where it is, which is the observable
            // half of it.
            dest = (self.fighters[fighter].x, self.fighters[fighter].y);
        }

        self.sim.figures[sim].state = new_state;
        let f = &mut self.fighters[fighter];
        if f.target != dest {
            f.path.clear();
            f.barred = 0;
        }
        f.target = dest;
    }

    // -- the per-figure tick ------------------------------------------------

    fn step_one(&mut self, i: usize) {
        if !self.is_alive(i) {
            // Dying plays once and then holds on its last frame.
            let f = &mut self.fighters[i];
            if f.phase < 95 {
                f.phase += 1;
            }
            return;
        }

        self.fighters[i].phase = self.fighters[i].phase.wrapping_add(1);
        if self.fighters[i].hold > 0 {
            self.fighters[i].hold -= 1;
        }

        // A duel is a *mutual* lock. Break it when the other half is gone —
        // which happens whenever a third figure lands the killing blow, since
        // `melee::tick` only releases the pair it is resolving.
        //
        // This is not bookkeeping. `BattleUnits_RebuildFromFigures` reads state
        // 4 to raise the unit's in-melee flag, and thirteen of the seventeen
        // order handlers refuse to run while it is set — so one figure left
        // locked on a corpse silently stops its **whole unit** thinking for the
        // rest of the battle. The original's state-4 handler drops back to
        // state 0 for the same reason.
        let sim = self.fighters[i].sim;
        if self.sim.figures[sim].state == State::Melee {
            let mutual = self.sim.figures[sim].opponent.is_some_and(|o| {
                self.sim.figures[o].is_alive() && self.sim.figures[o].opponent == Some(sim)
            });
            if !mutual {
                self.sim.figures[sim].state = State::Idle;
                self.sim.figures[sim].opponent = None;
            }
        }

        // Already locked in a duel: face the opponent and swing.
        if self.sim.figures[self.fighters[i].sim].state == State::Melee {
            if let Some(op) = self.opponent_of(i) {
                let (ox, oy) = (self.fighters[op].x as i32, self.fighters[op].y as i32);
                let f = &mut self.fighters[i];
                if let Some(fc) = facing_from_delta(ox - f.x as i32, oy - f.y as i32) {
                    f.facing = fc;
                }
                f.anim = Motion::Attacking;
                f.progress = Progress::default();
            }
            return;
        }

        self.retarget(i);

        // Not fighting: look for somebody adjacent, exactly as the original's
        // melee search does — eight neighbours, first live enemy wins.
        if let Some(enemy) = self.adjacent_enemy(i) {
            let (ex, ey) = (self.fighters[enemy].x as i32, self.fighters[enemy].y as i32);
            {
                let f = &mut self.fighters[i];
                if let Some(fc) = facing_from_delta(ex - f.x as i32, ey - f.y as i32) {
                    f.facing = fc;
                }
                f.anim = Motion::Attacking;
                f.progress = Progress::default();
            }
            // Contact, by standing next to somebody rather than by walking
            // into them. The original raises the join from the *mover*
            // (`Cell_TryEnter` returning 999), and tells both units either way
            // — this adjacency check simply gets there first when the enemy is
            // the one who walked up.
            let (ua, ub) = (self.unit_of(i), self.unit_of(enemy));
            self.join_melee(ua);
            self.join_melee(ub);
            let (a, b) = (self.fighters[i].sim, self.fighters[enemy].sim);
            self.sim.engage(a, b);
            return;
        }

        if self.fighters[i].at_target() {
            self.fighters[i].anim = Motion::Idle;
            self.fighters[i].progress = Progress::default();
            return;
        }

        self.fighters[i].anim = Motion::Walking;
        let Some(next) = self.next_step(i) else {
            self.fighters[i].anim = Motion::Idle;
            return;
        };
        {
            let f = &mut self.fighters[i];
            let d = facing_from_delta(next.x as i32 - f.x as i32, next.y as i32 - f.y as i32);
            if let Some(d) = d {
                f.facing = d;
            }
        }
        // Only a committed sub-step moves the figure. This is where the
        // per-troop speed lives.
        let troop = self.fighters[i].troop;
        if !self.fighters[i].progress.step(troop) {
            return;
        }
        self.enter(i, next);
    }

    /// A figure in free pursuit or closing to shoot follows its own target
    /// rather than its unit's destination — `docs/battle-ai.md` §3.3 and §4.1.
    fn retarget(&mut self, i: usize) {
        let sim = self.fighters[i].sim;
        match self.sim.figures[sim].state {
            State::Chasing => {
                let alive = self.sim.figures[sim]
                    .target
                    .is_some_and(|t| self.sim.figures[t].is_alive());
                if !alive {
                    let chosen = self.chase_target(i);
                    self.sim.figures[sim].target = chosen;
                    if let Some(t) = chosen {
                        self.sim.figures[t].targeted =
                            self.sim.figures[t].targeted.saturating_add(2);
                    }
                }
                if let Some(t) = self.sim.figures[sim].target {
                    let (tx, ty) = (self.fighters[t].x, self.fighters[t].y);
                    self.fighters[i].target = (tx, ty);
                }
            }
            State::Shooting => {
                // Nothing fires yet, so a shooter whose target dies would stand
                // for the rest of the battle. Returning it to idle lets its
                // unit order it again.
                let alive = self.sim.figures[sim]
                    .target
                    .is_some_and(|t| self.sim.figures[t].is_alive());
                if !alive {
                    self.sim.figures[sim].state = State::Idle;
                    self.sim.figures[sim].target = None;
                }
            }
            _ => {}
        }
    }

    /// `Melee_ChooseChaseTarget` (`0x004954DD`): lowest score wins, where the
    /// score is the Chebyshev distance, **halved** if the enemy carries a
    /// missile weapon, plus that enemy's `targeted` count. No range limit at
    /// all, and siege engines are never chosen.
    fn chase_target(&self, i: usize) -> Option<usize> {
        let me = self.fighters[i].sim;
        let mine = self.sim.figures[me].owner;
        let (mx, my) = (self.fighters[i].x as i16, self.fighters[i].y as i16);
        let mut best: Option<(i32, usize)> = None;
        for j in 0..self.fighters.len() {
            let sim = self.fighters[j].sim;
            let f = &self.sim.figures[sim];
            if !f.is_alive() || f.owner == mine || f.owner == 0 || f.troop.is_siege() {
                continue;
            }
            let mut score = chebyshev(mx, my, self.fighters[j].x as i16, self.fighters[j].y as i16);
            if WEAPON_CLASS[f.troop.index()] != 0 {
                score /= 2;
            }
            score += f.targeted as i32;
            if best.is_none_or(|(bs, _)| score < bs) {
                best = Some((score, sim));
            }
        }
        best.map(|(_, j)| j)
    }

    /// `BattleUnit_JoinMelee`: contact drops the rest of an AI unit into free
    /// pursuit. **[D]** `docs/battle-ai.md` §4.2 — skipped for a human unit, a
    /// missile unit, or a unit holding an order lock, which is the single
    /// biggest visible difference between how the two sides fight.
    fn join_melee(&mut self, unit: usize) {
        if unit == 0 || unit > MAX_UNITS {
            return;
        }
        let u = *self.units.get(unit);
        if !u.is_live() || u.human || u.category == 1 || u.order_lock != 0 {
            return;
        }
        for j in 0..self.fighters.len() {
            let sim = self.fighters[j].sim;
            let f = &mut self.sim.figures[sim];
            if f.unit as usize != unit || !f.is_alive() || f.state == State::Melee {
                continue;
            }
            f.state = State::Chasing;
            f.target = None;
            self.fighters[j].path.clear();
            self.fighters[j].barred = 0;
        }
    }

    /// Try to move figure `i` into `next`, reproducing `Cell_TryEnter`'s
    /// outcomes: free, blocked by a friendly, impassable, or an enemy.
    fn enter(&mut self, i: usize, next: Pos) {
        let dst = next.y as usize * DIM + next.x as usize;
        // **The castle, before anything else.** `Cell_TryEnter` tests `0x40`,
        // then `0x20`, then `0x08` before it looks at the occupant, and each of
        // the three answers differently for the two sides.
        if self.siege.is_siege && self.strike_castle(i, dst) {
            return;
        }
        if self.blocked[dst] {
            self.request_path(i);
            return;
        }
        match self.occupant[dst] {
            None => {
                let src = self.fighters[i].y as usize * DIM + self.fighters[i].x as usize;
                if self.occupant[src] == Some(i as u16) {
                    self.occupant[src] = None;
                }
                self.occupant[dst] = Some(i as u16);
                let f = &mut self.fighters[i];
                f.x = next.x;
                f.y = next.y;
                if f.path.last() == Some(&next) {
                    f.path.pop();
                }
                f.barred = 0;
            }
            Some(other) => {
                let other = other as usize;
                if self.fighters[other].side == self.fighters[i].side {
                    // A friendly of the same type heading the same way swaps
                    // places rather than waiting. `BattleMen_SwapPlaces`.
                    if self.fighters[other].troop == self.fighters[i].troop
                        && self.fighters[other].target == self.fighters[i].target
                    {
                        self.swap_places(i, other);
                    } else {
                        self.request_path(i);
                    }
                } else if self.is_alive(other) {
                    // Contact. Both units are told before the two figures are
                    // locked together, which is the original's order.
                    let (ua, ub) = (self.unit_of(i), self.unit_of(other));
                    self.join_melee(ua);
                    self.join_melee(ub);
                    let (a, b) = (self.fighters[i].sim, self.fighters[other].sim);
                    self.sim.engage(a, b);
                    self.fighters[i].anim = Motion::Attacking;
                } else {
                    self.occupant[dst] = None;
                }
            }
        }
    }

    /// **A figure walks into the castle** — `Cell_TryEnter`'s three siege
    /// answers, and the two damage accumulators behind them.
    ///
    /// Returns true when the step was consumed here, whatever the outcome.
    ///
    /// ```c
    /// if (flags & 0x40) { /* drawbridge: a hole in the wall once it is down */ }
    /// if (flags & 0x20) return (side == 0) ? 1 : 5;   /* wall: 5 -> state 6  */
    /// if (flags & 0x08) { if (side == 4) { DAT_00553F3C = 1; return 2; } return 0; }
    /// ```
    ///
    /// and a **siege engine** goes through `Cell_TryEnterEngine` instead, which
    /// returns **6** — the value `BattleMan_Step` turns into state 14 — for a
    /// `0x20` or `0x40` cell, and **only when `troopType == 9`**. So a ram is
    /// the only figure in the game that reaches state 14, and every other
    /// engine is simply stopped by a wall.
    fn strike_castle(&mut self, i: usize, dst: usize) -> bool {
        use crate::siege::{FLAG_DRAWBRIDGE, FLAG_KEEP, FLAG_WALL};
        let flags = self.field.cells[dst].flags;
        let side = self.fighters[i].side;
        let troop = self.fighters[i].troop;
        let engine = troop.index() >= 7;
        let is_ram = troop == Troop::BatteringRams;

        // The drawbridge is a hole in the wall for whoever is standing on it —
        // it is the patch of passable ground the defender's routine lays down —
        // except that a ram treats it as something to break, which is the
        // `0x20 | 0x40` arm of `Cell_TryEnterEngine`.
        if flags & FLAG_DRAWBRIDGE != 0 && !is_ram {
            return false;
        }

        if flags & (FLAG_WALL | FLAG_DRAWBRIDGE) != 0 {
            if side == SIDE_A {
                // The garrison walks its own walls.
                return false;
            }
            // An engine that is not a ram is simply stopped: no state 14, no
            // hits, nothing. `Cell_TryEnterEngine` returns 6 for troop type 9
            // and 2 — blocked — for 7 and 8.
            if engine && !is_ram {
                self.fighters[i].anim = Motion::Idle;
                return true;
            }
            let standing = self.field.cells
                [self.fighters[i].y as usize * DIM + self.fighters[i].x as usize]
                .surface;
            let blow = crate::siege::strike_wall(&mut self.siege, standing, is_ram);
            self.fighters[i].anim = Motion::Attacking;
            match blow {
                crate::siege::WallBlow::RampartBreached => {
                    // The patch the attacker was standing beside comes down.
                    // Surface 4 is what `Siege_FindCellSurface4` hunts for, so
                    // this is how the order layer learns the wall is open.
                    self.field.cells[dst].surface = crate::siege::SURFACE_BREACH;
                    self.field.cells[dst].flags &= !FLAG_WALL;
                    self.field.cells[dst].elevation = 1;
                    self.blocked[dst] = self.field.cells[dst].impassable();
                    self.refresh_ai_surfaces();
                    self.ai.breach_score += 1;
                    self.ai.approach_score += 1;
                }
                crate::siege::WallBlow::GateBreached => {
                    self.field.cells[dst].surface = crate::siege::SURFACE_BREACH;
                    self.field.cells[dst].flags &= !(FLAG_WALL | FLAG_DRAWBRIDGE);
                    self.blocked[dst] = self.field.cells[dst].impassable();
                    self.refresh_ai_surfaces();
                    self.ai.breach_score += crate::siege::GATE_BREACH_SCORE;
                    self.ai.approach_score += crate::siege::GATE_BREACH_SCORE;
                }
                crate::siege::WallBlow::Absorbed => {}
            }
            return true;
        }

        if flags & FLAG_KEEP != 0 {
            // **The way in.** The step is refused for both sides; for the
            // besieger it also ends the battle.
            if side == SIDE_B {
                self.siege.broke_in = true;
            }
            self.fighters[i].anim = Motion::Idle;
            return true;
        }
        false
    }

    /// The AI reads the surfaces out of its own copy, so a breach has to reach
    /// it. Cheap enough at once per breach; there are at most a handful.
    fn refresh_ai_surfaces(&mut self) {
        for (c, cell) in self.field.cells.iter().enumerate() {
            self.ai_field.surface[c] = cell.surface;
            self.ai_field.elevation[c] = cell.elevation;
        }
    }

    /// Exchange two figures' positions.
    ///
    /// The original exchanges the whole records — hits, men and all
    /// (`docs/battle.md` §7). Swapping positions here is the same observable
    /// move with stable indices, which the renderer and the tests both want.
    fn swap_places(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        let (ax, ay) = (self.fighters[a].x, self.fighters[a].y);
        let (bx, by) = (self.fighters[b].x, self.fighters[b].y);
        self.fighters[a].x = bx;
        self.fighters[a].y = by;
        self.fighters[b].x = ax;
        self.fighters[b].y = ay;
        self.fighters[a].progress = Progress::default();
        self.fighters[b].progress = Progress::default();
        self.fighters[a].path.clear();
        self.fighters[b].path.clear();
        self.occupant[by as usize * DIM + bx as usize] = Some(a as u16);
        self.occupant[ay as usize * DIM + ax as usize] = Some(b as u16);
    }

    /// The next cell to try: the stored path if the figure is on one, else
    /// straight at the target. Figures normally walk straight and never search
    /// at all — the pathfinder is what happens when that fails.
    fn next_step(&self, i: usize) -> Option<Pos> {
        let f = &self.fighters[i];
        if let Some(&wp) = f.path.last() {
            return Some(wp);
        }
        let dx = f.target.0 as i32 - f.x as i32;
        let dy = f.target.1 as i32 - f.y as i32;
        let facing = facing_from_delta(dx, dy)?;
        let (sx, sy) = FACING_DELTA[facing as usize];
        let nx = f.x as i32 + sx;
        let ny = f.y as i32 + sy;
        if !(0..DIM as i32).contains(&nx) || !(0..DIM as i32).contains(&ny) {
            return None;
        }
        Some(Pos::new(nx as u8, ny as u8))
    }

    /// Ask [`crate::pathfind`] for a route, subject to the original's two
    /// throttles: a cooldown after each attempt, and a hard stop after four
    /// consecutive failures.
    fn request_path(&mut self, i: usize) {
        {
            let f = &self.fighters[i];
            if f.hold > 0 || f.barred >= 4 {
                return;
            }
        }
        let (start, dest, side) = {
            let f = &self.fighters[i];
            (f.pos(), Pos::new(f.target.0, f.target.1), f.side)
        };

        let mut grid = Grid::open();
        for (c, blocked) in self.blocked.iter().enumerate() {
            if *blocked {
                grid.blocked[c] = true;
            }
        }
        // Only *friendly* figures are marked. Enemies are deliberately left
        // out: the original routes straight through them and leaves contact to
        // the mover.
        for (c, occ) in self.occupant.iter().enumerate() {
            if let Some(o) = occ {
                if self.fighters[*o as usize].side == side && *o as usize != i {
                    grid.occupied[c] = true;
                }
            }
        }

        let search = pathfind::search(&grid, start, dest);
        let f = &mut self.fighters[i];
        f.hold = 64;
        f.reroutes = f.reroutes.saturating_add(1);
        match search.outcome {
            Outcome::Found => {
                let mut path = pathfind::extract(&grid, &search, start, dest);
                path.reverse(); // consumed from the end
                if path.is_empty() {
                    f.barred = f.barred.saturating_add(1);
                } else {
                    f.path = path;
                    f.barred = 0;
                }
            }
            // Adjacent or in clear line of sight: nothing to route around, so
            // the block is transient. Wait rather than counting a failure.
            Outcome::NoSearchNeeded => {}
            Outcome::Unreachable => f.barred = f.barred.saturating_add(1),
        }
    }

    fn opponent_of(&self, i: usize) -> Option<usize> {
        let op = self.sim.figures[self.fighters[i].sim].opponent?;
        self.fighters.iter().position(|f| f.sim == op)
    }

    /// The eight neighbours in `Melee_FindAdjacentEnemy`'s order: N, NW, NE, W,
    /// E, SW, SE, S. The order decides which enemy a figure picks, so it is
    /// part of the behaviour rather than an implementation detail.
    fn adjacent_enemy(&self, i: usize) -> Option<usize> {
        const ORDER: [(i32, i32); 8] = [
            (0, -1),
            (-1, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (1, 1),
            (0, 1),
        ];
        let f = &self.fighters[i];
        if f.troop.is_siege() {
            return None;
        }
        for (dx, dy) in ORDER {
            let nx = f.x as i32 + dx;
            let ny = f.y as i32 + dy;
            if !(0..DIM as i32).contains(&nx) || !(0..DIM as i32).contains(&ny) {
                continue;
            }
            let Some(o) = self.occupant[ny as usize * DIM + nx as usize] else { continue };
            let o = o as usize;
            // Siege engines are skipped entirely as melee targets.
            if self.fighters[o].side != f.side
                && self.is_alive(o)
                && !self.fighters[o].troop.is_siege()
            {
                return Some(o);
            }
        }
        None
    }
}

/// Everything positional the order handlers read, built from a `.skr`
/// battlefield.
///
/// The rally waypoints and the two deployment ends come out of the battlefield;
/// every castle table stays empty, because `Battlefield_BuildCastle` has not
/// been decompiled and inventing a castle would be worse than having none. The
/// siege handlers then find nothing to move to and issue no order, which is
/// what the original does with an unbuilt castle.
pub fn ai_field_for(field: &Battlefield) -> AiField {
    let mut f = AiField::field(
        (field.home_side0.0 as i16, field.home_side0.1 as i16),
        (field.home_side4.0 as i16, field.home_side4.1 as i16),
    );
    f.surface = field.cells.iter().map(|c| c.surface).collect();
    f.elevation = field.cells.iter().map(|c| c.elevation).collect();
    f
}

/// The order `Battle_RaiseSide` walks the eleven troop types in
/// (`g_raiseOrder`, `0x004D9870`): ram, oil, knight, sword, mace, pike,
/// crossbow, archer, peasant, tower, catapult. **[V]** — read out of the binary
/// as `9, 10, 6, 3, 2, 4, 1, 5, 0, 8, 7`. It matters because the tail is what
/// gets truncated when an army would overflow the 80-figure array.
pub const RAISE_ORDER: [Troop; 11] = [
    Troop::BatteringRams,
    Troop::Oil,
    Troop::Knights,
    Troop::Swordsmen,
    Troop::Macemen,
    Troop::Pikemen,
    Troop::Crossbowmen,
    Troop::Archers,
    Troop::Peasants,
    Troop::SiegeTowers,
    Troop::Catapults,
];

/// Turn eleven troop counts — the layout of a `.skr` army record and of a
/// `TROOPS*.ENG` row alike — into figures, in the order the original raises
/// them.
///
/// The size ladder that picks `men_per_figure` from the two armies' totals
/// (`docs/battle.md` §5.1) is not implemented here; the caller supplies it.
pub fn army_from_counts(counts: &[u32; 11], men_per_figure: u32) -> Vec<(Troop, u16)> {
    let per = men_per_figure.max(1);
    RAISE_ORDER
        .iter()
        .filter_map(|t| {
            let men = counts[t.index()];
            if men == 0 {
                return None;
            }
            Some((*t, men.div_ceil(per).min(crate::MAX_FIGURES as u32) as u16))
        })
        .collect()
}

/// A battlefield with nothing on it but the two markers — the editor's blank
/// template, which is what nineteen of `USER.SKR`'s twenty maps are.
pub fn blank_field() -> Battlefield {
    let mut layer = vec![0u8; terrain::CELLS];
    layer[20 * DIM + 40] = 0x04;
    layer[60 * DIM + 40] = 0x0F;
    terrain::build(&layer, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Action;

    fn small_battle() -> BattleRunner {
        BattleRunner::deploy(
            blank_field(),
            &[(Troop::Swordsmen, 6), (Troop::Archers, 4)],
            &[(Troop::Pikemen, 6), (Troop::Peasants, 4)],
        )
    }

    // --- the outcome test, `FUN_00477DFC` --------------------------------

    #[test]
    fn a_battle_with_both_armies_standing_has_not_concluded() {
        let r = small_battle();
        assert_eq!(r.conclusion(), None);
        assert_eq!(r.men_of_side(SIDE_A), 40, "ten figures of four");
        assert_eq!(r.men_of_side(SIDE_B), 40);
    }

    /// The whole of the non-siege rule: a side's men reaching zero.
    #[test]
    fn a_side_with_no_men_left_has_lost_and_the_other_holds_the_field() {
        let mut r = small_battle();
        for f in &mut r.sim.figures {
            if f.side == SIDE_A {
                f.men = 0;
                f.state = State::Dead;
            }
        }
        assert_eq!(r.men_of_side(SIDE_A), 0);
        assert_eq!(
            r.conclusion(),
            Some(Conclusion { winner: SIDE_B, cause: End::Annihilation })
        );
    }

    /// The original tests army A first, so a frame that empties both sides is
    /// won by B — army A is side 4 here, and `menA < 1` is the first arm.
    #[test]
    fn a_battle_that_kills_everyone_at_once_falls_to_the_side_tested_first() {
        let mut r = small_battle();
        for f in &mut r.sim.figures {
            f.men = 0;
            f.state = State::Dead;
        }
        assert_eq!(r.conclusion().unwrap().winner, SIDE_A, "army B holds an empty field");
    }

    /// Withdrawal outranks annihilation: the original tests `DAT_0056D5C8`
    /// before it looks at either men counter, so a side that has already
    /// withdrawn loses even if the enemy is the one that was wiped out.
    #[test]
    fn a_withdrawal_decides_the_battle_before_the_men_are_counted() {
        let mut r = small_battle();
        for f in &mut r.sim.figures {
            if f.side == SIDE_A {
                f.men = 0;
                f.state = State::Dead;
            }
        }
        r.withdraw(SIDE_B);
        let c = r.conclusion().unwrap();
        assert_eq!(c.cause, End::Withdrawal);
        assert_eq!(c.winner, SIDE_A, "the side that left the field loses whatever the count says");
    }

    /// Siege engines are worth no men: the original's counting loop is
    /// `if (troopType < 7)`, so a side reduced to catapults has already lost.
    #[test]
    fn siege_engines_do_not_count_towards_a_sides_men() {
        let mut r = BattleRunner::deploy_muster(
            blank_field(),
            DEFAULT_SEED,
            Muster { troops: &[(Troop::Catapults, 2)], owner: 1, human: false },
            Muster { troops: &[(Troop::Peasants, 40)], owner: 2, human: true },
        );
        assert!(r.sim.figures.iter().any(|f| f.troop == Troop::Catapults), "they were raised");
        assert_eq!(r.men_of_side(SIDE_B), 0, "and they are worth nobody");
        assert_eq!(r.conclusion().unwrap().winner, SIDE_A);
        r.withdraw(SIDE_A);
        assert_eq!(r.conclusion().unwrap().winner, SIDE_B);
    }

    // --- raising from real men, `docs/battle.md` §5.1 and §5.2 ------------

    /// The last figure of a unit takes the remainder rather than a full
    /// complement, so the men that go in are the men that come out.
    #[test]
    fn the_last_figure_of_a_unit_carries_the_remainder() {
        // 306 + 200 = 506, between the ladder's first two breaks at 305 and 609:
        // class 1, eight men a figure.
        let a = [(Troop::Peasants, 306u32)];
        let b = [(Troop::Peasants, 200u32)];
        let r = BattleRunner::deploy_muster(
            blank_field(),
            DEFAULT_SEED,
            Muster { troops: &a, owner: 1, human: false },
            Muster { troops: &b, owner: 2, human: true },
        );
        // Army A deploys as side 4 - `Battle_InitArmies`.
        assert_eq!(r.men_per_figure(SIDE_B), 8);
        assert_eq!(r.men(SIDE_B), 306, "not 312, which 39 full figures of eight would give");
        assert_eq!(r.survivors(SIDE_B)[Troop::Peasants.index()], 306);
        // ceil(306 / 8) = 39 figures, cut into units of at most twelve.
        assert_eq!(r.sim.figures.iter().filter(|f| f.side == SIDE_B).count(), 39);
    }

    /// A side small enough to be nearly invisible halves its scale on its own —
    /// and the *other* side keeps the scale the pair chose.
    #[test]
    fn a_small_side_beside_a_large_one_gets_its_own_finer_scale() {
        // 1000 + 40 = 1040, under the third break at 1217: class 2, sixteen a
        // figure. The small side would draw two figures at that scale.
        let a = [(Troop::Peasants, 1000u32)];
        let b = [(Troop::Peasants, 40u32)];
        let r = BattleRunner::deploy_muster(
            blank_field(),
            DEFAULT_SEED,
            Muster { troops: &a, owner: 1, human: false },
            Muster { troops: &b, owner: 2, human: true },
        );
        assert_eq!(size_class(1040), 2);
        assert_eq!(r.men_per_figure(SIDE_B), 16, "the large side keeps the pair's scale");
        assert_eq!(r.men_per_figure(SIDE_A), 8, "and the small one halves it");
        assert_eq!(r.men(SIDE_A), 40, "nobody lost either way");
        assert_eq!(r.men(SIDE_B), 1000);
    }

    /// `deploy_muster` walks [`RAISE_ORDER`] whatever order the caller wrote
    /// the troops in — the tail of that order is what gets truncated at the
    /// eighty-figure ceiling, so it is not cosmetic.
    #[test]
    fn the_raise_order_is_the_binarys_and_not_the_callers() {
        let a = [(Troop::Peasants, 40u32), (Troop::Knights, 40)];
        let b = [(Troop::Peasants, 40u32)];
        let r = BattleRunner::deploy_muster(
            blank_field(),
            DEFAULT_SEED,
            Muster { troops: &a, owner: 1, human: false },
            Muster { troops: &b, owner: 2, human: true },
        );
        let first = r.sim.figures.iter().find(|f| f.side == SIDE_B).unwrap();
        assert_eq!(first.troop, Troop::Knights, "knights are raised before peasants");
    }

    #[test]
    fn deployment_puts_every_figure_on_its_own_passable_cell() {
        let r = small_battle();
        assert_eq!(r.fighters.len(), 20);
        let mut seen = std::collections::HashSet::new();
        for f in &r.fighters {
            assert!(seen.insert((f.x, f.y)), "two figures on {:?}", (f.x, f.y));
            assert!(!r.field.at(f.x as usize, f.y as usize).impassable());
            assert!(f.x < 80 && f.y < 80);
        }
        assert_eq!(r.occupant.iter().filter(|o| o.is_some()).count(), 20);
    }

    /// The unit layer, which is what the AI dispatches on. Six swordsmen are
    /// one unit (the ceiling is eight); four archers are another; and the
    /// categories are the ones `g_unitOrderTableField` is indexed by.
    #[test]
    fn each_troop_group_becomes_a_unit_of_at_most_its_ceiling() {
        let r = small_battle();
        let live: Vec<usize> = r.units.live().collect();
        assert_eq!(live.len(), 4, "two units a side");
        let sizes: Vec<u8> = live.iter().map(|&u| r.units.get(u).figures).collect();
        assert_eq!(sizes, vec![6, 4, 6, 4]);
        // Categories: swordsmen 3, archers 1, pikemen 2, peasants 2.
        let cats: Vec<u8> = live.iter().map(|&u| r.units.get(u).category).collect();
        assert_eq!(cats, vec![3, 1, 2, 2]);
        // Every figure belongs to exactly one of them, and no figure is loose.
        for f in &r.sim.figures {
            assert!(f.unit >= 1 && f.unit as usize <= MAX_UNITS, "figure with no unit");
        }
        // Side 4 is the AI and side 0 the player.
        assert!(!r.units.get(live[0]).human && r.units.get(live[0]).side == SIDE_B);
        assert!(r.units.get(live[2]).human && r.units.get(live[2]).side == SIDE_A);
    }

    /// A unit bigger than its ceiling splits, and each part takes its own
    /// deployment slot. Twenty peasants is two units of twelve and eight, not
    /// one of twenty — `MAX_FIGURES_PER_UNIT[peasants] == 12`.
    #[test]
    fn an_oversized_troop_group_splits_into_units_at_the_ceiling() {
        let r = BattleRunner::deploy(blank_field(), &[(Troop::Peasants, 20)], &[]);
        let live: Vec<usize> = r.units.live().collect();
        assert_eq!(live.len(), 2);
        assert_eq!(r.units.get(live[0]).figures, 12);
        assert_eq!(r.units.get(live[1]).figures, 8);
        // And they are not on top of one another: different marker slots.
        let a = (r.units.get(live[0]).x, r.units.get(live[0]).y);
        let b = (r.units.get(live[1]).x, r.units.get(live[1]).y);
        assert_ne!(a, b, "both units deployed on the same slot");
    }

    #[test]
    fn the_two_sides_deploy_at_opposite_markers_and_face_each_other() {
        let r = small_battle();
        let side4: Vec<u8> = r.fighters.iter().filter(|f| f.side == SIDE_B).map(|f| f.y).collect();
        let side0: Vec<u8> = r.fighters.iter().filter(|f| f.side == SIDE_A).map(|f| f.y).collect();
        assert!(!side4.is_empty() && !side0.is_empty());
        // Side 0 is the 0x04 marker at y = 20; side 4 is 0x0F at y = 60.
        assert!(side0.iter().all(|&y| y < 40), "side 0 should be at the low end: {side0:?}");
        assert!(side4.iter().all(|&y| y > 40), "side 4 should be at the high end: {side4:?}");
        // `BattleMan_Create` picks the facing from the row: north of 41 a man
        // faces south (4), south of it he faces north (0).
        for f in &r.fighters {
            assert_eq!(f.facing, if f.y < 41 { 4 } else { 0 }, "at {:?}", (f.x, f.y));
        }
        // And nobody has been ordered anywhere yet: `BattleUnit_Recentre` seeds
        // an un-ordered unit's destination from its own position, which is why
        // an army stands still until something tells it not to.
        for u in r.units.live() {
            let unit = r.units.get(u);
            assert_eq!((unit.target_x, unit.target_y), (unit.x, unit.y));
        }
    }

    #[test]
    fn nobody_moves_before_their_troops_move_delay_has_elapsed() {
        let mut r = BattleRunner::deploy(
            blank_field(),
            &[(Troop::Pikemen, 1)],
            &[(Troop::Pikemen, 1)],
        );
        r.order_unit(r.unit_of(0), 40, 70);
        let start = (r.fighters[0].x, r.fighters[0].y);
        // Pikemen take 45 ticks to cross a cell.
        for _ in 0..44 {
            r.step();
        }
        assert_eq!((r.fighters[0].x, r.fighters[0].y), start, "moved early");
        r.step();
        assert_ne!((r.fighters[0].x, r.fighters[0].y), start, "should have crossed by tick 45");
    }

    #[test]
    fn a_knight_crosses_five_cells_while_a_pikeman_crosses_one() {
        fn cells_moved(troop: Troop, ticks: u32) -> i32 {
            let mut r = BattleRunner::deploy(blank_field(), &[(troop, 1)], &[]);
            r.order_unit(r.unit_of(0), 40, 20);
            let start = r.fighters[0].y as i32;
            r.run(ticks);
            (r.fighters[0].y as i32 - start).abs()
        }
        assert_eq!(cells_moved(Troop::Knights, 45), 5);
        assert_eq!(cells_moved(Troop::Pikemen, 45), 1);
    }

    /// The AI advances of its own accord — nothing here orders anybody. The
    /// only thing that moves side 4 is `Battle_UpdateAllUnits` dispatching
    /// `UnitOrder_FieldMelee` on its swordsmen.
    #[test]
    fn the_ai_marches_on_the_enemy_without_being_told_to() {
        let mut r = small_battle();
        let gap = |r: &BattleRunner| -> i32 {
            let a = r.fighters.iter().filter(|f| f.side == SIDE_B).map(|f| f.y as i32).min().unwrap();
            let b = r.fighters.iter().filter(|f| f.side == SIDE_A).map(|f| f.y as i32).max().unwrap();
            a - b
        };
        let before = gap(&r);
        // First order at frame 1000, then forty cells at 36 ticks a cell for a
        // swordsman: contact lands a little after 2400.
        r.run(3_000);
        let after = gap(&r);
        assert!(after < before, "the armies did not close: {before} -> {after}");
        assert!(
            r.fighters.iter().any(|f| f.anim == Motion::Attacking),
            "nobody ever engaged"
        );
    }

    /// The cadence, which is the AI's most distinctive property: a unit decides
    /// once every 200 frames and does nothing at all in between.
    ///
    /// The two silent thinks at the start are `docs/battle-ai.md` §12's first
    /// erratum — `UnitOrder_FieldMelee` calls `Order_DoNothing` while
    /// `orders < 4`, so the first *order* a swordsman unit issues is on its
    /// fifth think, at frame 1000.
    #[test]
    fn a_unit_thinks_once_every_two_hundred_frames_and_not_before() {
        let mut r = small_battle();
        let swords = r.units.live().next().unwrap();
        assert_eq!(r.units.get(swords).category, 3, "the swordsman unit");

        r.run(199);
        assert_eq!(r.units.get(swords).orders, 0, "no think inside the first 200 frames");
        r.step();
        assert_eq!(r.units.get(swords).orders, 1, "the 200th frame is the first think");
        assert_eq!(r.ai.last_action[swords], Action::DoNothing);

        r.run(800);
        assert_eq!(r.units.get(swords).orders, 5, "five thinks by frame 1000");
        assert!(
            matches!(r.ai.last_action[swords], Action::ToEnemyEnd(_)),
            "the fifth think should march: {:?}",
            r.ai.last_action[swords]
        );
        // And that order actually reached the men.
        let marching = r
            .fighters
            .iter()
            .enumerate()
            .filter(|&(i, _)| r.unit_of(i) == swords)
            .any(|(_, f)| f.target != (f.x, f.y));
        assert!(marching, "the unit was ordered but no figure was given a destination");
    }

    /// The one number the whole AI turns on, recomputed on the 101st frame and
    /// not the 100th.
    #[test]
    fn the_strength_advantage_is_recomputed_every_hundred_and_first_frame() {
        let mut r = small_battle();
        assert_eq!(r.ai.strength_advantage, 0, "nothing computed before the first frame");
        r.run(100);
        assert_eq!(r.ai.advantage_timer, 100, "counted up, not yet fired");
        assert_eq!(r.ai.strength_advantage, 0);
        r.step();
        assert_eq!(r.ai.advantage_timer, 0, "fired on frame 101");
        // Six swordsmen (weight 3) and four archers (2) against six pikemen (2)
        // and four peasants (1), four men a figure: 104 against 64, so +62
        // before the -10..+21 jitter.
        assert!(
            (52..=83).contains(&r.ai.strength_advantage),
            "advantage {} outside 62 plus the jitter",
            r.ai.strength_advantage
        );
        assert!(r.ai.strength_advantage > crate::ai::AGGRESSION_THRESHOLD);
    }

    /// Contact drops the rest of an **AI** unit into free pursuit and leaves a
    /// player's unit formed up — `docs/battle-ai.md` §4.2, the single biggest
    /// visible difference between how the two sides fight.
    #[test]
    fn contact_dissolves_the_ai_unit_and_not_the_players() {
        let mut r = small_battle();
        for _ in 0..30_000 {
            r.step();
            if r.fighters.iter().any(|f| f.anim == Motion::Attacking) {
                break;
            }
        }
        assert!(
            r.fighters.iter().any(|f| f.anim == Motion::Attacking),
            "the armies never met"
        );
        let chasing = |side: Side, r: &BattleRunner| {
            r.fighters
                .iter()
                .enumerate()
                .filter(|(i, f)| f.side == side && r.is_alive(*i))
                .filter(|(i, _)| r.sim.figures[r.fighters[*i].sim].state == State::Chasing)
                .count()
        };
        assert!(chasing(SIDE_B, &r) > 0, "the AI unit did not dissolve on contact");
        assert_eq!(chasing(SIDE_A, &r), 0, "a player's unit must not be dissolved");
    }

    #[test]
    fn a_battle_ends_with_one_side_standing() {
        let mut r = BattleRunner::deploy(
            blank_field(),
            &[(Troop::Knights, 8)],
            &[(Troop::Peasants, 8)],
        );
        for _ in 0..60_000 {
            r.step();
            if r.is_decided() {
                break;
            }
        }
        assert!(r.is_decided(), "battle never resolved");
        assert_eq!(r.living(SIDE_A), 0, "knights should have beaten peasants");
        assert!(r.living(SIDE_B) > 0);
        // Everybody who died is drawn falling, and has released their cell.
        for (i, f) in r.fighters.iter().enumerate() {
            if !r.is_alive(i) {
                assert_eq!(f.anim, Motion::Dying);
            }
        }
    }

    /// The property lockstep depends on. Two runs from the same setup must
    /// reach bit-identical state, with nothing leaking in from allocation
    /// order, hashing or the clock.
    #[test]
    fn two_runs_of_the_same_battle_stay_identical() {
        let mut a = small_battle();
        let mut b = small_battle();
        for _ in 0..3_000 {
            a.step();
            b.step();
            assert_eq!(a.fighters, b.fighters, "diverged at tick {}", a.tick);
            assert_eq!(a.sim, b.sim, "simulation diverged at tick {}", a.tick);
            assert_eq!(a.units, b.units, "units diverged at tick {}", a.tick);
            assert_eq!(a.ai, b.ai, "the AI diverged at tick {}", a.tick);
        }
    }

    /// And a different seed is a different battle: the jitter is real state,
    /// not decoration. If it ever stopped reaching a decision this would stop
    /// failing.
    #[test]
    fn a_different_seed_reaches_a_different_strength_advantage() {
        let armies = || {
            (
                vec![(Troop::Swordsmen, 4)],
                vec![(Troop::Swordsmen, 4)],
            )
        };
        let run = |seed: u64| {
            let (a, b) = armies();
            let mut r = BattleRunner::deploy_armies(
                blank_field(),
                seed,
                Army { troops: &a, owner: 1, human: false },
                Army { troops: &b, owner: 2, human: true },
            );
            r.run(101);
            r.ai.strength_advantage
        };
        assert_ne!(run(1), run(2), "the seed does not reach the advantage");
    }

    #[test]
    fn a_wall_of_obstacles_is_routed_around_rather_than_walked_through() {
        let mut layer = vec![0u8; terrain::CELLS];
        layer[20 * DIM + 40] = 0x04;
        layer[60 * DIM + 40] = 0x0F;
        // A wall across the middle with one gap, well clear of both markers.
        for x in 5..70 {
            layer[40 * DIM + x] = 0x02;
        }
        for x in 60..70 {
            layer[40 * DIM + x] = 0x00;
        }
        let field = terrain::build(&layer, 1);
        // Side 0 (the player's side, at the y = 20 marker) ordered across the
        // wall, so this tests the pathfinder and not the AI.
        let mut r = BattleRunner::deploy(field, &[], &[(Troop::Knights, 3)]);
        r.order_side(SIDE_A, 40, 70);
        r.run(3_000);
        for f in &r.fighters {
            assert!(
                !r.field.at(f.x as usize, f.y as usize).impassable(),
                "a figure stands on impassable ground at {:?}",
                (f.x, f.y)
            );
        }
        assert!(
            r.fighters.iter().any(|f| f.y > 40),
            "nobody got past the wall: {:?}",
            r.fighters.iter().map(|f| (f.x, f.y)).collect::<Vec<_>>()
        );
        assert!(r.fighters.iter().any(|f| f.reroutes > 0), "nobody ever asked for a route");
    }

    /// A reform re-issues the whole unit onto a rectangle around its
    /// destination, and the rectangle is the one `Formation_ComputeRect`
    /// builds — not a heap of figures on one cell.
    #[test]
    fn an_order_forms_the_unit_up_on_a_rectangle_around_the_destination() {
        let mut r = BattleRunner::deploy(blank_field(), &[], &[(Troop::Pikemen, 10)]);
        let unit = r.unit_of(0);
        r.order_unit(unit, 40, 40);
        let targets: Vec<(u8, u8)> = (0..r.fighters.len())
            .map(|i| r.fighters[i].target)
            .collect();
        let unique: std::collections::HashSet<_> = targets.iter().collect();
        assert_eq!(unique.len(), 10, "every figure needs its own slot: {targets:?}");
        // Pikemen: footprint 1, five to a row, so a 5 x 2 block centred on the
        // destination.
        let xs: Vec<u8> = targets.iter().map(|t| t.0).collect();
        let ys: Vec<u8> = targets.iter().map(|t| t.1).collect();
        let (x0, x1) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
        let (y0, y1) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        assert_eq!((x1 - x0, y1 - y0), (4, 1), "a 5 x 2 rectangle");
        assert!((x0..=x1).contains(&40) && (y0..=y1).contains(&40), "centred on (40, 40)");
    }
}
