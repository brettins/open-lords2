//! **Fire, oil and the siege tower, each against a battle built so it must
//! happen** — and the determinism proof extended to a siege that uses all of
//! them.
//!
//! A child of [`crate::runner`], so a test can stand a man on a chosen cell
//! the way `firing_line` does. The siege is [`crate::proving`]'s, and every
//! literal asserted below is the original's: a frame count, a fire's life, a
//! cell's height, read out of the routine the test names — never computed from
//! the constant under test.

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

/// The fire record burning over `(x, y)`, if there is one.
fn fire_at(r: &BattleRunner, x: u8, y: u8) -> Option<crate::missile::Missile> {
    r.missiles
        .iter()
        .map(|(_, m)| *m)
        .find(|m| m.class == crate::missile::CLASS_FIRE && (m.cell_x, m.cell_y) == (x as i16, y as i16))
}

/// Stand one figure on a cell by hand, as `firing_line` does.
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

// ------------------------------------------------------------------ boiling oil

