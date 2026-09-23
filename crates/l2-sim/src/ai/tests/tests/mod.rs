
mod thinking;
pub use thinking::*;
mod marching;
pub use marching::*;
mod siege;
pub use siege::*;
mod reform;
pub use reform::*;

use super::*;
use crate::figure::{SIDE_A, SIDE_B};
use crate::unit::CATEGORY_OF_TROOP;

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


