
mod handlers;
pub use handlers::*;
mod breaches_and_assaults;
pub use breaches_and_assaults::*;
mod wall_damage;
pub use wall_damage::*;
mod outcomes;
pub use outcomes::*;
mod player_tactics;
pub use player_tactics::*;
mod order_pullback;
pub use order_pullback::*;

use l2_sim::ai::{handler_for, TABLE_FIELD, TABLE_SIEGE_ATT, TABLE_SIEGE_DEF};
use l2_sim::runner::{ASSAULT_REPEATS_BELOW_LEVEL, ASSAULT_REPEAT_SCORE};
use l2_sim::siege::{
    self, SiegeState, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY, SURFACE_RAMPART_WALK, SURFACE_WALL,
};
use l2_sim::{BattleRunner, End, Muster, Troop, SIDE_A, SIDE_B};

fn siege_battle(level: u8, seed: u64) -> BattleRunner {
    let attacker = [
        (Troop::Peasants, 240u32),
        (Troop::Archers, 80),
        (Troop::Swordsmen, 80),
        (Troop::Knights, 40),
        (Troop::Catapults, 2),
        (Troop::SiegeTowers, 2),
        (Troop::BatteringRams, 1),
    ];
    let defender = [
        (Troop::Archers, 500u32),
        (Troop::Crossbowmen, 200),
        (Troop::Swordsmen, 40),
        (Troop::Pikemen, 40),
        (Troop::Knights, 20),
        (Troop::Oil, 3),
    ];
    BattleRunner::deploy_siege(
        siege::our_castle(level),
        seed,
        Muster {
            troops: &attacker,
            owner: 1,
            human: false,
        },
        Muster {
            troops: &defender,
            owner: 2,
            human: false,
        },
        level,
    )
}
