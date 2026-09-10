# Plan — 1:1 with the original, all of it, start to finish

**Revision 5.** Written to be attacked, like its predecessors, and rewritten rather than edited:
the goal moved, and a plan whose goal moved is not a longer version of the old plan. The
adversarial review that reshaped revision 2 is still in [`plan-review.md`](plan-review.md), its
numbers frozen at the day it was written, and it is still the model for how to argue with this
file.

The goal, as set:

> **1:1 with the original game experience. All features. Start to finish.**

Revision 4's goal was *"the game is playable from start to finish"*. That is a weaker claim in a
specific way: a screen that drew the right background and the wrong contents satisfied it, and a
behaviour the original has that we never noticed was not a failure because nothing named it. 1:1
makes both of those defects, and that reordering is most of this document.

**A note on evidence, before anything else.** C13 governs the findings below: *an agent's summary
is a lead, not a finding.* Every number in this file that describes the tree is **generated** by
`tools/figures/figures.js` and checked in CI, because three separate briefs this month quoted
stale counts as current — one of them mine. Numbers that are frozen measurements say so.

---

## 0. What would prove we have not got there

**A goal with no failing condition is a mood.** Revision 4 could be satisfied by opinion; this one
is written to be losable, so here is what losing looks like. Any one of these, demonstrated, means
the goal is not met — not "mostly met".

1. **A player performs a gesture the original responds to and ours does not.** This is now
   countable rather than rhetorical: `docs/arms.json` is the inventory of the original's input
   arms and `crates/l2-game/tests/arms.rs` checks it against the code in both directions. Of the
   arms enumerated so far we reproduce **<!--fig:arms-reproduced-->122<!--/fig--> of
   <!--fig:arms-live-->158<!--/fig--> live arms (<!--fig:arms-pct-->77<!--/fig-->%)** — so
   **<!--fig:arms-missing-->36<!--/fig-->** gestures a player can make get no answer — with
   **<!--fig:arms-dead-->3<!--/fig-->** more arms that are in the binary and cannot run.
2. **We do something the original does not.** The other direction, and the half nobody was
   counting: **<!--fig:arms-inventions-->24<!--/fig-->** inventions are on file. An invention is
   worse than an omission, because nothing looks broken.
3. **A screen shows something the original does not, or fails to show something it does.** This
   is the least measured of the three and §2.10 is about that.
4. **The hundred-turn game diverges** — a rule that is right on turn one and wrong on turn forty.
   §2.5.
5. **A number in a saved game differs from the original's** for the same inputs.

The first two have instruments. **The third now has one, and it has fired ahead of the player
twice in one evening.** The fourth has a partial one. The fifth is what the fixtures do.

### The third row's instrument, and what it still cannot see

`docs/draws-map.md` is the campaign map's draw-call inventory — **139 calls, 121 live, 59
reproduced (49 %)**, derived by `tools/draws/mapdraws.js` rather than typed. Beside it,
`docs/arms.json` reports **20 of 25** input arms on the same two screen ids. So:

> **On the screen a player spends most of the game looking at, we answer four gestures in five
> and draw one picture in two.**

That asymmetry is the whole case for the row. It is invisible from the input side, invisible to
every test, and it is the shape of both reports that started the audit — *"I see placeholder
shit everywhere"* and *"why do the pastures not have cows in them?"* Neither is an arm.

**Twice on 9 September the inventory named a defect before the player did**, and after a week
in which every player-visible defect was explained *after* he found it, that is the row
changing state rather than the row being ticked:

* §5.5 counted the eight `Ui_DrawDelta` calls as missing. Hours later: *"Sidebar doesn't show
  grain being planted as a negative number."* The listing could then say which of three causes
  it was — the value never arrives — by reading rather than guessing.
* §5.5 also counted the sidebar's five **industry** rows as undrawn, 21 of 31 row-painter draw
  calls missing. He asked about the icons the same evening.

**What it still cannot see, stated because a partial instrument that is trusted whole is worse
than none:**

* **It is one screen.** Twenty-two others have no inventory, and `0x04` — two painters, eleven
  layouts — is measured at 174 draw calls against our 24.
* **It counts calls, not correctness — and that is now measured rather than feared.** A call
  we make at the wrong coordinate, in the wrong font, or with the wrong frame counts as
  reproduced. Of the eighteen sidebar draws read back line-by-line against their call sites,
  **three were four pixels wrong** — the population, the happiness and the tax rate, every one
  of them dropping `Ui_DrawNumber`'s sign column — and **every test in the tree passed, two of
  them while asserting the wrong coordinate and quoting the right call site.** A player found
  it. So `reproduced` is coverage, not fidelity, and the fix is the same third verdict
  `arms.json` is acquiring: `reproduced` / `placed` / `absent`. Until that pass runs, **59 of
  121 is an upper bound.** `docs/draws-map.md` §5a.
* **It cannot see a draw that is *right* and never runs.** The wheat's growth was a
  reproduced-looking call whose input never changed. Nothing in a draw-call count is capable
  of noticing that, and the check that would have — *does this picture ever change over a
  played game?* — does not exist. That is the next instrument, and it is the one the fourth
  row wants too.
* **It cannot see a per-frame animation that never advances.** There is an existing assertion
  that a hundred ticks leave `Kingdom` byte-identical, which is the *correct* invariant for
  display state and says nothing whatever about whether the display moved. A true check about
  the wrong claim — `docs/agents.md`'s standing pattern, and the reason the industry sites
  being static was reported by a person rather than by us.

**The third row is the most valuable line in this file and it must not disappear when it
improves.** *"A screen showing the wrong thing — instrument: none"* is the only entry that tells
you what to build next; the numbers above it tell you where you are. When the draw-call audit
(§2.10) narrows it, the row does not become a tick — it names **which** instrument now covers it
and **what that instrument still cannot see**. A falsification condition that vanishes on
acquiring a partial instrument is a condition that has been quietly weakened rather than met,
and this list is the one place in the project where that would not be caught by anything.

**The honest caveat on the headline, stated here rather than in a footnote:** `arms.json` covers
**<!--fig:arms-groups-done-->10<!--/fig--> of <!--fig:arms-groups-->11<!--/fig-->** enumerated
groups, and those two are the battlefield and the battle seam. It is not yet the project's
number. The wider, coarser measurement is `docs/decisions.md` C61's — **80 of 185 arms across
three other screen groups, 43%** — taken by a different counting rule and superseded in detail by
the arms file as each group is enumerated into it. Quote whichever you like, but say which.

---

## 0.1 Multiplayer is excluded, deliberately and not by omission

The original's multiplayer sync is **the reason for this rewrite** — it is the defect being
replaced, not a model. So "1:1 with the original" explicitly does not extend to it: `docs/netcode.md`'s
deterministic lockstep is a *better* design than the original's and is meant to be, and
"the original did it" is evidence of nothing there.

That is the one place fidelity is not the method. Everywhere else it is, including the places
where the original is plainly wrong — `docs/bugs.md` catalogues the defects we reproduce on
purpose and the switches that turn them off.

Two consequences worth stating so nobody re-derives them:

* **A multiplayer feature is not a 1:1 requirement.** The lobby, the handshake and the quirk
  exchange exist because *our* determinism needs them, not because the original had them.
* **The rest of the engine still pays lockstep's costs** — no floats where ordering matters, a
  frozen PRNG, no hash-order iteration — because those constraints are nearly free while the
  simulation is being written and ruinous to retrofit.
---

## 0.2 Why this is a different plan, not a longer one

Every plan before revision 4 measured a **path**: one route through the game, walked once, by
somebody who knows where to click. *Start to finish* measures a **loop that survives
repetition** — a hundred turns, five realms, all of them acting. **1:1** adds a second axis to
that: not just *does the loop survive*, but *is each turn of it the turn the original would
have played*. Three classes of defect appear only under the first, and this project has been
bitten by all three:

* **C26** — a rule can be wrong at 45 of its 51 inputs and stay invisible, because the only
  fixture exercises one value of its input. Found twice in one afternoon, behind a green suite.
* **C27** — a rule with no way in is not a rule the game has. The whole grain economy was
  finished, tested and unreachable in play, for the player *and* for the AI. **Nobody farmed.**
* **C21** — work outside every named category gets no rigour, and its absence is invisible
  because nothing tracks it. A plan's categories decide what gets checked.

A fourth class belongs to the new axis and is numbered too:

* **C61** — a behaviour nobody enumerated cannot be missing, because nothing names it. The
  march preview appears on hover in the original and only after the click in ours; an army
  could not be deselected. Both had been true for weeks behind a green suite, and both were
  found by a player rather than by us. §0 and §2.10 are the two instruments that came out of it.

So this plan is organised by **what the goal requires**, not by what is left to build.

---

## 1. The definition, argued rather than assumed

### 1.1 Start

A person launches the binary, chooses a game, and is standing on a campaign map with a world
under it.

**Status: yes.** This section used to say *"no"*, and named exactly what was missing: a
**new game on a chosen scenario**, which is `Map_InitScenario`. It is written —
`crates/l2-scenario/src/newgame.rs` — and *Start* on the custom page now runs
`Game_NewGame`'s three steps in its order: the world from the chosen `L2_maps.dat` slot,
then the twelve options through `Settings::apply_to`, then the one immediate
`Season_Advance` that begins a game in Winter 1268. **Pick Ireland and you play Ireland**,
from an empty `Game` and with no save in the path at all.

All 44 shipped maps start and take a turn (`crates/l2-game/tests/newgame.rs`), and England
built from `L2_maps.dat` is diffed field by field against England read from `lastturn.sav`
— two files authored separately, agreeing on every fact the map decides.
`docs/decisions.md` C62 has the four corrections that fell out of it.

What is still short of the original at *Start*: the starting garrison (`Army_Create` at
setup, which is `Settings::unhonoured`'s and was already), and the castle *on* the start
county's plot — the world builder stamps the bare plot and `FUN_0046826C` raises the chosen
level on it, which belongs beside the castle rules. Both are marked on the page.

*(`docs/decisions.md` D9 calls the two constructors "load-a-save" and "new-game-on-this-map".
The second used to be loose enough to mislead, because the map it meant was the one inside
that save. It is now literal.)*

### 1.2 Play

Turns that resolve, for every realm, with the orders a player gives every turn reachable.

**Status: partly, and the missing part is larger than it looks.** Working: the economy — and
it has been *reachable* only since the field brush landed today — the county panels, the
village and its job drag, the industry toggle from the map, the twenty-five-pass season
pipeline, random events, scoring. Not working: §2.1 through §2.4.

### 1.3 Finish

The game ends, and somebody wins it.

**Status: the ending is built; reaching it in play still needs sieges.** A game *can* now end
— constructed positions are driven to a win and to a loss in `tests/ending.rs`, land on the
right outcome byte and the right sentence of screen `0x1C`, and a campaign steps map to map.
What is still missing is the way a person actually gets to that position on the board.

**Sieges are on the critical path.** `crates/l2-kingdom/src/conquest.rs:114` implements the
gate, and it is one `if`: a county with a castle *and* a garrison that is not yours cannot be
entered at all — `Refusal::Garrisoned`. It is correct and it is tested at all four corners.
Its consequence is that **without sieges the map stops moving and a game cannot be won.** Any
plan that files sieges as a later phase is wrong, and revision 3 filed them that way twice.

~~**Victory detection is missing, and cheap.**~~ **Done, and the chain was not quite the one
written here.** `Realm_RecountStrength` (`0x0049B42B`) recomputes `strength = 3 × counties +
armies` at the top of **every** realm's turn — the human's too, because `AI_RunTurnStep`'s
`isHuman` test guards the handlers and not the step-0 initialisation — and a realm at zero is
eliminated and told so. The message is chosen on *"is this the local player"*: group 224 for
you, 194 for an AI, and silence for a second human in a network game.

`Score_RankRealms`'s *"trailer equals the leader"* is a comparison of **realm indices**, not
of scores: it means one realm is left standing, and the paraphrase above is what made it read
strange. It is also not the mainline victory. **`Msg_DrawWindow` is**: any ending message
displayed while `g_opponentsRemaining` is zero enqueues group 225, and the game is won when
*that* is dismissed. `g_gameOutcome` (`0x0053F0C4`) is 10 won, 11 lost, and dismissing the
message that set it calls `Campaign_EnterConquest` and enters screen `0x1C`.

Implemented in `l2_kingdom::victory` (the rules) and `l2_game::victory` (the campaign and the
message queue), with `crates/l2-game/tests/ending.rs` driving a won game and a lost game
through the real phase machine. `docs/decisions.md` C32 records the four ways the reading
above was wrong and the two bugs of ours that only an ending could expose.

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

> **Closed.** Screen `0x17` is `crates/l2-game/src/screens/army.rs` and screen `0x11` is
> `screens/divide.rs`; both have left the shell table, and the name is corrected in it, in
> `docs/screens-county.md` and in `docs/mechanics.md`. `Map_Click`'s army branch is
> `screens/map.rs` — a click on your own army selects it and the next click on the map is the
> march order, a click on your own *besieging* army opens `0x1D` — and turn phase 2 calls
> `engagement::run_siege_phase`, which nothing outside its own tests had ever called.
> `crates/l2-game/tests/military.rs` drives all three verbs as `Event` values through
> `Machine::handle`: levy, equip, raise, march, take a county, split, disband, assault.
> `docs/decisions.md` C45 is the correction, and it names the two stale things the name was
> hiding.

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

> **Closed** — §3 items 5 and 7. Twelve of the fourteen handlers run; the five styles are
> `ai_farm.rs` and the war is `ai_army.rs`. The section is kept as written because the
> *reasoning* is what item 7 is now recommended on, and because the expired-comment failure it
> names has already recurred once inside this very work: `docs/kingdom.md` §3.2's *"create a
> type-7 unit"* was copied into `ai.rs` as *"needs the unit mission byte"* and neither reader
> noticed they were the same wrong claim.

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

## 2.10 What we do not measure, which is now the largest unknown

**New in revision 5, and it is the section I would attack first.**

This project measures two things well and one thing not at all.

* **Input arms** — what the original responds to. `docs/arms.json`, checked both ways.
* **Fields and rules** — what the simulation computes. Fixtures, the encode/decode check, the
  hundred-turn assertions.
* **What the screen should show.** Nothing. There is no inventory of the original's *draw* calls
  the way there is of its input arms.

Two findings in one evening say that gap is not theoretical, and they are the same shape.

**The pastures have no cattle.** A player said so while running the build. No arm is missing —
nothing is clicked to make a cow appear. No field is wrong — the herd count is right, and
`docs/kingdom.md` has the arithmetic. No test could fail, because no test enumerated what the
pasture tile is supposed to have on it. **Nothing is missing that anyone wrote down.** It is
visible content in a dimension we have never counted, and the <!--fig:shells-->0<!--/fig-->
remaining shells — which the same player summarised as *"placeholder everywhere"* without being
told which screens were shells — are the same gap at a coarser grain. That his count and the
shell table's agree is the only corroboration either has.

**Nothing was checking where an agent works.** Seven agents ran today in one shared checkout
rather than seven worktrees. Every test passed, every lint passed, every document was silent, and
it was found only because the shared tree stopped compiling mid-merge. There is no file in this
repository whose job it is to know that, and no check could have failed.

The general form, and it is the reason this is a section rather than two anecdotes:

> **A dimension nobody has enumerated cannot produce a failing test, however good the tests in
> the dimensions somebody did enumerate.** Coverage is measured *inside* a taxonomy. It says
> nothing about the taxonomy.

### The obvious next audit

The arms enumeration worked because the original's input is *discoverable in the binary*:
`Screen_FrameInput` is a ladder, the widget tables are data, and an exhaustive scan of
`g_screenId` writes settled reachability. **Its drawing is discoverable the same way.**
`Screen_Draw` (`0x0040F1A0`) is the painter's own switch — 39 cases, one per screen — and each
case calls a named painter whose body is a sequence of `Pl8_DrawFrame`, `Ui_DrawText` and
`Widget_Draw` calls against tables that are also data.

So: **a per-screen inventory of the original's draw calls, in the shape `arms.json` already
has** — one record per thing the original puts on a screen, with the address that draws it, and a
status saying whether we do. The same set-equality check applies, against a `// draws:` marker.
That gives the third falsification condition an instrument, and it would have caught the cattle,
the seven shells, and the text-layout defect that placeholder assets hid for weeks.

It is a large job and it should be scoped like the arms one was — one screen group, enumerated
completely, before any judgement about whether the shape works.

**A pilot is already running and nobody has to commit to the full audit before reading it.** The
agent finishing the last shells and the pasture cattle is keeping an inventory as it goes: every
draw the original makes that we do not, on the screens it touches. That is the cheapest possible
form of the estimate — the work was happening anyway, and the by-product answers the only
question that matters before scoping the rest, which is *how many records per screen* and *how
long each takes to establish*. If its rate is close to the arms enumeration's, the shape holds
and the audit is schedulable; if it is much worse, the reason will be visible in its records
rather than argued about.

---

## 2.11 Which of our checks would survive their own assumptions changing

**Also new, and it is about the instruments rather than the game.** Under a 1:1 goal the checks
*are* the project: the claim "we match the original" is only as good as the things that would
notice if we stopped. Two measurements from this month say the instruments need auditing as much
as the code does.

**Three of nine merge defects had no automated defence at all** and were found by a person
reading — a duplicated correction heading, a doc comment attached to the wrong constant, and a
misaligned symbol merge that would have put one function's comment on another's address. Two of
the nine were caught by the compiler, which is the cheap case and the argument for making a
mistake unrepresentable rather than checkable. The rest were caught by checks built for other
purposes.

**Three checks passed for reasons unrelated to why they were written.** `JSON.parse` caught the
misaligned merge because the misalignment happened to be syntactically invalid. `arms.json`'s
marker rule was correct only because every invention on file happens to have been removed. And a
build-stamp test that counted non-background pixels passed *with the line that draws the stamp
deleted*, because the title page carries full-screen artwork.

> **A check that passes for an accidental reason is indistinguishable from one that passes for
> the right reason, until the accident stops holding.**

`docs/agents.md` carries both, with the practice that finds them: **ablate the exact line the
assertion claims to be about and watch it go red.** A test written against a passing tree has
never been observed failing, and until it has, *"it passes"* is a statement about the tree and
not about the test.

**What this means for the plan.** Every item below that says "checked" should be read as "checked
by something that has been seen to fail". Where it has not been, that is work this section
claims, and it is cheaper than the feature it protects.
---

## 3. The order, re-derived from the new goal

**Seven agents were in flight when this was written**, which is a different regime from revision
4's four and changes the shape of the plan as much as the goal did: work now arrives in bursts,
several branches touch the same files, and the integrator is the only serial actor. Two of this
month's corrections are about that alone (`docs/decisions.md` C61's numbering collisions, and the
merge-by-key driver).

In flight: **keyboard text entry**, **diplomacy**, **the siege battle screen**, **battle casualty
write-back**, **the naming campaign**, **the remaining input arms on the county, village and
sidebar screens**, and **the last seven shells plus the pasture cattle**.

```text
  DONE ─┬─ victory & defeat            in flight ─┬─ A  keyboard text entry
        ├─ the battle's end                       ├─ B  diplomacy  ← built, see below
        ├─ a turn moves things                    ├─ C  the siege battle screen
        ├─ raise army                             ├─ D  battle casualty write-back
        ├─ the merchant                           ├─ E  the naming campaign
        ├─ sieges (campaign side)                 ├─ F  the remaining input arms
        ├─ save & load  ← confirmed in play       └─ G  the last shells & the cattle
        ├─ a world from any of the 44 maps                     │
        ├─ the AI at war                                       ▼
        ├─ castles, garrisons and being besieged     H  the draw-call audit (§2.10)
        ├─ the seasons, crops and village clock                │
        ├─ the armoury and the levy                            ▼
        └─ the battlefield's orders                  I  the long game, with a witness (§2.5)
```

**Save and load comes off the preconditions list.** Every plan for months has listed it as a
thing the long game waits on. A player has now saved and loaded in the wild and it worked. It is
done, and §2.5's hundred-turn item no longer has an excuse.

### What moved, and why

**Diplomacy moved from last to blocking, and it was found by measurement rather than by
argument.** Revision 4 put it at item 7 on the grounds that the AI can fight without talking.
That was wrong in a way nobody could see until the AI handlers were written: **four of the AI's
inputs — `standing`, `war_target`, `ally`, `target_county` — have exactly one writer each, and it
is diplomacy.** So AI step 10, the raid, is implemented, dispatched, tested and **cannot fire in
a played game** (`docs/decisions.md` C68). That is C27's seventh instance and the first where the
missing writer is an unbuilt *subsystem* rather than a missing line. A test holds both halves and
goes red the day it changes.

**And diplomacy is built** — `crates/l2-kingdom/src/diplomacy.rs`, AI turn steps 1 and 2, and
the player's side on screens `0x0B` and `0x1A`. **The measurement above was right and
incomplete**, which is the part worth carrying: with the module built and nothing else, forty
turns of England still produced *no standing below −10 anywhere on the map*, because
`Diplo_Offend`'s four call sites are not in the diplomacy code at all — they are in the mover
and in the battle return, two of them already sitting here as reported values with doc comments
saying *"for a caller that has a diplomacy layer to drive"*. C68's test went red exactly as
designed **and would have stayed green with the step 2 dispatch deleted.** A test written to
fire when a gap closes inherits the gap's framing, which is always *"is the field non-zero"*;
it should be replaced rather than merely satisfied. `docs/decisions.md` C84, and
`docs/diplomacy.md` §10 is the nine things implementing the document corrected in it.

**The siege battle screen is a one-way door, and it arrived in the place nobody was watching.**
§2.3 warned about exactly this for the campaign–battle seam and it was settled there. It was not
settled for sieges: a besieging army now reaches a castle correctly — that route was broken two
ways and both are fixed — and then lands in `engagement.rs::fight`, which deploys, issues one
synthetic order our own source describes as *"the click a player makes on the first frame"*, runs
headless to a conclusion and shows the report. **A siege is reachable and unplayable**, which is
worse than unreachable, because it looks finished.

**The draw-call audit is a new item and it is deliberately after the arms work**, not before.
Enumerating the original's drawing is the same technique applied to a second dimension, and the
arms enumeration is the evidence that the technique works. Doing both at once would mean two
half-inventories and no finished one.

**The naming campaign is in flight and is not on the critical path.**
<!--fig:functions-->974<!--/fig--> of <!--fig:binary-functions-->2,452<!--/fig-->
(<!--fig:functions-pct-->40<!--/fig-->%) are named. Most of the rest is CRT, allocator, string and
DirectDraw glue, and naming it is not the goal — but under 1:1 the fraction matters more than it
did, because an unnamed function is a behaviour nobody has looked for, and every miss this month
was exactly that.
---

## 3.1 A development affordance can be on the critical path, and this one was

**The instinct is to cut these first, so here is the evidence against it, from this month.**

A player spent an evening reporting five interface defects. **Three of the five were against a
binary four merges old** — two already fixed, one fixed twice. The cost was not his time; it was
that each report was investigated from the decompilation outward before anyone thought to ask
which build he was running, and nothing on screen could have told him or us.

The fix was a build stamp on the title screen: short commit, dirty marker, and the commit's date,
in the bottom-left in the dim ink. Hours, not days. It carries a date as well as a hash because
*"is this old?"* is the actual question and a hash cannot answer it without a lookup, and it says
`NO GIT` rather than inventing a version when the source is not a checkout. A static `0.1.0`
would have been worse than nothing, because it looks like an answer.

**It is in the field as of tonight** — the player has rebuilt and is running a binary that
identifies itself. The next report will name its build.

Two things this is evidence for, beyond the stamp:

* **A tool that shortens the feedback loop competes with features on the critical path**, because
  a wrong loop spends the scarcest thing this project has, which is a person willing to play the
  game and say what he saw. Five reports, three wasted, is a 60% loss rate on the only external
  oracle we have.
* **The affordance needs the same rigour as the feature.** The first test written for the stamp
  asserted it was on screen by counting pixels, passed, and passed just as well with the drawing
  deleted (`docs/agents.md`, *Ablate the line*). A development tool that is quietly broken is
  worse than one that is absent, for the same reason a stale version string is.

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
2. **Several seasons on England with the AI running, and at least one unowned county on a
   merchant route.** Note what this asks for and what it does *not*: it needs no trading skill
   and no particular action, only that the game be **left to run**. A human transaction adds
   nothing the tests do not already assert.

   What it settles: `County.purse` (`+0x1F4`) on an unowned county should be non-zero and
   equal `tax banked + 100 per season − purchases`, which promotes it; and the same save gives
   the first non-zero trade accumulators **from the original**, confirming our arithmetic
   writes the same numbers rather than merely writing consistent ones.

   **This item was four fields longer until C54 settled them without a save.** It used to ask
   for a merchant transaction to promote seven fields; `Realm +0x104…+0x110` turned out to
   have only two writers and no readers at all, which refuted the `thisSeason`/`total`
   reading and promoted all four from the code. `County.aleHappinessGiven` (`+0x219`) went
   the same way with C53. **A save is the witness of last resort, not the first** — three of
   the four fields this list has asked for longest were settled by reading, and asking for a
   save is worth doing only once the reading has been tried and has stopped.

   Still waiting on a different witness, and not to be folded in here: `fieldsOther`,
   `fieldsReclaimable`, `farmStyle` and the `armyFood` pair need wasteland, an AI-owned
   county and an army in the field respectively.
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
<!--fig:tests-->1,979<!--/fig-->. **Do not quote a count here that nothing recomputes**: mark
it, or label it frozen and say what it records.

---

## 7. What is not known, and is shaped like C3

C3 is the correction where three functions matched three modes and a plausible story assembled
itself out of decompiler output. These are that shape — coherent, unanchored, and load-bearing
enough that a wrong reading would spread. They are here so the risk register carries live
worries instead of dead ones.

* ~~**`FUN_0042FF10`**~~ — **closed.** It is **`Screen_FrameInput`** (`0x0042FF10`), and the
  whole 8,140-byte body has been read: 49 hand-written arms over 50 screen ids, the fourth
  and last function that switches on `g_screenId`, called once per frame from `Battle_Frame`
  **after every draw pass**. It is where the right mouse button lives — `DAT_004E6900`, right
  *released*, tested 48 times — and right-click is how nearly every screen in the game is
  left, which `L2.eng` group 12 index 0 states in English as *"Click Right to Exit"*.
  `docs/screens-county.md` §2.6.

  Three of the risk register's own claims about it were wrong, which is the part worth
  keeping. It **does** touch strings and shipped assets (`vill_gd8.pl8`, `demo1.pl8`); it
  writes `g_screenId` 150 times, not thirty; and `Ui_DispatchPointer` was too narrow in one
  direction and too wide in another — a third of the body is closing panels because
  **`Turn_End` ended the turn or the network is blocked**, with no pointer involved at all.
  The symbol is filed `inferred`, not `verified`: every line of the body is read, but the
  ~35 callees its arms dispatch to are not, and the `g_screenId` → screen mapping was not
  re-derived. That is the honest half of a name that used to claim more from less.
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
* **Naming more of the binary for its own sake.** <!--fig:functions-->974<!--/fig--> of
  <!--fig:binary-functions-->2,452<!--/fig--> functions are named, about
  <!--fig:functions-pct-->40<!--/fig-->%. The review measured that *"the rest is mostly CRT and
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
* ~~**`FUN_0042FF10` turns out to matter.**~~ **Retired — it did matter, and it has been
  read.** It is `Screen_FrameInput`, and the rule nobody could find was the right mouse
  button: a player said *"right click would close a bunch of popups"* and the function is
  where that lives. The early warning fired exactly as written, from the player rather than
  from us. `docs/decisions.md` C46 and `docs/screens-county.md` §2.6.
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
