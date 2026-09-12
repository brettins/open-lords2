//! **`docs/stored-fields.json` against the layout, the decompilation and the
//! original's own saves** — the check that makes *"a stored field our importer
//! drops"* a red test rather than a player's report.
//!
//! # Why it exists
//!
//! C142 (the tax panel's *People pay 0 crowns*), C153 (all 56
//! industry forecasts blank on load) and the three farm-row forecasts were one
//! defect found three times. Each field was stored by `Lords2.exe`, modelled by
//! `l2_kingdom`, drawn by our painter and carried by our own save format — and
//! never read out of a `.sav`. The exhaustive destructure of `CountyState` is
//! the defence this crate already had, and it could not see any of them:
//! **a field never added to `CountyState` is not in the list it enumerates.**
//! The producer was guarded, the consumer was guarded, and the gap was upstream
//! of both. So the list has to be the original's, not ours.
//!
//! # The check, from three sides
//!
//! * [`every_stored_field_is_imported_derived_or_excluded_with_a_reason`] —
//!   every row decides: `imported`, `derived`, or `excluded` with a one-line
//!   `why`. There is no fourth status, because *"not decided"* is the defect.
//! * [`every_field_the_layout_names_has_a_row_of_that_name_and_width`] — a field
//!   added to `docs/records.json` goes red here until somebody decides about it.
//! * [`every_offset_the_decompilation_touches_has_a_row`] — the original's side:
//!   `node tools/oracle/fields.js --check`, which fails on any county or realm
//!   offset an instruction reads or writes that no row covers. Needs the corpus.
//! * [`every_imported_and_derived_field_reaches_the_kingdom_holding_the_files_bytes`]
//!   — and the one that cannot be typed into agreement: after
//!   [`Scenario::kingdom`], every row claimed `imported` or `derived` must hold
//!   **the file's own value**, in every county and realm of every save on the
//!   machine. A row marked imported whose field the loader drops fails here with
//!   the save, the county and both numbers.
//!
//! # What it deliberately does not check
//!
//! That an **excluded** row's `why` is true. That is a claim a person reads
//! against the decompilation; `fields.js` prints each offset's writers and
//! readers so the reading is short. And a claimed row that is **zero in every
//! save** is checked only against zero — the corpus cannot tell its offset from
//! a zero neighbour — so the value check prints how many there are rather than
//! letting a green run imply more than it measured.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use l2_formats::save::{Save, COUNTY_BASE, COUNTY_STRIDE, REALM_BASE, REALM_STRIDE};
use l2_kingdom::Kingdom;
use l2_scenario::Scenario;

fn repo_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `<root>/crates/l2-scenario`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-scenario has two ancestors")
        .to_path_buf()
}

/// A row's on-disk type. `Bool` is one byte whose non-zero means true, which is
/// how every flag the kingdom holds as `bool` is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    U8,
    I8,
    U16,
    I16,
    I32,
    Bool,
}

impl Ty {
    fn parse(s: &str) -> Option<Ty> {
        Some(match s {
            "u8" => Ty::U8,
            "i8" => Ty::I8,
            "u16" => Ty::U16,
            "i16" => Ty::I16,
            "i32" => Ty::I32,
            "bool" => Ty::Bool,
            _ => return None,
        })
    }

    fn width(self) -> u32 {
        match self {
            Ty::U8 | Ty::I8 | Ty::Bool => 1,
            Ty::U16 | Ty::I16 => 2,
            Ty::I32 => 4,
        }
    }
}

/// One row of `docs/stored-fields.json`.
#[derive(Debug, Clone)]
struct Row {
    id: String,
    record: String,
    off: u32,
    name: String,
    ty: Ty,
    count: u32,
    stride: u32,
    status: String,
    why: String,
}

impl Row {
    /// **Does this row claim a loaded game holds the file's value?**
    ///
    /// Written as the predicate, not as `status != "excluded"`. That proxy agreed
    /// with it for every status the file defines and disagreed for every one it
    /// does not: ablating a row to `dropped` made the value check fail on *that*
    /// row — "nothing reads the kingdom field that would prove it" — and so hid
    /// the second ablation, a deleted row, which that same run existed to see.
    /// An unknown status is the well-formedness check's to report.
    fn claims(&self) -> bool {
        matches!(self.status.as_str(), "imported" | "derived")
    }

    /// Bytes from element 0's first to the last element's last.
    fn span(&self) -> u32 {
        (self.count.max(1) - 1) * self.stride + self.ty.width()
    }
}

/// `"key": "value"` out of one line of JSON.
fn string_field(line: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\": \"");
    let start = line.find(&pat)? + pat.len();
    let rest = &line[start..];
    Some(rest[..rest.find('"')?].to_string())
}

/// `"key": 12` out of one line of JSON.
fn number_field(line: &str, key: &str) -> Option<u32> {
    let pat = format!("\"{key}\": ");
    let start = line.find(&pat)? + pat.len();
    let digits: String = line[start..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Every row, in file order. **One object per line** is the file's contract,
/// and it is what lets this read it without a JSON crate — the build must not
/// grow a dependency for a test. A reformatted file loses its rows, and the
/// count assertion says so rather than passing on nothing.
fn rows() -> Vec<Row> {
    let path = repo_root().join("docs/stored-fields.json");
    let text = std::fs::read_to_string(&path).expect("docs/stored-fields.json");
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let t = line.trim();
        if !t.starts_with("{\"id\":") {
            assert!(
                !t.contains("\"status\""),
                "docs/stored-fields.json:{}: a row that does not open with \"id\" — one object \
                 per line, `id` first",
                n + 1
            );
            continue;
        }
        let at = |key: &str| {
            string_field(t, key)
                .unwrap_or_else(|| panic!("docs/stored-fields.json:{}: no \"{key}\"", n + 1))
        };
        let id = at("id");
        let (record, off) = id
            .split_once("+0x")
            .unwrap_or_else(|| panic!("docs/stored-fields.json:{}: id {id} is not <record>+0x<offset>", n + 1));
        let ty_name = at("type");
        let ty = Ty::parse(&ty_name).unwrap_or_else(|| {
            panic!("docs/stored-fields.json:{}: {id} has type `{ty_name}`", n + 1)
        });
        out.push(Row {
            record: record.to_string(),
            off: u32::from_str_radix(off, 16)
                .unwrap_or_else(|_| panic!("docs/stored-fields.json:{}: {id}'s offset", n + 1)),
            name: at("name"),
            ty,
            count: number_field(t, "count").unwrap_or(1),
            stride: number_field(t, "stride").unwrap_or(ty.width()),
            status: at("status"),
            why: string_field(t, "why").unwrap_or_default(),
            id,
        });
    }
    assert!(out.len() > 200, "only {} rows parsed — the scanner lost the file", out.len());
    out
}

/// The three answers a stored field may have. See the module documentation.
const STATUSES: &[&str] = &["imported", "derived", "excluded"];

/// Each record's size, from `docs/records.json` and `l2_formats::save`'s strides.
const RECORDS: &[(&str, u32)] = &[("County", COUNTY_STRIDE as u32), ("Realm", REALM_STRIDE as u32)];

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

/// **The original's side**, when the decompiled corpus is on this machine.
///
/// `tools/oracle/fields.js --check` walks every `g_counties[i].x`,
/// `g_counties[i].field_0xNNN`, `i * 0x300 + 0x53fXXX` and absolute `DAT_`
/// access in `tools/oracle/decomp/*.c` — and the same for `g_realms` — and fails
/// on an offset no row covers, or a row no instruction touches. The corpus is
/// gitignored and regenerated by `tools/oracle/decompile-all.ps1`, so this skips
/// where it is absent and says so; `LORDS2_DECOMP` points it at another
/// checkout's copy.
#[test]
fn every_offset_the_decompilation_touches_has_a_row() {
    let root = repo_root();
    let corpus = std::env::var_os("LORDS2_DECOMP")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("tools/oracle/decomp"));
    if !corpus.is_dir() {
        l2_testkit::skip!(
            "no decompiled corpus at {} (LORDS2_DECOMP overrides; tools/oracle/decompile-all.ps1 makes one)",
            corpus.display()
        );
    }
    let out = match std::process::Command::new("node")
        .arg("tools/oracle/fields.js")
        .arg("--check")
        .env("LORDS2_DECOMP", &corpus)
        .current_dir(&root)
        .output()
    {
        Ok(out) => out,
        Err(e) => l2_testkit::skip!("node is not runnable here: {e}"),
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "node tools/oracle/fields.js --check failed:\n{stdout}{stderr}");
    eprintln!("{stdout}");
}

/// The kingdom's value for one element of one row: `(kingdom, county or realm
/// id, element)`.
type Get = fn(&Kingdom, usize, usize) -> i64;

fn at(id: &'static str, get: Get) -> (&'static str, Get) {
    (id, get)
}

/// **Where each claimed row lives in a loaded kingdom.**
///
/// Written by the same hand as the inventory, which `docs/agents.md` warns is
/// the shape of a check that lies — and the reason this one does not is that
/// neither list is what it is compared against. Both are held to the bytes the
/// original wrote.
fn accessors() -> Vec<(&'static str, Get)> {
    vec![
        at("County+0x000", |k, i, _| k.counties[i].event_fired as i64),
        at("County+0x005", |k, i, _| k.counties[i].owner as i64),
        at("County+0x008", |k, i, _| k.counties[i].industry_share as i64),
        at("County+0x009", |k, i, _| k.counties[i].health_band as i64),
        at("County+0x00B", |k, i, _| k.counties[i].health_meter as i64),
        at("County+0x00C", |k, i, _| k.counties[i].happiness as i64),
        at("County+0x00D", |k, i, _| k.counties[i].happiness_last as i64),
        at("County+0x00E", |k, i, _| k.counties[i].d_hap_tax as i64),
        at("County+0x00F", |k, i, _| k.counties[i].d_hap_tax_local as i64),
        at("County+0x010", |k, i, _| k.counties[i].d_hap_health as i64),
        at("County+0x011", |k, i, _| k.counties[i].d_hap_ration as i64),
        at("County+0x012", |k, i, _| k.counties[i].shown_tax as i64),
        at("County+0x013", |k, i, _| k.counties[i].shown_ration as i64),
        at("County+0x014", |k, i, _| k.counties[i].shown_health as i64),
        at("County+0x015", |k, i, _| k.counties[i].shown_army as i64),
        at("County+0x016", |k, i, _| k.counties[i].tax_hap_other as i64),
        at("County+0x017", |k, i, _| k.counties[i].shown_events as i64),
        at("County+0x018", |k, i, _| k.counties[i].happiness_avg as i64),
        at("County+0x01C", |k, i, _| k.counties[i].happiness_sum as i64),
        at("County+0x020", |k, i, _| k.counties[i].unrest as i64),
        at("County+0x021", |k, i, _| k.counties[i].unrest_warned as i64),
        at("County+0x024", |k, i, _| k.counties[i].population as i64),
        at("County+0x028", |k, i, _| k.counties[i].pop_last as i64),
        at("County+0x02C", |k, i, _| k.counties[i].pop_change_pct as i64),
        at("County+0x030", |k, i, _| k.counties[i].births as i64),
        at("County+0x034", |k, i, _| k.counties[i].deaths as i64),
        at("County+0x038", |k, i, _| k.counties[i].army as i64),
        at("County+0x03C", |k, i, _| k.counties[i].emigrants as i64),
        at("County+0x040", |k, i, _| k.counties[i].immigrants as i64),
        at("County+0x044", |k, i, _| k.counties[i].largest_inflow as i64),
        at("County+0x048", |k, i, e| k.counties[i].inflow_sources[e] as i64),
        at("County+0x058", |k, i, _| k.counties[i].emigrant_destination as i64),
        at("County+0x059", |k, i, _| k.counties[i].largest_inflow_source as i64),
        at("County+0x05A", |k, i, _| k.counties[i].neighbour_count as i64),
        at("County+0x05B", |k, i, _| k.counties[i].change_reason as u8 as i64),
        at("County+0x05C", |k, i, e| k.counties[i].neighbours[e] as i64),
        at("County+0x06C", |k, i, _| k.counties[i].anchor_x as i64),
        at("County+0x06D", |k, i, _| k.counties[i].anchor_y as i64),
        at("County+0x090", |k, i, e| k.counties[i].field_progress[e] as i64),
        at("County+0x0B8", |k, i, _| k.counties[i].pop_band as i64),
        at("County+0x0B9", |k, i, _| k.counties[i].tax_rate as i64),
        at("County+0x0BC", |k, i, _| k.counties[i].tax_collected as i64),
        at("County+0x0C0", |k, i, _| k.counties[i].tax_shown as i64),
        at("County+0x0C4", |k, i, e| k.counties[i].labour[e] as i64),
        at("County+0x0C8", |k, i, e| k.counties[i].labour_wanted[e] as i64),
        at("County+0x0CC", |k, i, e| k.counties[i].labour_useful[e] as i64),
        at("County+0x130", |k, i, e| k.counties[i].labour_share[e] as i64),
        at("County+0x15D", |k, i, _| k.counties[i].ration_achieved as i64),
        at("County+0x15E", |k, i, _| k.counties[i].ration_wanted as i64),
        at("County+0x15F", |k, i, _| k.counties[i].ration_split as i64),
        // `Ration_Apply`'s *Fed* row, in the file's order dairy, grain, meat;
        // `people_fed` returns grain, meat, dairy.
        at("County+0x16C", |k, i, e| {
            let (grain, meat, dairy) = l2_kingdom::ration::people_fed(&k.tables, &k.counties[i]);
            [dairy, grain, meat][e] as i64
        }),
        at("County+0x178", |k, i, _| k.counties[i].grain_eaten as i64),
        at("County+0x17C", |k, i, _| k.counties[i].herd_eaten as i64),
        at("County+0x180", |k, i, _| k.counties[i].grain_available as i64),
        at("County+0x184", |k, i, _| k.counties[i].herd_available as i64),
        at("County+0x194", |k, i, _| k.counties[i].shown_ale as i64),
        at("County+0x198", |k, i, _| k.counties[i].friendly_troops as i64),
        at("County+0x19C", |k, i, _| k.counties[i].enemy_troops as i64),
        at("County+0x1A0", |k, i, _| k.counties[i].merchant_visits as i64),
        at("County+0x1A4", |k, i, _| k.counties[i].merchant_count as i64),
        at("County+0x1A5", |k, i, _| k.counties[i].merchant_unit as i64),
        // A `char` in the original: the byte, not the whole quotient.
        at("County+0x1A6", |k, i, _| {
            l2_kingdom::industry::castle_seasons_left(&k.tables, &k.counties[i]) as u8 as i64
        }),
        at("County+0x1A7", |k, i, _| k.counties[i].sow_shortfall as i64),
        at("County+0x1A8", |k, i, _| k.counties[i].tax_suppressed as i64),
        at("County+0x1AA", |k, i, _| k.counties[i].event_id as i64),
        at("County+0x1AD", |k, i, _| k.counties[i].mercenary_offer as i64),
        at("County+0x1B0", |k, i, _| k.counties[i].castle_switch as i64),
        at("County+0x1BC", |k, i, _| k.counties[i].garrison_unit as i64),
        at("County+0x1C0", |k, i, _| k.counties[i].castle_type as i64),
        at("County+0x1C1", |k, i, _| k.counties[i].castle_building as i64),
        at("County+0x1C2", |k, i, _| k.counties[i].castle_ruined as i64),
        at("County+0x1C3", |k, i, _| k.counties[i].castle_degraded as i64),
        at("County+0x1C4", |k, i, _| k.counties[i].castle_percent as i64),
        at("County+0x1CC", |k, i, _| k.counties[i].castle_work_left as i64),
        at("County+0x1D0", |k, i, _| k.counties[i].castle_stone_owed as i64),
        at("County+0x1D4", |k, i, _| k.counties[i].castle_wood_owed as i64),
        at("County+0x1D8", |k, i, _| k.counties[i].castle_work_total as i64),
        at("County+0x1DC", |k, i, _| k.counties[i].castle_stone_total as i64),
        at("County+0x1E0", |k, i, _| k.counties[i].castle_wood_total as i64),
        at("County+0x1E4", |k, i, _| k.counties[i].siege_scars.moat_filled as i64),
        at("County+0x1E6", |k, i, _| k.counties[i].siege_scars.wall_damage as i64),
        at("County+0x1E8", |k, i, _| k.counties[i].siege_scars.breach_score as i64),
        at("County+0x1EC", |k, i, _| k.counties[i].siege_scars.approach_score as i64),
        at("County+0x1F0", |k, i, _| k.counties[i].siege_scars.ramparts_breached as i64),
        at("County+0x1F1", |k, i, _| k.counties[i].siege_scars.gate_open as i64),
        at("County+0x1F4", |k, i, _| k.counties[i].purse as i64),
        at("County+0x1F9", |k, i, _| k.counties[i].castle_level_left as i64),
        at("County+0x1FB", |k, i, _| k.counties[i].event_population_pct as i64),
        at("County+0x1FC", |k, i, _| k.counties[i].event_grain_pct as i64),
        at("County+0x1FD", |k, i, _| k.counties[i].event_herd_pct as i64),
        at("County+0x1FE", |k, i, _| k.counties[i].farm_style as i64),
        at("County+0x1FF", |k, i, _| k.counties[i].fields_fallow as i64),
        at("County+0x200", |k, i, _| k.counties[i].fields_cattle as i64),
        at("County+0x201", |k, i, _| k.counties[i].fields_grain as i64),
        at("County+0x202", |k, i, _| k.counties[i].fields_grain_sown as i64),
        at("County+0x206", |k, i, _| k.counties[i].fields_grain_standing as i64),
        at("County+0x203", |k, i, _| k.counties[i].fields_waste as i64),
        at("County+0x204", |k, i, _| k.counties[i].fields_reclaiming as i64),
        at("County+0x208", |k, i, _| k.counties[i].fertility as i64),
        at("County+0x20C", |k, i, _| k.counties[i].reclaim_fields_finishing as i64),
        at("County+0x214", |k, i, _| k.counties[i].reclaim_seasons_to_next as i64),
        at("County+0x219", |k, i, _| k.counties[i].ale_happiness_given as i64),
        at("County+0x21B", |k, i, _| k.counties[i].weather.index() as i64),
        at("County+0x21D", |k, i, _| k.counties[i].dryness as i64),
        at("County+0x224", |k, i, _| k.counties[i].grain as i64),
        at("County+0x22C", |k, i, _| k.counties[i].grain_change_expected as i64),
        at("County+0x230", |k, i, _| k.counties[i].grain_sown_expected as i64),
        at("County+0x240", |k, i, e| k.counties[i].crop[e] as i64),
        at("County+0x250", |k, i, _| k.counties[i].herd as i64),
        at("County+0x258", |k, i, _| k.counties[i].herd_change_expected as i64),
        at("County+0x25C", |k, i, _| k.counties[i].herd_crowding as i64),
        at("County+0x268", |k, i, _| k.counties[i].herd_births_expected as i64),
        at("County+0x26C", |k, i, _| k.counties[i].herd_deaths_expected as i64),
        at("County+0x24C", |k, i, _| k.counties[i].grain_weather_change as i64),
        at("County+0x270", |k, i, _| k.counties[i].herd_weather_change as i64),
        at("County+0x274", |k, i, _| k.counties[i].herd_event_change as i64),
        at("County+0x278", |k, i, _| k.counties[i].grain_event_change as i64),
        at("County+0x280", |k, i, e| l2_kingdom::industry::panel_figures(&k.tables, &k.counties[i])[e] as i64),
        at("County+0x290", |k, i, _| k.counties[i].weapon_type as i64),
        at("County+0x294", |k, i, e| k.counties[i].industry[e].efficiency as i64),
        at("County+0x295", |k, i, e| k.counties[i].industry[e].has_resource as i64),
        at("County+0x296", |k, i, e| k.counties[i].industry[e].disabled_seasons as i64),
        at("County+0x297", |k, i, e| k.counties[i].industry[e].enabled as i64),
        at("County+0x29E", |k, i, e| k.counties[i].industry[e].capacity as i64),
        at("County+0x2A0", |k, i, e| k.counties[i].industry[e].total as i64),
        // The snapshot is not a field of `Industry`: `output` is the difference
        // the pass takes, so the snapshot is what makes that difference.
        at("County+0x2A4", |k, i, e| {
            let ind = &k.counties[i].industry[e];
            (ind.total - ind.output) as i64
        }),
        at("County+0x2A8", |k, i, e| k.counties[i].industry[e].next_season as i64),
        at("County+0x2F4", |k, i, _| k.counties[i].levy_surcharge as i64),
        at("County+0x2F8", |k, i, _| k.counties[i].event_population_swing as i64),
        at("County+0x2FC", |k, i, _| k.counties[i].grain_grown_expected as i64),
        at("Realm+0x000", |k, i, _| k.realms[i].ai_step as i64),
        at("Realm+0x004", |k, i, _| k.realms[i].strength as i64),
        at("Realm+0x005", |k, i, _| k.realms[i].is_human as i64),
        at("Realm+0x007", |k, i, _| k.realms[i].lord as i64),
        at("Realm+0x00A", |k, i, _| k.realms[i].shield_index as i64),
        at("Realm+0x00C", |k, i, _| k.realms[i].mean_happiness as i64),
        at("Realm+0x010", |k, i, _| k.realms[i].population_total as i64),
        at("Realm+0x014", |k, i, _| k.realms[i].population_mean as i64),
        at("Realm+0x018", |k, i, _| k.realms[i].population_last as i64),
        at("Realm+0x01C", |k, i, _| k.realms[i].offer_pending as i64),
        at("Realm+0x028", |k, i, _| k.realms[i].tax_hap_empire as i64),
        at("Realm+0x029", |k, i, _| k.realms[i].county_count as i64),
        at("Realm+0x02A", |k, i, _| k.realms[i].peak_counties as i64),
        at("Realm+0x02B", |k, i, _| k.realms[i].rank as i64),
        at("Realm+0x02C", |k, i, _| k.realms[i].army_count as i64),
        at("Realm+0x02D", |k, i, e| k.campaign.names.counters(i as u8)[e] as i64),
        at("Realm+0x045", |k, i, _| k.realms[i].muster_timer as i64),
        at("Realm+0x048", |k, i, _| k.realms[i].threat_realm as i64),
        at("Realm+0x04B", |k, i, _| k.realms[i].attack_county as i64),
        at("Realm+0x04C", |k, i, _| {
            k.realms[i].score_inputs[l2_kingdom::tables::SCORE_INPUT_CASTLES] as i64
        }),
        at("Realm+0x050", |k, i, _| k.realms[i].score as i64),
        at("Realm+0x054", |k, i, _| k.realms[i].total_men as i64),
        at("Realm+0x058", |k, i, _| k.realms[i].mean_health as i64),
        at("Realm+0x060", |k, i, _| k.realms[i].share_of_map_pct as i64),
        at("Realm+0x06C", |k, i, _| k.realms[i].weapon_rota as i64),
        at("Realm+0x070", |k, i, e| k.realms[i].want[e] as i64),
        at("Realm+0x080", |k, i, _| k.realms[i].ally_candidate as i64),
        at("Realm+0x081", |k, i, _| k.realms[i].ally as i64),
        at("Realm+0x084", |k, i, e| k.realms[i].pairs[e].standing as i64),
        at("Realm+0x085", |k, i, e| k.realms[i].pairs[e].allied as i64),
        at("Realm+0x086", |k, i, e| k.realms[i].pairs[e].grudge as i64),
        at("Realm+0x087", |k, i, e| k.realms[i].pairs[e].warnings_sent as i64),
        at("Realm+0x088", |k, i, e| k.realms[i].pairs[e].at_war as i64),
        at("Realm+0x089", |k, i, e| k.realms[i].pairs[e].compliments_from as i64),
        at("Realm+0x08C", |k, i, e| k.realms[i].pairs[e].best_gift as i64),
        at("Realm+0x090", |k, i, e| k.realms[i].pairs[e].has_mail as i64),
        at("Realm+0x091", |k, i, e| k.realms[i].pairs[e].help_price_multiple as i64),
        at("Realm+0x0E5", |k, i, _| k.realms[i].muster_county as i64),
        at("Realm+0x0E6", |k, i, _| k.realms[i].raid_county as i64),
        at("Realm+0x0E8", |k, i, _| k.realms[i].target_county as i64),
        at("Realm+0x0E9", |k, i, _| k.realms[i].taunt_timer as i64),
        at("Realm+0x0EA", |k, i, _| k.realms[i].taunt_stage as i64),
        at("Realm+0x0EB", |k, i, _| k.realms[i].war_target as i64),
        at("Realm+0x0EC", |k, i, _| k.realms[i].offer_timer as i64),
        at("Realm+0x0ED", |k, i, _| k.realms[i].crowned_once as i64),
        at("Realm+0x0F4", |k, i, e| k.realms[i].tax_ledger[e] as i64),
        at("Realm+0x0FC", |k, i, _| k.realms[i].wages as i64),
        at("Realm+0x104", |k, i, _| k.realms[i].trade_spent_a as i64),
        at("Realm+0x108", |k, i, _| k.realms[i].trade_spent_b as i64),
        at("Realm+0x10C", |k, i, _| k.realms[i].trade_received_a as i64),
        at("Realm+0x110", |k, i, _| k.realms[i].trade_received_b as i64),
        at("Realm+0x118", |k, i, _| k.realms[i].gold as i64),
        at("Realm+0x120", |k, i, _| k.realms[i].iron as i64),
        at("Realm+0x128", |k, i, _| k.realms[i].stone as i64),
        at("Realm+0x130", |k, i, _| k.realms[i].wood as i64),
        at("Realm+0x138", |k, i, _| k.realms[i].weapons_total() as i64),
        at("Realm+0x140", |k, i, e| k.realms[i].weapons[e] as i64),
        at("Realm+0x158", |k, i, _| k.realms[i].bankrupt_stage as i64),
        at("Realm+0x159", |k, i, _| k.realms[i].voice_rotation as i64),
        at("Realm+0x15A", |k, i, _| k.realms[i].raid_timer as i64),
        at("Realm+0x15C", |k, i, _| k.tax_expected(i as u8) as i64),
    ]
}

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
