//! One End Turn of ours from a before-save, read field by field against the
//! original's after-save.

use std::collections::BTreeMap;

use l2_formats::save::Save;
use l2_kingdom::tables::Tables;

use crate::fields::*;

pub struct Row {
    pub pair: String,
    pub path: &'static str,
    pub record: String,
    pub kind: Kind,
    pub was: i64,
    pub theirs: i64,
    pub ours: i64,
}

impl Row {
    pub fn delta(&self) -> i64 {
        self.ours - self.theirs
    }

    pub fn moved(&self) -> bool {
        self.was != self.theirs
    }
}

#[derive(Default)]
pub struct PairReport {
    pub compared: usize,
    pub agree: usize,
    pub moved: usize,
    pub moved_agree: usize,
    pub diverged: Vec<Row>,
    /// Per field path: (records compared, records that agree).
    pub per_field: BTreeMap<&'static str, (usize, usize)>,
}

pub fn clock(save: &Save) -> (i32, i32, i32) {
    let g = save.globals().expect("the globals block");
    (g.turn_count, g.season, g.year)
}

pub fn run(pair: &crate::FixturePair) -> Result<PairReport, String> {
    let before = l2_testkit::fixture_save(pair.before)?;
    let after = l2_testkit::fixture_save(pair.after)?;
    run_saves(pair.label, pair.before, &before, &after)
}

pub fn run_saves(
    label: &str,
    name: &str,
    before: &Save,
    after: &Save,
) -> Result<PairReport, String> {
    let mut game = l2_game::scenario::from_save(before, Tables::DEFAULT)
        .map_err(|e| format!("{name} does not import: {e:?}"))?;
    l2_game::turn::end_turn(&mut game)
        .ok_or_else(|| format!("the turn machine did not come round on {name}"))?;

    let mut r = PairReport::default();
    let mut note =
        |kind: Kind, path: &'static str, record: String, was: i64, theirs: i64, ours: i64| {
            if !kind.compared() {
                return;
            }
            let row = Row {
                pair: label.to_string(),
                path,
                record,
                kind,
                was,
                theirs,
                ours,
            };
            r.compared += 1;
            let agrees = row.ours == row.theirs;
            let f = r.per_field.entry(path).or_default();
            f.0 += 1;
            if agrees {
                r.agree += 1;
                f.1 += 1;
            }
            if row.moved() {
                r.moved += 1;
                if agrees {
                    r.moved_agree += 1;
                }
            }
            if !agrees {
                r.diverged.push(row);
            }
        };

    let was_counties = before.counties().map_err(|e| e.to_string())?;
    let counties = after.counties().map_err(|e| e.to_string())?;
    let county_count = after.globals().map_err(|e| e.to_string())?.county_count as usize;
    for id in 1..=county_count.min(l2_kingdom::county::MAX_COUNTY_ID as usize) {
        for f in COUNTY_FIELDS {
            note(
                f.kind,
                f.path,
                format!("county {id}"),
                (f.theirs)(&was_counties[id]),
                (f.theirs)(&counties[id]),
                (f.ours)(&game.kingdom.counties[id]),
            );
        }
    }

    let was_realms = before.realms().map_err(|e| e.to_string())?;
    let realms = after.realms().map_err(|e| e.to_string())?;
    // **Realms 3, 4 and 5 are out of play in all five pairs**, so nothing here
    // ever farmed or compared them: measured off this test's own printout,
    // which names realms 1 and 2 and no other. A ledger row claiming AI realm 3
    // never plants where the original does cannot be seen from here, and the
    // behaviour behind it is not a defect: realm 3's lord is 2, whose style
    // byte is 1 — `Ai_FarmStyleGrazing` (`0x004A42E3`), which clears grain in
    // every season. `county.fields_grain` is compared and agrees everywhere.
    for (id, theirs) in realms.iter().enumerate() {
        if !theirs.in_play() {
            continue;
        }
        for f in REALM_FIELDS {
            note(
                f.kind,
                f.path,
                format!("realm {id}"),
                (f.theirs)(&was_realms[id]),
                (f.theirs)(theirs),
                (f.ours)(&game.kingdom.realms[id]),
            );
        }
        for (other, them) in realms.iter().enumerate() {
            if other == id || !them.in_play() {
                continue;
            }
            for f in PAIR_FIELDS {
                note(
                    f.kind,
                    f.path,
                    format!("realm {id}->{other}"),
                    (f.theirs)(&was_realms[id].pairs[other]),
                    (f.theirs)(&theirs.pairs[other]),
                    (f.ours)(&game.kingdom.realms[id].pairs[other]),
                );
            }
        }
    }

    let was_globals = before.globals().map_err(|e| e.to_string())?;
    let globals = after.globals().map_err(|e| e.to_string())?;
    for f in GLOBAL_FIELDS {
        note(
            f.kind,
            f.path,
            "-".to_string(),
            (f.theirs)(&was_globals),
            (f.theirs)(&globals),
            (f.ours)(&game),
        );
    }

    Ok(r)
}
