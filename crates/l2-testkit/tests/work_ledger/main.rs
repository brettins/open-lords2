//! **The work ledger's schema, checked where the suite runs.**
//!
//! `docs/work.json` holds every piece of live work as a row of *intent*, and
//! `tools/pm/work.js` derives everything git can answer about it. It exists
//! because `docs/plan.md`'s in-flight lists read as current long after they
//! were not — `docs/agents.md`, *The work ledger*.
//!
//! `work.js --check` has two halves. The **git half** needs the clone the work
//! happens in — the agent branches — and a CI runner is a fresh clone that has
//! none, so there it skips and says so. The **schema half** needs only the
//! file, and this is where it runs on every push.
//!
//! Both tests shell out to the tool. A Rust
//! copy of the schema would be a second list maintained by whoever maintains
//! the first, in the same commit, for the same reason — two artefacts that agree
//! because they were written to, which `docs/agents.md` records as the pattern
//! that lies.

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

