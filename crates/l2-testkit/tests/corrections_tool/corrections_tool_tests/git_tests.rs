use super::*;

impl Tree {
    fn git(&self, args: &[&str]) -> Output {
        let config = self.dir.with_extension("gitconfig");
        let _ = std::fs::write(&config, "");
        Command::new("git")
            .args(["-c", "user.name=l2 fixture", "-c", "user.email=fixture@invalid", "-c", "core.autocrlf=false"])
            .args(args)
            .current_dir(&self.dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &config)
            .output()
            .expect("git runs")
    }

    fn commit_all(&self, msg: &str) {
        assert!(self.git(&["add", "-A"]).status.success());
        assert!(self.git(&["commit", "-q", "-m", msg]).status.success(), "commit {msg}");
    }
}

/// Row assign-refuses-conflicted-log. Ablation: the `refuseConflicts` call in
/// `readLogs` removed lets --assign number a half-merged log.
#[test]
fn a_log_holding_conflict_markers_is_refused_by_every_mode() {
    let Some(tree) = Tree::new("conflicted") else { return };
    let one = tag('C', "half");
    tree.append("docs/decisions.md", &format!("\n<<<<<<< HEAD\n**{one} — ours.**\n=======\n**C3 — theirs.**\n>>>>>>> b\n"));
    let before = tree.snapshot();
    for args in [vec!["--check"], vec!["--assign", one.as_str()], vec!["--relock"]] {
        let out = tree.run(&args);
        assert!(!out.status.success(), "{args:?} accepted a conflicted log");
        assert!(err(&out).contains("docs/decisions.md:") && err(&out).contains("conflict marker"), "{}", err(&out));
    }
    assert!(tree.snapshot() == before, "a refused run changed the tree");
}

/// Row assign-gate-reads-worktree. Ablation: `materialise` returning `base`
/// makes `--at HEAD` read the clean worktree and pass.
#[test]
fn check_at_head_sees_the_placeholder_the_worktree_no_longer_has() {
    let Some(tree) = Tree::new("at-head") else { return };
    if !tree.git(&["init", "-q"]).status.success() {
        return;
    }
    tree.commit_all("base");
    let one = tag('C', "late");
    tree.append("docs/decisions.md", &format!("\n**{one} — committed unnumbered.**\n"));
    tree.relock();
    tree.commit_all("a placeholder reaches the commit");
    assert!(tree.run(&["--assign", &one]).status.success());
    assert!(tree.run(&["--check"]).status.success(), "the worktree should be clean after --assign");
    let out = tree.run(&["--check", "--at", "HEAD"]);
    assert!(!out.status.success(), "--at HEAD passed a commit holding {one}");
    assert!(err(&out).contains(&one), "{}", err(&out));
}

/// Row assign-gate-reads-worktree. Ablation: `stageDuringMerge` returning
/// early leaves the merge commit holding the placeholder.
#[test]
fn assign_during_a_merge_stages_what_it_wrote() {
    let Some(tree) = Tree::new("merging") else { return };
    if !tree.git(&["init", "-q"]).status.success() {
        return;
    }
    tree.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
    tree.commit_all("base");
    tree.git(&["checkout", "-q", "-b", "b"]);
    let one = tag('C', "merged");
    tree.append("docs/decisions.md", &format!("\n**{one} — from a branch.**\n"));
    tree.relock();
    tree.commit_all("branch");
    tree.git(&["checkout", "-q", "main"]);
    assert!(tree.git(&["merge", "-q", "--no-ff", "--no-commit", "b"]).status.success());
    assert!(tree.run(&["--assign", &one]).status.success());
    let staged = String::from_utf8_lossy(&tree.git(&["show", ":docs/decisions.md"]).stdout).to_string();
    assert!(staged.contains("**C3 — from a branch.**") && !staged.contains(&one), "{staged}");
    assert!(tree.run(&["--check", "--at", "index"]).status.success());
}
