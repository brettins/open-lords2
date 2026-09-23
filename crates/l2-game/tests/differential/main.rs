//! Everything else in this workspace checks either a set of **names** or a
//! block of **static data**. `crates/l2-game/tests/arms.rs` asserts set
//! equality between `docs/arms.json` and the `// arm:` markers in the source.
//!
//! `crates/l2-sim/tests/oracle.rs` opens `Lords2.exe` at its fixed `0x400000`
//! base and compares three battle tables byte for byte — a real oracle, and a
//! **static** one: those bytes were the same before the game was ever run.
//!
//! The second number is the one that means anything. Both are asserted, and the
//! ablation below is what proves the difference between them is real rather
//! than rhetorical: with our End Turn deleted the first number is still 66 %
//! and the second is 0 %.
//!
//! Every exclusion here is one thing: the original draws from two 31-bit LFSRs
//! (`FUN_00404A46`) whose state is **not among the blocks `Save_Write`
//! stores**, and we draw from `l2_net::Pcg32`. `l2_game::scenario::SEED`'s own
//! documentation says it — *"the weather and the event deck will not follow the
//! original's from this save; everything that does not draw a random number
//! will"* — and this file is the first thing that puts a number on the second
//! half of that sentence.
//!
//! # The ablation, and why the raw percentage is the wrong number
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
//!    * `Turn_BeginPlayersTurn` (`0x0049B6D3`) writes `aiStep = 0` into **every**
//!      realm — 999 only for one at zero strength — and `AI_RunTurnStep`'s
//! (`0x0049A581`) `isHuman` test guards the fourteen handlers and the
//!      counter's increment, **not the step-0 prologue above them**. That
//!      prologue is `Realm_RecountStrength(r); Realm_UpdateTotals(r);
//!      offerPending = 0; aiStep = 1;` and it runs for the human. We ran only
//!      the first of the four, and `Realm_UpdateTotals` (`0x0049D1E0`) is the
//!      only thing in the binary that fills the score inputs.
//!
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
//!
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
//!
//! 2. **Neutral counties buy grain and ours could not. Fixed — C149.** In
//!    `battle 4->5`, unowned counties 1 and 3 went 71 → 121 and 57 → 103 while
//!    ours went 71 → 71 and 57 → 53: **50 sacks** a county a season, the first
//!    measurement of a seam that had been documented and left unbuilt.
//!
//! `l2_kingdom::ai_farm::NoMarket` said *"
//!    style's opening shopping cascade is refused."* County `+0x1A4`, the stall
//! gate, is **non-zero on every fixture county holding a merchant**, and the
//!    purse at `+0x1F4` carries 186, 297 and 260 on county 1 and 195, 316 and
//! 294 on county 3. The stall was there and the money was there; nobody had
//!    opened the file. Third of the class after `NOT SIMULATED` (C120), the font
//!    comment (C107) and `INDUSTRY / NOT DRAWN` (C136) — **a placeholder that
//!    explains itself is asserting a finding, and nothing checks it.**
//!
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
//!
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

mod chain;
mod compare;
mod fields;

use compare::{clock, run, Row};

struct FixturePair {
    label: &'static str,
    before: &'static str,
    after: &'static str,
}

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
    // The first pair played with `g_optAdvancedFarming` set; every pair above
    // it has the option off.
    FixturePair {
        label: "advfarm 3->4",
        before: "advfarm-old_turn.sav",
        after: "advfarm-lastturn.sav",
    },
];

const SAME_TURN_TRIPLES: &[[&str; 3]] = &[
    ["battle-before.sav", "battle-during.sav", "battle-after.sav"],
    ["siege-lastturn.sav", "siege-sieging.sav", "siege-aftersie.sav"],
];


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
///
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
///
/// The swing change that came before it (C169) moved nothing.
///
/// **The hundred maces, settled.** `Ai_TradeForCounty` (`0x0049E39B`) is built
/// — `l2_kingdom::ai_farm::Market::trade_for_county` — and it is exactly what
/// realm 2's siege 13->14 rows were. The Knight's personality `+0x7C` orders a
/// hundred of the county's own weapon type once `+0x78`'s floor of 1,000 is
/// cleared; the county makes maces, good 11 quotes 10 + `Pct(10, 100)` = 20,
/// and 100 × 20 = 2,000 crowns. `realm.weapons.1` left the baseline and
/// `realm.gold` on realm 2 went **+2,005 → +5**, which is the standing
/// `realm.wages` (−5) divergence and nothing else; `realm.score` went +47 → −3.
///
/// **Moved up by the siege build records, from the numbers the test printed**:
/// `AGREE_TOTAL` 914 → 927 and `MOVED_AGREE_TOTAL` 271 → 278. Both siege pairs'
/// `realm.strength`, `realm.score` and `realm.wages` rows went, and realm 2's
/// `realm.gold` with them; siege 12->13 is now clean but for the unsimulated
/// `global.ai_lords`. The whole residual was one defaulted import:
/// `l2_scenario`'s `read_unit` wrote `engines: Default::default()` and
/// `siege_seasons_left: 0`, so `Siege_BuildTick` (`0x004A8507`) — which skips
/// any record with `ordered == 0` — saw nothing remaining and reported ready on
/// the first tick. Phase 2 launched the assault three seasons early and realm
/// 1's 43 men died against a 149-man garrison the original leaves standing.
/// Ablation: put the two defaults back and these thirteen rows return — run,
/// on this tree. `docs/decisions.md` C235.
///
/// **What is left on siege 13->14 is `realm.gold` realm 1, +500**, and it is
/// not the siege: it is the player's own half of the step, an armoury or castle
/// spend this pair cannot replay. `[I]`.
///
/// # `advfarm 3->4`, the first pair played with *Advanced Farming* on
///
/// `county.crop.1` (`+0x244`, row weather-draw-not-in-save) is the eighth: county
/// 2, 316 against our 474, through `grain_weather_change` — the same draw.
///
/// It arrives at 226 of 233, and the seven are one cause. Six are
/// `county.herd`/`county.herd_available` on counties 1, 2 and 3, each +5, and
/// each is that county's `herd_weather_change` (`+0x270`): our weather byte
/// came out *Sunny* where the original's is *Cloudy*, and
/// `HERD_WEATHER_PCT` is +5 % against 0 %. County 4 drew *Cloudy* on both
/// sides and its herd agrees exactly, 146.
///
/// **So it is the excluded `county.weather` leaking downstream, not a missing
/// rule**, and it could not show up before: `Weather_UpdateAll`
/// (`0x00449889`) flattens every county to *Cloudy* when the option is off,
/// which is every earlier pair. The dryness decomposes under our own
/// `update_all` on both sides — ours is `delta = 8`, chosen county 3,
/// neighbours 1, 2 and 4 (+8/+12/+16); the original's is `delta = 3`, chosen
/// county 1, neighbours 2 and 3 (+3/+4/+6) — the same rule over a different
/// draw from the two 31-bit LFSRs (`FUN_00404A46`). Not closable from a save.
///
/// **Every other Advanced Farming branch agrees on this pair, and each is one
/// the option decides.** `Fertility_Update` (`0x0044BFD5`) is forced to 0 with
/// the option off and here moves 42 -> 54, 24 -> 36 and 54 -> 72, all three
/// ours exactly. `Industry_EfficiencyRamp` (`0x0044F248`) is a flat 80 with it
/// off and county 4's four efficiencies are 100/100/100/0 on both sides.
/// Nothing was ported: the branches were already built, and this is the first
/// pair that runs them.
#[rustfmt::skip]
const BASELINE: &[(&str, &str, usize)] = &[
    ("advfarm 3->4", "county.crop.1", 1),
    ("advfarm 3->4", "county.herd", 3),
    ("advfarm 3->4", "county.herd_available", 3),
    ("advfarm 3->4", "global.ai_lords", 1),
    ("battle 3->4", "global.ai_lords", 1),
    ("battle 4->5", "global.ai_lords", 1),
    ("siege 12->13", "global.ai_lords", 1),
    ("siege 13->14", "global.ai_lords", 1),
    ("siege 13->14", "realm.gold", 1),
];

const COMPARED_TOTAL: usize = 1185;

const AGREE_TOTAL: usize = 1172;

const MOVED_TOTAL: usize = 352;

const MOVED_AGREE_TOTAL: usize = 344;


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
    eprintln!("\nthe one-End-Turn pairs:\n{}\n", lines.join("\n"));
}

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
                 crates/l2-game/tests/seam/main.rs both want re-reading",
                seen[0].0
            );
        }
        eprintln!(
            "{:?} are all turn {} season {} year {} - one turn, not a before/after pair",
            triple, first.0, first.1, first.2
        );
    }
}

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
        rows.sort_by_key(|row| (row.kind, -row.delta().abs(), row.path, row.record.clone()));
        for row in rows {
            let e = by_kind.entry(row.kind.tag()).or_default();
            e.0 += 1;
            if row.moved() {
                e.1 += 1;
            }
            *found.entry((row.pair.as_str(), row.path)).or_default() += 1;
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
             Replace BASELINE in crates/l2-game/tests/differential/main.rs with:\n\n\
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
