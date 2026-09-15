
mod oil_and_rampart_tests;
pub use oil_and_rampart_tests::*;
mod bridge_and_tower_tests;
pub use bridge_and_tower_tests::*;
mod wood_and_proving_tests;
pub use wood_and_proving_tests::*;

use super::*;
use crate::cue::Cues;
use crate::fire::{SURFACE_BRIDGE, SURFACE_BURNING, SURFACE_WOODLAND, SURFACE_WOOD_BURNING, SURFACE_WOOD_CATCHING};
use crate::proving;
use crate::terrain::flag;

fn cell(x: u8, y: u8) -> usize {
    y as usize * DIM + x as usize
}

fn fire_at(r: &BattleRunner, x: u8, y: u8) -> Option<crate::missile::Missile> {
    r.missiles
        .iter()
        .map(|(_, m)| *m)
        .find(|m| m.class == crate::missile::CLASS_FIRE && (m.cell_x, m.cell_y) == (x as i16, y as i16))
}

fn stand(r: &mut BattleRunner, troop: Troop, side: Side, owner: u8, human: bool, at: (u8, u8)) -> usize {
    let sim = r.sim.add(troop, side, 4).unwrap();
    r.sim.figures[sim].owner = owner;
    r.sim.figures[sim].owner_is_human = human;
    r.fighters.push(Fighter {
        sim,
        troop,
        side,
        x: at.0,
        y: at.1,
        target: at,
        facing: 0,
        progress: Progress::default(),
        anim: Motion::Idle,
        phase: 0,
        facing_drawn: 0,
        fidget: 0,
        fidget_period: 0xB4,
        path: Vec::new(),
        barred: 0,
        hold: 0,
        reroutes: 0,
        moat_cell: None,
        moat_load: 0,
        polar: 0,
        corpse: 0,
    });
    let i = r.fighters.len() - 1;
    r.occupant[cell(at.0, at.1)] = Some(i as u16);
    i
}


