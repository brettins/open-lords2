
use crate::cue::Cues;
use crate::figure::{Figure, Side, State, SIDE_A, SIDE_B};
use crate::melee;
use crate::troop::{Troop, TroopTable};
use crate::MAX_FIGURES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Battle {
    pub figures: Vec<Figure>,
    pub tick: u32,
    pub troops: TroopTable,
    pub cues: Cues,
}

impl Battle {
    pub fn new() -> Self {
        Battle::with_troops(TroopTable::DEFAULT)
    }

    pub fn with_troops(troops: TroopTable) -> Self {
        Battle { figures: Vec::new(), tick: 0, troops, cues: Cues::default() }
    }

    pub fn add(&mut self, troop: Troop, side: Side, men: u16) -> Option<usize> {
        if self.figures.len() >= MAX_FIGURES {
            return None;
        }
        self.figures.push(Figure::with_table(&self.troops, troop, side, men));
        Some(self.figures.len() - 1)
    }

    pub fn engage(&mut self, a: usize, b: usize) {
        melee::engage(&mut self.figures, a, b);
    }

    pub fn step(&mut self) {
        for i in 0..self.figures.len() {
            if self.figures[i].state == State::Melee {
                let pair = self.figures[i].opponent.filter(|&o| o != i && o < self.figures.len());
                let before = self.duel_snapshot(i, pair);
                melee::tick(&mut self.figures, i);
                self.hear_the_duel(i, pair, before);
            }
        }
        self.tick += 1;
    }

    fn duel_snapshot(&self, i: usize, pair: Option<usize>) -> [(u16, bool); 2] {
        let of = |f: &Figure| (f.men, f.is_alive());
        [of(&self.figures[i]), pair.map_or((0, false), |o| of(&self.figures[o]))]
    }

    /// **`Melee_Tick` (`0x00494908`)'s two sounding arms, as occasions.**
    ///
    /// ```c
    /// if (99 < me.hits) { me.hits -= 100; me.men -= 1;
    ///     FUN_004262cf(other.troopType == 2 ? 4 : other.troopType == 3 ? 5
    ///                : other.troopType == 6 ? 5 : 6); }
    /// if (me.men < 1) { FUN_004262cf(me.side == 0 ? 0xb : 0xc); me.state = 2; }
    /// ```
    ///
    /// `[V]`. The casualty is keyed by the troop that **struck** and the death
    /// by the side that **died**, which is exactly what the two ladders branch
    /// on. Found by comparing the pair before and after [`melee::tick`] rather
    /// than by threading a report through it, so the rules module is untouched
    /// and cannot be told a listener exists.
    ///
    /// `[D]`.
    fn hear_the_duel(&mut self, i: usize, pair: Option<usize>, before: [(u16, bool); 2]) {
        let Some(o) = pair else { return };
        for (me, other, (men, alive)) in [(i, o, before[0]), (o, i, before[1])] {
            let f = &self.figures[me];
            if f.men < men {
                let striker = self.figures[other].troop;
                self.cues.melee_casualty(striker);
            }
            let f = &self.figures[me];
            if alive && !f.is_alive() {
                let side = f.side;
                self.cues.melee_death(side);
            }
        }
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
    fn a_supplied_troop_table_decides_the_battle_instead_of_the_default() {
        let stock = {
            let mut bt = Battle::new();
            bt.add(Troop::Peasants, SIDE_A, 20).unwrap();
            bt.add(Troop::Swordsmen, SIDE_B, 20).unwrap();
            bt.engage(0, 1);
            bt.run(2000);
            bt.men(SIDE_A)
        };

        let mut table = TroopTable::DEFAULT;
        table.stats[Troop::Peasants.index()].melee_attack = [40, 40, 40, 40];
        let modded = {
            let mut bt = Battle::with_troops(table);
            bt.add(Troop::Peasants, SIDE_A, 20).unwrap();
            bt.add(Troop::Swordsmen, SIDE_B, 20).unwrap();
            bt.engage(0, 1);
            bt.run(2000);
            bt.men(SIDE_A)
        };

        assert!(
            modded > stock,
            "peasants swinging for 40 should fare better than the stock 5: {modded} vs {stock}"
        );
        assert_eq!(Battle::new().troops, TroopTable::DEFAULT);
    }

    #[test]
    fn a_figure_keeps_the_row_it_was_built_from() {
        let mut table = TroopTable::DEFAULT;
        table.hits_per_casualty[Troop::Peasants.index()] = 10;
        let mut bt = Battle::with_troops(table);
        bt.add(Troop::Peasants, SIDE_A, 4).unwrap();
        assert_eq!(bt.figures[0].hits_per_casualty, 10);
        assert_eq!(bt.figures[0].take_hits(10), 1, "ten hits is a casualty under this table");

        bt.troops.hits_per_casualty[Troop::Peasants.index()] = 100;
        assert_eq!(bt.figures[0].hits_per_casualty, 10);
    }

    #[test]
    fn damage_accumulates_and_carries_over_between_casualties() {
        let mut f = Figure::new(Troop::Peasants, SIDE_A, 4);
        assert_eq!(f.take_hits(99), 0, "99 hits is not yet a casualty");
        assert_eq!(f.men, 4);
        assert_eq!(f.take_hits(1), 1, "the hundredth hit kills");
        assert_eq!(f.men, 3);
        assert_eq!(f.hits, 0, "no remainder here");
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
    fn a_man_felled_in_melee_is_cued_by_the_troop_that_struck_him() {
        let mut bt = duel(Troop::Macemen, Troop::Knights, 8);
        let mut fell = [0u32; 2];
        for _ in 0..20_000 {
            let was = bt.cues;
            let men = [bt.figures[0].men, bt.figures[1].men];
            let alive = [bt.figures[0].is_alive(), bt.figures[1].is_alive()];
            bt.step();
            let now = bt.cues;
            if bt.figures[0].men < men[0] {
                assert!(
                    now.melee_casualties(Troop::Knights) > was.melee_casualties(Troop::Knights),
                    "a maceman fell at tick {} and no knight was cued",
                    bt.tick
                );
                fell[0] += 1;
            }
            if bt.figures[1].men < men[1] {
                assert!(
                    now.melee_casualties(Troop::Macemen) > was.melee_casualties(Troop::Macemen),
                    "a knight fell at tick {} and no maceman was cued",
                    bt.tick
                );
                fell[1] += 1;
            }
            if alive[0] && !bt.figures[0].is_alive() {
                assert_eq!(now.melee_deaths(SIDE_A), was.melee_deaths(SIDE_A) + 1);
            }
            if alive[1] && !bt.figures[1].is_alive() {
                assert_eq!(now.melee_deaths(SIDE_B), was.melee_deaths(SIDE_B) + 1);
            }
            if bt.is_decided() {
                break;
            }
        }
        assert!(bt.is_decided(), "the duel should end");
        assert!(fell[0] > 0 && fell[1] > 0, "both sides lost men: {fell:?}");
        assert_eq!(bt.cues.melee_deaths(SIDE_A) + bt.cues.melee_deaths(SIDE_B), 1, "one figure died");
        assert_eq!(bt.cues.melee_casualties(Troop::Peasants), 0, "and nobody else struck");
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
