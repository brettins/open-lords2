
mod ledger_tests;
pub use ledger_tests::*;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

fn work(args: &[&str]) -> Option<Output> {
    Command::new("node")
        .arg("tools/pm/work.js")
        .args(args)
        .current_dir(root())
        .output()
        .ok()
}

