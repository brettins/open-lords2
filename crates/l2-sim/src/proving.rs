//! **A proving ground** — a siege built so that boiling oil, a burning bridge,
//! men burning, a docked siege tower and a rampart too high to shoot down all
//! have to happen, inside two thousand frames, on a timetable.
//!
//! # ⚠ Ours, like [`crate::siege::our_castle`], and for the same reason
//!
//! `Battlefield_BuildCastle`'s layout rasters are unread, and the castle this
//! crate draws for itself has **no cell two high** — its walls stand at 1 —
//! so a tower there has nothing it may dock with, and an AI's oil, which will
//! not pour from below elevation 2 (`Oil_FindPourTarget`), never finds a
//! target. That is a property of our layout, not of the rules, and it is why
//! the rules need a field of their own to be seen at all. Nothing here is a
//! statement about what a castle in Lords of the Realm II looks like.
//!
//! What is the original's is every rule the field exercises, and every number
//! a test asserts against it.
//!
//! # The field, north at the top
//!
//! ```text
//!  y 29   . . r r r r r r r r r r r r r r r r r r r r r r r r r r r r . .   the rampart walk, height 2, surface 4
//!  y 30   . . # # # # # # # # # # # # # # # # # # # # # # # # # # # # . .   the curtain, height 2, impassable
//!                     ^ the tower docks here          ^ the pot pours from above
//!  y 40…46                                                   ~ = ~    a bridge, surface 7, over a ditch
//!  y 52   H H H H H                                                   a wall four high, flag 0x20
//! ```
//!
//! Both armies are a human's, so **no order handler runs** and every figure
//! does what [`orders`] tells it and nothing else. That is the point: a
//! determinism proof over a battle whose events can be named in advance.

use crate::runner::BattleRunner;
use crate::terrain::{flag, id, Battlefield, DIM};
use crate::{Muster, Side, Troop, SIDE_A, SIDE_B};

/// A palisade, so an assault that runs out of engines repeats
/// ending (`ASSAULT_REPEATS_BELOW_LEVEL`).
pub const LEVEL: u8 = 1;
pub const SEED: u64 = 0x0F1E_5EED;

/// The rampart walk and the curtain in front of it, both two high.
pub const WALK_Y: u8 = 29;
pub const CURTAIN_Y: u8 = 30;
pub const RAMPART_X: core::ops::RangeInclusive<u8> = 18..=52;

/// The tower deploys here and is sent due north, through the curtain.
pub const TOWER_AT: (u8, u8) = (25, 38);
pub const TOWER_TO: (u8, u8) = (25, 20);
/// Where it stops: its leading edge two cells ahead is the curtain.
pub const TOWER_DOCKS_AT: (u8, u8) = (25, 32);
/// The knight who climbs the ramp once it is there.
pub const KNIGHT_AT: (u8, u8) = (25, 47);
pub const KNIGHT_TO: (u8, u8) = (25, WALK_Y);

/// The garrison's pot, on the walk, and the cell it is ordered down to.
pub const POT_AT: (u8, u8) = (35, WALK_Y);
pub const POUR_AT: (u8, u8) = (35, 34);
/// The peasants standing under it.
pub const PEASANTS_AT: (u8, u8) = (35, 35);

/// The bridge and the man who walks onto it.
pub const BRIDGE_X: u8 = 50;
pub const BRIDGE_Y: core::ops::RangeInclusive<u8> = 40..=46;
pub const SWORDSMAN_AT: (u8, u8) = (50, 50);
pub const SWORDSMAN_TO: (u8, u8) = (50, 36);

/// The catapult, the wall four high in front of it, and the archer behind
/// that wall which it will choose as its target.
pub const CATAPULT_AT: (u8, u8) = (6, 62);
pub const HIGH_WALL_Y: u8 = 52;
pub const HIGH_WALL_X: core::ops::RangeInclusive<u8> = 4..=8;
pub const HIGH_WALL_ELEVATION: u8 = 4;
pub const ARCHER_AT: (u8, u8) = (6, 44);

/// The timetable [`orders`] keeps.
pub const MARCH_TICK: u32 = 1;
pub const POUR_TICK: u32 = 60;
pub const CLIMB_TICK: u32 = 420;

/// The besieger, in `(troop, men)`. Thirty-four men and two engines against
/// five is size class 0: four men a figure, three hits a frame in fire.
pub const BESIEGER: [(Troop, u32); 5] = [
    (Troop::Knights, 4),
    (Troop::Swordsmen, 4),
    (Troop::Peasants, 24),
    (Troop::SiegeTowers, 1),
    (Troop::Catapults, 1),
];
pub const GARRISON: [(Troop, u32); 2] = [(Troop::Oil, 1), (Troop::Archers, 4)];

/// The field. See the module header.
pub fn field() -> Battlefield {
    let mut f = crate::runner::blank_field();
    let at = |x: u8, y: u8| y as usize * DIM + x as usize;
    for x in RAMPART_X {
        let walk = &mut f.cells[at(x, WALK_Y)];
        walk.elevation = 2;
        walk.surface = crate::siege::SURFACE_RAMPART_WALK;
        let curtain = &mut f.cells[at(x, CURTAIN_Y)];
        curtain.elevation = 2;
        curtain.surface = crate::siege::SURFACE_WALL;
        curtain.flags = flag::IMPASSABLE;
    }
    for y in BRIDGE_Y {
        let bridge = &mut f.cells[at(BRIDGE_X, y)];
        bridge.terrain = id::BRIDGE_SPAN;
        bridge.surface = crate::fire::SURFACE_BRIDGE;
        for x in [BRIDGE_X - 2, BRIDGE_X - 1, BRIDGE_X + 1, BRIDGE_X + 2] {
            let ditch = &mut f.cells[at(x, y)];
            ditch.terrain = id::WATER;
            ditch.surface = crate::siege::SURFACE_WATER;
            ditch.flags = flag::IMPASSABLE;
        }
    }
    for x in HIGH_WALL_X {
        let wall = &mut f.cells[at(x, HIGH_WALL_Y)];
        wall.elevation = HIGH_WALL_ELEVATION;
        wall.surface = crate::siege::SURFACE_WALL;
        wall.flags = crate::siege::FLAG_WALL;
    }
    // One deployment slot per unit, in `RAISE_ORDER`: knights, swordsmen,
    // peasants, tower, catapult for the besieger; oil, archers for the garrison.
    for (k, slot) in [KNIGHT_AT, SWORDSMAN_AT, PEASANTS_AT, TOWER_AT, CATAPULT_AT].into_iter().enumerate() {
        f.deploy_side4[k] = slot;
    }
    for (k, slot) in [POT_AT, ARCHER_AT].into_iter().enumerate() {
        f.deploy_side0[k] = slot;
    }
    f
}

/// The siege, deployed and not yet started.
pub fn deploy() -> BattleRunner {
    BattleRunner::deploy_siege(
        field(),
        SEED,
        Muster { troops: &BESIEGER, owner: 1, human: true },
        Muster { troops: &GARRISON, owner: 2, human: true },
        LEVEL,
    )
}

/// The unit a side's first figure of `troop` belongs to, or 0.
pub fn unit_of(r: &BattleRunner, side: Side, troop: Troop) -> usize {
    r.fighters
        .iter()
        .position(|f| f.side == side && f.troop == troop)
        .map_or(0, |i| r.unit_of(i))
}

/// The fighter index of a side's first figure of `troop`.
pub fn fighter_of(r: &BattleRunner, side: Side, troop: Troop) -> Option<usize> {
    r.fighters.iter().position(|f| f.side == side && f.troop == troop)
}

/// **The timetable** — call before every [`BattleRunner::step`]. The orders
/// are the player's, through [`BattleRunner::order_unit`], which is
/// `BattleUnit_Order`'s path and so the path a pot pours from.
pub fn orders(r: &mut BattleRunner) {
    match r.tick {
        MARCH_TICK => {
            let tower = unit_of(r, SIDE_B, Troop::SiegeTowers);
            r.order_unit(tower, TOWER_TO.0, TOWER_TO.1);
            let swordsman = unit_of(r, SIDE_B, Troop::Swordsmen);
            r.order_unit(swordsman, SWORDSMAN_TO.0, SWORDSMAN_TO.1);
        }
        POUR_TICK => {
            let pot = unit_of(r, SIDE_A, Troop::Oil);
            r.order_unit(pot, POUR_AT.0, POUR_AT.1);
        }
        CLIMB_TICK => {
            let knight = unit_of(r, SIDE_B, Troop::Knights);
            r.order_unit(knight, KNIGHT_TO.0, KNIGHT_TO.1);
        }
        _ => {}
    }
}

/// [`deploy`], then `ticks` frames of [`orders`] and [`BattleRunner::step`].
pub fn run(ticks: u32) -> BattleRunner {
    let mut r = deploy();
    for _ in 0..ticks {
        orders(&mut r);
        r.step();
    }
    r
}
