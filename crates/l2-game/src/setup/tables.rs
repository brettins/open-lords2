#![allow(unused_imports)]
use super::*;
use super::setup_options::*;
use super::settings::*;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::unit::TROOP_TYPES;
use crate::game::Game;

/// `g_timeLimitSeconds` (`0x004DBBF8`) — what *Time limit*'s seven strings mean
/// in seconds. The last is 0, which is *"no limit"*.
pub const TIME_LIMIT_SECONDS: [i32; 7] = [30, 60, 120, 240, 480, 600, 0];

/// `g_startingGold` (`0x004DBC18`) — *Crowns*
/// group 103's five entries here are literally *"100"*, *"500"*, *"1000"*,
/// *"2500"*, *"5000"*.
pub const STARTING_GOLD: [i32; 5] = [100, 500, 1000, 2500, 5000];

/// `g_startArmoury` (`0x004DC070`) — *Weapons*: four rows of six, in
/// `Realm::weapons` order (crossbow, mace, sword, pike, bow, mail).
pub const START_ARMOURY: [[i32; 6]; 4] = [
    [0, 0, 0, 0, 0, 0],
    [0, 0, 25, 0, 25, 0],
    [0, 0, 50, 50, 50, 0],
    [100, 100, 100, 100, 100, 0],
];

/// `g_startTroops` (`0x004DC110`) — *Army Size*: four rows of seven, in
/// `Unit::troops` order, so index 0 is unarmed peasants and 1..=6 are the six
/// weapon types.
pub const START_TROOPS: [[i32; TROOP_TYPES]; 4] = [
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 25, 0, 25, 0],
    [0, 0, 0, 50, 50, 50, 0],
    [0, 0, 0, 100, 100, 100, 0],
];

