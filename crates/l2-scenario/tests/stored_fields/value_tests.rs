#![allow(unused_imports)]
use super::*;
use super::layout_tests::*;
use super::oracle_tests::*;
use super::accessors_part::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use l2_formats::save::{Save, COUNTY_BASE, COUNTY_STRIDE, REALM_BASE, REALM_STRIDE};
use l2_kingdom::Kingdom;
use l2_scenario::Scenario;

/// One element of one row, as the file holds it.
fn decode(save: &Save, va: u32, ty: Ty) -> i64 {
    let value = match ty {
        Ty::U8 => save.u8_at(va).map(i64::from),
        Ty::I8 => save.i8_at(va).map(i64::from),
        Ty::U16 => save.u16_at(va).map(i64::from),
        Ty::I16 => save.i16_at(va).map(i64::from),
        Ty::I32 => save.i32_at(va).map(i64::from),
        Ty::Bool => save.u8_at(va).map(|b| i64::from(b != 0)),
    };
    value.unwrap_or_else(|e| panic!("{va:#X}: {e}"))
}

/// **Every claimed row, in every county and realm of every save, reaches the
/// loaded kingdom holding the original's own value.**
///
/// This is the half that cannot be typed into agreement, and it is the one that
/// would have caught all three instances before a player did: C142's `+0xC0`,
/// the 56 industry forecasts and the three farm rows were each a row that
/// would have been `imported` in intent and `0` in the kingdom.
#[test]
fn every_imported_and_derived_field_reaches_the_kingdom_holding_the_files_bytes() {
    let saves = l2_testkit::saves!();
    let rows = rows();
    let get = accessors();

    let claimed: BTreeSet<&str> =
        rows.iter().filter(|r| r.claims()).map(|r| r.id.as_str()).collect();
    let checked: BTreeSet<&str> = get.iter().map(|(id, _)| *id).collect();
    assert_eq!(get.len(), checked.len(), "an accessor is listed twice");
    let unchecked: Vec<&&str> = claimed.difference(&checked).collect();
    let unclaimed: Vec<&&str> = checked.difference(&claimed).collect();
    assert!(
        unchecked.is_empty(),
        "docs/stored-fields.json says the load carries these, and nothing here reads the kingdom \
         field that would prove it — add an accessor:\n  {unchecked:?}"
    );
    assert!(
        unclaimed.is_empty(),
        "this test reads these kingdom fields, and docs/stored-fields.json does not claim them as \
         imported or derived:\n  {unclaimed:?}"
    );

    let mut wrong: Vec<String> = Vec::new();
    let mut wrong_total = 0usize;
    let mut compared = 0usize;
    let mut nonzero: BTreeSet<&str> = BTreeSet::new();
    for f in &saves {
        let save = &f.save;
        let scenario =
            Scenario::from_save(save).unwrap_or_else(|e| panic!("{}: the import refused: {e}", f.label()));
        let k = scenario.kingdom(1);
        for r in rows.iter().filter(|r| r.claims()) {
            let read = get.iter().find(|(id, _)| *id == r.id).expect("checked above").1;
            let (base, stride, ids): (u32, u32, Vec<usize>) = match r.record.as_str() {
                "County" => (COUNTY_BASE, COUNTY_STRIDE as u32, scenario.county_ids().collect()),
                _ => (REALM_BASE, REALM_STRIDE as u32, (1..l2_kingdom::MAX_REALMS).collect()),
            };
            for id in ids {
                for e in 0..r.count as usize {
                    let va = base + id as u32 * stride + r.off + e as u32 * r.stride;
                    let file = decode(save, va, r.ty);
                    let ours = read(&k, id, e);
                    compared += 1;
                    if file != 0 {
                        nonzero.insert(r.id.as_str());
                    }
                    if file != ours {
                        wrong_total += 1;
                        if wrong.len() < 40 {
                            wrong.push(format!(
                                "{} {}  {} {id} element {e}  {}: the file holds {file}, the loaded \
                                 kingdom {ours}",
                                r.id,
                                r.name,
                                r.record,
                                f.label()
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{wrong_total} value(s) a loaded game does not start with (first {}):\n  {}\n\
         A row marked imported or derived claims a loaded game starts where the original left \
         it. Either the importer drops the field — carry it in crates/l2-scenario — or the row \
         is wrong about the offset, and docs/stored-fields.json should say what it is instead.",
        wrong.len(),
        wrong.join("\n  ")
    );

    // **What a green run did not measure.** A claimed row that is zero in every
    // save was compared with zero, and a wrong offset landing on another zero
    // passes. Named, so the next save with a castle under construction or a
    // crop in the ground knows what it can settle.
    let silent: Vec<&str> = claimed.iter().filter(|id| !nonzero.contains(**id)).copied().collect();
    eprintln!(
        "{compared} values compared over {} saves; {} of {} claimed rows hold zero in every save, \
         so this measured their offsets only against zero: {}",
        saves.len(),
        silent.len(),
        claimed.len(),
        silent.join(", ")
    );
}

