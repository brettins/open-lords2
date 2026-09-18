//! 1. **Every behavioural entry in `docs/bugs.md` §2 appears here exactly once,
//!    with a disposition.** Add a bug to the catalogue and this goes red until
//!    somebody says what its switch is.
//!
//!    `docs/bugs.md` §6.3, `docs/decisions.md` C62.

mod quirks;
pub use quirks::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Disposition {
    Switchable(Home),
    Unwired(&'static str),
    Unswitchable(&'static str),
    Retracted,
}

use Disposition::{Retracted, Switchable, Unswitchable, Unwired};
use Home::{Behavioural, Presentation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Home {
    Behavioural,
    #[allow(dead_code)]
    Presentation,
}

const DISPOSITIONS: &[(&str, Disposition)] = &[
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
    ("B43", Unwired("the merchant route script and crates/l2-kingdom/src/merchant/mod.rs")),
    (
        "B44",
        Unswitchable("not reproduced: the campaign hover is traced and the UI arm does not exist yet"),
    ),
    ("B45", Unwired("crates/l2-sim/src/unit/mod.rs — l2-sim takes no Quirks value yet")),
    ("B46", Unwired("crates/l2-sim/src/formation.rs")),
    ("B47", Unwired("crates/l2-sim/src/formation.rs")),
    ("B48", Unwired("crates/l2-sim/src/runner/mod.rs")),
    ("B49", Unwired("crates/l2-sim/src/terrain/mod.rs")),
    ("B50", Unwired("crates/l2-view/src/scene/mod.rs")),
    ("B102", Unwired("crates/l2-sim/src/fire/mod.rs — l2-sim takes no Quirks value yet")),
    (
        "B100",
        Unswitchable(
            "not reproduced, deliberately: the original reads a byte one record past the figure \
             array, which we do not model — reproducing it would copy the address, not the behaviour",
        ),
    ),
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
    (
        "B56",
        Unswitchable(
            "catalogue-only, and CLAUDE.md settles it: reproducing a defect in the mechanism \
             that DETECTS divergence buys nothing a player can see and costs the ability to \
             find real desyncs",
        ),
    ),
    ("B57", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    ("B58", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    ("B59", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    ("B60", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    ("B61", Unwired("crates/l2-kingdom/src/siege/mod.rs")),
    (
        "B62",
        Unswitchable("not yet applicable: there is no post-battle fanfare, because there is no battle screen"),
    ),
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
    ("B64", Switchable(Presentation)),
    ("B65", Unswitchable("dead code: two counters no reader ever looks at")),
    ("B67", Unwired("l2_kingdom::industry, the castle materials round")),
    ("B68", Unswitchable("the rule, not a defect: see docs/rules.md")),
    ("B105", Unswitchable("the block's whole statement about an absent castle; a corrected pair is a different screen")),
    ("B99", Unwired("crates/l2-game/src/turn_clock/mod.rs")),
    ("B69", Unwired("l2_kingdom::siege, the repair bill material")),
    ("B70", Unwired("l2_kingdom::ai_army::pick_threat")),
    ("B71", Unwired("l2_kingdom::ai_army::mission_join_garrison")),
    ("B72", Unwired("l2_kingdom::ai_army::mission_seek_enemy")),
    ("B73", Unswitchable("a discarded result and two identical arms: nothing to reproduce")),
    ("B74", Unwired("l2_kingdom::ai_army::action_allowed")),
    ("B75", Unwired("l2_kingdom::ai_army::aim_tile")),
    ("B76", Unswitchable("single-player arm reproduced; the other arm is multiplayer")),
    ("B79", Unwired("crates/l2-game/src/text/mod.rs, the overwrite branch")),
    ("B101", Unswitchable("it changes which advice a player sees and when, never a number in the world")),
    ("B80", Unwired("crates/l2-game/src/text/mod.rs, the End-key arm")),
    ("B84", Unwired("crates/l2-sim/src/runner/mod.rs, the repair bill")),
    ("B85", Unswitchable("no observable difference: the scan finds the same cell")),
    ("B86", Unwired("crates/l2-kingdom/src/diplomacy/mod.rs::ai_diplomacy")),
    ("B87", Unwired("crates/l2-kingdom/src/diplomacy/mod.rs::reconcile_alliances")),
    ("B88", Unwired("crates/l2-kingdom/src/diplomacy/mod.rs::reconcile_alliances")),
    ("B89", Unwired("crates/l2-game/src/screens/diplomacy/mod.rs::refusal")),
    ("B90", Unswitchable("the prompt it lives in is not built")),
    ("B92", Unwired("crates/l2-kingdom/src/weather/mod.rs::local_modifier")),
];

