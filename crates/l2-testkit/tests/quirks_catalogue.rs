//! **The toggle list is derived from `docs/bugs.md`, not written beside it.**
//!
//! # The problem
//!
//! `docs/bugs.md` §2 is the catalogue of the original's defects that we
//! reproduce on purpose, and `l2_net::Quirk` is the set a player can switch off.
//! Those are two lists of the same thing kept in two files, which is the shape
//! this project has been bitten by repeatedly: thirty-four documented figures
//! went stale at once, a comment in `ai.rs` stated an expired constraint and set
//! the AI's priority for weeks, and `l2-kingdom` cited a correction number that
//! had never been written. A hand-maintained toggle list would drift within a
//! week, and the drift would be invisible — the code compiles either way.
//!
//! So this test **reads both files as text** and asserts they agree. It follows
//! `crates/l2-testkit/tests/census.rs`, including the habit worth copying: when
//! it fails it prints the corrected inventory, so accepting a deliberate change
//! is a paste rather than an afternoon.
//!
//! It reads the *source* rather than linking `l2-net`, deliberately. A textual
//! join cannot be satisfied by code that merely compiles, and this crate's
//! dependency list stays as short as it is (`docs/environment.md`).
//!
//! # There are two switch lists, and that is why this is worth its length
//!
//! `docs/bugs.md` §6.3a splits quirks in two by a one-line test — *if flipping
//! it can change a number in a saved game it is behavioural; if it can only
//! change which pixels are painted from the same numbers it is presentation* —
//! and the two live in different places at very different prices:
//!
//! | | list | home | a new one costs |
//! |---|---|---|---|
//! | behavioural | `l2_net::Quirk` | `l2_kingdom::kingdom::Options` | a `save::VERSION` bump, a handshake field, a replay stamp |
//! | presentation | `l2_game::game::PRESENTATION` | `Assets` | one `bool` |
//!
//! **The catalogue is the only artefact that spans both**, which is what makes
//! generating from it the right answer rather than merely a tidy one: a check
//! driven by either enum would silently omit the other half.
//!
//! # The five rules
//!
//! 1. **Every behavioural entry in `docs/bugs.md` §2 appears here exactly once,
//!    with a disposition.** Add a bug to the catalogue and this goes red until
//!    somebody says what its switch is.
//! 2. **`Switchable(home)` ⇔ a switch in that home, and in no other.** A switch
//!    with no catalogue entry fails; an entry marked switchable with no switch
//!    fails; and an entry implemented in *both* homes fails. That last one is
//!    the assertion the split created the need for — the price asymmetry above
//!    means a rule variation will be tempted onto `Assets`, and it would be
//!    invisible until a multiplayer desync.
//! 3. **A `Quirk` variant must be read by the simulation.** `docs/agents.md`: *a
//!    quirk switch that nothing reads is worse than no switch*, because a
//!    checkbox claims a behaviour is configurable. So every variant has to be
//!    named somewhere outside its own definition and outside a test.
//! 4. **No quirk may be filed under `Tables`.** `Tables` is hashed into the save
//!    *header* and `save::decode` refuses a mismatch, so a quirk there would
//!    invalidate every existing save and would frame a quirk as a rule.
//!    `docs/bugs.md` §6.3, `docs/decisions.md` C62.
//! 5. **The presentation table and its struct are the same list.** A field with
//!    no row cannot be shown on the quirks page and is joined to no catalogue
//!    entry; a row with no field names nothing.
//!
//! # What this cannot catch
//!
//! Stated here rather than discovered later. It checks that a variant is
//! *named* in the simulation, not that the branch it guards is reachable or
//! correct — `crates/l2-kingdom/tests/quirks.rs` is what flips each one and
//! observes a different answer, and that is the check that actually pays.
//! It also cannot tell a wrong disposition from a right one: calling a
//! switchable bug `Unswitchable` silences it here, which is why the reason is
//! mandatory and is prose a reader can disagree with.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a catalogue entry's switch is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Disposition {
    /// A switch exists, in the home named. `Behavioural` means an
    /// `l2_net::Quirk` variant that the simulation reads and
    /// `l2-kingdom/tests/quirks.rs` flips; `Presentation` means a field of
    /// `l2_game::game::Quirks` named in `l2_game::game::PRESENTATION`.
    Switchable(Home),
    /// Reproduced in our code, switchable at a reasonable price, **and nobody
    /// has done it**. The honest middle: not impossible, just not done. The
    /// string says where the code is, so the next person starts from a path
    /// rather than from the catalogue.
    Unwired(&'static str),
    /// A switch would be meaningless, harmful, or has nothing to switch. The
    /// reason is mandatory and is prose a reader can disagree with.
    Unswitchable(&'static str),
    /// The entry was withdrawn. It stays in the catalogue because the
    /// retraction is the record; it is not a bug and has no switch.
    Retracted,
}

use Disposition::{Retracted, Switchable, Unswitchable, Unwired};
use Home::{Behavioural, Presentation};

/// **Which of the two homes a switch lives in**, and the assertion that keeps a
/// quirk from drifting between them.
///
/// `docs/bugs.md` §6.3a: *if flipping it can change a number in a saved game it
/// is behavioural; if it can only change which pixels are painted from the same
/// numbers it is presentation.* The two cost wildly different amounts — a
/// behavioural quirk bumps `save::VERSION`, goes into the lockstep digest and
/// has to be agreed before the first tick; a presentation quirk is one `bool` —
/// and **that asymmetry is the danger**. A rule variation filed on `Assets`
/// because it is cheaper there would be invisible until a multiplayer desync.
///
/// So the home is written down here, beside the catalogue entry, and the check
/// below fails if the implementation is anywhere else — including if it is in
/// *both*. Moving a quirk between homes is then a visible edit to this file
/// rather than a silent one to a struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Home {
    /// `l2_net::Quirk`, on `l2_kingdom::kingdom::Options`. In the save body and
    /// the per-tick digest.
    Behavioural,
    /// A field of `l2_game::game::Quirks`, on `Assets`. Cannot reach the
    /// simulation at all — `l2-game` is above every crate that computes a turn.
    ///
    /// **Unused on `main` today**, because the first presentation quirk (the
    /// county name's parchment emboss, `docs/bugs.md` B64) is on another
    /// branch. The variant exists ahead of it deliberately: the check has to
    /// be able to demand the row the day the entry lands, rather than being
    /// taught about the second home after the first one has already drifted.
    #[allow(dead_code)]
    Presentation,
}

/// **The inventory. Update it deliberately.**
///
/// One row per behavioural entry in `docs/bugs.md` §2, in catalogue order.
const DISPOSITIONS: &[(&str, Disposition)] = &[
    // 2.1 — the nine that change play the most
    ("B1", Switchable(Behavioural)),
    ("B2", Switchable(Behavioural)),
    ("B3", Switchable(Behavioural)),
    ("B4", Switchable(Behavioural)),
    (
        "B5",
        Unswitchable(
            "the pathfinder's stale counters make one search's result depend on which \
             searches ran before it, so a per-entry toggle has no meaning; bugs.md §6.4",
        ),
    ),
    ("B6", Unwired("crates/l2-kingdom/src/units_tick.rs")),
    ("B7", Unwired("crates/l2-sim/src/ai.rs — l2-sim takes no Quirks value yet")),
    ("B8", Unwired("crates/l2-sim/src/runner.rs — l2-sim takes no Quirks value yet")),
    ("B9", Unwired("crates/l2-sim/src/ai.rs — l2-sim takes no Quirks value yet")),
    // 2.2 — the county economy
    ("B10", Switchable(Behavioural)),
    ("B11", Retracted),
    ("B11a", Switchable(Behavioural)),
    ("B12", Switchable(Behavioural)),
    ("B13", Unwired("crates/l2-kingdom/src/levy.rs")),
    ("B14", Unwired("crates/l2-kingdom/src/industry.rs")),
    ("B15", Switchable(Behavioural)),
    ("B16", Switchable(Behavioural)),
    ("B17", Switchable(Behavioural)),
    ("B18", Unwired("crates/l2-kingdom/src/land.rs")),
    ("B19", Unwired("crates/l2-kingdom/src/land.rs")),
    ("B20", Unwired("crates/l2-kingdom/src/land.rs")),
    (
        "B21",
        Unswitchable("invisible: nothing a player can see turns on it; bugs.md §6.4"),
    ),
    (
        "B21a",
        Unswitchable(
            "the overrun is benign and arguably useful, and the ten words are asserted \
             against the user's own Lords2.exe; a switch would make that assertion conditional",
        ),
    ),
    (
        "B22",
        Unswitchable("nothing observable turns on it — a county that sowed no seed grows none either way"),
    ),
    // 2.3 — the AI
    ("B23", Unswitchable("invisible: every reader tests `< 999`, so the stored 1000 changes nothing")),
    ("B24", Unwired("crates/l2-kingdom/src/ai.rs")),
    ("B25", Unwired("crates/l2-kingdom/src/ai.rs")),
    ("B26", Unwired("crates/l2-kingdom/src/ai.rs")),
    ("B27", Unwired("crates/l2-kingdom/src/ai_farm.rs")),
    ("B28", Unwired("crates/l2-kingdom/src/ai_farm.rs")),
    ("B29", Unwired("crates/l2-kingdom/src/ai_farm.rs")),
    ("B30", Unwired("crates/l2-kingdom/src/ai_farm.rs")),
    ("B31", Unwired("crates/l2-kingdom/src/ai_farm.rs")),
    ("B32", Unwired("crates/l2-sim/src/ai.rs — l2-sim takes no Quirks value yet")),
    ("B33", Unwired("crates/l2-sim/src/ai.rs — l2-sim takes no Quirks value yet")),
    ("B34", Unwired("crates/l2-sim/src/ai.rs — l2-sim takes no Quirks value yet")),
    ("B35", Unwired("crates/l2-sim — the two siege attack scripts")),
    // 2.4 — things that move
    (
        "B36",
        Unswitchable("the pathfinder: fixing it changes every path in every battle and invalidates every recorded replay; bugs.md §6.4"),
    ),
    (
        "B37",
        Unswitchable("the pathfinder: see B36"),
    ),
    (
        "B38",
        Unswitchable("the campaign flood fill: see B36, and its cursors are shared between both distance fields"),
    ),
    ("B39", Unwired("crates/l2-kingdom/src/movement.rs")),
    ("B40", Unwired("crates/l2-kingdom/src/movement.rs")),
    ("B41", Unwired("crates/l2-kingdom/src/units_tick.rs")),
    ("B42", Switchable(Behavioural)),
    ("B43", Unwired("the merchant route script and crates/l2-kingdom/src/merchant.rs")),
    (
        "B44",
        Unswitchable("not reproduced: the campaign hover is traced and the UI arm does not exist yet"),
    ),
    // 2.5 — the battle
    ("B45", Unwired("crates/l2-sim/src/unit.rs — l2-sim takes no Quirks value yet")),
    ("B46", Unwired("crates/l2-sim/src/formation.rs")),
    ("B47", Unwired("crates/l2-sim/src/formation.rs")),
    ("B48", Unwired("crates/l2-sim/src/runner.rs")),
    ("B49", Unwired("crates/l2-sim/src/terrain.rs")),
    ("B50", Unwired("crates/l2-view/src/scene.rs")),
    // 2.6 — victory, defeat and the score
    ("B51", Switchable(Behavioural)),
    ("B52", Switchable(Behavioural)),
    ("B53", Switchable(Behavioural)),
    ("B54", Unwired("crates/l2-kingdom/src/conquest.rs and its caller")),
    ("B55", Unswitchable("invisible: a wrapping subtraction on a message variant; bugs.md §6.4")),
    (
        "B55a",
        Unswitchable(
            "not two behaviours: the original cannot save the option and ours can. \
             That is a declared divergence in our save format (decisions.md D11), not a quirk",
        ),
    ),
    (
        "B66",
        Unswitchable(
            "nothing to switch: storing the same constant into g_optSpeech twice is storing \n             it once, and the store that apparently lost its target is a guess, not a finding",
        ),
    ),
    // 2.7 — multiplayer
    (
        "B56",
        Unswitchable(
            "catalogue-only, and CLAUDE.md settles it: reproducing a defect in the mechanism \
             that DETECTS divergence buys nothing a player can see and costs the ability to \
             find real desyncs",
        ),
    ),
    // 2.8 — sieges
    ("B57", Unwired("crates/l2-kingdom/src/siege.rs")),
    ("B58", Unwired("crates/l2-kingdom/src/siege.rs")),
    ("B59", Unwired("crates/l2-kingdom/src/siege.rs")),
    ("B60", Unwired("crates/l2-kingdom/src/siege.rs")),
    ("B61", Unwired("crates/l2-kingdom/src/siege.rs")),
    // 2.9 — sound
    (
        "B62",
        Unswitchable("not yet applicable: there is no post-battle fanfare, because there is no battle screen"),
    ),
    // 2.10 — screens and navigation
    (
        "B63",
        Unwired(
            "crates/l2-game/src/screen.rs — one flag in `Machine::apply_at`, and then a pass \
             over 49 of the original's arms, which is the part that is not one flag",
        ),
    ),
    (
        "B63a",
        Unswitchable(
            "the original's own asymmetry between its three village screen ids; a switch here \
             would be a switch on uniformity, which nothing asks for",
        ),
    ),
    // Presentation — the first of its kind, and the reason `PRESENTATION` exists.
    ("B64", Switchable(Presentation)),
    // Nothing to switch: both counters are stepped every frame and read by
    // NOTHING in the whole binary, so there is no behaviour to turn off. The
    // 21-state counter matching villani1.pl8 21 frames is recorded as a
    // coincidence and deliberately not built on.
    ("B65", Unswitchable("dead code: two counters no reader ever looks at")),
    ("B67", Unwired("l2_kingdom::industry, the castle materials round")),
    // Filed under bugs because it READS as one; it is the rule. A county
    // building a castle never puts a man on the walls while its mines run,
    // and AI_ChooseIndustry switching iron off is the only way anything
    // finishes. Switching it would change the game rather than fix it.
    ("B68", Unswitchable("the rule, not a defect: see docs/rules.md")),
    ("B69", Unwired("l2_kingdom::siege, the repair bill material")),
    // The six the AI branch reproduced. All are behavioural and none is wired
    // to a switch yet: each is a defect of the original the AI now reproduces,
    // named in `ai_army.rs` at the site. B73 is the exception in kind -- a call
    // whose result is discarded and whose two arms are the same code, so there
    // is nothing to reproduce and nothing to switch.
    ("B70", Unwired("l2_kingdom::ai_army::pick_threat")),
    ("B71", Unwired("l2_kingdom::ai_army::mission_join_garrison")),
    ("B72", Unwired("l2_kingdom::ai_army::mission_seek_enemy")),
    ("B73", Unswitchable("a discarded result and two identical arms: nothing to reproduce")),
    ("B74", Unwired("l2_kingdom::ai_army::action_allowed")),
    ("B75", Unwired("l2_kingdom::ai_army::aim_tile")),
];

/// How many rows `l2_game::game::PRESENTATION` has.
fn presentation_rows(root: &Path) -> usize {
    presentation(root).0.len()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn read(root: &Path, rel: &str) -> String {
    let p = root.join(rel);
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("{} is part of this check and could not be read: {e}", p.display()))
        // `.gitattributes` pins line endings per extension; a check that measures
        // text must be a property of the text, not of the checkout.
        .replace("\r\n", "\n")
}

/// Every behavioural entry id in `docs/bugs.md` §2, in the order the document
/// gives them.
///
/// Two shapes carry an entry: a `### B1 — …` heading in §2.1 and a `| **B10** |`
/// table row everywhere else. A retracted row wears strikethrough,
/// `| ~~**B11**~~ |`, and is still an entry.
///
/// **Scoped to §2 on purpose.** §6.2 quotes B1, B2 and B4 again in a table of
/// what the ruleset can express, and an unscoped scan counts them twice — which
/// it did, on the first run of this function.
fn catalogue(root: &Path) -> Vec<String> {
    let doc = read(root, "docs/bugs.md");
    let mut out = Vec::new();
    let mut inside = false;
    for line in doc.lines() {
        if line.starts_with("# 2. The catalogue") {
            inside = true;
            continue;
        }
        if inside && line.starts_with("# 3.") {
            break;
        }
        if !inside {
            continue;
        }
        let id = if let Some(rest) = line.strip_prefix("### ") {
            rest.split_whitespace().next().unwrap_or("")
        } else if let Some(rest) = line.strip_prefix("| **") {
            rest.split("**").next().unwrap_or("")
        } else if let Some(rest) = line.strip_prefix("| ~~**") {
            rest.split("**").next().unwrap_or("")
        } else {
            continue;
        };
        if is_entry_id(id) {
            out.push(id.to_string());
        }
    }
    out
}

/// `B` then digits then an optional lower-case suffix: `B1`, `B11a`, `B63a`.
fn is_entry_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('B') else { return false };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let tail = &rest[digits.len()..];
    tail.is_empty() || (tail.len() == 1 && tail.chars().all(|c| c.is_ascii_lowercase()))
}

/// `Quirk::Name => "Bn",` out of `Quirk::entry`, as `(variant, entry)`.
///
/// Parsed from the source rather than linked, so that the two lists are joined
/// by *text* — code that compiles is not evidence that two documents agree.
fn quirk_variants(root: &Path) -> Vec<(String, String)> {
    let src = read(root, "crates/l2-net/src/quirks.rs");
    let body = src
        .split("pub const fn entry(self) -> &'static str {")
        .nth(1)
        .expect("l2_net::Quirk::entry is what this check joins on; it has moved or been renamed");
    let body = body.split("\n    }").next().unwrap_or(body);
    let mut out = Vec::new();
    for arm in body.split("Quirk::").skip(1) {
        let variant: String =
            arm.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let Some(rest) = arm.split_once("=> ") else { continue };
        let entry: String = rest.1.trim_start().trim_start_matches('"').chars().take_while(|c| *c != '"').collect();
        if !variant.is_empty() && is_entry_id(&entry) {
            out.push((variant, entry));
        }
    }
    out
}

/// Every `.rs` under `crates/`, with its repo-relative path.
fn sources(root: &Path) -> Vec<(String, String)> {
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
    let mut paths = Vec::new();
    walk(&root.join("crates"), &mut paths);
    paths
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&p).unwrap_or_default().replace("\r\n", "\n");
            (rel, text)
        })
        .collect()
}

/// `(field, docs/bugs.md entry)` out of `l2_game::game::PRESENTATION`, and the
/// field list of `l2_game::game::Quirks`.
///
/// The presentation half of the switch list. Parsed from the source for the same
/// reason the behavioural half is: a textual join cannot be satisfied by code
/// that merely compiles.
fn presentation(root: &Path) -> (Vec<(String, String)>, Vec<String>) {
    let src = read(root, "crates/l2-game/src/game.rs");

    let table = src
        .split("pub const PRESENTATION: &[(&str, &str)] = &[")
        .nth(1)
        .expect("l2_game::game::PRESENTATION is what this check joins on; it has moved");
    let table = table.split("];").next().unwrap_or("");
    let mut rows = Vec::new();
    for row in table.split('(').skip(1) {
        let quoted: Vec<&str> = row.split('"').skip(1).step_by(2).collect();
        if quoted.len() >= 2 {
            rows.push((quoted[0].to_string(), quoted[1].to_string()));
        }
    }

    let body = src
        .split("pub struct Quirks {")
        .nth(1)
        .expect("l2_game::game::Quirks is what this check joins on; it has moved");
    let body = body.split("\n}").next().unwrap_or("");
    let fields: Vec<String> = body
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split(':').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    (rows, fields)
}

/// Rules 1 and 2: the catalogue and the inventory are the same list, every
/// switch names a real entry, **and every switch is in the home the catalogue
/// says it is in**.
///
/// That last clause is the one the split created the need for. A behavioural
/// quirk filed on `Assets` because a `bool` is cheaper than a `save::VERSION`
/// bump would be invisible until a multiplayer desync; a presentation quirk
/// filed on `Options` would put a text-shadow colour in the state two peers
/// have to agree on before the first tick. Both fail here, and so does a quirk
/// implemented in *both* homes at once.
#[test]
fn every_bug_has_a_disposition_every_switch_has_a_bug_and_every_switch_is_in_its_own_home() {
    let root = repo_root();
    let catalogue = catalogue(&root);
    assert!(
        catalogue.len() > 50,
        "only {} entries found in docs/bugs.md §2 — the section's shape has changed and this \
         check is now reading nothing. Fix the parser, do not lower the bound.",
        catalogue.len()
    );

    let inventory: BTreeMap<&str, Disposition> = DISPOSITIONS.iter().copied().collect();
    assert_eq!(inventory.len(), DISPOSITIONS.len(), "an entry id appears twice in DISPOSITIONS");

    let variants = quirk_variants(&root);
    let behavioural: BTreeMap<&str, &str> =
        variants.iter().map(|(v, e)| (e.as_str(), v.as_str())).collect();
    let (rows, fields) = presentation(&root);
    let presentational: BTreeMap<&str, &str> =
        rows.iter().map(|(f, e)| (e.as_str(), f.as_str())).collect();

    let mut problems: Vec<String> = Vec::new();

    // The one that matters most: a quirk cannot be in both homes.
    for entry in behavioural.keys() {
        if presentational.contains_key(entry) {
            problems.push(format!(
                "  {entry} is implemented in BOTH homes — Quirk::{} on Options and the field \
                 `{}` on Assets. One of them is wrong and the wrong one is probably the cheap \
                 one; docs/bugs.md §6.3a has the test.",
                behavioural[entry], presentational[entry]
            ));
        }
    }

    for id in &catalogue {
        let id = id.as_str();
        let in_b = behavioural.contains_key(id);
        let in_p = presentational.contains_key(id);
        match inventory.get(id) {
            None => problems.push(format!(
                "  docs/bugs.md has {id} and DISPOSITIONS does not. Say what its switch is, and \
                 which home it is in."
            )),
            Some(Switchable(Behavioural)) if !in_b => problems.push(format!(
                "  {id} is marked Switchable(Behavioural) and no l2_net::Quirk variant names it.{}",
                if in_p { " It is on Assets instead — that is the wrong home for a rule." } else { "" }
            )),
            Some(Switchable(Presentation)) if !in_p => problems.push(format!(
                "  {id} is marked Switchable(Presentation) and no row of \
                 l2_game::game::PRESENTATION names it.{}",
                if in_b {
                    " It is on Options instead — that costs a save bump and a handshake field \
                     for something that only changes pixels."
                } else {
                    ""
                }
            )),
            Some(d) if !matches!(d, Switchable(_)) && in_b => problems.push(format!(
                "  {id} has the switch Quirk::{} and is not marked Switchable.",
                behavioural[id]
            )),
            Some(d) if !matches!(d, Switchable(_)) && in_p => problems.push(format!(
                "  {id} has the presentation switch `{}` and is not marked Switchable.",
                presentational[id]
            )),
            _ => {}
        }
    }
    for (id, _) in DISPOSITIONS {
        if !catalogue.iter().any(|c| c == id) {
            problems.push(format!(
                "  DISPOSITIONS has {id} and docs/bugs.md §2 does not. A switch for a bug that \
                 is not in the catalogue is a switch nobody can look up."
            ));
        }
    }
    for (variant, entry) in &variants {
        if !catalogue.iter().any(|c| c == entry) {
            problems.push(format!(
                "  Quirk::{variant} claims docs/bugs.md entry {entry}, which is not in §2."
            ));
        }
    }
    // The presentation half's own two-way join: table against struct, and table
    // against catalogue. This is the shape that would have caught a field whose
    // doc cited an entry that does not exist.
    for (field, entry) in &rows {
        if !fields.iter().any(|f| f == field) {
            problems.push(format!(
                "  PRESENTATION names `{field}`, which is not a field of l2_game::game::Quirks."
            ));
        }
        if !catalogue.iter().any(|c| c == entry) {
            problems.push(format!(
                "  PRESENTATION's `{field}` claims docs/bugs.md entry {entry}, which is not in §2."
            ));
        }
    }
    for field in &fields {
        if !rows.iter().any(|(f, _)| f == field) {
            problems.push(format!(
                "  l2_game::game::Quirks has the field `{field}` and PRESENTATION does not name \
                 it, so the quirks page cannot show it and nothing joins it to a catalogue entry."
            ));
        }
    }

    if !problems.is_empty() {
        let mut lines = String::new();
        for id in &catalogue {
            let id = id.as_str();
            let d = match (
                inventory.get(id),
                behavioural.contains_key(id),
                presentational.contains_key(id),
            ) {
                (_, true, _) => "Switchable(Behavioural)".to_string(),
                (_, _, true) => "Switchable(Presentation)".to_string(),
                (Some(Retracted), _, _) => "Retracted".to_string(),
                (Some(Unwired(w)), _, _) => format!("Unwired({w:?})"),
                (Some(Unswitchable(w)), _, _) => format!("Unswitchable({w:?})"),
                (Some(Switchable(_)), _, _) => "Unwired(\"SAY WHERE\")".to_string(),
                (None, _, _) => {
                    "Unwired(\"SAY WHERE, or Unswitchable(\\\"why\\\")\")".to_string()
                }
            };
            lines.push_str(&format!("    ({id:?}, {d}),\n"));
        }
        panic!(
            "the quirk catalogue and the switch lists disagree.\n\n{}\n\n\
             docs/bugs.md §2 is the catalogue; l2_net::Quirk is the behavioural switch list and \
             l2_game::game::PRESENTATION the presentation one. A hand-kept copy of any of them \
             beside another drifts. If the change is intended, replace DISPOSITIONS in \
             crates/l2-testkit/tests/quirks_catalogue.rs with:\n\n\
             const DISPOSITIONS: &[(&str, Disposition)] = &[\n{}];\n",
            problems.join("\n"),
            lines
        );
    }
}

/// Rule 3: **a switch that nothing reads is worse than no switch.**
///
/// Every `Quirk` variant has to be named somewhere in the workspace that is
/// neither its own definition nor a test — that is, by the simulation. A
/// checkbox for a flag no rule consults claims a behaviour is configurable when
/// it is not, and `docs/agents.md` records four separate occasions where a field
/// nothing wrote sat behind a green suite.
#[test]
fn every_switch_is_read_by_the_simulation() {
    let root = repo_root();
    let variants = quirk_variants(&root);
    assert!(!variants.is_empty(), "no Quirk variants parsed at all");
    let files = sources(&root);

    let mut inert = Vec::new();
    for (variant, entry) in &variants {
        let needle = format!("Quirk::{variant}");
        let read_by: Vec<&str> = files
            .iter()
            .filter(|(path, text)| {
                path != "crates/l2-net/src/quirks.rs"
                    && !path.contains("/tests/")
                    && text.contains(&needle)
                    // Only the *reading* side counts. A `match` arm inside a
                    // name or summary table is the definition wearing another
                    // hat, and the definition lives in one file anyway.
                    && text.contains("reproduces(")
            })
            .map(|(path, _)| path.as_str())
            .collect();
        if read_by.is_empty() {
            inert.push(format!(
                "  Quirk::{variant} ({entry}) is named by no simulation file that calls \
                 `reproduces(`. It is a checkbox with nothing behind it."
            ));
        }
    }
    assert!(
        inert.is_empty(),
        "{} quirk(s) are inert:\n{}\n\nEither wire the switch or take the variant out and mark \
         the entry Unwired in crates/l2-testkit/tests/quirks_catalogue.rs. Do not ship a \
         checkbox that does nothing.",
        inert.len(),
        inert.join("\n")
    );
}

/// Rule 4: **no quirk reaches `Tables`.**
///
/// `l2_kingdom::save::ruleset_fingerprint` hashes `Tables` into the save
/// *header* and `decode` refuses a save whose supplied tables hash differently,
/// so a quirk field there would invalidate every existing save on the day it was
/// added — and would frame a quirk as a *rule*, which it is not. It belongs on
/// `Options`, which is in the save *body* and therefore already inside the
/// per-tick lockstep digest. `docs/bugs.md` §6.3, `docs/decisions.md` C62.
#[test]
fn no_quirk_is_filed_under_tables_where_it_would_reach_the_save_header() {
    let root = repo_root();
    let tables = read(&root, "crates/l2-kingdom/src/tables.rs");
    // Everything from `pub struct Tables` to the end of its `Encode` impl is
    // what the fingerprint covers.
    for needle in ["Quirk", "quirks"] {
        assert!(
            !tables.contains(needle),
            "crates/l2-kingdom/src/tables.rs names `{needle}`. A quirk on `Tables` is hashed \
             into the save header (`ruleset_fingerprint`), so adding one invalidates every \
             existing save — and it frames a quirk as a rule. Put it on \
             `l2_kingdom::kingdom::Options::quirks`, which is in the save body and in the \
             per-tick digest. docs/bugs.md §6.3, docs/decisions.md C62."
        );
    }
    // And the other direction: `Options` really is the home, so that this test
    // cannot pass by the field having quietly gone away.
    let kingdom = read(&root, "crates/l2-kingdom/src/kingdom.rs");
    assert!(
        kingdom.contains("pub quirks: l2_net::Quirks"),
        "l2_kingdom::kingdom::Options::quirks has moved. It is where the quirk set lives; \
         docs/decisions.md C62."
    );
    let save = read(&root, "crates/l2-kingdom/src/save.rs");
    assert!(
        save.contains("out.encode(&self.options.quirks)"),
        "the quirk set is not encoded into the save body any more. That body IS the per-tick \
         lockstep digest (`save::checksum` is `Canonical::hash_of(kingdom)`), so dropping it \
         makes two peers with different quirks agree on a checksum while computing different \
         games. docs/netcode.md §6."
    );
}

/// **What this run actually asserted**, printed every time, in the census's
/// habit: a number that nobody looks at is a number that drifts.
#[test]
fn the_switchable_count_is_reported() {
    let root = repo_root();
    let catalogue = catalogue(&root);
    let inventory: BTreeMap<&str, Disposition> = DISPOSITIONS.iter().copied().collect();

    let count = |f: fn(&Disposition) -> bool| {
        catalogue.iter().filter(|id| inventory.get(id.as_str()).is_some_and(f)).count()
    };
    let behavioural = count(|d| matches!(d, Switchable(Behavioural)));
    let presentation = count(|d| matches!(d, Switchable(Presentation)));
    let switchable = behavioural + presentation;
    let unwired = count(|d| matches!(d, Unwired(_)));
    let unswitchable = count(|d| matches!(d, Unswitchable(_)));
    let retracted = count(|d| matches!(d, Retracted));

    eprintln!(
        "quirk catalogue: {} entries in docs/bugs.md §2 — {switchable} switchable \
         ({behavioural} behavioural, {presentation} presentation), {unwired} unwired, \
         {unswitchable} unswitchable, {retracted} retracted",
        catalogue.len()
    );
    assert_eq!(
        switchable + unwired + unswitchable + retracted,
        catalogue.len(),
        "an entry fell out of every bucket"
    );
    assert_eq!(
        behavioural,
        quirk_variants(&root).len(),
        "Switchable(Behavioural) rows and l2_net::Quirk variants must be the same number"
    );
    assert_eq!(
        presentation,
        presentation_rows(&root),
        "Switchable(Presentation) rows and l2_game::game::PRESENTATION rows must be the same number"
    );
}
