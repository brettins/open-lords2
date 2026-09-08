//! `unit.*` — the battle-simulation combat constants, as rules.
//!
//! This is the half of the platform `docs/decisions.md` C11 is really about.
//! The skirmish army table in [`crate::troops`] came out of `TROOPS*.ENG`, a
//! file the original at least *read*; these numbers were never in a file at
//! all. `docs/battle.md` §6.1 read them out of the instruction stream of
//! `Lords2.exe`, and the only way to change them in the 1996 game is to patch
//! the binary. Here they are eleven tables in a document.
//!
//! # Why `unit.*` and not `troop.*`
//!
//! The two namespaces describe the same eleven things and are deliberately
//! kept apart, because they have different authorities and different
//! lifetimes:
//!
//! * `troop.<id>` says where a type sits in the eleven columns of
//!   `TROOPS*.ENG` and what the skirmish battles start with. It is **generated
//!   on the player's machine** from their own copy of the game, and it does
//!   not exist at all on an install that never shipped those files.
//! * `unit.<id>` says what a soldier does when it swings. It is **ours**,
//!   shipped with the engine, and present on every install.
//!
//! Merging them into one table would mean a file that is half generated and
//! half authored, and a mod author could not tell by looking which half
//! regenerating would overwrite. The ids are the same in both, and
//! [`crate::core`] has a test that they stay the same.

use crate::ruleset::{RuleError, Ruleset};
use l2_sim::{Troop, TroopStats, TroopTable, ALL_TROOPS};

/// The rule id of each troop type, in [`ALL_TROOPS`] order.
///
/// These are the ids `TROOPS*.ENG`'s two-letter column header gives, so
/// `troop.archers` and `unit.archers` are the same soldier. `crossbows` rather
/// than `crossbowmen` because the file's header says `Xb`, and a mod author
/// typing one of these has already met the other.
pub const UNIT_IDS: [&str; ALL_TROOPS.len()] = [
    "peasants",
    "crossbows",
    "maces",
    "swords",
    "pikes",
    "archers",
    "knights",
    "catapults",
    "siege_towers",
    "rams",
    "oil",
];

/// The id of one troop type.
pub fn unit_id(troop: Troop) -> &'static str {
    UNIT_IDS[troop.index()]
}

/// The troop type an id names.
pub fn troop_for_id(id: &str) -> Option<Troop> {
    UNIT_IDS.iter().position(|&u| u == id).map(|i| ALL_TROOPS[i])
}

/// The largest value the simulation's `u16` fields can hold.
const MAX_STAT: i64 = u16::MAX as i64;

/// Build a [`TroopTable`] out of a merged ruleset.
///
/// Every type must be present: unlike the army table, where an absent troop
/// legitimately means "none of those", an absent `recovery` would mean a
/// figure that can be struck every tick. A missing rule here is a mistake, and
/// the error names the rule rather than the file, because after a merge the
/// rule is the thing that is missing and no single file is to blame.
///
/// Ranges are checked and refused rather than clamped. `recovery = 0` is the
/// one that matters: it is a divisor-shaped value in the melee loop and a
/// figure with no recovery interval is struck on every tick by every
/// neighbour, which reads as a hang rather than as a rebalance.
pub fn troop_table(rs: &Ruleset) -> Result<TroopTable, RuleError> {
    let mut table = TroopTable::DEFAULT;
    for troop in ALL_TROOPS {
        let id = unit_id(troop);
        let base = format!("unit.{id}");
        let index = rs.integer_in(&format!("{base}.index"), 0, ALL_TROOPS.len() as i64 - 1)?;
        if index as usize != troop.index() {
            return Err(RuleError::Range {
                path: format!("{base}.index"),
                message: format!(
                    "'{id}' is troop slot {}, so its index must be {} and not {index}",
                    troop.index(),
                    troop.index()
                ),
                origin: rs
                    .origin(&format!("{base}.index"))
                    .cloned()
                    .unwrap_or_else(|| crate::value::Origin::synthetic("unit index")),
            });
        }

        let attack = rs.integer_array(&format!("{base}.melee_attack"), 4)?;
        let mut melee_attack = [0u16; 4];
        for (band, &value) in attack.iter().enumerate() {
            if !(0..=MAX_STAT).contains(&value) {
                return Err(range_err(rs, &format!("{base}.melee_attack"), format!(
                    "band {band} is {value}, outside 0..={MAX_STAT}"
                )));
            }
            melee_attack[band] = value as u16;
        }
        // Best band first. The original's bands are a degradation ladder, and
        // a rising one would make a weakened figure hit harder.
        for band in 1..4 {
            if melee_attack[band] > melee_attack[band - 1] {
                return Err(range_err(
                    rs,
                    &format!("{base}.melee_attack"),
                    format!(
                        "band {band} ({}) is stronger than band {} ({}); the bands run best first",
                        melee_attack[band],
                        band - 1,
                        melee_attack[band - 1]
                    ),
                ));
            }
        }

        let stats = TroopStats {
            melee_attack,
            recovery: rs.integer_in(&format!("{base}.recovery"), 1, MAX_STAT)? as u16,
            heavy_blow: rs.integer_in(&format!("{base}.heavy_blow"), 0, MAX_STAT)? as u16,
            armour: rs.integer_in(&format!("{base}.armour"), 0, MAX_STAT)? as u16,
            exchange: rs.integer_in(&format!("{base}.exchange"), 0, MAX_STAT)? as u16,
        };
        table.stats[troop.index()] = stats;
        // Zero would be an infinite kill loop in `take_hits`.
        table.hits_per_casualty[troop.index()] =
            rs.integer_in(&format!("{base}.hits_per_casualty"), 1, MAX_STAT)? as u16;
    }
    Ok(table)
}

fn range_err(rs: &Ruleset, path: &str, message: String) -> RuleError {
    RuleError::Range {
        path: path.to_string(),
        message,
        origin: rs
            .origin(path)
            .cloned()
            .unwrap_or_else(|| crate::value::Origin::synthetic("unit rules")),
    }
}

/// Render a [`TroopTable`] as the document [`troop_table`] reads back.
///
/// This is how `rulesets/core/rules/units.toml` is produced, and a test
/// asserts the shipped file is exactly this text — so the document and
/// `l2_sim::TroopTable::DEFAULT` cannot drift apart in either direction.
pub fn render_toml(table: &TroopTable) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    out.push_str(
        "# Battle combat constants: what each of the eleven troop types does when\n\
         # it fights. Read by l2-mods into an l2_sim::TroopTable.\n\
         #\n\
         # GENERATED, NOT AUTHORED. This file is rendered from\n\
         # l2_sim::TroopTable::DEFAULT and a test asserts the two agree, so\n\
         # editing it here will fail the build. To change a number for a game,\n\
         # put your change in a mod: a file with just the lines you want\n\
         # different is merged over this one.\n\
         #\n\
         #   [unit.archers]\n\
         #   armour = 4\n\
         #\n\
         # is a complete, valid mod, and leaves the other ten types and the\n\
         # archers' other five numbers exactly as they were.\n\
         #\n\
         # Where these numbers come from: docs/battle.md 6.1, read out of the\n\
         # original's per-type tick handlers and its melee attack table. In\n\
         # Lords2.exe they are instructions, not data - see docs/decisions.md\n\
         # C11. This file is the first time they have been editable.\n\
         #\n\
         # Fields:\n\
         #   index             position in the eleven-slot order; fixed, and\n\
         #                     checked, because it is the array index the\n\
         #                     simulation uses\n\
         #   melee_attack      damage per blow by strength band, best band\n\
         #                     first; four numbers, and an array replaces\n\
         #                     whole, so restate all four\n\
         #   recovery          ticks between blows suffered. THIS IS MELEE\n\
         #                     DEFENCE - there is no separate defence value.\n\
         #                     Must be at least 1\n\
         #   heavy_blow        the once-per-exchange blow, and large\n\
         #   armour            flat subtraction, missiles only; never read in\n\
         #                     a melee exchange\n\
         #   exchange          blows before attacker and defender swap roles\n\
         #   hits_per_casualty damage absorbed before one man dies. Must be at\n\
         #                     least 1\n\
         #\n\
         # Not here, on purpose: which types are siege engines. That decides\n\
         # whether the melee code runs at all rather than how hard it hits, so\n\
         # it is structure rather than balance and stays in the engine.\n",
    );
    for troop in ALL_TROOPS {
        let id = unit_id(troop);
        let s = table.stats(troop);
        let _ = write!(
            out,
            "\n[unit.{id}]\n\
             index = {}\n\
             melee_attack = [{}, {}, {}, {}]\n\
             recovery = {}\n\
             heavy_blow = {}\n\
             armour = {}\n\
             exchange = {}\n\
             hits_per_casualty = {}\n",
            troop.index(),
            s.melee_attack[0],
            s.melee_attack[1],
            s.melee_attack[2],
            s.melee_attack[3],
            s.recovery,
            s.heavy_blow,
            s.armour,
            s.exchange,
            table.hits_per_casualty(troop),
        );
    }
    out
}
