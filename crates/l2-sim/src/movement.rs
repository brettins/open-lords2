//! Movement across the battlefield grid.
//!
//! Reimplemented from `docs/battle.md` §7. A figure **enters** the next cell
//! and then counts `walking` 1, 3 … 15 across it, eight sub-steps of
//! `moveDelay + 1` ticks each — 8 ticks for a knight, 40 for a pikeman, 48 for
//! a siege engine.
//!
//! All integer, all in a fixed order. A figure's progress depends only on its
//! own counters, so two machines stepping the same figures reach the same
//! positions.

use crate::figure::Figure;
use crate::troop::Troop;

/// Sub-steps a crossing takes. `BattleMan_Step` (`0x0048F1DD`) sets
/// `walking = 1` at the commit and adds 2 a sub-step until it reaches 17:
/// 1, 3 … 15 is eight, and the eighth lands him.
///
/// **Corrected, against the binary.** This was 9 and `docs/battle.md` §7 read
/// `walking += 2; if (walking < 17) return; commit`, counting 2, 4 … 18 from
/// zero. The counter never starts at zero — `if (local_10 == 1) { dirc = dir;
/// walking = 1; FUN_00491b1f(man); }` — so the ninth increment does not exist
/// and a cell costs `8 * (moveDelay + 1)`, not `9 * (…)`. Every relative
/// speed the manual states is a ratio and is unchanged, which is why the
/// error survived. `docs/decisions.md` CNEW-crossing.
pub const SUBSTEPS_PER_CELL: u32 = 8;

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

/// Per-figure movement progress — figure `+0x32` `walking`, `+0x33` the move
/// tick, and bit 0 of `+0x34` `stepFlags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Ticks accumulated toward the next sub-step — figure `+0x33`.
    pub tick_counter: u32,
    /// Sub-cell progress — figure `+0x32`, the debug panel's **`walking`**.
    /// **0 standing on the cell, else 1, 3 … 15 across the one already
    /// entered.** Never even: the counter starts at 1.
    pub substep: u32,
    /// `stepFlags` bit 0, figure `+0x34` — *"ready to leave this cell"*.
    /// `BattleMan_Create` (`0x0046E4C8`) writes `stepFlags = 1`, so a new
    /// figure decides on its first tick.
    ///
    /// **This is the field that stops a man changing his mind.** While it is
    /// clear `BattleMan_Step` returns before the direction, the melee search
    /// and `Cell_TryEnter` are reached, so a committed crossing finishes in
    /// the direction it began and onto the cell it began on.
    pub free: bool,
}

impl Default for Progress {
    fn default() -> Self {
        Progress { tick_counter: 0, substep: 0, free: true }
    }
}

impl Progress {
    /// One tick of `BattleMan_Step`'s head. **True when the figure may decide
    /// a step this tick** — it is standing, or it has just landed.
    ///
    /// ```text
    /// if ((stepFlags & 1) == 0) {                     /* mid-crossing */
    ///     if (++moveTick <= moveDelay) return 1;
    ///     moveTick = 0;  walking += 2;
    ///     if (walking < 17) return 1;
    ///     stepFlags |= 1;  walking = 0;               /* landed; fall through */
    /// } else { walking = 0; moveTick = 0; }
    /// ```
    pub fn tick(&mut self, troop: Troop) -> bool {
        if self.free {
            self.substep = 0;
            self.tick_counter = 0;
            return true;
        }
        self.tick_counter += 1;
        if self.tick_counter <= move_delay(troop) {
            return false;
        }
        self.tick_counter = 0;
        self.substep += 2;
        if self.substep < 17 {
            return false;
        }
        self.free = true;
        self.substep = 0;
        true
    }

    /// The commit — `stepFlags &= ~1; dirc = dir; walking = 1;
    /// FUN_00491b1f(man)`. The caller does the move; this is the counter's
    /// half, and after it the figure's cell is the one it is crossing to.
    pub fn begin_crossing(&mut self) {
        self.free = false;
        self.substep = 1;
        self.tick_counter = 0;
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
    fn a_cell_costs_eight_substeps_of_delay_plus_one() {
        for t in ALL_TROOPS {
            assert_eq!(ticks_per_cell(t), 8 * (move_delay(t) + 1));
        }
        assert_eq!(ticks_per_cell(Troop::Knights), 8);
        assert_eq!(ticks_per_cell(Troop::Macemen), 16);
        assert_eq!(ticks_per_cell(Troop::Pikemen), 40);
        assert_eq!(ticks_per_cell(Troop::Catapults), 48);
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

    /// A standing figure decides every tick; a crossing one decides on none
    /// of them until it lands. `walking` runs 1, 3 … 15 and never 0 or even.
    #[test]
    fn a_knight_crosses_a_cell_in_exactly_eight_ticks_and_decides_on_none_of_them() {
        let mut p = Progress::default();
        assert!(p.tick(Troop::Knights), "a standing figure is free to decide");
        p.begin_crossing();
        let mut seen = vec![p.substep];
        for t in 1..8 {
            assert!(!p.tick(Troop::Knights), "tick {t} of a crossing must not decide");
            seen.push(p.substep);
        }
        assert_eq!(seen, vec![1, 3, 5, 7, 9, 11, 13, 15]);
        assert!(p.tick(Troop::Knights), "the eighth tick lands him");
        assert_eq!(p, Progress::default(), "landing leaves him free, on the cell");
    }

    #[test]
    fn slower_troops_take_proportionally_longer() {
        for t in ALL_TROOPS {
            let mut p = Progress::default();
            p.begin_crossing();
            let mut ticks = 0;
            while !p.tick(t) {
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
