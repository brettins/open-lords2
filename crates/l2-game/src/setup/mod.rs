//! **[V]**, all three, from the decompilation:
//!
//! | | | |
//! |---|---|---|
//! | `Setup_SetOption` | `0x00433BA2` | `(which, value)` — one `if`/`else if` chain of twelve arms, each writing one global. Called from the drop-down's click handler with the row that was clicked. |
//! | `Setup_DefaultOptions` | `0x004AE539` | writes all twelve at once. This is the *Defaults* button, and it is **not** twelve zeroes. |
//! | `Setup_CommitOptions` | `0x00499DC3` | reads the twelve and writes the eleven globals a game. Called from the *Start* button's handler, immediately before `Setup_StartGame`. |
//!
//! Labels are `L2.eng` group 102, values group 103 — the runs
//! `screens::setup::OPTION_BASE` names.
//!
//! | # | label | selection | commits to | how |
//! |---:|---|---|---|---|
//! | 0 | Advanced Farming | `0x0053F29C` | `g_optAdvancedFarming` `0x0053F25C` | as is |
//! | 1 | Exploration | `0x0053F2A4` | `g_optExploration` `0x0053F264` | as is |
//! | 2 | Nobles | `0x0053F288` | `0x0053F268` | `(n + 2) - humanPlayers` — the **AI** lord count |
//! | 3 | Armies Eat | `0x0053F2A0` | `g_optArmiesEat` `0x0053F260` | as is |
//! | 4 | Difficulty | `0x0053F290` | `g_optDifficulty` `0x0053F23C` | as is |
//! | 5 | Army Size | `0x0053F298` | `0x0053F274` | a row of [`START_TROOPS`] |
//! | 6 | Starting Castle | `0x0053F2B0` | `0x0053F280` | the start county's `castleType` |
//! | 7 | Weapons | `0x0053F294` | `0x0053F270` | a row of [`START_ARMOURY`] |
//! | 8 | Crowns | `0x0053F2A8` | `0x0053F278` | through [`STARTING_GOLD`] |
//! | 9 | County Status | `0x0053F2AC` | `0x0053F27C` | a row of [`COUNTY_STATUS`] |
//! | 10 | Time limit | `0x0053F28C` | `g_optTimeLimit` `0x0053F26C` | through [`TIME_LIMIT_SECONDS`] |
//! | 11 | Fight? | `0x0053F2B4` | `g_optFightHumansOnly` `0x0053F284` | as is
//!
//! Every table was read out of a GOG `Lords2.exe` at the address the
//! decompilation names, and each one has **
//! drop-down has strings** — which is the check that could have failed and did
//! not. [`tests`] asserts it against `screens::setup::OPTION_COUNT`, which is
//! derived independently from the drop-down *geometry* table at `0x004D3158`.
//!
//! They are also contiguous in the image: [`START_ARMOURY`] at `0x004DC070` is
//! four rows of `0x18` and ends at `0x004DC0D0`, where [`COUNTY_STATUS`]
//! begins; three rows of `0x14` end at `0x004DC10C`, four bytes short of
//! [`START_TROOPS`] at `0x004DC110`. Three adjacent tables, three row counts,
//! no overlap and no remainder.
//!
//! Six of the twelve are **starting conditions** — gold, castle, armoury,
//! garrison, county stores, how many lords — and they are spent once, by
//! `FUN_0049BD99`, while the world is being built. [`Settings::apply_to`] is
//! our version of that function over a world the scenario loader has already
//! built, and it says at each field which half it is doing.

mod tables;
pub use tables::*;
mod setup_options;
pub use setup_options::*;
mod settings;
pub use settings::*;

use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::unit::TROOP_TYPES;

use crate::game::Game;

pub const OPTION_COUNT: usize = 12;

/// The twelve, in the order the grid draws them and `Setup_SetOption` switches
/// on them — which is also `L2.eng` group 102's order.
pub mod option {
    pub const ADVANCED_FARMING: usize = 0;
    pub const EXPLORATION: usize = 1;
    pub const NOBLES: usize = 2;
    pub const ARMIES_EAT: usize = 3;
    pub const DIFFICULTY: usize = 4;
    pub const ARMY_SIZE: usize = 5;
    pub const STARTING_CASTLE: usize = 6;
    pub const WEAPONS: usize = 7;
    pub const CROWNS: usize = 8;
    pub const COUNTY_STATUS: usize = 9;
    pub const TIME_LIMIT: usize = 10;
    pub const FIGHT: usize = 11;
}

/// **A second, independent reading.** `screens::setup::OPTION_COUNT` gets the
/// same twelve numbers out of the *drop-down geometry* table at `0x004D3158`
/// (its row count minus the two border cells); these are the lengths of the
/// `L2.eng` group 103 runs the value tables are indexed by. The two agree, and
/// [`tests::the_two_readings_of_the_value_counts_agree`] is what says so.
pub const VALUE_COUNT: [usize; OPTION_COUNT] = [2, 2, 4, 2, 4, 4, 6, 4, 5, 3, 7, 2];

/// `Setup_DefaultOptions` (`0x004AE539`) — **the *Defaults* button, verbatim.**
pub const DEFAULTS: [u8; OPTION_COUNT] = [0, 0, 3, 0, 0, 0, 3, 2, 2, 1, 6, 1];

/// `DAT_0053F28C = g_multiplayer ? 3 : 6` and
/// `DAT_0053F2B4 = (g_multiplayer == 0)`.
pub const DEFAULTS_MULTIPLAYER: [u8; OPTION_COUNT] = [0, 0, 3, 0, 0, 0, 3, 2, 2, 1, 3, 0];

/// One row of `0x004DC0D0` — what every county on the map starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountyStart {
    pub grain: i32,
    pub herd: i32,
    pub population: i32,
    pub health_meter: i32,
    pub happiness: i32,
}

/// `g_countyStatus` (`0x004DC0D0`) — *County Status*: three rows of five, read
/// as `{grain, herd, population, healthMeter, happiness}` because that is the
/// order `FUN_0049BD99` assigns them in.
pub const COUNTY_STATUS: [CountyStart; 3] = [
    CountyStart { grain: 10, herd: 40, population: 167, health_meter: 45, happiness: 41 },
    CountyStart { grain: 0, herd: 95, population: 417, health_meter: 65, happiness: 65 },
    CountyStart { grain: 500, herd: 330, population: 1181, health_meter: 85, happiness: 85 },
];

/// An unowned county gets a hundred sacks on top of whatever
/// [`COUNTY_STATUS`] gave it — the last loop of `FUN_0049BD99`
/// place the neutral counties are treated differently at setup.
pub const UNOWNED_COUNTY_GRAIN_BONUS: i32 = 100;

/// `FUN_0049BD99` writes `0x32` into `iron`, `wood` and `stone` unconditionally.
pub const STARTING_MATERIALS: i32 = 50;

pub const AI_EXTRA_MAIL_PER_DIFFICULTY: i32 = 20;

/// Which of `Realm::weapons` that extra lands in. `FUN_0049BD99` writes index
/// 4, which is *bow* in [`START_ARMOURY`]'s order — carried as the index the
/// binary uses, because the name is `docs/kingdom.md`'s
/// to settle and the index is not in doubt.
pub const AI_EXTRA_WEAPON_SLOT: usize = 4;


/// **The twelve drop-down selections**, and nothing else: this is
/// `0x0053F288 … 0x0053F2B4`, twelve indices into twelve string runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupOptions {
    value: [u8; OPTION_COUNT],
}

impl Default for SetupOptions {
    fn default() -> SetupOptions {
        SetupOptions::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub advanced_farming: bool,
    pub exploration: bool,
    pub armies_eat: bool,
    pub difficulty: u8,
    pub fight_humans_only_byte: u8,
    pub time_limit: i32,
    /// `docs/decisions.md` C62.
    pub quirks: l2_kingdom::Quirks,
    pub gold: i32,
    pub castle_type: u8,
    pub armoury: [i32; 6],
    pub garrison: [i32; TROOP_TYPES],
    pub county: CountyStart,
    pub ai_lords: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::setup as screen;

    #[test]
    fn the_two_readings_of_the_value_counts_agree() {
        // `screen::OPTION_COUNT` comes out of the drop-down geometry table at
        // 0x004D3158 (rows minus the two border cells); `VALUE_COUNT` is the
        // length of each group 103 run. Different tables, same twelve numbers.
        assert_eq!(VALUE_COUNT, screen::OPTION_COUNT);
    }

    #[test]
    fn every_value_table_is_exactly_as_long_as_its_drop_down() {
        assert_eq!(TIME_LIMIT_SECONDS.len(), VALUE_COUNT[option::TIME_LIMIT]);
        assert_eq!(STARTING_GOLD.len(), VALUE_COUNT[option::CROWNS]);
        assert_eq!(START_ARMOURY.len(), VALUE_COUNT[option::WEAPONS]);
        assert_eq!(START_TROOPS.len(), VALUE_COUNT[option::ARMY_SIZE]);
        assert_eq!(COUNTY_STATUS.len(), VALUE_COUNT[option::COUNTY_STATUS]);
        assert_eq!(VALUE_COUNT[option::NOBLES], 4, "two, three, four, five");
        assert_eq!(
            VALUE_COUNT[option::STARTING_CASTLE],
            6,
            "none, wooden, bailey, keep, stone, royal"
        );
    }

    #[test]
    fn the_defaults_are_the_originals_and_not_twelve_zeroes() {
        let o = SetupOptions::new();
        assert_eq!(o.lords(), 5, "the default game is five lords");
        assert_eq!(o.get(option::STARTING_CASTLE), 3, "a keep");
        let s = o.commit(1, l2_kingdom::Quirks::FAITHFUL);
        assert_eq!(s.gold, 1000);
        assert_eq!(s.time_limit, 0, "no limit in a single-player game");
        assert_eq!(s.armoury, START_ARMOURY[2], "some weapons");
        assert_eq!(s.county, COUNTY_STATUS[1], "a medium county");
        assert_eq!(s.difficulty, 0);
        assert_eq!(s.ai_lords, 4);
        assert_ne!(DEFAULTS, [0; OPTION_COUNT], "C21: the reset used to be this");
        for i in 0..OPTION_COUNT {
            assert!((DEFAULTS[i] as usize) < VALUE_COUNT[i], "default {i}");
        }
    }

    #[test]
    fn the_multiplayer_defaults_differ_in_exactly_two_places() {
        let differ: Vec<usize> =
            (0..OPTION_COUNT).filter(|&i| DEFAULTS[i] != DEFAULTS_MULTIPLAYER[i]).collect();
        assert_eq!(differ, vec![option::TIME_LIMIT, option::FIGHT]);
        assert_eq!(TIME_LIMIT_SECONDS[DEFAULTS_MULTIPLAYER[option::TIME_LIMIT] as usize], 240);
    }

    #[test]
    fn the_start_troops_are_the_start_armoury_shifted_one_slot_right() {
        for row in 0..3 {
            for w in 0..6 {
                assert_eq!(
                    START_TROOPS[row][w + 1],
                    START_ARMOURY[row][w],
                    "row {row} weapon {w}",
                );
            }
        }
        assert_eq!(START_ARMOURY[3], [100, 100, 100, 100, 100, 0]);
        assert_eq!(START_TROOPS[3], [0, 0, 0, 100, 100, 100, 0]);
        assert_eq!(START_ARMOURY[3].iter().sum::<i32>(), 500);
        assert_eq!(START_TROOPS[3].iter().sum::<i32>(), 300);
        for row in 0..4 {
            assert_eq!(START_TROOPS[row][0], 0, "nobody starts with unarmed peasants");
            assert_eq!(START_ARMOURY[row][5], 0, "and nobody starts with mail");
        }
    }

    #[test]
    fn the_map_sets_the_lord_count_in_both_directions() {
        let mut o = SetupOptions::new();
        assert_eq!(o.lords(), 5);
        o.set_nobles_from_map(2);
        assert_eq!(o.lords(), 2, "a two-seat map is a two-lord game");
        o.set_nobles_from_map(5);
        assert_eq!(o.lords(), 5, "and it goes back up again");
        for seats in 0..=5 {
            o.set_nobles_from_map(seats);
            assert!(o.lords() <= seats.max(2), "{seats} seats seated {}", o.lords());
        }
        assert_eq!(SetupOptions::nobles_rows_for_map(5), 4);
        assert_eq!(SetupOptions::nobles_rows_for_map(4), 3);
        assert_eq!(SetupOptions::nobles_rows_for_map(2), 1);
    }

    #[test]
    fn a_selection_is_always_inside_its_own_run() {
        let mut o = SetupOptions::new();
        for i in 0..OPTION_COUNT {
            o.set(i, 99);
            assert_eq!(o.get(i), VALUE_COUNT[i] - 1, "option {i}");
        }
        let s = o.commit(1, l2_kingdom::Quirks::FAITHFUL);
        assert_eq!(s.gold, 5000);
        assert_eq!(s.time_limit, 0);
        assert_eq!(s.difficulty, 3);
    }

    #[test]
    fn the_ai_lord_count_is_the_lords_minus_the_people() {
        let mut o = SetupOptions::new();
        o.set(option::NOBLES, 3);
        assert_eq!(o.commit(1, l2_kingdom::Quirks::FAITHFUL).ai_lords, 4);
        assert_eq!(o.commit(3, l2_kingdom::Quirks::FAITHFUL).ai_lords, 2);
        o.set(option::NOBLES, 0);
        assert_eq!(o.commit(1, l2_kingdom::Quirks::FAITHFUL).ai_lords, 1);
    }

    #[test]
    fn difficulty_reaches_the_ai_armoury() {
        let mut o = SetupOptions::new();
        o.set(option::WEAPONS, 0);
        for d in 0..4usize {
            o.set(option::DIFFICULTY, d);
            let s = o.commit(1, l2_kingdom::Quirks::FAITHFUL);
            assert_eq!(s.armoury[AI_EXTRA_WEAPON_SLOT], 0, "the table row itself is untouched");
            assert_eq!(s.difficulty as usize, d);
        }
    }
}

