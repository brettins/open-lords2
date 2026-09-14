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
//! correct — `crates/l2-kingdom/tests/quirks/main.rs` is what flips each one and
//! observes a different answer, and that is the check that pays.
//! It also cannot tell a wrong disposition from a right one: calling a
//! switchable bug `Unswitchable` silences it here, so the reason is
//! mandatory and is prose a reader can disagree with.

mod quirks;
pub use quirks::*;

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
    ("B6", Unwired("crates/l2-kingdom/src/units_tick/mod.rs")),
    ("B7", Unwired("crates/l2-sim/src/ai/mod.rs — l2-sim takes no Quirks value yet")),
    ("B8", Unwired("crates/l2-sim/src/runner/mod.rs — l2-sim takes no Quirks value yet")),
    ("B9", Unwired("crates/l2-sim/src/ai/mod.rs — l2-sim takes no Quirks value yet")),
    // 2.2 — the county economy
    ("B10", Switchable(Behavioural)),
    ("B11", Retracted),
    ("B11a", Switchable(Behavioural)),
    ("B12", Switchable(Behavioural)),
    ("B13", Unwired("crates/l2-kingdom/src/levy/mod.rs")),
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
    ("B41", Unwired("crates/l2-kingdom/src/units_tick/mod.rs")),
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
    ("B50", Unwired("crates/l2-view/src/scene/mod.rs")),
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
    ("B54", Unwired("crates/l2-kingdom/src/conquest/mod.rs and its caller")),
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
    ("B57", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    ("B58", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    ("B59", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    ("B60", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    ("B61", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    // 2.9 — sound
    (
        "B62",
        Unswitchable("not yet applicable: there is no post-battle fanfare, because there is no battle screen"),
    ),
    // 2.10 — screens and navigation
    (
        "B63",
        Unwired(
            "crates/l2-game/src/screen/mod.rs — one flag in `Machine::apply_at`, and then a pass \
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
    ("B86", Unwired("crates/l2-kingdom/src/diplomacy/mod.rs::ai_diplomacy")),
    ("B87", Unwired("crates/l2-kingdom/src/diplomacy/mod.rs::reconcile_alliances")),
    ("B88", Unwired("crates/l2-kingdom/src/diplomacy/mod.rs::reconcile_alliances")),
    ("B89", Unwired("crates/l2-game/src/screens/diplomacy/mod.rs::refusal")),
    // Not reproduced and not switchable: the accept-alliance prompt is not
    // built, because Msg_DrawWindow window layouts have never been read.
    ("B90", Unswitchable("the prompt it lives in is not built")),
    // Summer's climate ladder skips band 3 and has an unreachable arm.
    // Reproduced literally with the hole named, and not wired: switching it
    // would change the weather every county gets, which is a rule.
    ("B92", Unwired("crates/l2-kingdom/src/weather.rs::local_modifier")),
];

