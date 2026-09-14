//! **The first test that compares what the ORIGINAL did with what WE do.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-game --test differential -- --nocapture
//! ```
//!
//! # Why this file exists
//!
//! Everything else in this workspace checks either a set of **names** or a
//! block of **static data**. `crates/l2-game/tests/arms.rs` asserts set
//! equality between `docs/arms.json` and the `// arm:` markers in the source.
//! `crates/l2-sim/tests/oracle.rs` opens `Lords2.exe` at its fixed `0x400000`
//! base and compares three battle tables byte for byte — a real oracle, and a
//! **static** one: those bytes were the same before the game was ever run.
//!
//! Nothing compared **behaviour**. No test started the original's state and
//! ours from the same point, advanced both, and looked at the difference. This
//! is that test.
//!
//! # What it does
//!
//! 1. Open the **before** save and import it — `l2_game::scenario::from_save`,
//!    which is `l2_scenario::Scenario` plus the interface's own seeding.
//! 2. Run **our** End Turn: `l2_game::turn::end_turn`, the headless door on the
//!    seven-phase machine — `Turn_Tick`'s phases, with `Season_Advance`'s
//!    thirty passes inside phase 7.
//! 3. Open the **after** save and read it through
//!    `l2_formats::save::{County, Realm, DiploPair, Globals}` — the original's
//!    own record layout at the offsets `docs/kingdom.md` names, not a
//!    projection of ours. The two structures were written independently and
//!    `l2-formats` knows nothing about a kingdom, which is what makes this
//!    evidence.
//! 4. Compare, field by field, and print every divergence with the field's
//! path, the original's value, ours, and the delta.
//!
//! **This is a measuring instrument, not a passing test.** Nothing here was
//! tuned to agree and no comparison was weakened to go green. The assertion is
//! on a recorded baseline ([`BASELINE`] and four totals).
//! *improves* agreement fails just as loudly as one that worsens it and forces
//! somebody to move the number deliberately. That is `GATED_TOTAL`'s contract
//! in `crates/l2-testkit/tests/census.rs`, and the totals are stated separately
//! from the per-field list for the same reason they are there: a change that
//! moves a divergence from one field to another still has to be acknowledged.
//!
//! # The pairs, and the thing the filenames get wrong
//!
//! The work this file came from was briefed as "before/after pairs taken from
//! the real game across a single End Turn", naming `battle-before.sav` and
//! `battle-after.sav` first.
//!
//! **They are not a turn apart.** `battle-before`, `battle-during` and
//! `battle-after` all read `g_turnCount = 5`, `g_season = 4`, `g_year = 1269`.
//! They are **one battle caught at three moments inside one turn**, which is
//! what `crates/l2-game/tests/seam.rs` already uses them for. The same is true
//! of `siege-lastturn` / `siege-sieging` / `siege-aftersie`, all turn 14.
//! [`the_named_before_and_after_saves_are_the_same_turn`] asserts that rather
//! than leaving it as prose.
//!
//! What *is* a turn apart is the **autosave rotation**. `Save_RotateAndWrite`
//! does `safeturn.sav <- old_turn.sav <- lastturn.sav` at every turn boundary,
//! so a fixture directory copied in one go holds three consecutive turn
//! openings. The clocks, read out of the eleven fixtures:
//!
//! | fixture | turn | season | year |
//! |---|---:|---:|---:|
//! | `safeturn.sav` | 3 | 2 | 1268 |
//! | `old_turn.sav` | 4 | 3 | 1268 |
//! | `battle-before.sav` | 5 | 4 | 1269 |
//! | `siege-safeturn.sav` | 12 | 3 | 1270 |
//! | `siege-old_turn.sav` | 13 | 4 | 1271 |
//! | `siege-lastturn.sav` | 14 | 1 | 1271 |
//!
//! So there are **four** one-End-Turn pairs on disk and none of them is the
//! pair the filenames advertise. [`PAIRS`] is those four.
//!
//! # Read the second column, not the percentage
//!
//! **A field neither side moved agrees for free.** Most of a county record is
//! carried straight through the import and is not touched by a season.
//! raw agreement percentage over every field is a statement about how much of
//! the record is inert. This report therefore counts twice:
//!
//! * every comparison, and
//! * only the comparisons where **the original's own value changed between the
//!   two saves** — the fields the turn moved.
//!
//! The second number is the one that means anything. Both are asserted, and the
//! ablation below is what proves the difference between them is real rather
//! than rhetorical: with our End Turn deleted the first number is still 66 %
//! and the second is 0 %.
//!
//! # What this instrument cannot see, and it is not small
//!
//! **A save-to-save step is a player's turn plus an End Turn, and only the
//! second half is ours to reproduce.** `old_turn.sav` is the *opening* of turn
//! 4 and `battle-before.sav` the opening of turn 5. Between them a person
//! played: they may have moved the tax slider, changed the ration, ordered a
//! castle, bought weapons, marched an army. None of that is in either file as
//! an *action*, and no amount of simulation recovers it.
//! consecutive saves that avoids this — a turn is where the player lives.
//!
//! That is why divergences are classified. [`Kind`] has four
//! values and the report keeps them apart:
//!
//! * [`Kind::Simulated`] — we compute it, and a divergence is **ours to
//!   explain**;
//! * [`Kind::PlayerInput`] — a person or an AI lord set it during the turn we
//! cannot replay.
//! * [`Kind::Unsimulated`] — nothing of ours ever writes it. A field we never
//!   write is a different fact from a field we write differently, and lumping
//!   them produces a number that means nothing;
//! * [`Kind::Excluded`] — **not comparable at all**, each with its reason
//!   written at the field. An unexplained exclusion is how a differential
//!   becomes decorative.
//!
//! Every exclusion here is one thing: the original draws from two 31-bit LFSRs
//! (`FUN_00404A46`) whose state is **not among the blocks `Save_Write`
//! stores**, and we draw from `l2_net::Pcg32`. `l2_game::scenario::SEED`'s own
//! documentation says it — *"the weather and the event deck will not follow the
//! original's from this save; everything that does not draw a random number
//! will"* — and this file is the first thing that puts a number on the second
//! half of that sentence.
//!
//! **`g_units` is not compared**, and that is a scope decision.
//! oversight: a unit's tile, path and orders are almost entirely the player's
//! turn.
//! nothing else. The county, realm and global records are where a season's
//! arithmetic lands.
//!
//! **And the honest note about [`Kind::Unsimulated`]**: it has exactly one
//! member,
//! the *field list is the save reader's vocabulary*, and
//! `l2_formats::save::County` was written by people adding the fields they had
//! a use for — so a field we do not model is usually a field it does not read
//! either. The one member is `g_optAiLords`, which `l2-formats` reads and the
//! importer drops. A wider reader would find more; this list cannot.
//!
//! # The ablation, and why the raw percentage is the wrong number
//!
//! `docs/agents.md`: *delete the exact line the assertion claims to be about,
//! and watch the test go red.* The line is the `l2_game::turn::end_turn` call
//! in [`run`]. Deleting it — so that the imported before-state is compared
//! against the after-save with **no turn run into it at all** — gives:
//!
//! | | with our End Turn | with it deleted |
//! |---|---:|---:|
//! | agree, all fields | 889 of 932 (95 %) | 623 of 932 (**66 %**) |
//! | agree, fields the original moved | 251 of 279 (89 %) | **0 of 279 (0 %)** |
//!
//! **The raw percentage falls by 28 points and the moved-field percentage falls
//! to zero.** That is the whole argument for the second column in one table: a
//! differential that quoted only the first would have reported *sixty-six per
//! cent agreement* for an engine that did nothing whatever, which is a number
//! nobody should be allowed to feel good about. The moved-field count is zero
//! by construction under ablation — every field the original moved is a field
//! we then left where the before-save had it — and that is exactly the claim it
//! is supposed to make.
//!
//! # What the first run found
//!
//! Three leads, and none of them was known before this file existed. They are
//! reported: this is an instrument, and tuning the simulation
//! in the same change that builds the ruler is how a ruler stops measuring.
//!
//! 1. **The human realm's `score` and `rank` were never recomputed** — 50 in
//!    ours against 576, 590, 1333 and 1334, every pair, 50 being
//!    `Tables::score_gold_bracket` alone with all six weighted inputs at zero.
//! **Fixed, and the first entry's *"we have not found the function that does
//!    it"* is answered.** Two call sites, both read out of `Lords2.exe`:
//!
//!    * `Turn_BeginPlayersTurn` (`0x0049B6D3`) writes `aiStep = 0` into **every**
//!      realm — 999 only for one at zero strength — and `AI_RunTurnStep`'s
//! (`0x0049A581`) `isHuman` test guards the fourteen handlers and the
//!      counter's increment, **not the step-0 prologue above them**. That
//!      prologue is `Realm_RecountStrength(r); Realm_UpdateTotals(r);
//!      offerPending = 0; aiStep = 1;` and it runs for the human. We ran only
//!      the first of the four, and `Realm_UpdateTotals` (`0x0049D1E0`) is the
//!      only thing in the binary that fills the score inputs.
//!      `l2_game::turn::step_zero` is the prologue in full. [V]
//!    * `Turn_Tick` (`0x0049A010`) phase 7 calls `Score_RankRealms()` a second
//!      time, after `Season_Advance()`, which we already had at
//!      `finish_tick`'s `game.rank_realms()`. [V]
//!
//!    With the five inputs carried, every realm came in **exactly 50 short**,
//!    once per castle: `score_inputs[5]`, realm `+0x4C`, which
//!    `l2_kingdom::tables::SCORE_INPUT_OFFSETS` has documented as *castles held*
//!    — with the C for it — and which **nothing in the workspace wrote**. Its
//!    only writer in the original is `Castle_BuildTick` (`0x004508DE`), verified
//!    exhaustively: seven instructions in the whole binary mention
//!    `g_realms + 0x4C` and they are that function's clear and increment, the
//!    setup clear, `Score_RankRealms` three times, and one painter. It is the
//!    heaviest-weighted of the six (×50, more than the other five combined), so
//!    a documented-and-unwritten field was silently deleting most of the score.
//!    [V] `l2_kingdom::Kingdom::castle_build_tick` writes it now.
//!
//!    **`realm.strength` 10-against-9 on the siege pairs is not this**, and it
//!    survives: realm 1's 43-man army at (46,41) is intact in all three siege
//!    saves, and **our** turn has realm 2's 149-man army destroy it
//!    (`loser_owner: 1, loser_destroyed: true`) on every one of the three. The
//!    original leaves the two standing adjacent for three consecutive turns. So
//!    it is an AI army-movement or siege divergence, not a scoring one — and it
//!    is also the whole of the residual score gap, since 43 men is `43 / 5 = 8`
//!    and realm 1's score is short by exactly 8 in both siege pairs. [V] Left
//!    diverging and reported, which is what this file is for.
//! 2. **Neutral counties buy grain and ours could not. Fixed — C149.** In
//!    `battle 4->5`, unowned counties 1 and 3 went 71 → 121 and 57 → 103 while
//!    ours went 71 → 71 and 57 → 53: **50 sacks** a county a season, the first
//!    measurement of a seam that had been documented and left unbuilt.
//!
//!    **The comment that documented it was wrong, and that is the lesson.**
//! `l2_kingdom::ai_farm::NoMarket` said *"
//!    style's opening shopping cascade is refused."* County `+0x1A4`, the stall
//! gate, is **non-zero on every fixture county holding a merchant**, and the
//!    purse at `+0x1F4` carries 186, 297 and 260 on county 1 and 195, 316 and
//! 294 on county 3. The stall was there and the money was there; nobody had
//!    opened the file. Third of the class after `NOT SIMULATED` (C120), the font
//!    comment (C107) and `INDUSTRY / NOT DRAWN` (C136) — **a placeholder that
//!    explains itself is asserting a finding, and nothing checks it.**
//!
//!    The 50 is a *computation*, not a constant: grain's base price is 2 and
//! every merchant's morale is 100.
//!    `Ai_BuyGood` (`0x004A4B12`) takes the first of 400/200/100/50 whose whole
//!    bill the purse covers. The control is in the same fixture — at 186 and 195
//!    crowns one turn earlier the 200-crown rung is unaffordable and nothing is
//!    bought, which is exactly what `battle 3->4` shows. [V]
//! 2a. **And underneath it, the money.** `Tax_CollectAll` (`0x0044B59B`) ends in
//!    a two-limb branch — `if (realm == 0) county.purse += taxCollected;` — and
//!    we ran only the `else`. `docs/kingdom.md` §4.1 carried that limb in its
//!    pseudocode under a `[V]` paragraph, `l2_kingdom::ai`'s comment carried it,
//!    and `County::purse`'s doc carried it: **three documents right and no code
//!    doing it.** `County_RecountMerchants` (`0x00451061`) was in the printed
//!    season-pass list and in no pipeline.
//! 3. **`g_optAiLords` is read by the save reader and dropped by the
//! importer**.
//!    with. [V] — `grep -rn ai_lords crates/` puts it in `l2-formats`, in
//!    `l2_game::setup` (which *starts* a game) and nowhere on the load path.
//!
//! The population arithmetic, by contrast, came out within one or two of the
//! original's on every county that moved — births 79 against 80, deaths 96
//! against 95, population 767 against 769 — which was the half of this report
//! that was good news and which nothing had ever measured either. **The one
//! was a rule, and it is exact now**: `Population_UpdateAll` gives the season's
//! extra person to the deaths when the *happiness-scaled* birth rate is below
//! the death rate, and we compared the ladder's unscaled rate
//! (`docs/decisions.md` C170).

use l2_formats::save::{
    County as SavedCounty, DiploPair, Globals, Realm as SavedRealm, Save,
};
use l2_game::game::Game;
use l2_kingdom::county::County;
use l2_kingdom::realm::{Pair as OurPair, Realm};
use l2_kingdom::tables::Tables;

/// One before/after fixture pair, and what it is.
struct FixturePair {
    /// A short name used in the report and in [`BASELINE`].
    label: &'static str,
    before: &'static str,
    after: &'static str,
}

/// **The four pairs that are one End Turn apart**, established by
/// reading `g_turnCount` out of both files.
/// See [`the_pairs_are_one_end_turn_apart`].
const PAIRS: &[FixturePair] = &[
    FixturePair { label: "battle 3->4", before: "safeturn.sav", after: "old_turn.sav" },
    FixturePair { label: "battle 4->5", before: "old_turn.sav", after: "battle-before.sav" },
    FixturePair {
        label: "siege 12->13",
        before: "siege-safeturn.sav",
        after: "siege-old_turn.sav",
    },
    FixturePair {
        label: "siege 13->14",
        before: "siege-old_turn.sav",
        after: "siege-lastturn.sav",
    },
];

/// The saves the brief called a before/after pair, which are one turn.
const SAME_TURN_TRIPLES: &[[&str; 3]] = &[
    ["battle-before.sav", "battle-during.sav", "battle-after.sav"],
    ["siege-lastturn.sav", "siege-sieging.sav", "siege-aftersie.sav"],
];

/// **What a divergence in this field means.** The whole value of the report is
/// in this enum: three fields disagreeing for three different reasons is three
/// findings, and one number over all of them is none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// We compute it. A divergence is ours to explain.
    Simulated,
    /// A person or an AI lord set it during the turn between the two saves.
    /// The pair carries the *result* and not the *action*.
/// here is a missing input.
    PlayerInput,
    /// Nothing in our tree writes it.
    Unsimulated,
    /// Not comparable, with the reason.
    Excluded(&'static str),
}

impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Kind::Simulated => "simulated",
            Kind::PlayerInput => "player-input",
            Kind::Unsimulated => "unsimulated",
            Kind::Excluded(_) => "excluded",
        }
    }

    fn compared(self) -> bool {
        !matches!(self, Kind::Excluded(_))
    }
}

struct CountyField {
    path: &'static str,
    kind: Kind,
    ours: fn(&County) -> i64,
    theirs: fn(&SavedCounty) -> i64,
}

struct RealmField {
    path: &'static str,
    kind: Kind,
    ours: fn(&Realm) -> i64,
    theirs: fn(&SavedRealm) -> i64,
}

struct PairField {
    path: &'static str,
    kind: Kind,
    ours: fn(&OurPair) -> i64,
    theirs: fn(&DiploPair) -> i64,
}

struct GlobalField {
    path: &'static str,
    kind: Kind,
    ours: fn(&Game) -> i64,
    theirs: fn(&Globals) -> i64,
}

/// Every county field `l2_formats::save::County` carries, except the two that
/// are not scalars (`index`, `neighbours`).
///
/// The list is the **save's** vocabulary, on purpose:
/// `l2_kingdom::county::County` has 101 fields and some seventy of them are
/// derived, computed later, or absent from the file.
/// that struct would be mostly filler — and noise is where an omission hides
/// (`docs/agents.md`, *choose the smaller list*).
#[rustfmt::skip]
const COUNTY_FIELDS: &[CountyField] = &[
    CountyField { path: "county.owner", kind: Kind::Simulated, ours: |c| c.owner as i64, theirs: |c| c.owner as i64 },
    // --- the happiness chain, `Happiness_UpdateAll` ----------------------
    CountyField { path: "county.happiness", kind: Kind::Simulated, ours: |c| c.happiness as i64, theirs: |c| c.happiness as i64 },
    CountyField { path: "county.happiness_last", kind: Kind::Simulated, ours: |c| c.happiness_last as i64, theirs: |c| c.happiness_last as i64 },
    CountyField { path: "county.d_hap_tax", kind: Kind::Simulated, ours: |c| c.d_hap_tax as i64, theirs: |c| c.d_hap_tax as i64 },
    CountyField { path: "county.d_hap_health", kind: Kind::Simulated, ours: |c| c.d_hap_health as i64, theirs: |c| c.d_hap_health as i64 },
    CountyField { path: "county.d_hap_ration", kind: Kind::Simulated, ours: |c| c.d_hap_ration as i64, theirs: |c| c.d_hap_ration as i64 },
    CountyField { path: "county.shown_tax", kind: Kind::Simulated, ours: |c| c.shown_tax as i64, theirs: |c| c.shown_tax as i64 },
    CountyField { path: "county.shown_ration", kind: Kind::Simulated, ours: |c| c.shown_ration as i64, theirs: |c| c.shown_ration as i64 },
    CountyField { path: "county.shown_health", kind: Kind::Simulated, ours: |c| c.shown_health as i64, theirs: |c| c.shown_health as i64 },
    CountyField { path: "county.shown_army", kind: Kind::Simulated, ours: |c| c.shown_army as i64, theirs: |c| c.shown_army as i64 },
    CountyField { path: "county.shown_events", kind: Kind::Simulated, ours: |c| c.shown_events as i64, theirs: |c| c.shown_events as i64 },
    // `+0x18` and `+0x1C` are a running mean and its accumulator over the
    // **whole game**, and `l2-scenario` seeds both from the current happiness
    // because the save stores no sample count to resume from. A pair twelve
    // turns in therefore cannot agree, and the divergence would say nothing
    // about the rule.
    CountyField { path: "county.happiness_avg", kind: Kind::Excluded("a running mean over the whole game; the save stores no sample count, so the import restarts it"), ours: |c| c.happiness_avg as i64, theirs: |c| c.happiness_avg as i64 },
    CountyField { path: "county.happiness_sum", kind: Kind::Excluded("the accumulator behind happiness_avg, restarted by the import for the same reason"), ours: |c| c.happiness_sum as i64, theirs: |c| c.happiness_sum as i64 },
    // --- health, `Health_UpdateAll` --------------------------------------
    CountyField { path: "county.health_meter", kind: Kind::Simulated, ours: |c| c.health_meter as i64, theirs: |c| c.health_meter as i64 },
    CountyField { path: "county.health_band", kind: Kind::Simulated, ours: |c| c.health_band as i64, theirs: |c| c.health_band as i64 },
    // --- unrest, `Unrest_UpdateAll` --------------------------------------
    CountyField { path: "county.unrest", kind: Kind::Simulated, ours: |c| c.unrest as i64, theirs: |c| c.unrest as i64 },
    // --- population, `Migration_UpdateAll` then `Population_UpdateAll` ----
    CountyField { path: "county.population", kind: Kind::Simulated, ours: |c| c.population as i64, theirs: |c| c.population as i64 },
    CountyField { path: "county.pop_last", kind: Kind::Simulated, ours: |c| c.pop_last as i64, theirs: |c| c.pop_last as i64 },
    CountyField { path: "county.births", kind: Kind::Simulated, ours: |c| c.births as i64, theirs: |c| c.births as i64 },
    CountyField { path: "county.deaths", kind: Kind::Simulated, ours: |c| c.deaths as i64, theirs: |c| c.deaths as i64 },
    CountyField { path: "county.emigrants", kind: Kind::Simulated, ours: |c| c.emigrants as i64, theirs: |c| c.emigrants as i64 },
    CountyField { path: "county.immigrants", kind: Kind::Simulated, ours: |c| c.immigrants as i64, theirs: |c| c.immigrants as i64 },
    CountyField { path: "county.pop_band", kind: Kind::Simulated, ours: |c| c.pop_band as i64, theirs: |c| c.pop_band as i64 },
    // --- structure, which a season must not move -------------------------
    CountyField { path: "county.neighbour_count", kind: Kind::Simulated, ours: |c| c.neighbour_count as i64, theirs: |c| c.neighbour_count as i64 },
    CountyField { path: "county.anchor_x", kind: Kind::Simulated, ours: |c| c.anchor_x as i64, theirs: |c| c.anchor_x as i64 },
    CountyField { path: "county.anchor_y", kind: Kind::Simulated, ours: |c| c.anchor_y as i64, theirs: |c| c.anchor_y as i64 },
    // --- tax, `Tax_CollectAll` -------------------------------------------
    CountyField { path: "county.tax_rate", kind: Kind::PlayerInput, ours: |c| c.tax_rate as i64, theirs: |c| c.tax_rate as i64 },
    CountyField { path: "county.tax_collected", kind: Kind::Simulated, ours: |c| c.tax_collected as i64, theirs: |c| c.tax_collected as i64 },
    // --- the ration, `Ration_Apply` --------------------------------------
    CountyField { path: "county.ration_wanted", kind: Kind::PlayerInput, ours: |c| c.ration_wanted as i64, theirs: |c| c.ration_wanted as i64 },
    CountyField { path: "county.ration_split", kind: Kind::PlayerInput, ours: |c| c.ration_split as i64, theirs: |c| c.ration_split as i64 },
    CountyField { path: "county.ration_achieved", kind: Kind::Simulated, ours: |c| c.ration_achieved as i64, theirs: |c| c.ration_achieved as i64 },
    CountyField { path: "county.grain_eaten", kind: Kind::Simulated, ours: |c| c.grain_eaten as i64, theirs: |c| c.grain_eaten as i64 },
    CountyField { path: "county.herd_eaten", kind: Kind::Simulated, ours: |c| c.herd_eaten as i64, theirs: |c| c.herd_eaten as i64 },
    CountyField { path: "county.grain_available", kind: Kind::Simulated, ours: |c| c.grain_available as i64, theirs: |c| c.grain_available as i64 },
    CountyField { path: "county.herd_available", kind: Kind::Simulated, ours: |c| c.herd_available as i64, theirs: |c| c.herd_available as i64 },
    // --- the stores, `Grain_SeasonTick` and `Herd_SeasonTick` ------------
    CountyField { path: "county.grain", kind: Kind::Simulated, ours: |c| c.grain as i64, theirs: |c| c.grain as i64 },
    CountyField { path: "county.herd", kind: Kind::Simulated, ours: |c| c.herd as i64, theirs: |c| c.herd as i64 },
    // --- the land --------------------------------------------------------
    CountyField { path: "county.fields_fallow", kind: Kind::Simulated, ours: |c| c.fields_fallow as i64, theirs: |c| c.fields_fallow as i64 },
    CountyField { path: "county.fields_cattle", kind: Kind::Simulated, ours: |c| c.fields_cattle as i64, theirs: |c| c.fields_cattle as i64 },
    CountyField { path: "county.fields_grain", kind: Kind::Simulated, ours: |c| c.fields_grain as i64, theirs: |c| c.fields_grain as i64 },
    CountyField { path: "county.fertility", kind: Kind::Simulated, ours: |c| c.fertility as i64, theirs: |c| c.fertility as i64 },
    // --- the castle, `Castle_BuildTick` -----------------------------------
    CountyField { path: "county.castle_type", kind: Kind::Simulated, ours: |c| c.castle_type as i64, theirs: |c| c.castle_type as i64 },
    CountyField { path: "county.castle_building", kind: Kind::PlayerInput, ours: |c| c.castle_building as i64, theirs: |c| c.castle_building as i64 },
    // --- the weather, and the two fields that cannot be compared at all ---
    CountyField { path: "county.weather", kind: Kind::Excluded("Weather_UpdateAll's band is drawn from the original's two 31-bit LFSRs, whose state Save_Write does not store"), ours: |c| c.weather.index() as i64, theirs: |c| c.weather as i64 },
    CountyField { path: "county.dryness", kind: Kind::Excluded("the dryness accumulator the band comes from, moved by the same unreproducible jitter"), ours: |c| c.dryness as i64, theirs: |c| c.dryness as i64 },
];

/// Every realm field `l2_formats::save::Realm` carries, except `index` and the
/// pair block, which has its own table.
#[rustfmt::skip]
const REALM_FIELDS: &[RealmField] = &[
    RealmField { path: "realm.strength", kind: Kind::Simulated, ours: |r| r.strength as i64, theirs: |r| r.strength as i64 },
    RealmField { path: "realm.is_human", kind: Kind::Simulated, ours: |r| r.is_human as i64, theirs: |r| r.is_human as i64 },
    RealmField { path: "realm.lord", kind: Kind::Simulated, ours: |r| r.lord as i64, theirs: |r| r.lord as i64 },
    RealmField { path: "realm.shield_index", kind: Kind::Simulated, ours: |r| r.shield_index as i64, theirs: |r| r.shield_index as i64 },
    RealmField { path: "realm.tax_hap_empire", kind: Kind::Simulated, ours: |r| r.tax_hap_empire as i64, theirs: |r| r.tax_hap_empire as i64 },
    RealmField { path: "realm.county_count", kind: Kind::Simulated, ours: |r| r.county_count as i64, theirs: |r| r.county_count as i64 },
    RealmField { path: "realm.rank", kind: Kind::Simulated, ours: |r| r.rank as i64, theirs: |r| r.rank as i64 },
    RealmField { path: "realm.score", kind: Kind::Simulated, ours: |r| r.score as i64, theirs: |r| r.score as i64 },
    RealmField { path: "realm.wages", kind: Kind::Simulated, ours: |r| r.wages as i64, theirs: |r| r.wages as i64 },
    RealmField { path: "realm.gold", kind: Kind::Simulated, ours: |r| r.gold as i64, theirs: |r| r.gold as i64 },
    // The three stockpiles are `Industry_Produce`'s output less the castle's
    // and the armoury's spend. The armoury spend is the person's, so they
    // carry a rule *and* a missing input; left `Simulated` because the
    // season's production is the larger term and the divergence is worth
// reading.
    RealmField { path: "realm.iron", kind: Kind::Simulated, ours: |r| r.iron as i64, theirs: |r| r.iron as i64 },
    RealmField { path: "realm.stone", kind: Kind::Simulated, ours: |r| r.stone as i64, theirs: |r| r.stone as i64 },
    RealmField { path: "realm.wood", kind: Kind::Simulated, ours: |r| r.wood as i64, theirs: |r| r.wood as i64 },
    // The six weapon counters move only when somebody presses a button in the
    // armoury, which is the half of the step this pair cannot carry.
    RealmField { path: "realm.weapons.0", kind: Kind::PlayerInput, ours: |r| r.weapons[0] as i64, theirs: |r| r.weapons[0] as i64 },
    RealmField { path: "realm.weapons.1", kind: Kind::PlayerInput, ours: |r| r.weapons[1] as i64, theirs: |r| r.weapons[1] as i64 },
    RealmField { path: "realm.weapons.2", kind: Kind::PlayerInput, ours: |r| r.weapons[2] as i64, theirs: |r| r.weapons[2] as i64 },
    RealmField { path: "realm.weapons.3", kind: Kind::PlayerInput, ours: |r| r.weapons[3] as i64, theirs: |r| r.weapons[3] as i64 },
    RealmField { path: "realm.weapons.4", kind: Kind::PlayerInput, ours: |r| r.weapons[4] as i64, theirs: |r| r.weapons[4] as i64 },
    RealmField { path: "realm.weapons.5", kind: Kind::PlayerInput, ours: |r| r.weapons[5] as i64, theirs: |r| r.weapons[5] as i64 },
    // `+0x00` is the AI turn cursor and the save catches it wherever the AI
    // happened to be when the rotation ran. Our machine is between turns when
    // `end_turn` returns, so the two are not sampled at the same instant and
    // the comparison would measure the sampling point.
    RealmField { path: "realm.ai_step", kind: Kind::Excluded("the AI turn cursor, sampled at a different instant: Save_RotateAndWrite runs inside phase 7 and our end_turn returns between turns"), ours: |r| r.ai_step as i64, theirs: |r| r.ai_step as i64 },
];

/// Realm `+0x84 + other * 0x10` — one realm's view of another. Nine fields,
/// and they move every turn: the standing heals by one a season, the warning
/// ladder climbs, the grudge accumulates while allied.
#[rustfmt::skip]
const PAIR_FIELDS: &[PairField] = &[
    PairField { path: "pair.standing", kind: Kind::Simulated, ours: |p| p.standing as i64, theirs: |p| p.standing as i64 },
    PairField { path: "pair.allied", kind: Kind::Simulated, ours: |p| p.allied as i64, theirs: |p| p.allied as i64 },
    PairField { path: "pair.grudge", kind: Kind::Simulated, ours: |p| p.grudge as i64, theirs: |p| p.grudge as i64 },
    PairField { path: "pair.warnings_sent", kind: Kind::Simulated, ours: |p| p.warnings_sent as i64, theirs: |p| p.warnings_sent as i64 },
    PairField { path: "pair.at_war", kind: Kind::Simulated, ours: |p| p.at_war as i64, theirs: |p| p.at_war as i64 },
    PairField { path: "pair.compliments_from", kind: Kind::Simulated, ours: |p| p.compliments_from as i64, theirs: |p| p.compliments_from as i64 },
    PairField { path: "pair.best_gift", kind: Kind::Simulated, ours: |p| p.best_gift as i64, theirs: |p| p.best_gift as i64 },
    PairField { path: "pair.has_mail", kind: Kind::Simulated, ours: |p| p.has_mail as i64, theirs: |p| p.has_mail as i64 },
    PairField { path: "pair.help_price_multiple", kind: Kind::Simulated, ours: |p| p.help_price_multiple as i64, theirs: |p| p.help_price_multiple as i64 },
];

/// The scalars `Save_Write` stores outside the two arrays.
#[rustfmt::skip]
const GLOBAL_FIELDS: &[GlobalField] = &[
    GlobalField { path: "global.county_count", kind: Kind::Simulated, ours: |g| g.kingdom.county_count as i64, theirs: |g| g.county_count as i64 },
    GlobalField { path: "global.local_player", kind: Kind::Simulated, ours: |g| g.player as i64, theirs: |g| g.local_player as i64 },
    GlobalField { path: "global.season", kind: Kind::Simulated, ours: |g| g.kingdom.season as i64, theirs: |g| g.season as i64 },
    GlobalField { path: "global.season_next", kind: Kind::Simulated, ours: |g| g.kingdom.season_next as i64, theirs: |g| g.season_next as i64 },
    GlobalField { path: "global.year", kind: Kind::Simulated, ours: |g| g.kingdom.year as i64, theirs: |g| g.year as i64 },
    GlobalField { path: "global.turn_count", kind: Kind::Simulated, ours: |g| g.kingdom.turn_count as i64, theirs: |g| g.turn_count as i64 },
    GlobalField { path: "global.merchant_count", kind: Kind::Simulated, ours: merchant_count, theirs: |g| g.merchant_count as i64 },
    GlobalField { path: "global.opt_difficulty", kind: Kind::Simulated, ours: |g| g.kingdom.options.difficulty as i64, theirs: |g| g.opt_difficulty as i64 },
    GlobalField { path: "global.opt_advanced_farming", kind: Kind::Simulated, ours: |g| g.kingdom.options.advanced_farming as i64, theirs: |g| (g.opt_advanced_farming != 0) as i64 },
    GlobalField { path: "global.opt_armies_eat", kind: Kind::Simulated, ours: |g| g.kingdom.options.armies_eat as i64, theirs: |g| (g.opt_armies_eat != 0) as i64 },
    GlobalField { path: "global.opt_exploration", kind: Kind::Simulated, ours: |g| g.kingdom.options.exploration as i64, theirs: |g| (g.opt_exploration != 0) as i64 },
    GlobalField { path: "global.opt_time_limit", kind: Kind::Simulated, ours: |g| g.kingdom.options.time_limit as i64, theirs: |g| g.opt_time_limit as i64 },
// **The one un-simulated field this list can see.**
    // `g_optAiLords` (`0x0053F268`) is stored by `Save_Write` and read by
    // `l2_formats::save::Globals`; `l2_scenario::Scenario::from_save` does not
    // carry it into `Options` and nothing after a load reads it. `grep -rn
    // ai_lords crates/` finds it in the save reader, in `l2_game::setup` (which
    // *starts* a game) and nowhere on the load path.
    // know how many lords it was started with, and this row says so with a
// number.
    GlobalField { path: "global.ai_lords", kind: Kind::Unsimulated, ours: |_| 0, theirs: |g| g.ai_lords as i64 },
    // `g_turnPhase` and its step counter are the state of `Turn_Tick`'s
    // machine at the moment `Save_RotateAndWrite` ran, which is *inside* phase
    // 7. `end_turn` returns with the machine back at the player's turn, so the
    // two numbers describe different instants.
    GlobalField { path: "global.turn_phase", kind: Kind::Excluded("Turn_Tick's phase at the instant the autosave was written, which is inside phase 7; our end_turn returns between turns"), ours: |g| g.kingdom.turn.phase.index() as i64, theirs: |g| g.turn_phase as i64 },
    GlobalField { path: "global.turn_phase_step", kind: Kind::Excluded("the step counter inside that phase, for the same reason"), ours: |g| g.kingdom.turn.step as i64, theirs: |g| g.turn_phase_step as i64 },
    // `g_weatherCounty` is the county `Weather_UpdateAll` gave the local swing
    // to, drawn from the same LFSR the band is.
    GlobalField { path: "global.weather_county", kind: Kind::Excluded("the county Weather_UpdateAll drew for the local swing, from the LFSR stream we do not reproduce"), ours: |g| g.kingdom.weather_county as i64, theirs: |g| g.weather_county as i64 },
];

/// How many merchants are on the map, for `g_merchantCount`.
fn merchant_count(g: &Game) -> i64 {
    g.kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| u.kind == l2_kingdom::unit::UnitKind::Merchant)
        .count() as i64
}

// --- the machinery ----------------------------------------------------------

/// One field of one record, compared.
struct Row {
    pair: &'static str,
    path: &'static str,
    record: String,
    kind: Kind,
    /// What the *before* save held. Only used to decide whether the turn moved
    /// this field at all.
    was: i64,
    theirs: i64,
    ours: i64,
}

impl Row {
    fn delta(&self) -> i64 {
        self.ours - self.theirs
    }

    /// Did the original's own value change across the End Turn? A field that
    /// did not is a field that agrees for free.
    fn moved(&self) -> bool {
        self.was != self.theirs
    }
}

/// What one pair produced. Every comparison is kept, agreeing or not, because
/// the *moved* count cannot be derived from the divergences alone.
struct PairReport {
    compared: usize,
    agree: usize,
    moved: usize,
    moved_agree: usize,
    diverged: Vec<Row>,
}

/// The clock, as the three numbers that decide whether two saves are a turn
/// apart.
fn clock(save: &Save) -> (i32, i32, i32) {
    let g = save.globals().expect("the globals block");
    (g.turn_count, g.season, g.year)
}

/// Run one pair and collect every divergence.
fn run(pair: &FixturePair) -> Result<PairReport, String> {
    let before = l2_testkit::fixture_save(pair.before)?;
    let after = l2_testkit::fixture_save(pair.after)?;

    let mut game = l2_game::scenario::from_save(&before, Tables::DEFAULT)
        .map_err(|e| format!("{} does not import: {e:?}", pair.before))?;
    l2_game::turn::end_turn(&mut game)
        .ok_or_else(|| format!("the turn machine did not come round on {}", pair.before))?;

    let mut r = PairReport { compared: 0, agree: 0, moved: 0, moved_agree: 0, diverged: Vec::new() };
    let mut note = |kind: Kind, path: &'static str, record: String, was: i64, theirs: i64, ours: i64| {
        if !kind.compared() {
            return;
        }
        let row = Row { pair: pair.label, path, record, kind, was, theirs, ours };
        r.compared += 1;
        let agrees = row.ours == row.theirs;
        if agrees {
            r.agree += 1;
        }
        if row.moved() {
            r.moved += 1;
            if agrees {
                r.moved_agree += 1;
            }
        }
        if !agrees {
            r.diverged.push(row);
        }
    };

    // --- counties --------------------------------------------------------
    let was_counties = before.counties().map_err(|e| e.to_string())?;
    let counties = after.counties().map_err(|e| e.to_string())?;
    let county_count = after.globals().map_err(|e| e.to_string())?.county_count as usize;
    for id in 1..=county_count.min(l2_kingdom::county::MAX_COUNTY_ID as usize) {
        for f in COUNTY_FIELDS {
            note(
                f.kind,
                f.path,
                format!("county {id}"),
                (f.theirs)(&was_counties[id]),
                (f.theirs)(&counties[id]),
                (f.ours)(&game.kingdom.counties[id]),
            );
        }
    }

    // --- realms and the diplomatic matrix --------------------------------
    let was_realms = before.realms().map_err(|e| e.to_string())?;
    let realms = after.realms().map_err(|e| e.to_string())?;
    // **Realms 3, 4 and 5 are out of play in all four pairs**, so nothing here
    // ever farmed or compared them: measured off this test's own printout,
    // which names realms 1 and 2 and no other. A ledger row claiming AI realm 3
    // never plants where the original does cannot be seen from here, and the
    // behaviour behind it is not a defect: realm 3's lord is 2, whose style
    // byte is 1 — `Ai_FarmStyleGrazing` (`0x004A42E3`), which clears grain in
    // every season. `county.fields_grain` is compared and agrees everywhere.
    // `l2_kingdom::ai_farm`'s `two_of_the_four_lords_graze_so_their_realms_never_sow`.
    for (id, theirs) in realms.iter().enumerate() {
        if !theirs.in_play() {
            continue;
        }
        for f in REALM_FIELDS {
            note(
                f.kind,
                f.path,
                format!("realm {id}"),
                (f.theirs)(&was_realms[id]),
                (f.theirs)(theirs),
                (f.ours)(&game.kingdom.realms[id]),
            );
        }
        for (other, them) in realms.iter().enumerate() {
            if other == id || !them.in_play() {
                continue;
            }
            for f in PAIR_FIELDS {
                note(
                    f.kind,
                    f.path,
                    format!("realm {id}->{other}"),
                    (f.theirs)(&was_realms[id].pairs[other]),
                    (f.theirs)(&theirs.pairs[other]),
                    (f.ours)(&game.kingdom.realms[id].pairs[other]),
                );
            }
        }
    }

    // --- the globals ------------------------------------------------------
    let was_globals = before.globals().map_err(|e| e.to_string())?;
    let globals = after.globals().map_err(|e| e.to_string())?;
    for f in GLOBAL_FIELDS {
        note(
            f.kind,
            f.path,
            "-".to_string(),
            (f.theirs)(&was_globals),
            (f.theirs)(&globals),
            (f.ours)(&game),
        );
    }

    Ok(r)
}

// --- the recorded baseline --------------------------------------------------

/// **The divergence baseline.** `(pair, field path, how many records of that
/// pair disagree on that field)`, sorted.
///
/// A line here is a statement that *this many records of this pair disagree on
/// this field today*. Fix a rule and this goes red; break one and it goes red
/// the same way. The failure message prints the replacement block, so updating
/// it is a one-line diff that puts the change in the history.
///
/// **Moved up by C161, from the numbers the test printed.** Eleven
/// divergences went away and none arrived: iron, wood and the weapons counters
/// on the battle pairs and both siege pairs, `realm.weapons.0` on both siege
/// pairs, and battle 3->4's `county.unrest`. Those are the fields a season
/// computes from the industry ramp, the running totals and the weapon type,
/// and from the unrest warning latch — all of which a loaded game used to start
/// at `County::new()`'s values. Which import removed which divergence was not
/// ablated one at a time; the attribution is inferred from what each pass reads.
///
/// **Moved up again by the AI's second farming pass, from the numbers the test
/// printed**: `AGREE_TOTAL` 900 → 906 and `MOVED_AGREE_TOTAL` 258 → 264. Six
/// divergences went, all on realm 2 (the one AI realm in play) and none
/// arrived: battle 3->4's `realm.wood` (+31), `realm.iron` (+5) and
/// `realm.weapons.4` (−2), and siege 12->13's `realm.iron` (+35),
/// `realm.wood` (+35) and `realm.weapons.1` (−6). All three are fields
/// `Industry_ProduceAll` writes from the county's labour split and industry
/// share, which `Ai_ManageFarmsAll` (`0x0049A990`) resets at the head of the
/// season.
///
/// **Attributed by ablation, not by reading.** That branch made three changes:
/// the season-head pass, the owned-county arm of the stall (AI step 5 and the
/// season head both shop now), and realm `+0xF4`/`+0xF8`. Emptying the
/// `Pass::AiManageFarms` arm alone — the other two still in — put this test back
/// to exactly 900 and 258 with all six rows returned. So on these four pairs
/// **the season-head pass moved all six and the other two moved nothing**; the
/// stall bought nothing for realm 2 here, which is also what its unchanged
/// `realm.gold` rows say.
///
/// **What did not move**, and is not chased here: realm 2's `realm.gold` (+5,
/// then **+2,005** on siege 13->14), `realm.wages` (−5) and `realm.score`. The
/// original's realm 2 goes 2,015 → 394 on siege 13->14 while its
/// `realm.weapons.1` goes 26 → 126 — a hundred maces bought — which is the shape
/// of `Ai_TradeForCounty` (`0x0049E39B`), the weapon purchase every realm
/// farming style runs before `Ai_BuyGood` and which is not implemented. `[I]`.
///
/// **Moved up again by C170, from the numbers the test printed,
/// and this time ablated.** Seven divergences went away and none arrived: every
/// county row left on the list — `births`, `deaths` and `population` on siege
/// 12->13's county 3 and 13->14's county 1, and 12->13's `pop_band`. Each was one
/// person: `Population_UpdateAll` hands the season's extra person to the deaths
/// when the **happiness-scaled** birth rate is below the death rate, and we
/// compared the unscaled one. Putting `base < death` back restores all seven.
/// The swing change that came before it (C169) moved nothing.
///
/// **Both together, measured at the merge that brought them together**: 913 of 932 agree and 270 of
/// 279 moved fields agree, and neither branch's BASELINE rows remain.
///
/// **The hundred maces, settled.** `Ai_TradeForCounty` (`0x0049E39B`) is built
/// — `l2_kingdom::ai_farm::Market::trade_for_county` — and it is exactly what
/// realm 2's siege 13->14 rows were. The Knight's personality `+0x7C` orders a
/// hundred of the county's own weapon type once `+0x78`'s floor of 1,000 is
/// cleared; the county makes maces, good 11 quotes 10 + `Pct(10, 100)` = 20,
/// and 100 × 20 = 2,000 crowns. `realm.weapons.1` left the baseline and
/// `realm.gold` on realm 2 went **+2,005 → +5**, which is the standing
/// `realm.wages` (−5) divergence and nothing else; `realm.score` went +47 → −3.
/// 913 → **914** of 932 and 270 → **271** of 279. **Ablated**: returning
/// `trade_for_county` to the trait's empty default puts all four rows back.
/// Realm 1's `realm.gold` (+510) did not move — its lord's floor is not
/// cleared on this pair.
#[rustfmt::skip]
const BASELINE: &[(&str, &str, usize)] = &[
    ("battle 3->4", "global.ai_lords", 1),
    ("battle 4->5", "global.ai_lords", 1),
    ("siege 12->13", "global.ai_lords", 1),
    ("siege 12->13", "realm.gold", 2),
    ("siege 12->13", "realm.score", 2),
    ("siege 12->13", "realm.strength", 1),
    ("siege 12->13", "realm.wages", 2),
    ("siege 13->14", "global.ai_lords", 1),
    ("siege 13->14", "realm.gold", 2),
    ("siege 13->14", "realm.score", 2),
    ("siege 13->14", "realm.strength", 1),
    ("siege 13->14", "realm.wages", 2),
];

/// How many field comparisons the four pairs make between them, stated
/// separately from [`BASELINE`] so that a change which moves a divergence from
/// one field to another still has to be acknowledged as a change — the reason
/// `GATED_TOTAL` is stated apart from `INVENTORY` in
/// `crates/l2-testkit/tests/census.rs`.
const COMPARED_TOTAL: usize = 932;

/// How many of them agree. **Read [`MOVED_AGREE_TOTAL`] before quoting this
/// one**: most of a county record is inert across a season.
/// side touched agrees for free and this number is mostly a measure of how much
/// of the record the import carried unchanged.
const AGREE_TOTAL: usize = 914;

/// How many comparisons are of a field **the original's own End Turn moved**.
const MOVED_TOTAL: usize = 279;

/// How many of *those* agree. This is the number that means something, and it
/// is the one to quote.
const MOVED_AGREE_TOTAL: usize = 271;

// --- the tests --------------------------------------------------------------

/// **Establish the gap before trusting a filename.**
#[test]
fn the_pairs_are_one_end_turn_apart() {
    if l2_testkit::fixtures_dir().is_none() {
        l2_testkit::skip!("no fixture directory");
    }
    let mut lines = Vec::new();
    for pair in PAIRS {
        let (Ok(b), Ok(a)) =
            (l2_testkit::fixture_save(pair.before), l2_testkit::fixture_save(pair.after))
        else {
            l2_testkit::skip!("{} or {} is not in the fixture directory", pair.before, pair.after);
        };
        let (bt, bs, by) = clock(&b);
        let (at, as_, ay) = clock(&a);
        lines.push(format!(
            "  {:<14} {:<20} turn {bt:>2} season {bs} {by}  ->  {:<20} turn {at:>2} season {as_} {ay}",
            pair.label, pair.before, pair.after
        ));
        assert_eq!(
            at - bt,
            1,
            "{} -> {} is {} turns apart, not one; it is not an End Turn pair and must not be \
             used as one",
            pair.before,
            pair.after,
            at - bt
        );
        // One turn is one season. The year steps with the move into Winter,
        // which is season 4 and the first season of the game's year — a new
        // game opens in Winter 1268 (`Kingdom::start_new_game`).
        assert_eq!(
            (bs % 4) + 1,
            as_,
            "{} -> {}: the season did not step by one",
            pair.before,
            pair.after
        );
        assert_eq!(
            ay - by,
            i32::from(as_ == 4),
            "{} -> {}: the year moved other than into Winter",
            pair.before,
            pair.after
        );
    }
    eprintln!("\nthe four one-End-Turn pairs:\n{}\n", lines.join("\n"));
}

/// **The finding that is worth the job on its own**: the two saves named as a
/// before/after pair are the same turn.
/// have compared a kingdom against itself with a season run into it.
#[test]
fn the_named_before_and_after_saves_are_the_same_turn() {
    if l2_testkit::fixtures_dir().is_none() {
        l2_testkit::skip!("no fixture directory");
    }
    for triple in SAME_TURN_TRIPLES {
        let mut seen = Vec::new();
        for name in triple {
            let Ok(s) = l2_testkit::fixture_save(name) else {
                l2_testkit::skip!("{name} is not in the fixture directory");
            };
            seen.push((*name, clock(&s)));
        }
        let first = seen[0].1;
        for (name, c) in &seen {
            assert_eq!(
                *c, first,
                "{name} reads turn/season/year {c:?} where {} reads {first:?} - if these three \
                 have stopped being one moment caught three times, PAIRS and \
                 crates/l2-game/tests/seam.rs both want re-reading",
                seen[0].0
            );
        }
        eprintln!(
            "{:?} are all turn {} season {} year {} - one turn, not a before/after pair",
            triple, first.0, first.1, first.2
        );
    }
}

/// **The differential.** The report is printed in full on every run; the
/// assertion is on [`BASELINE`] and the four totals.
#[test]
fn one_end_turn_of_ours_against_one_end_turn_of_the_originals() {
    if l2_testkit::fixtures_dir().is_none() {
        l2_testkit::skip!("no fixture directory");
    }

    let mut reports = Vec::new();
    for pair in PAIRS {
        match run(pair) {
            Ok(r) => reports.push((pair.label, r)),
            Err(why) => l2_testkit::skip!("{why}"),
        }
    }

    let mut compared_total = 0usize;
    let mut agree_total = 0usize;
    let mut moved_total = 0usize;
    let mut moved_agree_total = 0usize;
    let mut by_kind: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();
    let mut found: std::collections::BTreeMap<(&str, &str), usize> = Default::default();

    for (label, r) in &reports {
        compared_total += r.compared;
        agree_total += r.agree;
        moved_total += r.moved;
        moved_agree_total += r.moved_agree;
        eprintln!(
            "\n=== {label}: {} compared, {} agree; of the {} the original's turn MOVED, {} agree",
            r.compared, r.agree, r.moved, r.moved_agree
        );
        let mut rows: Vec<&Row> = r.diverged.iter().collect();
        // Ordered: worst kind first, then by the size of the disagreement, so
        // the top of the list is the largest thing we get wrong.
        rows.sort_by_key(|row| (row.kind, -row.delta().abs(), row.path, row.record.clone()));
        for row in rows {
            let e = by_kind.entry(row.kind.tag()).or_default();
            e.0 += 1;
            if row.moved() {
                e.1 += 1;
            }
            *found.entry((row.pair, row.path)).or_default() += 1;
            eprintln!(
                "  {:<12} {:<26} {:<14} was {:>8}   original {:>8}   ours {:>8}   delta {:>+9}{}",
                row.kind.tag(),
                row.path,
                row.record,
                row.was,
                row.theirs,
                row.ours,
                row.delta(),
                if row.moved() { "  [moved]" } else { "" }
            );
        }
    }

    eprintln!("\n--- divergences by kind (total, of which the original moved) ---");
    for (kind, (n, m)) in &by_kind {
        eprintln!("  {n:>4}  {kind:<13} ({m} moved)");
    }
    let pct = |a: usize, b: usize| if b == 0 { 0 } else { a * 100 / b };
    eprintln!(
        "\n{compared_total} fields compared across {} pairs, {agree_total} agree ({}%)\n\
         of those, {moved_total} are fields the original's own End Turn MOVED: {moved_agree_total} agree ({}%)\n\
         -- the second line is the one that means anything; an inert field agrees for free.\n",
        reports.len(),
        pct(agree_total, compared_total),
        pct(moved_agree_total, moved_total),
    );

    // --- the assertion ---------------------------------------------------
    let expected: std::collections::BTreeMap<(&str, &str), usize> =
        BASELINE.iter().map(|&(p, f, n)| ((p, f), n)).collect();
    if found != expected {
        let mut block = String::new();
        for ((pair, path), n) in &found {
            block.push_str(&format!("    (\"{pair}\", \"{path}\", {n}),\n"));
        }
        panic!(
            "the divergence baseline has moved.\n\n\
             This is a measuring instrument. Do NOT weaken a comparison or exclude a field to \
             make it green: if the change improved agreement, say so by moving the numbers up. \
             Replace BASELINE in crates/l2-game/tests/differential.rs with:\n\n\
             const BASELINE: &[(&str, &str, usize)] = &[\n{block}];\n\n\
             and set COMPARED_TOTAL = {compared_total}, AGREE_TOTAL = {agree_total}, \
             MOVED_TOTAL = {moved_total}, MOVED_AGREE_TOTAL = {moved_agree_total}."
        );
    }
    assert_eq!(compared_total, COMPARED_TOTAL, "the number of fields compared has moved");
    assert_eq!(agree_total, AGREE_TOTAL, "the number of fields that agree has moved");
    assert_eq!(moved_total, MOVED_TOTAL, "the number of fields the original's turn moved has moved");
    assert_eq!(
        moved_agree_total, MOVED_AGREE_TOTAL,
        "agreement on the fields the original's turn moved has changed - this is the number that \
         means something, so say which way it went and why"
    );
}
