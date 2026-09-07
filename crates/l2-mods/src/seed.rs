//! Deriving the base ruleset from the player's own game files.
//!
//! ## Why this exists at all
//!
//! OpenXcom ships its rulesets. We cannot: a `rules/troops.toml` holding the
//! 3,885 numbers of `TROOPS.ENG` is a transcription of a shipped game file, and
//! `CLAUDE.md` rule 1 says game data does not enter this repository. So the
//! base ruleset is **generated on the player's machine from the copy of the
//! game they already own**, into their own data directory, and only the
//! generator is version-controlled.
//!
//! That turns out to be the better design anyway. The generated file is a
//! plain, commented, human-readable document sitting on disk next to the mods,
//! so the first thing a would-be mod author can do is open the base rules and
//! read them — which is exactly how people learn to mod OpenXcom.
//!
//! ## On decoding
//!
//! `l2-formats` owns decoding, and this module does not duplicate any of it:
//! there is no `.eng` decoder there yet, and what is needed here is not one.
//! `TROOPS*.ENG` is plain text, and the engine's own reader (`FUN_0042AC0C`)
//! is "skip to the first `*`, then take every decimal token and ignore
//! everything else". That is the ten lines below. When `l2_formats::eng`
//! lands, [`parse_troops_eng`] should become a call into it and this module
//! should keep only the rule-generation half.

use std::fmt::Write as _;

/// 35 battles, matching the 35 built-in battles of `BATTLES.ENG`.
pub const ROWS: usize = 35;
/// Very easy, Easy, Normal, Hard, Very hard.
pub const GROUPS: usize = 5;
/// Attacker then defender.
pub const SIDES: usize = 2;
/// Pe Xb Ma Sw Pi Ar Kn Ca To Ra Oi.
pub const COLUMNS: usize = 11;
/// `35 * (1 + 5 * 2 * 11)`.
pub const TOKEN_COUNT: usize = ROWS * (1 + GROUPS * SIDES * COLUMNS);
/// The index of the "Normal" difficulty group — the only one whose numbers the
/// engine keeps.
pub const NORMAL_GROUP: usize = 2;

/// The eleven columns. Names for 0-6 come from the two-letter header and the
/// unit set; 7-10 are the siege columns the engine clamps to 9, and their
/// individual identities are **inferred** from the abbreviations rather than
/// verified. `docs/formats/eng.md` marks the same distinction.
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

/// The difficulty curve the 1996 binary applies in code, lifted into data.
/// `(id, order, scale_percent)`.
pub const DIFFICULTIES: [(&str, i64, i64); GROUPS] = [
    ("very_easy", 0, 116),
    ("easy", 1, 108),
    ("normal", 2, 100),
    ("hard", 3, 92),
    ("very_hard", 4, 84),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedError {
    /// No `*` marker: not a `TROOPS*.ENG` at all.
    NoMarker,
    /// The token count did not close. `docs/formats/eng.md` shows all three
    /// shipped files hitting 3,885 exactly, so anything else is a different
    /// file or a corrupted one.
    TokenCount { found: usize, expected: usize },
    /// A token that does not fit an `i64`, i.e. not really a number.
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

/// The raw table, exactly as the file lays it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTroopTable {
    pub advantage: [i64; ROWS],
    /// `[battle][difficulty group][side][column]`.
    pub counts: Box<[[[[i64; COLUMNS]; SIDES]; GROUPS]; ROWS]>,
}

/// Scan the decimal tokens after the first `*`, exactly as the engine does.
pub fn parse_troops_eng(bytes: &[u8]) -> Result<RawTroopTable, SeedError> {
    let start = bytes.iter().position(|&b| b == b'*').ok_or(SeedError::NoMarker)?;
    let mut tokens: Vec<i64> = Vec::with_capacity(TOKEN_COUNT);
    let mut i = start;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let from = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let text = std::str::from_utf8(&bytes[from..i]).unwrap_or("");
            match text.parse::<i64>() {
                Ok(v) => tokens.push(v),
                Err(_) => return Err(SeedError::BadToken(text.to_string())),
            }
        } else {
            i += 1;
        }
    }
    if tokens.len() != TOKEN_COUNT {
        return Err(SeedError::TokenCount { found: tokens.len(), expected: TOKEN_COUNT });
    }

    let mut table = RawTroopTable {
        advantage: [0; ROWS],
        counts: Box::new([[[[0i64; COLUMNS]; SIDES]; GROUPS]; ROWS]),
    };
    let mut t = tokens.into_iter();
    for row in 0..ROWS {
        // The engine clamps the advantage to 0..10 on read, so the generated
        // rules record what the game actually used, not what the file said.
        table.advantage[row] = t.next().unwrap().clamp(0, 10);
        for g in 0..GROUPS {
            for s in 0..SIDES {
                for c in 0..COLUMNS {
                    table.counts[row][g][s][c] = t.next().unwrap();
                }
            }
        }
    }
    Ok(table)
}

/// The 35 built-in battle names from `BATTLES.ENG`.
///
/// The file is CR/LF text that the engine flattens by turning every byte below
/// `0x20` into a NUL and then reads as `(short, full, description)` triples.
/// Only the short name is wanted here, as the source of a readable rule id.
pub fn battle_names(bytes: &[u8]) -> Vec<String> {
    let fields: Vec<String> = bytes
        .split(|&b| b < 0x20)
        .filter(|f| !f.is_empty())
        .map(|f| String::from_utf8_lossy(f).trim().to_string())
        .filter(|f| !f.is_empty())
        .collect();
    fields.chunks(3).filter(|c| c.len() == 3).map(|c| c[0].clone()).collect()
}

/// A readable, stable rule id: `"Three Bridges"` -> `"three_bridges"`.
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

/// Render the base `rules/troops.toml`.
///
/// `names` may be short of 35 entries — the DOS install has no `BATTLES.ENG`
/// at all — in which case the remaining battles get `battle_NN` ids.
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
        // Ids must be unique even if two battles share a short name.
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

/// Quote a string for the rule syntax.
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
