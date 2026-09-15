
use crate::runner::BattleRunner;
use crate::terrain::{flag, id, Battlefield, DIM};
use crate::{Muster, Side, Troop, SIDE_A, SIDE_B};

pub const LEVEL: u8 = 1;
pub const SEED: u64 = 0x0F1E_5EED;

pub const WALK_Y: u8 = 29;
pub const CURTAIN_Y: u8 = 30;
pub const RAMPART_X: core::ops::RangeInclusive<u8> = 18..=52;

pub const TOWER_AT: (u8, u8) = (25, 38);
pub const TOWER_TO: (u8, u8) = (25, 20);
pub const TOWER_DOCKS_AT: (u8, u8) = (25, 32);
pub const KNIGHT_AT: (u8, u8) = (25, 47);
pub const KNIGHT_TO: (u8, u8) = (25, WALK_Y);

pub const POT_AT: (u8, u8) = (35, WALK_Y);
pub const POUR_AT: (u8, u8) = (35, 34);
pub const PEASANTS_AT: (u8, u8) = (35, 35);

pub const BRIDGE_X: u8 = 50;
pub const BRIDGE_Y: core::ops::RangeInclusive<u8> = 40..=46;
pub const SWORDSMAN_AT: (u8, u8) = (50, 50);
pub const SWORDSMAN_TO: (u8, u8) = (50, 36);

pub const CATAPULT_AT: (u8, u8) = (6, 62);
pub const HIGH_WALL_Y: u8 = 52;
pub const HIGH_WALL_X: core::ops::RangeInclusive<u8> = 4..=8;
pub const HIGH_WALL_ELEVATION: u8 = 4;
pub const ARCHER_AT: (u8, u8) = (6, 44);

pub const MARCH_TICK: u32 = 1;
pub const POUR_TICK: u32 = 60;
pub const CLIMB_TICK: u32 = 420;

pub const BESIEGER: [(Troop, u32); 5] = [
    (Troop::Knights, 4),
    (Troop::Swordsmen, 4),
    (Troop::Peasants, 24),
    (Troop::SiegeTowers, 1),
    (Troop::Catapults, 1),
];
pub const GARRISON: [(Troop, u32); 2] = [(Troop::Oil, 1), (Troop::Archers, 4)];

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
    for (k, slot) in [KNIGHT_AT, SWORDSMAN_AT, PEASANTS_AT, TOWER_AT, CATAPULT_AT].into_iter().enumerate() {
        f.deploy_side4[k] = slot;
    }
    for (k, slot) in [POT_AT, ARCHER_AT].into_iter().enumerate() {
        f.deploy_side0[k] = slot;
    }
    f
}

pub fn deploy() -> BattleRunner {
    BattleRunner::deploy_siege(
        field(),
        SEED,
        Muster { troops: &BESIEGER, owner: 1, human: true },
        Muster { troops: &GARRISON, owner: 2, human: true },
        LEVEL,
    )
}

pub fn unit_of(r: &BattleRunner, side: Side, troop: Troop) -> usize {
    r.fighters
        .iter()
        .position(|f| f.side == side && f.troop == troop)
        .map_or(0, |i| r.unit_of(i))
}

pub fn fighter_of(r: &BattleRunner, side: Side, troop: Troop) -> Option<usize> {
    r.fighters.iter().position(|f| f.side == side && f.troop == troop)
}

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

pub fn run(ticks: u32) -> BattleRunner {
    let mut r = deploy();
    for _ in 0..ticks {
        orders(&mut r);
        r.step();
    }
    r
}
