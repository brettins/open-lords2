#![allow(unused_imports)]
use super::*;
use super::oracle_tests::*;
use super::accessors_part::*;
use super::value_tests::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use l2_formats::save::{Save, COUNTY_BASE, COUNTY_STRIDE, REALM_BASE, REALM_STRIDE};
use l2_kingdom::Kingdom;
use l2_scenario::Scenario;

/// **Every row decides.**
#[test]
fn every_stored_field_is_imported_derived_or_excluded_with_a_reason() {
    let rows = rows();
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    for r in &rows {
        if !ids.insert(r.id.clone()) {
            problems.push(format!("{} appears twice", r.id));
        }
        if !STATUSES.contains(&r.status.as_str()) {
            problems.push(format!(
                "{} {} has status `{}`. A field the original stores is imported, derived, or \
                 excluded with a one-line `why` — there is no fourth answer, because a field \
                 nobody decided about is exactly how C142 happened",
                r.id, r.name, r.status
            ));
        }
        if r.status != "imported" && r.why.trim().is_empty() {
            problems.push(format!("{} {} is `{}` and does not say why", r.id, r.name, r.status));
        }
        if r.status == "imported" && !r.why.is_empty() {
            problems.push(format!(
                "{} {} is imported and carries a `why`; an imported row's evidence is the value \
                 check in this file, not prose",
                r.id, r.name
            ));
        }
        match RECORDS.iter().find(|(name, _)| *name == r.record) {
            None => problems.push(format!("{} names record {}, which is not County or Realm", r.id, r.record)),
            Some((_, size)) if r.off + r.span() > *size => problems.push(format!(
                "{} {} runs to +{:#X}, past the end of a {:#X}-byte {}",
                r.id,
                r.name,
                r.off + r.span(),
                size,
                r.record
            )),
            Some(_) => {}
        }
        if r.stride < r.ty.width() {
            problems.push(format!("{} has a stride narrower than its type", r.id));
        }
    }
    // In record order, then offset order, and no two rows share a byte: a
    // row that overlaps another is two claims about one field.
    let mut record_order: Vec<&str> = rows.iter().map(|r| r.record.as_str()).collect();
    record_order.dedup();
    if record_order != ["County", "Realm"] {
        problems.push(format!("rows must be every County then every Realm, and run {record_order:?}"));
    }
    for w in rows.windows(2) {
        if w[0].record == w[1].record && w[1].off <= w[0].off {
            problems.push(format!("{} follows {} — keep the rows in offset order", w[1].id, w[0].id));
        }
    }
    // **By byte, not by span.** The sub-fields of a nested record's array
    // interleave — `labour[].workers`, `.wanted` and `.useful` each cover
    // every twelfth byte across the same hundred — so comparing where one row
    // ends with where the next begins called every one of them an overlap.
    let mut owner: BTreeMap<(String, u32), String> = BTreeMap::new();
    for r in &rows {
        for e in 0..r.count.max(1) {
            for b in 0..r.ty.width() {
                let byte = r.off + e * r.stride + b;
                if let Some(first) = owner.insert((r.record.clone(), byte), r.id.clone()) {
                    problems.push(format!("{} and {} both claim {} +{byte:#05X}", first, r.id, r.record));
                }
            }
        }
    }
    assert!(problems.is_empty(), "docs/stored-fields.json:\n  {}", problems.join("\n  "));

    let n = |s: &str| rows.iter().filter(|r| r.status == s).count();
    eprintln!(
        "stored fields: {} rows — {} imported, {} derived, {} excluded",
        rows.len(),
        n("imported"),
        n("derived"),
        n("excluded")
    );
}

/// One struct of `docs/records.json`: its size and `(offset, type, name)`.
struct Layout {
    size: u32,
    fields: Vec<(u32, String, String)>,
}

/// `docs/records.json`'s structs, by name — scanned by indentation, for the
/// reason [`rows`] gives. The file is written by `JSON.stringify(_, null, 2)`,
/// so a struct's own keys sit six spaces in and a field's ten.
fn layouts() -> BTreeMap<String, Layout> {
    let text = std::fs::read_to_string(repo_root().join("docs/records.json")).expect("docs/records.json");
    let mut out: BTreeMap<String, Layout> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        match indent {
            6 => {
                if let Some(name) = string_field(t, "name") {
                    out.insert(name.clone(), Layout { size: 0, fields: Vec::new() });
                    current = Some(name);
                } else if let (Some(size), Some(c)) = (number_field(t, "size"), &current) {
                    out.get_mut(c).unwrap().size = size;
                }
            }
            10 => {
                let Some(c) = &current else { continue };
                let layout = out.get_mut(c).unwrap();
                if let Some(off) = string_field(t, "off") {
                    let off = u32::from_str_radix(off.trim_start_matches("0x"), 16).expect("an offset");
                    layout.fields.push((off, String::new(), String::new()));
                } else if let (Some(ty), Some(last)) = (string_field(t, "type"), layout.fields.last_mut()) {
                    last.1 = ty;
                } else if let (Some(name), Some(last)) = (string_field(t, "name"), layout.fields.last_mut()) {
                    last.2 = name;
                }
            }
            _ => {}
        }
    }
    for s in ["County", "Realm", "Industry", "LabourSlot", "DiploPair"] {
        assert!(
            out.get(s).is_some_and(|l| l.size > 0 && !l.fields.is_empty()),
            "docs/records.json: the scanner found no {s} — the file's shape changed"
        );
    }
    out
}

/// `"i32[8]"` → `("i32", 8)`.
fn split_type(t: &str) -> (&str, u32) {
    match t.split_once('[') {
        Some((elem, n)) => (elem, n.trim_end_matches(']').parse().expect("an array length")),
        None => (t, 1),
    }
}

fn scalar_width(t: &str) -> Option<u32> {
    Some(match t {
        "u8" | "i8" => 1,
        "u16" | "i16" => 2,
        "u32" | "i32" => 4,
        _ => return None,
    })
}

/// **A field `docs/records.json` names has a row, with that name, that width
/// and that shape.**
///
/// The layout file is where a newly identified field is written first. A row
/// has to follow, or the field joins the ones nobody decided about; and a row
/// whose name or width disagrees with the layout is two documents describing
/// two different fields at one offset.
#[test]
fn every_field_the_layout_names_has_a_row_of_that_name_and_width() {
    let layouts = layouts();
    let rows = rows();
    let mut problems = Vec::new();
    let mut compared = 0usize;
    for record in ["County", "Realm"] {
        let mut want: Vec<(u32, u32, u32, u32, String)> = Vec::new(); // off, width, count, stride, name
        for (off, ty, name) in &layouts[record].fields {
            let (elem, count) = split_type(ty);
            if let Some(w) = scalar_width(elem) {
                want.push((*off, w, count, w, name.clone()));
            } else if let Some(nested) = layouts.get(elem) {
                for (sub_off, sub_ty, sub_name) in &nested.fields {
                    let (sub_elem, sub_count) = split_type(sub_ty);
                    assert_eq!(sub_count, 1, "{elem}.{sub_name}: an array inside a nested record");
                    let w = scalar_width(sub_elem).expect("a scalar inside a nested record");
                    want.push((off + sub_off, w, count, nested.size, format!("{name}[].{sub_name}")));
                }
            } else {
                problems.push(format!("{record}.{name}: type {ty} is neither a scalar nor a struct"));
            }
        }
        for (off, width, count, stride, name) in want {
            compared += 1;
            let id = format!("{record}+{off:#05X}").replace("0X", "0x");
            let Some(r) = rows.iter().find(|r| r.record == record && r.off == off) else {
                problems.push(format!(
                    "docs/records.json names {record} +{off:#05X} `{name}` and the inventory has no \
                     row for it. Add one — imported, derived, or excluded with a reason"
                ));
                continue;
            };
            if r.name != name {
                problems.push(format!("{}: the inventory calls it `{}`, docs/records.json `{name}`", r.id, r.name));
            }
            if r.ty.width() != width {
                problems.push(format!(
                    "{}: the inventory reads {} byte(s), docs/records.json types it {width}",
                    r.id,
                    r.ty.width()
                ));
            }
            if r.count != count || (count > 1 && r.stride != stride) {
                problems.push(format!(
                    "{id}: the inventory has {} x stride {}, docs/records.json {count} x stride {stride}",
                    r.count, r.stride
                ));
            }
        }
    }
    assert!(problems.is_empty(), "docs/stored-fields.json against docs/records.json:\n  {}", problems.join("\n  "));
    // 95 County fields and 51 Realm fields once the three nested arrays are
    // expanded, counted by hand from docs/records.json when this was written.
    // A layout may gain fields; a lower number means the scanner lost some.
    assert!(compared >= 146, "only {compared} layout fields compared — the scanner lost docs/records.json");
}

