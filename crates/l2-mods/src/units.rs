//! This is the half of the platform `docs/decisions.md` C11 is really about.

use crate::ruleset::{RuleError, Ruleset};
use l2_sim::{Troop, TroopStats, TroopTable, ALL_TROOPS};

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

pub fn unit_id(troop: Troop) -> &'static str {
    UNIT_IDS[troop.index()]
}

pub fn troop_for_id(id: &str) -> Option<Troop> {
    UNIT_IDS.iter().position(|&u| u == id).map(|i| ALL_TROOPS[i])
}

const MAX_STAT: i64 = u16::MAX as i64;

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
         #   heavy_blow        landed once per figure in the whole battle, and\n\
         #                     large. Maceman 300 is the highest\n\
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
