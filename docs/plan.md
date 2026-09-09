# Plan — a game you can play from start to finish

**Revision 4.** Written to be attacked, like its predecessor. Revision 3 described a vertical
slice; most of that slice landed and the plan did not move with it, so it stopped being stale
and started being misleading. The adversarial review that reshaped revision 2 is still in
[`plan-review.md`](plan-review.md), its numbers frozen at the day it was written, and it is
still the model for how to argue with this file: it attacked the plan, verified its own claims
with the commands that produced them, said where it could not settle something, and won on
substance.

The goal has been set explicitly, and it is not "a slice":

> **The game is playable from start to finish.**

That is a definition of done. It reorders things: several items that were optional are on the
critical path, one that felt urgent is not, and one whole item turns out not to be in this
executable at all.

**A note on evidence, before anything else.** Several findings below were produced by agents
in the hours this file was written. C13 governs them: *an agent's summary is a lead, not a
finding.* Each one names the file and the address it came from so it can be checked, and the
ones that have not yet been written into `docs/` or `symbols.json` say so.

---

## 0. Why "start to finish" is a different plan, not a longer one

Every previous plan measured a **path**: one route through the game, walked once, by somebody
who knows where to click. "Start to finish" measures a **loop that survives repetition** — a
hundred turns, five realms, all of them acting. That is a harsher test, and three classes of
defect appear only under it. This project has been bitten by all three and they are numbered:

* **C26** — a rule can be wrong at 45 of its 51 inputs and stay invisible, because the only
  fixture exercises one value of its input. Found twice in one afternoon, behind a green suite.
* **C27** — a rule with no way in is not a rule the game has. The whole grain economy was
  finished, tested and unreachable in play, for the player *and* for the AI. **Nobody farmed.**
* **C21** — work outside every named category gets no rigour, and its absence is invisible
  because nothing tracks it. A plan's categories decide what gets checked.

So this plan is organised by **what the goal requires**, not by what is left to build.

---

## 1. The definition, argued rather than assumed

### 1.1 Start

A person launches the binary, chooses a game, and is standing on a campaign map with a world
under it.

**Status: no — and the barrier is not where it was expected.** `l2-scenario` needs no map
file: `Scenario::from_save` reads the three tile planes out of the save's own `TILES` block,
so `starting_kingdom()` works today and produces a real, playable turn-one position from
`lastturn.sav`. It is called from ten tests and **from nowhere in `l2-game`**, which always
takes the load-a-save path (`crates/l2-game/src/scenario.rs:123`).

What is missing is a **new game on a chosen scenario**. All thirteen setup pages draw, their
page graph is read out of the original, and the boundary is written in the code:

> *"The setup screen chooses a scenario; nothing in this workspace can yet build a world from
> that choice, so Start enters the campaign the scenario loader already put in `Game`."*
> — `crates/l2-game/src/screens/setup.rs:538`

Every drop-down, the map list, the lord and shield choice: all local, none reaching `Scenario`
or `Kingdom`. Press Start on any configuration and you get England turn one.

*(`docs/decisions.md` D9 calls the two constructors "load-a-save" and "new-game-on-this-map".
The second is loose enough to mislead — the map it means is the one inside that save.)*

### 1.2 Play

Turns that resolve, for every realm, with the orders a player gives every turn reachable.

**Status: partly, and the missing part is larger than it looks.** Working: the economy — and
it has been *reachable* only since the field brush landed today — the county panels, the
village and its job drag, the industry toggle from the map, the twenty-five-pass season
pipeline, random events, scoring. Not working: §2.1 through §2.4.

### 1.3 Finish

The game ends, and somebody wins it.

**Status: no. This is what reorders the plan.**

**Sieges are on the critical path.** `crates/l2-kingdom/src/conquest.rs:114` implements the
gate, and it is one `if`: a county with a castle *and* a garrison that is not yours cannot be
entered at all — `Refusal::Garrisoned`. It is correct and it is tested at all four corners.
Its consequence is that **without sieges the map stops moving and a game cannot be won.** Any
plan that files sieges as a later phase is wrong, and revision 3 filed them that way twice.

**Victory detection is missing, and cheap.** In the original the chain is four functions and
is fully readable. AI turn step 0 (`0x0049B42B`) recomputes `strength = 3 × counties +
armies`; a realm at zero is eliminated and told so (`L2.eng` group 224, *"Defeat!"*).
`Score_RankRealms` (`0x0049AA0E`) ranks the survivors and, **when the trailer equals the
leader — one realm standing — enqueues group 225, "Victory!"**. `Msg_DrawWindow` sets the
terminal flag `DAT_0053F0C4` to 10 for won and 11 for lost, and screen `0x1C` reads it.

Ours has the first half and none of the rest. `ai::begin_realm_turn` already recomputes
strength and sets `realm.in_play`, and `Realm::is_eliminated` exists with a test. **Nothing
reads either as an ending**, there is no ranking, and the conquest interstitial cycles its
three outcomes on a key press as a demo affordance. Our reimplementation cannot end. It will
run forever.

---

## 2. What the goal requires that was on nobody's list

`docs/mechanics.md` exists to answer this question; this section exists because it did not
answer all of it. Each item is something the goal needs, that no plan named, that is not a
restatement of a known gap.

### 2.1 Nothing moves during a turn

`crates/l2-game/src/turn.rs` answers the phase 2, 3, 5 and 6 waits `true` the moment they
start — *"None of those exist yet"*. Those phases are army movement, peasant mobs, supply
transports and merchants. The campaign unit layer landed **today**, in `l2-kingdom`, and is
exercised only by `tests/campaign.rs`. **Armies are done as a library and absent from the game
loop.** C27's shape, one level up from a rule.

This sits *underneath* the campaign–battle seam that is in flight — a battle happens where two
units meet, and no unit meets anything — and underneath the AI's army steps, which would have
nothing to drive.

### 2.2 You cannot raise an army, so conquest is unreachable independently of sieges

Three facts that only bite together:

1. The England turn-one fixture **contains no army**. Six units, all merchants.
2. `levy.rs` implements raising one, and nothing in `l2-game` calls it.
3. The door is shell `0x17`, and `crates/l2-game/src/screens/shells.rs` calls it **"Hire
   mercenaries"** — while `docs/symbols.json` names its painter `0x00418653`
   **`Screen_RaiseArmy`**, and `docs/armies.md` §5 records that there is no separate
   mercenaries screen at all: the mercenary offer lives *on* the raise-army screen.

So a game started from the fixture has no army, no way to make one, and the door to making one
is filed under a name that reads as optional content. **A name is a claim** (C25, C28, C29 —
three corrections about exactly this), and this one has been quietly setting the priority of
the most gameplay-critical shell in the table.

### 2.3 A battle has no end

This is the sharpest of the eight, because work is in flight on the seam *into* a battle.

`crates/l2-sim/src/runner.rs` is a frame loop — rebuild, update units, reform, update men,
melee tick — with **no victory test, no retreat, no surrender, no outcome and no return to the
campaign.** Wiring the seam without an end condition builds a one-way door: you can enter a
battle and the campaign never resumes.

The original's answer is one unread function. `FUN_00477DFC` (1,225 bytes,
`tools/oracle/decomp/00470000.c:3218`) runs every frame and asks, in order: has someone
retreated or surrendered; is either side annihilated; and three siege-specific arms, one of
which is *"the assault is repulsed and repeats"* and one *"the siege is lifted"*. Each arm
sets `g_screenId = 0x2B` and writes `g_battleLoser` — **which holds the winner**, and this is
now the third independent confirmation of the inversion `docs/armies.md` §7 flags. Then
`Screen_BattleOutcome` picks one of **seven** `L2.eng` group-82 outcome pairs — won, lost,
siege won, siege lost, siege lifted, castle lost, and a neutral *"The conflict is over."* —
and after 5,000 ticks calls `Battle_ReturnToCampaign`.

**None of that is in any document, and `0x00477DFC` is in neither `symbols.json` nor
`hypotheses.json`.** The reading above is a lead (C13): it came from a decompiled function on
disk, so checking it is cheap, and it should be written up before anything is built on it.
What is *not* known even after that reading: how casualties get back to the campaign
(`docs/armies.md` §7 says that path was not traced), what `DAT_0053F028` and `DAT_00553C58`
count, how retreat and surrender are commanded, and how the seven outcomes are chosen —
`FUN_00478419`, 177 bytes, is the obvious candidate and has not been read.

### 2.4 The AI cannot farm, cannot fight, and the reason it cannot was true yesterday

`crates/l2-kingdom/src/ai.rs` names all fourteen handlers with addresses — real progress;
`docs/kingdom.md` §12 used to call them *"the single largest remaining piece of the kingdom
layer"*. **Four are implemented.** The AI sets tax rates, takes its resource grants, adds
fallow fields and recomputes its own totals. It raises no armies, moves nothing, builds no
castles, trades nothing, chooses no weapons and answers no diplomacy.

Two things about that are worse than the count.

**The stated reason has expired.** The module says nine steps *"drive armies, merchants,
diplomacy and map tiles, none of which `l2-kingdom` owns — reproducing them here would mean
inventing a unit model to hang them on."* **`l2-kingdom` owns a unit model now**; it landed
the same day, in the same crate. Steps 7, 9, 11 (three army passes; raise and move the main
army; walk every army to its target) and 10 (send a merchant or transport) are no longer
blocked by anything structural. A stale comment that reads as a decision is the most expensive
kind.

**No AI realm can ever plant grain**, and the brief's framing of this is wrong in two ways
worth correcting. `FUN_004A3C67` is **not** an AI realm's farming style — its only caller is
`AI_ManageFields(0)`, the pass over **neutral** counties. AI realms go through step 5 →
`Ai_ManageCountyFarms` (`0x0049DD01`), which copies the lord's `farmStyle` into county `+0x1FE`
and dispatches styles 0, 1 and 9 to three *other* allocators (`FUN_004A4052`, `FUN_004A42E3`,
`FUN_004A440F`). There are **five style allocators in total and none of the five is
implemented**; `tables.rs` carries `AiPersonalityRow::farm_style` and says outright that
nothing reads it. And only *part* of `FUN_004A3C67` is Winter-gated — the grain-field
assignment; buying grain, the labour allocation and the herd share run every season.

So the answer to *"is the AI farming style on the critical path?"* is **yes**. Ours already
adds fallow fields and never grain, which is C27 restated for the opponent: a game played to
the end against realms that starve is not a game that was won. It is also **cheap** — three
readable functions, 657 and 300 bytes and one unmeasured — which is why it gets its own item
in §3 rather than waiting behind the AI's army work.

*(Two names in `ai.rs`'s table need reconciling first. Row 5 cites `0x0049DD01` as
`AI_ManageFields`, while `symbols.json` has `0x0049DD01` as `Ai_ManageCountyFarms` **[V]** and
`0x0049DFC6` as `AI_ManageFields` **[I]** — two functions with near-identical reclamation
ladders and different style dispatches, and the dispatcher sends step 5 to the first. Row 13
says "offer or break an alliance"; `symbols.json` says `AI_Taunt` at that address is **not**
that, and says so as an explicit correction to `docs/kingdom.md` §3.2. Both are C28's shape.)*

### 2.5 Every fixture is turn one, and every realm in every fixture holds one county

The largest of the eight and the least visible, because it is an absence.

C23 established that the five starting counties are always 1, 4, 8, 11 and 13 and that realms
1 to 5 take one each. So **every rule that fires only when a realm holds more than one county
has no oracle at all**: the "other counties" tax happiness term (`TAX_HAPPINESS_OTHER` is flat
zero from rate 0 to 19, and every county in every fixture is at rate 0 — C26); empire
happiness and the realm-wide sums; secession and territorial contiguity, whose invariant
`docs/mechanics.md` says is *trivially true in all six saves*; bankruptcy's six seasons;
revolt; alliances; every AI ladder at an interesting treasury.

**The project has no evidence about the game it is now trying to finish.** C26 measured this
failure mode at turn one and found two wrong rules in an afternoon. The late game is a much
larger version of the same exposure and, unlike turn one, is not covered by a reproduction
test that could go red. The fix is §5's first ask and should be made now rather than when the
code is ready for it.

### 2.6 Save and load is a precondition of the goal, not a feature of it

Nobody plays a hundred turns in one sitting. "Start to finish" spans sessions by definition,
so the item listed last on every plan for months is the one the goal most directly requires.

### 2.7 `WEATHER_JITTER_BOUND` is invented, and it compounds

`docs/decisions.md` records it: `docs/kingdom.md` §7.3 gives the weather jitter as `random/8`
with no stated range, which is not implementable, so this is the one constant in `l2-kingdom`
with no evidence behind it. Weather drives sowing, growth, harvest and the herd every season
for a hundred seasons. Survivable in a five-turn slice; not survivable in a game played to the
end, and no test can tell us which we have.

### 2.8 Nothing points at this document

`docs/plan.md` is referenced by no file in the tree — not `CLAUDE.md`, not `README.md`, not
`docs/status.html`. The brief for this rewrite assumed `CLAUDE.md` pointed here; it does not.
That table is the index everybody reads, and a plan nobody is routed to is a plan that gets
rewritten instead of followed. One row, owned by the lead session.

---

## 3. The order, and why

Four items are in flight as this is written: **save and load**, **the labour allocator and
secession**, **the campaign–battle seam**, and this plan. §2.3 is a warning to the third of
them: without an end condition the seam is a one-way door, and that should be settled inside
that work rather than after it.

```text
  in flight ──┬─ 1 victory & defeat
              ├─ 2 the battle ends ─────────────┐
              └─ 3 a turn moves things ─┬─ 4 raise army ─┬─ 7 the AI's units ─┬─ 8 sieges ─ 9 the
                                        │                │                    │             long
                                        └─ 5 the AI farms ┘   6 merchant,     │             game
                                                               castle chooser ┘
                                                          10 new game from a chosen map
```

**1 — Victory and defeat.** Hours. Elimination and `in_play` exist and are tested; victory is
`Score_RankRealms`'s one line — the trailer equals the leader — and the interstitial that
shows the result already draws all three outcomes. First because it is the definition of done,
because it is the cheapest thing here, and because it **unblocks nothing**, which is exactly
why it keeps losing priority arguments (§4).

**2 — The battle's end condition and its outcome.** `FUN_00477DFC` is 1,225 bytes on disk and
unread. Do it with or immediately after the seam; a battle you can enter and not leave is
worse than no battle.

**3 — A turn moves the things that move.** Phases 2, 3, 5 and 6 stop being answered `true`
unconditionally. The movement code exists and is tested; this is the driver, not the
algorithm. Unblocks the seam from underneath and the AI's army steps from underneath.

**4 — The raise-army screen.** The only door to conquest, and conquest is the only route to
"finish". Its shell already loads the right art and the right `L2.eng` group; what is missing
is the levy slider, the six weapon stocks and the mercenary offer, all of which
`docs/armies.md` §5 has read out of the binary. Rename the shell in the same commit.

**5 — The AI plants grain.** Three style allocators, readable, none implemented (§2.4). Small,
and it is the difference between opponents who compete and opponents who starve. Placed before
the AI's army work because it is a tenth of the size and most of the effect on whether a
finished game means anything.

**6 — The merchant, then the castle chooser.** The merchant is touched every turn, its shell
is *"very nearly the real screen"* already, and it is the oracle for seven zero fields (§5).
But `Merchant_Trade` is implemented **nowhere** — `l2-kingdom` has `GOOD_SELL_PRICE` and no
transaction — so this is a rule to write, not only a screen to fill. It is also the only exit
for iron, stone and timber, the only entrance for bought weapons, and the only source of ale.
The castle chooser is five buttons and an OK (§8), and it is the door to castles, which are
the door to sieges.

**7 — The AI's remaining steps.** Armies first (7, 9, 11) and merchants (10), because those
are the ones the expired constraint was blocking; then castles (6) and industry (12), which
need the per-lord ladders; then diplomacy (1, 2, 13) last, because a game can be finished
without diplomacy and cannot be finished without opponents that attack.

**8 — Sieges.** On the critical path, not after it. The **campaign half** is mapped:
`Army_BeginSiege`, engines built on the spot over seasons (200 / 200 / 400 man-seasons) with
the army pinned, `Siege_BuildTick`, `Siege_Prepare`'s per-lord order, the assault gate,
`Siege_Break` and its three callers. Ours has the gate and the link bookkeeping and none of
the rest. The **battle half** is the harder piece and is now well understood:
`Battlefield_BuildCastle` (`0x0047C4BA`) takes an **int** and reads a stock cell layout out of
a `batfield`-family `.pl8` at record `castle × 0x20`, with `Siege_LaunchAssault` passing
`castleType − 1`. `l2-sim` has the siege troop types, the siege AI scores, the defence posts
and the siege-side raise order, and **no castle terrain to fight on** — `is_siege` is set true
only inside the crate's own test fixtures. Unlocks 14 of the 17 battle AI order handlers,
which are today called by nothing.

**9 — The long game.** Secession and contiguity (in flight), empire happiness at non-zero tax
rates, bankruptcy, revolt, alliances. All writable today and none checkable against anything
until §5's first ask exists. Get the save first, so it lands with a witness.

**10 — New game from a chosen map.** The ordering call most worth arguing with. A game that
cannot be started is not a game, so by the goal's own words this belongs near the front.
Against that: the imported position is a *better* development base than a new game, because it
is an oracle and a new game is not, and items 1 to 9 can all be built and checked against it.
Doing 10 early means building the rest of the game against a position nothing can verify. So:
last of the required work — and if it proves cheaper than §1.1 suggests, move it forward.

### Three orderings that are defensible, with the call made

* **Merchant before raise-army.** The merchant is touched more often, its shell is closer to
  done, and it promotes seven hypotheses. The call goes the other way because *a game with a
  merchant and no army cannot be finished, and a game with an army and no merchant can.* The
  merchant is the better **verification** item; the army is the **required** one.
* **Sieges before the AI's turn.** Sieges are the harder gate and the one that stops the map
  moving. The call goes the other way because a siege you can only conduct against an opponent
  who never builds a castle, never garrisons it and never attacks you is not a test of sieges.
  AI step 6 is what puts castles on the map to besiege.
* **The battle's end before the seam, or with it.** Written here as item 2 because the seam is
  already in flight; if that work has not landed by the time this is read, fold them together.

---

## 4. The cheapest items, named — because this project defers them

This is not a suspicion. **Save and load has been the cheapest item on every plan for months
and last on every one of them**, including revision 3, where the review moved persistence to
*first* and it still did not get done. Naming the pattern is the point of this table.

| item | why it is cheap | why it keeps being deferred |
|---|---|---|
| **Victory and defeat** | elimination exists and is tested; the interstitial already draws all three outcomes | it unblocks nothing, so it never wins a priority argument |
| **Save and load to a file** | `crates/l2-kingdom/src/save.rs` is a complete, versioned, ruleset-fingerprinted codec over `l2-net`'s canonical encoder, round-tripped by 17 tests, and deliberately contains **no `std::fs`**. Screens `0x35` and `0x36` already draw | the codec looks done, so the item looks done; what is missing is a file path and a menu |
| **`turn.rs`'s four unconditional waits** | the movement code exists and is tested | four lines that say `true` do not look like a subsystem |
| **Renaming shell `0x17`** | one string | nobody re-reads a name — three corrections about exactly this |
| **`promotion` on the merchant field hypotheses** | the criteria are already written in their `caveat`s and need moving to the field `docs/method.md` §7.6 says they belong in | see §5 |
| **A row in `CLAUDE.md`'s index** | one line | §2.8 |

The first two are load-bearing for the goal and the rest are hygiene, but they share one
mechanism: **cheap items lose priority arguments to expensive ones, forever, unless something
names them.**

---

## 5. The oracle asks — cheap, high-yield, and not a nice-to-have

Several things cannot be verified without saves that do not exist. This costs a person with
the game about twenty minutes, and it has already paid: **three battle saves produced today
caught a bug in code merged hours earlier.** C21 puts it more strongly — *the cheapest oracle
in this project turned out to be a person glancing at a picture*, and it beat 899 tests.

In order of yield:

1. **A late-game save — turn 40 or later, one realm holding several counties.** The largest
   gap in the project's evidence (§2.5), and the only thing that can test empire happiness,
   secession and contiguity, bankruptcy, revolt, alliances, or any tax rate above zero.
2. **A merchant transaction, before and after.** Buy something, sell something, end the turn.
   Promotes the seven fields that are zero in every save we hold: `Realm.spentThisSeason`
   (`+0x104`), `spentTotal` (`+0x108`), `receivedThisSeason` (`+0x10C`), `receivedTotal`
   (`+0x110`), `County.purse` (`+0x1F4`), `County.merchantFlag` (`+0x15C`), and — this one an
   addition rather than a quotation — `County.aleHappinessGiven` (`+0x219`), which only a
   purchase writes. **None of these entries carries a `promotion` field**, which
   `docs/method.md` §7.6 requires; writing these asks into that field is the half of this item
   that needs no save at all. Four more fields wait on different witnesses and should not be
   folded in here: `fieldsOther`, `fieldsReclaimable`, `farmStyle` and the `armyFood` pair need
   wasteland, an AI-owned county and an army in the field respectively.
3. **A castle standing with an enemy army beside it**, and ideally one mid-siege with engines
   under construction. Sieges are on the critical path and nothing about them can be checked
   against data today.
4. **An army in the field with a treasury behind it**, before and after one End Turn. Wages,
   foraging, the starvation ladder and the move reset are implemented and none has met a file.
5. **A finished game**, won or lost. The only thing that can confirm what the original does at
   the end — and it is what item 1 in §3 will be measured against.

`Save_RotateAndWrite` rotates `safeturn.sav ← old_turn.sav ← lastturn.sav` at every turn
boundary, so **one played turn produces a before/after pair for free**. C23 is the warning that
goes with it: name each save, fingerprint it, put it in `%LORDS2_FIXTURES%`, and never read one
out of an install directory — `lastturn.sav` is a rolling autosave and ten minutes of play
destroys it.

---

## 6. What the method changes bought

Three things landed recently that change what is *possible*, not only what is done. They are
why this plan can be more ambitious than revision 3.

**Record-struct typing.** `docs/records.json` plus `ghidra_scripts/ApplyRecords.java` give the
fixed-stride arrays a struct type before every corpus rebuild, so `DAT_0052F218` reads as
`g_units[i].totalMen`. Measured over the same 2,452 functions (`docs/method.md` §7.5, frozen
there): `DAT_` occurrences fell from **24,608 to 15,308**, distinct `DAT_` names from 2,977 to
2,309, and **334 synthetic globals stopped existing in the first action alone** — they were
field offsets of three arrays that were already named. Seventy-two functions crossed from
"unanchored" to "in a cluster" without anybody reading one. The consequence for this plan:
**reading a new subsystem is cheaper than it was**, so the AI's nine steps, `FUN_00477DFC` and
`Battlefield_BuildCastle` should all be costed lower than revision 3 would have costed them.

**Constraint propagation by subsystem.** A guess is a set of predictions about its neighbours;
test the predictions rather than the guess. Measured on one campaign-map pass
(`docs/method.md` §7.7, frozen): **21 role predictions made, 15 held, 6 refuted** — against the
one-at-a-time role rate of **58%** measured in §7.3. Three of the six refutations were the
pass's most valuable results, because a refuted prediction invalidates its neighbours and stops
the error spreading. The failure mode is C3's and propagation makes it *worse*, so pin every
region to something that cannot lie.

**The two-tier split, enforced.** `symbols.json` is **[V]** and reaches Ghidra and the corpus;
`hypotheses.json` is **[I]** and reaches nothing. Promotion is a move, not a copy, and
`tools/symbols/symbols_md.js` fails CI on a non-enum `confidence` or on an address in both
files. C28 is why: an inferred global name reached the corpus 155 times with nothing marking it
a guess. **Nothing in this plan may be cited from `hypotheses.json` as a finding.**

**And fixtures no longer skip silently.** C23: a fixture is a name plus a fingerprint, and
`crates/l2-testkit/tests/census.rs` reads the source, works out which gate each `#[test]` sits
behind and asserts the count against a written-down inventory. Before that, `cargo test
--workspace` printed the same green number with the game present and absent, and **116 tests
did not exist on CI and nothing said so.**

**The figures are generated.** `tools/figures/figures.js` rewrites the marked numbers in
`README.md`, `docs/status.html`, `docs/method.md` and this file, and `--check` fails CI on a
stale one. Twelve stale figures were found in a day, one document claiming 542 tests against
<!--fig:tests-->1,265<!--/fig-->. **Do not quote a count here that nothing recomputes**: mark
it, or label it frozen and say what it records.

---

## 7. What is not known, and is shaped like C3

C3 is the correction where three functions matched three modes and a plausible story assembled
itself out of decompiler output. These are that shape — coherent, unanchored, and load-bearing
enough that a wrong reading would spread. They are here so the risk register carries live
worries instead of dead ones.

* **`FUN_0042FF10`** — 8,140 bytes, the largest unnamed function in the interface. It writes
  `g_screenId` with **thirty different values, more than any other function in the binary**,
  and touches no string and no shipped asset. **Only the first 6% of the body has been read.**
  `hypotheses.json` calls it `Ui_DispatchPointer` on the strength of its opening — hit region,
  input handler, then a per-screen fallback chain — which is a claim about the role and not
  about the other 94%. Every screen this plan adds routes clicks through whatever it actually
  is. The largest unread object on the critical path.
* **The command journal at `0x0050D7C0`.** `Net_JournalCommand` writes a ring of `0x38`-byte
  slots — a live flag, a sub-opcode and twelve int arguments — and every command writer/reader
  pair traced so far ends in a call to it. **Nothing traced reads the ring.** Whether it is a
  replay journal or a debug overlay decides whether the original kept a command log we should
  be reproducing, which `docs/netcode.md` would care about. Its promotion criterion is already
  written: find the consumer.
* **`Anim_DrawBowA2` reading `g_mapRotation`.** The bow-draw animation is the only function in
  the battle that reads the *campaign* map's orientation, and it rotates the figure facing by
  it. Either the battlefield honours a rotation nothing else implements, or this is a paste
  from the campaign renderer whose extra branches never run. Both coherent, neither anchored,
  written down as unresolved rather than narrated into place.

And two that are unknown only because nobody has looked, which is a cheaper kind: the seven
battle outcomes' selector (`FUN_00478419`, 177 bytes) and how casualties return from a battle
to the campaign — `docs/armies.md` §7 says that path was not traced, and §8, *"Taking a
county"*, calls itself *"the hole in the middle of this document"*.

---

## 8. What the goal demotes, and what is refused

The owner has been consistent, and this is the test to read the plan against:

> *"Mods should probably come after menus and you know playing the game."*

**Framework first. Then playing the game. Then mods.** Nothing is reverted — the mod platform
is good and further ahead than it needed to be — but no further mod work is scheduled unless
the framework requires it. Loading the core ruleset at startup is framework work and proceeds.

### The castle designer is not on the critical path, and may not be in this game

The item the brief was least sure about, and the evidence is one-sided.

`Screen_CastleBuild` (`0x00419789`) is a **picture-and-stats browser over an integer 0…4**. It
blits the selected type's picture from `caspics.pl8` by index and names it from `L2.eng` group
71 — *"Wooden palisade."* through *"Royal castle."*, five of them — then prices it from
`g_castleMaterial` net of the castle already standing. Its OK handler refuses a type equal to
or lower than the one you have and otherwise calls one function with two integers, county and
type, which sets `castleType`, loads the outstanding work from `g_castleWorkforce`, deducts the
materials and stamps the map. Then it plays `Castle1.smk … Castle5.smk` — **one construction
movie per type**, all five shipped. `Castle_BuildTick` finishes it over seasons and never
touches `castles.dat`. Five parallel five-entry tables — materials, workforce, garrison cap,
tax bonus, free archers — hang off one county byte, `+0x1C0`.

Three further facts, each of which could have gone the other way:

* **`grep -n "esign" tools/oracle/decomp/*.c` returns nothing** across all 2,452 decompiled
  functions, and the install holds two executables: `Lords2.exe` and the *map* editor.
* **`castles.dat` is a siege-damage cache, not a design file** — 16 blocks of `0x3200` bytes,
  which is 6,400 battlefield cells × `{frame, flags}`. Its only writer is the end-of-siege
  bookkeeper and its only reader fires when `castleDegraded == 2`, so that re-besieging a
  breached castle restores the breach. This sharpens `docs/decisions.md` C11, which has it as
  *"working state created zeroed at new-game"* — true, and it says nothing about what fills it.
* **You start with a castle you never asked for.** Read at county offset `+0x1C0` in
  `england-turn1.sav`: counties 1, 4, 8, 11 and 13 hold `castleType = 3`, the Norman keep, and
  everything else 0. That is the five starting counties, and it matches the *"Starting
  Castle"* new-game option.

So a player gets a castle with no designer at all, and the battlefield you besiege is a stock
layout keyed by the type byte. **`docs/mechanics.md`'s line — *"the castle designer. One of the
game's signature features. Untouched."* — is not supported by anything in the traced binary.**
The likeliest explanation is that the wall-drawing designer people remember belongs to the
*Siege Pack* edition rather than to the retail executable this repo decompiles; **that
attribution is inferred and unverified**, and it is exactly the sort of question a player can
settle in one sentence, as in C21 and C22. Ask before writing it down.

### Also demoted

* **Transports.** A transport is your own supply caravan — `Transport_Spawn` loads grain and
  cattle out of one of your counties and `Transport_Deliver` adds them to another when it steps
  on the town tile. It is the send-supplies screen. It is **not** required: there are no boats
  (`g_unitTickTable` has six slots and none is a water unit), sea is cost 0 in the one cost map
  every unit type shares, and England's fourteen counties are **one connected component** —
  checked by reading the neighbour lists out of the fixture and walking them, which no existing
  test does. Nothing on the map needs a boat to be reached.
* **Naming more of the binary for its own sake.** <!--fig:functions-->648<!--/fig--> of
  <!--fig:binary-functions-->2,452<!--/fig--> functions are named, about
  <!--fig:functions-pct-->26<!--/fig-->%. The review measured that *"the rest is mostly CRT and
  glue"* is **false** — 418 unnamed functions touch `g_counties`, `g_units` or `g_tiles` — and
  the conclusion survives for a different reason: we are inventing our interface rather than
  cloning the original's. Name what a plan item needs, when it needs it.
* **Pixel comparison against the original's framebuffer.** Follows from the point above and
  should be said plainly rather than left as an aspiration: for every screen we invent, it is
  off the table. It stays available for what we do reproduce — the map, the sprites, the fonts.

Refused outright until §1's three gates are green: sound, video and D5a's licence choice with
it, the multiplayer lobby, the map editor, the village's animations, missiles that visibly fly,
and any screen not required by a turn.

---

## 9. How this plan fails

Written so the failure is recognisable early rather than in hindsight.

* **The AI's steps turn out to be nine subsystems.** Costed here as reading, on the strength of
  §6's cheaper corpus. *Early warning:* naming the three army steps does not produce three
  named callees each within a day. Stop and re-cost.
* **The long game diverges and nothing notices.** §2.5 realised. A hundred turns with an
  invented weather bound, an unexercised bankruptcy ladder and an untested contiguity pass can
  produce a plausible, wrong game that every test passes. *Early warning:* the late-game save
  does not arrive and item 9 is built anyway.
* **The shells consume the plan.** Nineteen screens is a lot of surface and a screen is where
  scope goes to die. Only three are required by §1 — raise army, the merchant, the castle
  chooser — plus the two file-list shells for save and load. *Early warning:* a fourth screen
  gets built because it was next in the table.
* **`FUN_0042FF10` turns out to matter.** 94% of the biggest interface function is unread and
  every screen routes clicks through whatever it is. *Early warning:* a screen's input needs a
  dispatch rule nobody can find.
* **The plan is followed and the game is unplayable anyway**, because none of it was shown to
  somebody who has played it. C21 and C22 were both overturned by a player looking at a
  picture, in one sentence each, against a green suite. **Show screens early.**

---

## 10. How to attack this file

Three targets, in the order I would take them:

1. **That sieges are on the critical path.** It rests on one `if` in `conquest.rs`. Check
   whether the original really has no route past a defended castle — a starved-out garrison, a
   county taken by revolt or secession rather than by arms, or the county town while the castle
   stands, which is what C25's own evidence says: *"Your troops may capture a castleless county
   by attacking its county town."* If any such route exists, sieges move down and this plan's
   biggest reordering is wrong.
2. **That the castle designer is not in this executable.** Absence of evidence, from a grep and
   a directory listing, plus an unverified guess about which edition. A player can overturn it
   in a sentence, and if they do, §8 is wrong and a large phase reappears.
3. **The ordering of item 10.** Building the whole game against an imported fixture and only
   then teaching it to start a new one is exactly how an interface gets built around
   assumptions the real entry path breaks. §3 is honest about the trade and could still be the
   wrong call.

Everything in §2 is a finding rather than an opinion, and each names the file or address it
came from. Those are the claims to check first if you think this plan points the wrong way.
