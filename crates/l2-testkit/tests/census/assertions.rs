#![allow(unused_imports)]
use super::*;
use super::scanner::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// **What this run **
/// ends.
///
/// Run it with `--nocapture` to see the breakdown. The two ends are the ones
/// worth pinning: with nothing configured every gated test skips, and with
/// everything configured none of the decidable ones does.
#[test]
fn the_number_of_tests_this_environment_will_skip_is_reported_and_bounded() {
    let found = scan();
    let total: usize = found.values().sum();

    let mut running = 0usize;
    let mut skipping = 0usize;
    let mut undecidable = 0usize;
    let mut by_gate: BTreeMap<&str, (usize, Option<bool>)> = BTreeMap::new();
    for ((_, gate), n) in &found {
        let g = [Gate::England, Gate::Fixture, Gate::Saves, Gate::Executable, Gate::Install, Gate::Other]
            .into_iter()
            .find(|g| g.name() == *gate)
            .expect("a gate name the scanner produced");
        let entry = by_gate.entry(gate).or_insert((0, g.satisfied()));
        entry.0 += n;
        match g.satisfied() {
            Some(true) => running += n,
            Some(false) => skipping += n,
            None => undecidable += n,
        }
    }

    eprintln!("\ninstall-gated tests: {total}");
    for (gate, (n, satisfied)) in &by_gate {
        let state = match satisfied {
            Some(true) => "available",
            Some(false) => "MISSING - these tests will skip",
            None => "decided inside the test",
        };
        eprintln!("  {n:>3}  {gate:<11} {state}");
    }
    eprintln!(
        "  ->  {running} will run, {skipping} will skip, {undecidable} decide for themselves\n"
    );

    assert_eq!(running + skipping + undecidable, total);

    // The two ends, pinned. Anything between them is a partly configured
    // machine and is nobody's business but its own.
    let nothing_configured = l2_testkit::install_dir().is_none()
        && l2_testkit::fixtures_dir().is_none();
    if nothing_configured {
        assert_eq!(
            running, 0,
            "nothing is configured, so no gated test can be running"
        );
        assert_eq!(skipping + undecidable, total);
    }
    if matches!(l2_testkit::england_turn1(), l2_testkit::FixtureState::Ready(_))
        && l2_testkit::install_dir().is_some()
    {
        assert_eq!(
            skipping, 0,
            "everything is configured, so no decidable gate should be skipping"
        );
    }
}

/// **One hard-coded install path in the workspace, and it is in `l2-testkit`.**
///
/// Forty-two test functions used to carry their own copy of
/// `F:\games\Lords of the Realm II`, so the suite behaved differently on the
/// author's machine from everywhere else and no single place could be changed
/// to fix it. Re-introducing one fails here.
#[test]
fn no_test_carries_its_own_copy_of_a_game_directory() {
    let root = repo_root();
    let mut offenders = Vec::new();
    for path in source_files(&root) {
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        for (n, line) in src.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            let is_path = lower.contains("games\\lords")
                || lower.contains("games/lords")
                || lower.contains("games\\lords2")
                || lower.contains("games/lords2");
            // A path inside a doc comment is documentation, not a fall back.
            let is_doc = line.trim_start().starts_with("//") || line.trim_start().starts_with("#");
            if is_path && !is_doc {
                offenders.push(format!("{}:{}: {}", relative(&root, &path), n + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a game directory is hard-coded outside l2-testkit:\n  {}\n\n\
         Use l2_testkit::install_dir() / fixtures_dir(); the defaults live in one place so that \
         one place can be changed.",
        offenders.join("\n  ")
    );
}

/// **No game asset has crept into the repository.** `.gitignore` refuses them
/// and `CLAUDE.md` rule 1 forbids them, but a `git add -f` would defeat both
/// silently — and the fixture work of this task moved five `.sav` files around
/// on disk, which is exactly when such a thing happens.
#[test]
fn no_game_data_is_checked_in() {
    let root = repo_root();
    let mut offenders = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.filter_map(|e| e.ok()) {
            let p = e.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if p.is_dir() {
                // `.claude/worktrees` holds other agents' checkouts of this same
                // repository; walking into them would report their files as ours.
                if name == "target" || name == "node_modules" || name.starts_with('.') {
                    continue;
                }
                walk(&p, out);
            } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_ascii_lowercase();
                // `l2sav` is **ours**, not the publisher's, so it is not here
                // for rule 1's reason. It is here for the other one: a saved
                // game belongs in `%APPDATA%\open-lords2\saves`, and one that
                // has appeared in the working tree is a test writing where it
                // should not. `docs/decisions.md` D11.
                if ["sav", "l2sav", "pl8", "256", "smk", "wav", "saf"].contains(&ext.as_str()) {
                    out.push(p);
                }
            }
        }
    }
    walk(&root, &mut offenders);
    assert!(
        offenders.is_empty(),
        "game data is in the working tree: {:?}\nCLAUDE.md rule 1: never commit game assets.",
        offenders
    );
}

