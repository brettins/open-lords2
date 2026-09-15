
use crate::figure::Figure;
use crate::troop::Troop;

/// Sub-steps a crossing takes. `BattleMan_Step` (`0x0048F1DD`) sets
/// `walking = 1` at the commit and adds 2 a sub-step until it reaches 17:
///
/// **Corrected, against the binary.** This was 9 and `docs/battle.md` §7 read
/// `walking += 2; if (walking < 17) return; commit`, counting 2, 4 … 18 from
/// zero. The counter never starts at zero — `if (local_10 == 1) { dirc = dir;
/// walking = 1; FUN_00491b1f(man); }` — so the ninth increment does not exist
/// and a cell costs `8 * (moveDelay + 1)`, not `9 * (…)`. Every relative
/// speed the manual states is a ratio and is unchanged,
/// error survived. `docs/decisions.md` C200.
pub const SUBSTEPS_PER_CELL: u32 = 8;

pub fn move_delay(troop: Troop) -> u32 {
    match troop {
        Troop::Knights => 0,
        Troop::Macemen => 1,
        Troop::Peasants | Troop::Crossbowmen | Troop::Archers => 2,
        Troop::Swordsmen => 3,
        Troop::Pikemen => 4,
        Troop::Catapults | Troop::SiegeTowers | Troop::BatteringRams | Troop::Oil => 5,
    }
}

pub fn ticks_per_cell(troop: Troop) -> u32 {
    SUBSTEPS_PER_CELL * (move_delay(troop) + 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellEntry {
    Free,
    BlockedFriendly,
    Impassable,
    SideGated,
    EnemyPresent,
}

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
    pub substep: u32,
    /// `stepFlags` bit 0, figure `+0x34` — *"ready to leave this cell"*.
    ///
    /// `BattleMan_Create` (`0x0046E4C8`) writes `stepFlags = 1`, so a new
    /// figure decides on its first tick.
    pub free: bool,
}

impl Default for Progress {
    fn default() -> Self {
        Progress { tick_counter: 0, substep: 0, free: true }
    }
}

impl Progress {
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

    #[test]
    fn speeds_match_what_the_manual_says() {
        let mut order: Vec<(Troop, u32)> = ALL_TROOPS.iter().map(|&t| (t, ticks_per_cell(t))).collect();
        order.sort_by_key(|&(t, ticks)| (ticks, t));
        assert_eq!(order[0].0, Troop::Knights, "knights fastest");
        assert_eq!(order[1].0, Troop::Macemen, "macemen second");

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
