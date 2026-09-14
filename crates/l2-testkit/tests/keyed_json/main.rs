//! **The symbol databases are keyed files, and this keeps them mergeable.**
//!
//! `docs/symbols.json` once produced nine conflict hunks in which the `ours`
//! body of every hunk sat under the **wrong entry's `name`** — git had aligned
//! two arrays that were ordered differently, so the diff paired unrelated
//! records and offered `County_PlaceBlacksmith`'s comment as a candidate body
//! for `Move_BuildCostMap`. Nothing was wrong with the data; only the order
//! differed. A textual resolution would have produced a symbol database that
//! parses, reads plausibly, and lies. `docs/agents.md`, *A file that looks like
//! data is usually a claim*.
//!
//! There are two fixes and they are not equal.
//!
//! `tools/symbols/merge-json.js` **handles** the case: a three-way merge keyed
//! per array. The canonical sort **removes** it: two branches that both keep the
//! file in address order cannot produce a misaligned diff in the first place,
//! whatever git does and whether or not the driver is registered. That is the
//! better half, and it is this file's first test — *prefer a shape that cannot
//! be wrong to a check that notices when it is*.
//!
//! The second test is about the driver's one weakness: **git will not run a
//! merge driver a repository merely names.** `.gitattributes` asking for
//! `merge=l2json` does nothing until someone registers it in their own clone,
//! and nobody reads setup instructions. So the check is here, where it runs
//! and it fails with the command in the message.

mod keyed_json_tests;
pub use keyed_json_tests::*;

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

/// The files `.gitattributes` marks `merge=l2json`.
///
/// `docs/arms.json` is the one that matters most and was the last to be added:
/// **three agents are writing it at once**, and it carries both `id` and `addr`
/// with 42 records across 23 addresses, because one function can hold several
/// input arms. Merged on `addr` the driver would have collapsed those 42 into 23
/// and discarded 19 in silence.
/// `docs/audio.json` is the same shape one inventory along — 143 trigger sites
/// across 70 functions, so `Msg_DrawWindow` alone holds twenty-four of them and
/// an `addr` key would discard 73 records in silence. It was registered in the
/// same commit that created it, which is the half `docs/agents.md`'s worked
/// example records as the one nobody checks.
/// `docs/work.json` is the first whose records are not the whole file: they
/// sit under `items` beside `about`, `states` and `tracks`, their **order is
/// intent** (the merge queue reads top to bottom), and each row is one line.
/// So the driver keeps the file's order and shape
/// re-indenting it — `FILE_POLICY` in `merge-json.js` — and
/// [`the_ledger_merges_by_id_and_keeps_its_order_and_its_shape`] is the proof.
const KEYED: &[&str] = &[
    "docs/symbols.json",
    "docs/hypotheses.json",
    "docs/records.json",
    "docs/arms.json",
    "docs/audio.json",
    "docs/stored-fields.json",
    "docs/work.json",
];

