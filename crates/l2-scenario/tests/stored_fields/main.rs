//! **`docs/stored-fields.json` against the layout, the decompilation and the
//! original's own saves** — the check that makes *"a stored field our importer
//! drops"* a red test.
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
//! `why`.
//! * [`every_field_the_layout_names_has_a_row_of_that_name_and_width`] — a field
//!   added to `docs/records.json` goes red here until somebody decides about it.
//! * [`every_offset_the_decompilation_touches_has_a_row`] — the original's side:
//!   `node tools/oracle/fields.js --check`, which fails on any county or realm
//!   offset an instruction reads or writes that no row covers. Needs the corpus.
//! * [`every_imported_and_derived_field_reaches_the_kingdom_holding_the_files_bytes`]
//! — and the one that cannot be typed into agreement: after
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
//! a zero neighbour — so the value check prints how many there are
//! letting a green run imply more than it measured.

mod layout_tests;
pub use layout_tests::*;
mod oracle_tests;
pub use oracle_tests::*;
mod accessors_part;
pub use accessors_part::*;
mod value_tests;
pub use value_tests::*;

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
/// count assertion says so.
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

