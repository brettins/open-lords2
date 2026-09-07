//! Worked example: the skirmish army table as rules rather than as code.
//!
//! This is the smallest piece of the game that is genuinely *rules* — 35
//! battles x 11 troop columns x 2 sides, plus a per-battle defensive
//! advantage — and it is fully documented in `docs/formats/eng.md`, so it can
//! be used to demonstrate the whole path without guessing at anything.
//!
//! Three things about it are worth noticing, because they are the argument for
//! doing this at all:
//!
//! * **The difficulty curve was hard-coded.** `Lords2.exe` reads only the
//!   "Normal" rows and then synthesises the other four difficulties as
//!   `Normal +16% / +8% / -8% / -16%`, applying the scaling to troop types
//!   0-6 and leaving the four siege columns alone. Those five percentages
//!   exist only as instructions in a 1996 binary. Here they are five lines of
//!   a rule file, and a mod that wants a harsher curve edits them.
//! * **The clamps were a parser detail.** The engine clamps the siege columns
//!   to 9 and the advantage to 0..10 because a text file it could not validate
//!   might contain anything. We validate instead, so a mod with 40 catapults
//!   gets an error naming its file and line rather than a silent 9.
//! * **The columns had no names.** The file has an eleven-column header of
//!   two-letter abbreviations and the code has indices. Naming the columns is
//!   what lets a mod write `crossbows = 40` and change nothing else.

use crate::ruleset::{RuleError, Ruleset};

/// Eleven columns, in the order the file and the `.skr` army record use.
pub const TROOP_COLUMNS: usize = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Attacker,
    Defender,
}

impl Side {
    pub fn key(self) -> &'static str {
        match self {
            Side::Attacker => "attacker",
            Side::Defender => "defender",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TroopType {
    pub id: String,
    /// Position in the eleven-column row.
    pub column: usize,
    /// The two-letter header abbreviation, kept so a generated file can be
    /// read next to the original.
    pub abbrev: String,
    pub name: String,
    /// Siege equipment: capped at 9 and exempt from difficulty scaling.
    pub siege_engine: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difficulty {
    pub id: String,
    /// Presentation order, easiest first.
    pub order: i64,
    /// Percentage applied to non-siege columns. 100 is unscaled.
    pub scale_percent: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Battle {
    pub id: String,
    pub name: String,
    /// 0..=10.
    pub defensive_advantage: i64,
    /// The "Normal" row: the only one the original file's data is used for.
    pub attacker: [i64; TROOP_COLUMNS],
    pub defender: [i64; TROOP_COLUMNS],
}

/// The typed view of the merged ruleset.
#[derive(Debug, Clone, Default)]
pub struct TroopRules {
    pub troops: Vec<TroopType>,
    pub difficulties: Vec<Difficulty>,
    pub battles: Vec<Battle>,
}

impl TroopRules {
    /// Read the `troop`, `difficulty` and `battle` tables out of a ruleset.
    ///
    /// Every failure carries the document and line that caused it, because the
    /// ruleset carried that provenance through the merge. That is the whole
    /// reason for the value tree.
    pub fn from_ruleset(rs: &Ruleset) -> Result<TroopRules, RuleError> {
        let mut troops = Vec::new();
        for id in rs.keys("troop").into_iter().map(str::to_string).collect::<Vec<_>>() {
            let base = format!("troop.{id}");
            let column =
                rs.integer_in(&format!("{base}.column"), 0, TROOP_COLUMNS as i64 - 1)? as usize;
            troops.push(TroopType {
                abbrev: rs.string_or(&format!("{base}.abbrev"), "").to_string(),
                name: rs.string_or(&format!("{base}.name"), &id).to_string(),
                siege_engine: rs.boolean_or(&format!("{base}.siege_engine"), false)?,
                id,
                column,
            });
        }
        troops.sort_by_key(|t| t.column);

        let mut difficulties = Vec::new();
        for id in rs.keys("difficulty").into_iter().map(str::to_string).collect::<Vec<_>>() {
            let base = format!("difficulty.{id}");
            difficulties.push(Difficulty {
                order: rs.integer_or(&format!("{base}.order"), 0)?,
                // A wider range than the original's five values, but not
                // unbounded: 0 means "no troops at all", and beyond 1000 the
                // u16 the engine stores overflows.
                scale_percent: rs.integer_in(&format!("{base}.scale_percent"), 0, 1000)?,
                id,
            });
        }
        difficulties.sort_by(|a, b| a.order.cmp(&b.order).then(a.id.cmp(&b.id)));

        let siege: Vec<bool> = {
            let mut v = vec![false; TROOP_COLUMNS];
            for t in &troops {
                v[t.column] = t.siege_engine;
            }
            v
        };

        let mut battles = Vec::new();
        for id in rs.keys("battle").into_iter().map(str::to_string).collect::<Vec<_>>() {
            let base = format!("battle.{id}");
            let attacker = read_row(rs, &base, Side::Attacker, &troops, &siege)?;
            let defender = read_row(rs, &base, Side::Defender, &troops, &siege)?;
            battles.push(Battle {
                name: rs.string_or(&format!("{base}.name"), &id).to_string(),
                defensive_advantage: rs
                    .integer_in(&format!("{base}.defensive_advantage"), 0, 10)?,
                id,
                attacker,
                defender,
            });
        }
        battles.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(TroopRules { troops, difficulties, battles })
    }

    pub fn battle(&self, id: &str) -> Option<&Battle> {
        self.battles.iter().find(|b| b.id == id)
    }

    pub fn difficulty(&self, id: &str) -> Option<&Difficulty> {
        self.difficulties.iter().find(|d| d.id == id)
    }

    pub fn troop(&self, id: &str) -> Option<&TroopType> {
        self.troops.iter().find(|t| t.id == id)
    }

    /// The army a side fields, with the difficulty scaling applied.
    ///
    /// Reproduces the original's arithmetic exactly: `x * percent / 100` with
    /// truncating integer division (`FUN_00404D6B`), applied only to the
    /// non-siege columns.
    pub fn army(&self, battle: &Battle, difficulty: &str, side: Side) -> [i64; TROOP_COLUMNS] {
        let row = match side {
            Side::Attacker => battle.attacker,
            Side::Defender => battle.defender,
        };
        let percent = self.difficulty(difficulty).map(|d| d.scale_percent).unwrap_or(100);
        let mut out = row;
        for t in &self.troops {
            if !t.siege_engine {
                out[t.column] = row[t.column] * percent / 100;
            }
        }
        out
    }

    /// Look a column up by troop id, for callers that think in names.
    pub fn count(&self, army: &[i64; TROOP_COLUMNS], troop_id: &str) -> Option<i64> {
        self.troop(troop_id).map(|t| army[t.column])
    }
}

fn read_row(
    rs: &Ruleset,
    base: &str,
    side: Side,
    troops: &[TroopType],
    siege: &[bool],
) -> Result<[i64; TROOP_COLUMNS], RuleError> {
    let mut row = [0i64; TROOP_COLUMNS];
    for t in troops {
        let path = format!("{base}.{}.{}", side.key(), t.id);
        // Absent means zero: that is what makes a mod able to write one line.
        let Some(v) = rs.get(&path) else { continue };
        let _ = v;
        row[t.column] = if siege[t.column] {
            // The original clamps siege columns to 9. We refuse instead, so a
            // mod finds out at load rather than wondering where its 40
            // catapults went.
            rs.integer_in(&path, 0, 9)?
        } else {
            rs.integer_in(&path, 0, u16::MAX as i64)?
        };
    }
    Ok(row)
}
