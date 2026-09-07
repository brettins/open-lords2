//! A battle: a fixed set of figures, advanced one tick at a time.

use crate::figure::{Figure, Side, State, SIDE_A, SIDE_B};
use crate::melee;
use crate::troop::Troop;
use crate::MAX_FIGURES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Battle {
    /// Index order is the simulation order and never changes. Figures are never
    /// removed — a dead one stays in place — so indices stay stable and two
    /// machines walk them identically.
    pub figures: Vec<Figure>,
    pub tick: u32,
}

impl Battle {
    pub fn new() -> Self {
        Battle { figures: Vec::new(), tick: 0 }
    }

    /// Add a figure. Returns its index, or `None` once the original's ceiling is
    /// reached — the real engine truncates silently, and reproducing that
    /// matters if we ever diff against it.
    pub fn add(&mut self, troop: Troop, side: Side, men: u16) -> Option<usize> {
        if self.figures.len() >= MAX_FIGURES {
            return None;
        }
        self.figures.push(Figure::new(troop, side, men));
        Some(self.figures.len() - 1)
    }

    pub fn engage(&mut self, a: usize, b: usize) {
        melee::engage(&mut self.figures, a, b);
    }

    /// Advance every figure by one tick, in index order.
    pub fn step(&mut self) {
        for i in 0..self.figures.len() {
            if self.figures[i].state == State::Melee {
                melee::tick(&mut self.figures, i);
            }
        }
        self.tick += 1;
    }

    pub fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.step();
        }
    }

    pub fn living(&self, side: Side) -> usize {
        self.figures.iter().filter(|f| f.side == side && f.is_alive()).count()
    }

    pub fn men(&self, side: Side) -> u32 {
        self.figures
            .iter()
            .filter(|f| f.side == side && f.is_alive())
            .map(|f| f.men as u32)
            .sum()
    }

    /// True once one side has no living figures.
    pub fn is_decided(&self) -> bool {
        self.living(SIDE_A) == 0 || self.living(SIDE_B) == 0
    }
}

impl Default for Battle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::Role;

    fn duel(a: Troop, b: Troop, men: u16) -> Battle {
        let mut bt = Battle::new();
        bt.add(a, SIDE_A, men).unwrap();
        bt.add(b, SIDE_B, men).unwrap();
        bt.engage(0, 1);
        bt
    }

    #[test]
    fn damage_accumulates_and_carries_over_between_casualties() {
        let mut f = Figure::new(Troop::Peasants, SIDE_A, 4);
        assert_eq!(f.take_hits(99), 0, "99 hits is not yet a casualty");
        assert_eq!(f.men, 4);
        assert_eq!(f.take_hits(1), 1, "the hundredth hit kills");
        assert_eq!(f.men, 3);
        assert_eq!(f.hits, 0, "no remainder here");
        // Damage is never wasted: 250 is two more men and 50 carried forward.
        assert_eq!(f.take_hits(250), 2);
        assert_eq!(f.men, 1);
        assert_eq!(f.hits, 50);
    }

    #[test]
    fn a_siege_engine_absorbs_more_before_it_loses_a_crew() {
        let mut f = Figure::new(Troop::Catapults, SIDE_A, 2);
        assert_eq!(f.take_hits(159), 0);
        assert_eq!(f.take_hits(1), 1);
        assert_eq!(f.men, 1);
    }

    #[test]
    fn a_figure_that_loses_its_last_man_dies() {
        let mut f = Figure::new(Troop::Archers, SIDE_A, 1);
        f.take_hits(100);
        assert_eq!(f.men, 0);
        assert_eq!(f.state, State::Dead);
        assert!(!f.is_alive());
    }

    #[test]
    fn siege_engines_are_never_drawn_into_melee() {
        let mut bt = Battle::new();
        bt.add(Troop::Catapults, SIDE_A, 4).unwrap();
        bt.add(Troop::Swordsmen, SIDE_B, 4).unwrap();
        bt.engage(0, 1);
        assert_eq!(bt.figures[0].state, State::Idle);
        assert_eq!(bt.figures[1].state, State::Idle);
        bt.run(500);
        assert_eq!(bt.men(SIDE_A), 4, "no one should have been hurt");
        assert_eq!(bt.men(SIDE_B), 4);
    }

    #[test]
    fn engaging_sets_one_attacker_and_one_defender() {
        let bt = duel(Troop::Swordsmen, Troop::Swordsmen, 4);
        assert_eq!(bt.figures[0].state, State::Melee);
        assert_eq!(bt.figures[1].state, State::Melee);
        assert_eq!(bt.figures[0].role, Role::Attacking);
        assert_eq!(bt.figures[1].role, Role::Defending);
        assert_eq!(bt.figures[0].opponent, Some(1));
        assert_eq!(bt.figures[1].opponent, Some(0));
    }

    #[test]
    fn a_duel_resolves_and_leaves_exactly_one_side_standing() {
        let mut bt = duel(Troop::Swordsmen, Troop::Peasants, 4);
        for _ in 0..20_000 {
            bt.step();
            if bt.is_decided() {
                break;
            }
        }
        assert!(bt.is_decided(), "duel should terminate");
        assert!(
            bt.living(SIDE_A) == 0 || bt.living(SIDE_B) == 0,
            "exactly one side should remain"
        );
    }

    /// Recovery is the only melee defence, so the figure that recovers more
    /// slowly should survive longer against identical opposition. Pikemen have
    /// the longest recovery in the game; peasants among the shortest.
    #[test]
    fn slower_recovery_survives_longer() {
        fn ticks_to_die(defender: Troop) -> u32 {
            let mut bt = Battle::new();
            bt.add(Troop::Knights, SIDE_A, 8).unwrap();
            bt.add(defender, SIDE_B, 8).unwrap();
            bt.engage(0, 1);
            for t in 0..100_000 {
                bt.step();
                if bt.living(SIDE_B) == 0 {
                    return t;
                }
            }
            u32::MAX
        }
        let pikemen = ticks_to_die(Troop::Pikemen);
        let peasants = ticks_to_die(Troop::Peasants);
        assert!(
            pikemen > peasants,
            "pikemen (recovery {}) should outlast peasants (recovery {}): {pikemen} vs {peasants}",
            Troop::Pikemen.stats().recovery,
            Troop::Peasants.stats().recovery
        );
    }

    /// The property lockstep actually depends on: identical inputs must give
    /// bit-identical state, every time, with no dependence on allocation or
    /// iteration order.
    #[test]
    fn two_identical_battles_stay_identical() {
        let build = || {
            let mut bt = Battle::new();
            for (i, t) in [
                Troop::Swordsmen,
                Troop::Pikemen,
                Troop::Knights,
                Troop::Archers,
                Troop::Macemen,
                Troop::Crossbowmen,
            ]
            .into_iter()
            .enumerate()
            {
                bt.add(t, if i % 2 == 0 { SIDE_A } else { SIDE_B }, 6).unwrap();
            }
            bt.engage(0, 1);
            bt.engage(2, 3);
            bt.engage(4, 5);
            bt
        };

        let (mut a, mut b) = (build(), build());
        for _ in 0..5_000 {
            a.step();
            b.step();
            assert_eq!(a, b, "diverged at tick {}", a.tick);
        }
    }

    #[test]
    fn the_figure_ceiling_truncates_rather_than_growing() {
        let mut bt = Battle::new();
        for _ in 0..MAX_FIGURES {
            assert!(bt.add(Troop::Peasants, SIDE_A, 4).is_some());
        }
        assert_eq!(bt.add(Troop::Peasants, SIDE_A, 4), None);
        assert_eq!(bt.figures.len(), MAX_FIGURES);
    }
}
