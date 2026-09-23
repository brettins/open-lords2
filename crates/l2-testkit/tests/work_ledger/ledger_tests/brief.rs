use super::*;

/// Row brief-from-ledger-rows. Ablation: dropping `preamble` from `out` in
/// brief.js loses the branch line; dropping the FIELDS loop loses `next`.
#[test]
fn a_brief_is_the_preamble_and_the_rows_in_the_order_asked() {
    let dir = std::env::temp_dir().join(format!("l2-work-brief-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ledger = dir.join("work.json");
    let row = |id: &str, next: &str| {
        format!(r#"{{"id": "{id}", "title": "title of {id}", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "{next}", "note": ""}}"#)
    };
    std::fs::write(
        &ledger,
        format!("{{\"about\": \"x\", \"states\": {{\"open\": \"x\"}}, \"tracks\": {{\"play\": \"x\"}}, \"items\": [{}, {}]}}\n", row("first", "do one"), row("second", "do two")),
    )
    .unwrap();
    let ledger = ledger.to_string_lossy().to_string();
    let Some(out) = work(&["--brief", "second,first", "--branch", "agent/fixture", "--budget", "50", "--file", &ledger]) else {
        let _ = std::fs::remove_dir_all(&dir);
        return; // no node
    };
    let missing = work(&["--brief", "first,nowhere", "--file", &ledger]).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(text.contains("git checkout -b agent/fixture") && text.contains("50 tool calls"), "{text}");
    assert!(!text.contains("{branch}") && !text.contains("{budget}"), "{text}");
    let (two, one) = (text.find("1. second (open): title of second").unwrap(), text.find("2. first").unwrap());
    assert!(two < one && text.contains("next: do two") && text.contains("next: do one"), "{text}");
    assert!(!missing.status.success() && String::from_utf8_lossy(&missing.stderr).contains("nowhere"));
}
