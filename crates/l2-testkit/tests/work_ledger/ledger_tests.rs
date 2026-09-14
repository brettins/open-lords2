#![allow(unused_imports)]
use super::*;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    assert!(
        stdout.contains("work: features: schema:") && stdout.contains("SKIP feature references and evidence"),
        "docs/features.json's schema was not checked, or its skipped halves were \
         not said to be skipped:\n{stdout}"
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
/// rule —, because a count of nine can be nine of the
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

// ---- the feature checklist and the player's page ----------------------------

/// A ledger with a row for every section of the player's page, and prose in
/// every row that must never reach that page.
const PAGE_LEDGER: &str = r##"{
  "about": "a fixture ledger with a row in every section of the player's page",
  "states": {"in-flight": "a", "queued-merge": "b", "awaiting-user": "c", "open": "d", "deferred": "e", "abandoned": "f"},
  "tracks": {"play": "p", "screens": "s", "presentation": "v", "instruments": "i", "process": "r"},
  "items": [
    {"id": "row-flight", "title": "Row in flight", "track": "screens", "system": "s", "state": "in-flight", "branch": "worktree-agent-fixture1", "depends_on": [], "source": "PROSE-SOURCE", "next": "PROSE-NEXT", "note": "PROSE-NOTE"},
    {"id": "row-queued", "title": "Row queued", "track": "play", "system": "s", "state": "queued-merge", "branch": "worktree-agent-fixture2", "depends_on": [], "source": "PROSE-SOURCE", "next": "PROSE-NEXT", "note": "PROSE-NOTE"},
    {"id": "row-asks", "title": "Try in the original: a question", "track": "instruments", "system": "s", "state": "awaiting-user", "branch": null, "depends_on": [], "source": "PROSE-SOURCE", "next": "PROSE-NEXT", "note": "PROSE-NOTE"},
    {"id": "row-play", "title": "Row open in play", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "PROSE-SOURCE", "next": "PROSE-NEXT", "note": "PROSE-NOTE"},
    {"id": "row-sound", "title": "Row open in sound", "track": "presentation", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "PROSE-SOURCE", "next": "PROSE-NEXT", "note": "PROSE-NOTE"},
    {"id": "row-tool", "title": "Row open in process", "track": "process", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "PROSE-SOURCE", "next": "PROSE-NEXT", "note": "PROSE-NOTE"},
    {"id": "row-parked", "title": "Row deferred", "track": "play", "system": "s", "state": "deferred", "branch": null, "depends_on": [], "source": "PROSE-SOURCE", "next": "PROSE-NEXT", "note": "PROSE-NOTE"},
    {"id": "row-dropped", "title": "Row abandoned", "track": "process", "system": "s", "state": "abandoned", "branch": null, "depends_on": [], "source": "PROSE-SOURCE", "next": "PROSE-NEXT", "note": "PROSE-NOTE"}
  ]
}
"##;

/// A clean feature list with one feature in each status: two of five graded
/// features done, one out of scope, and one missing feature no row covers.
const PAGE_FEATURES: &str = r##"{
  "about": "a fixture feature list",
  "statuses": {"done": "a", "partial": "b", "missing": "c", "not-assessed": "d", "out-of-scope": "e"},
  "areas": {"realm": "The realm", "war": "War"},
  "features": [
    {"id": "fx-done", "area": "realm", "name": "Taxes", "status": "done", "evidence": ["C1", "docs/decisions.md"], "gap": "", "rows": []},
    {"id": "fx-done-too", "area": "war", "name": "Marching", "status": "done", "evidence": ["arms:g"], "gap": "", "rows": []},
    {"id": "fx-partial", "area": "realm", "name": "Rations", "status": "partial", "evidence": ["crates/not-here.rs"], "gap": "one panel", "rows": ["row-play"]},
    {"id": "fx-missing", "area": "war", "name": "Fog of war", "status": "missing", "evidence": [], "gap": "", "rows": []},
    {"id": "fx-unknown", "area": "war", "name": "Ransom", "status": "not-assessed", "evidence": [], "gap": "", "rows": []},
    {"id": "fx-scope", "area": "realm", "name": "Multiplayer", "status": "out-of-scope", "evidence": [], "gap": "", "rows": ["row-parked"]}
  ]
}
"##;

/// A feature list with one defect per feature, and one feature with none.
const BROKEN_FEATURES: &str = r##"{
  "about": "a feature list broken on purpose, one defect per feature",
  "statuses": {"done": "a", "partial": "b", "missing": "c", "not-assessed": "d", "out-of-scope": "e"},
  "areas": {"realm": "The realm"},
  "features": [
    {"id": "fine", "area": "realm", "name": "Taxes", "status": "done", "evidence": ["C1"], "gap": "", "rows": ["row-play"]},
    {"id": "bad-status", "area": "realm", "name": "Rations", "status": "shipped", "evidence": ["C1"], "gap": "", "rows": []},
    {"id": "bad-area", "area": "sky", "name": "Weather", "status": "missing", "evidence": [], "gap": "", "rows": ["row-play"]},
    {"id": "done-by-vibes", "area": "realm", "name": "Ale", "status": "done", "evidence": [], "gap": "", "rows": []},
    {"id": "vague-partial", "area": "realm", "name": "Cattle", "status": "partial", "evidence": ["C1"], "gap": "", "rows": ["row-play"]},
    {"id": "no-rows", "area": "realm", "name": "Grain", "status": "done", "evidence": ["C1"], "gap": ""},
    {"id": "loose-cite", "area": "realm", "name": "Happiness", "status": "done", "evidence": ["it works in my game"], "gap": "", "rows": []},
    {"id": "long-name", "area": "realm", "name": "A name that runs on and on, well past the one line a feature gets", "status": "done", "evidence": ["C1"], "gap": "", "rows": []},
    {"id": "twice", "area": "realm", "name": "Health", "status": "done", "evidence": ["C1"], "gap": "", "rows": []},
    {"id": "twice", "area": "realm", "name": "Health", "status": "done", "evidence": ["C1"], "gap": "", "rows": []},
    {"id": "gone-row", "area": "realm", "name": "Migration", "status": "missing", "evidence": [], "gap": "", "rows": ["landed-and-left"]}
  ]
}
"##;

/// The `id` of every row of a file kept one row per line, which both
/// `docs/work.json` and `docs/features.json` are.
fn row_ids(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.trim_start().strip_prefix("{\"id\": \""))
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_string)
        .collect()
}

/// **The feature checklist's schema goes red on the feature it is about, the
/// ledger references only when they are compared, and a gap with no row is
/// reported without failing.**
///
/// The same ablation-made-permanent as the ledger's own: each defect is
/// asserted by the feature it names and the rule it breaks, not by a count.
#[test]
fn a_broken_feature_list_fails_and_names_every_broken_feature() {
    let dir = std::env::temp_dir().join(format!("l2-work-features-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let ledger = dir.join("work.json");
    let broken = dir.join("broken.json");
    let clean = dir.join("clean.json");
    std::fs::write(&ledger, PAGE_LEDGER).unwrap();
    std::fs::write(&broken, BROKEN_FEATURES).unwrap();
    std::fs::write(&clean, PAGE_FEATURES).unwrap();
    let s = |p: &Path| p.to_str().expect("utf-8 temp path").to_string();
    let (ledger, broken, clean) = (s(&ledger), s(&broken), s(&clean));

    let schema = work(&["--check", "--schema", "--file", &ledger, "--features", &broken]);
    let full = work(&["--check", "--file", &ledger, "--features", &broken]);
    let reported = work(&["--check", "--schema", "--file", &ledger, "--features", &clean]);
    let _ = std::fs::remove_dir_all(&dir);
    let (Some(schema), Some(full), Some(reported)) = (schema, full, reported) else {
        return; // no node on this machine; the CI job has one
    };

    let stdout = String::from_utf8_lossy(&schema.stdout);
    let stderr = String::from_utf8_lossy(&schema.stderr);
    assert!(!schema.status.success(), "a feature list with eight defects passed:\n{stderr}");
    let said = |text: &str, id: &str, words: &str| {
        text.lines().any(|l| l.starts_with(&format!("work: feature {id}: ")) && l.contains(words))
    };
    for (id, words) in [
        ("bad-status", "unknown status \"shipped\""),
        ("bad-area", "unknown area \"sky\""),
        ("done-by-vibes", "is done and cites nothing"),
        ("vague-partial", "is partial and does not say what is missing"),
        ("no-rows", "missing field \"rows\""),
        ("loose-cite", "is not a citation anybody can check"),
        ("long-name", "a few words"),
        ("twice", "duplicate id"),
    ] {
        assert!(said(&stderr, id, words), "no line names feature {id} with \"{words}\":\n{stderr}");
    }
    assert!(!stderr.contains("work: feature fine:"), "the well-formed feature was reported:\n{stderr}");
    assert!(
        !stderr.contains("work: feature gone-row:") && stdout.contains("SKIP feature references and evidence"),
        "--schema compared the ledger references, or did not say it skipped them:\n{stdout}\n{stderr}"
    );

    let stderr = String::from_utf8_lossy(&full.stderr);
    assert!(
        said(&stderr, "gone-row", "cites ledger row \"landed-and-left\""),
        "a feature citing a row that is not in the ledger was not named:\n{stderr}"
    );
    assert!(!stderr.contains("work: feature fine:"), "the well-formed feature was reported:\n{stderr}");

    let stdout = String::from_utf8_lossy(&reported.stdout);
    assert!(
        reported.status.success(),
        "a clean list whose only gap is a missing feature with no row FAILED -- that is to be reported, \
         not refused:\n{stdout}\n{}",
        String::from_utf8_lossy(&reported.stderr)
    );
    assert!(
        stdout.contains("work:   fx-missing (missing) Fog of war") && !stdout.contains("work:   fx-partial"),
        "the report should name the missing feature no row covers, and not the partial one that has a row:\n{stdout}"
    );
}

/// **The player's page shows every feature and every unmerged row exactly
/// once, a missing feature as missing, computed counts, and none of the prose.**
///
/// Built in a scratch repository whose `main` holds the inventories and the
/// fixture checklist, so the page is generated. Then the real `docs/features.json` and the real ledger are
/// real `main`. Then the real `docs/features.json` and the real ledger are
/// drawn through the same tool, and every one of their ids is counted.
#[test]
fn the_players_page_shows_every_feature_and_every_open_row_once_and_no_prose() {
    let root = root();
    let dir = std::env::temp_dir().join(format!("l2-work-page-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).expect("temp dir");
    let config = dir.join("empty.gitconfig");
    std::fs::write(&config, "").unwrap();
    let put = |rel: &str, body: &str| {
        let p = repo.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    };
    let files: [(&str, String); 9] = [
        ("tools/pm/work.js", std::fs::read_to_string(root.join("tools/pm/work.js")).unwrap()),
        ("tools/figures/figures.js", std::fs::read_to_string(root.join("tools/figures/figures.js")).unwrap()),
        (
            "docs/arms.json",
            r#"{"arms": [{"id": "arm-0", "status": "reproduced", "group": "g", "gesture": "key"}, {"id": "arm-1", "status": "missing", "group": "g", "gesture": "key"}]}"#.into(),
        ),
        ("docs/audio.json", r#"{"sites": [{"id": "site#0", "status": "reproduced", "class": "file", "sound": null}]}"#.into()),
        ("docs/stored-fields.json", r#"{"fields": [{"id": "County+0x000", "status": "imported"}]}"#.into()),
        (
            "crates/l2-game/tests/differential.rs",
            "const COMPARED_TOTAL: usize = 10;\nconst AGREE_TOTAL: usize = 9;\nconst MOVED_TOTAL: usize = 4;\nconst MOVED_AGREE_TOTAL: usize = 3;\n".into(),
        ),
        ("crates/l2-testkit/tests/census.rs", "    (\"crates/x/tests/y.rs\", \"install\", 1),\nconst GATED_TOTAL: usize = 1;\n".into()),
        ("docs/decisions.md", "**C1 — a fixture correction.**\n".into()),
        ("docs/features.json", PAGE_FEATURES.into()),
    ];
    for (rel, body) in &files {
        put(rel, body);
    }
    let git = |args: &[&str]| scratch_git(&repo, &config, args);
    let Some(_) = git(&["init", "-q"]) else {
        let _ = std::fs::remove_dir_all(&dir);
        return; // no git on this machine
    };
    git(&["symbolic-ref", "HEAD", "refs/heads/main"]).expect("name the branch main");
    let mut add = vec!["add", "--"];
    add.extend(files.iter().map(|(rel, _)| *rel));
    git(&add).expect("stage the fixture");
    git(&["commit", "-q", "-m", "main"]).expect("commit the fixture");

    let ledger = dir.join("work.json");
    std::fs::write(&ledger, PAGE_LEDGER).unwrap();
    let page = dir.join("out").join("page.html");
    let detail = dir.join("out").join("detail.html");
    let real_page = dir.join("out").join("real.html");
    let node = |args: &[&std::ffi::OsStr]| {
        Command::new("node")
            .arg(repo.join("tools/pm/work.js"))
            .args(args)
            .current_dir(&repo)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &config)
            .output()
            .ok()
    };
    let os = |s: &str| std::ffi::OsString::from(s);
    let fixture = node(&[&os("--html"), page.as_os_str(), &os("--html-detail"), detail.as_os_str(), &os("--file"), ledger.as_os_str()]);
    let check = node(&[&os("--check"), &os("--file"), ledger.as_os_str()]);
    let real_ledger = root.join("docs/work.json");
    let real_features = root.join("docs/features.json");
    let real = node(&[&os("--html"), real_page.as_os_str(), &os("--file"), real_ledger.as_os_str(), &os("--features"), real_features.as_os_str()]);
    let read = |p: &Path| std::fs::read_to_string(p).unwrap_or_default();
    let (html, detail_html, real_html) = (read(&page), read(&detail), read(&real_page));
    let _ = std::fs::remove_dir_all(&dir);
    let (Some(fixture), Some(check), Some(real)) = (fixture, check, real) else {
        return; // no node on this machine; the CI job has one
    };
    assert!(fixture.status.success(), "the page failed on the fixture:\n{}", String::from_utf8_lossy(&fixture.stderr));
    assert!(real.status.success(), "the page failed on the real files:\n{}", String::from_utf8_lossy(&real.stderr));

    let once = |page: &str, attr: &str, id: &str| page.matches(&format!("{attr}=\"{id}\"")).count() == 1;
    for id in row_ids(PAGE_FEATURES) {
        assert!(once(&html, "data-feature", &id), "feature {id} is not on the player's page exactly once");
    }
    for id in row_ids(PAGE_LEDGER) {
        assert!(once(&html, "data-row", &id), "ledger row {id} is not on the player's page exactly once");
    }
    assert!(
        html.contains(r#"data-feature="fx-missing" data-status="missing""#),
        "the missing feature is not drawn as missing"
    );
    assert!(
        html.contains(r#"data-done="2" data-of="5""#),
        "the tally should count two of five graded features done, leaving the out-of-scope one out"
    );
    for prose in ["PROSE-SOURCE", "PROSE-NEXT", "PROSE-NOTE"] {
        assert!(!html.contains(prose), "{prose} reached the player's page, which carries a row's title and nothing else");
    }
    assert!(
        html.contains("<title>") && !html.contains("<html") && !html.contains("<body"),
        "the page must carry a title and no document tags, so it publishes as an artifact"
    );
    assert!(
        detail_html.contains("What is in flight, and what is left") && detail_html.contains("PROSE-NEXT"),
        "--html-detail should still be the detailed page, next steps and all"
    );

    let stdout = String::from_utf8_lossy(&check.stdout);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("work: feature fx-partial: evidence \"crates/not-here.rs\" is not a file in main")
            && !stderr.contains("work: feature fx-done:")
            && !stderr.contains("work: feature fx-done-too:"),
        "the evidence half should name the one citation main does not hold, and only that one:\n{stderr}"
    );
    assert!(stdout.contains("work:   fx-missing (missing) Fog of war"), "the unrecorded gap was not reported:\n{stdout}");

    let real_features = row_ids(&std::fs::read_to_string(root.join("docs/features.json")).unwrap());
    let real_rows = row_ids(&std::fs::read_to_string(root.join("docs/work.json")).unwrap());
    assert!(!real_features.is_empty() && !real_rows.is_empty(), "no ids were read from the real files: {} features, {} rows", real_features.len(), real_rows.len());
    for id in real_features {
        assert!(once(&real_html, "data-feature", &id), "docs/features.json's {id} is not on the page exactly once");
    }
    for id in real_rows {
        assert!(once(&real_html, "data-row", &id), "docs/work.json's {id} is not on the page exactly once");
    }
}

