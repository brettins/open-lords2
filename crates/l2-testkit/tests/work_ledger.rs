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
//! Both tests shell out to the tool rather than re-reading the JSON here. A Rust
//! copy of the schema would be a second list maintained by whoever maintains
//! the first, in the same commit, for the same reason — two artefacts that agree
//! because they were written to, which `docs/agents.md` records as the pattern
//! that lies.

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

/// **The real ledger passes its schema, and the skipped half says it skipped.**
///
/// The second assertion is not decoration. A check that silently does half its
/// job reads exactly like one that did all of it, so the git half's absence has
/// to be *printed*, the way the census prints what it cannot run.
#[test]
fn the_ledger_passes_its_schema_and_says_it_did_not_compare_git() {
    let Some(out) = work(&["--check", "--schema"]) else {
        return; // no node on this machine; the CI job has one
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "docs/work.json fails its schema. The lead owns the rows, so this is a \
         message for the lead, not a test to adjust:\n{stderr}"
    );
    assert!(
        stdout.contains("SKIP git agreement"),
        "the schema half ran without saying that the git half did not — a skip \
         that prints nothing is a pass that means nothing:\n{stdout}"
    );
}

/// Git in a scratch repository, isolated from this machine's configuration so
/// a developer's global hooks, signing or aliases cannot change what the
/// fixture is.
fn scratch_git(dir: &Path, empty_config: &Path, args: &[&str]) -> Option<Output> {
    Command::new("git")
        .args(["-c", "user.name=l2 fixture", "-c", "user.email=fixture@invalid", "-c", "core.autocrlf=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", empty_config)
        .output()
        .ok()
        .filter(|o| o.status.success())
}

/// **Every figure is `main`'s, and the ledger's provenance is the ledger's.**
///
/// The first version of the view read the inventories out of whatever checkout
/// the tool sat in while its header said `main 76a0437`: run from an agent's
/// worktree, it quoted that worktree's differential and census under main's
/// name, and called a file main had "not on this base". It also stamped the
/// ledger with the *tool's* HEAD, which is a different file's history.
///
/// So this builds a repository whose working tree **disagrees with its `main`
/// on every inventory** — each figure is changed, and `stored-fields.json` is
/// deleted — and whose ledger has uncommitted changes and was last committed
/// one commit *before* HEAD. Every figure in the view must be main's, and the
/// ledger line must name the ledger's own last commit and its uncommitted
/// state. A copy of the tool that reads the working tree reports the worktree's
/// numbers, and a copy that borrows HEAD names the wrong commit; both go red.
#[test]
fn the_view_counts_main_not_the_working_tree_and_dates_the_ledger_by_its_own_commit() {
    let root = root();
    let dir = std::env::temp_dir().join(format!("l2-work-ref-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let config = dir.join("empty.gitconfig");
    std::fs::write(&config, "").unwrap();
    let repo = dir.join("repo");

    let put = |rel: &str, body: &str| {
        let p = repo.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    };
    let copy = |rel: &str| put(rel, &std::fs::read_to_string(root.join(rel)).expect(rel));
    let arms = |statuses: &[&str]| {
        let rows: Vec<String> = statuses
            .iter()
            .enumerate()
            .map(|(i, s)| format!(r#"{{"id": "arm-{i}", "status": "{s}", "group": "g", "gesture": "key"}}"#))
            .collect();
        format!("{{\"arms\": [{}]}}\n", rows.join(", "))
    };
    let audio = |statuses: &[&str]| {
        let rows: Vec<String> = statuses
            .iter()
            .enumerate()
            .map(|(i, s)| format!(r#"{{"id": "site#{i}", "status": "{s}", "class": "file", "sound": null}}"#))
            .collect();
        format!("{{\"sites\": [{}]}}\n", rows.join(", "))
    };
    let differential = |agree: usize, moved_agree: usize| {
        format!(
            "const COMPARED_TOTAL: usize = 932;\nconst AGREE_TOTAL: usize = {agree};\n\
             const MOVED_TOTAL: usize = 279;\nconst MOVED_AGREE_TOTAL: usize = {moved_agree};\n"
        )
    };
    let census = |n: usize| format!("    (\"crates/x/tests/y.rs\", \"install\", {n}),\nconst GATED_TOTAL: usize = {n};\n");
    let ledger = |title: &str| {
        format!(
            "{{\n  \"about\": \"a fixture ledger\",\n  \"states\": {{\n    \"open\": \"known\"\n  }},\n  \"tracks\": {{\n    \"play\": \"a game plays\"\n  }},\n  \"items\": [\n    \
             {{\"id\": \"only\", \"title\": \"{title}\", \"track\": \"play\", \"system\": \"s\", \"state\": \"open\", \"branch\": null, \"depends_on\": [], \"source\": \"\", \"next\": \"\", \"note\": \"\"}}\n  ]\n}}\n"
        )
    };

    // --- main, as committed ------------------------------------------------
    copy("tools/pm/work.js");
    copy("tools/figures/figures.js");
    put("docs/arms.json", &arms(&["reproduced", "reproduced", "missing"]));
    put("docs/audio.json", &audio(&["reproduced", "missing"]));
    put(
        "docs/stored-fields.json",
        "{\"fields\": [{\"id\": \"County+0x000\", \"status\": \"imported\"}, {\"id\": \"County+0x001\", \"status\": \"excluded\"}]}\n",
    );
    put("crates/l2-game/tests/differential.rs", &differential(900, 258));
    put("crates/l2-testkit/tests/census.rs", &census(412));
    put("docs/work.json", &ledger("as committed"));

    let files = [
        "tools/pm/work.js",
        "tools/figures/figures.js",
        "docs/arms.json",
        "docs/audio.json",
        "docs/stored-fields.json",
        "crates/l2-game/tests/differential.rs",
        "crates/l2-testkit/tests/census.rs",
        "docs/work.json",
    ];
    let git = |args: &[&str]| scratch_git(&repo, &config, args);
    let Some(_) = git(&["init", "-q"]) else {
        let _ = std::fs::remove_dir_all(&dir);
        return; // no git on this machine
    };
    git(&["symbolic-ref", "HEAD", "refs/heads/main"]).expect("name the branch main");
    let mut add = vec!["add", "--"];
    add.extend(files);
    git(&add).expect("stage the fixture");
    git(&["commit", "-q", "-m", "main's inventories and the ledger"]).expect("first commit");
    let ledger_commit = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).unwrap().stdout).trim()[..7].to_string();
    put("notes.txt", "a later commit that does not touch the ledger\n");
    git(&["add", "--", "notes.txt"]).expect("stage the note");
    git(&["commit", "-q", "-m", "a later commit"]).expect("second commit");
    let head = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).unwrap().stdout).trim()[..7].to_string();

    // --- the working tree, disagreeing with main on everything ----------------
    put("docs/arms.json", &arms(&["reproduced", "reproduced", "reproduced", "reproduced"]));
    put("docs/audio.json", &audio(&["reproduced", "reproduced", "reproduced"]));
    std::fs::remove_file(repo.join("docs/stored-fields.json")).unwrap();
    put("crates/l2-game/tests/differential.rs", &differential(1, 2));
    put("crates/l2-testkit/tests/census.rs", &census(7));
    put("docs/work.json", &ledger("edited and not committed"));

    let out = Command::new("node")
        .arg(repo.join("tools/pm/work.js"))
        .args(["--status", "--file"])
        .arg(repo.join("docs/work.json"))
        .current_dir(&repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &config)
        .output();
    let _ = std::fs::remove_dir_all(&dir);
    let Ok(out) = out else {
        return; // no node on this machine; the CI job has one
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "the view failed on the fixture:\n{stderr}\n{stdout}");

    for (main_says, worktree_says) in [
        ("figures and git facts: main ", "figures and git facts: (working tree)"),
        ("Input arms: 2 of 3 live arms reproduced", "Input arms: 4 of 4"),
        ("Sound triggers: 1 of 2 trigger sites fire", "Sound triggers: 3 of 3"),
        ("Stored fields: 1 of 2 stored fields carried", "Stored fields: not in"),
        ("End Turn differential: 258 of 279", "End Turn differential: 2 of 279"),
        ("900 of 932 comparisons agree", "1 of 932 comparisons agree"),
        ("Install-gated tests: 412 tests", "Install-gated tests: 7 tests"),
    ] {
        assert!(
            stdout.contains(main_says) && !stdout.contains(worktree_says),
            "the view should say \"{main_says}\" (main's file) and never \"{worktree_says}\" \
             (the working tree's) — a page that names main must count main:\n{stdout}"
        );
    }
    assert!(stdout.contains(&format!("figures and git facts: main {head}")), "the ref line does not name main's commit {head}:\n{stdout}");

    let ledger_line = stdout.lines().find(|l| l.starts_with("ledger: ")).unwrap_or_default();
    assert!(
        ledger_line.starts_with(&format!(
            "ledger: docs/work.json on main, with uncommitted changes since {ledger_commit}"
        )),
        "the ledger line must name the ledger file's own last commit ({ledger_commit}) and say it has \
         uncommitted changes — not the tool's HEAD ({head}):\n{ledger_line}"
    );
}

/// A ledger with one defect per row, and one row with none.
const BROKEN: &str = r##"{
  "about": "a ledger broken on purpose, one defect per row",
  "states": {"open": "known", "in-flight": "being worked"},
  "tracks": {"play": "a game plays"},
  "items": [
    {"id": "fine", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "bad-state", "title": "t", "track": "play", "system": "s", "state": "shipped", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "bad-track", "title": "t", "track": "sound", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "no-next", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "note": ""},
    {"id": "typed-git", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": "", "merged": true},
    {"id": "twice", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "twice", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "gone-dep", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": ["landed-and-left"], "source": "", "next": "", "note": ""},
    {"id": "loop-a", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": ["loop-b"], "source": "", "next": "", "note": ""},
    {"id": "loop-b", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": ["loop-a"], "source": "", "next": "", "note": ""},
    {"id": "branchless", "title": "t", "track": "play", "system": "s", "state": "in-flight", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""}
  ]
}
"##;

/// **Every schema rule goes red, on the row it is about, and on no other.**
///
/// This is the ablation made permanent: `docs/agents.md` asks that a check
/// whose subject is a whole file be made to fail deliberately once, by
/// corrupting its input, and its message read. Here that happens on every run,
/// and what is asserted is the *identity* of each failure — which row, which
/// rule — rather than a count, because a count of nine can be nine of the
/// wrong things.
#[test]
fn a_broken_ledger_fails_and_names_every_broken_row() {
    let dir = std::env::temp_dir().join(format!("l2-work-ledger-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("work.json");
    std::fs::write(&file, BROKEN).expect("write the broken ledger");
    let out = work(&["--check", "--schema", "--file", file.to_str().expect("utf-8 temp path")]);
    let _ = std::fs::remove_dir_all(&dir);
    let Some(out) = out else {
        return; // no node on this machine; the CI job has one
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a ledger with nine defects passed:\n{stderr}");

    let said = |row: &str, words: &str| {
        stderr
            .lines()
            .any(|l| l.starts_with(&format!("work: {row}: ")) && l.contains(words))
    };
    for (row, words) in [
        ("bad-state", "unknown state \"shipped\""),
        ("bad-track", "unknown track \"sound\""),
        ("no-next", "missing field \"next\""),
        ("typed-git", "carries \"merged\", which git answers"),
        ("twice", "duplicate id"),
        ("gone-dep", "depends_on \"landed-and-left\", which is not a row"),
        ("loop-a", "dependency cycle: loop-a -> loop-b -> loop-a"),
        ("branchless", "is in-flight with no branch"),
    ] {
        assert!(said(row, words), "no line names {row} with \"{words}\":\n{stderr}");
    }
    assert!(
        !stderr.contains("work: fine:"),
        "the one well-formed row was reported, so the check is not about rows:\n{stderr}"
    );
}
