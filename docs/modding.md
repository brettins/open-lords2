# Modding: the platform foundation

Implemented in `crates/l2-mods`. Nothing in the engine yet depends on it, which
is the point — this is the layer everything else will be built *on top of*.

Status: **69 tests, all green on a bare checkout.** The install-dependent ones
skip when `LORDS2_DIR` is unset, exactly as `l2-formats`' corpus test does.

```bash
cargo test -p l2-mods
LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-mods -- --nocapture
```

---

## 1. Why this exists before the game does

OpenXcom's modding power did not come from a mod API bolted on late. It came
from being data-driven from the first commit: rules live in files, assets load
through an indirection, and *the base game is itself the first mod*. Nothing in
its engine can tell whether a value came from the original game or from
someone's rebalance, because there is no other path for a value to arrive by.

That property is nearly free to build and brutally expensive to retrofit. Every
constant written directly into simulation code is a place a mod cannot reach,
and moving it later means finding and rewriting every call site. Doing it now
costs a crate. Doing it after the simulation exists costs the simulation.

So this is deliberately early, and deliberately a *foundation* — see §8 for
what was left out on purpose.

---

## 2. The shape

Three pieces, and one asymmetry between them that is the whole design.

```
      ask for "Base1a.pl8"            ask for "battle.three_bridges.attacker.archers"
              |                                          |
        +-----v------+                           +-------v-------+
        |    Vfs     |  last layer wins          |    Ruleset    |  every layer merges
        +-----+------+                           +-------+-------+
              |                                          |
   +----------+----------+                     +---------+---------+
   | longbows/Base1a.pl8 |  <- winner          | longbows/rules/*  |  <- applied last
   | base/Base1a.pl8     |                     | base/rules/*      |  <- applied first
   +---------------------+                     +-------------------+
```

**Assets shadow. Rules accumulate.**

A sprite has no partial form. There is no meaningful way to "override half of
`Base1a.pl8`", so a mod that provides one replaces it, and the layers below
become invisible for that name. A rule table *does* have a partial form —
changing one troop's archer count is a complete, meaningful statement — so
every layer's rule documents are read and merged, and the last writer of each
individual key wins.

That asymmetry is why `Vfs` has both `resolve()` (one winner, for assets) and
`layer_entries_under()` (every layer's copy, for rules). It would be easy to
give rules the asset treatment by accident; the two accessors exist so that
choosing is explicit.

---

## 3. The virtual filesystem

`crates/l2-mods/src/vfs.rs`.

A stack of layers: the base install at the bottom, each enabled mod above it in
load order. Each layer's tree is indexed eagerly on mount — for the shipped
install that is one directory of 1,196 entries, so the cost is a single walk and
every later lookup is a map hit with no filesystem round trip.

### 3.1 Case

The shipped install is not consistent about case, and the executable is not
consistent with the install:

```
Axemen.smk    AXMEN.SMK       <- two different files, not case variants
Bat_los4.smk  BAT_LOS5.SMK    <- the same series, two different casings
```

and `Lords2.exe` asks for `axmen.smk` in a third casing again. Lookup is
therefore case-insensitive regardless of what the host filesystem does. On
Windows that is redundant. On Linux — where the same install sits on a
case-sensitive filesystem — it is the difference between a working game and 45
missing videos.

Folding is **ASCII only**, and that is a decision rather than an oversight:
every name in the shipped game is ASCII, and Unicode case folding is
locale-dependent in ways that would make the same mod resolve differently in
Turkey than in England.

Case-folding can collide where the filesystem allowed two files that differ only
in case. NTFS cannot hold such a pair; ext4 can. The index resolves it
deterministically — lowest raw name wins — and records a `CaseCollision` rather
than picking whichever `read_dir` happened to return first. The corpus test
asserts that neither shipped install produces one.

### 3.2 Read-only by construction

`CLAUDE.md` rule 2 says the game installs are read-only. The cheapest way to
keep a rule is to make breaking it impossible, so `Vfs` has **no write API at
all** — no create, no write, no delete, not even behind a flag. A mod's output
goes somewhere else entirely, through code that never held a handle on a layer
root. There is a test whose only job is to make adding one a visible decision.

---

## 4. The rule format, and why it is a TOML subset

### 4.1 Why not RON

RON is Rust's data shape written down: tuples, enums with payloads, nested maps.
That is a good fit for serialising a Rust type and a poor one for a human
editing a table of troop statistics. Compare:

```ron
(battles: { "three_bridges": (attacker: (archers: 120)) })
```
```toml
[battle.three_bridges.attacker]
archers = 120
```

The TOML version also happens to *be* the override: a mod file containing
exactly those two lines and nothing else is a complete, valid mod. In RON the
same change requires restating the enclosing map structure, which is precisely
the "restate the whole table" problem composable mods exist to avoid. TOML's
table headers make partial documents the natural thing to write rather than a
trick.

Secondarily: a mod author has probably met TOML. It is what `Cargo.toml`,
`pyproject.toml` and half the tools on their machine already use.

### 4.2 Why no dependency

The obvious move is `serde` + `toml`. It was rejected, and the reason is not
dependency squeamishness:

**Merging needs a generic value tree anyway.** You cannot merge two documents
into `#[derive(Deserialize)]` structs — the second document does not contain
most of the fields, so every field would have to become `Option<T>` and every
consumer would have to unwrap. The workable design is to merge `toml::Value`
trees and deserialise once at the end. But if the merge operates on a generic
tree regardless, `#[derive]` is contributing only the last step, and that step
is the easy one.

**And the tree we want carries provenance.** `toml::Value` does not record which
file and line each scalar came from. Without that, "two mods set the same rule"
is a silent last-write-wins. With it, it is:

```
rules overridden:
  battle.three_bridges.attacker.archers : base:rules/troops.toml:117:11 -> longbows:rules/longbows.toml:12:11
```

That diagnostic is the single most useful thing a mod platform can produce, and
it is the thing the off-the-shelf option makes hardest.

**The cost side.** `serde` + `toml` is about 14 crates once `serde_derive`
pulls in `syn`, `quote` and `proc-macro2`; it adds a proc-macro build step; and
it is a supply-chain surface on a project whose licence position is already
delicate (`docs/decisions.md` D5a). The reader is ~680 lines and has 11 tests
covering the syntax and every error message. `l2-formats` is deliberately
dependency-free for the same kind of reason.

**What it cost us.** A hand-written parser is a hand-written parser: it can have
bugs a widely-used crate would not, and it will not track TOML spec revisions.
If the ruleset ever needs to round-trip through other tooling, that changes the
calculation. The mitigation is that the supported subset is small, documented,
and tested against its own error messages rather than only its successes.

### 4.3 The subset

Supported: comments; bare and quoted keys; dotted keys; `[table]`; `[[array of
tables]]`; basic and literal strings, single- and multi-line; integers (decimal
with `_` separators, `0x`/`0o`/`0b`); floats; `true`/`false`; arrays; inline
tables.

Not supported, on purpose: **dates and times**. The game has no use for them,
and accepting them would mean threading a date type through the merge and every
accessor for nothing. A document containing one gets

```
bad.toml:1:5: dates and times are not part of the rule syntax
```

rather than a wrong parse or a confusing "expected end of line" two characters
later.

Within a single document, setting a key twice or opening a table twice is an
error naming both lines. Across documents it is the merge, which is the point.

---

## 5. Merge semantics

Three rules. The count is deliberate: every extra rule is one more thing a mod
author must hold in their head to predict what two mods will do together.

1. **Table into table: recurse.** Keys only the newer document has are added.
   Keys both have are resolved one level deeper.
2. **Anything else: replace.** Scalars replace scalars. Arrays replace arrays
   *whole*. There is no element-wise array merge, because array elements have
   no identity — with `[1, 2, 3]` there is no principled way to say which
   element an override refers to. Anything that wants partial override should be
   a table keyed by name, and the seeded rulesets are.
3. **`"$delete"` removes.** `"$delete" = ["knight"]` inside a table removes
   those keys before the rest of that table merges — so a mod can delete a
   sub-table and then define a fresh one in the same document, replacing rather
   than merging. `$` is not a legal bare-key character, so the directive has to
   be written quoted and can never collide with a key that means something in
   the game.

Every replacement of an existing value is logged with both origins. So are
deletions, and so are `"$delete"`s that named something absent — usually a typo,
or a mod written against a version of another mod that has since renamed
something. `Report` prints the lot; `MergeLog::contested_paths()` picks out the
paths three or more documents have fought over, which is a stronger smell than a
single override.

---

## 6. Mods and load order

A mod is a directory containing `mod.toml`. Everything else about it — which
assets it replaces, which rules it changes — is discovered from its contents,
not declared. A declaration would be a second source of truth that goes stale.

Ordering constraints:

| Field | Meaning |
|---|---|
| `requires` | must be enabled, and loads first. Accepts `"core >= 1.2"`, `"ui ^0.4"`, `"maps = 2.0"`, or a bare id |
| `after` | loads after that mod *if it happens to be enabled*. The knob for compatibility patches, which must win over the thing they patch without depending on it |
| `conflicts` | refuses to load alongside that mod |

Versions are `major.minor.patch` with trailing components optional. Not full
semver: no pre-release tags, no build metadata. Those exist to coordinate a
published package ecosystem, and each one is a comparison rule a mod author
would have to learn.

**The user's order is the primary signal.** In an overlay system the load order
*is* the conflict-resolution policy, so silently re-sorting the player's list
would be taking their decision away. `resolve_load_order` runs Kahn's algorithm
over the dependency edges, always taking the lowest remaining *user* index — so
the stated order survives wherever the constraints allow, and where they do not,
the smallest possible change is made. Ties break on the user's index, so the
same list plus the same manifests gives the same order on every machine and
every run. A bug report is worth nothing otherwise.

Failures are checked in an order chosen so the message is about the real
problem: conflicts and missing/mismatched dependencies first, then cycles. A
cycle error names the loop (`a -> b -> c -> a`) rather than asserting one
exists.

---

## 7. What a mod author actually writes

This is the mod in `crates/l2-mods/example-mods/longbows/`. There is a test that
loads it, so what follows cannot drift away from what works.

```
longbows/
  mod.toml
  rules/
    longbows.toml
  Base1a.pl8          <- optional: any file here shadows the base install's
```

### `mod.toml`

```toml
[mod]
id = "longbows"
name = "Longbow Rebalance"
version = "1.0.0"
author = "an example"
description = """
Archers were the one thing England was actually good at, and in the shipped
skirmish tables they are an afterthought. This gives them numbers worth
fielding, and makes the difficulty curve bite harder at the top end.
"""

# requires = ["core >= 1.0"]     # must be enabled, and loads first
# after    = ["some-other-mod"]  # loads after it, if it happens to be enabled
# conflicts = ["shortbows"]      # refuses to load alongside it
```

`id` is the only required field. A one-line manifest is a valid mod.

### `rules/longbows.toml`

```toml
# Everything this mod changes. Nothing that it does not.

[battle.three_bridges.attacker]
archers = 120

[battle.three_bridges.defender]
archers = 90

[battle.isthmus.attacker]
archers = 140

[battle.the_arena.attacker]
archers = 100
crossbows = 10          # crossbows lose what the longbows gained

# In the original these five percentages are instructions in the executable.
# Here they are rules, so this is all it takes to change them.

[difficulty.hard]
scale_percent = 80      # was 92

[difficulty.very_hard]
scale_percent = 65      # was 84
```

That is the entire mod. The other ten troop columns of each row, the other 32
battles, the other three difficulties and every asset in the game are untouched
and keep whatever the base game — or an earlier mod — gave them.

To replace a sprite, drop a file with the same name in the mod directory. No
declaration, no registration.

### Enabling it

```rust
let platform = Platform::builder()
    .base(r"F:\games\Lords of the Realm II")
    .mods_dir("mods")
    .enable(["longbows", "harder-sieges"])
    .build()?;

println!("{}", platform.report());
```

and the report that mod actually produces, over a ruleset seeded from the
Windows install:

```
rules overridden:
  battle.isthmus.attacker.archers : base:rules/troops.toml:148:11 -> longbows:rules/longbows.toml:18:11
  battle.the_arena.attacker.archers : base:rules/troops.toml:334:11 -> longbows:rules/longbows.toml:21:11
  battle.the_arena.attacker.crossbows : base:rules/troops.toml:330:13 -> longbows:rules/longbows.toml:22:13
  battle.three_bridges.attacker.archers : base:rules/troops.toml:117:11 -> longbows:rules/longbows.toml:12:11
  battle.three_bridges.defender.archers : base:rules/troops.toml:130:11 -> longbows:rules/longbows.toml:15:11
  difficulty.hard.scale_percent : base:rules/troops.toml:98:17 -> longbows:rules/longbows.toml:29:17
  difficulty.very_hard.scale_percent : base:rules/troops.toml:102:17 -> longbows:rules/longbows.toml:32:17
```

Add a `Base1a.pl8` to the mod directory and an `assets provided by more than one
layer:` section joins it, naming every layer that offers the file and which one
won.

A corpus test asserts that **every** line of that report is an *override* rather
than an addition. A mod that misspells a battle id would otherwise merge
silently, creating a rule nothing reads; insisting the count matches the mod's
leaf count catches it.

---

## 8. The worked example: `TROOPS*.ENG`

`docs/formats/eng.md` documents these files completely, they are small, and they
are the one part of the shipped game that is unambiguously *rules* — 35 battles
x 11 troop columns x 2 sides, plus a per-battle defensive advantage. That makes
them the right thing to build the whole path against.

Three things about the original are worth noticing, because each is an argument
for doing this at all.

**The difficulty curve was hard-coded.** `Lords2.exe` reads all five difficulty
rows and then throws four of them away, re-deriving them from "Normal" as
+16% / +8% / −8% / −16%, applied only to troop types 0–6. Those five percentages
exist nowhere but as instructions in a 1996 binary. Here they are five lines of
a rule file:

```toml
[difficulty.very_hard]
order = 4
scale_percent = 84
```

and the engine reproduces the original's arithmetic exactly — `x * percent / 100`
with truncating integer division, siege columns exempt.

**The clamps were a parser detail.** The engine clamps the siege columns to 9
and the defensive advantage to 0–10 because it was reading a text file it could
not validate, and had to survive anything. We can validate, so a mod that asks
for 40 catapults gets

```
silly:rules/silly.toml:2:13: rule 'battle.three_bridges.attacker.catapults': 40 is outside 0..=9
```

instead of a silent 9 and an afternoon wondering why.

**The columns had no names.** The file has an eleven-column header of two-letter
abbreviations (`Pe Xb Ma Sw Pi Ar Kn Ca To Ra Oi`) and the code has indices.
Naming the columns is what lets a mod write `archers = 120` and change nothing
else. Columns 0–6 are named from the abbreviations and the unit set; 7–10 are
the siege columns, and their individual identities are **[I] inferred** from the
abbreviations, as `eng.md` marks them.

### The base ruleset is generated, not shipped

OpenXcom ships its rulesets. We cannot: a `rules/troops.toml` holding the 3,885
numbers of `TROOPS.ENG` is a transcription of a shipped game file, and
`CLAUDE.md` rule 1 keeps game data out of this repository. So the base ruleset
is **generated on the player's machine, from the copy of the game they already
own**, into their own data directory. Only the generator is version-controlled.

That turns out to be better anyway. The generated file is plain, commented and
human-readable, and it sits on disk next to the mods — so the first thing a
would-be mod author can do is open the base rules and read them, which is
exactly how people learn to mod OpenXcom.

Real excerpts from a file generated off the Windows install — 1,189 lines,
16 KB:

```toml
# Skirmish army rules, generated from TROOPS2.ENG.
#
# Generated, not authored: regenerating overwrites it. To change a
# number, put your change in a mod instead - a mod file with just the
# lines you want different is merged over this one.
#
# Only the game's "Normal" numbers are stored. The original engine
# read the other four difficulty rows and then threw them away,
# re-deriving them from Normal by a fixed percentage; those
# percentages are [difficulty] below.

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

Battle ids come from `BATTLES.ENG`, slugged: `Three Bridges` → `three_bridges`,
`Expanded Keep` → `expanded_keep`. Readable ids matter more than they look —
they are what a mod author types, and `battle_17` would make every mod file
unreadable.

On decoding: `l2-formats` owns decoders and keeps them. What `seed.rs` does is
not one. `TROOPS*.ENG` is plain text and the engine's own reader
(`FUN_0042AC0C`) is "skip to the first `*`, then take every decimal token and
ignore everything else" — ten lines. When `l2_formats::eng` lands,
`parse_troops_eng` should become a call into it and `seed.rs` should keep only
the rule-generation half.

---

## 9. What was deliberately left out

**Scripting, event hooks, and any way for a mod to run code.** This is the big
one. Every question a scripting layer answers — which events fire, what a script
may touch, how errors are contained, what the sandbox is — should be decided
against a real simulation. Deciding now would be guessing, and a guessed API is
worse than none because it has users. The data-driven half is the half that is
expensive to retrofit; the scripting half genuinely is not.

**Rules for anything but troops.** There is one worked schema, for the one
subsystem that is fully documented and that exists. Terrain properties, building
costs and unit statistics get schemas when the code that consumes them exists;
inventing them now would produce a vocabulary nothing validates.

**Mod-supplied assets in new formats, and asset *patching*.** A mod can replace
`Base1a.pl8` wholesale. It cannot yet add a frame to one, or supply a PNG for
the engine to convert. Both are real wants; both need the renderer to have
settled first.

**Localisation.** `L2.eng`'s `(group, index)` string addressing is documented in
`eng.md` and is obviously the next rules-shaped thing to lift — the German and
French builds swapped exactly this file. It is left out because it wants its own
schema decision (do string ids get names, or stay numeric?) and that decision is
better made when something is displaying strings.

**Enable/disable persistence.** The load order comes in as a list from the
caller. Where that list is stored, and the UI for reordering it, belong to
whatever ends up owning user settings.

**Signing, checksums, versioned rule schemas.** All premature. Worth revisiting
when mods are distributed rather than hand-copied.

---

## 10. Where things are

| | |
|---|---|
| `src/vfs.rs` | overlay filesystem, case folding, shadowing diagnostics |
| `src/reader.rs` | the TOML-subset reader, with per-value origins |
| `src/value.rs` | the value tree, `Origin`, `Spanned` |
| `src/merge.rs` | merge rules, `$delete`, the override log |
| `src/ruleset.rs` | merged ruleset, typed accessors, range checks |
| `src/modmeta.rs` | manifests, versions, discovery, load order |
| `src/troops.rs` | worked example: typed troop rules and difficulty scaling |
| `src/seed.rs` | generating the base ruleset from the player's install |
| `src/lib.rs` | `Platform`, the builder, `Report` |
| `example-mods/longbows/` | the mod printed in §7, loaded by a test |
| `tests/corpus.rs` | install-dependent checks; skips without `LORDS2_DIR` |

The crate is **not yet a workspace member** — `Cargo.toml`'s `members` list has
to gain `crates/l2-mods` before `cargo test -p l2-mods` will work from the repo
root.
