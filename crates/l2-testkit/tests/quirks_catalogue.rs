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
//! week — the code compiles either way.
//!
//! So this test **reads both files as text** and asserts they agree. It follows
//! `crates/l2-testkit/tests/census.rs`, including the habit worth copying: when
//! it fails it prints the corrected inventory, so accepting a deliberate change
//! is a paste.
//!
//! It reads the *source*, deliberately. A textual
//! join cannot be satisfied by code that compiles, and this crate's
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
//! generating from it the right answer: a check
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
//! *header* and `save::decode` refuses a mismatch, so a quirk there would
//!    invalidate every existing save and would frame a quirk as a rule.
//!    `docs/bugs.md` §6.3, `docs/decisions.md` C62.
//! 5. **The presentation table and its struct are the same list.** A field with
//!    no row cannot be shown on the quirks page and is joined to no catalogue
//!    entry; a row with no field names nothing.
//!
//! # What this cannot catch
//!
//! Stated here. It checks that a variant is
//! *named* in the simulation, not that the branch it guards is reachable or
//! correct — `crates/l2-kingdom/tests/quirks.rs` is what flips each one and
//! observes a different answer, and that is the check that pays.
//! It also cannot tell a wrong disposition from a right one: calling a
//! switchable bug `Unswitchable` silences it here, so the reason is
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
    ///
    Unwired(&'static str),
    /// A switch would be meaningless, harmful, or has nothing to switch. The
    /// reason is mandatory and is prose a reader can disagree with.
    Unswitchable(&'static str),
    /// The entry was withdrawn. It stays in the catalogue because the
/// retraction is the record; it has no switch.
    Retracted,
}

use Disposition::{Retracted, Switchable, Unswitchable, Unwired};
use Home::{Behavioural, Presentation};

/// **Which of the two homes a switch lives in**
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
///
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
/// be able to demand the row the day the entry lands,
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
    ("B7", Unwired("crates/l2-sim/src/ai/mod.rs — l2-sim takes no Quirks value yet")),
    ("B8", Unwired("crates/l2-sim/src/runner/mod.rs — l2-sim takes no Quirks value yet")),
    ("B9", Unwired("crates/l2-sim/src/ai/mod.rs — l2-sim takes no Quirks value yet")),
    // 2.2 — the county economy
    ("B10", Switchable(Behavioural)),
    ("B11", Retracted),
    ("B11a", Switchable(Behavioural)),
    ("B12", Switchable(Behavioural)),
    ("B13", Unwired("crates/l2-kingdom/src/levy.rs")),
    ("B14", Unwired("crates/l2-kingdom/src/industry/mod.rs")),
    ("B15", Switchable(Behavioural)),
    ("B16", Switchable(Behavioural)),
    ("B17", Switchable(Behavioural)),
    ("B18", Unwired("crates/l2-kingdom/src/land/mod.rs")),
    ("B98", Unwired("crates/l2-kingdom/src/land/mod.rs")),
    ("B19", Unwired("crates/l2-kingdom/src/land/mod.rs")),
    ("B20", Unwired("crates/l2-kingdom/src/land/mod.rs")),
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
        "B104",
        Unswitchable(
            "the notification model is the rule, not a variation of it: clearing the latch at the \
             roll destroys the letter and posting for every county is a different game",
        ),
    ),
    (
        "B22",
        Unswitchable("nothing observable turns on it — a county that sowed no seed grows none either way"),
    ),
    // 2.3 — the AI
    ("B23", Unswitchable("invisible: every reader tests `< 999`, so the stored 1000 changes nothing")),
    ("B24", Unwired("crates/l2-kingdom/src/ai/mod.rs")),
    ("B25", Unwired("crates/l2-kingdom/src/ai/mod.rs")),
    ("B26", Unwired("crates/l2-kingdom/src/ai/mod.rs")),
    ("B27", Unwired("crates/l2-kingdom/src/ai_farm/mod.rs")),
    ("B28", Unwired("crates/l2-kingdom/src/ai_farm/mod.rs")),
    ("B29", Unwired("crates/l2-kingdom/src/ai_farm/mod.rs")),
    ("B30", Unwired("crates/l2-kingdom/src/ai_farm/mod.rs")),
    ("B31", Unwired("crates/l2-kingdom/src/ai_farm/mod.rs")),
    ("B32", Unwired("crates/l2-sim/src/ai/mod.rs — l2-sim takes no Quirks value yet")),
    ("B33", Unwired("crates/l2-sim/src/ai/mod.rs — l2-sim takes no Quirks value yet")),
    ("B34", Unwired("crates/l2-sim/src/ai/mod.rs — l2-sim takes no Quirks value yet")),
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
    ("B39", Unwired("crates/l2-kingdom/src/movement/mod.rs")),
    ("B40", Unwired("crates/l2-kingdom/src/movement/mod.rs")),
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
    ("B48", Unwired("crates/l2-sim/src/runner/mod.rs")),
    ("B49", Unwired("crates/l2-sim/src/terrain.rs")),
    ("B50", Unwired("crates/l2-view/src/scene.rs")),
    ("B102", Unwired("crates/l2-sim/src/fire.rs — l2-sim takes no Quirks value yet")),
    // Numbered B69 until `corrections.js` learned to read `docs/bugs.md`: it
    // shared that number with the siege repair bill below, and this list held
    // one row for the two, so nothing here could see the collision.
    (
        "B100",
        Unswitchable(
            "not reproduced, deliberately: the original reads a byte one record past the figure \
             array, which we do not model — reproducing it would copy the address, not the behaviour",
        ),
    ),
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
    // Presentation — the first of its kind
    ("B64", Switchable(Presentation)),
    // Nothing to switch: both counters are stepped every frame and read by
// NOTHING in the whole binary. The
    // 21-state counter matching villani1.pl8 21 frames is recorded as a
    // coincidence and deliberately not built on.
    ("B65", Unswitchable("dead code: two counters no reader ever looks at")),
    ("B67", Unwired("l2_kingdom::industry, the castle materials round")),
    // Filed under bugs because it READS as one; it is the rule. A county
    // building a castle never puts a man on the walls while its mines run,
    // and AI_ChooseIndustry switching iron off is the only way anything
// finishes. Switching it would change the game.
    ("B68", Unswitchable("the rule, not a defect: see docs/rules.md")),
    // Castle_DrawStatusBlock's two table reads, one word low, reproduced in
    // screens::job::castle_word. Nothing to switch: the pair IS what the block
    // says about a castle that is not there.
    ("B105", Unswitchable("the block's whole statement about an absent castle; a corrected pair is a different screen")),
    ("B99", Unwired("crates/l2-game/src/turn_clock.rs")),
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
    // Reproduced, single-player arm only, because that is the only arm that
    // exists here: the network half of the split is a multiplayer behaviour and
    // docs/netcode.md is the one place the original is not the authority. Not
    // switchable -- turning it off would mean inventing the write-back the solo
// path does not do, which is a different game.
    ("B76", Unswitchable("single-player arm reproduced; the other arm is multiplayer")),
    // The two text-entry defects, both reproduced in `l2_game::text` and both
    // behavioural in the sense that they change what a saved game is called --
    // but neither reaches the simulation or the digest, because a filename is
// not state the rules read. Unwired: a switch for
    // either would change what the field CONTAINS, not which pixels show it.
    ("B79", Unwired("crates/l2-game/src/text.rs, the overwrite branch")),
    ("B101", Unswitchable("it changes which advice a player sees and when, never a number in the world")),
    ("B80", Unwired("crates/l2-game/src/text.rs, the End-key arm")),
    // Both from the siege battle, both reproduced and neither wired: a repeat
    // assault billing the same repair twice is a rule the original has, and
    // switching it would change what a siege costs; the drawbridge search
    // missing a break is a defect with no observable consequence, because the
    // scan finds the same cell either way.
    ("B84", Unwired("crates/l2-sim/src/runner/mod.rs, the repair bill")),
    ("B85", Unswitchable("no observable difference: the scan finds the same cell")),
    // The five diplomacy defects. Four are reproduced in l2_kingdom::diplomacy
    // or the screen and none is wired to a switch; the fifth cannot be
    // reproduced because the prompt it lives in is not built.
    ("B86", Unwired("crates/l2-kingdom/src/diplomacy.rs::ai_diplomacy")),
    ("B87", Unwired("crates/l2-kingdom/src/diplomacy.rs::reconcile_alliances")),
    ("B88", Unwired("crates/l2-kingdom/src/diplomacy.rs::reconcile_alliances")),
    ("B89", Unwired("crates/l2-game/src/screens/diplomacy.rs::refusal")),
    // Not reproduced and not switchable: the accept-alliance prompt is not
    // built, because Msg_DrawWindow window layouts have never been read.
    ("B90", Unswitchable("the prompt it lives in is not built")),
    // Summer's climate ladder skips band 3 and has an unreachable arm.
    // Reproduced literally with the hole named, and not wired: switching it
    // would change the weather every county gets, which is a rule.
    ("B92", Unwired("crates/l2-kingdom/src/weather.rs::local_modifier")),
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
    // A `mod.rs` stands for its whole directory: a split module keeps its text in siblings.
    if p.file_name().is_some_and(|f| f == "mod.rs") {
        let dir = p.parent().unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|q| q.extension().is_some_and(|x| x == "rs"))
            .collect();
        names.sort();
        return names.iter().map(|q| std::fs::read_to_string(q).unwrap().replace("\r\n", "\n")).collect::<Vec<_>>().join("\n");
    }
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
///
/// **A placeholder is an entry.** A branch cannot know the number its new row
/// will get, so it writes `B` + `NEW-` + a slug and the integrator assigns the
/// number at merge with `node tools/decisions/corrections.js --assign`. This
/// parser used to accept digits only:
/// the branch's own suite was green
/// the row was numbered, on `main`, in the integrator's hands. Now the row is
/// seen on the branch, `DISPOSITIONS` has to carry the placeholder, and
/// `--assign` renames it here in the same pass that renames the document.
fn catalogue(root: &Path) -> Vec<String> {
    catalogue_of(&read(root, "docs/bugs.md"))
}

/// [`catalogue`] over a document's text, so the parser can be asked about a
/// document built to test it.
fn catalogue_of(doc: &str) -> Vec<String> {
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

/// `B` then digits then an optional lower-case suffix: `B1`, `B11a`, `B63a` —
/// or `B` + `NEW-` + a slug, the placeholder a branch writes (see
/// [`catalogue`]). The slug is the one `corrections.js` recognises: word
/// characters in hyphen-separated runs,
/// part of it.
fn is_entry_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('B') else { return false };
    if let Some(slug) = rest.strip_prefix("NEW-") {
        return slug
            .split('-')
            .all(|run| !run.is_empty() && run.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let tail = &rest[digits.len()..];
    tail.is_empty() || (tail.len() == 1 && tail.chars().all(|c| c.is_ascii_lowercase()))
}

/// `Quirk::Name => "Bn",` out of `Quirk::entry`, as `(variant, entry)`.
///
/// Parsed from the source, so that the two lists are joined
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

/// `(field, docs/bugs.md entry)` out of `l2_game::game::PRESENTATION`
/// field list of `l2_game::game::Quirks`.
///
/// The presentation half of the switch list. Parsed from the source for the same
/// reason the behavioural half is: a textual join cannot be satisfied by code
/// that compiles.
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

/// **A placeholder row is a catalogue entry**,
/// does not say what its switch is goes red on its own branch.
///
/// The failure this exists for: a branch added a row as a placeholder, its
/// suite was green because the parser read digits only, and two tests went red
/// at merge the moment the row was numbered — so the branch's green was a false
/// signal, and every branch that adds a row would have hit it.
///
/// Ablated: restoring the digits-only `is_entry_id` fails this test, because
/// the three placeholder entries vanish from the parse; and with the real
/// catalogue, adding a placeholder heading to `docs/bugs.md` §2 makes
/// `every_bug_has_a_disposition…` fail with *"docs/bugs.md has BNEW-… and
/// DISPOSITIONS does not"*, where before this change it passed.
///
/// The placeholders are assembled at run time so this file does not itself
/// carry one — `corrections.js --check` would report it as unassigned.
#[test]
fn a_placeholder_row_is_an_entry_so_an_unwired_one_fails_on_its_own_branch() {
    let tag = |slug: &str| format!("{}NEW-{slug}", 'B');
    let doc = format!(
        "# 1. Before the catalogue\n\n### {early} — not in §2\n\n\
         # 2. The catalogue — the original's bugs, reproduced\n\n\
         ### B1 — a numbered heading\n\n\
         ### {heading} — a placeholder heading\n\n\
         | **B10** | a numbered row | — | here |\n\
         | **{row}** | a placeholder row | — | here |\n\
         | ~~**{struck}**~~ | a retracted placeholder row | — | here |\n\n\
         # 3. The original's bugs we do not reproduce\n\n### {late} — not in §2 either\n",
        early = tag("early"),
        heading = tag("heading"),
        row = tag("row"),
        struck = tag("struck"),
        late = tag("late"),
    );
    assert_eq!(
        catalogue_of(&doc),
        vec!["B1".to_string(), tag("heading"), "B10".to_string(), tag("row"), tag("struck")],
        "the catalogue parser does not see placeholder entries, so a branch's new bug row is \
         invisible to every test in this file until the integrator numbers it"
    );

    // The slug is corrections.js's: what it would not assign, this does not accept.
    for bad in ["NEW-", "NEW-trailing-", "NEW--lead", "NEW-two--hyphens", "NEW-sp ace"] {
        assert!(!is_entry_id(&format!("B{bad}")), "B{bad} is not a placeholder corrections.js recognises");
    }
    assert!(is_entry_id(&tag("herd-four")) && is_entry_id(&tag("group_87")));
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
                    // hat
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
///
/// added — and would frame a quirk as a *rule*, which it is not. It belongs on
/// `Options`, which is in the save *body* and therefore already inside the
/// per-tick lockstep digest. `docs/bugs.md` §6.3, `docs/decisions.md` C62.
#[test]
fn no_quirk_is_filed_under_tables_where_it_would_reach_the_save_header() {
    let root = repo_root();
    let tables = read(&root, "crates/l2-kingdom/src/tables/mod.rs");
    // Everything from `pub struct Tables` to the end of its `Encode` impl is
    // what the fingerprint covers.
    for needle in ["Quirk", "quirks"] {
        assert!(
            !tables.contains(needle),
            "crates/l2-kingdom/src/tables/mod.rs names `{needle}`. A quirk on `Tables` is hashed \
             into the save header (`ruleset_fingerprint`), so adding one invalidates every \
             existing save — and it frames a quirk as a rule. Put it on \
             `l2_kingdom::kingdom::Options::quirks`, which is in the save body and in the \
             per-tick digest. docs/bugs.md §6.3, docs/decisions.md C62."
        );
    }
    // And the other direction: `Options` really is the home,
    // cannot pass by the field having quietly gone away.
    let kingdom = read(&root, "crates/l2-kingdom/src/kingdom/mod.rs");
    assert!(
        kingdom.contains("pub quirks: l2_net::Quirks"),
        "l2_kingdom::kingdom::Options::quirks has moved. It is where the quirk set lives; \
         docs/decisions.md C62."
    );
    let save = read(&root, "crates/l2-kingdom/src/save/mod.rs");
    assert!(
        save.contains("out.encode(&self.options.quirks)"),
        "the quirk set is not encoded into the save body any more. That body IS the per-tick \
         lockstep digest (`save::checksum` is `Canonical::hash_of(kingdom)`), so dropping it \
         makes two peers with different quirks agree on a checksum while computing different \
         games. docs/netcode.md §6."
    );
}

/// **What this run asserted**, printed every time, in the census's
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
