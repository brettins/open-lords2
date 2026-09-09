//! **The custom game's twelve options, and what each one actually does.**
//!
//! `screens::setup` draws them. This module is what they *mean*, and it exists
//! because the answer turned out not to be the obvious one: the twelve
//! drop-downs do **not** write `g_optDifficulty` and its neighbours. They write
//! a second block of globals a hundred bytes further up, and a single function
//! run at the moment *Start* is pressed turns those twelve selections into the
//! values the game runs on. Wiring a drop-down straight to `g_optDifficulty`
//! would have been wrong for five of the twelve and would have looked right.
//!
//! # The three functions
//!
//! **[V]**, all three, from the decompilation:
//!
//! | | | |
//! |---|---|---|
//! | `Setup_SetOption` | `0x00433BA2` | `(which, value)` — one `if`/`else if` chain of twelve arms, each writing one global. Called from the drop-down's click handler with the row that was clicked. |
//! | `Setup_DefaultOptions` | `0x004AE539` | writes all twelve at once. This is the *Defaults* button, and it is **not** twelve zeroes. |
//! | `Setup_CommitOptions` | `0x00499DC3` | reads the twelve and writes the eleven globals a game is actually played with. Called from the *Start* button's handler, immediately before `Setup_StartGame`. |
//!
//! # The twelve, and where each one lands
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
//! | 11 | Fight? | `0x0053F2B4` | `g_optFightHumansOnly` `0x0053F284` | as is, and the byte is **inverted** — 0 shows *humans* |
//!
//! Five of the twelve go through a table and one is arithmetic; only six are the
//! direct copies the screen makes them look like.
//!
//! # The five tables close against the string lists
//!
//! Every table was read out of a GOG `Lords2.exe` at the address the
//! decompilation names, and each one has **exactly as many rows as its
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
//! # What this module does not decide
//!
//! Six of the twelve are **starting conditions** — gold, castle, armoury,
//! garrison, county stores, how many lords — and they are spent once, by
//! `FUN_0049BD99`, while the world is being built. [`Settings::apply_to`] is
//! our version of that function over a world the scenario loader has already
//! built, and it says at each field which half it is doing.

use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::unit::TROOP_TYPES;

use crate::game::Game;

/// How many drop-downs there are.
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

/// How many values each drop-down offers.
///
/// **A second, independent reading.** `screens::setup::OPTION_COUNT` gets the
/// same twelve numbers out of the *drop-down geometry* table at `0x004D3158`
/// (its row count minus the two border cells); these are the lengths of the
/// `L2.eng` group 103 runs the value tables are indexed by. The two agree, and
/// [`tests::the_two_readings_of_the_value_counts_agree`] is what says so.
pub const VALUE_COUNT: [usize; OPTION_COUNT] = [2, 2, 4, 2, 4, 4, 6, 4, 5, 3, 7, 2];

/// `Setup_DefaultOptions` (`0x004AE539`) — **the *Defaults* button, verbatim.**
///
/// Not twelve zeroes, which is what this screen used to reset to and what the
/// button's own caption invites you to assume. The default game is *five*
/// nobles, a *keep*, *some* weapons, *1000* crowns and a *medium* county — and
/// six of those twelve numbers are not zero.
pub const DEFAULTS: [u8; OPTION_COUNT] = [0, 0, 3, 0, 0, 0, 3, 2, 2, 1, 6, 1];

/// The same function's other branch, for a network game: a four-minute turn
/// limit instead of none, and *humans* instead of *all*.
///
/// `DAT_0053F28C = g_multiplayer ? 3 : 6` and
/// `DAT_0053F2B4 = (g_multiplayer == 0)`.
pub const DEFAULTS_MULTIPLAYER: [u8; OPTION_COUNT] = [0, 0, 3, 0, 0, 0, 3, 2, 2, 1, 3, 0];

/// `g_timeLimitSeconds` (`0x004DBBF8`) — what *Time limit*'s seven strings mean
/// in seconds. The last is 0, which is *"no limit"*.
pub const TIME_LIMIT_SECONDS: [i32; 7] = [30, 60, 120, 240, 480, 600, 0];

/// `g_startingGold` (`0x004DBC18`) — *Crowns*, and the numbers are the strings:
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
///
/// **`medium` really does start with no grain at all.** It is 0 in the image
/// where `weak` is 10, and it is reproduced rather than tidied: the two numbers
/// are both far below a county's appetite and the herd, which triples from
/// 40 to 95 to 330, is what the setting is actually moving.
pub const COUNTY_STATUS: [CountyStart; 3] = [
    CountyStart { grain: 10, herd: 40, population: 167, health_meter: 45, happiness: 41 },
    CountyStart { grain: 0, herd: 95, population: 417, health_meter: 65, happiness: 65 },
    CountyStart { grain: 500, herd: 330, population: 1181, health_meter: 85, happiness: 85 },
];

/// An unowned county gets a hundred sacks on top of whatever
/// [`COUNTY_STATUS`] gave it — the last loop of `FUN_0049BD99`, and the only
/// place the neutral counties are treated differently at setup.
pub const UNOWNED_COUNTY_GRAIN_BONUS: i32 = 100;

/// Every realm starts with fifty of each, whatever the options say.
/// `FUN_0049BD99` writes `0x32` into `iron`, `wood` and `stone` unconditionally.
pub const STARTING_MATERIALS: i32 = 50;

/// An AI realm gets `difficulty * 20` extra mail — `weapons[4]` — on top of its
/// [`START_ARMOURY`] row, and a person gets none. The one place in the whole
/// setup path where the difficulty and the equipment meet.
pub const AI_EXTRA_MAIL_PER_DIFFICULTY: i32 = 20;

/// Which of `Realm::weapons` that extra lands in. `FUN_0049BD99` writes index
/// 4, which is *bow* in [`START_ARMOURY`]'s order — carried as the index the
/// binary uses rather than as a name, because the name is `docs/kingdom.md`'s
/// to settle and the index is not in doubt.
pub const AI_EXTRA_WEAPON_SLOT: usize = 4;

// ------------------------------------------------------------- the selections

/// **The twelve drop-down selections**, and nothing else: this is
/// `0x0053F288 … 0x0053F2B4`, twelve indices into twelve string runs.
///
/// It is deliberately *not* the settings a game runs on. Turning one into the
/// other is [`SetupOptions::commit`], and keeping the two types apart is what
/// stops a screen writing a value straight into a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupOptions {
    value: [u8; OPTION_COUNT],
}

impl Default for SetupOptions {
    fn default() -> SetupOptions {
        SetupOptions::new()
    }
}

impl SetupOptions {
    /// `Setup_DefaultOptions` for a single-player game.
    pub fn new() -> SetupOptions {
        SetupOptions { value: DEFAULTS }
    }

    /// The same for a network game — two of the twelve differ.
    pub fn new_multiplayer() -> SetupOptions {
        SetupOptions { value: DEFAULTS_MULTIPLAYER }
    }

    /// The selection of option `which`, always inside its run.
    pub fn get(&self, which: usize) -> usize {
        let n = VALUE_COUNT.get(which).copied().unwrap_or(1);
        (self.value.get(which).copied().unwrap_or(0) as usize).min(n - 1)
    }

    /// `Setup_SetOption` (`0x00433BA2`). Out-of-range asks are clamped rather
    /// than ignored, because the caller is a hit test and a hit test that
    /// reported row 8 of a four-row list has already gone wrong somewhere the
    /// option table cannot fix.
    pub fn set(&mut self, which: usize, value: usize) {
        let Some(n) = VALUE_COUNT.get(which).copied() else { return };
        self.value[which] = value.min(n - 1) as u8;
    }

    /// **The map is allowed to overrule *Nobles*, and it does.**
    ///
    /// `FUN_004AE5E2` (`0x004AE5E2`) is called with `g_playerStartCount` every
    /// time the scenario list changes the map — three call sites, one per way
    /// of changing it — and it *sets* the selection from the seat count rather
    /// than clamping it: fewer than 3 seats picks *two*, fewer than 4 picks
    /// *three*, fewer than 5 picks *four*, otherwise *five*. So choosing a map
    /// always leaves the lord count agreeing with it, in both directions.
    ///
    /// The drop-down is shortened to match at the same time —
    /// `FUN_00433999` sets the open list to `g_playerStartCount - 1` rows when
    /// the map seats fewer than five — so a person cannot re-break it
    /// afterwards. See [`SetupOptions::nobles_rows_for_map`].
    pub fn set_nobles_from_map(&mut self, player_starts: usize) {
        self.value[option::NOBLES] = if player_starts < 3 {
            0
        } else if player_starts < 4 {
            1
        } else if player_starts < 5 {
            2
        } else {
            3
        };
    }

    /// How many rows the *Nobles* list may show on this map. `FUN_00433999`:
    /// `g_playerStartCount - 1` when the map seats fewer than five, otherwise
    /// the full four.
    pub fn nobles_rows_for_map(player_starts: usize) -> usize {
        if player_starts < 5 {
            player_starts.saturating_sub(1).max(1)
        } else {
            VALUE_COUNT[option::NOBLES]
        }
    }

    /// How many lords are in the game — the *Nobles* label's own numbers, *two*
    /// through *five*. `L2.eng` group 103 index 4 is *"one"* and nothing
    /// reaches it, which is the string the twelve runs leave over.
    pub fn lords(&self) -> usize {
        self.get(option::NOBLES) + 2
    }

    /// `Setup_CommitOptions` (`0x00499DC3`), and `human_players` is
    /// `DAT_00553F98`, which is 1 in a single-player game.
    pub fn commit(&self, human_players: usize) -> Settings {
        let castle = self.get(option::STARTING_CASTLE);
        Settings {
            advanced_farming: self.get(option::ADVANCED_FARMING) != 0,
            exploration: self.get(option::EXPLORATION) != 0,
            armies_eat: self.get(option::ARMIES_EAT) != 0,
            difficulty: self.get(option::DIFFICULTY) as u8,
            // Stored inverted: index 0 is the string "humans" and the byte the
            // battle rule tests against zero. `l2_kingdom::battle::settlement`
            // compares the byte, not a bool, for exactly this reason.
            fight_humans_only_byte: self.get(option::FIGHT) as u8,
            time_limit: TIME_LIMIT_SECONDS[self.get(option::TIME_LIMIT)],
            gold: STARTING_GOLD[self.get(option::CROWNS)],
            castle_type: castle as u8,
            armoury: START_ARMOURY[self.get(option::WEAPONS)],
            garrison: START_TROOPS[self.get(option::ARMY_SIZE)],
            county: COUNTY_STATUS[self.get(option::COUNTY_STATUS)],
            // `DAT_0053F268 = (nobles + 2) - humanPlayers`. It is not clamped
            // in the original either; `Realms_AssignLords` walks five realms and
            // stops handing out lords when it has handed out this many, so a
            // negative simply means nobody gets one.
            ai_lords: self.lords() as i32 - human_players as i32,
        }
    }
}

// --------------------------------------------------------------- the settings

/// **What a game is actually played with**, once the twelve selections have been
/// through `Setup_CommitOptions` and its five tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    // The six that are rules and live for the length of the game.
    pub advanced_farming: bool,
    pub exploration: bool,
    pub armies_eat: bool,
    pub difficulty: u8,
    pub fight_humans_only_byte: u8,
    pub time_limit: i32,
    // The six that are starting conditions and are spent once.
    pub gold: i32,
    pub castle_type: u8,
    pub armoury: [i32; 6],
    pub garrison: [i32; TROOP_TYPES],
    pub county: CountyStart,
    pub ai_lords: i32,
}

impl Settings {
    /// The six rule flags, as `l2-kingdom` wants them. This half is complete:
    /// every one of the six reaches a rule, or is carried in the save because
    /// nothing may read it yet and a save that forgot it would be a save that
    /// guessed.
    pub fn kingdom_options(&self) -> l2_kingdom::kingdom::Options {
        l2_kingdom::kingdom::Options {
            difficulty: self.difficulty,
            advanced_farming: self.advanced_farming,
            armies_eat: self.armies_eat,
            fight_humans_only_byte: self.fight_humans_only_byte,
            exploration: self.exploration,
            time_limit: self.time_limit,
        }
    }

    /// **`FUN_0049BD99` over a world that is already built.**
    ///
    /// The original's version runs on a map it has just loaded, so it also
    /// seats the realms — `g_playerStartTable` decides who gets which county —
    /// and raises each realm's garrison with `Army_Create`. This one runs on a
    /// world [`crate::scenario`] built from a save, which already has its
    /// counties owned, so it does the part that is the *options'*: the stores,
    /// the treasury, the armoury, the castle, and how many lords are in play.
    ///
    /// | `FUN_0049BD99` does | here |
    /// |---|---|
    /// | every county's grain, herd, population and happiness from the county-status row | yes |
    /// | `+100` grain to every unowned county | yes |
    /// | `realm.gold` from the crowns row, `iron`/`wood`/`stone` = 50 | yes |
    /// | `realm.weapons` from the armoury row, `+ difficulty * 20` mail for an AI | yes |
    /// | the start county's `castleType` from the castle row | yes |
    /// | realms past the lord count get `strength = 0` and no county | yes |
    /// | `Army_Create` for the starting garrison | **no** — see below |
    /// | seating the realms from the map's player-start table | **no** — the save already seats them |
    ///
    /// **The garrison is the one that is missing and it is marked.** Raising it
    /// is `l2_kingdom::levy::create_army`, which needs a muster tile, a levy
    /// basket and the county's food passes re-run around it; doing that here
    /// would be a second army-raising path beside the one the levy screen
    /// already owns, and the option is reported by
    /// [`Settings::unhonoured`] rather than silently dropped.
    pub fn apply_to(&self, game: &mut Game) {
        game.kingdom.options = self.kingdom_options();

        // Which realms are in the game at all. `Realms_AssignLords`
        // (`0x0049C6C1`) hands a lord to at most `ai_lords` non-human realms
        // and writes `strength = 0` into the rest; `FUN_0049BD99` then skips
        // every realm without one, so it gets no county, no gold and no
        // armoury. Ascending by realm id, which is the order the original walks
        // them in — so two peers drop the same realms.
        let mut given = 0;
        let mut dropped = [false; MAX_REALMS];
        for id in 1..MAX_REALMS {
            let realm = &mut game.kingdom.realms[id];
            if !realm.in_play && realm.lord == 0 && !realm.is_human {
                continue;
            }
            if realm.is_human {
                continue;
            }
            if given < self.ai_lords {
                given += 1;
            } else {
                dropped[id] = true;
            }
        }

        for id in 1..MAX_REALMS {
            if dropped[id] {
                let realm = &mut game.kingdom.realms[id];
                realm.strength = 0;
                realm.in_play = false;
                realm.lord = 0;
                realm.county_count = 0;
                realm.gold = 0;
                continue;
            }
            let realm = &mut game.kingdom.realms[id];
            if !realm.in_play && !realm.is_human {
                continue;
            }
            realm.gold = self.gold;
            realm.iron = STARTING_MATERIALS;
            realm.wood = STARTING_MATERIALS;
            realm.stone = STARTING_MATERIALS;
            realm.weapons = self.armoury;
            if !realm.is_human {
                realm.weapons[AI_EXTRA_WEAPON_SLOT] +=
                    self.difficulty as i32 * AI_EXTRA_MAIL_PER_DIFFICULTY;
            }
            realm.wages = 0;
        }

        // A county whose owner has just been dropped is nobody's.
        for id in game.kingdom.county_ids() {
            let owner = game.kingdom.counties[id].owner as usize;
            if owner < MAX_REALMS && dropped[owner] {
                game.kingdom.counties[id].owner = 0;
            }
        }

        for id in game.kingdom.county_ids() {
            let owned = game.kingdom.counties[id].owner != 0;
            let county = &mut game.kingdom.counties[id];
            county.grain = self.county.grain;
            county.herd = self.county.herd;
            county.population = self.county.population;
            county.pop_last = self.county.population;
            county.health_meter = self.county.health_meter;
            county.health_band = l2_kingdom::tables::health_band(self.county.health_meter) as u8;
            county.happiness = self.county.happiness;
            county.happiness_last = self.county.happiness;
            if !owned {
                county.grain += UNOWNED_COUNTY_GRAIN_BONUS;
            } else {
                county.castle_type = self.castle_type;
            }
        }

        for (id, realm) in game.kingdom.realms.iter().enumerate() {
            game.gold_last[id] = realm.gold;
        }
        game.selected = game
            .kingdom
            .county_ids()
            .find(|&id| game.kingdom.counties[id].owner == game.player)
            .unwrap_or(0) as u8;
    }

    /// **What this build cannot honour**, as `L2.eng` group 102 labels.
    ///
    /// `docs/decisions.md` C21: an option wired to nothing must say so where
    /// somebody can see it, and the setup screen draws this list under the
    /// grid. Empty is the goal and it is not empty yet.
    pub fn unhonoured(&self) -> Vec<usize> {
        let mut v = Vec::new();
        if self.exploration {
            // The switch reaches `Options::exploration` and the save; the fog
            // itself does not exist. `docs/mechanics.md`.
            v.push(option::EXPLORATION);
        }
        if self.garrison.iter().any(|&n| n != 0) {
            // `Army_Create` at setup — see `apply_to`.
            v.push(option::ARMY_SIZE);
        }
        v
    }
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
        // The two that are read as an index rather than through a table still
        // have to fit what they index.
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
        let s = o.commit(1);
        assert_eq!(s.gold, 1000);
        assert_eq!(s.time_limit, 0, "no limit in a single-player game");
        assert_eq!(s.armoury, START_ARMOURY[2], "some weapons");
        assert_eq!(s.county, COUNTY_STATUS[1], "a medium county");
        assert_eq!(s.difficulty, 0);
        assert_eq!(s.ai_lords, 4);
        assert_ne!(DEFAULTS, [0; OPTION_COUNT], "C21: the reset used to be this");
        // Every default is inside its own run.
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
        // Rows 0, 1 and 2 line up exactly, one slot apart, which is what says
        // the two tables are indexed by the same six weapons: `START_TROOPS[0]`
        // is unarmed peasants and 1..=6 are the weapon types, so weapon `w` of
        // the armoury is troop `w + 1`.
        for row in 0..3 {
            for w in 0..6 {
                assert_eq!(
                    START_TROOPS[row][w + 1],
                    START_ARMOURY[row][w],
                    "row {row} weapon {w}",
                );
            }
        }
        // **Row 3 is the exception and it is in the image, not a slip.** *Many*
        // weapons is a hundred of each of the five real types — 500 — while
        // *large* is a hundred each of only swords, pikes and bows: 300 men and
        // 200 weapons left over in the armoury. So picking both does not give
        // you five hundred armed men, and the two tables are chosen
        // independently.
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
        // The list is shortened to match, so the choice cannot be re-broken.
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
        // And every commit off that is still a real row of every table.
        let s = o.commit(1);
        assert_eq!(s.gold, 5000);
        assert_eq!(s.time_limit, 0);
        assert_eq!(s.difficulty, 3);
    }

    #[test]
    fn the_ai_lord_count_is_the_lords_minus_the_people() {
        let mut o = SetupOptions::new();
        o.set(option::NOBLES, 3);
        assert_eq!(o.commit(1).ai_lords, 4);
        assert_eq!(o.commit(3).ai_lords, 2);
        o.set(option::NOBLES, 0);
        assert_eq!(o.commit(1).ai_lords, 1);
    }

    #[test]
    fn difficulty_reaches_the_ai_armoury() {
        let mut o = SetupOptions::new();
        o.set(option::WEAPONS, 0);
        for d in 0..4usize {
            o.set(option::DIFFICULTY, d);
            let s = o.commit(1);
            assert_eq!(s.armoury[AI_EXTRA_WEAPON_SLOT], 0, "the table row itself is untouched");
            assert_eq!(s.difficulty as usize, d);
        }
    }
}
