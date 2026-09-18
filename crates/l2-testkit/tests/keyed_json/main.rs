
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

const KEYED: &[&str] = &[
    "docs/symbols.json",
    "docs/hypotheses.json",
    "docs/records.json",
    "docs/arms.json",
    "docs/audio.json",
    "docs/stored-fields.json",
    "docs/work.json",
];

