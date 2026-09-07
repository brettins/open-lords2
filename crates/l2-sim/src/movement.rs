//! Movement across the battlefield grid.
//!
//! Reimplemented from `docs/battle.md` §7. A figure crosses one cell in nine
//! sub-steps, and each sub-step costs `moveDelay + 1` ticks, so a cell costs
//! `9 * (moveDelay + 1)` — from 9 ticks for a knight to 54 for a siege engine.
//!
//! All integer, all in a fixed order. A figure's progress depends only on its
//! own counters, so two machines stepping the same figures reach the same
//! positions.

use crate::figure::Figure;
use crate::troop::Troop;

/// Sub-steps needed to cross one cell. The original counts up by 2 and commits
/// at 17, which is nine increments.
pub const SUBSTEPS_PER_CELL: u32 = 9;

/// Ticks per sub-step is `move_delay + 1`.
pub fn move_delay(troop: Troop) -> u32 {
    match troop {
        Troop::Knights => 0,
        Troop::Macemen => 1,
        Troop::Peasants | Troop::Crossbowmen | Troop::Archers => 2,
        Troop::Swordsmen => 3,
        Troop::Pikemen => 4,
        // Every siege engine moves at the same crawl.
        Troop::Catapults | Troop::SiegeTowers | Troop::BatteringRams | Troop::Oil => 5,
    }
}

pub fn ticks_per_cell(troop: Troop) -> u32 {
    SUBSTEPS_PER_CELL * (move_delay(troop) + 1)
}

/// What `Cell_TryEnter` reports. Kept as distinct cases rather than a boolean
/// because they lead to different behaviour: a friendly blocker of the same
/// troop type causes a swap, an enemy stops movement and starts a melee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellEntry {
    Free,
    /// A friendly figure, or a cell flagged closed.
    BlockedFriendly,
    Impassable,
    /// Passable only by one side — a gate.
    SideGated,
    EnemyPresent,
}

/// The whole terrain-height movement rule: a step is allowed when the two cells
/// differ by at most one level, unless the destination is exactly level 5.
///
/// Skirmish maps carry no elevation at all, so this is inert there — but
/// campaign battles do, and getting it wrong would let figures walk up cliffs.
pub fn can_step_elevation(from: i32, to: i32) -> bool {
    (to - from).abs() <= 1 || to == 5
}

/// Per-figure movement progress. Separate from `Figure` so a figure that is
/// standing still carries no movement state at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    /// Ticks accumulated toward the next sub-step.
    pub tick_counter: u32,
    /// Sub-step progress across the current cell, counted in twos to 17.
    pub substep: u32,
}

impl Progress {
    /// Advance one tick. Returns true when the figure commits to the next cell,
    /// resetting progress.
    pub fn step(&mut self, troop: Troop) -> bool {
        let delay = move_delay(troop);
        self.tick_counter += 1;
        if self.tick_counter <= delay {
            return false;
        }
        self.tick_counter = 0;
        self.substep += 2;
        if self.substep < 17 {
            return false;
        }
        self.substep = 0;
        true
    }
}

/// Exchange two figures' positions and state.
///
/// When a figure is blocked by a friendly of the same troop type heading for
/// the same cell, the original swaps the two records rather than moving either.
/// It is a genuine state exchange — including hits and men — not a move.
pub fn swap_places(figs: &mut [Figure], a: usize, b: usize) {
    if a != b {
        figs.swap(a, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::troop::ALL_TROOPS;

    #[test]
    fn a_cell_costs_nine_substeps_of_delay_plus_one() {
        for t in ALL_TROOPS {
            assert_eq!(ticks_per_cell(t), 9 * (move_delay(t) + 1));
        }
        assert_eq!(ticks_per_cell(Troop::Knights), 9);
        assert_eq!(ticks_per_cell(Troop::Macemen), 18);
        assert_eq!(ticks_per_cell(Troop::Pikemen), 45);
        assert_eq!(ticks_per_cell(Troop::Catapults), 54);
    }

    /// The manual states two speed facts. Both hold exactly, and the knight to
    /// pikeman ratio is precisely 1:5.
    #[test]
    fn speeds_match_what_the_manual_says() {
        // "macemen are second only to knights in speed"
        let mut order: Vec<(Troop, u32)> = ALL_TROOPS.iter().map(|&t| (t, ticks_per_cell(t))).collect();
        order.sort_by_key(|&(t, ticks)| (ticks, t));
        assert_eq!(order[0].0, Troop::Knights, "knights fastest");
        assert_eq!(order[1].0, Troop::Macemen, "macemen second");

        // "pikemen move very slowly" - slowest of anything that fights in melee.
        let slowest_fighter = ALL_TROOPS
            .iter()
            .filter(|t| !t.is_siege())
            .max_by_key(|&&t| ticks_per_cell(t))
            .copied()
            .unwrap();
        assert_eq!(slowest_fighter, Troop::Pikemen);

        assert_eq!(ticks_per_cell(Troop::Pikemen), 5 * ticks_per_cell(Troop::Knights));
    }

    #[test]
    fn a_knight_crosses_a_cell_in_exactly_nine_ticks() {
        let mut p = Progress::default();
        for t in 1..9 {
            assert!(!p.step(Troop::Knights), "should not commit at tick {t}");
        }
        assert!(p.step(Troop::Knights), "commits on the ninth tick");
        assert_eq!(p, Progress::default(), "progress resets on commit");
    }

    #[test]
    fn slower_troops_take_proportionally_longer() {
        for t in ALL_TROOPS {
            let mut p = Progress::default();
            let mut ticks = 0;
            while !p.step(t) {
                ticks += 1;
                assert!(ticks < 1000, "{t:?} never crossed a cell");
            }
            assert_eq!(ticks + 1, ticks_per_cell(t), "{t:?}");
        }
    }

    #[test]
    fn elevation_allows_one_level_of_climb_and_the_level_five_exception() {
        assert!(can_step_elevation(2, 2));
        assert!(can_step_elevation(2, 3));
        assert!(can_step_elevation(3, 2));
        assert!(!can_step_elevation(2, 4), "two levels is a cliff");
        assert!(!can_step_elevation(4, 2));
        // The one documented exception: level 5 is always enterable.
        assert!(can_step_elevation(0, 5));
        assert!(can_step_elevation(2, 5));
    }

    #[test]
    fn swapping_exchanges_whole_figures_not_just_positions() {
        use crate::figure::{SIDE_A, SIDE_B};
        let mut figs = vec![
            Figure::new(Troop::Swordsmen, SIDE_A, 8),
            Figure::new(Troop::Pikemen, SIDE_B, 3),
        ];
        figs[0].hits = 40;
        swap_places(&mut figs, 0, 1);
        assert_eq!(figs[0].troop, Troop::Pikemen);
        assert_eq!(figs[0].men, 3);
        assert_eq!(figs[1].troop, Troop::Swordsmen);
        assert_eq!(figs[1].hits, 40, "hits travel with the figure");
    }
}
