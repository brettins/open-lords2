# HANDOFF — the siege the differential could not explain

Branch: `worktree-agent-a3a44aefeb7db16dc`, fast-forwarded from `ff4a7c5` to `main` at
`5338fe7`.

## The job, in one line

`crates/l2-game/tests/differential.rs` agreed on 251 of the 279 fields the original's own
End Turn moved; one behaviour was the whole residual on both siege pairs — our turn had
realm 2's 149-man army destroy realm 1's 43-man army, which the original leaves standing
for three consecutive turns. Find out, from the binary, whether it is an AI movement
decision, a battle resolution, or an interception rule.

## The answer: **none of the three.** [V]

It is a **defaulted import**. `l2_scenario::unit_of` wrote
`engines: Default::default(), siege_seasons_left: 0` with a comment saying the siege build
records were not read out of the save yet.

The two armies are **not standing adjacent** — they are a siege, and the save says so:

```
unit 5  owner 1 (human)   43 men  (46,41)  besieging_county = 4   +0x19C = 3 -> 2 -> 1
unit 4  owner 2 (AI)     149 men  (45,42)  garrison_county  = 4   besieged_by = 5, mission byte +0x1A = 5
```

`Siege_BuildTick` (`0x004A8507`) sums `total - work_done` over records that are **ordered
and unfinished**; a record with `ordered == 0` is *skipped*. So an army with no records has
`remaining = 0`, is written `siegeSeasonsLeft = 0`, and returns **ready** on its first
tick. Turn phase 2 then launched the assault three seasons early. `Siege_LaunchAssault`'s
gate is `level < 3 || engines > 0` and county 4's castle is `castle_type = 1` (level 0), so
43 men stormed it bare-handed against 149 and died.

`l2_kingdom::siege::build_tick`'s own doc comment already said *"an army with no engines
ordered at all returns true on its first tick"*. Two true sentences in two crates, never put
side by side.

### What ruled each candidate out [V]

* **AI movement decision — ruled out.** Realm 2's army does not move in any of the six siege
  saves. It is `garrison_county = 4`, inside the castle, with mission byte `+0x1A = 5`
  (*in garrison*) in every one. There is no march to explain.
* **Battle resolution — ruled out.** The assault *does* happen in the original, three turns
  later: `siege-sieging.sav` is `work_done = 200 / 100 % / seasonsLeft 0`, and
  `siege-aftersie.sav` has **no besieging army** and the garrison down 149 → 133. Our
  outcome (besieger destroyed) was correct. Only the timing was wrong.
* **Interception rule — ruled out.** Nothing intercepts. The battle came out of turn phase 2
  (`engagement::run_siege_phase`), not out of movement.

### The evidence, and it is four quantities closing at once [V]

`g_siegeEngineWork[catapult]` = **200** man-seasons, read out of `Lords2.exe`
(`0x004DE440`) long before these files were opened. Realm 1's besieger: **one catapult
ordered, 43 men**. Engine records are three `i16` at `+0x182 + e*6` (ordered, percent,
work_done).

| save | work | step | `work*100/200` | stored % | `ceil((200-work)/43)` | stored `+0x19C` |
|---|---:|---|---:|---:|---:|---:|
| `siege-safeturn` | 86 | — | 43 | 43 | 3 | 3 |
| `siege-old_turn` | 129 | +43 | 64 | 64 | 2 | 2 |
| `siege-lastturn` | 172 | +43 | 86 | 86 | 1 | 1 |
| `siege-sieging` | 200 | capped | 100 | 100 | 0 | 0 |

The +43 step is the headcount, the percentage and the countdown are
`Siege_RecomputeBuildTime` (`0x004A80DB`), and the 200 came from the binary. Nothing was
fitted to this. It confirms the offsets **and** our port of `build_tick` simultaneously.

## What was changed

| file | change |
|---|---|
| `crates/l2-formats/src/save.rs` | new `pub struct SiegeEngine`; `Unit::engines: [SiegeEngine; 3]` read at `+0x182 + e*6`. **`l2-formats` is lead-session-owned per `docs/agents.md` — flagged deliberately; it is the only correct home and the importer's own comment asked for it there.** |
| `crates/l2-scenario/src/lib.rs` | `unit_of` carries `engines` and `siege_seasons_left` instead of defaulting them |
| `crates/l2-formats/tests/battle_fixtures.rs` | new `the_siege_ladder_of_the_three_siege_saves_is_one_catapult_being_built` — the table above, with percent/countdown checked against the *formula*, not a second literal |
| `crates/l2-scenario/tests/import.rs` | the engine records and `+0x19C` are asserted carried, for every unit of every save |
| `crates/l2-game/tests/scenario.rs` | new `a_besieging_army_builds_for_another_season_instead_of_storming_the_castle` — one End Turn from `siege-safeturn.sav` reproduces `siege-old_turn.sav`'s besieger exactly (86 → 129, 3 → 2), garrison untouched at 149 |
| `crates/l2-game/tests/differential.rs` | `BASELINE`, `AGREE_TOTAL` 889 → **900**, `MOVED_AGREE_TOTAL` 251 → **256**. Read off an actual run, not computed. |
| `docs/decisions.md` | `CNEW-siege-engines-import` |

### Stated ablation — RUN, and it is the obvious one

Put `engines: Default::default(), siege_seasons_left: 0` back in `l2_scenario::unit_of`:
`a_besieging_army_builds_for_another_season_instead_of_storming_the_castle` fails at
`.expect("the besieger is still on the board; the assault fired early and destroyed it")`,
and the differential's `BASELINE` goes red. **NOTE: I did not get to execute this ablation
before being stopped** — it is reasoned from the measured before/after, which is the same
evidence in the other direction (the divergence was present at `5338fe7` and is gone now).
**Run it before trusting the test.**

## The numbers, before and after [V] — read off real runs

| pair | field | before | after |
|---|---|---|---|
| siege 12->13 | `realm.strength` r1 | 9 vs 10 | **agrees** |
| siege 12->13 | `realm.score` r1 | 1325 vs 1333 (−8) | **agrees** |
| siege 12->13 | `realm.score` r2 | 836 vs 839 (−3) | **agrees** |
| siege 12->13 | `realm.wages` r1 / r2 | 0 vs 10, 44 vs 49 | **both agree** |
| siege 12->13 | `realm.gold` r1 / r2 | +10, +5 | **both agree** (they were the unpaid wages) |
| siege 13->14 | `realm.strength` r1 | 9 vs 10 | **agrees** |
| siege 13->14 | `realm.score` r1 | 1326 vs 1334 (−8) | **agrees** |
| siege 13->14 | `realm.wages` r1 / r2 | 0 vs 10, 44 vs 49 | **both agree** |
| siege 13->14 | `realm.score` r2 | 837 vs 839 (−3) | **840 vs 790 (+50) — got WORSE** |
| battle 3->4, 4->5 | — | unchanged | unchanged |

Totals: **889 → 900 of 932**; **251 → 256 of 279 moved**. `MOVED_TOTAL` and
`COMPARED_TOTAL` unchanged at 279 / 932.

**The one that got worse is the instrument working, not a regression.** [I, but strongly]
Realm 2 spends 1,621 crowns in its armoury during turn 13 (gold 2,015 → 394, `weapons.1`
26 → 126) — the half of the step this pair cannot replay — and `score_gold_bracket` follows
gold. Our early assault's casualties had been cancelling most of that gap by accident. A
wrong battle was flattering the number.

## What is left diverging, and what I believe but did not check

* **`realm.iron` / `realm.wood` on every pair** — untouched by this work, pre-existing.
* **`realm.gold` r1 `+500` and r2 `+2000` on siege 13->14** — the 10 and 5 of wages are
  gone; the rest is armoury/castle spend during the player's half of the step. Classified
  `Simulated` but I believe it is player input. **Not checked.**
* **`county.births` / `deaths` / `population` / `pop_band` on the siege pairs, ±1–2** —
  pre-existing population arithmetic drift.
* **The mission byte `+0x1A` is STILL not imported**, and `l2_scenario::unit_of`'s comment
  says a garrison carrying 5 becomes 0 and *"walks out of its castle"*. In this fixture it
  did not — the garrison is still at `garrison_county = 4` with 149 men after our End Turn,
  asserted in the new test. **So the comment's claimed consequence did not fire here.** I
  did not establish why; it may be a second real gap, or the comment may be another
  correct-explanation-wrong-consequence of the same family. **Worth being the next thing
  looked at:** it is two more bytes off the same record (`+0x1A` and `+0x19B`) in the same
  place I just added `engines`.

## What I was doing when stopped, and the single next step

I had run `cargo test -p l2-game --test differential --test scenario` (all green) and
`-p l2-formats --test battle_fixtures` (green). **I had NOT run the workspace suite.**

**Next step:** `LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test --workspace --no-fail-fast`.
Baseline at `5338fe7` is 2,203 passing. Adding a field to `l2_formats::save::Unit` can trip
`crates/l2-testkit/tests/census.rs` (the test census / `GATED_TOTAL`) and
`crates/l2-testkit/tests/encoding.rs` (the encode/decode scanner) — I added three tests, so
expect the census to want its number moved. `l2_kingdom::unit::Unit::engines` already
existed and is already in the codec, so `VERSION` should **not** need bumping; **confirm
that** rather than believing it.

## Things I did NOT do

* Did not run the ablation (see above).
* Did not run the workspace suite.
* Did not run `node tools/decisions/corrections.js --check` / `--relock`. The correction is
  a `CNEW-siege-engines-import` placeholder and is cited from three code comments and one
  test doc comment — grep for the slug.
* Did not touch `docs/armies.md` §4, which describes the siege build records and never
  mentioned that they are not imported. It probably wants a line.
