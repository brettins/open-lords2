use super::*;
use std::process::Command;

/// Row work-js-fresh-branch-false-merge. Ablation: `unworked` in work.js
/// `gitFacts` returning false makes the child read as merged.
#[test]
fn a_branch_cut_from_a_merged_branch_with_no_commits_is_not_merged() {
    let root = root();
    let dir = std::env::temp_dir().join(format!("l2-work-fresh-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("empty.gitconfig");
    std::fs::write(&config, "").unwrap();
    let repo = dir.join("repo");
    let put = |rel: &str, body: &str| {
        let p = repo.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    };
    let row = |id: &str, branch: &str| {
        format!(r#"{{"id": "{id}", "title": "t", "track": "play", "system": "s", "state": "in-flight", "branch": "{branch}", "depends_on": [], "source": "", "next": "", "note": ""}}"#)
    };
    for rel in ["tools/pm/work.js", "tools/figures/figures.js"] {
        put(rel, &std::fs::read_to_string(root.join(rel)).unwrap());
    }
    put(
        "docs/work.json",
        &format!(
            "{{\n  \"about\": \"fixture\",\n  \"states\": {{\"in-flight\": \"x\"}},\n  \"tracks\": {{\"play\": \"x\"}},\n  \"items\": [\n    {},\n    {}\n  ]\n}}\n",
            row("parent", "worktree-agent-parent"),
            row("child", "worktree-agent-child")
        ),
    );
    let git = |args: &[&str]| scratch_git(&repo, &config, args);
    let Some(_) = git(&["init", "-q"]) else {
        let _ = std::fs::remove_dir_all(&dir);
        return; // no git on this machine
    };
    git(&["symbolic-ref", "HEAD", "refs/heads/main"]).unwrap();
    git(&["add", "-A"]).unwrap();
    git(&["commit", "-q", "-m", "base"]).unwrap();
    git(&["checkout", "-q", "-b", "worktree-agent-parent"]).unwrap();
    put("parent.txt", "work\n");
    git(&["add", "-A"]).unwrap();
    git(&["commit", "-q", "-m", "parent's work"]).unwrap();
    git(&["branch", "worktree-agent-child"]).unwrap();
    git(&["checkout", "-q", "main"]).unwrap();
    git(&["merge", "-q", "--no-ff", "-m", "merge parent", "worktree-agent-parent"]).unwrap();

    let out = Command::new("node")
        .arg(repo.join("tools/pm/work.js"))
        .args(["--check", "--file"])
        .arg(repo.join("docs/work.json"))
        .current_dir(&repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &config)
        .output();
    let _ = std::fs::remove_dir_all(&dir);
    let Ok(out) = out else { return };
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(said.contains("worktree-agent-parent") && said.contains("already merged"), "the control did not fire:\n{said}");
    assert!(!said.contains("worktree-agent-child ("), "an unworked branch read as merged:\n{said}");
}
