//! A watchable battle: positions, facings and animation over `l2-sim`'s rules.
//!
//! `l2-sim` owns what a battle *is* — figures, melee, missiles, movement
//! timing, pathfinding — and deliberately does not own where anybody stands.
//! Its `Figure` has men, hits and a recovery counter and no coordinates,
//! because none of the rules it implements need them.
//!
//! Something has to, for a battle to be drawn. This is that something: a
//! driver that holds a position, a facing and a walk phase per figure, moves
//! them with `l2_sim::movement` timing and `l2_sim::pathfind` routing, and
//! hands melee back to `l2_sim::Battle`. It is a *composition* of the
//! simulation, not a second one — every rule it applies is called, not
//! reimplemented.
//!
//! # Determinism
//!
//! The same rules as `l2-sim` (`docs/netcode.md`): integer arithmetic only, a
//! fixed iteration order over a `Vec`, no hashing anywhere a decision is made,
//! and nothing read from the clock. `two_runs_of_the_same_battle_stay_identical`
//! is the test that holds it. The renderer never writes here; it only reads.
//!
//! # What is faithful and what is ours
//!
//! * Movement timing, the nine sub-steps per cell, the cost of a cell by troop
//!   type: `l2_sim::movement`. **[V]** in that crate.
//! * Pathfinding, including the 998/999 distinction and the unrelaxed cost
//!   field: `l2_sim::pathfind`. **[V]** in that crate.
//! * Melee: `l2_sim::melee`, through `Battle::engage` and `Battle::step`.
//! * Facings and animation frames: `crate::figures`. **[V]** from the
//!   original's animation handlers.
//! * Deployment slots: the twelve `(dx, dy)` offsets the original expands each
//!   marker into. **[V]**
//! * **Orders are ours, and that is the remaining gap.** Every figure here is
//!   told to advance on the enemy's deployment marker. The original's
//!   seventeen order handlers now exist — `l2_sim::ai`, dispatched through the
//!   three tables `docs/battle-ai.md` §1.1 reads out of the binary — but this
//!   driver has no *units* to run them on: it deploys figures four to a marker
//!   slot and has no `l2_sim::unit::Units` array. Until it does, this is a
//!   melee that happens rather than the battle the original would fight.
//!
//!   What connecting it needs is a unit per raised troop group, `Units`
//!   rebuilt from the figures each tick, and `ai::update_all_units` between
//!   the rebuild and the mover — the shape `crates/l2-sim/tests/lockstep.rs`'s
//!   `AiNetBattle` already runs, on positions rather than a `.skr` map.

use l2_sim::movement::Progress;
use l2_sim::pathfind::{self, Grid, Outcome, Pos};
use l2_sim::{Battle, Side, State, Troop, SIDE_A, SIDE_B};

use crate::figures::{self, Anim};
use crate::terrain::{self, Battlefield, DIM};

/// One drawn man: an `l2-sim` figure plus everywhere it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fighter {
    /// Index into [`BattleRunner::sim`]'s figures. Stable for the whole battle.
    pub sim: usize,
    pub troop: Troop,
    pub side: Side,
    pub x: u8,
    pub y: u8,
    /// Where this figure is walking to.
    pub target: (u8, u8),
    pub facing: u8,
    /// Sub-cell progress, 0 … 16 in twos, from `l2_sim::movement`.
    pub progress: Progress,
    pub anim: Anim,
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

/// The battle as something that can be watched.
pub struct BattleRunner {
    /// The rules. Melee, damage and death happen in here.
    pub sim: Battle,
    pub field: Battlefield,
    pub fighters: Vec<Fighter>,
    /// Which fighter stands on each cell, mirroring the original's cell byte
    /// `+5`. `None` is the original's zero.
    occupant: Vec<Option<u16>>,
    /// Impassable terrain, built once from the battlefield flags.
    blocked: Vec<bool>,
    pub tick: u32,
}

/// How many figures a side deploys per marker slot before spilling into the
/// next one. Ours: the original packs a *unit* into a slot and we have no
/// units yet.
const PER_SLOT: usize = 4;

impl BattleRunner {
    /// Deploy two armies onto a battlefield.
    ///
    /// `army_a` is side 4 and `army_b` is side 0, matching
    /// `Battle_InitArmies`: army A is raised with side 4, army B with side 0,
    /// and side 0 is the one that deploys at the `0x04` marker. **[V]**
    pub fn deploy(field: Battlefield, army_a: &[(Troop, u16)], army_b: &[(Troop, u16)]) -> Self {
        let blocked: Vec<bool> = field.cells.iter().map(|c| c.impassable()).collect();
        let mut runner = BattleRunner {
            sim: Battle::new(),
            field,
            fighters: Vec::new(),
            occupant: vec![None; DIM * DIM],
            blocked,
            tick: 0,
        };
        // Side 4 first, then side 0. The order fixes figure indices, and figure
        // indices are the simulation order.
        runner.raise(army_a, SIDE_B);
        runner.raise(army_b, SIDE_A);
        runner.aim_everyone();
        runner
    }

    fn slots(&self, side: Side) -> [(u8, u8); 12] {
        if side == SIDE_A {
            self.field.deploy_side0
        } else {
            self.field.deploy_side4
        }
    }

    fn home(&self, side: Side) -> (u8, u8) {
        if side == SIDE_A {
            self.field.home_side0
        } else {
            self.field.home_side4
        }
    }

    fn raise(&mut self, army: &[(Troop, u16)], side: Side) {
        let slots = self.slots(side);
        let mut ordinal = 0usize;
        for (troop, count) in army {
            for _ in 0..*count {
                let Some(sim) = self.sim.add(*troop, side, 4) else {
                    // The original truncates silently at 80 figures and so do
                    // we — reproducing the ceiling matters if we ever diff.
                    return;
                };
                let slot = slots[(ordinal / PER_SLOT).min(slots.len() - 1)];
                let within = ordinal % PER_SLOT;
                let Some((x, y)) = self.free_cell_near(slot, within) else { return };
                let facing = if side == SIDE_A { 4 } else { 0 };
                self.occupant[y as usize * DIM + x as usize] = Some(self.fighters.len() as u16);
                self.fighters.push(Fighter {
                    sim,
                    troop: *troop,
                    side,
                    x,
                    y,
                    target: (x, y),
                    facing,
                    progress: Progress::default(),
                    anim: Anim::Idle,
                    // The original's seed is `(index * 9 + x * 16) & 0x3F`.
                    phase: ((sim as u32 * 9 + x as u32 * 16) & 0x3F) as u8,
                    path: Vec::new(),
                    barred: 0,
                    hold: 0,
                    reroutes: 0,
                });
                ordinal += 1;
            }
        }
    }

    /// A free, enterable cell for a figure being deployed. Spirals outward from
    /// the slot in a fixed order, so deployment is reproducible.
    fn free_cell_near(&self, slot: (u8, u8), nth: usize) -> Option<(u8, u8)> {
        let mut skipped = 0usize;
        for radius in 0..20i32 {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let x = slot.0 as i32 + dx;
                    let y = slot.1 as i32 + dy;
                    if !(1..=78).contains(&x) || !(1..=78).contains(&y) {
                        continue;
                    }
                    let i = y as usize * DIM + x as usize;
                    if self.blocked[i] || self.occupant[i].is_some() {
                        continue;
                    }
                    if skipped < nth {
                        skipped += 1;
                        continue;
                    }
                    return Some((x as u8, y as u8));
                }
            }
        }
        None
    }

    /// Order everyone at the enemy's deployment marker, keeping the formation
    /// offset each figure was deployed with so a body of men arrives as a body
    /// rather than a queue.
    fn aim_everyone(&mut self) {
        for i in 0..self.fighters.len() {
            let f = &self.fighters[i];
            let home = self.home(f.side);
            let enemy = self.home(other_side(f.side));
            let dx = f.x as i32 - home.0 as i32;
            let dy = f.y as i32 - home.1 as i32;
            let tx = (enemy.0 as i32 + dx).clamp(1, 78) as u8;
            let ty = (enemy.1 as i32 + dy).clamp(1, 78) as u8;
            self.fighters[i].target = (tx, ty);
        }
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

    /// Advance one tick: move, engage, then let `l2-sim` resolve the melee.
    pub fn step(&mut self) {
        for i in 0..self.fighters.len() {
            self.step_one(i);
        }
        // Damage, casualties and death all happen here, in l2-sim.
        self.sim.step();
        // A figure that died this tick lets go of its cell and starts falling.
        for i in 0..self.fighters.len() {
            if !self.is_alive(i) && self.fighters[i].anim != Anim::Dying {
                let f = &self.fighters[i];
                let cell = f.y as usize * DIM + f.x as usize;
                if self.occupant[cell] == Some(i as u16) {
                    self.occupant[cell] = None;
                }
                self.fighters[i].anim = Anim::Dying;
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

        // Already locked in a duel: face the opponent and swing.
        if self.sim.figures[self.fighters[i].sim].state == State::Melee {
            if let Some(op) = self.opponent_of(i) {
                let (ox, oy) = (self.fighters[op].x as i32, self.fighters[op].y as i32);
                let f = &mut self.fighters[i];
                if let Some(fc) = figures::facing_from_delta(ox - f.x as i32, oy - f.y as i32) {
                    f.facing = fc;
                }
                f.anim = Anim::Attacking;
                f.progress = Progress::default();
            }
            return;
        }

        // Not fighting: look for somebody adjacent, exactly as the original's
        // melee search does — eight neighbours, first live enemy wins.
        if let Some(enemy) = self.adjacent_enemy(i) {
            let (ex, ey) = (self.fighters[enemy].x as i32, self.fighters[enemy].y as i32);
            {
                let f = &mut self.fighters[i];
                if let Some(fc) = figures::facing_from_delta(ex - f.x as i32, ey - f.y as i32) {
                    f.facing = fc;
                }
                f.anim = Anim::Attacking;
                f.progress = Progress::default();
            }
            let (a, b) = (self.fighters[i].sim, self.fighters[enemy].sim);
            self.sim.engage(a, b);
            return;
        }

        if self.fighters[i].at_target() {
            self.fighters[i].anim = Anim::Idle;
            self.fighters[i].progress = Progress::default();
            return;
        }

        self.fighters[i].anim = Anim::Walking;
        let Some(next) = self.next_step(i) else {
            self.fighters[i].anim = Anim::Idle;
            return;
        };
        {
            let f = &mut self.fighters[i];
            let d = figures::facing_from_delta(
                next.x as i32 - f.x as i32,
                next.y as i32 - f.y as i32,
            );
            if let Some(d) = d {
                f.facing = d;
            }
        }
        // Only a committed sub-step moves the figure. This is where the
        // per-troop speed lives, and it is l2-sim's.
        let troop = self.fighters[i].troop;
        if !self.fighters[i].progress.step(troop) {
            return;
        }
        self.enter(i, next);
    }

    /// Try to move figure `i` into `next`, reproducing `Cell_TryEnter`'s
    /// outcomes: free, blocked by a friendly, impassable, or an enemy.
    fn enter(&mut self, i: usize, next: Pos) {
        let dst = next.y as usize * DIM + next.x as usize;
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
                    let (a, b) = (self.fighters[i].sim, self.fighters[other].sim);
                    self.sim.engage(a, b);
                    self.fighters[i].anim = Anim::Attacking;
                } else {
                    self.occupant[dst] = None;
                }
            }
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
        let facing = figures::facing_from_delta(dx, dy)?;
        let (sx, sy) = figures::FACING_DELTA[facing as usize];
        let nx = f.x as i32 + sx;
        let ny = f.y as i32 + sy;
        if !(0..DIM as i32).contains(&nx) || !(0..DIM as i32).contains(&ny) {
            return None;
        }
        Some(Pos::new(nx as u8, ny as u8))
    }

    /// Ask `l2_sim::pathfind` for a route, subject to the original's two
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

fn other_side(side: Side) -> Side {
    if side == SIDE_A {
        SIDE_B
    } else {
        SIDE_A
    }
}

/// The order `Battle_RaiseSide` walks the eleven troop types in
/// (`g_raiseOrder`, `0x004D9870`): ram, oil, knight, sword, mace, pike,
/// crossbow, archer, peasant, tower, catapult. **[V]** It matters because the
/// tail is what gets truncated when an army would overflow the 80-figure array.
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
            Some((*t, men.div_ceil(per).min(l2_sim::MAX_FIGURES as u32) as u16))
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

    fn small_battle() -> BattleRunner {
        BattleRunner::deploy(
            blank_field(),
            &[(Troop::Swordsmen, 6), (Troop::Archers, 4)],
            &[(Troop::Pikemen, 6), (Troop::Peasants, 4)],
        )
    }

    #[test]
    fn deployment_puts_every_figure_on_its_own_passable_cell() {
        let r = small_battle();
        assert_eq!(r.fighters.len(), 20);
        let mut seen = std::collections::HashSet::new();
        for f in &r.fighters {
            assert!(seen.insert((f.x, f.y)), "two figures on {:?}", (f.x, f.y));
            assert!(!r.field.at(f.x as usize, f.y as usize).impassable());
            assert!((1..=78).contains(&f.x) && (1..=78).contains(&f.y));
        }
        assert_eq!(r.occupant.iter().filter(|o| o.is_some()).count(), 20);
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
        for f in &r.fighters {
            // Everyone is ordered at the far marker.
            let want = if f.side == SIDE_A { 60 } else { 20 };
            assert!((f.target.1 as i32 - want).abs() < 20, "target {:?}", f.target);
        }
    }

    #[test]
    fn nobody_moves_before_their_troops_move_delay_has_elapsed() {
        let mut r = BattleRunner::deploy(
            blank_field(),
            &[(Troop::Pikemen, 1)],
            &[(Troop::Pikemen, 1)],
        );
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
            let start = r.fighters[0].y as i32;
            r.run(ticks);
            (r.fighters[0].y as i32 - start).abs()
        }
        assert_eq!(cells_moved(Troop::Knights, 45), 5);
        assert_eq!(cells_moved(Troop::Pikemen, 45), 1);
    }

    #[test]
    fn figures_walk_toward_the_enemy_and_eventually_meet() {
        let mut r = small_battle();
        let gap = |r: &BattleRunner| -> i32 {
            let a = r.fighters.iter().filter(|f| f.side == SIDE_B).map(|f| f.y as i32).min().unwrap();
            let b = r.fighters.iter().filter(|f| f.side == SIDE_A).map(|f| f.y as i32).max().unwrap();
            a - b
        };
        let before = gap(&r);
        r.run(1_200);
        let after = gap(&r);
        assert!(after < before, "the armies did not close: {before} -> {after}");
        assert!(
            r.fighters.iter().any(|f| f.anim == Anim::Attacking),
            "nobody ever engaged"
        );
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
                assert_eq!(f.anim, Anim::Dying);
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
        for _ in 0..1_500 {
            a.step();
            b.step();
            assert_eq!(a.fighters, b.fighters, "diverged at tick {}", a.tick);
            assert_eq!(a.sim, b.sim, "simulation diverged at tick {}", a.tick);
        }
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
        let mut r = BattleRunner::deploy(field, &[], &[(Troop::Knights, 3)]);
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
}
