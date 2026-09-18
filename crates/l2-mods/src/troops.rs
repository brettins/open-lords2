
use crate::ruleset::{RuleError, Ruleset};

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
    pub column: usize,
    pub abbrev: String,
    pub name: String,
    pub siege_engine: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difficulty {
    pub id: String,
    pub order: i64,
    pub scale_percent: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Battle {
    pub id: String,
    pub name: String,
    pub defensive_advantage: i64,
    pub attacker: [i64; TROOP_COLUMNS],
    pub defender: [i64; TROOP_COLUMNS],
}

#[derive(Debug, Clone, Default)]
pub struct TroopRules {
    pub troops: Vec<TroopType>,
    pub difficulties: Vec<Difficulty>,
    pub battles: Vec<Battle>,
}

impl TroopRules {
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
        let Some(v) = rs.get(&path) else { continue };
        let _ = v;
        row[t.column] = if siege[t.column] {
            rs.integer_in(&path, 0, 9)?
        } else {
            rs.integer_in(&path, 0, u16::MAX as i64)?
        };
    }
    Ok(row)
}
