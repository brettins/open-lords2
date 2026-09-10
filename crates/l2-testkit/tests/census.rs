//! **The census of install-gated tests**, so that a silent skip stops being
//! the failure mode.
//!
//! # The problem
//!
//! `cargo test --workspace` on this machine and `cargo test --workspace` with
//! `LORDS2_DIR` and `LORDS2_FIXTURES` pointing nowhere — which is what CI does —
//! print **the same** `N passed; 0 failed`, whatever `N` is that week. The two
//! runs assert wildly different amounts and are indistinguishable from their
//! output, because a gated test that finds no game prints a line to stderr and
//! returns green.
//!
//! The gap is [`GATED_TOTAL`] tests, which this file names.
//!
//! Nobody notices a test that stops existing. The reproduction against a real
//! save, the renderer against real sprites, the scenario against the England
//! fixture — the project's strongest evidence — did not run on CI at all, and
//! nothing said so.
//!
//! # The mechanism
//!
//! Every gate in the workspace goes through a macro in `l2-testkit`. This test
//! reads the source of every test file, counts the gated test functions per
//! file and per gate, and asserts the result against [`INVENTORY`] below. It
//! runs everywhere, needs no game, and cannot itself be skipped.
//!
//! So: **add a gated test tomorrow and this goes red** until the inventory is
//! updated, which is a one-line diff that makes the new gate visible in the
//! history. Remove a gate and it goes red the same way. The number is small
//! enough to argue with, which is the point — 91 of 946 test functions were
//! install-gated before anybody counted, and 42 of them resolved the install
//! through a hard-coded path copied between files.
//!
//! It also prints, on every run, how many of those gates the current
//! environment satisfies. A run that asserted a third of what it looks like it
//! asserted now says so.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Which gate a test sits behind, in the order a test body is searched. The
/// order is the precedence: a test that takes both the fixture and the install
/// is counted against the fixture, because that is the stronger requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Gate {
    /// `l2_testkit::england!()` — needs the England turn-one fixture.
    England,
    /// `l2_testkit::fixture!(..)` — needs one named save from `LORDS2_FIXTURES`.
    Fixture,
    /// `l2_testkit::saves!()` — needs at least one save from anywhere.
    Saves,
    /// `l2_testkit::executable!()` — needs `Lords2.exe`.
    Executable,
    /// `l2_testkit::install!()`, or a local helper built on
    /// `l2_testkit::install_dir()`.
    Install,
    /// A `l2_testkit::skip!` on some other condition — a file missing from an
    /// install that is otherwise present, say. Counted, because a skip is a
    /// skip.
    Other,
}

impl Gate {
    fn name(self) -> &'static str {
        match self {
            Gate::England => "england",
            Gate::Fixture => "fixture",
            Gate::Saves => "saves",
            Gate::Executable => "executable",
            Gate::Install => "install",
            Gate::Other => "other",
        }
    }

    /// Is this gate satisfied in the environment the test process is running
    /// in? `Other` cannot be decided from outside the test, so it is `None`.
    fn satisfied(self) -> Option<bool> {
        match self {
            Gate::England => {
                Some(matches!(l2_testkit::england_turn1(), l2_testkit::FixtureState::Ready(_)))
            }
            Gate::Fixture => Some(l2_testkit::fixtures_dir().is_some()),
            Gate::Saves => Some(!l2_testkit::every_available_save().is_empty()),
            Gate::Executable => Some(l2_testkit::executable().is_some()),
            Gate::Install => Some(l2_testkit::install_dir().is_some()),
            Gate::Other => None,
        }
    }
}

/// **The inventory. Update it deliberately.**
///
/// `(file, gate, number of gated `#[test]` functions)`, sorted. A line here is
/// a statement that this many tests in this file do not run without that
/// input.
const INVENTORY: &[(&str, &str, usize)] = &[
    ("crates/l2-formats/tests/battle_fixtures.rs", "fixture", 3),
    ("crates/l2-formats/tests/corpus.rs", "install", 5),
    ("crates/l2-formats/tests/maps.rs", "install", 5),
    ("crates/l2-formats/tests/save.rs", "executable", 1),
    ("crates/l2-formats/tests/save.rs", "saves", 18),
    ("crates/l2-formats/tests/save_england_turn1.rs", "england", 13),
    ("crates/l2-game/tests/ai_war.rs", "england", 2),
    ("crates/l2-game/tests/armoury.rs", "england", 6),
    ("crates/l2-game/tests/audio_install.rs", "england", 1),
    ("crates/l2-game/tests/audio_install.rs", "executable", 1),
    ("crates/l2-game/tests/audio_install.rs", "install", 7),
    ("crates/l2-game/tests/long_game.rs", "england", 3),
    ("crates/l2-game/tests/long_game.rs", "install", 1),
    ("crates/l2-game/tests/long_game.rs", "other", 1),
    ("crates/l2-game/tests/merchant.rs", "england", 7),
    ("crates/l2-game/tests/minimap.rs", "england", 6),
    ("crates/l2-game/tests/newgame.rs", "install", 2),
    ("crates/l2-game/tests/newgame.rs", "other", 2),
    ("crates/l2-game/tests/right_column.rs", "executable", 1),
    ("crates/l2-game/tests/save.rs", "england", 1),
    ("crates/l2-game/tests/save.rs", "install", 1),
    ("crates/l2-game/tests/scenario.rs", "england", 12),
    ("crates/l2-game/tests/scenario.rs", "fixture", 1),
    ("crates/l2-game/tests/screens.rs", "england", 69),
    ("crates/l2-game/tests/screens.rs", "install", 2),
    ("crates/l2-game/tests/seam.rs", "fixture", 3),
    ("crates/l2-game/tests/setup.rs", "england", 8),
    ("crates/l2-game/tests/setup.rs", "install", 1),
    ("crates/l2-game/tests/shell.rs", "install", 3),
    ("crates/l2-game/tests/text.rs", "install", 3),
    ("crates/l2-kingdom/tests/defence.rs", "fixture", 2),
    ("crates/l2-kingdom/tests/fields.rs", "england", 8),
    ("crates/l2-kingdom/tests/oracle.rs", "executable", 6),
    ("crates/l2-kingdom/tests/reproduction.rs", "england", 24),
    ("crates/l2-kingdom/tests/siege.rs", "fixture", 5),
    ("crates/l2-mods/tests/corpus.rs", "install", 6),
    ("crates/l2-scenario/tests/import.rs", "england", 10),
    ("crates/l2-scenario/tests/import.rs", "saves", 11),
    ("crates/l2-scenario/tests/newgame.rs", "england", 1),
    ("crates/l2-scenario/tests/newgame.rs", "install", 4),
    ("crates/l2-sim/tests/oracle.rs", "executable", 4),
    ("crates/l2-view/tests/install.rs", "executable", 2),
    ("crates/l2-view/tests/install.rs", "install", 24),
];

/// The total the inventory adds up to, stated separately so that a change
/// which moves a test between two files still has to be acknowledged as a
/// change in how much of this suite exists on CI.
const GATED_TOTAL: usize = 296;

/// The workspace root, from this crate's manifest.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

/// Every `.rs` under `crates/`, excluding this crate.
fn source_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("crates"), &mut out);
    out.retain(|p| !p.components().any(|c| c.as_os_str() == "l2-testkit"));
    out
}

fn relative(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// The needles that name a gate, strongest first. A body containing several is
/// counted against the first that matches.
const NEEDLES: &[(&str, Gate)] = &[
    ("england!(", Gate::England),
    ("england_turn1(", Gate::England),
    ("fixture!(", Gate::Fixture),
    ("fixture_save(", Gate::Fixture),
    ("fixtures_dir(", Gate::Fixture),
    ("saves!(", Gate::Saves),
    ("every_available_save(", Gate::Saves),
    ("executable!(", Gate::Executable),
    ("l2_testkit::executable(", Gate::Executable),
    ("install!(", Gate::Install),
    ("install_dir(", Gate::Install),
    ("read_install(", Gate::Install),
    ("l2_testkit::skip!(", Gate::Other),
];

/// The gate a chunk of source sits behind, if any, counting only what the
/// chunk itself names.
fn direct_gate(body: &str) -> Option<Gate> {
    NEEDLES.iter().find(|(needle, _)| body.contains(needle)).map(|&(_, g)| g)
}

/// Split a file into its `#[test]` functions. Everything before the first is
/// the preamble — helpers and macro definitions — and is not a test.
fn test_bodies(src: &str) -> Vec<&str> {
    src.split("#[test]").skip(1).collect()
}

/// **One level of indirection has to be followed**, or the census undercounts
/// badly: most gated tests call a file-local `macro_rules!` or helper that
/// holds the gate, and the fifteen screen tests would have counted as zero.
///
/// So the preamble is chopped into named items and each is given the gate its
/// own text names; a test naming such an item inherits it.
fn local_items(preamble: &str) -> Vec<(String, Gate)> {
    let mut marks: Vec<(usize, String)> = Vec::new();
    for (pat, skip) in [("macro_rules! ", 14usize), ("\nfn ", 4), ("\n    fn ", 8)] {
        let mut from = 0usize;
        while let Some(rel) = preamble[from..].find(pat) {
            let at = from + rel;
            let name: String = preamble[at + skip..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                marks.push((at, name));
            }
            from = at + pat.len();
        }
    }
    marks.sort();
    let mut out = Vec::new();
    for (i, (at, name)) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map(|(a, _)| *a).unwrap_or(preamble.len());
        if let Some(g) = direct_gate(&preamble[*at..end]) {
            out.push((name.clone(), g));
        }
    }
    out
}

fn gate_of(body: &str, items: &[(String, Gate)]) -> Option<Gate> {
    let mut best = direct_gate(body);
    for (name, gate) in items {
        if body.contains(&format!("{name}!(")) || body.contains(&format!("{name}(")) {
            best = Some(match best {
                Some(b) => b.min(*gate),
                None => *gate,
            });
        }
    }
    best
}

fn scan() -> BTreeMap<(String, &'static str), usize> {
    let root = repo_root();
    let mut found: BTreeMap<(String, &'static str), usize> = BTreeMap::new();
    for path in source_files(&root) {
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        if !src.contains("l2_testkit") {
            continue;
        }
        let preamble = src.split("#[test]").next().unwrap_or("");
        let items = local_items(preamble);
        for body in test_bodies(&src) {
            if let Some(gate) = gate_of(body, &items) {
                *found.entry((relative(&root, &path), gate.name())).or_insert(0) += 1;
            }
        }
    }
    found
}

/// The census itself.
#[test]
fn the_install_gated_tests_are_the_ones_we_have_written_down() {
    let found = scan();
    let expected: BTreeMap<(String, &'static str), usize> =
        INVENTORY.iter().map(|&(f, g, n)| ((f.to_string(), g), n)).collect();

    if found != expected {
        let mut lines = String::new();
        for ((file, gate), n) in &found {
            lines.push_str(&format!("    (\"{file}\", \"{gate}\", {n}),\n"));
        }
        panic!(
            "the gate census has moved.\n\n\
             A gated test does not run on CI, and nothing else in the suite says so - which is \
             why this number is written down. If the change is intended, replace INVENTORY in \
             crates/l2-testkit/tests/census.rs with:\n\n\
             const INVENTORY: &[(&str, &str, usize)] = &[\n{lines}];\n\n\
             expected {} entries, found {}",
            expected.len(),
            found.len()
        );
    }
    assert_eq!(
        found.values().sum::<usize>(),
        GATED_TOTAL,
        "{GATED_TOTAL} test functions in this workspace do not exist without a copy of the game"
    );
    eprintln!("gate census: {GATED_TOTAL} install-gated tests across {} files", INVENTORY.len());
}

/// **What this run actually asserted**, printed every time and asserted at the
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
