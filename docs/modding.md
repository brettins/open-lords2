# Modding lords2

Implemented in `crates/l2-mods`. This document is two things at once: a
**reference for mod authors** (§3–§9), and the **design record** for why the
platform is shaped the way it is (§1–§2, §10–§13). If you are here to write a
mod, start at §3 and treat everything before it as optional.

```bash
cargo test -p l2-mods
LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-mods -- --nocapture
```

The install-dependent tests skip when `LORDS2_DIR` is unset, exactly as
`l2-formats`' corpus test does. No game data lives in this repository and none
is written by the tests.

---

## 1. Why this exists before the game does

OpenXcom's modding power did not come from a mod API bolted on late. It came
from being data-driven from the first commit: rules live in files, assets load
through an indirection, and *the base game is itself the first mod*. Nothing in
its engine can tell whether a value came from the original game or from
someone's rebalance, because there is no other path for a value to arrive by.

That property is nearly free to build and brutally expensive to retrofit. Every
constant written directly into simulation code is a place a mod cannot reach,
and moving it later means finding and rewriting every call site.

For this game the argument is sharper than usual, and it is
`docs/decisions.md` **C11**: *every* economic constant — tax, happiness,
rations, births, deaths, yields, wages — lives inside `Lords2.exe`, clustered
around `0x004D6300` and `0x004D8900`. The combat constants in `docs/battle.md`
§6.1 are the same: instructions, not data. `TROOPS*.ENG` set an expectation
that rules would be reachable as data and it is false. **Modding the 1996
game's balance means patching a binary.** The rule documents in
`crates/l2-mods/rulesets/core/rules/` are the first time those numbers have
been text.

---

## 2. The shape

Three pieces, and one asymmetry between them that is the whole design.

```
      ask for "Base1a.pl8"            ask for "unit.archers.armour"
              |                                     |
        +-----v------+                      +-------v-------+
        |    Vfs     |  last layer wins     |    Ruleset    |  every layer merges
        +-----+------+                      +-------+-------+
              |                                     |
   +----------+----------+                +---------+-----------+
   | longbows/Base1a.pl8 |  <- winner     | longbows/rules/*    |  <- applied last
   | base/Base1a.pl8     |                | base/rules/*        |
   +---------------------+                | (core, compiled in) |  <- applied first
                                          +---------------------+
```

**Assets shadow. Rules accumulate.**

A sprite has no partial form. There is no meaningful way to "override half of
`Base1a.pl8`", so a mod that provides one replaces it, and the layers below
become invisible for that name. A rule table *does* have a partial form —
changing one troop's armour is a complete, meaningful statement — so every
layer's rule documents are read and merged, and the last writer of each
individual key wins.

That asymmetry is why `Vfs` has both `resolve()` (one winner, for assets) and
`layer_entries_under()` (every layer's copy, for rules). It would be easy to
give rules the asset treatment by accident; the two accessors exist so that
choosing is explicit.

It is also why `mod.toml` and `rules/*.toml` are excluded from the shadowing
report, in one shared place — `package::is_platform_metadata`. Every mod has a
manifest, so without that exclusion every *pair* of enabled mods reports a
conflict over their manifests; and two mods that each ship a `rules/rules.toml`
are both read and both merged, so calling the second one the winner would be
exactly backwards.

---

## 3. Writing a mod

A mod is a directory containing `mod.toml`. Everything else about it — which
assets it replaces, which rules it changes — is discovered from its contents,
not declared. A declaration would be a second source of truth that goes stale
the first time someone adds a file and forgets.

```
longbows/
  mod.toml            the manifest; `id` is the only required field
  rules/*.toml        rule documents, merged with every other layer's
  Base1a.pl8          any other file shadows the same name in a lower layer
  art/Base1b.pl8      subdirectories work; the path is part of the name
```

The smallest useful mod is two files:

```toml
# longbows/mod.toml
[mod]
id = "longbows"
```

```toml
# longbows/rules/longbows.toml
[unit.archers]
armour = 4
```

That is complete and valid. The other ten troop types, the archers' other five
numbers, every battle, every economic constant and every asset in the game are
untouched and keep whatever the layer below gave them.

To replace a sprite, drop a file with the same name in the mod directory. No
declaration, no registration.

### Enabling it

```rust
let platform = Platform::builder()
    .base(data_dir)                          // the player's own data directory
    .mods_dir(data_dir.join("mods"))
    .enable(["longbows", "harder-sieges"])   // the player's order
    .build()?;

println!("{}", platform.report());           // organised by conflict
println!("{}", platform.effect_report());    // organised by mod
```

`crates/l2-mods/example-mods/longbows/` is a real mod that a test loads, so it
cannot drift away from what works.

---

## 4. `mod.toml` reference

| Field | Type | Meaning |
|---|---|---|
| `id` | string | **Required.** Letters, digits, `-` and `_` only. The layer name in every diagnostic |
| `name` | string | Display name. Defaults to `id` |
| `version` | string | `major.minor.patch`, trailing components optional. Defaults to `0.0.0` |
| `author` | string | Free text |
| `description` | string | Free text; a multi-line string is fine |
| `requires` | array of strings | Must be enabled, and loads first |
| `after` | array of strings | Loads after that mod *if it happens to be enabled* |
| `conflicts` | array of strings | Refuses to load alongside that mod |

`requires` entries accept a version requirement: `"core >= 1.2"`, `"ui ^0.4"`,
`"maps = 2.0"`, or a bare id for "any version". `^` means "at least this, below
the next major", and below 1.0 the minor component is treated as the breaking
one, as Cargo does.

Versions are **not full semver**: no pre-release tags, no build metadata. Those
exist to coordinate a published package ecosystem, and each one is a comparison
rule a mod author would otherwise have to learn.

`after` is the knob for compatibility patches, which must win over the thing
they patch without depending on it.

---

## 5. Rule document syntax

Rule documents are a **subset of TOML**. §10.2 says why it is a subset and why
it is hand-parsed.

Supported: comments; bare and quoted keys; dotted keys; `[table]`; `[[array of
tables]]`; basic and literal strings, single- and multi-line; integers (decimal
with `_` separators, and `0x` / `0o` / `0b`); floats; `true` / `false`; arrays;
inline tables.

Not supported, on purpose: **dates and times**. The game has no use for them,
and accepting them would mean threading a date type through the merge and every
accessor for nothing. A document containing one gets

```
bad.toml:1:5: dates and times are not part of the rule syntax
```

rather than a wrong parse, or a confusing "expected end of line" two characters
later.

Within a single document, setting a key twice or opening a table twice is an
error naming both lines. Across documents it is the merge, which is the point.

### Floats work, and you should not use one

The reader accepts `4.5`, and a rule holding it will merge and load. But the
simulation is integer-only — `docs/netcode.md` requires bit-identical results
across machines — so **nothing the simulation evaluates will ever read a
decimal**. Every one is reported:

```
rules holding a decimal, which the simulation cannot use:
  battle.three_bridges.some_ratio at floaty:rules/f.toml:2:14
```

Where the original game wants a fraction it uses a percentage and truncating
integer division, and so do we: `scale_percent = 84` means `x * 84 / 100`.

---

## 6. Merge semantics

Three rules. The count is deliberate: every extra rule is one more thing a mod
author must hold in their head to predict what two mods will do together.

1. **Table into table: recurse.** Keys only the newer document has are added.
   Keys both have are resolved one level deeper.
2. **Anything else: replace.** Scalars replace scalars. **Arrays replace arrays
   whole.** There is no element-wise array merge, because array elements have
   no identity — with `[1, 2, 3]` there is no principled way to say which
   element an override refers to. So a mod that changes one rung of a ladder
   restates the ladder, and a mod that changes one strength band restates all
   four. Anything meant for partial override is a table keyed by name instead,
   and the rulesets are written that way wherever it makes sense. One
   consequence to know when reading the counts in §7.3: a whole array is **one**
   rule, so the twenty-row birth-rate ladder counts as a single overridable
   thing and not as forty.
3. **`"$delete"` removes.** `"$delete" = ["knight"]` inside a table removes
   those keys before the rest of that table merges — so a mod can delete a
   sub-table and then define a fresh one in the same document, replacing rather
   than merging. `$` is not a legal bare-key character, so the directive has to
   be written quoted and can never collide with a key that means something in
   the game.

Every replacement of an existing value is logged with both origins. So are
deletions, and so are `"$delete"`s that named something absent — usually a typo,
or a mod written against a version of another mod that has since renamed
something. `MergeLog::contested_paths()` picks out the paths three or more
documents have fought over, which is a stronger smell than a single override.

---

## 7. Load order, and reading the report

### 7.1 How the order is decided

**The player's order is the primary signal.** In an overlay system the load
order *is* the conflict-resolution policy, so silently re-sorting the player's
list would be taking their decision away. `resolve_load_order` runs Kahn's
algorithm over the dependency edges, always taking the lowest remaining *user*
index — so the stated order survives wherever the constraints allow, and where
they do not, the smallest possible change is made.

Ties break on the user's index, so the same list plus the same manifests gives
the same order on every machine and every run. A bug report is worth nothing
otherwise.

Failures are checked in an order chosen so the message is about the real
problem: conflicts and missing or mismatched dependencies first, then cycles. A
cycle error names the loop (`a -> b -> c -> a`) rather than asserting one
exists.

The resolved layer stack, bottom first:

| Layer | Where it comes from |
|---|---|
| `core` | the engine's own rules, compiled into the binary (§10.3) |
| `base` | the player's data directory: assets, plus rules generated from their install (§9) |
| each mod | in resolved load order |

### 7.2 `report()` — organised by conflict

This is the "my two mods are fighting" view. Here it is for the example mod
over a ruleset seeded from a real Windows install — the exact output of
`LORDS2_DIR=... cargo test -p l2-mods --test corpus -- --nocapture`, which is
also how to check that this document still matches the code:

```
rules overridden:
  battle.isthmus.attacker.archers : base:rules/troops.toml:148:11 -> longbows:rules/longbows.toml:18:11
  battle.the_arena.attacker.archers : base:rules/troops.toml:334:11 -> longbows:rules/longbows.toml:21:11
  battle.the_arena.attacker.crossbows : base:rules/troops.toml:330:13 -> longbows:rules/longbows.toml:22:13
  battle.three_bridges.attacker.archers : base:rules/troops.toml:117:11 -> longbows:rules/longbows.toml:12:11
  battle.three_bridges.defender.archers : base:rules/troops.toml:130:11 -> longbows:rules/longbows.toml:15:11
  difficulty.hard.scale_percent : base:rules/troops.toml:98:17 -> longbows:rules/longbows.toml:29:17
  difficulty.very_hard.scale_percent : base:rules/troops.toml:102:17 -> longbows:rules/longbows.toml:32:17
  unit.archers.armour : core:rules/units.toml:92:10 -> longbows:rules/longbows.toml:56:10
  unit.archers.melee_attack : core:rules/units.toml:89:16 -> longbows:rules/longbows.toml:54:16
```

A corpus test asserts that **every** line of that is an *override* rather than
an addition. A mod that misspelt a battle id would otherwise merge silently,
creating a rule nothing reads; insisting the count matches the mod's leaf count
catches it.

The other sections appear when something is wrong:

```
assets provided by more than one layer:
  base1a.pl8: base -> longbows -> prettier (last wins)
rules a mod added rather than overrode (check the spelling):
  battle.three_brdiges.attacker.archers (set by typo, defined nowhere below)
rules holding a decimal, which the simulation cannot use:
  battle.three_bridges.some_ratio at floaty:rules/f.toml:2:14
mods worth a second look:
  oops: 'longbows.toml' is a .toml outside rules/, so it is treated as a file to shadow rather than as rules
mod 'aaa' loaded but changes nothing: everything it sets is overridden
```

`report().is_quiet()` is true when none of that fired.

### 7.3 `effect_report()` — organised by mod

This is the "my mod is not working" view, and it is the one to reach for first,
because that is the question people actually ask. The same load as above:

```
1. core: 283 rule(s) and 0 file(s) in force
     rule unit.archers.armour lost to longbows:rules/longbows.toml:56:10
     rule unit.archers.melee_attack lost to longbows:rules/longbows.toml:54:16
2. base: 922 rule(s) and 0 file(s) in force
     rule battle.isthmus.attacker.archers lost to longbows:rules/longbows.toml:18:11
     rule battle.the_arena.attacker.archers lost to longbows:rules/longbows.toml:21:11
     rule battle.the_arena.attacker.crossbows lost to longbows:rules/longbows.toml:22:13
     rule battle.three_bridges.attacker.archers lost to longbows:rules/longbows.toml:12:11
     rule battle.three_bridges.defender.archers lost to longbows:rules/longbows.toml:15:11
     rule difficulty.hard.scale_percent lost to longbows:rules/longbows.toml:29:17
     rule difficulty.very_hard.scale_percent lost to longbows:rules/longbows.toml:32:17
3. longbows: 9 rule(s) and 0 file(s) in force
```

That is a healthy load: the mod set nine rules and all nine are in force, and
the two layers below it each lost exactly what the mod took. An unhealthy one
looks like this instead:

```
3. aaa: HAS NO EFFECT - everything it supplies is overridden below
     rule battle.three_bridges.attacker.archers lost to zzz:rules/zzz.toml:2:11
4. zzz: 1 rule(s) and 0 file(s) in force
```

A rule a mod wrote can fail to reach the game in three quite different ways,
and the fix is different for each:

| What you see | What happened | What to do |
|---|---|---|
| `lost to <source>` | a later mod set the same rule | change the load order, or use `after` |
| `was added, not overridden` | the path exists nowhere below | check the spelling; you invented a rule nothing reads |
| `deleted by <source>` | a later `"$delete"` removed the table it was in | change the load order, or stop deleting it |
| `HAS NO EFFECT` | none of the above survived | some combination of the three |
| `supplied nothing` | the mod has a manifest and nothing else | check where your files are (§7.4) |

The second row is the one nothing else catches. A misspelt battle id merges
perfectly: the value is set, no conflict is reported, and the engine goes on
reading the rule it was always going to read. It is invisible without
per-value provenance, which is the main reason the value tree carries it.

It is reported for mods only. The `core` and `base` layers are the bottom of
the stack, so *every* rule they set is an addition and none of them is a typo;
printing a thousand of those would bury the handful that mean something.

### 7.4 Inspection — organised by mod, before loading

Inspection is deliberately separate from loading. Loading asks what the game
will run on once every layer has had its turn, and stops at the first error;
inspection asks what *this one mod* says with nothing else in the picture, and
reports everything it finds. That is the right shape for a tool you run against
your own work before shipping it.

`package::inspect(dir)` returns the manifest, the rule documents, the assets,
every rule path claimed, a **rules digest**, and warnings:

- a `.toml` outside `rules/` — legal, but nearly always a rule file in the
  wrong place, which loads with no error at all and does nothing;
- a non-`.toml` inside `rules/`, which the loader will skip;
- a mod with no rules and no assets;
- a rule holding a decimal.

Every enabled mod is inspected during `build()`, and the warnings appear in
`report()` under *mods worth a second look*, so a player who never runs an
inspection tool still finds out.

The rules digest identifies the *rules*, not the installation: the same mod at
a different path digests the same, and comments and whitespace do not count.

---

## 8. Ruleset reference

Four namespaces. Two are shipped with the engine and two are generated from the
player's own copy of the game.

| Namespace | Contents | Where it comes from | Consumed by |
|---|---|---|---|
| `unit.<id>` | battle combat constants | shipped: `rulesets/core/rules/units.toml` | `l2_sim::TroopTable` — **live** |
| `kingdom.*` | the economy | shipped: `rulesets/core/rules/kingdom.toml` | `l2_kingdom::tables::Tables` — **live for the economic core**; see §11 |
| `troop.<id>`, `difficulty.<id>`, `battle.<id>` | the skirmish army table | generated from `TROOPS*.ENG` and `BATTLES.ENG` | `l2_mods::TroopRules` |

Read the shipped documents. They are plain, commented and human-readable, they
sit next to the mods on disk after `core::write_to`, and they are the fastest
way to learn the vocabulary — which is exactly how people learn to mod
OpenXcom.

### 8.1 `unit.<id>` — battle combat constants

Eleven types: `peasants`, `crossbows`, `maces`, `swords`, `pikes`, `archers`,
`knights`, `catapults`, `siege_towers`, `rams`, `oil`.

| Key | Type | Range | Meaning |
|---|---|---|---|
| `index` | integer | fixed | Position in the eleven-slot order. Checked; it is the array index the simulation uses |
| `melee_attack` | array of 4 integers | `0..=65535`, non-increasing | Damage per blow by strength band, best band first |
| `recovery` | integer | `1..=65535` | Ticks between blows suffered. **This is melee defence** |
| `heavy_blow` | integer | `0..=65535` | The once-per-exchange blow, and large |
| `armour` | integer | `0..=65535` | Flat subtraction, **missiles only**; never read in a melee exchange |
| `exchange` | integer | `0..=65535` | Blows before attacker and defender swap roles |
| `hits_per_casualty` | integer | `1..=65535` | Damage absorbed before one man dies |

Three things about this table trip people up:

- **There is no defence stat.** A figure's melee defence *is* its `recovery`:
  the interval between blows it suffers is its own recovery counter, so a
  slow-recovering figure is struck rarely. That is why pikemen, whose recovery
  is the longest of all, are what the manual calls good defenders.
- **`armour` is missiles only.** Raising it does nothing whatever to a melee.
- **The bands run best first**, and a rising row is refused rather than
  accepted, because a weakened figure hitting harder is not a rebalance.

`recovery` and `hits_per_casualty` are refused at zero: the first is the
interval a figure can be struck on and the second is a divisor, and both read
as a hang rather than as a change.

**Not modifiable:** which types are siege engines. That decides whether the
melee code path runs at all rather than how hard it hits, so it is structure
rather than balance and stays in the engine.

### 8.2 `kingdom.*` — the economy

| Table | Rows | Keys |
|---|---|---|
| `kingdom.food` | — | `dairy_per_head`, `food_per_head`, `food_per_sack` |
| `kingdom.grain` | — | `yield_per_sack`, `max_sacks_per_field`, `labour_divisor_advanced`, `labour_divisor_basic` |
| `kingdom.field` | — | `progress_max`, `reclaim_per_season` |
| `kingdom.event` | — | `population_cap_pct`, `first_year` |
| `kingdom.season.<id>` | `none`, `spring`, `summer`, `autumn`, `winter` | `index`, `death_rate`, `dryness` |
| `kingdom.happiness` | — | `ration_slope`, `ration_offset` |
| `kingdom.ration.<id>` | `none`, `quarter`, `half`, `normal`, `double`, `triple` | `index`, `divisor`, `multiplier`, `health_delta` (5 numbers) |
| `kingdom.health` | — | `band_ladder` (5 numbers) |
| `kingdom.health.band.<id>` | `diseased`, `sick`, `average`, `good`, `perfect` | `index`, `happiness`, `death_rate` |
| `[[kingdom.population.birth_rate]]` | 20 | `up_to`, `percent` |
| `[[kingdom.population.happiness_factor]]` | 5 | `below`, `percent` |
| `kingdom.weather.<id>` | `frost`, `drought`, `sunny`, `cloudy`, `storms`, `flooding` | `index`, `herd_pct` |
| `kingdom.castle` | — | `starting_type` |
| `kingdom.castle.type.<id>` | `none` … `royal_castle` | `index`, `tax_base`; and for types 1–5 `tax_bonus_pct`, `cost_wood`, `cost_stone`, `workforce`, `garrison_cap`, `free_archers` |
| `kingdom.job` | — | `count`, `iron_mining`, `stone_quarrying`, `wood_cutting`, `blacksmith`, `grain_farming`, `castle_building` |
| `kingdom.commodity.<id>` | `wood`, `iron`, `weapons`, `stone` | `index`, `job`, `divisor`, `base_efficiency` |
| `kingdom.weapon.<id>` | `crossbow`, `mace`, `sword`, `pike`, `bow`, `armour` | `index`, `wood`, `iron` |
| `kingdom.good.<id>` | `none`, `grain` … `mail` | `index`, `sell_price` |
| `kingdom.wages` | — | `divisor_human`, `divisor_ai` (3 numbers), `bankrupt_stage_max` |
| `kingdom.ai` | — | `grant_*_per_difficulty`, `grant_min_*` |
| `[[kingdom.ai.gold_grant]]` | 5 | `lord`, `by_difficulty` (4 numbers) |
| `[[kingdom.score.gold_bracket]]` | 4 | `at_least`, `points` |
| `[[kingdom.score.weight]]` | 6 | `offset`, `numerator`, `denominator` |

Two conventions run through it:

- **Named rows carry the array index they stand for, and it is checked.** The
  names are for you; the indices are what the simulation uses. Renaming
  `winter` would be harmless; moving winter to slot 2 would not be, and this is
  the line between the two.
- **Ladders are arrays of tables, tried in order, and the last row is the
  catch-all.** Arrays replace whole (§6 rule 2), so changing one rung means
  restating the ladder. The `happiness_factor` catch-all's `below` is written
  as `2147483647` and is never actually read.

Several values are refused at zero because they are divisors in the original's
arithmetic: `kingdom.ration.*.divisor`, the two grain labour divisors,
`kingdom.commodity.*.divisor`, `kingdom.wages.divisor_human` and `divisor_ai`,
and `kingdom.score.weight.*.denominator`.

Three things worth knowing before you rebalance:

- **`kingdom.grain.max_sacks_per_field` is 10, and the printed manual says 5,
  twice.** The manual is wrong; `docs/decisions.md` C10 records it, along with
  a second case where it is also wrong.
- **The labour divisors run backwards from how they read.** The *smaller*
  divisor demands *more* labour, so turning Advanced Farming off makes sowing
  harder.
- **`kingdom.ai.gold_grant` rows 1–3 are all zeros and that is a gap, not a
  rule.** Only the endpoints are documented. Row 0 being zeros *is* a rule: the
  human's lord byte is 0, so the human gets nothing.

`kingdom.castle.type.*.workforce` is one number in the ruleset and a pair in
the binary — the table at `0x004D89E8` holds two ints per castle level and both
hold the same value in all five rows. A mod author should not have to reproduce
an oddity of a 1996 memory layout, so the format holds one scalar and the
loader writes it into both columns. A test asserts the two columns are still
identical, because the day someone works out what the second one means, this
format stops being able to express it.

### 8.3 `troop.<id>`, `difficulty.<id>`, `battle.<id>` — the skirmish armies

Generated from `TROOPS*.ENG` (§9). The ids are the same eleven as `unit.<id>`.

| Path | Keys |
|---|---|
| `troop.<id>` | `column` (0–10), `abbrev`, `name`, `siege_engine` |
| `difficulty.<id>` | `order`, `scale_percent` (0–1000) |
| `battle.<id>` | `index`, `name`, `defensive_advantage` (0–10) |
| `battle.<id>.attacker`, `.defender` | one key per troop id; absent means zero |

Siege columns (`catapults`, `siege_towers`, `rams`, `oil`) are limited to
`0..=9`, which is the original's own clamp. The original clamped silently
because it was reading a text file it could not validate; we validate instead,
so a mod that asks for 40 catapults gets

```
silly:rules/silly.toml:2:13: rule 'battle.three_bridges.attacker.catapults': 40 is outside 0..=9
```

rather than a silent 9 and an afternoon wondering why.

`difficulty.<id>.scale_percent` is applied as `x * percent / 100` with
truncating integer division, to the seven non-siege columns only. That is
exactly what the original does — and the five percentages exist nowhere in the
1996 game but as instructions, which is §1 in miniature. The original reads all
five difficulty rows out of the file and then throws four of them away,
re-deriving them from Normal.

### 8.4 Why `unit.<id>` and `troop.<id>` are separate

They describe the same eleven soldiers, and they are deliberately kept apart,
because they have different authorities and different lifetimes:

- `troop.<id>` says where a type sits in the eleven columns of `TROOPS*.ENG`
  and what each skirmish battle starts with. It is **generated on the player's
  machine** from their own copy of the game, and does not exist at all on an
  install that never shipped those files.
- `unit.<id>` says what a soldier does when it swings. It is **ours**, shipped
  with the engine, and present on every install.

Merging them would produce one file that is half generated and half authored,
and a mod author could not tell by looking which half regenerating would
overwrite. The ids are the same in both, and a test keeps them so.

---

## 9. The base ruleset is partly shipped and partly generated

OpenXcom ships its rulesets. We can only ship half of ours, and the line
between the halves is the only rule that decides where a future ruleset
belongs:

- **Generated, where the numbers are the player's.** A `rules/troops.toml`
  holding the 3,885 numbers of `TROOPS.ENG` is a transcription of a shipped
  game file, and `CLAUDE.md` rule 1 keeps game data out of this repository. So
  it is generated on the player's machine, from the copy of the game they
  already own, into their own data directory. Only the generator (`seed.rs`)
  is version-controlled.
- **Shipped, where the numbers are ours.** `units.toml` and `kingdom.toml` were
  read out of `Lords2.exe` by `docs/battle.md` and `docs/kingdom.md` and
  already live in this repository as Rust constants. Writing the same numbers
  as `.toml` adds nothing that was not already committed.

Real excerpts from a `troops.toml` generated off the Windows install — 1,189
lines, 16 KB:

```toml
# Skirmish army rules, generated from TROOPS2.ENG.
#
# Generated, not authored: regenerating overwrites it. To change a
# number, put your change in a mod instead - a mod file with just the
# lines you want different is merged over this one.

[troop.archers]
column = 5
abbrev = "Ar"
name = "Archers"
siege_engine = false

[difficulty.very_hard]
order = 4
scale_percent = 84

[battle.three_bridges]
index = 0
name = "Three Bridges"
defensive_advantage = 5

[battle.three_bridges.attacker]
peasants = 250
crossbows = 75
maces = 75
swords = 75
pikes = 125
archers = 200
knights = 0
catapults = 0
siege_towers = 0
rams = 0
oil = 0
```

Battle ids come from `BATTLES.ENG`, slugged: `Three Bridges` →
`three_bridges`, `Expanded Keep` → `expanded_keep`. Readable ids matter more
than they look — they are what a mod author types, and `battle_17` would make
every mod file unreadable.

On decoding: `l2-formats` owns decoders and keeps them. What `seed.rs` does is
not one. `TROOPS*.ENG` is plain text and the engine's own reader
(`FUN_0042AC0C`) is "skip to the first `*`, then take every decimal token and
ignore everything else" — ten lines. When `l2_formats::eng` lands,
`parse_troops_eng` should become a call into it and `seed.rs` should keep only
the rule-generation half.

---

## 10. Design record

### 10.1 Why the rule format is TOML rather than RON

RON is Rust's data shape written down: tuples, enums with payloads, nested
maps. That is a good fit for serialising a Rust type and a poor one for a human
editing a table of statistics. Compare:

```ron
(units: { "archers": (armour: 4) })
```
```toml
[unit.archers]
armour = 4
```

The TOML version also happens to *be* the override: a mod file containing
exactly those two lines and nothing else is a complete, valid mod. In RON the
same change requires restating the enclosing map structure, which is precisely
the "restate the whole table" problem composable mods exist to avoid. TOML's
table headers make partial documents the natural thing to write rather than a
trick.

Secondarily: a mod author has probably met TOML. It is what `Cargo.toml`,
`pyproject.toml` and half the tools on their machine already use.

### 10.2 Why there is no `serde` and no `toml` crate

**Merging needs a generic value tree anyway.** You cannot merge two documents
into `#[derive(Deserialize)]` structs — the second document does not contain
most of the fields, so every field would have to become `Option<T>` and every
consumer would have to unwrap. The workable design is to merge `toml::Value`
trees and deserialise once at the end. But if the merge operates on a generic
tree regardless, `#[derive]` is contributing only the last step, and that step
is the easy one.

**And the tree we want carries provenance.** `toml::Value` does not record
which file and line each scalar came from. Without that, "two mods set the same
rule" is a silent last-write-wins. With it, it is a diagnostic naming both
files — and, less obviously but more usefully, it is what makes the "added, not
overridden" typo check in §7.3 possible at all.

**The cost side.** `serde` + `toml` is about 14 crates once `serde_derive`
pulls in `syn`, `quote` and `proc-macro2`; it adds a proc-macro build step; and
it is a supply-chain surface on a project whose licence position is already
delicate (`docs/decisions.md` D5a). The reader is ~680 lines and has tests
covering the syntax and every error message.

**What it cost us.** A hand-written parser is a hand-written parser: it can
have bugs a widely-used crate would not, and it will not track TOML spec
revisions. If the ruleset ever needs to round-trip through other tooling, that
changes the calculation. The mitigation is that the supported subset is small,
documented, and tested against its own error messages rather than only its
successes.

### 10.3 Why the core rules are compiled into the binary

A rules directory that can go missing is a rules directory that can go missing
*on one peer only*, and two lockstep peers running different rules is exactly
the failure `docs/netcode.md` exists to prevent. Compiled in, the engine's own
rules are as present as the code. `core::write_to` still drops a readable copy
next to the player's mods, because the first thing a would-be author should be
able to do is open the base rules and read them.

Both documents are *rendered* from the tables they describe, and
`tests/core.rs` checks both directions: loading the shipped `.toml` must
reproduce `TroopTable::DEFAULT` and `Tables::DEFAULT` exactly, **and** the
shipped text must be byte-identical to what the renderer produces now. Neither
half alone is enough — the first would let a Rust constant change with nobody
noticing the document had gone stale, the second would let the document and the
renderer agree on something the loader cannot read.

Regenerate after a deliberate change to either table:

```bash
L2_MODS_REGENERATE=1 cargo test -p l2-mods --test core
```

### 10.4 Why the simulation crates do not depend on this one

`l2-sim` and `l2-kingdom` expose plain table types — `TroopTable`, `Tables` —
and cannot read a file, parse a document, or tell that mods exist. Building one
out of a ruleset happens here, and the dependency points that way and never
back.

A simulation that loads its own rules is a simulation that can fail to load,
and two lockstep peers that fail differently desync. Keeping the loader out
also keeps `l2-sim` dependency-free, which is its own standing requirement.

The seam in `l2-sim` is worth stating precisely, because it is what makes
"data-driven" mean something rather than being a type that merely exists.
`Battle::with_troops(table)` takes a `TroopTable`; `Battle::add` builds each
figure through it; and **each figure copies its own row in at construction**.
Melee and missile code reads `figure.stats`, never a constant. So whatever
table built the figures is the table the whole battle runs on — and a table
cannot change under a battle already in progress, which a lockstep peer very
much needs.

---

## 11. What is data-driven today, and what is not

Stated plainly, because the difference between "loadable" and "wired" is
exactly the kind of thing a modding document is tempted to blur.

| | Status |
|---|---|
| `unit.*` → `l2_sim::TroopTable` | **Live.** `Battle::with_troops` runs the simulation on the loaded table, and a test asserts a modded table changes the outcome of a duel |
| `troop.*`, `difficulty.*`, `battle.*` → `l2_mods::TroopRules` | **Live.** Typed, range-checked, and the difficulty curve reproduces the original's arithmetic |
| Assets, through the overlay | **Live**, and proven against a real install's 291 sprite files |
| `kingdom.*` → `l2_kingdom::tables::Tables` | **Live for the economic core.** `Kingdom::with_tables` runs the season pipeline on the loaded table, and a test starts at a `.toml` and ends at a different number of sacks in a barn |

The kingdom half was, for a long time, loaded and validated and then ignored,
and this section said so. It no longer is: `Kingdom` carries a `Tables` and
around thirty rule functions take `&Tables` and read it. What a mod now
genuinely reaches:

food and dairy; the whole ration ladder and its happiness slope; the health
delta grid and the band ladder; the birth ladder and the happiness factor;
deaths by health band and by season; the random-event population cap and first
year; grain yield per sack, sacks per field and the sowing labour divisors;
field reclamation; the herd's weather swing; castle tax bases, costs,
workforce, garrison caps and free archers; weapon costs; the industry job and
divisor columns; wage divisors; the AI's gold grants and resource floors; and
the score weights and gold brackets.

**What is still a constant, and honestly so.** Array *sizes* are structure, not
balance — the nine job slots, the six ration levels, the eleven troop types —
and a ruleset that changed one would be describing a different simulation
rather than a different game. Three real rules are also still compiled in
because `Tables` does not carry them yet: the **ale** happiness ladder, the
**army-raising** happiness cost, and the efficiency ramp's ceiling. So are the
AI's four tax ladders and its personality table. Each is a `const` in
`crates/l2-kingdom/src/tables.rs` with its address and its evidence beside it;
adding one to `Tables` and to `kingdom.toml` is now a small change rather than
a structural one, because the seam it would arrive through already exists.

`Tables::DEFAULT` is assembled *from* those constants, which remain the source
of truth, and a test checks the gathered value against the free functions over
their whole domain — so the document and the constants cannot drift.

Also still hardcoded, deliberately: which troop types are siege engines
(§8.1), the eleven-slot order itself, and the mapping from a rule id to a
simulation slot.

---

## 12. Determinism, and what it asks of a mod

`docs/netcode.md` makes deterministic lockstep the architecture. `l2-net`'s
lobby hashes the resolved ruleset and refuses a peer whose hash differs, so
this is load-bearing rather than theoretical: an order-dependent merge would
turn into a refused session, or worse, a session that starts and desyncs an
hour later.

Three things could break it, and there is a test for each in
`crates/l2-mods/tests/determinism.rs`:

- **Iteration in hash order.** The value tree is `BTreeMap` throughout, and a
  test reads the crate's own source and fails if a `HashMap` or `HashSet`
  appears anywhere in it. Adding one has to be a decision somebody takes on
  purpose.
- **Filesystem enumeration order.** `read_dir` guarantees nothing and the two
  filesystems this project runs on do not agree. Every layer's documents are
  sorted by name before they apply, so `zz-final.toml` lands after
  `aa-first.toml` no matter when either was written.
- **Floats.** See §5.

### The two digests

Both are 64-bit checksums produced by `l2_net::Canonical` — the same encoder
and the same seed the per-tick desync checksum uses, so there is one byte
stream rather than two that could disagree with each other.

| | Covers | For |
|---|---|---|
| `Platform::digest()` | the merged rule values | "do our rules agree" — a diagnostic, and the per-mod identity in §7.4 |
| `Platform::session_digest()` | the merged rule values **plus the mod ids in load order** | `l2_net::Hello::ruleset_hash` — the handshake |

Neither includes **origins**. A mod installed at a different path is the same
*rules*, and a digest that said otherwise would refuse sessions that would have
run perfectly.

They differ on the **mod list**, and the reason is the interesting part.
`Platform::digest()` hashes what survived, so two load orders that merge to the
same numbers give the same answer — reordering two mods that never touch each
other leaves it alone, and reordering two that disagree changes it. That is the
right shape for "do our rules agree" and the wrong shape for a handshake,
because **the merged rule tree is not everything a mod can change**:

- A mod can replace an asset that is *simulation input*. A `.skr` battlefield
  is terrain, and terrain decides pathfinding. Not one value in the rule tree
  would move, and the two peers would desync on the first unit to walk.
- A mod can set rules a *future* build will read. Values this engine ignores
  are still a difference between the two installs, and one of the peers may be
  running the build that reads them.

So the handshake takes the strict answer. `docs/netcode.md` D-12 asks for
exactly that — agreement on the mod set and load order — and a false refusal,
two players with harmlessly different mod lists being told to fix it, is the
cheap failure. The expensive one is letting them in and desyncing an hour later
with a symptom that points nowhere.

**The asset gap is real and is not closed.** Hashing all 1,196 files at load
would cover it properly; hashing the mod list is the cheap proxy that catches
"you have a mod I do not" and misses "we have the same mod list but your copy
of one mod has a different `.skr` in it". Worth doing when mods are distributed
rather than hand-copied — the same trigger as signing (§13).

A known-answer test pins the byte stream over a document that will never
change. If that value ever moves, every previously recorded digest is wrong and
a peer on an older build gets refused for no reason — which is why it is pinned
rather than computed, the same argument `l2-net` makes for freezing its hash
and its PRNG.

---

## 13. What is deliberately left out

**Scripting, event hooks, and any way for a mod to run code.** This is the big
one. Every question a scripting layer answers — which events fire, what a
script may touch, how errors are contained, what the sandbox is — should be
decided against a real simulation. Deciding now would be guessing, and a
guessed API is worse than none because it has users. The data-driven half is
the half that is expensive to retrofit; the scripting half genuinely is not.

**An archive format.** A `.l2mod` file buys one thing — a single file to send
someone — and every operating system already ships a zip tool that does exactly
that to a directory. Against it: the platform has to read directories anyway,
because that is what a mod under development is, so an archive would be a
*second* loading path to keep working and in step, and a compression dependency
is a thing `docs/decisions.md` D5a says to think twice about here. Revisit when
mods are distributed rather than hand-copied — the same trigger as signing.

**Rules for anything but units, the skirmish armies and the kingdom economy.**
Terrain properties, building costs and sprite metadata get schemas when the
code that consumes them exists; inventing them now would produce a vocabulary
nothing validates.

**Mod-supplied assets in new formats, and asset *patching*.** A mod can replace
`Base1a.pl8` wholesale. It cannot yet add a frame to one, or supply a PNG for
the engine to convert. Both are real wants; both need the renderer to have
settled first.

**Localisation.** `L2.eng`'s `(group, index)` string addressing is documented
in `docs/formats/eng.md` and is obviously the next rules-shaped thing to lift —
the German and French builds swapped exactly this file. It is left out because
it wants its own schema decision (do string ids get names, or stay numeric?)
and that decision is better made when something is displaying strings.

**Enable/disable persistence.** The load order comes in as a list from the
caller. Where that list is stored, and the UI for reordering it, belong to
whatever ends up owning user settings.

**Signing and distribution checksums.** The rules digest exists for lockstep,
not for trust. Signing is premature while mods are hand-copied.

---

## 14. Where things are

| | |
|---|---|
| `src/vfs.rs` | overlay filesystem, case folding, shadowing diagnostics |
| `src/reader.rs` | the TOML-subset reader, with per-value origins |
| `src/value.rs` | the value tree, `Origin`, `Spanned` |
| `src/merge.rs` | merge rules, `$delete`, the override log |
| `src/ruleset.rs` | merged ruleset, typed accessors, range checks, per-document claims |
| `src/modmeta.rs` | manifests, versions, discovery, load order |
| `src/package.rs` | what a mod is on disk; inspection and warnings |
| `src/effect.rs` | per mod: what took effect and what did not |
| `src/digest.rs` | the ruleset checksum |
| `src/core.rs` | the engine's own ruleset, compiled in |
| `src/units.rs` | `unit.*` → `l2_sim::TroopTable` |
| `src/kingdom.rs` | `kingdom.*` → `l2_kingdom::tables::Tables` |
| `src/troops.rs` | `troop.*`, `difficulty.*`, `battle.*` → `TroopRules` |
| `src/seed.rs` | generating the base ruleset from the player's install |
| `src/lib.rs` | `Platform`, the builder, `Report` |
| `rulesets/core/rules/` | the shipped documents, rendered from the tables |
| `example-mods/longbows/` | a real mod, loaded by a test |
| `tests/core.rs` | the shipped documents cannot drift, in either direction |
| `tests/determinism.rs` | §12 |
| `tests/effect.rs` | the three ways a rule fails to arrive |
| `tests/package.rs` | packaging and inspection |
| `tests/corpus.rs` | install-dependent checks; skips without `LORDS2_DIR` |
