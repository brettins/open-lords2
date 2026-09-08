# Adversarial review of `docs/plan.md`

Commissioned by the plan itself. Everything below was measured; every claim carries the
command or the file that produced it. Where I could not settle something, I say so.

---

## Verdict

**The central claim is half true, and the half that is false is load-bearing.** Five
crates do work, 727 tests pass, and nothing is joined up — that part reproduces. But *"the
gap is integration, not knowledge"* is false for three of the slice's five items: starting
a scenario, moving an army, and fighting a campaign battle each require subsystems that are
neither implemented nor reverse-engineered, and the plan's own workstream B is not
mechanical for the same reason.

**The plan should change, not be replaced.** A vertical slice is still the right first
move; the slice as scoped is not the one to build. Three specific changes: put persistence
**first** rather than last (the shipped `lastturn.sav` is already a readable dump of a
complete campaign, which unblocks slice item 1 for almost nothing), move the battle driver
out of `l2-view` before `l2-game` exists, and re-scope item 4 so campaign unit movement is
its own workstream rather than a hidden prerequisite.

---

## The strongest argument against the plan, made properly

Not "the UI is hard". The UI *is* hard, the plan says so, and it names the right mitigation.

The strongest argument is this: **the plan's cost model assumes the slice is assembly work,
and it is not, so the overrun will be misattributed.** Workstreams A and B are budgeted as
integration — wiring, threading, plumbing. If they were, the plan would be excellent. But
walking the slice list against the repo, three of the five steps cross code nobody has read:

* Step 1, *start on the campaign map of a shipped scenario*, needs new-game county
  initialisation. `Kingdom::start_new_game` sets the clock and nothing else
  (`crates/l2-kingdom/src/kingdom.rs:247`). The original's `Game_NewGame` (`0x00497CED`)
  calls twenty functions and all but four are still `FUN_`.
* Step 4, *move an army into a neighbouring county*, needs the campaign unit layer. No
  crate has one. `Unit_EnterOccupiedTile` — the function that decides that two units
  meeting is a battle — is 1,112 bytes and calls four unnamed functions.
* Step 4 also needs a *second* battlefield builder. `l2-view` builds a battlefield from a
  `.skr` terrain layer. A campaign battle uses `Battlefield_BuildRandom` (`0x0047AAA3`,
  3,599 bytes), which `docs/battle.md` §11 records as **not read**.

None of that makes a slice wrong. A slice is *supposed* to expose which unknowns are
load-bearing, and it has just exposed three before anyone wrote a line — which is the
process working. What is wrong is the budget and the sequencing that follow from calling it
integration. If A and B start on the stated scope, the first stretch is spent reverse-
engineering under a plan that says reverse engineering is a distraction, and the correction
log gets a seventeenth entry about it.

The counter-plan is not "do more RE instead". It is: **take the two steps that convert
unknown work into known work first, because both are nearly free and both are measured
below.**

---

## Specific holes

### 1. The shipped save is a complete campaign, and the plan calls reading it a stretch goal

`docs/plan.md` §C: *"Reading the original's `lastturn.sav` is a stretch goal — the container
arithmetic is already proven exactly."* The container arithmetic is the least of what is
proven. `g_saveBlocks` (`0x004DE960`) is a table of 225 `{address, length}` records naming
every block, and the tool that reads it already exists:

```
$ node tools/kingdom/savedump.js layout "F:/games/Lords of the Realm II"
save-block table at 0x4de960 in Lords2.exe
  0  0x00522f90    32768  @0        g_tiles
  3  0x0057bf00     2112  @84232    g_realms
  4  0x0053f9b0    13056  @86344    g_counties
  5  0x0052f0b0    63420  @99400    g_units
```

```
$ node tools/kingdom/savedump.js county "F:/games/Lords of the Realm II"
idx owner healthBand healthMeter happiness ... neighbourCount ... castleType ... grain herd
  1     5          3          67        72 ...              1 ...         3 ...     0   74
  2     0          3          67        77 ...              3 ...         0 ...   100   67
  ...
```

That is fourteen counties of real state: ownership, population, happiness, health,
**neighbour counts**, castle types, fields, grain, herd, weather, fertility. Plus five realm
records and 150 unit records. It ran instantly, on a file the install already ships.

So slice item 1 does not need `Game_NewGame` reverse-engineered at all. It needs a save
importer, and the block map an importer is built on is already read out of the binary and
already verified by the size invariant. **Workstream C is not the last thing the slice
needs; the import half of it is the first.**

### 2. `reproduction.rs` reproduces a model of the save, not the save

This is the sharpest finding, and it is C12 again in a different subsystem.

`crates/l2-kingdom/tests/reproduction.rs` is headed *"**The reproduction from the shipped
save.**"* Its model is *"fourteen counties, four owned by the human realm and ten
unowned"* (`OWNED: usize = 4`, owners assigned to ids 1..4), and it asserts happiness 72 for
ids 1–4 and 77 for ids 5–14.

The save's owner bytes, read directly at `county + 0x05`:

```
$ node -e "...b[86344 + i*0x300 + 5]..."
0,5,0,0,4,0,0,0,1,0,0,3,0,2,0,0,0
```

**Five counties are owned — 1, 4, 8, 11 and 13 — one by each of the five realms. Nine are
unowned.** The human realm (realm 1, `isHuman = 1`) owns exactly one county, number 8. The
happiness column of `savedump.js county` confirms it independently: 72 at counties 1, 4, 8,
11, 13; 77 at the other nine.

The test asserts counties 8, 11 and 13 are unowned and store 77. The file says 72.

The test passes, and it cannot fail, because it builds a kingdom from numbers quoted out of
`docs/kingdom.md` and then checks that kingdom against the same document — its own doc
comment says so: *"Nothing here needs a game install. The numbers are quoted from
`docs/kingdom.md`, not read from a file."* The **rules** it exercises are right (the 72/77
split, births 63/84, deaths 45, populations 435/456 all match the file exactly). The
**scenario** it claims to reproduce is not the one in the file.

`docs/kingdom.md` §9 and the errata in `crates/l2-kingdom/src/lib.rs` both say *"four owned
counties"*, and §4.1's `taxHapEmpire = 20` arithmetic is stated on that premise. The
conclusion of that errata note survives (only the negative part of `5 - rate` reproduces,
which is 0 either way) but the premise it is argued from does not.

Fix: make the test read the file when an install is present, field by field over all
seventeen records — `crates/l2-view/tests/install.rs` already has the skip-if-no-install
pattern to copy. This is method §2 step 4, *a property of the data rather than of our code*,
applied to the one crate that does not yet have one.

### 3. Workstream B's second half is not mechanical, and 14/17 of it is for excluded content

`l2-sim` has **no unit layer at all**:

```
$ grep -rniE '\bunit\b' crates/l2-sim/src --include=*.rs -c
battle.rs:0  figure.rs:0  lib.rs:0  melee.rs:1  missile.rs:0
movement.rs:0  pathfind.rs:0  troop.rs:1
```

`Battle` is a flat `Vec<Figure>` and `Battle::step` ticks melee only. The 17 order handlers
operate on units: they need `BattleUnit_Alloc`/`Create`/`Classify`/`Order`/`Reform`/
`Recentre`/`JoinMelee`, the eleven-way category ladder, the order lock, the 500-frame
re-target counter and three dispatch tables. "Wiring 17 handlers into `l2-sim`" is
therefore *building the battle unit subsystem*, then wiring.

And `docs/battle-ai.md` §6 opens: *"Fourteen of the seventeen handlers are siege-only."* The
three field handlers are `0048a9d2`, `0048acd2`, `0048b02b`; the other fourteen split 7/7
between siege attacker and siege defender (§11's reproduction commands list them). The slice
explicitly excludes sieges. So the stated work is fourteen handlers for the excluded half of
the game, on top of a subsystem the plan does not mention.

(Also: `crates/l2-view/src/battle.rs:35` still says *"twenty-five unit order handlers"*.
`docs/battle-ai.md` §10 corrected that to 25 slots / 18 functions / **17 real handlers**.
Cosmetic, but it is the kind of stale number that gets quoted into a plan.)

### 4. Workstream B's first half is real, and larger than "~30 function signatures"

Confirmed, and worse than stated. `Tables` is referenced **nowhere** in `l2-kingdom` outside
`tables.rs`:

```
$ grep -rn 'Tables' crates/l2-kingdom/src/*.rs | grep -v '/tables.rs'
(no output)
```

`Kingdom` does not hold one. So this is not only a signature change on ~30 free functions,
it is a change to the crate's central type.

Sizing it: 81 module-level `pub const`s, of which **61 are referenced outside `tables.rs`,
across 234 reference sites**. But the important number is the split:

```
in Tables: 40    not in Tables: 21
```

Twenty-one of the constants the simulation actually reads are **not fields of `Tables` at
all** — `AI_TAX_LADDERS`, `AI_TAX_LADDER_NEUTRAL`, `AI_FIELD_LADDER`, `AI_GOLD_GRANT_SMALL`,
`ALE_HAPPINESS_MAX`, `ALE_HAPPINESS_STEP_PCT`, `ARMY_HAPPINESS_COST`, `INDUSTRY_ORDER`,
`EFFICIENCY_MAX`, `EFFICIENCY_WITHOUT_ADVANCED_FARMING`, `RESOURCE_LIMIT_UNLIMITED` and
others. `crates/l2-mods/src/kingdom.rs` has no ruleset keys for them either.

So the stated goal — *"a modded `kingdom.toml` … actually take effect"* — is not reached by
threading `&Tables` through. It also needs `Tables` widened, `l2-mods`'s reader widened in
lockstep, and the round-trip and digest tests extended. Still worth doing and still mostly
mechanical, but it is a three-crate change, not a one-crate one, and the plan should say
which of the 21 are in scope. Deciding "these 21 stay hard-coded for now" is a fine answer;
discovering them halfway through is not.

### 5. The battle simulation lives in the renderer crate

`crates/l2-view/src/battle.rs` (`BattleRunner`) owns figure positions, facings, occupancy,
deployment, path consumption and the per-tick drive loop. `crates/l2-view/src/terrain.rs`
owns the `Battlefield` — the cell array the pathfinder reads. Both are simulation state.
Both sit in a crate that depends on `pixels` and `winit`:

```
$ cargo tree -p l2-view --prefix none | sort -u | wc -l
124
```

124 transitive crates. `l2-sim`, `l2-kingdom`, `l2-net`, `l2-mods` and `l2-formats` have
zero, and each carries a long Cargo.toml comment explaining that this is a *correctness*
property, not a preference, because a lockstep value stream may not be owned by somebody
else. Putting the thing that actually simulates a battle behind wgpu contradicts that
directly.

The plan says workstream A is *"the one piece with no prior art in the repo"* and *"the
place a wrong structural call is expensive"*. This is the wrong structural call that is
already made, and `l2-game` is the moment it becomes expensive to reverse: once the
application spine imports `l2-view` for its battle, moving `BattleRunner` means touching the
spine too.

Right now it is a file move plus imports. It should happen before A, not after.

### 6. Nothing that matters implements `Simulation`

```
$ grep -rn 'impl.*Simulation' crates/*/src crates/*/tests
crates/l2-net/tests/common/mod.rs:131:impl Simulation for ToySim
crates/l2-net/tests/replay.rs:210:    impl Simulation for Counter
```

Plus a `NetBattle` wrapper in `crates/l2-sim/tests/lockstep.rs`, which is real — it runs
`l2_sim::Battle` over a socket and checks bit-identity. But `Kingdom` does not implement
`Simulation`, and neither does `BattleRunner` — the thing that actually moves figures. So
*"the netcode syncs"* is true of a toy and of the melee-only `Battle`; it has never
synchronised the simulation a player would watch.

Good news attached: `Canonical` (`crates/l2-net/src/canonical.rs`) is already a
section-tagged deterministic encoder with `recording()` and `hashing()` modes, and
`state_snapshot` already exists because lockstep needs a whole-world snapshot for late join.
**Workstream C does not need a new save encoder.** Implementing `Simulation for Kingdom` is
the save format, the desync detector and the replay format in one change. Another reason C
is cheaper than the plan thinks and should not wait behind A.

### 7. The campaign map screen has no test, and uses its own projection

`crates/l2-view/tests/install.rs` has 7 tests: frame layout, knight sheets, walk offsets,
battlefield tile coverage, no-holes render, animation, figure visibility. **None touches the
campaign map.** `compose_map` (`crates/l2-view/src/main.rs:283`) is only ever checked by
looking at it.

It also invents its projection — `sx = (x-y) * TILE_W/2`, `sy = (x+y) * TILE_H/2` at the
zoom-2 tile size. The game's own mapping is exact and settled:
`docs/formats/maps-layers.md` §4, **`row = x + y + 1`, `col = (x - y + 64) >> 1`**, verified
cell-for-cell against a live process, with screen placement
`x = col*58 + (row odd ? 0 : 29) - 29`.

That is *good* news for slice item 2 — county picking is inverting a two-line formula, not a
research problem. It is bad news for two documents:

* `docs/decisions.md` "Open questions" still lists *"the exact tile → lattice mapping, whose
  best affine fit reaches only 72%"*.
* `docs/formats/maps.md` §3 and its open-questions list say the same.

`maps-layers.md` §4 explains exactly why the affine search stalled at 72%. Both stale
entries should be struck; otherwise the plan's risk register inherits a dead worry and
somebody re-solves it.

### 8. "Most of the remaining 90% is CRT and glue" is measurably false

The plan lists this as an untested assertion. It is testable in one command, and it fails.

```
$ per-64K-region function counts over tools/oracle/decomp/
00400000 total=216 unnamed=200      00480000 total=144 unnamed=61
00410000 total=136 unnamed=131      00490000 total=135 unnamed=105
00420000 total=139 unnamed=123      004a0000 total=171 unnamed=164
00430000 total=329 unnamed=327      004b0000 total=240 unnamed=212
00440000 total=358 unnamed=334      004c0000 total=246 unnamed=19
00450000 total=56  unnamed=54
00460000 total=174 unnamed=154      total 2452, unnamed 1971
00470000 total=108 unnamed=87
```

The CRT is **already named**, by Ghidra's function-ID signatures, and it is concentrated in
one region: `004c0000` is 246 functions of which 227 are `__CrtMemCheckpoint`, `__nh_malloc`,
`_atol` and friends. Part of `004b0000` is the rest of it plus the DirectX imports.
Everything below `0x004c0000` is game code: **2,206 functions, 1,952 of them unnamed.**

And directly:

```
functions referencing g_counties (0x0053f9..), g_units (0x0052f0..) or g_tiles (0x00522f9):
  unnamed = 418    named = 68
```

**418 unnamed functions touch the county array, the campaign unit array or the runtime tile
array.** They cluster in `0x00405000`–`0x00413000` (the map view and the panels) and
`0x004a0000` (164 unnamed — the AI turn handlers). That is the UI and the campaign layer:
precisely what workstream A is about to reimplement.

This does **not** mean the plan's conclusion is wrong. Not naming them is still probably
right, because we are writing our own interface rather than cloning the original's, and
`docs/method.md` §7 argues that well. But the *reason given* is false, and the plan's fourth
risk — "could be wrong in a way that bites during integration" — is now measured and comes
out badly rather than unknown. Rewrite the risk as what it actually is: **we are choosing to
invent the UI rather than reproduce it, and pixel-comparison against the original is off the
table for every screen we invent.**

### 9. Slice item 4 hides an entire subsystem

Walking "move an army into a neighbouring county, fight the battle, get the result back":

* **County adjacency** — exists. `County::neighbours` / `add_neighbour`
  (`crates/l2-kingdom/src/county.rs:217`), and the save carries real neighbour counts.
* **The turn seam** — exists and is the right shape. `Phase::ArmyMovement = 2` is documented
  as *"Army movement, including battle resolution"*, and `PhaseWait::Units(UnitKind)` is
  answered by the caller because *"phases 2, 3, 5 and 6 move units, which are not this
  crate's"* (`crates/l2-kingdom/src/kingdom.rs:291`). Genuinely good design; credit where
  due.
* **The army itself** — does not exist anywhere. `County` carries `army: i32`,
  `friendly_troops: i32`, `enemy_troops: i32` and nothing else; no type has troop
  composition. In the original the composition looks like seven `i16` at unit `+0x16C`,
  written from the realm table at `0x0053F6A0` by `FUN_004a9a9a`. *That offset reading is
  mine, from the decompiler, and it is a lead rather than a verified fact* —
  `docs/symbols.md` labels `+0x16C..` as "cargo", and the shipped save has **no armies at
  turn 1** (only the six merchants), so it cannot be cross-checked against the file.
* **Campaign movement** — does not exist. AI step 11 (`0x004A5667`) *"walk every army towards
  its target tile"* is a one-line description with no second source.
* **The battle trigger** — `Unit_EnterOccupiedTile` (`0x004658C1`), 1,112 bytes, calling
  `FUN_004a16f7`, `FUN_004a7158`, `FUN_004aa181`, `FUN_004a1ee1`. None named.
* **The battlefield** — `Battlefield_BuildRandom`, 3,599 bytes, recorded as unread.
* **Casualties back to the kingdom** — no path exists.

Seven items, three of them unread code. The mitigation is not to cut item 4; it is to split
it, and to make the first pass the version that does not need a map unit at all (A4 below).

### 10. Smaller things

* `docs/status.html` says 711 tests. Measured 727 (`cargo test --workspace`, summing
  `test result: ok. N passed`). It also says *"230 functions named"* while `symbols.json`
  now carries more. Neither is load-bearing, but the plan quotes 711 as evidence.
* Workstream B's mitigation — *"do it in one pass, alone, and land it before anything else
  touches the crate"* — is already violated. `l2-kingdom` changed twice during this review
  (`e486803`, plus six modified files in `git status` that were committed mid-review). That
  mitigation needs an owner and a lock, not a sentence.

---

## Alternatives, costed

Ordered by what I would actually do. Costs use `docs/method.md`'s table.

**A0 — Import `lastturn.sav` into `Kingdom`. Do this first.**
*Cost:* well under one subagent; the block table, the reader tool and the field map all
exist. *Buys:* slice item 1 outright, with real ownership, populations, happiness, castles,
adjacency and unit positions, and it removes twenty unnamed `Game_NewGame` callees from the
critical path. It also gives `l2-kingdom` its first input it did not write itself. The
single highest-value change to the plan.

**A1 — Turn `reproduction.rs` into a file-driven differential test.**
*Cost:* hours, on top of A0. *Buys:* the fix for hole 2, and the kingdom layer's first
oracle that is a property of the data rather than of our documentation — every field of
every county record checked against the bytes.
*Extension, and it needs the user:* `Save_RotateAndWrite` (`0x0049A453`) rotates
`safeturn.sav ← old_turn.sav ← lastturn.sav` at every turn boundary, so **one played turn
produces a before/after pair** and the whole `SEASON_PIPELINE` becomes differentially
testable against the original with no automation at all. Measured caveat: the install ships
exactly one save — `find "F:/games/Lords of the Realm II" -iname '*.sav'` returns
`lastturn.sav` only — so this asks the user to sit down and press End Turn once. That is not
"launching the game" in method §3's sense; it is a one-time human action that buys a
permanent oracle. Worth asking for.

**A2 — Move `BattleRunner` and `Battlefield` from `l2-view` into `l2-sim`, before `l2-game`.**
*Cost:* a file move, import fixes, and relocating `tests/install.rs`'s battle tests. Hours.
*Buys:* the simulation stops living behind 124 crates, `BattleRunner` becomes reachable from
a headless test and from `l2-net`, and `l2-game` gets a dependency graph that does not route
gameplay through the window. Gets more expensive with every day A exists.

**A3 — Re-scope workstream B.**
B1 (Tables) stays, widened to name which of the 21 non-`Tables` constants are in scope.
B2 becomes *"build the battle unit layer in `l2-sim` and wire the three field handlers"*;
the fourteen siege handlers go with the sieges, which the slice already excludes. Same
sentence, one third the work, and honest about the prerequisite.

**A4 — Re-scope slice item 4 for the first pass.**
Replace *"move an army into a neighbouring county"* with *"from the county screen, attack an
adjacent county"*: an abstract order, resolved at phase 2, no map unit, no tile pathing, no
trample or burn, no `Battlefield_BuildRandom` — use a `.skr` battlefield, which already
renders and is already tested. That still proves the whole spine the slice exists to prove
(county → order → battle → casualties → county → next season) and removes four unread
subsystems from the critical path. Then make the campaign unit layer its own workstream,
budgeted as reverse engineering, because that is what it is.

**A5 — What I would *not* do.**
Not "finish one subsystem to fidelity" — the crates are already deeper than the integration
around them, and another 200 kingdom tests against invented counties would make hole 2
worse, not better. Not "build the differential harness against a running original" — C16
settled that direction, and A1 gets a real oracle out of two files instead. Not more naming
for its own sake, despite hole 8: the 418 unnamed game-state functions are mostly the
original's UI, and we are not cloning it.

---

## What I checked and could not fault

Spend no more thought here.

* **The tests.** 727 pass, `cargo test --workspace`, 6.4 s warm. Names are specific and
  several are the kind that cannot be satisfied by accident —
  `every_graphic_index_a_skr_can_produce_fits_the_252_frame_tileset`,
  `the_frame_layout_accounts_for_every_frame_of_every_shipped_sheet`,
  `a_peer_running_different_rules_is_caught`. The post-C12 discipline is visible.
* **The claim that nothing is joined up.** Verified from the other side: no mouse handling
  anywhere (`grep -rn 'CursorMoved\|MouseInput' crates/l2-view/src` → nothing), no screen
  state machine, no save path, no county→battle path. The plan is not overstating this.
* **Dependency direction.** `l2-mods` → `l2-sim`/`l2-kingdom` and never the reverse, so the
  simulations cannot read a file or fail to load. The reasoning in
  `crates/l2-mods/Cargo.toml` is correct and the code honours it.
* **The zero-dependency policy on the deterministic crates**, and the specific reasons given
  for vendoring PCG32 and XXH64 rather than depending on them. Right call, well argued,
  correctly implemented — which is exactly why hole 5 matters.
* **`l2-kingdom`'s errata list** (`src/lib.rs`, thirteen numbered items). Unusually honest:
  it names where `docs/kingdom.md` is wrong, where the original is arguably buggy, and what
  is still a stub. Nothing in it overclaims. Hole 2 is a scenario-modelling error, not a
  rules error — every rule the reproduction test exercises matches the file.
* **The `TurnMachine`.** Seven phases in the original's order, phase 2 already labelled army
  movement including battle resolution, its wait condition delegated to the caller because
  units are not that crate's. The seam the slice needs already exists and is the right
  shape. This is the strongest evidence *for* the plan's central claim.
* **The campaign map does render from real data** — the six planes, the bank table at
  `0x004DA050`, back-to-front by `x+y`. It just has no test and its own projection.
* **`Canonical` as the save encoder.** Section-tagged, deterministic, already used by the
  checksum, the snapshot and the replay. C should build on it and needs nothing new.
* **Holding A for review while B starts.** Correct instinct, and the plan is right that A is
  the contested piece. The review's answer is that B is contested too.

---

## Reproduction

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test --workspace                                     # 727 pass, ~6 s warm

node tools/kingdom/savedump.js layout "F:/games/Lords of the Realm II"
node tools/kingdom/savedump.js county "F:/games/Lords of the Realm II"
node tools/kingdom/savedump.js realm  "F:/games/Lords of the Realm II"
# county owner bytes, read straight out of the save:
#   b[86344 + i*0x300 + 5] for i in 0..17  ->  0,5,0,0,4,0,0,0,1,0,0,3,0,2,0,0,0

grep -rn 'Tables' crates/l2-kingdom/src/*.rs | grep -v '/tables.rs'   # empty
grep -rniE '\bunit\b' crates/l2-sim/src --include=*.rs -c             # all zero
grep -rn 'impl.*Simulation' crates/*/src crates/*/tests               # tests only
cargo tree -p l2-view --prefix none | sort -u | wc -l                 # 124

for f in tools/oracle/decomp/*.c; do
  echo "$(basename $f .c) total=$(grep -c '^// ==== ' $f) unnamed=$(grep '^// ==== ' $f | grep -c '  FUN_')"
done
```
