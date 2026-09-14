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
///
/// It is also the campaign table's third column, which is how
/// `crate::victory`'s `CampaignMap::gold` came to hold 5000 / 2500 / 1000
/// without anybody having to name this table first.
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
///
/// **Shifted one slot right of [`START_ARMOURY`]** for rows 0, 1 and 2, which
/// is the check that they are the same six weapons: row 2 is 50 swords, 50
/// pikes and 50 bows in both, at indices 3, 4, 5 here and 2, 3, 4 there.
///
/// **Row 3 breaks the pattern and it is in the image.** *Many* weapons is 100
/// of each of the five real types; *large* is 100 each of swords, pikes and
/// bows only. Take both and you get 300 men and 200 crossbows and maces sitting
/// in the armoury — the two tables were chosen separately, and reading either
/// off the other would have been wrong here.
pub const START_TROOPS: [[i32; TROOP_TYPES]; 4] = [
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 25, 0, 25, 0],
    [0, 0, 0, 50, 50, 50, 0],
    [0, 0, 0, 100, 100, 100, 0],
];

