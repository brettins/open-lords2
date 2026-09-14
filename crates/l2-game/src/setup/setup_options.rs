#![allow(unused_imports)]
use super::*;
use super::tables::*;
use super::settings::*;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::unit::TROOP_TYPES;
use crate::game::Game;

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
    /// the map seats fewer than five —
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

    /// How many lords are in the game — the *Nobles* label's own numbers
    /// through *five*. `L2.eng` group 103 index 4 is *"one"* and nothing
    /// reaches it, which is the string the twelve runs leave over.
    pub fn lords(&self) -> usize {
        self.get(option::NOBLES) + 2
    }

    /// `Setup_CommitOptions` (`0x00499DC3`), and `human_players` is
    /// `DAT_00553F98`, which is 1 in a single-player game.
    pub fn commit(&self, human_players: usize, quirks: l2_kingdom::Quirks) -> Settings {
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
            quirks,
            gold: STARTING_GOLD[self.get(option::CROWNS)],
            castle_type: castle as u8,
            armoury: START_ARMOURY[self.get(option::WEAPONS)],
            garrison: START_TROOPS[self.get(option::ARMY_SIZE)],
            county: COUNTY_STATUS[self.get(option::COUNTY_STATUS)],
            // `DAT_0053F268 = (nobles + 2) - humanPlayers`. It is not clamped
            // in the original either; `Realms_AssignLords` walks five realms and
            // stops handing out lords when it has handed out this many,
            // negative simply means.
            ai_lords: self.lords() as i32 - human_players as i32,
        }
    }
}

// --------------------------------------------------------------- the settings

