// `docs/draw-coords.json` against today's tree: the stored counts and the stored
// near misses are what `node tools/draws/drawcoords.js --check` measures now.
// Ablation: change `FROM_AT` in `screens/supplies/mod.rs` from `0x9E` to `0x9C`
// and the check reports `supplies/screen.rs:239 … 2 px out` and `exact 57, stored 58`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

#[test]
fn our_coordinates_are_the_originals() {
    let store = root().join("docs/draw-coords.json");
    assert!(store.is_file(), "docs/draw-coords.json is missing");

    let out = match Command::new("node")
        .args(["tools/draws/drawcoords.js", "--check"])
        .current_dir(root())
        .output()
    {
        Ok(o) => o,
        Err(_) => return, // no node: docs/environment.md says the suite still runs
    };
    let said = String::from_utf8_lossy(&out.stdout).to_string()
        + &String::from_utf8_lossy(&out.stderr);
    // No corpus is a skip, not a pass dressed as one: the tool says which.
    if said.contains("SKIP drawcoords") {
        return;
    }
    assert!(out.status.success(), "drawcoords.js --check:\n{said}");
}

#[test]
fn the_screen_inventory_names_modules_that_exist() {
    let out = match Command::new("node")
        .args(["tools/draws/screendraws.js", "--check"])
        .current_dir(root())
        .output()
    {
        Ok(o) => o,
        Err(_) => return,
    };
    let said = String::from_utf8_lossy(&out.stdout).to_string()
        + &String::from_utf8_lossy(&out.stderr);
    if said.contains("SKIP screendraws") {
        return;
    }
    assert!(out.status.success(), "screendraws.js --check:\n{said}");
}
