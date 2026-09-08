# The battle AI

What an *unordered* battle unit does. [`battle.md`](battle.md) §8.1 established that
`Battle_UpdateAllUnits` dispatches an order handler per unit unless the unit's owner is
human, and listed those handlers as the largest undecompiled hole in the tactical layer.
This document is that hole filled in: the dispatch table, the 17 handlers behind it, how a
unit picks something to attack, and what `Path_Search` actually costs.

Same status legend as `battle.md`, and applied with the same strictness:

* **[V] verified** — read out of the binary *and* cross-checked against a second,
  independent source: the game's own debug labels, an `L2.eng` string, an exact invariant
  over shipped data, or a second unrelated structure in the binary that has to agree.
* **[D] decompiler-only** — a straightforward reading of decompiled C with no second
  source. Almost certainly right about *what the code does*; the *name* may be wrong.
* **[I] inferred** — consistent with everything measured, not proven.

**Most of this document is [D].** There is very little outside the binary to check a
tactical AI against: the printed manual describes units, not the opponent, and `L2.eng`
has no order names at all (§1.4). Where a [V] appears below, the second source is named.

> `docs/symbols.json` uses a *different* two-value scale, defined in its own header:
> `verified` there means "read out of the binary", which is this document's [D] or better.
> A `verified` symbol is not a [V] claim.

`docs/decisions.md` C3 is the failure mode: three functions matched three storage modes,
the numbers lined up, and the story was wrong. Game AI invites it especially, because a
wrong story about *intent* is untestable and reads convincingly. §9 lists what was not
established, including several places where a tidy story was available and refused.

Addresses are the GOG Windows build, `ImageBase 0x400000`, no ASLR.

---

## 0. The headline

**The battle AI is one number, a per-unit script counter, and a memory of who hit you
last.**

```
g_aiStrengthAdvantage  = (weighted AI men * 100 / weighted human men) - 100  ± jitter
unit.orders  (+0x1A)   = how many times this unit has thought
unit.+0x0A / +0x0C     = the enemy unit that last hit me, and 50 frames of memory of it
```

Every one of the 17 order handlers is a variation on one skeleton:

```c
unit.+0x1C += 1;                                  /* think timer */
if (unit.+0x1C > 199 && unit.+0x0D == 0) {        /* +0x0D: some figure is in melee */
    unit.+0x1C = 0;
    if (g_aiStrengthAdvantage > threshold)  ... aggressive branch ...
    else                                    ... cautious branch ...
    unit.orders += 1;
}
```

Counts re-derived from the seventeen decompiled bodies rather than carried across: the
think timer is **200 frames in 15 of them and 100 in 2** (`UnitOrder_SiegeDefFoot`,
`UnitOrder_SiegeDefMelee`); the `+0x0D` melee gate is present in **13** and absent in
**4** (`SiegeAttCatapult`, `SiegeAttTower`, `SiegeAttRam`, `SiegeDefFoot`); and
`g_aiStrengthAdvantage` is read by **8** (the three field handlers, `SiegeAttFoot`, and all
four real siege defenders).

**[D]** Three consequences fall straight out of that skeleton and are worth stating before
any of the detail:

* **A unit makes a decision once every 200 frames.** Nothing else in the battle is that
  slow; the figure state machine, the mover and the missiles all run every frame.
* **A unit that is in melee stops thinking.** `+0x0D` is set by
  `BattleUnits_RebuildFromFigures` whenever any figure of the unit is in state 4, and 13 of
  the 17 handlers refuse to run while it is set. Once the AI's line makes contact, most of
  the AI stops manoeuvring.
* **The AI's aggression is global, not per unit and not per side.** `g_aiStrengthAdvantage`
  is a single word recomputed every 101 frames from the *whole battlefield*, and every
  handler that has a mood at all reads that same word.

---

## 1. The dispatch

### 1.1 The three tables, re-derived

`Battle_UpdateAllUnits` (`0x00489401`) picks a table by battle kind and side, then indexes
it by unit category (`unit +0x08`) with a hard bound:

| table | address | bound | when |
|---|---|---|---|
| `g_unitOrderTableField` | `0x004D91B8` | `category < 5` | `g_battleIsSiege == 0` |
| `g_unitOrderTableSiegeAtt` | `0x004D91D0` | `category < 9` | siege, unit side ≠ 0 |
| `g_unitOrderTableSiegeDef` | `0x004D91F8` | `category < 11` | siege, unit side == 0 |

**[D]** Each table is followed by a zero dword in the data, which is what fixes the entry
counts at 5, 9 and 11 — and the counts match the bounds the code tests. 5 + 9 + 11 = **25
slots**.

**Those 25 slots hold 18 distinct functions, one of which is empty.** `UnitOrder_None`
(`0x0048A9C7`) is a bare `return` and fills **seven** slots. So there are **17 real
handlers**, not 25. `battle.md` §11 says "twenty-five functions", counting slots; and its
address range `0x0048A9C7 … 0x0048ECA9` overshoots — the last real handler is at
`0x0048E8B8`, and `0x0048ECA9` is `BattleUnit_FewOnRampart`, a helper.

### 1.2 Category comes from the troop type

**[D]** `BattleUnit_Create` (`0x00480662`) assigns the category in an eleven-way ladder,
read here from the disassembly at `0x00480743`–`0x00480934`:

| troop type | 0 peasant | 1 crossbow | 2 mace | 3 sword | 4 pike | 5 archer | 6 knight | 7 catapult | 8 tower | 9 ram | 10 oil |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **category** | 2 | 1 | 3 | 3 | 2 | 1 | 4 | 5 | 6 | 7 | 8 |

and then, **only** in a siege and **only** for side 0 (the defender), the first missile
unit raised becomes **category 9** and the next one **category 10**, latched by two globals
so it happens at most once each.

### 1.3 The whole table, with names

| cat | field (`0x004D91B8`) | siege attacker (`0x004D91D0`) | siege defender (`0x004D91F8`) |
|---|---|---|---|
| 0 | — stub | — stub | — stub |
| 1 missile | `UnitOrder_FieldMissile` `0x0048A9D2` | `UnitOrder_SiegeAttMissile` `0x0048D16E` | `UnitOrder_SiegeDefMissile` `0x0048E097` |
| 2 peasant/pike | `UnitOrder_FieldFoot` `0x0048ACD2` | `UnitOrder_SiegeAttFoot` `0x0048D412` | `UnitOrder_SiegeDefFoot` `0x0048E39C` |
| 3 mace/sword | `UnitOrder_FieldMelee` `0x0048B02B` | `UnitOrder_SiegeAttMelee` `0x0048D6FC` | `UnitOrder_SiegeDefMelee` `0x0048E4CC` |
| 4 knight | `UnitOrder_FieldMelee` (same fn) | `UnitOrder_SiegeAttKnight` `0x0048D9CE` | `UnitOrder_SiegeDefKnight` `0x0048E774` |
| 5 catapult | *out of range* | `UnitOrder_SiegeAttCatapult` `0x0048DB84` | — stub |
| 6 tower | *out of range* | `UnitOrder_SiegeAttTower` `0x0048DDC7` | — stub |
| 7 ram | *out of range* | `UnitOrder_SiegeAttRam` `0x0048DFBB` | — stub |
| 8 oil | *out of range* | — stub | `UnitOrder_SiegeDefOil` `0x0048E8B8` |
| 9 | — | — | `UnitOrder_SiegeDefWallMissileA` `0x0048E234` |
| 10 | — | — | `UnitOrder_SiegeDefWallMissileB` `0x0048E2C8` |

Two things in that grid are load-bearing.

**[V] The siege-defender stubs at categories 5, 6 and 7 line up exactly with
`g_raiseOrderSiege`.** `battle.md` §5.2 establishes that the castle-defender raise order
holds eight troop types with **no catapult, siege tower or ram**. Those are precisely the
three categories whose defender slots are the empty handler. Two unrelated structures — a
raise order at `0x004D98A0` and a dispatch table at `0x004D91F8` — agree about which troops
a garrison never has. The same holds on the other side: the attacker's category-8 (oil)
slot is a stub, and oil is a defender's weapon.

**[D] In a field battle, categories 5 to 8 have no handler at all.** The field table has
five entries and the code tests `category < 5`, so an AI catapult in an open-field battle
is never given an order by this machinery. Its figures still shoot, because that happens in
the figure state machine; but the unit never repositions. Not observed in a running game.

### 1.4 `orders` is a script counter, not an order

`battle.md` §1 lists unit `+0x1A` as "order state within the category handler", from the
debug panel's label `orders`. It is more specific than that, and the distinction matters
for anyone reimplementing it.

**[V]** A cross-reference sweep over the whole unit record (`0x00566520`–`0x00566554`)
finds `+0x1A` touched by exactly two kinds of code: the 17 order handlers, and
`BattleDebug_Panel`, which prints it. `BattleUnit_Order` — the entry point for a player's
click, per `battle.md` §8.2 — never writes it. The second source is `L2.eng`: the game has
**no order-name strings** (§1.4), and its own help text describes battle control as
click-and-drag plus click-on-enemy. `orders` cannot be a player-chosen order because there
is nothing to choose.

**[D]** What it is: a monotonically increasing count of *thinks*, incremented once at the
bottom of each handler, and read as a program counter — `orders < 10`, `orders < 0x12`,
`orders % 5 == 0`, `orders & 7`. Two handlers also *jump* it: `UnitOrder_SiegeAttFoot` and
`UnitOrder_SiegeAttMelee` set `orders = 100` outright when the castle layout flag is set,
skipping the rest of the approach script. Two others (`SiegeAttCatapult`, `SiegeAttTower`)
`return` before the increment on their wall-found path, so a unit that is doing its job
stops advancing its script.

It is never reset, and it is an `i16` bumped once per 200 frames, so overflow is not
reachable in a battle.

---

## 2. The field handlers

Three functions, and they are close variants of one another. This is the whole open-field
tactical AI.

### 2.1 The shared shape

```c
lastAttacker = unit.+0x0A;                      /* unit index, from §3.2 */
if (g_aiStrengthAdvantage > g_aiAggressionThreshold /* 5 */) {
    /* ---- aggressive ---- */
    if (unit.+0x0C != 0 && units[X].owner != 0)  /* X = lastAttacker for the two melee
                                                    handlers, the *nearest* unit for the
                                                    missile handler                     */
        respond to X;
    else
        run the opening script keyed on `orders`;
} else {
    /* ---- cautious ---- */
    if (unit.+0x0C != 0 && units[lastAttacker].owner != 0)
        react to the attacker, possibly raising g_aiCommitCounter;
    else
        g_aiCommitCounter -= 1;
    ... then a second, independent decision ...
}
unit.orders += 1;
```

The `units[X].owner != 0` half of that test is a liveness check: `owner == 0` marks a free
unit slot (`battle.md` §1), so a remembered attacker that has been wiped out silently stops
counting.

**[D]** `g_aiAggressionThreshold` is `0x0057C8B4`, and it is set to **5** by
`Rules_InitConstants` (`0x004983B7`), in the same run of `MOV dword ptr [...], imm` that
sets the other tuning constants. It is read by these three handlers and by nothing else.
So: **the field AI attacks when it believes it is more than 5 % stronger.**

### 2.2 `UnitOrder_FieldMissile` — category 1, archers and crossbowmen

| condition | action |
|---|---|
| **aggressive**, hit within the last 50 rebuild passes | `Order_ShootAtUnit(nearest)` — every figure takes a target inside the nearest enemy unit within 40 cells and enters state 17 |
| aggressive, `orders < 10` | nothing |
| aggressive, `orders < 18` | `Order_HalfwayToUnit(nearest)` |
| aggressive, `orders < 21` | `Order_HoldPosition` |
| aggressive, otherwise | `Order_HalfwayToUnit(nearest)` |
| **cautious**, `g_aiCommitCounter != 0` | `Order_HalfwayToUnit(nearest)` |
| cautious, hit recently | `Order_ShootAtUnit(lastAttacker)` |
| cautious, a rally request is pending | `Order_StepTowardRallyPoint` |
| cautious, `orders % 4 == 0` | `Order_ToRallyWaypoint(0)` |
| cautious *and additionally*, hit more than 10 times, not firing, not already withdrawing | `Order_StepAwayFromUnit(lastAttacker)` |

The last row is a second, independent decision taken after the first — and it exists only
on the cautious branch. An AI archer unit that thinks it is winning never backs away.

`nearest` is `Enemy_NearestUnit(unit, 80, 1)` — **the whole 80-cell field**. A missile unit
looks everywhere; §2.3 shows melee units look 8 or 9 cells.

**[D] `Order_HalfwayToUnit` is why AI archers advance in stages and then stop.** It halves
the separation on each axis, does nothing at all unless one axis is separated by eight
cells or more, and then moves only the axes separated by six or more. Combined with
`Order_StopShortOfTarget` inside `BattleUnit_Order` (`battle.md` §8.2, which pulls a
missile unit's destination back to `range/8 − 3` cells), an AI archer unit converges on a
standoff distance rather than closing.

### 2.3 `UnitOrder_FieldFoot` and `UnitOrder_FieldMelee` — categories 2, 3 and 4

Categories 3 and 4 share one function; the two functions differ only in constants:

| | `FieldFoot` (cat 2) | `FieldMelee` (cat 3, 4) |
|---|---|---|
| march phase | `orders < 10` | `orders < 13` |
| charge radius | 8 cells | 9 cells |
| rally cadence | `orders % 5 == 0` | `orders % 8 == 0` |
| withdrawal limit | first withdrawal only | fewer than two withdrawals |

**Aggressive branch.** For the first ten (thirteen) thinks the unit marches to a slot of
the *enemy's* deployment marker via `BattleUnit_OrderToEnemyEnd`; after that it calls
`Order_ChargeNearest`, which drops the whole unit into free pursuit (§3.3). If it has been
hit recently it instead walks straight onto its attacker with `Order_OntoUnit`.

**[D] The march slot is chosen with the kingdom's season byte.** Literally:

```c
if ((g_season & 1) == 0) BattleUnit_OrderToEnemyEnd((unit & 1) + g_battleApproachLane & 3);
else                     BattleUnit_OrderToEnemyEnd(g_battleApproachLane);
```

In even seasons alternate units take alternate slots; in odd seasons they all take the
same one. `g_season` is the strategic-layer season, not anything battle-local. This looks
like a free coin-flip that happened to be lying around, but nothing establishes intent.

**Cautious branch**, and this is where the only inter-unit coordination in the game lives:

```c
if (this unit has a live attacker) {
    g_aiEngagementCount += 1;
    if (g_aiEngagementCount > budget[battlefield])      g_aiCommitCounter += 3;
    else if (g_aiMenMissile < g_aiMenTotal / 8)         g_aiCommitCounter += 20;
    else {
        g_aiRallyRequest = 1;                       /* "archers, onto this man" */
        g_aiRallyX = attacker.x;  g_aiRallyY = attacker.y;
        Order_StepAwayFromUnit(attacker);           /* back off two cells */
    }
} else if (g_aiCommitCounter) g_aiCommitCounter -= 1;

nearest = Enemy_NearestUnit(unit, 8 or 9, 3);
if (g_aiCommitCounter != 0)  Order_ChargeNearest(unit);
else if (nearest != 0)       Order_ChargeNearest(unit);
else if (orders % 5 (or 8) == 0 && never withdrawn)  Order_ToRallyWaypoint(2);
```

**[D]** Three behaviours worth naming:

* **Disengage-and-mark.** A melee unit that is losing men backs two cells away from its
  attacker *and* publishes that attacker's position; the field missile handler reads
  `g_aiRallyRequest` and shifts two cells toward it. That is a fire-support call, and it is
  the only message any unit sends to any other.
* **`g_aiCommitCounter` is an escalation, not a hold.** While it is non-zero **every** AI
  melee unit charges regardless of what is near it, and missile units close half the
  distance. It is raised by 3 when the side is fighting more engagements than its
  battlefield budget allows, and by 20 when its missile troops fall below an eighth of its
  army — "we
  have run out of archers, so go in". It decays by 1 per unengaged unit per think.
* **Charge radius is tiny.** `Enemy_NearestUnit(unit, 8, 3)` will not see an enemy nine
  cells away, and will not see one at all if it has fewer than three figures. A worn-down
  enemy unit becomes invisible to the melee AI.

---

## 3. Target selection

The single most visible AI behaviour, and it happens at three levels.

### 3.1 Unit picks a unit — `Enemy_NearestUnit` (`0x0048EF45`)

**[D]** Nearest enemy **unit** by Chebyshev distance between the two units' recentred
positions (`battle.md` §1, `+0x1E`/`+0x20`), subject to `distance <= maxDist` and
`figureCount >= minFigures`; returns 0 when nothing qualifies. No weighting of any kind —
no troop type, no strength, no facing, no threat. Callers:

| caller | maxDist | minFigures | note |
|---|---|---|---|
| `UnitOrder_FieldMissile` | 80 | 1 | |
| `UnitOrder_FieldFoot` | 8 | 3 | |
| `UnitOrder_FieldMelee` | 9 | 3 | |
| `UnitOrder_SiegeDefMissile` | 80 | 1 | result discarded — §6.3 |
| `UnitOrder_SiegeDefFoot` | 10 | 0 | result discarded — §6.3 |

### 3.2 Unit remembers who hit it — `+0x0A` and `+0x0C`

**[D]** `BattleUnits_RebuildFromFigures` (`0x00488DFE`) runs a full pass over both arrays
while `g_battlePhase == 2`, rebuilding each unit's membership from its figures. In the same
sweep, when a figure raises its was-hit flag (`+0x15`) it clears it and:

```c
unit.+0x12 += 1;                                   /* times hit; a u8, so it wraps */
unit.+0x0C  = 50;                                  /* "recently hit" countdown */
unit.+0x0A  = figures[figure.+0x16].unit;          /* who did it */
```

`+0x0C` is decremented once per pass while nothing new hits the unit; `+0x12` decrements
only once `+0x0C` has reached zero, so it is a hit-*frequency* measure, and the field
missile handler uses it as its retreat trigger (`+0x12 > 10`).

**[D] The grudge is much shorter than the think interval.**
`BattleUnits_RebuildFromFigures` is called once per frame from `Battle_Frame`
(`0x004B99C0`, which also drives `Battle_UpdateAllMen`, `Missile_UpdateAll`,
`Battle_CountMenByType` and `Battle_UpdateAllUnits`). So `+0x0C` lasts **50 frames** while a
unit thinks every **200**. A unit that is being shot at intermittently will find `+0x0C`
already expired at three thinks out of four, and will fall through to the "nobody is
attacking me" branch. Anyone reimplementing this must not round 50 up to "until the next
decision".

This is the *only* mechanism by which a unit acquires a target it did not simply walk into
or find by proximity. There is no threat assessment.

### 3.3 Figure picks a figure — `Melee_ChooseChaseTarget` (`0x004954DD`)

**[D]** State 8 — free pursuit — is what `Order_ChargeNearest` and `BattleUnit_JoinMelee`
put figures into, and `BattleMan_StateChase` re-acquires with this function every time its
target dies. It is the closest thing the game has to "who does that soldier run at":

```c
score = Dist_Chebyshev(me, enemy);
if (enemy carries a missile weapon)  score /= 2;
score += enemy.targeted;                          /* +0x175 */
/* skip: dead figures, and troop types 7..10 (siege engines) */
/* lowest score wins; no range limit at all */
chosen.targeted += 2;
```

Three properties, and all three are visible to a player:

* **AI melee troops go for the archers.** Halving the distance to a figure with a weapon
  class makes a crossbowman twice as attractive as a swordsman at the same range.
* **They spread out.** Every chaser adds 2 to its victim's `targeted`, and
  `Battle_UpdateAllMen` decrements `targeted` once per frame, so a figure that several men
  are already running at stops being the cheapest choice. This is a load balancer, not a
  focus-fire rule.
* **They never chase siege engines.** Same exclusion `Melee_FindAdjacentEnemy` applies
  (`battle.md` §6.1), so catapults are safe from being *hunted* as well as from being
  meleed.

**[V]** `targeted` is the debug panel's own label for `+0x175`, per `battle.md` §0 — so the
field this rule balances against is one the developers named.

### 3.4 Figure picks a figure to shoot — three weighted variants

`Missile_FindTarget` (`0x004956CC`) was already documented. It has **two siblings**,
byte-for-byte the same except for one weighting block:

| function | enemy in state 9 (filling the moat) | enemy with a missile weapon | used by |
|---|---|---|---|
| `Missile_FindTarget` `0x004956CC` | — | — | firing (`BattleMan_FireMissile`), `Order_ShootAtUnit` |
| `Missile_FindTargetPreferShooters` `0x00495956` | score ÷ 4 | score ÷ 2 | `Order_ToWallNearPreferredTarget` |
| `Missile_FindTargetAvoidShooters` `0x00495C41` | score ÷ 4 | score × 2 | `Order_ToWallNearAvoidedTarget` |

**[D]** All three: score is Manhattan distance capped at 160; the unit's player-ordered
target (`+0x2C`) is forced to score 0; a siege-engine target costs +35 and is invisible to
a melee figure; and the *range* test is a square box (`|dx| <= range && |dy| <= range`)
even though the *score* is Manhattan. Only the two siege-defender positioning routines use
the weighted variants, and they use them to choose where to *stand*, not what to shoot.

---

## 4. What a handler can actually do

**[D]** Every handler's effect on the world goes through one of about twenty small action
routines. All of them do the same two things — write `unit +0x22/+0x24` (targ x, targ y)
and call `BattleUnit_Order` — except the four that touch figure states directly.

| action | what it writes |
|---|---|
| `Order_DoNothing` | nothing |
| `Order_HoldPosition` | destination = own position |
| `Order_OntoUnit(u)` | destination = `u`'s position |
| `Order_HalfwayToUnit(u)` | destination = halfway to `u`, per axis, with the 8/6-cell gates |
| `Order_StepAwayFromUnit(u)` | destination shifts 2 cells away from `u`; sets `+0x10`, counts `+0x2B` |
| `Order_StepTowardRallyPoint` | destination shifts 2 cells toward `g_aiRallyX/Y` |
| `Order_ToRallyWaypoint(k)` | destination = `g_rallyWaypoints[side][group][k]` |
| `BattleUnit_OrderToEnemyEnd(k)` | destination = the enemy's deployment marker slot `k` |
| `Order_ChargeNearest(u)` | **every figure → state 8**, routes cleared, `+0x2A` set |
| `Order_ShootAtUnit(u)` | **every figure → state 17** aimed at a target inside `u` |
| `Order_ToCell(off)` and the siege family | destination = a battlefield cell |

**There is no formation change, no facing order, no fire-at-will toggle, no reserve, and no
withdrawal from the field.** The AI's entire vocabulary is "go here", "shoot that unit" and
"everybody charge".

### 4.1 The charge

`Order_ChargeNearest` (`0x0048C8AF`) is the most consequential of them. It sets the unit's
halted flag `+0x2A` — which switches the every-500-frame reform off (§5) — and puts every
figure of the unit into state 8, clearing `on route` and `barred`. From then on each figure
picks its own target with `Melee_ChooseChaseTarget` and ignores the unit's destination
entirely. **A charged unit stops being a formation.**

### 4.2 Contact triggers a charge, but only for the AI

**[D]** When a figure walks into an enemy, `BattleMan_Step` gets 999 back from
`Cell_TryEnter` (`battle.md` §7) and calls `BattleUnit_JoinMelee` on **both** units before
locking the two figures into melee. That routine puts every other live, non-melee figure of
the unit into state 8. It is skipped when:

* `unit.+0x01` is set — **the unit is human-controlled**;
* `unit.+0x08 == 1` — the unit is a missile unit;
* `unit.+0x0E` is non-zero — an order lock.

**[D]** `BattleUnit_Order` sets that lock to 64 whenever a unit that is *already in melee*
is given an order (disassembly at `0x00479EF7`), and
`BattleUnits_RebuildFromFigures` counts it down.

So: **an AI melee unit that touches the enemy dissolves into a general brawl; a player's
unit does not, and a player who re-orders a unit mid-melee buys 64 passes of immunity from
it.** That asymmetry is in the code with no ambiguity, and it is probably the single
biggest visible difference between how the two sides fight.

---

## 5. Reforming, and what `re targ` really counts

`battle.md` §1 reads unit `+0x14` (debug label **`re targ`**) as "re-target countdown … at
zero it is reset to 500 and the unit looks for a new target". The countdown and the 500 are
right; what happens at zero is not target selection.

**[D]** `Battle_UpdateAllUnits`, *outside* the human-control guard:

```c
unit.+0x14 -= 1;
if (unit.+0x14 < 1) {
    unit.+0x14 = 500;
    if (BattleUnit_NeedsReform(unit)) BattleUnit_Reform(unit);
}
```

`BattleUnit_Reform` (`0x0048970E`) **re-issues every figure of the unit a destination**: it
builds the unit's formation rectangle around its target and assigns figures to slots. No
enemy is chosen anywhere in it. So `re targ` is "re-target *my own men*" — a formation
tidy-up — and it runs for **human-controlled units too**, which is why
`BattleUnit_NeedsReform` carries a special exemption for a player's units of fewer than
four figures.

The machinery, briefly, because a reimplementation needs it:

| step | function | what it does |
|---|---|---|
| geometry | `Formation_PickGeometry` | the unit's highest-priority troop type (`g_formationTypePriority` = `0,4,1,2,5,3,6,10,8,9,7` by type) dictates footprint and figures-per-row |
| rectangle | `Formation_ComputeRect` | centres it on the unit target, clamps to the map |
| clear? | `Formation_RectIsClear` | every slot on-map, at the target's elevation, not impassable, no foreign figure |
| assign (clear) | `Formation_AssignRectSlots` | walk the slots; each gets the nearest figure not yet assigned |
| assign (blocked) | `Formation_AssignSearchedSlots` | per figure, ring-search outward for usable ground, claiming cells in a 100-entry list |
| dispatch | `Formation_SendFigure` | writes the figure's `tg x/tg y` and its entry state |

**[D]** `Formation_SendFigure` is where a figure's *state* is chosen, and it is worth
reading as a table:

| destination / figure | state |
|---|---|
| ordinary | 3 (walk) |
| destination cell has surface 2 (water) | 9 (fill in the moat) — **except knights (type 6), who are returned unchanged** |
| figure is a siege engine (type ≥ 7) | 10 |
| figure has a bow or crossbow, and either an enemy stands on the destination or the unit has a stored target cell | 17 (close to attack), and its `tg x/tg y` are rewritten to its own position so it stays put |
| destination elevation is 1–3 and the figure is a catapult (weapon class 3) | 12, and the unit's `+0x16/+0x18` pair records the aim point |

**[V]** The moat case is corroborated by `L2.eng` group 214 index 3: *"If the castle has a
moat, select some of your soldiers (preferably peasants) and position the cursor over the
water. Click on the water and the soldiers will begin filling in the moat."* The code sends
figures to state 9 exactly when the destination cell's surface is 2, and the state-9
handler (`0x00483FE1`) raises that cell's terrain byte until it reaches `g_moatFillSteps`
(15, from `Rules_InitConstants`) and then converts it to open ground. The help text's
"preferably peasants" is not in the code — knights are the only type excluded.

**A third `ownerIsHuman` asymmetry.** The moat-fill tick interval is **81 frames when the
figure's owner is not human and 101 when it is**. `battle.md` §6.2 flags the
`ownerIsHuman` byte as an unexplained asymmetry in the damage path; this is an independent
instance of it, in a completely different subsystem, and it points the same way: the human
side is slower. §7 adds a fourth.

---

## 6. The siege handlers

Fourteen of the seventeen handlers are siege-only. They are more scripted and less reactive
than the field ones: most are a ladder of `orders` ranges selecting between a small set of
fixed positions taken from tables `Battlefield_BuildCastle` filled in. `Battlefield_BuildCastle`
was **not** decompiled (it is still open in `battle.md` §11), so the *meaning* of those
positions is inferred from how they are used, not read.

### 6.1 The two progress counters everything keys off

**[I]** Two globals measure how the siege is going, and every siege handler reads at least
one:

* **`g_siegeApproachScore`** (`0x00553FB0`) — raised by the routine that converts a
  filled-in moat cell to open ground, once per orthogonal neighbour still of surface 3, 4
  or 5. Thresholds used against it are 3, 4, 8, 10, 16 and 400.
* **`g_siegeBreachScore`** (`0x0053E990`) — raised by the routine `Missile_Step` calls when
  a shot destroys a cell, once per orthogonal neighbour still of surface 5.

The names are readings of what raises them. What is solid is the *mechanism*: both count
wall-adjacency around newly opened ground, and both are read as "how far in are we".

Two more, recounted every frame by `Battle_UpdateAllMen`:

* **`g_attackersOnWall`** (`0x00553E64`) — live side-4 figures standing on surface 5. Every
  defender handler branches on it, at 1, 2, 3, 4 and 6. **[I]**
* **`g_siegeEngineCount`** (`0x00553FF0`) — live figures of troop type 7, 8 or 9. Three
  attacker handlers will not move onto the castle objective while it is zero. **[D]**

### 6.2 The attacker

| handler | shape |
|---|---|
| `SiegeAttMissile` | field corner on think 0; castle approach every 15th think; from think 11, alternate two staging routines on an **11-of-16** rotation; once there is a breach, shoot into it or move onto the castle objective |
| `SiegeAttFoot` | `orders` ladder at 8, 18, 25, 31, 40, 51, 60, 71 alternating two staging spots; jumps `orders` to 100 on one castle layout; **charges outright once `g_aiStrengthAdvantage` reaches 151** |
| `SiegeAttMelee` | gated on `orders > 5`, then the same ladder at 15, 25, 30, 41, 50, 61, 70, 81, plus a six-phase rotation |
| `SiegeAttKnight` | field corner for 10 thinks, then castle approach points until think 31 |
| `SiegeAttCatapult` | nothing for 4 thinks, field corner to think 13, castle approach to think 21, then from think 31 `Siege_FindCellSurface4(x, y, **20**, 2)` from one of its figures |
| `SiegeAttTower` | field corner for 11 thinks; then, once `g_siegeApproachScore` passes 6 (after think 60) or 10 (any time), widen a search from radius **10 to 30** for a surface-4 cell |
| `SiegeAttRam` | ignores `orders` entirely; picks between its stored second destination and a castle approach from three flags |

**[V] The catapult searches exactly its own firing range.** `Siege_FindCellSurface4` is
called with radius 20, and `g_missileStats` gives the catapult class a range of 160 eighths
of a cell = **20 cells** (`battle.md` §6.2). Two unrelated constants in the binary agreeing
is the strongest check available here, and `L2.eng` 214/2 says the same thing in prose:
*"Battering rams and siege towers must be moved right up to the castle wall to work. A
catapult can attack from a distance."* The tower handler is the other half of that sentence
— it is the one that walks its unit onto the wall cell itself.

### 6.3 The defender

All four of the "real" defender handlers share an opening: if `g_aiStrengthAdvantage`
exceeds **`g_aiSortieThreshold`** (`0x00552FF0`, set to **260** by `Rules_InitConstants`),
they call the drawbridge routine and then charge or advance. **[D]** A garrison therefore
sallies out only when it believes it is 3.6× the attacker's strength, which is close to
never.

**[D] And for missile units the sortie does nothing at all.** `UnitOrder_SiegeDefMissile`'s
sortie branch is

```c
Enemy_NearestUnit(g_curBattleUnit, 80, 1);   /* return value discarded */
Order_HalfwayToUnit(g_curBattleUnit);        /* the unit, halfway to itself */
```

The search result is thrown away, and `Order_HalfwayToUnit` is handed the unit's *own*
index, so both axis separations are zero and it returns without writing anything.
`UnitOrder_SiegeDefFoot` has the same discarded `Enemy_NearestUnit(unit, 10, 0)` call.
Both read as bugs — a missing assignment to the local the next line should have used — but
they are unreachable in practice anyway, because the 260 % threshold that guards them is
almost never met. Stated because a reimplementation that "fixes" them would change
observable behaviour.

**[I]** The drawbridge routine (`0x00496B9F`) is a one-shot latch: it scans for any cell
carrying flag `0x40`, and if one exists it rewrites a 7 × 4 block of cells to surface 3
with graphics from a table, sets a global flag, and adds 4 to **both** progress counters.
Mechanically that is a patch of new passable ground appearing on command. "Lowering the
drawbridge" is a reading of it; nothing outside the code says so.

Otherwise the defenders rotate through fixed wall positions (nine slots for missile units,
four inner slots for melee units, group 2 for knights) and abandon them for the castle
objective as `g_attackersOnWall` climbs. `Siege_ClaimDefencePost` is a twenty-entry
(cell, unit) reservation table that stops two units posting to the same place.

`UnitOrder_SiegeDefOil` is the most specific handler in the set: it moves only if its unit
is at elevation ≥ 2, and then only to the densest cluster of enemy figures within six cells
(needing three enemies) or within four cells (needing two, when it is on the rampart
proper). That is a genuine "wait until they bunch up under you" rule.

**[D]** Categories 9 and 10 — the first two missile units of a garrison — are the odd ones
out. Category 9's handler does nothing at all except increment `orders`, so that unit holds
whatever wall slot `Deploy_SlotForUnitSiege` put it on for the entire battle.

---

## 7. `Path_Search`, extended

`battle.md` §8.3 covers the flood fill's structure. Three of its statements need refining,
and the cost function and the failure path were not covered at all.

### 7.1 The cost function is real, and it is almost always zero

`battle.md` §8.3 says "terrain cost is charged by deferral, not by a priority queue". **Both happen.**
Reading the neighbour expansion in `Path_Search`:

```c
newCost = g_pathCost[cur] + 1 + g_pathStepCost[neighbour];    /* weighted */
...
if (g_pathStepCost[cur] == 0 || g_pathVisitCount[cur]++ >= g_pathStepCost[cur])
    expand cur;
else
    push cur to the back of the queue again;                  /* deferral */
```

So the recorded cost **is** weighted by the destination cell's step cost, *and* an expensive
cell must be popped `stepCost` extra times before it expands. **[D]**

`Path_BuildStepCost` (`0x00471DA6`) is the entire cost function, and it is small:

* every cell starts at 0;
* a cell whose flags carry `0x20` or `0x40` costs **100**, and lays a gradient of **12, 8,
  4, 4** on the cells one, two, three and four rows further **south** — the last two bands
  three cells wide;
* when the flag at `0x0057C910` is set, a cell diagonally adjacent to surface 5 costs 4.

**[D] On a `.skr` battlefield every step costs the same.** `Battlefield_BuildFromSkr` sets
only flags `0x10` and `0x80` on cell byte `+1` — never `0x20` or `0x40` — and `battle.md`
§3 establishes that it never writes the elevation byte either. Both branches of
`Path_BuildStepCost` are therefore dead on a `.skr` map, so `Path_Search` degenerates to a
plain unweighted breadth-first search there. The cost function only exists for castles.

**[D] The cost field is never relaxed.** A neighbour is considered only when
`g_pathCost[neighbour] == 0`, so the first cost written to a cell is the one that stands,
even if a cheaper route reaches it later. This is not Dijkstra, and with non-zero step costs
the recorded field is not a metric.

### 7.2 998 and 999 are different things

`battle.md` §8.3 gives `g_pathCost` as "0 unvisited, 1 the start, 998 blocked". There are **two** blocked
values and they mean different things: **[D]**

| value | written by | meaning |
|---|---|---|
| **999** | `Path_BuildTerrainTemplate` (`0x00471C1F`) | terrain: flags `0x10` or `0x40`, or the one-cell map border |
| **998** | `Path_BuildBlockedMap` (`0x00471F45`) | a **friendly** figure is standing here |

and `Path_Search` treats them differently at the destination: **998 is cleared to 0** (walk
onto the friendly figure's cell anyway — the mover will swap or wait), while **≥ 999 aborts
the search before it starts**.

Two further properties of the blocked map: **enemy figures are never marked**, so the
pathfinder routes straight through the enemy and leaves contact to the mover; and a siege
engine blocks its diagonal neighbours as well as itself, in a pattern that depends on its
facing.

**[D] The pathfinder's map is more permissive than the mover's.**
`Path_BuildTerrainTemplate` blocks on flags `0x10` and `0x40`. `Cell_TryEnter`
(`battle.md` §7) also rejects `0x80` and gates `0x20`. So a route can be planned through
cells the mover will then refuse, which is one mechanism behind the `barred` counter.

### 7.3 What happens on failure

The whole chain lives in `BattleMan_Step`, which is `Path_Search`'s **only caller**:

```
1  the mover finds every direction blocked (Melee/Dir/TryStepDir all fail)
2  if figure.barred >= 4                     -> stop asking the pathfinder at all
3  if figure.holdIt != 0                     -> holdIt--, wait
4  Path_Search (or Path_SearchSiege in state 9); routed++; holdIt = 64
5  Path_DetourTooLong?                       -> cancel the move: tg = own position
6  len = Path_Extract
7  if len == 0:  g_pathFailCount++;  barred++;
                 Path_NearestReachedNear -> retry Path_Extract at the closest cell reached
8  if len != 0 && len < 150                  -> BattleMan_SetPath, follow it
```

**[D]** `barred` is reset to 0 the moment the figure manages any step, so step 2 is "four
consecutive failures", not a permanent state — but a figure genuinely walled in never gets
that step and so stands still for the rest of the battle.

**[D]** `Path_Extract`'s return codes are three-valued, not two: **0** means the destination
was never reached or is blocked; **150** means either the destination was adjacent to begin
with or the descent ran out of downhill neighbours or hit the 150-waypoint cap. The caller
accepts only `0 < len < 150`, so a 150 is a silent failure that leaves the figure walking
straight at its target.

**[D] Step 5 is a fourth `ownerIsHuman` asymmetry, and the clearest one yet.**
`Path_DetourTooLong` (`0x00472227`) returns false immediately unless the figure's owner is
human. For a human figure it returns true when the path cost exceeds 20 (field) or 150
(siege) *and* is more than five times the straight-line Chebyshev distance — and the caller
then cancels the move outright. **A player's soldiers refuse absurd detours and stop where
they are; the AI's soldiers always take them.** That is a plausible reading of a UI
nicety — a player who clicks across a wall should not watch his men march round the world —
but it is also a concrete behavioural difference between the two sides, and it is not
tunable.

Two ready-made diagnostics, both zeroed by `Battle_Start`: `g_pathSearchCount`
(`0x004F037C`) counts flood fills actually run, and `g_pathFailCount` (`0x00507248`) counts
extractions that found nothing. Reading them out of a live process (`battle.md` §9) would
measure how much the pathfinder is struggling without instrumenting anything.

---

## 8. Randomness

`battle.md` §6.4 establishes that nothing in the damage path touches a random number, and
explicitly excludes the untraced order handlers from that claim. They were right to.

**[D]** `Battle_UpdateStrengthAdvantage` calls `Rand_Advance` (`0x00404A46`) — two 31-bit
Fibonacci LFSRs with taps at bits 0 and 4, each stepped 31 times per call, masked into six
output globals. It then adds `(g_rand7A & 0x1F) - 10` to the strength advantage, so **the
AI's estimate of its own strength wanders by −10 to +21 percentage points**, re-rolled every
101 frames. With the aggression threshold at 5, that jitter alone decides whether the field
AI attacks in every battle within about 30 points of even.

`Battle_Start` uses the *other* generator (`FUN_00404B2C` / `DAT_005C9A84`, the one
`battle.md` §6.4 names) to pick `g_battleApproachLane` (0…3) and `g_battleRallyGroup`
(0 or 1) once per battle.

**[I]** Both are gated on `g_deterministicBattle` (`0x00553030`) being zero. When it is
non-zero, `Battle_Start` advances both selectors cyclically instead and the jitter is
dropped entirely. That global is read by well over a hundred functions across the binary,
including `Sync_Checksum` and `Turn_AllRealmsDone`, and `Troops_Load` switches to the
side-neutral `TROOPS.ENG` when it is set. Everything is consistent with "this is a
networked game, so nothing may diverge" — which is a reading, and the flag was not traced to
where it is set.

Either way: **the battle AI is not deterministic in single-player**, and any differential
test against the original will have to account for that.

---

## 9. What is still unknown

* **Nothing here has been observed running.** Every claim is static. `battle.md` §9's
  `battlestate.ps1` can read `g_aiStrengthAdvantage`, `orders`, `+0x0A`, `g_pathFailCount`
  and the rest out of a live process; none of it has been.
* **What `orders` phase boundaries mean.** The ladders (8, 18, 25, 31, 40, 51, 60, 71 and
  friends) are read directly from the code. Whether they correspond to anything a designer
  would have named is unknown, and the temptation to call them "approach / deploy / assault"
  was refused.
* **`Battlefield_BuildCastle` was not decompiled**, so the castle position tables
  (`0x0055CD90`, `0x00554180`, `0x00553274`, `0x00553EE4`) are named by *use*, not by
  content. Every siege claim in §6 rests on that.
* **The drawbridge routine's intent.** `0x00496B9F` demonstrably paints 7 × 4 cells and
  bumps both progress counters. Whether that is a drawbridge, a sally port or a siege ramp
  is not established.
* **`g_siegeApproachScore` and `g_siegeBreachScore` names.** The mechanism is read; the
  names are [I].
* **`ownerIsHuman` still has no explanation**, but there are now four independent instances
  of it, all pointing the same way: missile damage and burning (`battle.md` §6.2, §6.3),
  the moat fill rate (§5), and the detour cancellation (§7.3). The first three all leave the
  human-owned figure worse off; the fourth reads at least as easily as a UI nicety. A
  deliberate handicap is now the more economical reading than an inverted test, but it is
  not proof, and one instance of the four points away from it.
* **The per-battlefield engagement budget** at `0x00553080`, indexed by a battlefield
  identifier at `0x005653F8`, was not traced to where it is filled.
* **`L2.eng` group 47** — the "Group Information" panel — holds five category names
  (`Heavy Infantry`, `Light Infantry`, `Slingers`, `Mixed Troops`, `Auxiliaries`) and the
  field order table holds five slots. **No mapping between them could be established**, and
  the coincidence of the number five is exactly the shape of `decisions.md` C3. `battle.md`
  §6.4 already records that no caller pushes group 47 to `Eng_DrawString`, so the panel may
  not ship enabled at all. Left unassigned deliberately.
* **`Path_SearchSiege`** (`0x00471718`), the state-9 variant, was read only far enough to
  confirm it is the same shape. Its differences were not enumerated.
* **Frame rate.** As in `battle.md` §11: "thinks once per 200 frames" cannot be converted
  to seconds.

---

## 10. Corrections to `battle.md`

For that document's owner; nothing outside `battle-ai.md`, `symbols.json`, `symbols.md` and
`tools/battleai/` was touched.

* **§11, "Twenty-five functions across three dispatch tables."** Twenty-five *slots*, **18
  distinct functions**, one of which (`UnitOrder_None`) is an empty stub filling seven of
  them — so **17 real handlers**. The quoted range `0x0048A9C7 … 0x0048ECA9` also overshoots:
  the last handler is `0x0048E8B8`, and `0x0048ECA9` is a helper. §1.1.
* **§1, unit `+0x14` `re targ`.** The countdown and the 500 are right, but at zero the unit
  **reforms its own figures onto formation slots**; it does not look for an enemy. It also
  runs for human-controlled units. §5.
* **§1, unit `+0x1A` `orders`.** It is a monotonically increasing count of AI decisions used
  as a script program counter, written only by the 17 handlers (plus two `= 100` jumps), and
  never by `BattleUnit_Order`. §1.4.
* **§8.3, "terrain cost is charged by deferral, not by a priority queue."** Both mechanisms
  are present: the recorded cost is `cost[cur] + 1 + stepCost[neighbour]` *and* an expensive
  cell is re-queued. Also worth adding that the step cost is all zeros on a `.skr`
  battlefield, so the field pathfinder is a plain BFS. §7.1.
* **§8.3, "`g_pathCost` … 998 blocked."** 998 (a friendly figure) and 999 (terrain or
  border) are distinct and the search treats them differently at the destination. §7.2.
* **§6.4, "no random numbers."** Still true of the damage path. The AI layer *does* use a
  PRNG — a different one from the battlefield builder's — and the AI's estimate of its own
  strength carries a −10…+21 jitter. §8.
* **§11, `ownerIsHuman`.** Two more instances found, in the moat fill rate and in
  `Path_DetourTooLong`. §5, §7.3.

---

## 11. Reproduction

Everything below writes into `tools/battleai/out/`, which is gitignored — it is derived
from the game binary and must never be committed. The output paths must be **absolute**:
the Ghidra scripts open them relative to the JVM working directory, not the repository.

```powershell
# the three dispatch tables and the two tables above them, resolved to function names
powershell -File tools/battleai/ghraw.ps1 -postScript BTable E:\dev\lords2\tools\battleai\out\tables.txt `
    4d9140 12 4d9170 18 4d91b8 6 4d91d0 10 4d91f8 12

# the 17 handlers
powershell -File tools/battleai/ghraw.ps1 -postScript BDecomp E:\dev\lords2\tools\battleai\out\field.c 0048a9d2 0048acd2 0048b02b
powershell -File tools/battleai/ghraw.ps1 -postScript BDecomp E:\dev\lords2\tools\battleai\out\siegeatt.c `
    0048d16e 0048d412 0048d6fc 0048d9ce 0048db84 0048ddc7 0048dfbb
powershell -File tools/battleai/ghraw.ps1 -postScript BDecomp E:\dev\lords2\tools\battleai\out\siegedef.c `
    0048e097 0048e234 0048e2c8 0048e39c 0048e4cc 0048e774 0048e8b8

# target selection
powershell -File tools/battleai/ghraw.ps1 -postScript BDecomp E:\dev\lords2\tools\battleai\out\tgt.c `
    0048ef45 004954dd 004956cc 00495956 00495c41

# the pathfinder and its three template builders
powershell -File tools/battleai/ghraw.ps1 -postScript BDecomp E:\dev\lords2\tools\battleai\out\path.c `
    0047095e 00471c1f 00471d30 00471da6 00472392 0047265d 00472227

# who touches a unit field: range refs over unit[0]
powershell -File tools/battleai/ghraw.ps1 -postScript BRefs E:\dev\lords2\tools\battleai\out\urefs.txt range 00566520 00566554

# the rule constants, as immediates
powershell -File tools/battleai/ghraw.ps1 -postScript BDump E:\dev\lords2\tools\battleai\out\rules.txt disasm 004983b7 30

# everything reachable from the handlers, two levels deep
powershell -File tools/battleai/ghraw.ps1 -postScript BClosure E:\dev\lords2\tools\battleai\out\closure.txt 2 `
    0048a9d2 0048acd2 0048b02b 0048d16e 0048e097

# the per-frame passes, which is what fixes every "frames" figure above
powershell -File tools/battleai/ghraw.ps1 -postScript BRefs E:\dev\lords2\tools\battleai\out\frame.txt callees 004b99c0
```

```bash
# rewrite g_battleUnits / g_battleMen / g_battlefield DAT_ symbols as field accesses
node tools/battleai/annotate.js tools/battleai/out/field.c > tools/battleai/out/field.ann.c

# dump every L2.eng string as "group<TAB>index<TAB>text"
node tools/battleai/engdump.js "F:/games/Lords of the Realm II/L2.eng"

# merge the names in tools/battleai/newsymbols.json into docs/symbols.json
node tools/battleai/merge.js --dry-run
node tools/battleai/merge.js
node tools/symbols/symbols_md.js
```

Ghidra scripts live in `ghidra_scripts_battleai/` (`BDecomp`, `BRefs`, `BDump`, `BTable`,
`BClosure`, `BCallArg`), kept separate from `ghidra_scripts/` and `ghidra_scripts_battle/`
so parallel agents never edit the same files. `BTable` and `BClosure` are new here; the
rest are copies. Names go into the database with
`ghidra_scripts/ApplySymbols.java` as described in [`symbols.md`](symbols.md).

---

## 12. Corrections to this document

Found by implementing it in `crates/l2-sim/src/ai.rs` and re-reading the
seventeen decompiled bodies against each claim. Implementing a document is the
only way to find out whether it is true; these are the places it was not.

* **§2.3, "for the first ten (thirteen) thinks the unit marches".** There is a
  silent phase before the march. `UnitOrder_FieldFoot` calls `Order_DoNothing`
  while `orders < 2` and `UnitOrder_FieldMelee` while `orders < 4`; only then
  does the `BattleUnit_OrderToEnemyEnd` ladder start, and it still ends at 10
  and 13. So an aggressive foot unit stands still for two thinks — four hundred
  frames — before it moves at all.

* **§2.3's pseudocode gives `Order_ToRallyWaypoint(2)` for both handlers.**
  `UnitOrder_FieldFoot` uses waypoint **2**; `UnitOrder_FieldMelee` uses
  waypoint **1**. The two functions differ in five constants, not four.

* **§2.3's cautious branch is two decisions, and the second overwrites the
  first.** The disengage-and-mark block and the charge/rally block both run, and
  both write the unit's destination, so the rally wins when it fires. It usually
  does not, for `UnitOrder_FieldFoot`: its rally is gated on `withdrawals == 0`
  and `Order_StepAwayFromUnit` has just incremented that field to 1.
  `UnitOrder_FieldMelee` allows two withdrawals, so there the rally *does* fire
  and the withdrawal it just ordered is discarded on the same think. Both are
  the original.

* **§2.1's cautious skeleton omits a write.** Both melee handlers clear the
  unit's halted flag (`+0x2A`) whenever `g_aiCommitCounter < 1`, which is how a
  unit that charged earlier becomes eligible for the every-500-frame reform
  again.

* **§6.2's catapult ladder is missing a rung.** Between the castle approach at
  think 21 and the surface-4 search at think 31 there is an
  `orders % 100 == 0 -> Order_ToCastleApproach(5)` branch, which fires again at
  think 100 and every hundredth after it.

* **`Order_ToBreachOrStaging` is not looking for a breach.** Below approach
  score 16 it calls `FUN_00496768`, which searches radii 12…19 for the nearest
  cell of **surface 2** — water — and walks the unit onto it. Surface 2 is what
  `Formation_SendFigure` sends figures to state 9 for (§5), and state 9 is the
  moat fill. So the opening phase of a siege attack is an order to go and fill
  the moat in, and the name reads it backwards. The 16…400 fallback to the
  secondary staging table, and the silence above 400, are as §6.2 describes.

* **`Siege_FindCellSurface5` measures its distance from the wrong point, and it
  is an original bug.** It saves the query cell into two locals, then overwrites
  the *parameters* with the clipped top-left corner of its search box, and then
  calls `Dist_Manhattan` with the **parameters** rather than the saved locals.
  Its sibling `Siege_FindCellSurface4` saves the query point and uses it, and
  does not have the fault. So `Order_ToSurface5Near` puts a defending unit on
  the rampart cell nearest the corner of the search box rather than the one
  nearest the post it was reserving. Reproduced in `l2-sim` with a comment
  saying why: the original's choice of cell is the specification. The same
  overwrite pattern appears in `FUN_00496768`, where it is harmless, because
  there the saved locals *are* what the distance call uses.

* **§3.1, "nearest enemy unit".** `Enemy_NearestUnit` compares the two units'
  **owner bytes** (`+0x00`), not their sides. Two AI realms fighting each other
  on the same side of a battle are enemies to it; a unit is never its own
  owner's enemy. The distinction only matters in a multi-realm battle, which is
  exactly where a side-based reading would be wrong.

* **`Order_ToWallSlot`'s groups are sixteen entries wide,** not the nine and
  four §6.3 gives. Nine and four are how far the two *cursors* count before
  wrapping; the routine itself scans forward from the requested slot, skips
  empty `(0, 0)` entries, wraps, and then offsets the destination one cell west
  and two north of the recorded position. A reimplementation that stored nine
  slots would still be right about behaviour and wrong about the table.

* **What could not be checked.** Everything positional in §6 still rests on
  `Battlefield_BuildCastle`, which is still not decompiled, so `l2-sim` takes
  those tables as data supplied by the caller rather than asserting their
  contents. And §9's first bullet stands unchanged: **nothing here has been
  observed running.**
