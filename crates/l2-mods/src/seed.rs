//! `TROOPS*.ENG` is plain text, and the engine's own reader (`FUN_0042AC0C`)
//! is "skip to the first `*`, then take every decimal token and ignore
//! everything else". That is the ten lines below. When `l2_formats::eng`
//! lands, [`parse_troops_eng`] should become a call into it and this module
//! should keep only the rule-generation half.

use std::fmt::Write as _;

pub const ROWS: usize = 35;
pub const GROUPS: usize = 5;
pub const SIDES: usize = 2;
pub const COLUMNS: usize = 11;
pub const TOKEN_COUNT: usize = ROWS * (1 + GROUPS * SIDES * COLUMNS);
pub const NORMAL_GROUP: usize = 2;

pub const TROOP_COLUMNS: [(&str, &str, &str, bool); COLUMNS] = [
    ("peasants", "Pe", "Peasants", false),
    ("crossbows", "Xb", "Crossbowmen", false),
    ("maces", "Ma", "Macemen", false),
    ("swords", "Sw", "Swordsmen", false),
    ("pikes", "Pi", "Pikemen", false),
    ("archers", "Ar", "Archers", false),
    ("knights", "Kn", "Knights", false),
    ("catapults", "Ca", "Catapults", true),
    ("siege_towers", "To", "Siege towers", true),
    ("rams", "Ra", "Battering rams", true),
    ("oil", "Oi", "Boiling oil", true),
];

pub const DIFFICULTIES: [(&str, i64, i64); GROUPS] = [
    ("very_easy", 0, 116),
    ("easy", 1, 108),
    ("normal", 2, 100),
    ("hard", 3, 92),
    ("very_hard", 4, 84),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedError {
    NoMarker,
    TokenCount { found: usize, expected: usize },
    BadToken(String),
}

impl std::fmt::Display for SeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeedError::NoMarker => write!(f, "no '*' marker: not a TROOPS*.ENG file"),
            SeedError::TokenCount { found, expected } => {
                write!(f, "found {found} numbers, expected {expected}")
            }
            SeedError::BadToken(t) => write!(f, "'{t}' is not a number"),
        }
    }
}

impl std::error::Error for SeedError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTroopTable {
    pub advantage: [i64; ROWS],
    pub counts: Box<[[[[i64; COLUMNS]; SIDES]; GROUPS]; ROWS]>,
}

/// `l2_formats::troops::parse_raw` is `Troops_Load`'s own token scan; this is
/// the widening to `i64` the rule generator wants.
pub fn parse_troops_eng(bytes: &[u8]) -> Result<RawTroopTable, SeedError> {
    let raw = l2_formats::troops::parse_raw(bytes).map_err(|e| match e {
        l2_formats::Error::TroopsNoMarker => SeedError::NoMarker,
        l2_formats::Error::TroopsTokenCount { found, expected } => {
            SeedError::TokenCount { found, expected }
        }
        l2_formats::Error::TroopsBadToken(t) => SeedError::BadToken(t),
        other => SeedError::BadToken(other.to_string()),
    })?;
    let mut table = RawTroopTable {
        advantage: [0; ROWS],
        counts: Box::new([[[[0i64; COLUMNS]; SIDES]; GROUPS]; ROWS]),
    };
    for row in 0..ROWS {
        table.advantage[row] = raw.advantage[row] as i64;
        for g in 0..GROUPS {
            for s in 0..SIDES {
                for c in 0..COLUMNS {
                    table.counts[row][g][s][c] = raw.counts[row][g][s][c] as i64;
                }
            }
        }
    }
    Ok(table)
}

pub fn battle_names(bytes: &[u8]) -> Vec<String> {
    l2_formats::Battles::parse(bytes).names()
}

pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_sep = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_sep = false;
        } else if !last_sep {
            out.push('_');
            last_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("unnamed");
    }
    out
}

pub fn to_rules_toml(table: &RawTroopTable, names: &[String], source_file: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Skirmish army rules, generated from {source_file}.\n\
         #\n\
         # Generated, not authored: regenerating overwrites it. To change a\n\
         # number, put your change in a mod instead - a mod file with just the\n\
         # lines you want different is merged over this one.\n\
         #\n\
         # Only the game's \"Normal\" numbers are stored. The original engine\n\
         # read the other four difficulty rows and then threw them away,\n\
         # re-deriving them from Normal by a fixed percentage; those\n\
         # percentages are [difficulty] below."
    );

    out.push_str("\n# --- troop columns ---------------------------------------------------\n");
    for (col, (id, abbrev, name, siege)) in TROOP_COLUMNS.iter().enumerate() {
        let _ = writeln!(
            out,
            "\n[troop.{id}]\ncolumn = {col}\nabbrev = {}\nname = {}\nsiege_engine = {siege}",
            quote(abbrev),
            quote(name)
        );
    }

    out.push_str(
        "\n# --- difficulty ------------------------------------------------------\n\
         # scale_percent multiplies every non-siege column. Siege equipment is\n\
         # not scaled, and is capped at 9 per battle.\n",
    );
    for (id, order, percent) in DIFFICULTIES {
        let _ = writeln!(out, "\n[difficulty.{id}]\norder = {order}\nscale_percent = {percent}");
    }

    out.push_str("\n# --- battles ---------------------------------------------------------\n");
    let mut used: Vec<String> = Vec::new();
    for row in 0..ROWS {
        let name = names.get(row).cloned().unwrap_or_else(|| format!("Battle {}", row + 1));
        let mut id = slug(&name);
        if used.contains(&id) {
            id = format!("{id}_{row}");
        }
        used.push(id.clone());

        let _ = writeln!(
            out,
            "\n[battle.{id}]\nindex = {row}\nname = {}\ndefensive_advantage = {}",
            quote(&name),
            table.advantage[row]
        );
        for (side_index, side) in ["attacker", "defender"].into_iter().enumerate() {
            let _ = writeln!(out, "\n[battle.{id}.{side}]");
            for (col, (troop_id, _, _, _)) in TROOP_COLUMNS.iter().enumerate() {
                let n = table.counts[row][NORMAL_GROUP][side_index][col];
                let _ = writeln!(out, "{troop_id} = {n}");
            }
        }
    }
    out
}

pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
