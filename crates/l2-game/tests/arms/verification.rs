#![allow(unused_imports)]
use super::*;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use l2_game::press::Kind;

// both lists. Say so: the whole reason
    // the gesture is on the marker is that answering an arm with the wrong kind
    // used to be invisible, and reporting it as two unrelated failures would
    // put it back.
    let by_id: BTreeMap<&str, &Marker> = marked.iter().map(|m| (m.id.as_str(), m)).collect();
    let mut mismatched: Vec<String> = Vec::new();
    for c in &claimed {
        if let Some(m) = by_id.get(c.id.as_str()) {
            if m.gesture != c.gesture {
                mismatched.push(format!(
                    "{}\n      docs/arms.json says the original's gesture is `{}`\n      \
                     the marker in {} says we answer `{}`",
                    c.id,
                    c.gesture,
                    found[*m].join(", "),
                    if m.gesture.is_empty() { "<no gesture on the marker>" } else { &m.gesture },
                ));
            }
        }
    }
    assert!(
        mismatched.is_empty(),
        "these arms are implemented with the WRONG KIND of gesture, or the marker and the \
         record have drifted apart:\n  {}\n\
         An arm answered with the wrong kind is not reproduced. `docs/input.md` says what \
         each kind is; fix the handler, or the record if the record is what is wrong.",
        mismatched.join("\n  "),
    );

    let describe = |m: &Marker| format!("{} {}", m.id, m.gesture);
    assert!(
        unkept.is_empty(),
        "docs/arms.json claims these are implemented and no `// arm:` marker says so:\n  {}",
        unkept.iter().map(|m| describe(m)).collect::<Vec<_>>().join("\n  "),
    );
    assert!(
        unrecorded.is_empty(),
        "these `// arm:` markers are in the code and not in docs/arms.json:\n  {}",
        unrecorded
            .iter()
            .map(|m| format!("{}  ({})", describe(m), found[*m].join(", ")))
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

/// **Every gesture is one the file defines, and every marker carries one.**
///
/// The second half is what stops the pair check above from being satisfied by
/// leaving the gesture off both sides.
#[test]
fn every_arm_names_a_gesture_from_the_closed_vocabulary() {
    let root = repo_root();
    for r in records(&root) {
        assert!(
            GESTURES.contains(&r.gesture.as_str()),
            "{} has gesture `{}`, which is not one of {GESTURES:?}.\n\
             A new kind is a claim about how the original decides a control was used. \
             Read `docs/input.md` first; if it really is new, add it there and here \
             together.",
            r.id,
            r.gesture,
        );
    }
    for (m, files) in markers(&root) {
        assert!(
            GESTURES.contains(&m.gesture.as_str()),
            "the marker for {} in {} carries `{}`, which is not a gesture. \
             The marker is `// arm: <id> <gesture>`.",
            m.id,
            files.join(", "),
            m.gesture,
        );
    }
}

/// **A kind only `Press` answers is declared by that kind, never claimed by a
/// comment.**
///
/// The three are `Widget_Test`'s kinds 4 and 5 and `Hotspot_Test`'s kind 2.
/// Nothing in this engine answers them but a `press::Kind` handed to
/// `press::Press` —
/// and the only marker that cannot disagree with that value is the value. A
/// comment beside a table could say `left-press-delayed` over a row declared
/// `Kind::Press`, and did, and the set check above was satisfied by the comment.
///
/// **Ablations, run:** declare `opt-music`'s row `Press` in its `arm!` and the
/// set check above goes red, naming the arm and both words; replace the `arm!`
/// with a bare `Kind::Press` and put the old comment back above it, and this
/// one goes red naming the file and line.
#[test]
fn a_press_only_gesture_is_declared_by_its_kind_and_never_by_a_comment() {
    let root = repo_root();
    let press_only: BTreeSet<&'static str> = kinds_by_name()
        .into_values()
        .filter(|k| !matches!(k, Kind::Press | Kind::Release))
        .map(Kind::gesture)
        .collect();
    assert_eq!(press_only.len(), 3, "repeat, delayed and held: {press_only:?}");

    let all = sites(&root);
    let typed: Vec<String> = all
        .iter()
        .filter(|s| s.how == How::Comment && press_only.contains(s.marker.gesture.as_str()))
        .map(|s| format!("{}  // arm: {} {}", s.at, s.marker.id, s.marker.gesture))
        .collect();
    assert!(
        typed.is_empty(),
        "these markers claim a kind only press::Press answers, in a comment:\n  {}\n\
         A comment can say `left-press-delayed` over a widget declared `Kind::Press`, and \
         nothing would notice. Delete the comment and write the widget's kind as an \
         arm! declaration — the id and `Delayed` as its two arguments — so the marker and \
         the kind are one token.",
        typed.join("\n  "),
    );

    // **And the declared half is not empty**, or the scanner went blind and the
    // assertion above is vacuous. Thirty-four when this was written: every
    // widget table in `screens/`, and the auto-repeat in `press.rs`.
    let declared = all.iter().filter(|s| s.how == How::Declared).count();
    assert!(declared >= 34, "only {declared} arm! declarations found; it was 34");
}

/// One marker per arm
#[test]
fn no_arm_is_marked_twice() {
    let root = repo_root();
    // By `id`, not by the whole marker: two markers for one arm that disagree
    // about the gesture are still two markers for one arm, and folding them
    // into separate keys would hide exactly that case.
    let mut places: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (m, files) in markers(&root) {
        places.entry(m.id).or_default().extend(files);
    }
    for (id, files) in places {
        assert_eq!(files.len(), 1, "{id} is marked in {} places: {files:?}", files.len());
    }
}

/// Every id starts with something a `grep` for the marker convention finds, and
/// every status is one the file defines.
#[test]
fn the_inventory_uses_the_defined_statuses_and_the_marker_shape() {
    let root = repo_root();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in records(&root) {
        let (id, status) = (&r.id, &r.status);
        assert!(
            id.starts_with("0x") || id.starts_with("ours/"),
            "{id} is neither an address nor an invention",
        );
        assert!(
            STATUSES.contains(&status.as_str()),
            "{id} has status {status}, which is not one of {STATUSES:?}",
        );
        if r.in_the_tree() {
            let ours = r.ours.clone().unwrap_or_default();
            assert!(ours.starts_with("crates/"), "{id} does not say where it lives");
        }
        *counts.entry(status.clone()).or_default() += 1;
    }
    // Four of the five must be populated — they are the four things this file
    // exists to count, and an empty one means the enumeration stopped early.
    //
    // `dead-reproduced` is the exception and is expected to be EMPTY: it names
    // work we should not have done. It is asserted the other way round, because
    // a category that quietly acquires members is exactly how "we built an arm
    // no player can reach" would stop being a finding and become a bucket.
    for s in ["reproduced", "missing", "dead", "invention"] {
        assert!(counts.get(s).copied().unwrap_or(0) > 0, "no {s} arms at all");
    }
    let built_dead = counts.get("dead-reproduced").copied().unwrap_or(0);
    assert_eq!(
        built_dead, 0,
        "{built_dead} arm(s) are marked dead-reproduced: we have built input the shipped \
         game cannot reach. That is a finding, not a status to live with — read each one \
         and delete the code, or prove the screen is reachable after all and re-file it. \
         If it is genuinely worth keeping, change this assertion deliberately and say why."
    );
}

/// **Every `group` an arm names is declared in `groups`.**
///
/// This exists because a merge ate five declarations without failing anything.
/// The keyed JSON driver merges the `arms` **array** by `id` — it does that
/// correctly, and it says so — and then takes one side's `groups` object and
/// `_note` wholesale. On the first concurrent rebase of this file that dropped
/// five group declarations and twenty-one lines of prose, in the file whose
/// entire purpose is counting, and every test above stayed green because every
/// one of them reads the `arms` array and nothing else.
///
/// A `group` is *"the unit somebody can claim complete, with an owner"*. An arm
/// filed under a group that does not exist has no owner and no completeness
/// flag, so it is exactly the arm that stops being counted.
#[test]
fn every_group_an_arm_names_is_declared() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join("docs/arms.json")).expect("docs/arms.json");

    // The `groups` object's keys are the two-space-indented `"name": {` lines
    // inside it, and the `arms` array's are six-space-indented `"group": "x"`.
// A scan, for the reason `records` gives.
    let mut declared: BTreeSet<String> = BTreeSet::new();
    let mut in_groups = false;
    for line in text.lines() {
        if line.starts_with("  \"groups\": {") {
            in_groups = true;
            continue;
        }
        if in_groups {
            if line == "  }," || line == "  }" {
                in_groups = false;
                continue;
            }
            if let Some(rest) = line.strip_prefix("    \"") {
                if let Some((name, tail)) = rest.split_once("\":") {
                    if tail.trim_start().starts_with('{') {
                        declared.insert(name.to_string());
                    }
                }
            }
        }
    }
    assert!(!declared.is_empty(), "no groups parsed — the scanner lost the file");

    let mut used: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("\"group\": \"") {
            if let Some((value, _)) = rest.split_once('"') {
                used.insert(value.to_string());
            }
        }
    }
    assert!(used.len() > 1, "only {} groups used — the scanner lost the file", used.len());

    let undeclared: Vec<&String> = used.difference(&declared).collect();
    assert!(
        undeclared.is_empty(),
        "these arms name a group that docs/arms.json does not declare:\n  {}\n\
         A declaration carries the owner and the `complete` flag, so an arm without one is \
         an arm nobody can claim or finish. If a merge dropped them, put them back; the \
         keyed driver does not guard anything outside the `arms` array.",
        undeclared.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n  "),
    );

    // And the other way
    // nobody files under is a group whose arms went somewhere else.
    let unused: Vec<&String> = declared.difference(&used).collect();
    assert!(
        unused.is_empty(),
        "these groups are declared and no arm is filed under them:\n  {}",
        unused.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n  "),
    );
}

/// **The gesture, checked against the player's own `Lords2.exe`.**
///
/// This is the half that cannot be typed into agreement. Everything above
/// compares two things a person maintains; this one derives the answer from the
/// game
/// fires on the press goes red however carefully the record was written.
///
/// **How.** The interface is data. `Widget_Test` (`0x0040DA1E`) and
/// `Hotspot_Test` (`0x0040E3EE`) both walk arrays of 24-byte records with a
/// handler pointer at `+0x08` and a **kind byte at `+0x0F`**, and the kind byte
/// is the whole of what decides press, release, hold or repeat. So: scan the
/// two regions of `.data` the interface's tables live in, build handler address
/// -> kind, and cross it with `docs/arms.json`'s `addr`.
///
/// **What it does not cover, said in the same sentence as the number.** Only
/// records whose `addr` is a *table handler* — 39 of 211 today. An arm
/// dispatched from `Screen_FrameInput`'s own ladder reads
/// `g_mouseLeftPressed` / `g_mouseRightReleased` inline and has no kind byte to
/// read, so this check is silent about it. It is silent, not green: the
/// coverage is asserted below so that it cannot quietly fall to zero.
#[test]
fn the_gesture_of_every_table_handler_is_the_exes_own_kind_byte() {
    let exe = l2_testkit::executable!();

    // The two regions, and how they were established: the first is the lowest
    // hotspot table named by any `Hotspot_Test` call site in the decompilation
    // (`0x004DC4D0`, the six `FUN_00438B02` records), the last widget table is
    // the save/load scroll pair. The tables are laid out contiguously at a
    // 24-byte stride from the first, which is what makes a scan possible at
    // all — and what makes the two spot checks below necessary, because a
// wrong base would decode plausible-looking rubbish.
    const HOTSPOTS: (u32, u32) = (0x004D_C4D0, 0x004D_D310);
    const WIDGETS: (u32, u32) = (0x004D_D310, 0x004D_E400);

    let mut kinds: BTreeMap<u32, BTreeSet<&'static str>> = BTreeMap::new();
    // **And the same thing keyed by the RECORD's address**
    // the prose check below reach the four arms whose `addr` is a dispatcher.
    // See `docs/input.md` §7a.
    let mut at_record: BTreeMap<u32, &'static str> = BTreeMap::new();
    // And which handler each record calls, so that a handler reachable at two
    // kinds can be settled by the one record an arm's own prose names.
    let mut handler_at: BTreeMap<u32, u32> = BTreeMap::new();
    for (widget, (lo, hi)) in [(false, HOTSPOTS), (true, WIDGETS)] {
        let mut va = lo;
        while va < hi {
            let t = l2_testkit::pe::Table::at(&exe, va);
            let handler = (0..4).fold(0u32, |a, i| a | (t.u8_at(8 + i) as u32) << (8 * i));
            let kind = t.u8_at(0x0F);
            if (0x0040_1000..0x004D_0000).contains(&handler) {
                if let Some(g) = gesture_of_kind(widget, kind) {
                    kinds.entry(handler).or_default().insert(g);
                    at_record.insert(va, g);
                    handler_at.insert(va, handler);
                }
            }
            va += 24;
        }
    }

    // **Two spot checks on the scan itself, before believing a word of it.**
    // A wrong base or stride decodes garbage that still parses, so pin one
    // record in each region against a handler this project has already named
    // from a call site. `Turn_End` is the End Turn strip; `Ui_ConfirmClicked`
    // is the yes/no box's pair of gauntlets.
    assert_eq!(
        kinds.get(&0x0043_AC23).map(|s| s.iter().copied().collect::<Vec<_>>()),
        Some(vec!["left-press"]),
        "the hotspot scan did not find Turn_End (0x0043AC23) at kind 1 — the base or the \
         stride is wrong and every verdict below it is rubbish",
    );
    assert_eq!(
        kinds.get(&0x0043_4E1F).map(|s| s.iter().copied().collect::<Vec<_>>()),
        Some(vec!["left-press-delayed"]),
        "the widget scan did not find Ui_ConfirmClicked (0x00434E1F) at kind 5",
    );

    let mut checked = 0usize;
    let mut wrong: Vec<String> = Vec::new();
    // A handler reachable from two tables with two different kinds cannot be
    // classified from its address alone. There is one, and it is named rather
    // than skipped silently.
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();

    // **The prose half**, which is half the blind spot closed. See
    // `docs/input.md` §7a for why it is record bases only and why an ambiguity
// is reported.
    let mut by_prose = 0usize;
    for r in records(&repo_root()) {
        let named: BTreeSet<&'static str> =
            r.prose.iter().filter_map(|a| at_record.get(a).copied()).collect();
        if named.is_empty() {
            continue;
        }
        if named.len() > 1 {
            ambiguous.insert(format!(
                "{} names tables of kinds {:?} in its prose",
                r.id,
                named.iter().copied().collect::<Vec<_>>(),
            ));
            continue;
        }
        by_prose += 1;
        let want = *named.iter().next().unwrap();
        if r.gesture != want {
            wrong.push(format!(
                "{}\n      docs/arms.json says `{}`\n      the kind byte at +0x0F of the \
                 widget record its own prose names says `{want}`",
                r.id, r.gesture,
            ));
        }
    }

    for r in records(&repo_root()) {
        let Some(addr) = r.addr else { continue };
        let Some(found) = kinds.get(&addr) else { continue };
        // **A handler in two tables at two kinds is settled by the record the
        // arm names**, when it names one and that record calls this handler.
        // `Opt_ToggleSpeech` and `Opt_ToggleAnimations` are each kind 5 in
        // their options table and kind 4 in the orphaned table at 0x004DDE08
        // that no call site tests; their records name the kind-5 record base,
        // and nothing about the address alone could have said which was meant.
        let named: BTreeSet<&'static str> = r
            .prose
            .iter()
            .filter(|va| handler_at.get(va) == Some(&addr))
            .filter_map(|va| at_record.get(va).copied())
            .collect();
        let found: BTreeSet<&'static str> =
            if found.len() > 1 && named.len() == 1 { named } else { found.clone() };
        if found.len() > 1 {
            ambiguous.insert(format!(
                "{addr:#010X} is in tables of kinds {:?}",
                found.iter().copied().collect::<Vec<_>>(),
            ));
            continue;
        }
        checked += 1;
        let want = *found.iter().next().unwrap();
        if r.gesture != want {
            wrong.push(format!(
                "{}\n      docs/arms.json says `{}`\n      the kind byte at +0x0F of \
                 {addr:#010X}'s widget record says `{want}`",
                r.id, r.gesture,
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "these arms are filed under a gesture the game's own widget tables contradict:\n  {}\n\
         The exe is right. `docs/input.md` has the kind vocabulary.",
        wrong.join("\n  "),
    );

    // **The coverage, asserted, because the interesting failure is outside the
    // set the check is exhaustive over.** If this number falls, the check went
// quiet, and a quiet check reads exactly like a passing
    // one.
    // 37 until the options panels' twelve rows and the orphaned table beside
    // them were recorded; two of those twelve are only classifiable at all
    // because the resolution above reads the record base their prose names.
    assert!(
        checked >= 54,
        "only {checked} arms have a `addr` this check can classify; it was 54. \
         Either records lost their `addr`, or the table regions moved.",
    );
    // The prose half's coverage, asserted for the same reason: it reaches the
    // four arms whose `addr` is `Screen_HandleInput`, and if it fell silent it
    // would read exactly like passing. `docs/input.md` §7a.
    assert!(
        by_prose >= 31,
        "only {by_prose} arms name a widget record in their prose; it was 31. \
         A record that stops naming its table stops being classifiable.",
    );
    // **One handler is reachable at two different kinds**, and it is named
    //
    // judge is itself the signal. `FUN_00432B05` is the multiplayer setup
    // page's four-row table (`0x004DCB48`): three of its records are kind 1 and
    // **the third is kind 3** — one row of one table waits for the release
    // while its neighbours fire on the press. Nothing about the address says
    // which row an arm means, so both of its arms go unjudged here.
    assert_eq!(
        ambiguous.iter().cloned().collect::<Vec<_>>(),
        vec!["0x00432B05 is in tables of kinds [\"left-press\", \"left-release\"]".to_string()],
        "the set of handlers reachable at two different kinds changed",
    );
}

