# The original's bugs, and which of them we copy

This project reproduces *Lords of the Realm II* **including its defects**, because
reproducing it is the point. That has accumulated a real list, and until now it was
scattered across `kingdom.md`, `battle.md`, `battle-ai.md`, `armies.md`, `diplomacy.md`,
`rules.md`, source comments and the corrections log, with no single place that said *"here is
what the original gets wrong, and we copy it on purpose"*.

This is that place. It exists so that **"make them switchable" is a small job later rather
than a rediscovery exercise** — §6 works out what the mechanism would have to be, and
deliberately does not build it.

## Three things this document keeps apart

Blurring these is the failure mode; the line between them is the whole value of the document.

| | what it is | where it lives |
|---|---|---|
| **1. The original's bugs, which we reproduce** | the subject | §2 |
| **2. Our bugs, which we fix** | corrections, not content. **Not here at all** | [`decisions.md`](decisions.md), C1 upward |
| **3. Dead or unreachable code in the original** | a rung that can never fire, a handler nothing calls. No behaviour, nothing to switch | §5 |

Two more sections exist because they are the ones people get wrong in the *other* direction:
**§3, the original's bugs we do *not* reproduce** — every one of those is a decision somebody
made, and three of them are undeclared — and **§4, behaviour that looks like a bug and is
not.** Each §4 entry would have been a real balance change if someone had "fixed" it.

## How to read an entry

Every entry carries the project's evidence mark, meant literally (`kingdom.md`'s legend):

* **[V] verified** — read out of the binary *and* cross-checked against a second independent
  source: an `L2.eng` string, a shipped file's bytes, an arithmetic invariant that closes, or
  the shipped `lastturn.sav`.
* **[D] decompiled** — a straightforward reading of decompiled C or of the instruction
  stream, with no second source.
* **[I] inferred** — consistent with everything measured, not proven.

**The mark is on the reading, not on the word *bug*.** Several entries are `[V]` on what the
code does and `[D]` on calling it a mistake, and say so. Where the entry is a *suspicion*, it
is in §4.3 and not in §2 at all.

Addresses are the GOG Windows build, `ImageBase 0x400000`, no ASLR.

---

# 2. The catalogue — the original's bugs, reproduced

## 2.1 The nine that change play the most

These are the ones a player could notice, written out in full. §2.2 onward is the long tail,
in tables, with the same columns.

### B1 — The harvest weather band throws away the labour cap

**What the original does.** `Grain_Harvest` (`0x0044D1E5`) computes what the reapers can
actually bring in — `min(fieldShare(crop), reapers × perWorker)` — and then four of the six
weather bands **overwrite** that result with a multiple of `crop[1]`, the *standing* crop, in
four separate `if`s that read the wrong word. *Sunny* stores `3/2` of everything the county
grew; *Frost* and *Storms* store `1/2`; *Flooding* stores `1/4`. Only *Cloudy* and *Drought*
leave the labour-capped figure standing.

**Why it is a bug.** The two neighbouring branches — at sowing and at growing — are
self-sourced; only the harvest's four read `crop[1]`. A county that sends one reaper into a
sunny field reaps 150 % of the whole crop. The labour cap, which is the entire point of
`Grain_Harvest`, is discarded in four cases out of six.

**Evidence.** **[V]** on the reading. `kingdom.md` §7.1.

**Reproduced.** `crates/l2-kingdom/src/land.rs:485` — `harvest` computes `reaped` and then
deliberately ignores it for four of six bands.

**Reachability, which matters here.** It only bites with **Advanced Farming on**:
`Weather_UpdateAll` ends with `if (!g_optAdvancedFarming) band = Cloudy`, and *Cloudy* is one
of the two bands that leave the labour cap standing. The basic game never sees this bug; the
advanced game sees it in four seasons out of six. **[V]**, `crates/l2-kingdom/src/weather.rs`.

**Switching it off** is one line in `harvest`: use `reaped` as the base. Not a data change —
the weather factors are a Rust `match` (§6.2).

### B2 — Half the counties can never draw a random event

**What the original does.** The event deck is 256 slots with 26 filled; one number is drawn
per **season**, not per county, and the counties then walk consecutive slots. The seed is
`g_seasonRandom * 2` and so always even; the wrap resets the index to 0, also even; and every
filled slot sits at `index ≡ 7 (mod 8)`, which is odd. County *k* therefore always lands on a
slot of parity *k*.

**Why it is a bug.** Counties 2, 4, 6 … 16 are permanently exempt from every random event in
the game, at every seed, for the whole game. Nothing in play reveals it, because the counties
that *do* draw behave exactly as they should.

**Evidence.** **[V]** — arithmetic over the dumped deck, checked exhaustively across all 128
seeds, not sampled. `00448822 MOV EAX,[0x0058FD60] / ADD EAX,EAX` quoted from the
disassembly rather than the decompiler, per `decisions.md` C13. `kingdom.md` §8.1.

**Reproduced.** `crates/l2-kingdom/src/event.rs:998`, with
`only_odd_numbered_counties_can_ever_draw_an_event` and
`a_kingdom_of_even_numbered_counties_never_sees_an_event`.

**Switching it off** means changing the seed's parity or re-laying the deck. `EVENT_DECK` and
`EVENT_DECK_SLOTS` are Rust `const`s, not ruleset data (§6.2).

### B3 — The weapon a county finds is chosen by its county number

**What the original does.** Event `0x130` *"Weapons found."* and event `0x139`
*"Corruption."* both index the realm's weapon array with `(countyId & 3) + 1` rather than
with the county's own weapon type at `+0x290`. `FUN_0044938C` and `FUN_00449688` compute the
same index the same way, so it is a shared idiom rather than a slip in one place.

**Why it is a bug.** A county finds, and has embezzled, a weapon that has nothing to do with
what its blacksmith makes — and weapon type 0, the **crossbow, can never be found or stolen
at all** (§5, D6).

**Evidence.** **[D]** — two call sites, no second source.

**Reproduced.** `crates/l2-kingdom/src/event.rs:319` — *"This is a bug and it is reproduced."*

### B4 — The empire tax happiness term is summed into a signed byte

**What the original does.** `Tax_SumEmpireHappiness` (`0x0044B99A`) sums every owned county's
`+0x16` — a signed byte from `g_taxHappinessOther`, as low as −15 at the top rate — into
realm `+0x28`, **also a signed byte**, and nothing clamps it. Sixteen counties at −15 is
−240, which wraps.

**Why it is a bug.** The term is then added to every one of those counties' tax happiness, so
taxing a large empire hard enough can make its people *happier*. This entry has the largest
potential effect on play in the document, and **whether it wraps in an actual game has never
been tested** — `kingdom.md` §4.1 says so.

**Evidence.** **[V]** on both byte widths; **[D]** on the consequence.

**Reproduced.** `Realm::tax_hap_empire` is an `i8` deliberately
(`crates/l2-kingdom/src/realm.rs:181`) and `add_empire_tax_happiness` uses `wrapping_add`
(`realm.rs:407`). Widening the field would be a silent balance change, which is why it was
kept.

### B5 — The pathfinder never clears 36 % of its visit counters

**What the original does.** `Path_Search` clears its counters with
`FUN_004B3E51(0x004F6470, 0x1000)`, and that helper counts **bytes**. `0x1000` is 4,096 bytes
against an array of 6,400 one-byte counters.

**Why it is a bug.** Cells 4,096 and above — **rows 51 to 79, the bottom 36 % of the
battlefield** — begin each search holding whatever the *previous* search left there. A cell
down there whose stale counter already exceeds its step cost expands on its first pop, so its
terrain cost is silently not charged. Three independent checks fix the extent: the sibling
call passes `0x3200` for the `u16` cost array, `battle.md` records the counters as one byte
per cell, and `0x004F6470 + 6400` lands exactly on the next global the same function uses.

**Evidence.** **[V]**, from the instruction bytes.

**Reproduced.** `crates/l2-sim/src/pathfind.rs:185`, and `Scratch` is a struct that survives
between searches precisely so the carry-over is modelled — *"modelling them as a local would
quietly fix the original's bug"*.

**Reachability.** Inert on `.skr` skirmish maps, where every step costs zero (§5, D14). Live
on random and castle battlefields.

**Note for §6.** This makes pathfinding **order-dependent**: a search's result depends on
which searches ran before it. Lockstep peers run the same searches in the same order and stay
identical, but a caller that reorders searches changes the paths. That makes it the entry
least safe to expose as a switch — §6.4.

### B6 — Every revolting mob walks to a shared cursor's county, not to its own destination

**What the original does.** `FUN_004AC499` and its picker `FUN_004AC5BA`:

```c
if ((g_season == 2) || (unit.destCounty == 0)) {
    if (++cursor > g_countyCount) cursor = 1;
    if (unit.county == cursor && ++cursor > g_countyCount) cursor = 1;
    unit.destCounty = cursor;
}
if (unit.destCounty != 0) {
    unit.destX = county[cursor].anchorX;      /* <- cursor, not destCounty */
    unit.destY = county[cursor].anchorY;
}
```

**Why it is a bug.** The county **id** comes from `destCounty` and the **coordinates** come
from `cursor`, and outside spring those are not the same county. A mob that kept last
season's `destCounty` walks to *this* season's cursor county's anchor while believing it is
going somewhere else. (The shared cursor itself, and the guard that can only skip one county
so that a mob on a one-county map lands back where it started, are behaviour rather than
bugs — but they are the same three lines.)

**Evidence.** **[D]**.

**Reproduced.** `crates/l2-kingdom/src/units_tick.rs:422`, with
`phase_five_walks_one_cursor_for_every_mob`. The comment gives the second reason for keeping
it: *"a lockstep peer that 'fixed' it would desync."*

### B7 — A defending unit takes the rampart cell nearest the corner of the search box

**What the original does.** `Siege_FindCellSurface5` saves the query cell into two locals,
then **overwrites its own parameters** with the clipped top-left corner of the search box, and
then calls `Dist_Manhattan` with the parameters rather than the saved locals. Its sibling
`Siege_FindCellSurface4` saves the query point first and does not have the fault.

**Why it is a bug.** `Order_ToSurface5Near` puts a defending unit on the rampart cell nearest
the **corner of the search box** rather than the one nearest the post it was reserving. The
same overwrite pattern appears harmlessly in `FUN_00496768`.

**Evidence.** **[D]** — stated flatly as an original bug in `battle-ai.md` §10, from the
decompiled body and its sibling.

**Reproduced.** `crates/l2-sim/src/ai.rs:909` — *"Reproduces an original bug… Left as
written."*

### B8 — A thirteenth unit deploys on the enemy's first slot

**What the original does.** `Deploy_SlotForUnit` (`0x0048169E`) does not bound-check the
ordinal, and the two twelve-slot deployment tables are adjacent — side 0 at `0x00553150`,
side 4 at `0x005531B0`.

**Why it is a bug.** A side-0 army of more than twelve units deploys its thirteenth on the
enemy's first slot. Reachable in practice, because boiling oil holds one figure per unit and
so inflates the unit count cheaply.

**Evidence.** **[V]**-grade — asserted from the two table addresses and `Battle_RaiseSide`
(`0x0047FEA7`). `battle-ai.md` §11.

**Reproduced.** `crates/l2-sim/src/runner.rs:455`. Past the twenty-fourth entry the original
reads unidentified bytes and we clamp instead, and say so — see §3.

### B9 — The battle AI's attack jitter is not centred

**What the original does.** `Battle_UpdateStrengthAdvantage` (`0x0047FC01`) jitters the
strength ratio by `(rand & 0x1F) - 10`, which is **−10 … +21**, not ±10.

**Why it is a bug.** It looks like a `- 16` that became a `- 10`. With the attack threshold at
5, the AI is biased about half a point *toward* attacking, permanently.

**Evidence.** **[D]**.

**Reproduced.** `crates/l2-sim/src/ai.rs:430` — *"Reproduced rather than centred… that bias
is a real behaviour."*

## 2.2 The county economy

| | the original's bug | evidence | our code |
|---|---|---|---|
| **B10** | **Any ale at all buys the full five happiness in a village under ten people.** `Ale_Apply` (`0x00428C42`) walks rungs of `crowns >= rung × (population / 10)`; below ten people the step is 0, so `crowns >= 5 × 0` is true at the top rung. | [D] | `happiness.rs:136`, and the test at `:404` — *"reproduced rather than guarded, because the guard would be ours."* |
| ~~**B11**~~ | ~~The ale allowance is never reset.~~ **RETRACTED — this was not a bug and the "[V]" was wrong.** `Happiness_UpdateAll` (`0x0044BAEA`) zeroes `+0x219` every season for every county, in the same run of statements that clears `shownAle`. The claim rested on a search for writers *in the ale code*; the writer is in the happiness code, one line from a field the same paragraph described. `Readme.txt`'s *"Ale Limitations"* had said the benefit was per season in English. | the reset is quoted in `decisions.md` C53 | `happiness.rs::update` clears it; `trade.rs`, and the seasonal test in `crates/l2-game/tests/merchant.rs` |
| **B11a** | **An unowned county trading on its own account is checked for neither stock nor gold.** Both guards in `Merchant_Trade` are inside `if (realm != 0)`, so a county with no lord can sell grain it does not have and buy with a purse it has already emptied. `Ai_BuyGood` reaches this path for every unowned county on the map; its own purse test is all that stands in front of it, and it is applied *before* the lot size falls rather than per call. The negative store is then erased by the very next statement in the same function — the tail's `Ration_Apply` computes `sacks = min(wanted, grain)`, which against a negative store is negative, so subtracting it *adds the county back to zero*. The bug is therefore worth free crowns rather than a visible negative number, which is presumably why it has never been reported. | [V] — both guards read, and the erasure reproduced in `trade.rs`'s own test | `trade.rs::trade` reproduces both missing guards, and the test names the erasure |
| **B12** | **Turning castle building on removes its labour share.** `Industry_ToggleFromMap` reads `local_c = (castleSwitch != 0)` **before** flipping the switch and passes that stale value to the share toggle, so the share moves the wrong way on every click. | [D] | `industry.rs:757` |
| **B13** | **The levy slider lies about what it took.** `Levy_SetPercent` (`0x00435EBC`) walks the requested percentage down until the county can afford it, but `pct` is a by-value parameter, so `g_levyPercent` is never written back: the slider can read 80 % while the county gives up 59 %. | [D] | `levy.rs:215` returns it as `settled`, *"named `settled` rather than `percent` so it cannot be mistaken for the slider"* |
| **B14** | **A realm one crown short of its wage bill pays nothing and loses nothing.** The `else` branch of `Wages_PayAll` is the only place the treasury is debited. And the bankruptcy escalation counter at `+0x158` **wraps to 0 after stage 5** instead of saturating, so a realm that never pays loses its armies once every six seasons rather than permanently. | [V] | `industry.rs` (`BankruptcyAction`), `kingdom.md` §7.4 |
| **B15** | **The migration inflow list is written with no `break`**, so a county's sixteen `inflowSources` bytes hold one repeated value instead of a list, and the population panel's *"arrive from"* line names the wrong county. | [D] | `population.rs:124` — *"This reproduces a documented bug."* |
| **B16** | **A county that dies out records a negative death count.** `deaths = pop` with `pop` already negative — almost certainly a lost negation or a `popLast`. | [D] | `population.rs:190`, `lib.rs:94` — *"reproduced as written and flagged rather than silently corrected"* |
| **B17** | **The AI unrest ladder has a dead band from happiness 1 to 10.** ≥ 41 resets the counter, 11 … 40 walks it down, below 1 walks it up; 1 … 10 does nothing at all, so an AI county deep in revolt territory has its unrest frozen. | [D], and hedged — the source doc may have abbreviated `< 11` into `< 1` | `unrest.rs:17`, `lib.rs:92` — *"reproduced literally"* |
| **B18** | **The herd forecast subtracts the slaughter twice.** `FUN_0044DD4D` runs off `herd − herdEaten` while also assuming next season slaughters as many again. | — | `land.rs:718` |
| **B19** | **A county can never want every one of its people on the fields.** `Grain_Sow`'s worker search loops `workers < population`, stopping one short; and when the search finds nothing the floor/ceiling pair is left at its initialisers, −1 and 0. | [D] | `land.rs:819`, reproduced by search rather than closed form because the integer truncation is part of the answer |
| **B20** | **A county with no pasture is forced to maximum crowding twice** — once through a sentinel density and again as an explicit override after the bands. | — | `land.rs:563`; both reproduced *because they are separable* — a ruleset that lowered `no_pasture_density` would find the override still holding |
| **B21** | **The history ring records counties 1 … 16 unconditionally**, not `1..=g_countyCount`, so slots above the map's county count are filled from unused records. | — | `kingdom.rs:202` — *"what a reader of the ring has to expect"* |
| **B21a** | **The village's "put everybody to work" gesture reads two words past the end of its table.** `Village_BalanceAll` (`0x00439EDB`) loops `for (i = 0; i < 10; i++)` over `g_jobClusterToSlot` (`0x004D6780`), which has **eight** entries. The two past the end are the head of the table that follows, and they are **4** and **6** — read out of the shipped executable, not inferred. The effect is benign and arguably useful: slot 4, *Iron mining*, is otherwise unreachable from a cluster number unless cluster 0 has been overridden to it, so the overrun is what makes the gesture cover all nine slots. Slot 6 is simply balanced twice, which is idempotent. | [V] — the ten words are asserted against the user's own `Lords2.exe` in `crates/l2-view/tests/install.rs` | `l2_view::village::CLUSTER_TO_SLOT_BALANCE`, ten entries with the overrun written down; `Game::balance_all_labour` loops the same ten |
| **B22** | **`Grain_Sow`'s two early exits do not clear the shortfall flag.** An empty store or an empty workforce returns 0 with the flag left as it was. | — | `land.rs:295`. Reproduced, and listed only for completeness: nothing observable turns on it, because a county that sowed no seed grows no crop either way |

## 2.3 The AI

| | the original's bug | evidence | our code |
|---|---|---|---|
| **B23** | **The AI turn's completion sentinel is stored as 1000, not 999.** The increment below the dispatch sits outside the `if` that writes 999. Every reader tests `< 999`, so nothing in the original breaks — but the stored field does not mean what it looks like. | [V] from the disassembly (`0049A985 INC dword ptr [aiStep]`) | `ai.rs:240` — *"Reproduced bug."*; `Realm::turn_done` tests `>= 999` because of it |
| **B24** | **Later realms in the array get a longer turn.** The finish test is `15 + 2 × realmIndex <= aiStep`, so realm 5 gets ten more steps than realm 1. Steps above 14 dispatch nowhere, so what they buy is extra chances for the army-launch condition to come good. | [D] on calling it a mistake | `ai.rs:216` — *"reproduced rather than tidied"* |
| **B25** | **A zero gold threshold takes a castle type out of a lord's ladder** rather than meaning "free", so two of the four lords can never build the largest castle at any treasury (§5, D7). | [V] scoped to what realm `+0x4D` counts | `ai.rs:497` |
| **B26** | **The castle concurrency count is not refreshed as builds are ordered.** The limit is tested inside the county loop against a count that still reads what the season started with, so a lord with a limit of 4 and five castle-less counties can start **five** builds in one pass if he began it with none. | — | `ai.rs:512` |
| **B27** | **The AI's ration ladder punishes a county fed on cattle.** `Ai_SetRations` (`0x004A4782`) sizes the larder as `herd*5 + herd*10 + grain*6` — counting the herd twice — and puts two dairy rungs **above** two store rungs. The Double dairy rung can only ever *lower* the answer, so a cattle county is fed Double where a grain county holding the same food is fed Triple. Two of the four lords are graziers. | [V] on the arithmetic, [D] on calling it a bug | `ai_farm.rs:475` |
| **B28** | **The Winter grain quota underflows for a tiny county.** The subtraction is unsigned and passed to a signed parameter, so a small county gets a negative quota and **every grain field is returned to fallow**. | [V] | `ai_farm.rs:579`, reproduced exactly by `i32` |
| **B29** | **"Add a field" does nothing while a field is already reclaiming.** `FUN_0044C6C4` paints a reclamation order onto a wasteland tile, and a tile already reclaiming spends a place in the quota without anything happening — so at four seasons a field, the AI's expansion is stalled most of the time. | [V] | `field.rs::order_reclamation`, `ai_farm.rs:71` |
| **B30** | **Farm style 9 ends on allocate with no closing estimate**, so a style-9 county's panel forecast is permanently one allocation stale. | — | `ai_farm.rs:670` — *"reproduced, not an oversight"* |
| **B31** | **An unowned county holding farm style 9 is dispatched nowhere.** `Ai_ManageCountyFarms` writes county `+0x1FE` from the lord's personality; `AI_ManageFields` — the only pass that runs on unowned counties — only reads it and dispatches styles 0 and 1. An unowned county farms the way its last lord farmed, and one holding style 9 is never managed at all. | [V] | `kingdom.md` §8.6, `ai_farm.rs` |
| **B32** | **Siege engines fall off the end of the strength-weight ladder and count 1** — a missing `else` in the original. | [D] | `l2-sim/src/ai.rs:114` — *"reproduced rather than 'corrected' to 0"* |
| **B33** | **A shooter on surface 1 will not take a target on surface 5.** An exclusion with no evident reason. | — | `l2-sim/src/ai.rs:686` |
| **B34** | **A failed wall search leaves the destination changed but unordered.** `Order_ToWallBelowKeep` writes the offset destination *before* the search and only re-issues the order when the search succeeds. | — | `l2-sim/src/ai.rs:899` |
| **B70** | **An eliminated realm below the leader's index switches off the gang-up rule for the rest of the game.** `FUN_004A01EA` picks the realm each AI treats as *the* threat, and its loop is `for i in 1..6 { if (realms[i].rank < 2) { … } }` with **`return`** inside, not `continue`. [`crate::ai::rank_realms`](../crates/l2-kingdom/src/ai.rs) leaves rank **0** on a realm that is out of play, and 0 is `< 2` — so the loop stops at the first eliminated realm and the searcher ends with no threat at all. On a map where realm 1 is knocked out early, **nobody ever concentrates on the leader again.** | [D], and the early `return`s are in the disassembly | `ai_army.rs::pick_threat` — reproduced, and the doc comment says why it is not a transcription slip |
| **B71** | **`FUN_004A6270` returns 1 after disbanding its own army.** Mission 4 disbands an army that can find no castle with room for it, and then returns the "re-path me" value — so `Ai_AdvanceArmies` flood-fills from and writes `moving = 1` into a record `Army_Destroy` has already cleared. | [D] | `ai_army.rs::mission_join_garrison` — the slot is simply empty here, which is the same observable outcome by a route that cannot read freed memory |
| **B72** | **Mission 2's "attack where you stand" arm never updates `destCounty`.** `FUN_004A5B1F`'s last branch walks the army at the county it is standing in while unit `+0x151` still names the county the branch above it rejected. In the same arm, an *ally*-owned county with no alternative target aims the army at **county 0**, whose anchor is `g_counties[0]`'s. | [D] | `ai_army.rs::mission_seek_enemy` — both reproduced, both named |
| **B73** | **A dead call with two identical arms.** `FUN_004A5B1F` calls `FUN_00467F2E(county)` — *"does this county border a foreign-owned one?"* — between two branches and **discards the result**; the two arms it was written to choose between are the same code. Not reproduced, because a call with no side effect and an unread result is nothing to reproduce. | [D] | `ai_army.rs::mission_seek_enemy`, stated in the doc comment |
| **B74** | **`Diplo_ActionAllowed` is not a predicate, and the AI's own map search corrodes its alliances.** Every time it is asked about a county or a unit belonging to the asker's **ally** it increments the asker's own grudge against that ally. Both AI county choosers call it once per county, and the mission-3 and mission-6 enemy searches call it once per unit slot — so an AI hemmed in by its ally accumulates grudge purely by looking at the map, and the Knight's tolerance of 5 is reached in a handful of turns. | [D]; `symbols.json` already marks the function `[inferred]`, this is the per-turn volume | `ai_army.rs::action_allowed` — reproduced; whether it is *intended* is not established |
| **B75** | **Tile (0, 0) can never be a destination.** All three of the AI's destination finders keep their best candidate as a byte *offset* into the tile array and use **0** as the "nothing found" sentinel, so offset 0 is indistinguishable from failure and the county's fallback tile is used instead. Harmless on every shipped map. | [D], three functions with identical bodies | `ai_army.rs::aim_tile` — reproduced, because the alternative is a difference nobody would ever find |
| **B35** | **Siege attack scripts stall the unit that is doing its job.** `UnitOrder_SiegeAttCatapult` and `UnitOrder_SiegeAttTower` `return` before the `orders` increment on their wall-found path, so a unit successfully doing its job stops advancing its script. | [D] | `battle-ai.md` §1.3 |
| **B86** | **Two of the three envy tiers in `AI_Diplomacy` are unreachable.** The ally-is-winning rung reads `if (v < 0x15) { if (v < 0x22) { if (0x32 < v) grudge += 4; } else grudge += 2; } else grudge += 1;` over the ally's share of the map. `v < 0x15` implies `v < 0x22`, which makes `+= 2` dead, and implies `!(0x32 < v)`, which makes `+= 4` dead. **Only `v >= 21 → +1` can fire**, so envy of a runaway leader accumulates at the same rate whether the ally holds 21 % of the map or 90 %. | **[V]** — arithmetic over three constants, not a reading | `diplomacy.rs::ai_diplomacy` — reproduced by writing **only the rung that can fire**, which is what the code does |
| **B87** | **An alliance with an eliminated realm survives reconciliation if the dead realm's index is higher.** `Diplo_ReconcileAlliances` drops a pairing whose partner it has already marked dead — but the "already marked" array is one the same ascending loop is filling, so at realm *n* it can only see realms below *n*. A realm allied to a higher-indexed dead realm falls into the `else`, where the partner still points back, and the function **writes the alliance back on both sides**. | [D] | `diplomacy.rs::reconcile_alliances` — reproduced, and the doc comment says which half of the test is the bug |
| **B88** | **`Diplo_ReconcileAlliances` invents alliances rather than only dropping them.** A realm whose `ally` byte points at a partner that points at **nobody** is not a broken pairing to be cleared; the function writes `ally` back onto the partner and sets `allied` in both directions. So a one-sided write anywhere in the game becomes a real, mutual alliance at the next `Turn_Tick`. Whether that is a repair or a bug is not established — it is filed here because `docs/diplomacy.md` said the opposite for as long as the document existed. | [D] | `diplomacy.rs::reconcile_alliances` — reproduced |
| **B89** | **Asking an ally to attack a county of your own is refused with the wrong sentence.** `Diplo_SendClicked`'s kind-6 ladder answers `owner == localPlayer` with `L2.eng` **244**, *"This county is part of our alliance, my Lord. We can only ask that those territories belonging to our enemies be attacked"* — which is what the *next* rung is for. Group **242**, *"does not belong to us"*, exists and is raised only by kind 5. | **[V]** — the two branches send the same group id and 242 is unreached from kind 6 | `screens/diplomacy.rs::refusal` — reproduced, and the test names the case |
| **B90** | **Declining an AI's alliance offer does nothing at all — not even a refusal.** `FUN_00436872`, the *"Accept alliance ?"* prompt, guards its whole body with `(realms[offerer].isHuman != 0) || (hotspot != 0)`. In single player the offerer is an AI, so a **no** fails the guard and the function returns having stored the answer in a global nothing reads. The offer lapses silently when the offering realm clears `offerPending`. | [D] | not reproduced — the prompt is not built, because `Msg_DrawWindow`'s window layouts have never been read. `docs/arms.json` `0x00436872/accept-alliance-prompt`, status `missing` |

## 2.4 Things that move

| | the original's bug | evidence | our code |
|---|---|---|---|
| **B36** | **The pathfinder never relaxes a cell.** `Path_Search` (`0x0047095E`) considers a neighbour only while `cost[neighbour] == 0`, so the first cost written stands even when a cheaper route arrives later. It is a weighted search with no mechanism for the weights to win. | [D] | `pathfind.rs:23` — *"the original's paths are the specification, and 'fixing' this changes where armies walk"* |
| **B37** | **The battle frontier queue wraps at `0x1900`.** A search that outgrows 6,400 entries silently overwrites its own frontier and returns a path derived from a corrupted queue instead of failing. | [V] | `pathfind.rs:50` (`QUEUE_CAP`) |
| **B38** | **The campaign flood-fill queue wraps at 1,024**, so a fill that outgrows it stops early with a **half-filled distance field**. Its head and tail cursors are globals shared by both distance fields even though the buffers are per-field. | [D] | `movement.rs:72` — *"reproducing it costs one modulo"* |
| **B39** | **An unreachable destination comes back as an empty path, not as a failure.** `dist[dest] == 0` satisfies `Move_ExtractPath`'s first `< 2` test immediately, so the original returns success with `pathLen = 0`, the caller sets `moveState = 2`, and the army does not move. | [D] | `movement.rs:280` — *"the order is accepted and nothing happens, which is observable and therefore not ours to improve"* |
| **B40** | **`Move_FloodFill`'s first argument is not really a player.** Every call site but one passes literal 0 and `Unit_OrderMove` ignores its own parameter, so the two distance fields are "local player's" and "everyone else's" — and when the human is realm 0, the AI's searches land in the local field too. | — | `movement.rs:102` (`Routing`) |
| **B41** | **`Units_Tick` returns the moment a battle opens**, leaving every higher slot unmoved: an army in slot 9 does not move on the tick an army in slot 4 picks a fight. | — | `units_tick.rs:180` |
| **B42** | **A mercenary band overshoots a county after making an offer.** The second `nextCounty++` has no wrap guard, so the band sits at `countyCount + 1` for a season until the next call's guarded increment wraps it — costing the band a county. | [D] | `mercenary.rs:252` — *"Reproduce the sequence, not the invariant."* |
| **B43** | **`Merchant_PickStartCounties` has a buggy dedup loop.** When it runs off the end of a row it switches to odd entries 3, 5, 7 …, **never trying entry 1**, gives up after five retries, and is unguarded against the value `0`. Two observable consequences: slot 15 *"Rorschach"* gives two merchants the **same** start county, and a zero start county `break`s `Merchant_SpawnAll` (`0x00427ED0`) rather than skipping it, leaving **12 non-empty routes across the 44 shipped maps that nothing ever walks** (slot 23 *"YinYang"* has six routes and two merchants). | [V] | reproduced by the route script; `formats/plane4.md` |
| **B44** | **An out-of-range move click does nothing at all.** `Map_HoverUnitTarget` (`0x004A8E0B`) re-runs the pathfind against the hovered tile and, when the budget is short, **zeroes every interaction target it had collected** — so the click is discarded rather than walking as far as it can. | [D] | `armies.md` §2.3; the campaign UI is not built yet, so this is traced rather than reproduced |

## 2.5 The battle

| | the original's bug | evidence | our code |
|---|---|---|---|
| **B45** | **A figure's times-hit counter is a `u8` and wraps.** | — | `l2-sim/src/unit.rs:83` — *"reproduced, not widened"* |
| **B46** | **The formation offset ladder caps at five rows.** `Formation_OffsetX`/`OffsetY` are five-way ladders rather than divisions, so six rows or more behave as five — which changes the shape, not just the offset. | — | `formation.rs:88`, `:96` |
| **B47** | **The formation rectangle clamps its two axes differently.** `y` is pulled back by `bottom − 79` with `bottom = origin + depth` rather than `origin + depth − 1`, one cell more generous than `x`. And the width collapses to 1 for a single-figure unit even at footprint 3, so a lone catapult forms up on one cell. | [V] against the disassembly | `formation.rs:132` — *"reproduced rather than symmetrised"* |
| **B48** | **A battle that wipes both sides out on the same frame is won by B**, because the original tests A first. | — | `runner.rs:672` |
| **B49** | **The terrain variant LFSR is stepped once per cell whether or not the result is used**, which is what keeps the sequence aligned. | — | `terrain.rs:56` |
| **B50** | **Figures are drawn centred using the sprite *width* for both axes**, so a 48-pixel man sits eight pixels left of and sixteen above his cell's corner. | — | `l2-view/src/scene.rs:191` — *"reproduced rather than corrected"* |
| **B69** | **Whether you can order an attack depends on a figure index left over from another sweep.** `Battle_UpdateHover` (`0x0047ED9B`) counts the local player's selected non-siege figures into `DAT_00565404`, which gates `g_battleHoverEnemy` and so gates the attack cursor and the attack order. Its loop tests `g_battleMen[local_8].owner` and `.selected` — and then `g_battleMen[**g_curBattleMan**].troopType`, a *different global*, for the third clause. `g_curBattleMan` is the shared sweep index 74 functions share and every one of them leaves it at `0x51`, one record past the end of the 80-record array. | **[V] at instruction level**: `a1 f8 e8 53 00` (`mov eax,[g_curBattleMan]`) where the two clauses either side use `mov eax,[ebp-4]`. **[I]** on the effect: `0x554480 + 81 × 0x1B0` is `0x562000`, past `.data`'s raw extent and so in zeroed BSS, where `troopType` reads 0 and the clause is true — so the count is probably right in play and the bug invisible. | **Not reproduced**, deliberately: `battlefield.rs`'s `update_hover` uses the loop variable, and says so. Reproducing an out-of-bounds read of a byte we do not model would be reproducing the *address*, not the behaviour. |

### B76 — Leaving a battle early throws the battle away, and only in single player

**What the original does.** Retreat (`0x0043BA29`) and Autocalc (`0x0043BD67`) both open a
confirm box, and every *yes* reaches `FUN_0043BE65` — 129 bytes that are
`Battle_AutoResolve()` and the return to the campaign, with **no call to
`Battle_WriteBackCasualties`**. `Battle_AutoResolve` reads the *campaign* records at
`+0x16C`, which nothing has written since `Battle_Start`, so the strength ladder is applied
to the two armies **as they marched onto the field**. Every man killed in the battle so far
is unkilled.

Except on one of the two paths, in one configuration. `FUN_0043BDCD`, the Autocalc confirm's
own callback, splits on `g_multiplayer`:

```c
if (g_confirmAnswer == 1) {
    if (g_multiplayer == 0) { FUN_0043be65(); }               /* discard        */
    else { Battle_WriteBackCasualties(); ... Net_SendCommand(0x33,0); ... }
}
```

**Why it is a bug.** The same button means two different things depending on whether anybody
else is playing: in a network game the casualties are kept, in a single-player game they are
thrown away. Whichever half was intended, both cannot be. The Retreat confirms
(`FUN_0043BAF9`, `FUN_0043BB68`) have no such arm at all — they reach
`FUN_0043BBD7` → `FUN_0043BE65` in both modes — so a *retreat* discards even in
multiplayer, which makes the asymmetry autocalc-only and harder to read as design.

**Evidence.** **[V]** — the four functions read end to end, and
`Battle_WriteBackCasualties` has exactly five call sites in the whole binary: this one, two
in `Battle_CheckOutcome`'s post-banner arm, two in `FUN_004782C5`'s, and nothing else.
**[I]** on calling it a mistake rather than a network-protocol requirement.

**A second rule falls out of the same reading, and it is not a bug.** Because
`Battle_AutoResolve`'s **first statement clears `g_battleWithdrawal`**, pressing *Retreat*
does not perform a retreat: it auto-resolves, and a player who loses the ladder has his army
**destroyed** rather than withdrawn with half its men (`armies.md` §7.4a). The withdrawal
rules are reachable only from `UnitOrder_SiegeAttKnight`.

**Reproduced?** **Yes, the single-player arm**, which is the only one that exists here:
`l2_game::turn::finish_battle` drops the runner and re-runs the autocalc from the campaign
records. The multiplayer arm is catalogue-only until replication lands, and §6.4 is where a
switch would go. `armies.md` §7.1.

## 2.6 Victory, defeat and the score

| | the original's bug | evidence | our code |
|---|---|---|---|
| **B51** | **A game with nobody in play is won by the array slot.** Leader and trailer are both 0, `0 == 0` passes, and the original crowns `g_realms[0]` — which is not a realm. | — | `victory.rs:38`, test at `:614` |
| **B52** | **The human can win a game in which the human is dead.** When the last realm standing is an AI, the first call sends group 195 and sets the one-shot guard `+0xED`; the *next* call takes the other branch and sends the human group 225 *"Victory!"*. | — | `victory.rs:41`, test at `:604` — *"Reproduced deliberately."* |
| **B53** | **Dying at the same moment as the last opponent is scored a win**, because the opponents test is checked before the is-it-me test. | — | `victory.rs:670` — *"Strange, and reproduced."* |
| **B54** | **`County_ChangeOwner` cannot eliminate anybody.** `Realm_RecountStrength` runs one statement *before* `g_counties[c].owner = newOwner`, so the county being lost is still counted and a realm losing its last county survives until its own step 0. | — | `victory.rs:199`; reproduced by our caller doing the same in the same order |
| **B55** | **The realm message variant goes negative for a human.** `lord * 4 + rot − 4` with lord byte 0. | — | `realm.rs:365`, reproduced with a wrapping subtraction *"so the shape of the expression stays visible"* |
| **D1 →** | The score's gold bracket pays 50 flat rather than 50/100/200. That is **dead code**, not a behavioural bug, and it is §5's D1 — the shipped *behaviour* is what we reproduce, and it is the one entry already switchable by data (§6.1). | [V] from the bytes | `tables.rs:1315` |

## 2.6a Starting and reloading a game

### B55a — Two of the game's own options do not survive a save, and one of them still matters

**[V] on the block table, [D] on calling it a mistake.**

`Save_Write` walks a table of `{address, length}` records at `0x004DE960`; a global not in it
is not in the file. Seven four-byte entries fall in the option block `0x0053F2xx`:
`g_optDifficulty`, `g_campaignMap`, `g_optAdvancedFarming`, `g_optArmiesEat`,
`g_optExploration`, `g_aiLordCount` and `g_optTimeLimit`. **`g_optFightHumansOnly`
(`0x0053F284`) is not one of them**, and neither are the twelve custom-game selections at
`0x0053F288` or the five committed starting-condition globals.

Most of that is harmless: the starting gold, castle, armoury, garrison and county status are
spent once while the world is built and are never read again, so there is nothing to
remember. **`g_optFightHumansOnly` is different** — it is read on every battle for the rest of
the game (`FUN_004A6A30`: when it is 0 and the local player is not a participant, the fight is
auto-resolved instead of prompting), and it is a setting the person deliberately chose. Load a
saved game in a fresh session and the flag is whatever `Setup_DefaultOptions` last wrote,
which in a single-player game is *all* — so a game explicitly started on *humans only* comes
back fighting every AI-versus-AI battle by hand.

`g_optTimeLimit` being saved while the drop-down that set it is not is the same asymmetry
landing the right way round: the committed *value* is stored, the *index* is not, which is all
that is needed.

**How it was found**, and this is the useful half: `l2-formats` had a comment saying the
option was merely *not exposed*, and adding `0x0053F284` to its list of globals turned the
battle fixtures red with `NotSaved { va: 5501572 }` on the first run. The save reader refuses
an address in no block rather than returning a plausible number from the wrong offset, which
is `save.rs`'s design doing exactly its job. `docs/decisions.md` C44.

**What we do.** `l2-scenario` takes `FIGHT_HUMANS_ONLY_DEFAULT` on import and says at the
field that the file does not answer the question — the honest reproduction, since the original
cannot answer it either. `l2_kingdom::save` (**our** format, version 12) stores it, along with
`exploration` and `time_limit`, so a game saved by this engine does not lose them. That is a
divergence and a deliberate one: it is our save format, and D11 already separates the two.

### B66 — `Options_SetDefaults` defaults one sound flag twice and, apparently, another not at all

**[V]** on the duplicate; **[I]**, and no more than that, on what was meant.

`Options_SetDefaults` (`0x004AE310`) stores 1 into `g_optSpeech` (`0x0053F20C`) **twice**, at
`0x004AE369` and again at `0x004AE389`, with `g_optMusic` and `g_optSoundEffects` between them:

```text
004ae369  MOV dword ptr [0x0053f20c],0x1     ; g_optSpeech
004ae373  MOV dword ptr [0x0053f218],0x1     ; g_optMusic
004ae37d  MOV dword ptr [0x0053f214],0x1     ; g_optSoundEffects
004ae387  MOV dword ptr [0x0053f20c],0x1     ; g_optSpeech AGAIN
```

Read out of the instruction bytes and confirmed independently by a 32-bit scan of the image
for absolute references: `0x0053F20C` has exactly two inside this function and no others.

**Why it is a bug.** Four consecutive stores where one target is repeated is what a
copy-and-paste with a missed edit looks like. It is **completely harmless as shipped** —
storing the same constant twice is storing it once — so it changes no behaviour and there is
nothing to switch.

**What it does not establish.** `g_options+0x30` (`0x0053F210`) sits in the sound block, is
defaulted to 1, and is read by nothing; it is the obvious candidate for the store that lost
its target. **That is a guess and it stays one.** `CLAUDE.md` rule 4 is why this paragraph
stops here: the shape is suggestive and there is no second source, so it is not a finding.

**Reproduced?** There is nothing to reproduce. `l2_game::game::Prefs::default()` sets the
three sound flags once each, and the note lives on `Prefs::speech` so that whoever writes a
preferences file meets the fact rather than the assumption.

## 2.7 Multiplayer — catalogue only

### B56 — The shipped build silences one of its own eight checksum blocks

**What the original does.** `Sync_BuildDigest` (`0x00440231`) fills a ten-byte digest per
player: byte 1 counties, 2 realms, 3 units in the campaign phase; 5 battle men, 6 missiles, 7
battle units in battle. Its **last statement before totalling** is

```c
(&g_syncDigest)[g_localPlayer * 10 + 7] = 1;
```

which overwrites the battle-unit block's sum with the constant 1, unconditionally, after
computing it.

**Why it is a bug.** Byte 7 is then identical on every peer, so `Sync_BlockAgrees(7)` always
agrees, and the constant contributes the same 1 to byte 0's total. **Divergence in the battle
units is invisible to the shipped desync detector.** `L2.eng` group 14 names the block
*"AI GROUP"*, so it was meant to be checked. A debugging line left in is the obvious story;
that is **[I]**, and the write itself is **[V]**.

**Evidence.** **[V]** — read out of the decompiled function, whose six `(size, skip)`
descriptors match `g_syncBlocks` (`0x004D5B10`) byte for byte. `netcode.md`, `symbols.md`
(`0x00440231`).

**Reproduced?** **No, and not yet applicable.** Multiplayer replication is paused and
`l2-net`'s checksum is our own (`Canonical`), not a port of this one. **Catalogue-only** — it
is here so that whoever builds the replication layer decides deliberately rather than
inheriting a blind spot. Our answer should almost certainly be *don't*: §6.4.

## 2.8 Sieges

All five arrived with the siege branch, and all five are reproduced.

| | the original's bug | evidence | our code |
|---|---|---|---|
| **B57** | **The siege engine order is cumulative where it reads as alternative.** `Siege_Prepare` writes the tower count **before** the personality tests, and only the `== 8` arm overwrites it — so the Countess builds 3 catapults *and* 2 towers, 600 + 400 = **1,000** man-seasons rather than 600. Whether the `default:` arm was meant to be exclusive is unjudged; the arithmetic is not. | [V] | `l2-kingdom/src/siege.rs:289` |
| **B58** | **`Siege_ValidateLink` increments `g_siegeCount` for the link it has just broken.** The increment is unconditional, so the count means *"armies that were besieging when the phase began"* rather than *"armies besieging now"*. Harmless in the original, whose one consumer only asks whether it is above 1. | — | `siege.rs:556` — *"Reproduced"* |
| **B59** | **The engines that go into an assault are what was *ordered*, not what was finished.** The original copies each engine record's `ordered` field with no reference to the build percentages, and gets away with it only because the phase tick reaches the assault when `siegeSeasonsLeft` is zero. A caller that assaults early gets engines it has not paid for — and so does the original. | — | `siege.rs:731` — *"Reproduced as written"* |
| **B60** | **`Army_BeginSiege`'s guard does not check whose garrison it is.** Besieging your own castle is refused only by the map's hover test never offering the order, not by the function. | — | `siege.rs:212` — *"Reproduced as the original has it"* |
| **B61** | **A besieger starved to nothing reports stale readiness.** The whole body of the build-time recompute is inside `if (menTotal > 0)`, so an army with no men leaves `siegeSeasonsLeft` at whatever it held. | — | `siege.rs:361` — *"reproduced rather than tidied"* |

## 2.9 Sound

### B62 — Winning a battle plays the defeat fanfare

**What the original does.** `Battle_ReturnToCampaign` (`0x004AB383`) plays a fanfare at two
sites, one on each side of the outcome. Both name **`ff_lose.wav`** — and not through a
shared pointer either: they are two separate string literals, at `0x004DE8F8` and
`0x004DE904`, holding the same eleven characters.

**Why it is a bug.** `Ff_win.wav` **ships**, 8-bit 11 kHz like every other effect, and
`Lords2.exe` does not contain its name anywhere. A byte scan of the executable for each of
the install's 771 `.wav` filenames finds 753 and misses 18; `Ff_win.wav` is one of them, and
it is the only one of the eighteen with an obvious caller sitting right beside a literal
that ought to be it. Two adjacent identical literals is what a copy-paste looks like in
`.data`.

**Evidence.** **[V]** on the two call sites and on the executable containing no reference to
`ff_win.wav`, asserted over the user's own install by
`l2-game/tests/audio_install.rs::the_shipped_sounds_the_executable_never_names_are_the_eighteen_we_wrote_down`.
**[I]** that it is a mistake rather than a late decision to use one fanfare for both — but
a decision would have deleted the file, and it is still in the box.

**Reproduced?** **Not yet applicable.** There is no post-battle fanfare in our code because
there is no battle screen. `crates/l2-game/src/audio/names.rs` names the constant
`fanfare::AFTER_BATTLE` rather than `LOSE`, so whoever wires it up meets the fact rather
than the assumption.

## 2.10 Screens and navigation

### B79 — Typing your name over the default one leaves the tail of it behind

**[V]**, and it is the first thing anybody meets in the game, so it is worth being sure about.

`Edit_Insert` (`0x00401D26`) has two branches and the *overwrite* one is the default:

```c
if (!g_editActive) return;
Edit_Clamp();
if (g_editState == 2) return;                     /* full: dropped in silence */
if (g_editInsert == 0) { buf[caret++] = ch; }     /* OVERWRITE — g_editInsert starts at 0 */
else if (g_editLength < g_editMaxLen) { shift right; buf[caret++] = ch; }
```

`g_editInsert` is `0x005C9280`, it is BSS, and the **only** thing in the binary that writes it
is `Edit_ToggleInsert` (`0x00401CA3`) on `VK_INSERT` — `Edit_Begin` does not reset it. So a
fresh game starts in overwrite.

Setup page 4 seeds the field with the name you already have (`Edit_Begin(&g_options, 0x10,
0xC0, 0)`, and `g_options`'s name field defaults to `Player1`) and puts the caret at 0. So:

| you type | you get |
|---|---|
| `Richard` | `Richard` — seven over seven, and it looks like it works |
| `Ed` | **`Edayer1`** |
| `Ed`, then Delete five times | `Ed` |
| Insert, then `Ed` | `EdPlayer1` |

**Reproduced on purpose.** It is not a crash and not a rules bug; it is what the shipped game
does, and a person who has played the original and types `Ed` expects `Edayer1`. The two ways
out — `VK_DELETE` and `VK_INSERT` — are both the original's and both are wired, which is the
main reason those four keys were added at all rather than only backspace.

**Not switchable, and the test is the one in §6.3:** flipping it cannot change a number in a
saved game, only which characters a person ends up storing, and they can see it happening while
they type. It is not presentation either — it changes a stored string — so it is neither, and
that is fine: a quirk needs a switch only when somebody wants it switched.

**Where:** `crates/l2-game/src/text.rs`, `TextField::put`; `docs/arms.json`
`0x00401D26/overwrite-default`; tested in `crates/l2-game/tests/text.rs`.

### B80 — The End key cancels a save you have just confirmed

**[V]**, single player, and nobody would find it by playing carefully.

The window procedure's `VK_END` arm calls **two** functions:

```c
case 0x23:                 /* VK_END */
  Edit_End();              /* 0x00401D11 — caret to the end of the text     */
  Chat_Close();            /* 0x004360F2 — and this is the interesting half */
```

and `Chat_Close`'s whole body is `g_chatTimer = 0; g_saveLoadConfirm = 0; g_redrawRequest = 2;`.

`g_saveLoadConfirm` (`0x005CD41C`) is the save/load box's confirm latch: `VK_RETURN`
(`Edit_Confirm`, `0x00401C5B`) and the tick widget (`FUN_004342F3`) both set it to 100, and
`SaveLoad_Tick` (`0x004AD9F0`) is what reads it, builds the path and does the work — **150
frames later**, because the same block sets `g_fileOpDelay = 0x96`. So there is a window of
about two and a half seconds after pressing Enter in which End throws the save away.

**Neither half is guarded on `g_multiplayer`**, so this is reachable in a single-player game on
a real save box. The two counters are cleared by one function because chat and the save box are
the only two things Return arms, and nothing ever separated them again.

**Not reproduced, and the reason is structural rather than a choice:** our save is immediate and
has no latch to cancel. `docs/arms.json` records it as `0x004360F2/end-cancels-confirm`,
`missing`, so that a future change which gives the save a delay has the arm waiting for it.

### B63 — Closing a screen opened over the village closes the village with it

**Reported by a player, checked by the same player in the original.** *"Things that open a
dialog will open it and when you close that dialogue it will close town square and that
dialogue, probably something to fix so it only closes the dialog you opened, but list that in
future bug fixes that diverge from the game."* That last clause is why this is here rather
than fixed: the improvement is wanted **recorded**, not applied.

**What the original does. [V]** Open the village (screen `0x02`), click any sidebar button
while it is up — which works, because the village's arm hands the whole right-hand column
through (§below) — and the screen that opens replaces the village outright. Close it and you
are on the **campaign map**, not back in the village.

**Why. [V]** `g_screenId` is *one byte*, and there is no stack anywhere in the binary. Of the
**100** writes to `g_screenId` inside `Screen_FrameInput` (`0x0042FF10`), **57 are the literal
`0`** — the campaign map. A screen's exit is a **constant compiled into its own arm**, not a
memory of where it was opened from. The `0x0A` arm is the shape of all of them:

```c
if (g_screenId == '\n') {                       /* 0x0A, opened from the sidebar */
  if (g_mouseRightReleased == '\0') {
    if (Ui_OkButtonClicked()) { g_screenId = '\0'; Gfx_LoadCountyMode(); g_redrawRequest = 1; }
  } else                      { g_screenId = '\0'; Gfx_LoadCountyMode(); g_redrawRequest = 1; }
}
```

The counted alternative exists and is rare: **11** writes restore `g_menuPrevScreen` (the menu
bar's drop-downs), **2** restore `g_screenIdSaved` and **2** `g_sliderPrevScreen`. So the game
*can* remember where it came from — it does it in fifteen places out of a hundred, and none of
them is the village. The one screen that does come back to the village is the **job popup**,
and it does so by a constant too, not a memory: `0x0F`'s arm is
`if (DAT_005533F4 == 0) g_screenId = 0x02; else g_screenId = 0;` — a flag saying *"this popup
was opened from the map, not from the village"*, set at the two call sites rather than tracked.

So the collapse is not one arm closing two screens, and not a dismissal walking a stack.
**There is nothing to walk.** The village is not "closed" at all: it simply stops being what
`g_screenId` names, and every draw pass after that draws something else.

**Reproduced?** **Yes, deliberately, and structurally rather than by a special case.**
`Machine::apply_at` truncates the stack to the depth of the screen that acted before applying
its transition, so a `Push` from the campaign map — reached through the village by
[`Transition::Pass`] — discards the village on the way. That makes our stack behave like one
byte in exactly the situation the original has one byte, and leaves it a stack everywhere
else. Test:
`l2-game/tests/screens.rs::a_screen_opened_over_the_village_takes_the_village_with_it_when_it_closes`.

**What a switch would cost.** Small, and the cost is not in the code. `apply_at` would keep
the intervening screens instead of truncating, which is three lines; the work is deciding what
the *rest* of the screen set does once a stack is real, because the original's constants become
wrong everywhere at once — every one of those 57 literal zeroes is a screen that would now
return to whatever was under it rather than to the map, and some of them (the menu-bar
drop-downs, the job popup) already have their own idea of where to go and would start
disagreeing with the stack. So it is one flag and a pass over 49 arms, not one flag.

**Where the flag would live: `Quirks` on `Options`, not on `Tables`.** `Tables` is hashed into
the save header (`docs/modding.md`), so a quirk added there changes the hash and invalidates
every existing save. §6.3 has the general argument; this is the first entry that would use it.

### B63a — The sidebar goes dead for exactly as long as a peasant is in the air

**Not a divergence of ours — the original's own asymmetry, and worth the row because it looks
like an inconsistency somebody would tidy.** **[V].**

`Screen_FrameInput`'s `g_screenId == 0x02` arm runs six sidebar guards before any village verb:

```c
if (FUN_0043292d() ||        /* Hotspot_Test(0x262, 0x20, &g_minimapModeButtons, 4) */
    FUN_00432967() ||        /* Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)    */
    CountyStrip_Click() || Labour_SplitSliderDrag() ||
    CountyStrip_JobClick() || FUN_00439079()) goto done;
```

The village's **other two screen ids do not.** `0x05` (banding) runs `Village_BandRelease` and
`Village_BoxSelect`; `0x06` (carrying) runs `Village_Drop`. Neither tests a single sidebar
guard. So the entire right-hand column — minimap modes, the six buttons, the county strip, the
farm/industry slider — is live in the idle village and dead from the moment a rubber band
starts until the peasants are dropped.

Reproduced (`VillageScreen::handle` passes only in `Phase::Idle`) and asserted, in the half of
`the_sidebar_slider_still_works_with_the_village_open_but_not_mid_drag` that exists to stop it
being "fixed" into uniformity. `docs/decisions.md` C59.

### B64 — A county's name is embossed in parchment colours over a background that is not parchment

**Reported by a player, in the original.** *"There's a bug in the original where the town
name's embossing against the cloudy background, e.g. when you don't own it, still has the
emboss colour of the parchment that you see on a town you own, that blends it in with the
parchment. But the OG correctly has the 'sovereign land of the baron' properly tinged in
grey."* Both halves check out.

**What the original does. [V]** `CountyStrip_Draw` (`0x0040F7D3`) draws the county's name
through `Ui_DrawCentred` → `Ui_DrawText` (`0x00402637`), and `Ui_DrawText` takes its emboss
colours from `g_screenId` and two globals — **never from the caller**:

```c
local_1c = 0x10; local_20 = 0x1f;                       /* the campaign pair */
if (g_screenId == 0x1f || g_screenId == 0x1c) { local_1c = 0x36; local_20 = 0x2c; }
...
else if (g_flatText == 0) {
  if (g_embossGrey != 0) {                              /* 0x0058FE9C */
    y-1 in 0x3f;  y+1 in 0x26;  y in colour;            /* the grey pair   */
  } else if (g_dropShadow != 0) { ... }                 /* 0x005AEB90     */
  else { y-1 in local_1c; y+1 in local_20; y in colour; }
}
```

`CountyStrip_Draw` sets `g_embossGrey` around the three *Sovereign land of …* lines and
around nothing else, so the **name** — drawn before that block, on either plate — always gets
the default pair. In `Base01.256` those are `0x10` = `rgb(81, 73, 53)` and `0x1F` =
`rgb(247, 223, 134)`: a dark olive under a pale parchment yellow, which is the colour of the
`Misc_cty` frame `0x37` plate an **owned** county's strip stands on. A county you do not hold
gets frame `0x3A`, the cloudy one, and the name keeps the parchment highlight over a
background with no parchment in it. The grey pair the banner beneath it uses — `0x3F` =
`rgb(0, 0, 0)` over `0x26` = `rgb(202, 202, 202)` — is what the same plate calls for and what
the same function is already able to ask for, four lines further down.

So the two colours are not a judgement about what looks right: **both pairs are palette
indices in the executable and both are used in this plate**, and the defect is that the name
is on the wrong side of a switch the function flips a moment later.

**Reproduced?** **Yes, and switchable.** The name keeps the parchment pair by default. The
player asked for both — reproduce it, and give a way to turn it off — so
`l2_game::game::Quirks::grey_county_name` moves the name onto the grey pair, and is `false`
unless something sets it. Tests:
`l2-game/tests/screens.rs::the_county_name_keeps_the_parchment_emboss_and_the_sovereign_lines_do_not`
reads both pairs back off the canvas by their palette index, and
`the_grey_county_name_quirk_changes_the_emboss_and_nothing_else` runs the pair.

**Where the switch lives, and why it is not `Options`.** §6.3 argues for `Options` as the home
for a quirk set and is right about every quirk it is arguing about — all of them change a
*rule*, which is where its three constraints come from. **A text shadow changes no rule.**
`Quirks` is on `Assets`, whose definition is *"everything the screens draw with, not part of
the world"*: it cannot reach `l2-kingdom` or `l2-sim`, it is not in the save body, it is not in
the lockstep state, and two players running with different values compute identical turns.
`netcode.md` D-12 makes that a requirement rather than a convenience — display state must not
reach the simulation — so a shadow colour in the hashed options would be wrong, not merely
expensive. **A behavioural quirk still belongs on `Options` and still costs a `save::VERSION`
bump.** The two sets are different things; see §6.3a.

**Not to be confused with our own bug, which was the mirror image.** We drew the *Sovereign
land* lines with the parchment pair too, having collapsed the two into one, and we drew them
for **unclaimed** counties, which the original never does — its guard is `owner != 0` and
`L2.eng` group 15 has only the two strings `"Sovereign land"` and `"of"`, with the third line
a lord's name out of `g_playerNames`. Both are fixed; neither was the original's.

### B65 — Two of the village's eight animation counters are stepped every frame and drawn by nothing

**[V] on every number, [I] on the explanation.**

`Village_Animate` (`0x00412421`) steps **eight** counters and draws **six** overlays.
`DAT_004D2934` (wrapping at `0x14`, so 21 states, on the 160 ms pulse) and `DAT_004D2948`
(wrapping at `0x0F`, 16 states, on the 80 ms pulse) are incremented on every frame the village
is up, and `RefsTo` finds **no other reader of either address in the executable**. Two
animations were cut and their clocks were left running.

One number is worth writing down beside it. **`villani1.pl8` holds 21 frames** — 21 cells of
40 × 62, laid out 8, 8 and 5 on the artist's sheet with nothing to divide them — and the
counter that actually draws it, `DAT_004D2938`, wraps at `0x11`, so the iron mine plays 18 of
the 21 and three are never shown. The dead counter has **exactly 21 states.**

That is as far as this goes. The coincidence is real, it is asserted in
`every_village_overlay_run_fits_inside_its_own_sheet` so that it stays visible, and it is
**not** a demonstration that the mine was meant to run off `DAT_004D2934` — nothing in the
binary connects them and `CLAUDE.md` rule 4 is the reason this paragraph stops here.

**Reproduced?** The visible behaviour is: `l2_view::village::OVERLAYS` is the six that draw,
the iron mine's run is 18 frames and stops there, and
`l2_view::village::DEAD_COUNTER_PERIODS` records the two that do not draw rather than
implementing them. There is nothing to see either way — a counter with no consumer has no
pixels — so this is catalogued, not switched.

### B67 — A castle's materials bill is settled to the nearest whole per cent

**Reproduced.** **[V].**

`Castle_MaterialsPercent` (`0x00450FB4`) is
`min(100 - Pct(woodOwed, woodTotal), 100 - Pct(stoneOwed, stoneTotal))` in integer
arithmetic, and `Castle_BuildTick` opens the labour gate at `> 99`. `Pct(1, 400)` is 0, so a
wooden palisade **one stick of wood short of its bill** reads as fully delivered and the
builders start. The bigger the castle the more generous it gets: a royal castle is 3,000
stone, so up to 29 of them are free.

Reproduced rather than tightened, and the reasoning is not sentiment: the gate is two `PctOf`
calls and a `min`, a rule that rounded the other way would idle a whole county over a
rounding error, and the outcome — work starts a season earlier than a strict reading would
allow — is invisible to a player and harmless. `the_castle_ceiling_is_shut_until_the_wood_and_stone_have_arrived`
in `crates/l2-kingdom/tests/labour_gap.rs` asserts both sides of the boundary: one stick
short opens it, four sticks short (`Pct(4, 400) == 1`) shuts it.

### B69 — A siege bills the repair in the material the castle is made of

**Reproduced.** **[V].**

`Siege_RecordCastleDamage` (`0x004784CA`) is the only writer of `castleDegraded = 2`, and it
bills the repair from `g_castleLevel`: below 2 — a palisade or a motte and bailey — it charges
`wallDamage * 10` in **wood**, and at 2 or above `wallDamage * 15` in **stone**, with the work
at `moatFilled * 5 + wallDamage * 15` either way. When a build was already under way it
**adds** to the existing totals, so besieging a half-built castle makes the job bigger than
the castle was.

> **The entry used to say *"identified and not yet reproduced"*, and the reason it gave was
> half right.** *"Every number comes from two battle-side accumulators `l2-sim` does not
> keep"* — it keeps both now, as `SiegeState::moat_filled` and `SiegeState::wall_damage`,
> written by the moat fill and by a catapult's wall collapse.
>
> *"And the autocalc path produces no wall damage at all"* — that half is **not** a gap, it
> is the rule, and it is stronger than it looked. `Siege_RecordCastleDamage` has exactly one
> caller and it is **not** `Battle_ReturnToCampaign`: it is `FUN_004782C5`, the outcome
> banner's frame counter, which runs it at frame 5001 and then the write-back and then the
> return. `Battle_Decline`, the Retreat button and the Autocalc button all leave for screen
> `0x13` without passing that counter. So **a battle you gave up on un-does the castle
> damage exactly as it un-does the casualties**, and a repair is billed only for a siege
> somebody watched to its end. `crates/l2-game/tests/siege_battle.rs` asserts both halves —
> the bill after the banner, and no bill after the Autocalc button — through played clicks.
>
> **`docs/symbols.md` called the first accumulator `breachDamage` and that is a misnomer.**
> `DAT_0057A0D8` has three writers in the whole binary: `Battle_Start` zeroes it,
> `Siege_RestoreCastleDamage` restores it from the county, and **`Moat_Fill` adds one**.
> Nothing about a breach touches it. It counts cells of ditch shovelled full, which is why
> it only ever reaches the *work* line — five man-seasons of digging a cell, and not a stick
> of wood.

The three readers of `l2_kingdom::siege::CASTLE_DEGRADED_DAMAGED` are reachable from a
played route now, which is what the note on that constant existed to ask for.

### B84 — a repeat assault bills the same repair twice

**Reproduced, and flagged rather than fixed.** **[V]** on the round trip, **[I]** that nobody
meant it.

`Siege_RestoreCastleDamage` (`0x004787A4`) is the last statement but one of
`Battlefield_BuildCastle`, and when the county is mid-repair it copies the *previous* siege's
two accumulators back into `DAT_0057A0D8` and `DAT_0056D648`. Nothing zeroes them again. So
the next assault on the same castle opens with both non-zero, `Siege_RecordCastleDamage`'s
first `if` passes however peacefully that assault went, and the whole of the first siege's
bill is added to the totals a second time.

Reproduced because it is the round trip that makes the six stored fields *state* rather than
a write-only report, and the double bill falls out of the same three statements. A switch
would have to decide which of the two the restore is for, and nothing in the binary says.

### B85 — the drawbridge search is missing a `break`

**Reproduced, because there is nothing to reproduce.** **[V]** on the control flow, **[I]**
that it is harmless.

`Siege_LowerDrawbridge` (`0x00496B9F`) scans for the first cell flagged `0x40` with

```c
for (y = 0; y < 0x50; y++)
    for (x = 0; x < 0x50; x++) {
        if (flags[off] & 0x40) { found = true; break; }
        off += 8;
    }
```

The `break` leaves only the **inner** loop, and the offset is not advanced on the iteration
that broke — so every remaining row re-tests the same cell, finds `0x40` again and breaks
again. The answer is unaffected: the offset stops at the first such cell either way. What it
leaves behind is `g_foundTileX = 0` and `g_foundTileY = 0x50`, a battlefield-wide scratch
pair that half the siege code writes before reading. Nothing was found that reads them
between here and their next write, which is why this is `[I]` rather than a second entry in
§2.

### B68 — Building a castle stops a county mining, and only the AI knows

**Reproduced, and it is the rule rather than a defect — filed here because it reads as one.**
**[V].**

`Labour_Allocate` serves the industry half of a county as a round robin over wood, stone,
iron and the blacksmith, with **castle building as the tail**, reached only when all four are
at their ceilings. `Industry_LabourEstimate` gives wood, iron and stone a ceiling of
**100,000** wherever the site exists, so they are never full and the tail is never reached.
**A county with its industries running never puts one man on the walls, for ever.**

The human's way out is to click the buildings off on the campaign map. The AI's is
`AI_ChooseIndustry`, which switches iron and the blacksmith off outright while a castle is up
and keeps wood and stone only while `+0x1D4` / `+0x1D0` are still owed — a rule that looks
like an odd strategic preference until you see it is the only way an AI lord ever finishes
anything. `docs/kingdom.md` §7.5.2, and
`a_county_with_its_mines_running_never_gets_round_to_the_castle` in
`crates/l2-game/tests/castles.rs`.

---

# 3. The original's bugs we do **not** reproduce

Every line here is a decision. Three of them — N1, N2 and N3 — have their reasoning in a code
comment and **nowhere else**, which under a faithful-by-default policy is the weakest place
for it to live (§6.5).

| | the original's bug | why we do not reproduce it |
|---|---|---|
| **N1** | **`dryness` is a signed byte that nothing clamps**, so a long run of Summers rolls it through +127 into −128 and **turns a drought into a flood**. [D]; `kingdom.md` §7.3, and unreachability was never shown. | **We clamp where the original wraps.** `County::dryness` is an `i32` clamped to the `i8` range on every write, with the reasoning in a field comment and no correction number. This is the clearest undeclared divergence in the tree. |
| **N2** | **`local_modifier`'s Summer chain tests `== 4` twice**, leaving band 3 unhandled and the `−24` arm dead — *"almost certainly a mistyped `case 3`, and a reimplementation would 'fix' it by accident."* **[V]** by instruction bytes (`83 f9 04` at both `0x449E06` and `0x449E2C`); `audit-method.md` F5. | `weather.rs::local_modifier` is stubbed to **0**, so every county's local swing is wrong for a different reason. This one is not a choice, it is an untraced function — but the moment somebody traces it, the mistyped `case 3` has to be reproduced on purpose or declined on purpose. |
| **N3** | **`army_happiness_cost` is indexed without a bound**, so for any county under 50 people the read lands on the first entry of the merchant price table — 0, i.e. **a free army**. | Clamped (`tables.rs:1161`). Arguably the most player-visible bug in this table, and the divergence is declared only in a doc comment. |
| **N4** | **The campaign flood fill has no bounds guard at all**: stepping east from `x = 63` wraps into the next row, and expanding in row 0 writes *before* the array — into the fill's own queue cursor. [D] on the absence, [I] that every shipped map's sea border always saves it. | Not reproduced, and correctly so: *"reproducing the wrap would let a path teleport across the map edge, and reproducing the underflow is not reproducible behaviour at all, it is memory corruption."* |
| **N5** | **Player move orders can overrun `g_pathBuf`.** `Move_ExtractPath` has no length cap and `Unit_OrderMove` does not check one; the AI tests `g_pathLen < 0x96` and the player's path does not. | Clamped. A buffer overrun is not a rule. |
| **N6** | **`Diplo_Offend` indexes a five-realm table with 6**, because its only guard is `owner != 0` and an ownerless militia's owner byte is 6. | We refuse rather than reproduce an out-of-bounds write (`battle.rs:505`). |
| **N7** | **`Deploy_SlotForUnit` past the twenty-fourth entry reads unidentified memory.** | Clamped (`runner.rs:463`). The first twelve-past-the-end case **is** reproduced — that is B8. |
| **N8** | **A fresh army has a move allowance of 0** until the next tick, because `Army_Create` sets neither `moveAllowance` nor `movesUsed`. [V]. | Judged a rendering artefact of the original's frame loop rather than a rule; `Unit::new` sets the allowance from the kind (`levy.rs:401`). |
| **N9** | **The DirectPlay `guidApplication` is different on every run**, with a byte pattern that looks like a module address repeated into the field — *"a value that is supposed to identify 'this game' and instead identifies 'this process'."* Observation **[V]**, explanation **[I]**. And the game asks for `IID_IDirectPlay2` — the **wide** variant — while being an ANSI application that passes ANSI strings through it. | Not reproduced. `netcode.md` rule D-6 exists precisely to forbid this class of defect in our engine. |
| **N10** | **Two of the three alliance-envy tiers are unreachable** — `v < 0x15` implies `v < 0x22` and implies `!(0x32 < v)`, so only the `+1` arm can fire and envy of a winning ally accumulates at a fifth to a quarter of the intended rate. **[V]**, *"arithmetic over three constants, not a reading."* | **Diplomacy is traced and implemented nowhere.** Whoever implements it must reproduce this and move the row into §2 — or decline it and say so here. Same for two siblings: an AI's standing towards a **human** never heals (the +1 per turn is guarded on the other realm being non-human, **[V]**), and the **Bishop does not declare war** when his alliance breaks because `lord != 4` guards the `atWar` write, so he can be allied with again later ([D]; `diplomacy.md` declines to call that one a bug). |
| **N11** | **`Map_PickTile` is pure geometry, so the top of a tall building belongs to the tile behind it.** The mine sprite is 58 × 47 on a 58 × 30 tile: in the original its upper half resolves to a tile where `Map_Click` finds no flags and does nothing at all. The forest is worse, at 22 rows of overhang. | **We test the frame's opacity mask when the diamond misses.** It can add an answer where the original had none; it can never move one. Reproducing the dead zone faithfully would reproduce a defect that **our own county-selection arm makes worse than it is in the original** — `Map_Click` has no such arm, so there a missed click does nothing, while ours falls through to selecting the county and opening a screen. A player reported it twice. `docs/decisions.md` C57. |
| **N13** | **26 cells of `arm_grid.pl8` are hotspots that index the levy basket out of bounds.** The armoury's hit test (`FUN_0043582A`) accepts **any** non-zero cell as `g_uiHotspotId` and `FUN_004358B0` then reads `basket[id].available` — an eight-slot, 16-byte-stride array. The shipped grid holds **60…63** in 26 cells: column 0 for the first fifteen rows (x 0…7, y 0…119) and an eleven-cell sliver at y 216…223 between x 512 and 599, which are authoring leftovers rather than regions. `basket[62]` is `0x0053F6A4 + 0x3E0`, past the end of all six realms' baskets; if what is there happens to be positive, the click also sets `DAT_00553F20 = 62`, jumps to screen `0x0D`, falls through `Armoury_LoadScreen`'s branch to `arm_cros.pl8`, and draws `L2.eng` group 8 index 144 in a group of 74. **[V]** on the cells — counted out of the shipped file — and **[V]** on the absence of a bound in both functions. | **We answer `None` outside 1…6.** There is no faithful reproduction of an out-of-bounds read: what the original does there is not behaviour, it is whatever the next global happens to hold, and it differs between builds. The 26 cells are asserted by position in `crates/l2-game/tests/armoury.rs` so that a *different* `arm_grid.pl8` — a mod, or a re-release — fails loudly instead of quietly agreeing. Same reasoning as N4 and N5. |
| **N12** | **`g_moveOrderClickGuard` deadens the map for forty frames after a move order opens.** `Screen_FrameInput` polls the *level* of the mouse button, so without the guard the press that opens move-order mode is read again on the next frame as the press that confirms the destination. The constant is a workaround for the polling, not a rule about movement. | **We do not port the forty frames, because the defect cannot occur here.** Our `Event::Click` is edge-triggered: one press produces one event, so there is no second read to suppress. The property is asserted rather than the constant reproduced — which is the distinction worth keeping, because a parity audit that finds a constant in the original and none here should be able to see *why* in one line instead of filing it as a gap. `docs/decisions.md` C60. |

---

# 4. Surprising, and **not** a bug

## 4.1 Three that came off the candidate list

Each of these was suspected and came off after being read properly. Each would have been a
real balance change if someone had "fixed" it.

### S1 — Turning Advanced Farming **off** makes the AI plant far more grain

True, and **intended**. The option's `else` limb overwrites the whole planting ladder:
neutral style 0 plants `total − 3`, realm style 0 plants `total − 1` — nearly every field —
and style 9 plants `total / 2` instead of `total / 3`. **[V]**, `ai_farm.rs:57`.

It reads as a bug until you notice what else the option controls. With Advanced Farming off:

* `Weather_UpdateAll` ends with `if (!g_optAdvancedFarming) band = Cloudy` — **there is no
  weather at all**, in any county, ever;
* `update_fertility` ends with `if (!advanced_farming) fertility = 0` — **there is no
  fertility**, so leaving a field fallow buys nothing;
* industry uses a separate `without_advanced_farming` efficiency constant.

So Advanced Farming off is a coherent *simplified farming mode* with no weather risk and no
crop rotation, and in that mode planting the whole county is simply correct. A player has
independently described the option from play as the one that adds *"fallow fields"* — crop
rotation — which is exactly this. Do not touch it.

### S2 — The AI's free gold gets **smaller** when a realm is losing; its free goods get **larger**

Both halves are real and they point in opposite directions. `AI_SetTaxRates` (`0x0049D638`),
for a realm in play, not human, holding at least one county:

* **gold** comes from `g_aiGoldGrant` at **three or more** counties and from the uniformly
  smaller `g_aiGoldGrantSmall` below that. A losing AI is paid **less** — a snowball, not a
  rubber band;
* **goods** — free population, herd and grain, **per county** — are `d×20 / d×5 / d×40` at
  1 … 2 counties, `d×10 / d×2 / d×20` at 3 … 4, and **zero at 5 or more**. A losing AI is
  given more per county and a winning one nothing at all. That is a rubber band.

**[V]** on the code and both tables. Neither is a bug; both are difficulty scaling, and the
human gets nothing from either mechanism — the human's `lord` byte is 0 and row 0 of both
gold tables is all zeros. **[V]**

Two things worth recording, because nothing else does:

* `kingdom.md` §8.2 currently reads *"both grants therefore reward a realm that is already
  ahead … the goods stop entirely once a realm is doing well. That is the opposite of
  rubber-banding."* The second sentence does not follow from the first. Goods stopping when a
  realm is ahead is a catch-up mechanism by any reading.
* Because the goods are granted **per county**, the per-realm *total* is not monotone:
  `d×20` at one county, `d×40` at two, `d×30` at three, `d×40` at four, zero at five. A realm
  is better off in free grain holding two counties than three. **[V]** on the arithmetic,
  **unjudged** on intent — odd enough to be an oversight, with no evidence either way, so it
  stays here rather than in §2.

### S3 — An AI's farms run before the economy reads them; the human's do not

True, and by construction. `Season_Advance` (`0x00448440`)'s **first statement**, before
`Rand_Advance` and before the season and year roll, is `Ai_ManageFarmsAll` (`0x0049A990`),
which runs `Ai_ManageCountyFarms` for every realm in play and not human. So an AI's field
layout, rations and labour split are already this season's when tax, rations and industry read
them, while the human's county carries whatever the player left. **[V]**.

It is an AI advantage, not a defect: the AI has to act somewhere, and this is where. The one
thing genuinely worth flagging is that it runs **before the season counter rolls**, so the AI
lays its fields against the season that is *ending*. Whether that is intended is **[I]** and
untested; it is a candidate for §2 if anyone can show the AI plants for the wrong season.

**A documentation gap this exposes.** `rules.md` §2's pass table starts at "1 — Clock". There
is a pass 0, and it is this one.

## 4.2 Two more that read as bugs and are probably design

* **The AI stops making weapons the moment it starts a castle.** `AI_ChooseIndustry`
  (`FUN_0049E77D`) switches iron and the blacksmith off outright while a castle is going up,
  keeping forestry and quarrying only while the build still wants wood or stone. A real
  strategic quirk, and not an obvious one, but it reads as a deliberate priority.
  `ai.rs:589`.
* **An army whose owner is human fights worse.** With realm `+0x05` set, a figure gets *less*
  protection from high ground, takes more crossbow damage as a siege engine, and burns faster.
  Whether that is a deliberate handicap, an inverted test, or a different meaning for `+0x05`
  is **not established**; `battle-ai.md` judges a deliberate handicap *"the more economical
  reading than an inverted test, but it is not proof, and one instance of the four points away
  from it."*

## 4.2a Ours, latent: crashes nothing can currently reach

Not the original's bugs and not behaviour — **our own panics that no current caller can
provoke.** They are here rather than in `decisions.md` because there is nothing to correct
yet: the code is right for every input it is given today, and wrong for an input a future
caller could hand it.

* **`movement::order_move` panics on an off-grid destination.** Unreachable from the
  interface, because `pick_tile` only ever yields a tile that exists — which is precisely
  why it has never fired. Any caller that computes a destination rather than picking one
  (a script, an AI order, a replayed command) can reach it. It wants a `Result` or a clamp,
  and it wants deciding rather than defaulting: an off-grid order is a caller's bug and
  silently clamping it would hide one. Found by the agent that rewrote `pick_tile`, which
  correctly declined to touch `l2-kingdom` while other agents were working in it.

## 4.3 Suspected, unresolved — do not treat these as findings

* **The heavy blow may fire once per battle.** `+0x18C` (`blowUsed`) is set to 1 and, in the
  paths examined, never cleared. If that holds, a figure lands one heavy blow in the whole
  battle — *"which would be a bug worth reproducing."* **Not established**: the writers of
  `+0x18C` outside `Melee_Tick` were never traced. Reproduced faithfully and flagged rather
  than quietly fixed (`l2-sim/src/figure.rs:87`).
* **Font overhang rows appear to be stored and never painted.** Plausible readings: a
  localised build, or a bug in the shipped exe. **Not established**; `formats/pl8-failures.md`.
  A decoder should consume the bytes either way.
* **A possible off-by-one in the mode-2 type-4 overhang column mapping** — the one place the
  shipped binary and the stored data disagree; the literal reading discards 186 non-zero bytes
  across the corpus. Unresolved; `formats/pl8-mode2.md`.

---

# 5. Dead and unreachable code in the original

Nothing here changes play, so there is nothing to switch. It is a separate section because the
two are easy to confuse, and someone will.

## 5.1 The five worth reading in full

### D1 — The score's gold bracket is dead from the second rung up

`Score_RankRealms` (`0x0049AA0E`) tests the **smallest** threshold first:

```text
0049aed1  cmp  [eax+57c018], 2000
0049aedb  jle  0049aefb
0049aee1  add  [eax+57bf50], 50        ; and then jmp straight to the next realm
0049af19  add  [eax+57bf50], 100       ; unreachable
0049af51  add  [eax+57bf50], 200       ; unreachable
```

so the 5,000 and 10,000 arms are reached only by a treasury that has already failed `> 2000`.
**The shipped bonus is 0 below 2,001 and 50 above it**, and a full treasury is worth one
castle rather than four. Read out of the instruction bytes, not the decompiler's nesting.
**[V]**. `decisions.md` C33, `tables.rs:1315`.

**This is the only entry in the document already switchable by data**, and §6.1 explains why
it is the model for everything else.

### D2 — The instant-revolt arm can never run

```c
if (happiness < 0x19) {
  if (happiness < 0x19)      { if (unrest < 4) unrest++; }
  else if (happiness < 1)    { unrest = 4; }      /* unreachable */
}
```

Outer and inner tests are identical, so the "happiness 0 revolts instantly" arm never fires; a
county at happiness 0 climbs the counter one season at a time like any other.
`Unrest_UpdateAll` (`0x0044AA41`). **[V]**. Reproduced as a dead branch — `kingdom.md` §6.

### D3 — Six `a3` animation handlers have no caller and no table entry

Twelve animation functions sit in `0x00486249 … 0x00488240` in six adjacent pairs, reached
through six sixteen-byte thunks at `0x004861E9 … 0x00486239`. Every thunk calls the *first* of
a pair; the second of every pair has **no caller and no table entry anywhere in the binary**,
and is otherwise the same code with four constants substituted.

| a2, live | a3, unreachable |
|---|---|
| `Anim_StrikeA2` `0x00486249` | `Anim_StrikeA3` `0x004867CC` |
| `Anim_WalkA2` `0x00486D83` | `Anim_WalkA3` `0x0048702F` |
| `Anim_StandA2` `0x004872AE` | `Anim_StandA3` `0x004875F7` |
| `Anim_DyingA2` `0x00487908` | `Anim_DyingA3` `0x00487AF6` |
| `Anim_CollapseA2` `0x00487CE4` | `Anim_CollapseA3` `0x00487EB1` |
| `Anim_DrawBowA2` `0x0048804A` | `Anim_DrawBowA3` `0x00488240` |

This is the shape of C3 — six things matching six other things — so it is anchored **outside**
the binary, in the shipped art: the `a3` sheets hold `8N + 13` frames for exactly the six
frames-per-facing the `a3` handlers use, `A3_horse.pl8` has 8 frames against `A2_horse.pl8`'s
48, and `Anim_CollapseA3` plays the one frame the remaining space allows. Five identities over
four troop groups, none from the code. **[V]** that these are the `a3` handlers and that
nothing calls them; **not established** why they were compiled in — the skirmish and roster
screens do load the `a3` sheets, so something draws them, just not this state machine.
`battle.md` §14.4. Two data tables are dead with them: `g_strikeCycleA3Mace` (`0x004D9A78`)
and `g_drawBowArcherA3` (`0x004D9B68`).

### D4 — Screen `0x28` is dispatched and never entered

`Screen_Draw` (`0x0040F1A0`) and `Screen_DrawWidgets` (`0x004BA26E`) treat `0x28 … 0x2A` as
one range and hand it to `Screen_DrawBattlefield` (`0x004233F7`). `g_screenId` (`0x004EAC50`)
has exactly three dispatchers and every literal write to it is enumerable: `0x29`, `0x2A` and
`0x2B` are all written, **`0x28` is written nowhere**, by literal or by any of the saved-value
copies. **[D]** — the indirect writers (`g_menuPrevScreen`, `g_screenIdSaved`) can only carry
a value something wrote first.

### D5 — Campaign zoom level 1 is unreachable

`g_mapZoom` (`0x0057CB18`) has exactly three writers in 2,452 functions, and `FUN_00451FCC`'s
seven callers pass `2`, `0`, `0`, `0`, `0` and the global three times. **Nothing can ever set
it to 1**, so `Map_SetZoom`'s third case — 26 × 14 tiles — is dead. Corroborated three ways:
`Map_DrawTile` has no zoom-1 blitter branch; the two loaders disagree about the art size; and
no shipped PL8 holds 26 × 14 map tiles. **[V]**, and **[I]** that it is the DOS build's middle
zoom left in. `screens.md` §2.2, `l2-view/src/campaign.rs:25`.

## 5.2 The rest

| | what is dead | evidence |
|---|---|---|
| **D6** | **Weapon slot 0, the crossbow, can never be found or embezzled** — `(countyId & 3) + 1` is never 0. The behavioural half is B3. | — |
| **D7** | **The royal-castle branch is unreachable for two of the four lords**, because a zero gold threshold takes the type out of the ladder rather than making it free. The behavioural half is B25. | [V] scoped |
| **D8** | **The Winter grain quota's `fertility < -50` rung never runs**, because `< -20` is tested first: a ruined county gets the same one-field discount as a merely tired one. Reproduced verbatim, with the line annotated `/* dead code */`. | [V], from the branch order in all three functions that carry it |
| **D9** | **`Ai_SetRations`' Triple dairy rung can never change an answer**, because the store term counts the herd twice. Its sibling the Double rung *is* reachable and can only lower the answer — which is why that half is B27 and this half is dead code. | [V] on the arithmetic |
| **D10** | **AI steps above 14 dispatch nowhere**, so the extra steps B24 grants later realms run no handler. | — |
| **D11** | **The `-1` labour floor gates nothing.** Every writer but grain's and cattle's stores −1, and both readers compare `labour < wanted`, which −1 can never satisfy. | [D] |
| **D12** | **The 52nd row of the tax happiness table is unreachable** — `Tax_IncreaseCounty` (`0x0043AA83`) guards `taxRate < 0x32`. `0x004D63D8 + 52 × 4` is exactly where `g_healthDeltaTable` begins, which is what fixes the length. | [V] |
| **D13** | **The siege sortie branches write nothing, and are unreachable anyway.** `UnitOrder_SiegeDefMissile` (`0x0048E097`) discards `Enemy_NearestUnit`'s result and then calls `Order_HalfwayToUnit` on the unit's own index, so both separations are zero and it returns without writing; `SiegeDefFoot` has the same discarded call. Both read as a missing assignment — and both sit behind a 260 % strength gate that is almost never met. Reproduced deliberately, because *"a reimplementation that 'fixes' them would change observable behaviour."* | [D] |
| **D14** | **`UnitOrder_SiegeDefWallMissileA` (`0x0048E234`) does nothing but count `orders` up** — the unit holds its deployment wall slot for the whole battle. Not a stub: the original body is the think gate and the increment, and nothing else. | — |
| **D15** | **Battlefield step costs are inert on `.skr` maps.** `Path_BuildStepCost` (`0x00471DA6`) charges only cells flagged `0x20` or `0x40`, and `Battlefield_BuildFromSkr` sets neither — so skirmish pathfinding is a plain breadth-first search and the whole weighting mechanism is dead there. This is also why B5's stale counters are harmless on `.skr`. | [D] |
| **D16** | **High-ground protection is inert on `.skr` maps** — elevation is a missile-defence bonus only, nothing in melee reads it. | [V] |
| **D17** | **Two terrain-variant tables have entries behind their own catch-alls.** The hills table (`0x004D7550`) declares 11 entries whose tenth is a catch-all, so the eleventh is unreachable; the 49-entry water table (`0x004D7610`) has its last two behind its own. Both kept as found so the tables match the binary. | — |
| **D18** | **`Msg_Enqueue`'s sender tests are vestigial.** The guard resolves to `to == 0 \|\| to == g_localPlayer` down all three branches. | [V] |
| **D19** | **Two bytes of the diplomacy pair record are never initialised and never read** — `pair +0x06` / `+0x07`, padding. | [D] |
| **D20** | **Sheep and wool are goods with no code path in either direction.** Twelve goods have a merchant branch; goods 3 and 5 have none, buying or selling. Priced 0, produced by nobody, omitted from the game's own tutorial list. Carried anyway — *"do not prune content on a judgement that it looks dead."* | [V], closed from a third direction |
| **D21** | **`L2.eng` groups 62 and 63 are dead content** — *"Click on a food to swap its priority."*, *"Barrels swilled."*, *"A new field will be ready next season."* describe a richer ration and field panel than the one that shipped, and five of group 62's strings are literally `FREE`. No *literal* group id in 2,452 decompiled functions passes 62 or 63; a computed one is not excluded. | bounded claim |
| **D22** | **The "Group Information" panel (`L2.eng` group 47) has no caller** — five category names plus *"Morale"*, with no backing field found for morale. The panel may not ship enabled at all. | — |
| **D23** | **`TROOPS*.ENG`'s four non-Normal difficulty groups are dead data.** The engine applies its own ±8 % / ±16 % curve via `FUN_00404D6B` and only to troop types 0 … 6, so 402 of 3,080 populated rows in `TROOPS.ENG` are overwritten. Pinned by `l2-mods`' `the_shipped_difficulty_rows_are_dead_data_and_not_the_engines_curve`. | [V] |
| **D24** | **Four `Misc_cty` frames the job-icon table never names** — 5, 6, 11 and 12, which are exactly the four 2 × 2 stubs in that range. Nothing is left over. | [V] |
| **D25** | **`IID_IDirectPlay2A` and `IID_IDirectPlay` sit in `.rdata` and are referenced nowhere in `.text`.** Related to N9. | — |
| **D37** | **Screen `0x28` is a whole battlefield screen that nothing can enter.** It has a `Screen_FrameInput` arm (edge scroll, and a right release that goes to `0x29` and **clears the pause word** — the only site other than the pause button that does) and a `Screen_Draw` arm that paints the battlefield. Neither can run. `docs/battle.md` §15.1. Not built. | **[V]** by exhaustion rather than by failing to find: all 212 `mov byte ptr [g_screenId], imm8` sites in the binary were enumerated, they cover `0x00` … `0x45`, and `0x28` is absent while `0x29`, `0x2A` and `0x2B` are present. No decompiled function assigns it, and the five indirect writes can only restore a value `g_screenId` already held. |
| **D38** | **The battle's pause sound can never play.** `FUN_0043B9A1` toggles `DAT_0053F238` with a bitwise NOT — `Battle_Start` seeds it `0xFFFFFFFF`, so it alternates between `-1` and `0` — and then guards the sound on `if (DAT_0053F238 == 1)`. `s032_01.wav` and the flag `_DAT_005533F0` beside it are unreachable. | **[V]** on the two writes and the test. **[I]** that the author meant `== 0` and wanted a sound on *unpause*, which is the only reading in which the branch means anything. |
| **D26** | **Two one-past-the-end reads saved by adjacent padding.** `Score_RankRealms`' bubble sort reads one pair past the five-entry rank table, and the function zeroes the two dwords at `0x00565438` two statements earlier; and rows 8 and 9 of both campaign tables are zeroed padding, which is what makes the eighth win's read harmless. | [V] |
| **D27** | **The far campaign zoom shows 40 of 64 usable lattice columns and cannot scroll**, so 24 columns are unreachable in the original's view. | — |
| **D28** | **One of the fourteen AI turn steps is an empty function** — step 8. Listed here rather than in §2 because an empty step is not wrong, only unexplained. | [D] |
| **D29** | **The 50-men withdrawal-survival rule is unreachable under the autocalc**, which zeroes the loser's men. It is gated on `DAT_0056D5C8`, raised in exactly one place (`UnitOrder_SiegeAttKnight`). Note this is the *inner* test only: the **outer** `else` — a loser still linked as a besieger and still holding men keeps them and merely has its siege lifted — is reachable from a fought siege, and C31 stopped before it. | [V] (corrections C31 and **C38**) |
| **D32** | **The siege-preparation default of two towers is unreachable for every shipped lord.** `Siege_Prepare` tests the lord's doctrine byte against three constants, and the four shipped values are `8, 9, 7, 7` — so every lord takes a named arm and the `default:` never runs. | [V] — three of the four values are exactly the three constants tested and the fourth repeats one of them |
| **D30** | **Phase 5's wait predicate covers a unit nothing creates** — a player-owned peasant mob. The guard is reproduced anyway. | — |
| **D34** | **Three of every sixteen troop cries are unreachable, and the shipping build knew.** `FUN_00499CB1` round-robins four takes within an event class but forces take 0 for class 3, so cells 13, 14 and 15 of each unit's block at `0x004DB0D0` can never be selected. That is where the `_F1` names sit — one per troop type, `Peas_F1.wav` through `Knig_F1.wav`, seven names the binary carries and **not one of which is in the install**. Cells 13 and 14 name real files, which is why the block reads as complete. | [V] — the index arithmetic, plus the absence of all seven `_F1` files |
| **D35** | **Both sample banks reserve a slot for a file that has never existed.** Slot 1 of each is `null.wav`, which is in neither the Windows install nor the DOS one; the bank loader takes a count and the slot is how the original spells a hole in a fixed-size table. | [V] on both installs |
| **D31** | **81 % of the game's audio exists to satisfy a test whose answer is fixed.** `FUN_004AEF7E` opens `pumkin.wav` (320 MB across two byte-identical copies), measures it against 151,000,000 and closes it. `DAT_005C9A74` is set to 1 before the test, set to 1 again in both success branches, and never set to any other value anywhere; its three readers ask `(flag < 1) \|\| (2 < flag)`, which cannot be true. | [V] |
| **B77** | **The DirectDraw re-acquisition path cannot run, and it is the path that would have handled the crash already on file.** `g_displayLost` (`0x004E65D0`) has **four references in the whole decompiled corpus**: two readers and two writers, and both writers write **zero** — `App_OnPaused` and `App_OnResumed` alike. Nothing ever sets it. So `App_WndProc`'s `WM_ACTIVATEAPP` arm `if (g_displayLost && g_windowActive)` is unreachable, and with it `Display_RestoreSurfaces`, the log line *"ERR:Re-initializing direct draw.  "* and `Gfx_Restart`; while `App_WinMain`'s idle test `if (!g_displayLost && g_windowActive)` is decided by its second clause alone. `docs/symbols.md` already recorded a crash with `g_ddPrimary` NULL *"when the window was deactivated during startup under DxWnd"* — this is the recovery that was meant to catch it. Not reproduced: we do not lose surfaces. | **[V]** by exhaustion over all 2,452 decompiled functions — the flag's four references are listed above and there is no fifth. **[I]** that a missing `= 1` is the whole of it; no other candidate flag was looked for. |
| **B78** | **The dirty rectangle is accumulated and never read, and its bottom-edge clamp is wrong.** `Gfx_MarkDirty` (`0x00452306`) grows a box in `g_dirtyLeft`/`Top`/`Right`/`Bottom`; those four addresses have **five references each in the whole corpus** and every one is inside `Gfx_MarkDirty` or `Gfx_Present`'s reset. Nothing turns the box into a blit rectangle — `Gfx_Present` always blits the whole frame — so fourteen wrappers spend their time maintaining state with no consumer. The vestigiality hides a plain copy-paste defect in the same function: the bottom clamp reads `if (0x27F < g_dirtyBottom) g_dirtyBottom = 0x1DF;` — the **right edge's** bound with the **bottom edge's** value, so a bottom between 480 and 639 passes through unclamped. Harmless only because nobody reads it. Not reproduced: our renderer has no dirty box. | **[V]** on both — the reference counts are exhaustive and the mismatched constants are in one statement. |
| **D36** | **`L2.eng` 163/4 — *"This lesser castle will reduce the tax collected in the county."* — describes something no code path can produce.** `castleType` has two writers in the whole binary: `g_startCastle` at new-game, and `Castle_Order`, whose OK guard sends message `0x122` and returns whenever the type picked is below the one standing. Nothing lowers it, and a siege lowers `+0x1F9` instead. **Bounded claim:** what is verified is that the two writers cannot lower it. Which of group 163's indices message `0xA3` actually renders was not traced, so "unreachable" is a reading of the writers and not of the renderer. | bounded claim |

---

# 6. The mechanism — what switching these off would actually take

> **Built, as of `docs/decisions.md` C62.** This section was written as material for a
> decision and the decision has been taken; it is kept as the *argument*, because the
> argument is what a later reader needs in order to change the answer. What actually
> shipped, and where it differs from the recommendation below:
>
> | | recommended here | built |
> |---|---|---|
> | home | `Options` | `Options::quirks` for a **behavioural** quirk; `Assets` for a **presentation** one — §6.3a, which §6.3 did not anticipate |
> | shape | *"a struct of named `bool`s, or a bitfield"* | a `u64` bitfield, `l2_net::Quirks`, with the sense **inverted** so that faithful is zero |
> | version bumps | *"pay the bump once"* | paid once, and the inversion is what makes it once rather than once per bug: a new quirk sets a bit that was already written as zero |
> | default | faithful | faithful (§6.5) |
> | scope | *"roughly a dozen worth exposing"* | fourteen wired |
>
> **The switch list is generated from this document**, not written beside it:
> `crates/l2-testkit/tests/quirks_catalogue.rs` reads §2 and both switch lists as text and
> fails if they disagree — including if a quirk is filed in the wrong home, which §6.3a's
> price asymmetry makes the likely drift. §2 is now load-bearing: **adding an entry here
> turns the suite red until somebody says what its switch is.**

**The argument, as it was written.** The owner said *"make a call later"*; this was the
material for that call.

## 6.1 What is already switchable through the ruleset: one entry

**D1, the score gold bracket, and only D1.** The stock ruleset carries the three thresholds as
data with the *shipped* behaviour encoded in the points
(`crates/l2-mods/rulesets/core/rules/kingdom.toml`):

```toml
[[kingdom.score.gold_bracket]]
at_least = 10001
points   = 50
[[kingdom.score.gold_bracket]]
at_least = 5001
points   = 50
[[kingdom.score.gold_bracket]]
at_least = 2001
points   = 50
[[kingdom.score.gold_bracket]]
at_least = 0
points   = 0
```

`Tables::score_gold_bracket` walks it **richest-first**, so all-50 reproduces the original's
smallest-first behaviour exactly, and a ruleset that wants the ladder the designers wrote
changes **two numbers** — `points = 100` and `points = 200` on the top two rows.

**This is the pattern worth generalising, and it is worth naming precisely.** The bug was not
made switchable by adding a flag. It was made switchable by choosing a data *shape* that can
express both answers — a richest-first ladder — and then filling it with numbers that
reproduce the defect. The switch costs nothing at run time, nothing in the save format, and
nothing in the netcode.

**It generalises only where the bug is a number.** Where the bug is control flow — a branch
order, a wrong variable, a missing `break`, a byte width — no value of any table changes the
answer.

## 6.2 What the ruleset can and cannot express today

| | |
|---|---|
| **Namespaces** | `unit.<id>.*`, `kingdom.*` (21 sub-tables), and `troop.*` / `difficulty.*` / `battle.*`, which load but are not yet wired into the simulation. The registry is the three loader functions, which name every path as a string literal |
| **Vocabulary** | integers, fixed-length integer arrays, and arrays-of-tables with a **fixed row count**. Nothing else. Floats parse and are reported unusable |
| **Ranges** | every scalar is range-checked and **refused, not clamped** |
| **Ladders** | row counts pinned by `expect_rows`. The one exception is the AI tax ladders, which accept 1 … 8 rows and repeat the last to fill 8 — a convenience for omission, not extensibility |
| **Ordering** | not expressible. Named rows carry an `index` checked against their slot; ladder walk order is a Rust `for` loop |
| **Booleans** | **effectively none.** One key exists in the whole vocabulary (`unit.<id>.siege_engine`) and it is near-inert. `kingdom.toml` and `units.toml` contain no boolean at all |
| **Options** | `l2-mods` has no knowledge of `l2_kingdom::Options`. There is no `kingdom.options.*` path. Options arrive from a scenario or save |

Against that vocabulary:

| entry | data today? | why |
|---|---|---|
| **D1** gold bracket | **yes** (§6.1) | the ladder shape can express both answers |
| **B1** harvest weather | no | `harvest_factor` is a Rust `match`, and the bug is *which variable* the band multiplies |
| **B2** event deck | no | `EVENT_DECK` and `EVENT_DECK_SLOTS` are Rust `const`s; the only event keys are `population_cap_pct` and `first_year` |
| **B4** empire tax `i8` | no | the bug is a **type**, not a value |
| **B27 / D8 / D9** AI ladders | no | literal `if`/`else if` chains in `ai_farm.rs`; in B27's case the *order* is the bug |
| **B5, B36, B37** pathfinder | no | `l2-sim` has no rules seam at all beyond `TroopTable` |
| everything else | no | control flow or Rust literals |

**One entry of about ninety is switchable by data today, and it is switchable because somebody
chose the shape deliberately.** Everything else needs a behavioural flag.

## 6.3 Where a behavioural flag would have to live

Two candidate homes, with very different costs.

**`Tables` (the ruleset) — the expensive one.** `ruleset_fingerprint(tables)` is hashed into
the save header and `decode` **refuses** any save whose supplied `Tables` hashes differently
(`LoadError::RulesetMismatch`). Adding a field therefore invalidates every existing save,
needs an entry in `impl Encode for Tables` and in `kingdom::render_toml`, and needs the
shipped `.toml` regenerated. It also frames a quirk as a *rule*, which it is not.

**`Options` (the game settings) — the right one.** `Options` already holds `difficulty`,
`advanced_farming`, `armies_eat` and `fight_humans_only_byte`; it is already in the save body
and in the lockstep state; and it is already **exactly the shape the original uses for this**
(§6.4). Cost: one `save::VERSION` bump.

**Recommendation: `Options`, and add the whole set at once.** Each new field is another
version bump, so a quirk set introduced one flag at a time costs one save-format version per
bug. Introduce a single `Quirks` value — a struct of named `bool`s, or a bitfield with a
documented assignment and room to grow — and pay the bump once.

Three constraints, none optional:

1. **It must reach the simulation as a value**, never as a global and never as a file read
   inside `l2-sim` / `l2-kingdom` (`netcode.md` D-12).
2. **It must be in the lobby handshake and stamped into replays.** A quirk that changes the
   simulation and is not agreed before the first tick is a desync, and a replay recorded under
   one setting does not reproduce under the other. `session_digest` already carries the mod set
   and load order; the quirk set has to travel the same way.
3. **`decisions.md` C12 applies**: no quirk may be described as taking effect unless a
   simulation test reads a different answer with it flipped. Much of §2 currently has a test
   that asserts the *buggy* answer; each of those becomes a pair.

`l2-mods` could reasonably own the **defaults** for the quirk set — a `kingdom.quirks.*`
boolean table feeding `Options::default()`, which would also be the first real use of a
boolean in the ruleset. The authoritative value still has to travel in the save and the
handshake.

## 6.3a Presentation quirks are a second set, and they do not go on `Options`

**§6.3's argument is about quirks that change a rule.** All three of its constraints follow
from that — reach the simulation as a value, be agreed in the lobby handshake, be stamped into
replays — and all three are the reason a behavioural quirk costs a `save::VERSION` bump.

**None of them applies to a defect you can only see.** B64 is a text shadow colour. It cannot
change a turn, so it must not be in the state that decides one: `netcode.md` D-12 says display
state does not reach the simulation, which makes `Options` the *wrong* home for it rather than
the expensive one. Two players running with different values compute identical turns, and a
save that recorded the setting would be recording the reader's preferences in the world.

So there are two sets and they live in different places:

| | where | cost of a new flag | example |
|---|---|---|---|
| **behavioural** | `Options`, in the save body and the lockstep state | one `save::VERSION` bump, a handshake field, a replay stamp | B63's screen stack |
| **presentation** | `l2_game::game::Quirks`, on `Assets` | one `bool` | B64's emboss |

The test that tells them apart is the one §6.3 already implies: **if flipping it can change a
number in a saved game, it is behavioural.** If it can only change which pixels are painted
from the same numbers, it is presentation. A flag that is hard to classify is a flag that is
doing two things.

`l2-mods` can reasonably own the defaults for either set. The authoritative value for a
behavioural quirk still has to travel in the save and the handshake; a presentation quirk has
nowhere to travel to.

## 6.4 A coherent option group — and the game's own precedent for one

**The original ships four behaviour switches on one panel, and a player has confirmed using
three of them.**

> **Corrected: four, not three.** This paragraph said *"three behaviour switches"* and named
> `g_optAdvancedFarming`, `g_optArmiesEat` and `g_optExploration` as *"`L2.eng` group 50
> indices 1 … 3"*. **Group 50 has five strings** — a heading and four rows — and
> `g_advancedOptWidgets` (`0x004DDC10`) holds **four** 24-byte widget records whose callbacks
> are `Opt_ToggleAdvancedFarming`, `Opt_ToggleArmyForaging`, `Opt_ToggleExploration` and
> `Opt_ToggleFightHumansOnly`. Index 4 is *"Fight humans only?"*, it is on the same panel, and
> it changes a rule: `FUN_004A6A30` auto-resolves a battle the local player is not in when
> `g_optFightHumansOnly` (`0x0053F284`) is 0. **[V]** on both counts — the string count out of
> `L2.eng` and the record count out of `.data`. `crates/l2-game/src/screens/options.rs` draws
> all four. Nothing turned on the number; it was simply wrong, and B55a two sections up had
> been discussing that fourth option's *save* behaviour for weeks without either half noticing
> the other.

They are `g_optAdvancedFarming`, `g_optArmiesEat`, `g_optExploration` and
`g_optFightHumansOnly` — `L2.eng` group 50 indices 1 … 4, toggled by `Opt_ToggleExploration`
(`0x00434693`) and its siblings. They change *rules*, not presentation:

* **Advanced Farming** turns on weather, fertility and crop rotation — S1 above, and *"fallow
  fields"* in the player's own words;
* **Foraging** (`g_optArmiesEat`) decides whether armies eat county stores at all, and
  `Readme.txt` states the consequence in the authors' words: *"Armies in castles forage for
  themselves, and therefore do not eat from county stores. When foraging is on, building large
  castles and keeping your army inside is an effective way of avoiding starvation problems."*
* **Exploration** is fog of war, and the game specifies it itself in `L2.eng` group 218 index
  3: *"When Exploration is turned on, the world outside your county is blacked out. It is
  gradually revealed as your armies move through and conquer new counties."*

So **an options page that changes simulation rules is idiomatic to this game**, not an
imposition on it. That is the strongest argument for the group, and it is evidence rather than
taste. (Exploration is currently imported from the save, drawn on the setup screen, and
implemented nowhere — a 🕳 gap in [`mechanics.md`](mechanics.md), not a bug here.)

**The shape.** One group, on the advanced-options page beside the original's own three, with a
single control that sets all of them and per-entry overrides underneath:

* **Faithful** *(default)* — every quirk on. What the original does.
* **Fixed** — every *behavioural* quirk off.
* per-entry, for the ones that visibly change play.

**Not everything in §2 belongs in the group**, and saying so now is most of this section's
value:

* **The pathfinder (B5, B36, B37, B38) should not be exposed per-entry.** B5 makes a search's
  result depend on what ran before it, so a per-search toggle is meaningless; and any of them
  invalidates every recorded battle replay. If exposed at all, one all-or-nothing switch,
  clearly marked as changing every path in every battle.
* **B56, the sync digest, is catalogue-only, and the project rule already answers it.**
  `CLAUDE.md`: *"Networking is the one place the original is not the authority… the original's
  multiplayer sync is the reason for the rewrite — it is the defect being replaced, not a
  model."* So the answer is *don't reproduce this one*, and it is not even a judgement call:
  reproducing a defect in the mechanism that **detects** divergence buys nothing a player can
  see and costs the ability to find real desyncs. Note the same rule quietly settles N9, the
  per-run DirectPlay GUID, for the same reason.
* **The invisible ones stay off the menu** — B23's 1000 sentinel has no observable effect at
  all, B21, B22, B49 and B55 likewise. They belong in the model, not in a settings page.

That leaves a group of roughly a dozen worth exposing, all of them things a player could
notice: **B1** harvest weather, **B2** the event deck's parity, **B3** the weapon slot,
**B4** the empire tax byte, **B10**/**B11** ale, **B12** the castle switch, **B13** the levy
slider, **B14** the free wage bill, **B27** the AI ration ladder, **B25**/**B26** the AI's
castle ladder, **B42**/**B43** the merchant and mercenary walks, **B51**–**B54** the victory
oddities.

## 6.5 The default: faithful, and here is the argument

**Recommendation: faithful by default.** The instinct is right, and it is worth more than an
instinct.

1. **It is the project's premise, and the premise is load-bearing rather than sentimental.**
   `CLAUDE.md`: *"Two of our own implementations agreeing proves only that we ported our own
   misunderstanding faithfully."* The original is the oracle. If the default diverges, every
   differential test against a running `Lords2.exe` has to know which mode it is in, and the
   cheapest oracle we have gets more expensive to use. Fidelity-by-default keeps the default
   configuration the one we can *check*.

2. **The bugs are load-bearing on balance, and nobody has measured by how much.** B4 can flip
   the sign of a large empire's tax happiness. B1 multiplies a harvest by 1.5 regardless of
   labour. B2 halves the number of counties that see events. B14 makes an insolvent realm's
   wage bill free. Fixing all of these at once produces a game nobody has ever balanced, and
   calling that the default means shipping an untested balance as the reference.

3. **Determinism makes the default a wire format, not a preference.** Saves, replays and
   lockstep peers all have to agree on the quirk set, so whatever the default is becomes the
   value the vast majority of saves and replays are recorded under. Choosing the
   *reproducible* value is worth more than choosing the *nicer* one, and it can be changed
   later at the cost of one version bump — whereas a corpus of saves recorded under a "fixed"
   default is permanent.

4. **The counter-argument, stated honestly.** Some of these are not interesting defects, just
   annoying: B13's lying levy slider and B12's inverted castle switch are interface faults
   nobody has ever enjoyed, and a new player meeting them will file bugs against us. The honest
   answer is that the group exists precisely so they can be turned off in one click, and that
   the *documented* default is worth more than the *pleasant* one. If any entry is ever
   promoted to fixed-by-default it should be one of those two, individually, and with a
   correction number.

**And three things to settle regardless of the decision.** §3's N1, N2 and N3 are already
inconsistent with faithful-by-default: we clamp `dryness` where the original wraps, we clamp
`army_happiness_cost` where the original hands out a free army, and `local_modifier` is a stub
that will silently "fix" a mistyped `case 3` the moment anybody traces it. Each has its
reasoning in a code comment and nowhere else. Under faithful-by-default each should either be
reproduced, or recorded as a deliberate exception with a number. Leaving them undeclared is the
worst of the three options.

---

# 7. What is deliberately not here

**Our own bugs.** `decisions.md`'s numbered corrections are mistakes *we* made and fixed —
misread formats, wrong summaries, claims retracted. They are not content, they are not
switchable, and mixing them in would destroy the only distinction this document is for. C31
and C33 appear above as **citations for evidence**, not as entries.

**Missing features.** A rule we have not implemented is a 🕳 in
[`mechanics.md`](mechanics.md), not a bug here. Exploration is the current example: the game
reads the setting, shows the player the switch, and our engine ignores it.

**Third-party bugs.** The `smacker` crate's failure on `Pill_brn.smk` is a bug in a
dependency, not in `Lords2.exe`.

**Divergences we chose.** Where we knowingly differ — the save-load scroll clamp
(*"reproducing an off-by-fifteen in a list of our own files would be superstition, not
fidelity"*), our own checksum, `Pcg32` in place of the two LFSRs — the reason belongs at the
code and in `decisions.md`. §3's N1, N2 and N3 have neither and should.

### B81 — A supply shipment with nowhere to stand is lost, in silence

`Transport_Spawn` (`0x004292AF`) looks for a free road tile near the source county's anchor
and then any free open tile. If **neither** answers, it spawns nothing and — because the
deduction is inside the same `if` — **deducts nothing**. There is no message, no refusal and
no sound: `FUN_0043B04C` has already set `g_screenId = 0`, so the send-supplies screen closes
on the way in and the player is looking at the map.

The visible symptom is nothing at all. The grain and cattle stay in the source county, so the
shipment simply did not happen, and the only way to tell is to notice that no cart appeared.

**Reproduced**, as `l2_kingdom::supply::Sent::Nowhere`, and returned as a value rather than
raised as an error: a shipment that evaporates *with a warning* is not the shipment the
original loses. The three-tile search radius is `County_FindFree*Tile`'s own and is the same
one `docs/decisions.md` C47 corrected for raising an army, so a county crowded enough to
refuse an army will refuse a cart for the same reason and just as quietly.

### B82 — `L2.eng` 37/1, *"Before"*, is drawn by nothing

The Battle Master ratings screen has three rows per player. Two are labelled — 37/2
*"Killed"* at the second row's y and 37/3 *"Kills"* at the third's — and the **first row has
no label**, although the string for it sits in the file directly above them.

Every group-37 access in the binary was enumerated: eight, all in
`Screen_BattleMasterRatings` and `Screen_BattleMasterRank`, and **none uses index 1**. So
unless `score1.pl8` paints the word into the background image, the top row of both blocks is
a row of numbers with nothing saying what they are.

Not reproduced *as a fix*: we draw what the painter draws, which is nothing, and
`ratings::BEFORE` names the string so the next reader does not go hunting for the draw call.
`shell.rs`'s eng test asserts the string exists, which is the other half of the claim.

### B83 — `L2.eng` 70/1, *"Arms"*, is drawn by nothing either

The same shape on the court screen. Group 70 is *"Gold, Arms, Iron, Stone, Wood"* and the
painter draws Gold, Iron, Stone and Wood as labelled rows. The six weapon stocks below them
are drawn as **icons with numbers under them and no heading**, and index 1 is never passed to
anything. `court::ARMS` names it.

### BNEW-stale-garrison-widget — the tile panel's one widget survives the tile that gave it a count

`Screen_FrameInput`'s `0x04` arm runs `FUN_00438A91`, which is exactly one statement:

```c
Widget_Test(8, 0x20, &g_tilePanelWidgets, DAT_00568474);
```

The **count is a runtime global**, and the only thing in the binary that writes it is
`TileInfo_DrawCastle` (`0x0041DA2F`), on its first line:

```c
DAT_00568474 = (uint)(g_counties[g_pickedTileCounty].garrisonUnit != 0);
```

`TileInfo_DrawCastle` runs only for a **castle tile**. Every other tile takes a different
branch of `TileInfo_Draw` and writes nothing, so the count keeps whatever the last castle
panel left in it — and the panel is repainted from `Screen_Draw` every frame, so the value
is as durable as the session.

So: right-click a castle with a garrison in it (the count becomes 1), close the panel,
right-click a *field*, and the *"View these troops?"* widget is still hit-tested at
(344, 414) with nothing drawn there. `FUN_00438ACC` then sets
`g_pickedTileUnit = g_counties[g_pickedTileCounty].garrisonUnit` for **the field's** county
and repaints — so an invisible 24 × 24 box on the field panel flips it to that county's
garrison, or to unit 0 if it has none.

**`[I]`, and the inference is narrow.** The two writes and the one read were enumerated and
there is no third; what has *not* been established is whether `Widget_Test` refuses a record
`Widget_Draw` has not touched this frame — the record carries a state byte at `+0x0C` and a
press timer at `+0x0D`, and neither's lifecycle was read. That is the one thing that would
make this unreachable, and it is a twenty-minute read for whoever needs the answer.

**Not reproduced.** Ours offers the widget only when the panel's own tile is a castle whose
county holds a garrison, because our count is a function of the target rather than a global
left over from a previous paint. Reproducing it would mean modelling the leftover, which is
a global we do not have and a switch nobody has asked for.
