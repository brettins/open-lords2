
mod handlers;
mod breaches_and_assaults;
mod wall_damage;
mod outcomes;
mod player_tactics;
mod order_pullback;

use l2_sim::siege::{
    self, SURFACE_RAMPART_WALK,
};
use l2_sim::{BattleRunner, Muster, Troop, SIDE_A, SIDE_B};

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
            mercenaries: None,
        },
        Muster {
            troops: &defender,
            owner: 2,
            human: false,
            mercenaries: None,
        },
        level,
    )
}
