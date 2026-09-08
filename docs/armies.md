# Armies on the campaign map

The half of the game `docs/mechanics.md` marked ❓ *"Armies on the campaign map: raising,
moving, supplying. **Nothing exists.**"* — plus the mercenary system, which turned out not
to be a random event at all.

[`battle.md`](battle.md) §5 covers *raising a battle* from an army record. This covers the
record itself, and everything that happens to it between battles: how it is levied, how it
moves, what it costs to keep, what it eats, what it tramples, how it besieges a castle, and
how a battle result comes back.

Status legend as in `battle.md` and `kingdom.md`, meant literally:

* **[V] verified** — read out of the binary *and* cross-checked against a second
  independent source: an `L2.eng` string the code draws beside the field, an exact
  arithmetic invariant over a table's layout, or two unrelated functions agreeing.
* **[D] decompiler-only** — a straightforward reading of decompiled C, no second source.
  Probably right about *what the code does*; the *name* may be wrong.
* **[I] inferred** — consistent with everything measured, not proven.

`docs/decisions.md` C3 is the standing hazard here: decompiler output invites a plausible
story. §5.1 says explicitly where a match is a finding and where it would have been a fit.

Addresses are the GOG Windows build, `ImageBase 0x400000`, no ASLR. Everything named here
is in [`symbols.json`](symbols.json).

---

## 0. The headline

**An army is a unit.** It lives in the same 151-record array as merchants and transports —
`g_units`, `0x0052F0B0`, stride `0x1A4` — already documented by
[`plane4.md`](formats/plane4.md) for the merchant half. Unit **type 1** is an army; the type
byte is `+0x08`.

| | Value | Ev |
|---|---|---|
| array | `g_units` `0x0052F0B0`, stride `0x1A4`, slots **1 … 150** | [V] `g_saveBlocks` row 5: 151 × 0x1A4 = 63,420 bytes |
| unit types | 1 army, 2 revolting peasants, 3 merchant, 4 transport | [V] `L2.eng` group 31 and the dispatch table `0x004D6A50` |
| troop types in a record | **7** — peasant, crossbowman, maceman, swordsman, pikeman, archer, knight | [V] `L2.eng` group 8 indices 52 … 65 |
| movement budget | **15 points a season**, the same for every army | [V] `Army_Tick` writes 15 unconditionally; the panel prints `15 − used` |
| step cost | **1** on a road, **3** on open ground, **6** across a standing field | [V] two unrelated codings agree — §2.2 |
| maximum army | **1500 men** | [V] the literal `0x5DD` in `Army_Combine` |
| minimum army | **50** men to raise; below **30** it is destroyed on the map | [V] `L2.eng` 148 *"impractical to create an army of less than 50 men"* |
| wage | `men / 4` for a human owner | [V] already in `kingdom.md` §7.4 — §6.4 here connects it to something |
| mercenary bands | **12**, one per nationality, fixed sizes and prices | [V] six static tables that close arithmetically — §5.1 |

What makes all of this cheap to trust is that **`L2.eng` group 31 is the army info panel's
own field labels**, drawn by `UnitPanel_Draw` (`0x0041B19D`) right next to the offsets:

```
31/8  Wages          31/20 Formed        31/21 Morale       31/22 moves left.
31/23 Fed in your county.        31/24 Foraging in other lands.
31/25 Provisioned on home soil.  31/26 Foraging in your county.
31/27 These troops are healthy.  31/28 Ill, will perish in 4 seasons.
31/29 …in 3 seasons.  31/30 Diseased, …in 2 seasons.  31/31 Dying, will perish if not fed.
```

That is this layer's equivalent of the battle debug overlay, and it is what makes the supply
fields **[V]** rather than **[D]**.

---

## 1. The army record

`g_units + unit * 0x1A4`. Offsets shared with merchants and transports are marked *sh*; two
of those (`+0x14F`, `+0x164`) carry a different meaning per unit type.

### 1.1 Identity and position

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x00` | i8 | **owner** | [V] | realm 1 … 5; **0 = the slot is free**. 6 marks an ownerless unit. |
| `+0x01` | u8 | ownerIsHuman | [D] | copy of realm `+0x05` at creation. Gates a sound, the AI siege-engine defaults, and the post-battle move penalty. |
| `+0x02` | u8 | shield | [D] | copy of realm `+0x0A`; the banner. |
| `+0x06` | u8 | isPlayerDriven | [D] | 1 for armies made by `Army_Create`; suppresses the automatic re-path at the end of a move. |
| `+0x07` | u8 | spriteFrame | [V] | recomputed every tick — §2.4. |
| `+0x08` | u8 | **type** | [V] | 1 = army. |
| `+0x09` | u8 | facing | [V] | 0 … 7, the direction of the last step. |
| `+0x0A` `+0x0B` | i8 | **x, y** | [V] | tile coordinates 0 … 63. |
| `+0x0C` | i32 | tileOffset | [V] | `(y*64 + x) * 8` — a byte offset into `g_tiles`. |
| `+0x10` | u8 | **county** | [V] | the county it is standing in; `Army_Tick` keeps it equal to the tile's county byte. |
| `+0x11` | u8 | **homeCounty** | [V] | where it was raised. `L2.eng` 31/9 *"An army from"* + the county name. |
| `+0x12` `+0x13` | i8 | pixel sub-position | [D] | `x<<4`, `y<<4` at spawn. |
| `+0x14` `+0x15` | i8 | stepTargetX/Y | [D] | the tile currently being walked to. |
| `+0x16` `+0x17` | i8 | destX/destY | [D] | the end of the ordered path. |
| `+0x1B` | u8 | walkPhase | [D] | indexes `g_unitWalkFrames` (`0x004D6A78`) = `0,1,2,1,…`. |
| `+0x1C` | u8 | **pathLen** | [V] | steps remaining; the stepper counts it down to 0. |
| `+0x1D … +0x148` | u8×300 | **path** | [V] | 150 `(x, y)` pairs, walked from `pathLen−1` downwards. |

**[V] The path array closes exactly.** `+0x1D + 150 × 2 = +0x149`, and `+0x149` is the next
offset anything in the binary references. The 150 is the loop bound in `Path_CopyToUnit`
(`0x004707BE`), which copies from a shared buffer of the same size.

### 1.2 Movement and supply state

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x149` | i8 | stepAccum | [D] | 0 … 15 sub-tile accumulator; +2 a frame, +4 with animation off. At 16 the unit lands on the next tile. |
| `+0x14A` | i8 | animTick | [D] | frame delay: 3 off-road, 0 on a road, so units visibly move faster on roads too. |
| `+0x14B` | u8 | flags | [D] | bit 0 = "on a tile centre, ready for the next step". |
| `+0x14C` | u8 | **moveState** | [V] | 0 idle, **2 = moving**. |
| `+0x14D` | u8 | onRoad | [V] | set when the tile just entered was a road; makes the step cost 1 instead of 3. |
| `+0x14E` | u8 | ignoreSettlements | [D] | when set, a settlement tile stops blocking. |
| `+0x14F` | u8 | **nameIndex** *sh* | [V] | index into `L2.eng` group `93 + owner` — 24 army names a lord. For a merchant this same byte is the route number. |
| `+0x150` | u8 | needsDestination *sh* | [V] | 1 = idle, no orders. |
| `+0x151` | u8 | destCounty *sh* | [V] | the county the current order leads to. |
| `+0x152` | u8 | orderMode | [D] | written by `Unit_OrderMove`; not traced. |
| `+0x153` | i8 | **movesUsed** | [V] | this season. `L2.eng` 31/22 prints `15 − movesUsed` as *"moves left."* |
| `+0x154` | i8 | **moveAllowance** | [V] | **15** for an army, 10 for the other three types. |
| `+0x155` | i8 | **starvation** | [V] | 0 … 5, drawn as `L2.eng` 31/(27 + value): *healthy / ill … 4 / 3 / 2 seasons / dying*. |
| `+0x156 +0x157 +0x158` | u8 | order flags | [D] | set to 1 when a move is ordered, cleared by `Army_Starve`. |
| `+0x15C` | i32 | **wages** | [V] | `L2.eng` 31/8 *"Wages"*. Written by `Wages_ForUnit`. §6.4. |
| `+0x160` | i32 | cooldown | [D] | decremented once a tick while positive. |
| `+0x164` | i16 | **yearFormed** *sh* | [V] | `L2.eng` 31/20 *"Formed"*, drawn through `Ui_DrawYear`. For a merchant this is the route cursor. |
| `+0x166` | u8 | **morale** | [V] | `L2.eng` 31/21 *"Morale"*. Copied from the county's happiness when the army is raised; nothing was found that changes it afterwards. |

### 1.3 Troops

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x168` | i32 | **men** | [V] | total. Wages, the sprite class, starvation and the 1500 cap all read this. |
| `+0x16C + t*2` | i16×7 | **troops[t]** | [V] | t = 0 peasant, 1 crossbowman, 2 maceman, 3 swordsman, 4 pikeman, 5 archer, 6 knight. |
| `+0x17A +0x17C +0x17E` | i16 | catapults, towers, rams | [V] | troop types 7, 8, 9 — filled from `+0x182 …` immediately before a battle. |
| `+0x180` | i16 | oil | [V] | troop type 10. Filled for the *defender* with **1, 2, 3, 4 or 6** by castle type. |
| `+0x182 + e*6` | — | siege engine build record ×3 | [V] | `+0` count ordered, `+2` percent complete, `+4` man-seasons of work done. §4. |
| `+0x195` | u8 | mercTroopType | [V] | 0 … 6, the same numbering. |
| `+0x196` | u8 | **mercMen** | [V] | **a byte** — the largest shipped band is 250, so it fits, and nothing clamps it. |
| `+0x197` | u8 | **mercBand** | [V] | 1 … 12; indexes both `g_mercBands` and `L2.eng` group 16 (the nationality). |

**[V] `+0x16C` is one eleven-entry array, not two.** `Battle_RaiseSide` walks `g_raiseOrder`'s
eleven troop types and reads `+0x16C + t*2` for every one of them, so `+0x16C … +0x181` is
`i16 troops[11]`; the campaign only ever writes the first seven, and `Army_PrepareForBattle`
(`0x004AA6CA`) fills 7 … 10 from the siege records just before the battle starts. `+0x182` —
the next thing anything references — is exactly where that array ends.

### 1.4 Castle, siege and links

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x198` | u8 | **garrisonCounty** | [V] | non-zero = this army is inside that county's castle. County `+0x1BC` (i32) holds the unit index back. |
| `+0x199` | u8 | **besiegingCounty** | [V] | non-zero = camped outside that county's castle, building engines. |
| `+0x19A` | u8 | besiegedBy | [D] | on a *garrison*, the unit index of its besieger. |
| `+0x19B` | u8 | — | [D] | copied from realm `+0xE8`; not traced. |
| `+0x19C` | u8 | **siegeSeasonsLeft** | [V] | drawn with `L2.eng` group 8/66 *"Season(s)"* on the siege-preparation screen. §4. |

A garrisoned army (`+0x198 ≠ 0`) is **excluded from the county troop count**, never starves,
and draws a different icon set. [V]

### 1.5 What is not traced

`+0x03`–`+0x05`, `+0x0D`–`+0x0F`, `+0x18`–`+0x1A`, `+0x159`–`+0x15B`, `+0x15D`–`+0x15F`,
`+0x161`–`+0x163`, `+0x167`, `+0x193`, `+0x194`, `+0x19D`–`+0x1A3`. `+0x1A` takes small
enumerated values (2, 5) on the garrison path and looks like a UI feedback code. None were
chased.

---

## 2. Movement

### 2.1 The turn shape

`Turn_Tick` (`0x0049A010`) runs seven phases. Two matter here:

* **Phase 2 is *sieges*, not general movement.** `Siege_StartPhase` (`0x004A82B9`) breaks
  stale garrison/besieger links, then `Siege_BuildTick` runs for each besieging army until
  one reports its engines finished, at which point the assault launches. The existing symbol
  comment on `Turn_Tick` calls phase 2 "army movement"; that is too generous. [D]
* **Phase 7, end of season**, calls `Units_ResetMoves` (`0x004651B9`) — `+0x14C = 0` and
  `+0x153 = 0` for all 150 slots — then `Move_BuildCostMap` and `Mercenary_AdvanceAll`. [V]

Player-ordered movement happens during phase 4 (the players' turn); `Units_Tick`
(`0x004650B0`) is driven from the frame loop and dispatches each unit through
`g_unitTickTable[type]` (`0x004D6A50`). **Slot 5 of that table is NULL** while the dispatcher
accepts types up to 5, so a type-5 unit would call address 0. Nothing spawns one. [D]

### 2.2 The cost of a step — two codings that agree

`Move_BuildCostMap` (`0x0046FF43`) rebuilds a 64×64 `i16` cost map, `g_moveCost`
(`0x004F4080`), once a season, straight off the map planes documented in
[`maps-layers.md`](formats/maps-layers.md) §2:

```
plane0 & 0x0C        -> 0     impassable (0x04 sea / no county, 0x08 mountain or woodland)
tile county > 16     -> 0     impassable
plane0 & 0x01        -> 1     road
plane0 & 0x20        -> 3 if terrain < 2 ; 6 if terrain < 0x17 ; else 3        farm field
plane0 & 0x40        -> 100   castle site
plane0 & 0x80        -> 3 if terrain == 0x14 ; 0 if terrain in {3,6,9,12} ; else 100
plane0 & 0x10        -> 100 if terrain == 0x10 ; else 0
otherwise            -> 3     open ground
```

The *stepper* never reads that map. `Unit_Step` (`0x00465D28`) charges `+0x153` from the
return code of `Unit_TryEnterTile` (`0x00466C3C`), which classifies the same plane-0 bits
independently:

| tile | `Unit_TryEnterTile` returns | actually charged | cost map says |
|---|---|---|---|
| road (`0x01`) | 3, sets `onRoad` | **+1** | **1** |
| open ground | 1 | **+3** | **3** |
| farm field (`0x20`) | 8 → `Unit_CrossField` adds 3, then the general +3 | **+6** | **6** |
| castle (`0x40`) | 5 → `Transport_Deliver`, move ends | — | 100 |
| settlement (`0x80`) | 6 → `Unit_TrampleTile` adds 7, move ends | **+7** | 100 |
| dwelling plot (`0x10`) | 7 → `Unit_BurnDwelling`, move ends | — | 100 |
| occupied | `Unit_EnterOccupiedTile` — §2.7 | varies | — |

**[V] Road 1, ground 3, field 6 agree exactly between two unrelated functions**, and the
field's 6 is assembled in the stepper from two separate `+3`s that land on a single literal
`6` in the cost map. The player's account — every army has the same budget, terrain changes
the cost, size does not affect speed — holds on all three counts.

Note that impassability lives *only* in the cost map, i.e. in the pathfinder. The stepper
would walk into the sea if a path led there.

### 2.3 Ordering a move, and the gold-ball preview

`Unit_OrderMove` (`0x004A98FE`):

```c
Move_BuildCostMap();                       /* 0x0046FF43 */
Move_FloodFill(0, unit.x, unit.y, 0);      /* 0x0046F700 -> a distance field */
if (Path_Extract(0, destX, destY))         /* 0x004701AC */
    Path_CopyToUnit(0, unit);              /* 0x004707BE */
unit.destX = destX; unit.destY = destY; unit.moveState = 2;
```

There are **two distance fields**: `g_moveDistLocal` (`0x00500C30`) for the local player and
`g_moveDistOther` (`0x004F0380`) for everyone else, each 64×64 `i16`. `Path_Extract` is a
greedy descent down whichever it is given, appending `(x, y)` pairs into `g_pathBuf`
(`0x00553550`, stride 300) with the length in `g_pathLen` (`0x0053F070`). On a road tile
(cost 1) it steps the direction index by 2 — orthogonals only — and falls back to all eight
only when that finds nothing. [D]

**The preview is that same data read twice.** `Path_MarkPreviewTiles` (`0x004A91BA`) walks the
local player's path buffer and sets **bit `0x40` of byte `+2` of each runtime tile record**.
`Map_DrawPathMarker` (`0x004081A6`) then draws, for every tile carrying that bit:

```c
n = distance[tile] - 1;
if (unit.moveAllowance - unit.movesUsed < n) n = 0;             /* out of range */
frame = (tile is castle/settlement/plot) ? 0x4E : 0x38 + n;     /* g_flagsSheet */
```

so the frame index **is** the accumulated cost, and everything past the remaining budget
collapses to frame `0x38`. That is the player's *"gold balls along the steps, greyed where
out of range"*: one sprite bank, `0x38 … 0x4D`, indexed by cost. **[V]** on the mechanism;
**[I]** that frame `0x38` is specifically the grey one — nobody has looked at the sheet.

The hover handler `Map_HoverUnitTarget` (`0x004A8E0B`) runs the same pathfind against the
tile under the cursor and, when `allowance − used < distance`, **zeroes every interaction
target it had collected** — an out-of-range click does nothing at all rather than walking as
far as it can. [D]

### 2.4 The sprite, and the three size classes

`Army_Tick` (`0x0046521F`), every tick:

```c
unit.moveAllowance = 15;
base = men < 301 ? 0x48 : men < 601 ? 0x60 : 0x78;
unit.spriteFrame = base + 3 * ((unit.facing + 1) & 7) + g_unitWalkFrames[unit.walkPhase];
```

`0x48`, `0x60`, `0x78` are 24 apart = 8 facings × 3 walk frames, and `g_unitWalkFrames`
(`0x004D6A78`) is `0,1,2,1,…`, a ping-pong. **Three size classes, at 301 and 601 men** —
the player's *"one, two or three figures drawn depending on size"*. **[V]** on the thresholds
and the arithmetic; **[I]** that the three banks are literally one, two and three figures.

`Army_Tick` also calls `Unit_EnterCounty` whenever the tile's county byte differs from
`+0x10`, and decrements `+0x160`.

### 2.5 Crossing a border

`Unit_EnterCounty` (`0x004ABB36`) sets `+0x10` and then fires diplomacy.

**A neutral county** (owner 0) sends one of five messages, chosen by `County_GreetArmy`
(`0x004ABF77`) from the county's happiness and the army's size relative to its population:

| condition | msg id | `L2.eng` group |
|---|---:|---|
| happiness < 10 | `0x82` | 130 *"The people are wretched, my liege. We beg that you journey to our town…"* |
| happiness < 30 | `0x83` | 131 *"We welcome you to our humble county, noble lord."* |
| army ≥ population | `0x83` | 131 — the same welcome |
| 50 … 99 % of population | `0x84` | 132 *"We have had no notification of your army's passage…"* |
| 20 … 49 % | `0x85` | 133 *"…unacceptable. Remove your troops now, or face the consequences!"* |
| under 20 % | `0x86` | **134 *"Your violation of our borders is an outrage. You will pay for these bully-boy tactics…"*** |

**[V]** — the exact string the player quoted, at group 134, reached by message id `0x86`.
Message id equals group number throughout this table, which is the rule `eng.md` and three
earlier subsystems already established. The rule itself is **intimidation, and it runs the
way it reads**: the bigger your army relative to the county, the politer the peasants get;
only a small force gets the outrage.

**An owned county** whose owner is not the army's sends message `0xAA` = group 170,
*"Invasion of"* — sixteen variants indexed by `lord * 4 + realm[+0x159]`, a counter cycling
0 … 3 so a lord does not repeat himself. It fires only when the county is the army's declared
destination (`+0x151`), so marching *through* does not trigger it. [D]

Both paths then call `Army_RecountCountyTroops` (§3.2).

### 2.6 Trampling — fields, industry, and whether you can wreck your own

Two effects, both during a step, both with the same ownership guard.

**Fields** (`plane0 & 0x20`) — `Unit_CrossField` (`0x0046673C`):

```c
if (type != 3 && type != 4) {                        /* not a merchant, not a transport */
    unit.movesUsed += 3;
    if (unit.owner != county[tileCounty].owner && terrain in [2, 0x16]) {
        if (realm[unit.owner].isHuman) Diplomacy_Worsen(countyOwner, unit.owner, 10);
        County_DestroyField(tile);
    }
}
```

`County_DestroyField` (`0x00469E5B`) removes one field and its proportional share of the
produce: a grain field takes `crop × (100 / fieldsSown) %` off county `+0x244` and decrements
`+0x201` (`fieldsGrain`); a pasture takes the same fraction off the herd and decrements
`+0x200` (`fieldsCattle`). The tile is repainted as bare ground.

> **The player asked whether you can destroy your own fields. On this path, no.** The guard is
> `unit.owner != countyOwner`, tested against the county the *tile* belongs to, so an army
> never damages its own realm's fields. It does damage **neutral** counties, because owner 0
> equals no realm. **[D]** — a plain reading of one `if`, and a negative claim, which is the
> weaker kind. If a player has watched their own fields go, the mechanism is elsewhere; a
> battle fought on farmland is the obvious candidate and was not traced.

A second asymmetry: **the diplomatic penalty applies only when the trampling realm is human.**
An AI army wrecks fields for free. That is a fourth `ownerIsHuman` branch to add to the three
`docs/battle.md` lists. [D]

**Industry sites** (`plane0 & 0x80`) — `Unit_TrampleTile` (`0x0046873F`), same guard, and it
costs **+7 moves**. See §3.5: this is the writer of the industry disablement counter.

### 2.7 Entering an occupied tile

`Unit_EnterOccupiedTile` (`0x004658C1`) decides what happens when the next tile holds another
unit. Types 3 and 4 return immediately — merchants and transports are non-combatants, already
in `plane4.md` §3c. For armies: same owner or an ally → `Army_Combine`; otherwise the battle
is set up and both sides are charged moves before control leaves the campaign map. Charges of
**+5** (garrisoning, merging) and **+7** (losing) appear along these paths. The full branch
structure was not traced.

`Army_Combine` (`0x004AA181`) is where the size cap lives:

```c
if (menA + menB < 0x5DD) {                       /* 1501 -> a merged army is at most 1500 */
    if (mercA == 0 || mercB == 0) { ...merge... }
    else message 0xA7;                           /* group 167 */
}
```

**[V] 1500 is exact**, and it is the player's *"maximum army size is about 1500"*. Two armies
that both carry mercenaries refuse to merge: `L2.eng` **167** *"Cannot combine armies. The
mercenaries in these armies will not fight together."* The merge sums `+0x168`, the seven
troop counts and the three siege-engine records, takes the **lower** of the two `movesUsed`,
and destroys the absorbed unit.

---

## 3. Foraging and supply

### 3.1 What `g_optArmiesEat` gates

`g_optArmiesEat` (`0x0053F260`) is `L2.eng` group 50 index 2, ***"Army foraging"***, on the
advanced-options screen — so the game's own word for the feature is foraging. Off in the
shipped save. It gates exactly four things:

1. `Ration_Apply` adds the county's troop counts to the food requirement (§3.3a).
2. `Panel_Ration` grows by two rows and prints the troop total.
3. `UnitPanel_Draw` adds the two supply lines and the starvation line (§3.4).
4. `Army_Starve` (`0x004ACE5E`) does anything at all — with the option off it only clears
   every army's starvation counter.

`Army_RecountCountyTroops` runs regardless; its results are simply not read.

### 3.2 Counting the troops in a county

`Army_RecountCountyTroops` (`0x004AD6C0`) zeroes county `+0x198` and `+0x19C` for all 16
counties, then for every unit of type **1 or 2** (armies *and* peasant mobs) that is **not
garrisoned**:

```
county = unit.county
if (county.owner == unit.owner)                    county.friendlyTroops += unit.men
else if (realm[unit.owner].ally == county.owner)   county.friendlyTroops += unit.men
else                                               county.enemyTroops    += unit.men
```

then re-runs `Ration_Apply` for every county. Called from `Army_Create`, `Unit_EnterCounty`
and the garrison path — whenever the distribution changes. [D]

Realm `+0x81` as an alliance flag is **[I]**: it is read here as "an ally's troops eat as
friendlies", and the diplomacy layer was not traced.

### 3.3 Is foraging a straight subtraction? — **No, and the difference matters**

The player's claim was *"it takes from the county's food budget as a straight minus"*. The
effect is close and the mechanism is not, and there are **two separate mechanisms**.

**(a) The county's requirement is inflated.** `Ration_Apply` (`0x0044DF5F`) computes, at four
points in the function,

```c
need = DivCeil(population, g_rationTable[level].div) * g_rationTable[level].mul;
if (g_optArmiesEat) need += county.friendlyTroops + county.enemyTroops;
```

A soldier is **one extra mouth at the county's ration level**, not a fixed subtraction. Four
consequences a flat minus would not have:

* The cost **scales with the ration level**. At *Triple* an army costs three times what it
  costs at *Normal*.
* It comes out of the same pools, split between grain and livestock by the county's `+0x15F`
  slider, dairy first.
* **When the county cannot pay, the ration level falls for everyone.** The level-descent loop
  absorbs the shortfall, so the visible cost of a large garrison is the *peasants'* rations
  dropping — and with them health, happiness and births — before anything happens to the army.
* **Enemy troops eat too**, out of your county's store. `L2.eng` group 62 carries both
  *"Soldiers" / "take"* and *"Foragers" / "steal"* as separate labels.

**(b) The army is fed, or not, by a threshold test.** `Army_Starve` (`0x004ACE5E`), once a
season from `Wages_PayAll`:

```c
if (!g_optArmiesEat)                        unit.starvation = 0;
else if (unit.men < Food_Available(county)) unit.starvation = 0;      /* fed */
else if (unit.garrisonCounty != 0)          unit.starvation = 0;      /* the castle feeds it */
else {
    unit.starvation++;
    if (unit.starvation == 1)       message 0x116;                       /* 278 Unfed troops. */
    else if (unit.starvation < 5) { Army_Desert(unit); message 0x117; }  /* 279 Starving troops. */
    else                          { message 0x118; Army_Destroy(unit); } /* 280 Army perishes. */
}
```

`Food_Available` (`0x0044E7B4`) is `herd*5 + slaughterable*10 + grain*6` — the county's whole
feeding capacity in people-seasons. So the test is **not** "is there food left after the
peasants ate"; it is "is this county's entire larder bigger than this one army". It takes no
account of the peasants, of other armies in the same county, or of what `Ration_Apply` already
spent. An army of 400 in a county with a larder of 3000 is fed no matter how many other armies
stand beside it. [D]

`Army_Desert` (`0x004AD16C`) takes **10 % off each of the seven troop counts** (only where the
count exceeds 10) and subtracts the same total from `+0x168`. The same function is the
bankruptcy penalty in `kingdom.md` §7.4.

> **So the player's model is right about the direction and wrong about the arithmetic.**
> Foraging does come out of the county's food, but as an addition to the *requirement* rather
> than a subtraction from the store; and the army's own survival is decided by a separate
> threshold that ignores everything else in the county. **[V]** on both mechanisms — the five
> starvation strings drawn beside `+0x155` are the second source.

### 3.4 What the panel says about where an army is eating

With foraging on, `UnitPanel_Draw` picks one of `L2.eng` 31/23 … 31/26 from the army's owner
and the county's owner:

| army | county | string |
|---|---|---|
| yours | yours | 23 *"Fed in your county."* |
| yours | not yours | 24 *"Foraging in other lands."* |
| not yours | its own owner's | 25 *"Provisioned on home soil."* |
| not yours | anything else | 24 *"Foraging in other lands."* |

**String 26, *"Foraging in your county."*, is never selected here** — the obvious case, an
enemy army in *your* county, falls into 24 instead. Either it is drawn somewhere untraced or
it is dead text. [D]

### 3.5 Armies are what disables a county's industry

`crates/l2-kingdom`'s `disabled_seasons` had no known writer. **It is `Unit_TrampleTile`
(`0x0046873F`)**, on the settlement tiles an army walks over:

```c
if (type != 3 && type != 4 && county[tileCounty].owner != unit.owner) {
    /* terrain 0..2 -> 3 | 4..5 -> 6 | 7..8 -> 9 | 10..11 -> 12   ("ruined" states) */
    tile.terrain      = ruinedValue;
    industry[c].efficiency      = 0;      /* +0x294 + c*0x18 */
    industry[c].disabledSeasons = 3;      /* +0x296 + c*0x18 */
    unit.movesUsed += 7;
}
```

**It always writes 3**, and only when the army's owner differs from the *county's* owner —
the same guard as field trampling, so again you cannot wreck your own. [D]

**Which record, and which is on the map.** `County_PlaceResourceSites` (`0x00468E61`) runs at
load and scans for tiles of the county whose runtime byte `+2` has `(x & 0x1C) == 0x0C` — the
`Town` graphics bank of `maps-layers.md` §1 — and dispatches on the **plane-2 descriptor
index** at byte `+3`:

| plane 2 | sets `has_resource` at | site tile stored at | terrain written | industry record |
|---:|---|---|---:|---:|
| 30 | `+0x2AD` | `+0x2B0` | 1 | **1** (iron) |
| 0 | `+0x2DD` | `+0x2E0` | 4 | **3** (stone) |
| 20 | `+0x295` | `+0x298` | 10 | **0** (wood) |

`County_PlaceBlacksmith` (`0x0046902A`) then puts record **2** (weapons) on the plain tile
nearest a farm field — the blacksmith is derived, not authored, which is why every county has
one.

**[V] The placer and the trampler agree three for three.** Terrain 1 → the ruin ladder that
writes record 1; terrain 4 → record 3; terrain 10 → record 0; and the fourth ruin ladder
(terrain 7–8 → 9) targets record 2, the one with no map site. Two functions that share no
data path pick the same four records from the same four terrain groups.

So the player is right on both counts: **a county's resources come from the map**, derived at
load from plane 1's bank plus plane 2's descriptor index rather than stored in the county
record; and **an army can make them inactive**, for three seasons, by marching over the site.
`crates/l2-scenario` importing `has_resource` from the save is correct for a save, and there
is now a rule for building it from a map.

The commodity ordering used above is `kingdom.md` §7.4's (0 wood, 1 iron, 2 weapons, 3 stone),
which comes from `Industry_Produce`'s driver arguments; the mapping of plane-2 values to
*specific* commodities is **[I]** on top of that — 20 → wood, 30 → iron, 0 → stone follows
from the record index, not from anything that names the tile.

---

## 4. Sieges on the campaign map

The campaign half of the fourteen siege order handlers `docs/battle-ai.md` could not reach.

`Army_BeginSiege` (`0x004A7CA2` → `0x004A7E0A`) requires the county to have a garrison
(`+0x1BC ≠ 0`), the castle not to be under construction, and the garrison not to be besieged
already. It sets `unit.besiegingCounty`, links the garrison's `+0x19A` back, clears the three
engine records, and — for a human owner — opens **screen `0x1D`, the siege preparation
screen** (`L2.eng` group 83: *"Siege preparations. / Catapults / Siege towers / Battering rams
/ Siege will take / to make ready. / Lift siege / Proceed / — No engines to be built"*).

**Engines are built on the spot, not carried.** `g_siegeEngineWork` (`0x004DE440`) is three
`i32`:

| engine | troop type | man-seasons each |
|---|---:|---:|
| catapult | 7 | **200** |
| siege tower | 8 | **200** |
| battering ram | 9 | **400** |

`Siege_BuildTick` (`0x004A8507`), once a season in turn phase 2, spreads the army's `men`
evenly over the incomplete engine types, spills the remainder onto whichever is still short,
and recomputes each record's `+2` percentage. `Siege_RecomputeBuildTime` (`0x004A80DB`) writes

```
unit.siegeSeasonsLeft = ceil(remainingWork / unit.men)
```

which the screen prints as *"Siege will take N Season(s) to make ready."* An army of 400
building two towers (400 man-seasons) is ready in one season; the same army ordering three
rams (1200) waits three. **The army is pinned for that whole time** — it is in the besieging
state, phase 2 owns it, no move is issued. That is exactly the player's account. As for
cancelling: `L2.eng` 83/6 is *"Lift siege"* and group 10/13 is *"Lift the siege?"*, so **the
button exists**; its handler was not traced.

**The AI does not use the screen.** `Siege_Prepare` (`0x004A7EB5`), for a non-human owner,
hard-codes the order from the lord's personality record (`0x004D8AF8`, stride `0x50`, three
rows a lord):

```
default:                  2 siege towers
personality == 8:         4 siege towers
personality == 9:         1 battering ram
personality == 7:         3 catapults, plus 1 ram if castle type > 3 and season > 2
```

[D] — the personality field is `*(int*)(0x004D8AF8 + (lord*3 − 3) * 0x50)` and was not
identified; `kingdom.md` §8.2 has four AI personalities.

Immediately before the battle, `Army_PrepareForBattle` (`0x004AA6CA`) copies the three engine
counts into `+0x17A/+0x17C/+0x17E` (troop types 7, 8, 9) and, for the **defender**, sets
`+0x180` (**oil**) to **1, 2, 3, 4 or 6** by castle type 1 … 5. That is the campaign-side
answer to where a siege's engines and its boiling oil come from — the defender's oil count is
a plain switch on `kingdom.md` §7.5's castle type. [V]

(The player's description of oil — clicked, falling, leaving fire that kills quickly, and
*attackable* — is consistent with troop 10 being a real unit with a real armour value. The
`40 / 25-when-human` asymmetry in `battle.md` §6.2 stays unexplained; nothing here touches it.)

---

## 5. Mercenaries

They are not a random event and they are not on the merchant screen. **Twelve fixed mercenary
bands wander the map, one per nationality, and you hire whichever is standing in your county
on the turn you raise an army there.**

### 5.1 The roster — six static tables that close arithmetically

`Mercenary_Init` (`0x004AC904`), once from `Game_NewGame`, copies six parallel arrays into the
live band table. Every array is 12 entries indexed 1 … 12, and each ends exactly where the
next begins, with all the alignment padding zero:

| table | address | last byte | element |
|---|---|---|---|
| `g_mercMen` | `0x004DE760` | `0x004DE78F` | i32 |
| `g_mercTroopType` | `0x004DE790` | `0x004DE79B` (+4 pad) | u8 |
| `g_mercStartCounty` | `0x004DE7A0` | `0x004DE7AB` (+4 pad) | u8 |
| `g_mercPeriod` | `0x004DE7B0` | `0x004DE7BB` (+4 pad) | u8 |
| `g_mercPrice` | `0x004DE7C0` | `0x004DE7EF` | i32 |
| `g_mercWage` | `0x004DE7F0` | `0x004DE81F` | i32 |
| `g_mercBandCount` | `0x004DE820` | — | i32, indexed by `g_countyCount` |

**[V]** — three `i32` arrays of exactly 48 bytes and three `u8` arrays of exactly 12 bytes plus
alignment, laid end to end with no gaps and all twelve pad bytes zero. That is this table's
end-offset invariant, and it is why the reading is a finding rather than a guess.

The nationality is `L2.eng` group 16 (*"No mercenaries in the army."*, then Scottish, Irish,
Moorish, Welsh, Danish, Swedish, Flemish, Norman, Saxon, Burgundian, Spanish, Angevin) and the
noun is group 8 at `type*2 + 52`, exactly as the panel draws them:

| # | nationality | men | troop | price | listed wage | start county | period |
|---:|---|---:|---|---:|---:|---:|---:|
| 1 | Scottish | 100 | Pikemen | 1800 | 180 | 1 | 5 |
| 2 | Irish | 200 | Pikemen | 3500 | 350 | 2 | 3 |
| 3 | Moorish | 150 | Archers | 3000 | 300 | 4 | 6 |
| 4 | Welsh | 200 | Archers | 4000 | 400 | 5 | 4 |
| 5 | Danish | 200 | Swordsmen | 5500 | 550 | 7 | 2 |
| 6 | Swedish | 100 | Swordsmen | 2700 | 270 | 8 | 7 |
| 7 | Flemish | 100 | Crossbowmen | 3000 | 300 | 10 | 5 |
| 8 | Norman | 200 | Crossbowmen | 6000 | 600 | 11 | 2 |
| 9 | **Saxon** | **150** | **Macemen** | 1900 | 190 | 13 | 1 |
| 10 | Burgundian | **250** | **Macemen** | 3100 | 310 | 14 | 3 |
| 11 | Spanish | **50** | **Knights** | 2700 | 270 | 16 | 6 |
| 12 | **Angevin** | **100** | **Knights** | 5500 | 550 | 17 | 7 |

Nationalities come in pairs by troop type — a small band and a large one of each — and the
price per man is flat within a type: pikemen ≈ 18, archers 20, swordsmen ≈ 27, crossbowmen 30,
macemen ≈ 12.5, knights ≈ 54.

> **Against the player's prediction.** *"Angevin knights you get like 50 knights"* — the
> Angevin band is **100** knights; the **50**-knight band is the **Spanish** one. *"Saxon
> macemen are like 150 or 200 or 250"* — Saxon is exactly **150**, and the other maceman band
> (Burgundian) is **250**; there is no 200 maceman band. So the shape of the memory is right
> (knights come in 50 and 100, macemen in 150 and 250), one of the two nationality pairings is
> exactly right, and the other names the wrong half of a pair.
>
> **This is a finding, not a fit.** The numbers were read out of the file before they were
> compared to the recollection, and the table is pinned by the layout invariant above rather
> than by resemblance to it. Had the roster been recovered by looking for 50s and 150s it
> would be C3 again.

`g_mercWage` is **price / 10** for every band. It is copied into band `+0x10` and **never read
anywhere**. The raise-army screen prints `men / 2` as the seasonal wage instead, and
`Wages_ForUnit` charges nothing extra for mercenaries at all — they are simply part of
`+0x168`, so they cost `men/4` like everyone else. Three different numbers for the same thing,
of which only one is charged. [D]

The per-man price does **not** track `g_troopStrengthWeight` (`2, 16, 8, 13, 9, 13, 22`):
price ÷ weight comes out 2.0, 1.54, 2.1, 1.875, 1.56, 2.5 across the six types. The prices are
hand-authored. [D]

### 5.2 The live band table, and how an offer appears

`g_mercBands` (`0x00568DC0`), stride `0x14`, index 1 … 12:

| Off | Type | Meaning |
|---|---|---|
| `+0x00` | i16 | the unit index that hired this band; **0 = available** |
| `+0x02` | u8 | its start county (`g_mercStartCounty`) |
| `+0x03` | u8 | the county it is **currently offered in**; 0 = offered nowhere |
| `+0x04` | u8 | the next county in its walk |
| `+0x05` | u8 | troop type |
| `+0x06` | i8 | countdown |
| `+0x07` | i8 | countdown reload (`g_mercPeriod`) |
| `+0x08` | i32 | men |
| `+0x0C` | i32 | price |
| `+0x10` | i32 | the unread `g_mercWage` value |

**How many bands a map gets** is `g_mercBandCount[g_countyCount]`, clamped to 1 … 12:
`0, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 12, …`. England has 14 counties, so **all twelve
bands exist there**. [V]

`Mercenary_AdvanceAll` (`0x004ACA2B`), once a season at end of turn:

```c
for each band not currently hired:
    band.offeredIn = 0;
    band.countdown--;  band.nextCounty++;
    if (band.nextCounty > g_countyCount) band.nextCounty = 1;
    if (band.countdown < 1) {
        band.offeredIn = band.nextCounty;
        band.countdown = band.reload;
        band.nextCounty++;
    }
for each county:
    county.mercOffer /* +0x1AD */ = the first unhired band whose offeredIn == this county, else 0;
```

Each band **walks one county a season** and stops to offer itself every `period` seasons —
Saxon every season, Norman and Danish every other, Swedish and Angevin every seventh. County
`+0x1AD` is a cache and holds only one offer; if two bands land on the same county the
lower-numbered one wins. [D]

### 5.3 Hiring

The offer appears on the **raise-army screen** (`Screen_RaiseArmy`, `0x00418653`, `L2.eng`
group 69), not on a screen of its own:

```
150 Saxon Macemen
1900 crowns to hire.    75 crowns seasonal wages.
Hire mercenaries ?    [Yes] [No]
```

with *"You cannot afford to hire these mercenaries."* when the treasury is short and *"There
are no mercenaries currently available for hire in the county."* when `+0x1AD` is 0.
`Mercenary_Hire` (`0x004AC7F3`) runs from inside `Army_Create`:

```c
unit.mercBand = band;  unit.mercTroopType = bands[band].troopType;
unit.mercMen  = bands[band].men;                 /* a byte */
unit.men     += bands[band].men;
bands[band].hiredBy = unit;  bands[band].offeredIn = 0;
realm.gold   -= bands[band].price;
county.mercOffer = 0;
```

**A mercenary band is atomic.** It is one field triple on the army, it is raised as a single
extra battle unit (`BattleUnit_Create`'s last argument is 1 for it), and:

* two armies **cannot merge if both carry mercenaries** (§2.7);
* `Mercenary_Release` (`0x004AC6FE`) subtracts the band from `+0x168` and frees it for hire
  again — used when the army is destroyed;
* bankruptcy stage 1 walks every mercenary out: `kingdom.md` §7.4's `FUN_004AD230`, `L2.eng`
  160 *"Mercenaries desert!"*, which is the same `Mercenary_Release`.

The roster is identical on every map and every playthrough; only the starting counties and the
walk depend on the map. [V]

---

## 6. Raising an army, and the wages connection

### 6.1 Recruitment is a headcount

`Levy_SetPercent` (`0x00435EBC`), driven by a 0 … 100 slider (`0x0056D65C`):

```c
g_levyMen       = Pct(county.population, pct);
g_levyHappiness = g_armyHappinessCost[pct];
if (pct != 0) g_levyHappiness += county[+0x2F4];       /* a per-county surcharge */
clamp 0 .. 100;
while (county.happiness - g_levyHappiness < 1) { pct--; recompute; }
```

**This is the link `kingdom.md` was missing: `g_armyHappinessCost` (`0x004D8778`) is indexed by
the percentage of the county's population being levied.** 102 entries for 0 … 101; the slider
can only reach 100. The curve is gentle to about 20 % (cost 9), steepens through the 30s and
40s, hits 100 at 60 % and **saturates at 101 from 61 % upward** — which, after the `>100`
clamp, exceeds any possible happiness, so the walk-back loop always fires. In a county at
happiness 100 with no surcharge the largest levy is **59 %** (cost 98). **[V]** — the table was
already verified over all 102 entries; what is new is what indexes it.

County `+0x2F4` is set to **15** by `Army_Create` and added to every subsequent levy in that
county, so raising two armies from one county in quick succession costs far more than one.
Where it decays was not traced. [D]

The player's account is what the code does: **you recruit men, full stop.** There is no
per-troop-type recruitment price anywhere on this path.

### 6.2 Equipping

`Levy_Init` (`0x004AAA80`) builds a per-realm scratch buffer, `g_levyBasket` (`0x0053F6A0`,
stride `0x80`, **8 slots of `0x10` bytes**):

| slot | is |
|---:|---|
| 0 | unequipped men — **peasants** |
| 1 … 6 | the six weapon types |
| 7 | the total |

with `slot[t].+0x00` = chosen, `+0x04` = available, `+0x08` = remaining; slots 1 … 6 are
seeded from realm `+0x140 + (t−1)*4`, the weapon stockpiles. The `+`/`−` buttons on the
armoury screen move one man at a time between slot 0 and slot `t`; the auto-equip path fills
ten at a time round-robin until either the men or a weapon type runs out. The same buffer is
reused, with different field meanings, by the army-split screen (`L2.eng` group 17 *"Army
Division. / Split the army?"*), where row 7 is the mercenary band.

**The alignment is exact, and it cross-checks both tables.** `kingdom.md` §7.4's
`g_weaponCost` order — crossbow, mace, sword, pike, bow, armour, itself verified against two
published tables — lands one for one on `L2.eng` group 8's troop nouns at `type*2 + 52`:

| basket slot | weapon (`g_weaponCost`) | troop noun (group 8) |
|---:|---|---|
| 0 | *(none)* | 52/53 Peasant |
| 1 | crossbow | 54/55 Crossbowman |
| 2 | mace | 56/57 Maceman |
| 3 | sword | 58/59 Swordsman |
| 4 | pike | 60/61 Pikeman |
| 5 | bow | 62/63 Archer |
| 6 | **armour** | 64/65 **Knight** |

Seven for seven, from two sources that know nothing about each other. **A knight is a man in
mail**, and an unequipped levy is a peasant — the pitchfork default the player described. [V]

### 6.3 `Army_Create` (`0x004A9A9A`) end to end

```c
tile = County_FindFreeRoadTile(county) || County_FindFreeOpenTile(county);
Unit_Spawn(1, tile.x, tile.y, realm);
u.isPlayerDriven = 1;  u.needsDestination = 1;  u.ownerIsHuman = realm.isHuman;
u.county = u.homeCounty = county;
u.yearFormed = g_year;  u.morale = county.happiness;
county.population -= levyTotal;   county.army -= levyTotal;     /* +0x24, +0x38 */
u.men = basket[7];  for t in 0..6: u.troops[t] = basket[t];
u.nameIndex = Army_PickName(realm);                             /* 0x004A9F72 */
Levy_ConsumeWeapons(realm);                                     /* 0x004A9EB1 */
if (hireMercs) Mercenary_Hire(unit, county.mercOffer);
Army_RecountCountyTroops();  Wages_ForUnit(unit);
/* re-run the county's food passes twice */
county.happiness -= cost;  county[+0x15] /* shownArmy */ -= cost;
county[+0x2F4] = 15;
realm.wages = Wages_ForRealm(realm);                            /* realm +0xFC */
```

**County `+0x15` is `shownArmy`** — `kingdom.md` §1.1's *"From army"* line in `L2.eng` group
85 — and this is the only place it is written. That closes the loop: forming an army costs
happiness, the cost is `g_armyHappinessCost[levyPercent]` plus the county surcharge, and it
appears on the happiness panel's army row. [V]

`Army_PickName` (`0x004A9F72`) picks the least-used of **24 name slots** at realm `+0x2D` and
adds 2 to its counter, so a name repeats only after every other has been used twice. The names
are `L2.eng` groups **94 … 98**, 24 per lord: *"The Lions." "The Dragons." …* for lord 1, *"The
Invincibles." …* for lord 2, and so on. [V]

Two refusals before creation: **zero men** → message `0xA8` = group 168 *"Your proposed army of
zero men fails to fulfill certain principles of medieval troop management"*; **fewer than 50**
→ `0x94` = group 148 *"impractical to create an army of less than 50 men"*. Both are bypassed
if a mercenary band is being hired, since the band supplies the men. [V]

`Army_Destroy` (`0x004AA039`) reverses the name counter, clears the garrison and siege links,
frees the slot and recomputes the realm's wage bill.

### 6.4 Wages — the missing half

`kingdom.md` §7.4 already has `Wages_ForUnit`: `men/4` for a human owner, `men/3 · /5 · /10` by
difficulty for an AI, **and troop type does not enter it**. What it lacked was anything to pay.
It is this:

* `men` is `+0x168`, the sum the basket wrote in §6.3.
* The result is stored back into **`+0x15C`**, which is the number the army panel prints under
  `L2.eng` 31/8 *"Wages"*. That is the second source that makes the offset **[V]**.
* `Wages_ForRealm` (`0x004AD495`) loops slots 1 … 150 and sums over every unit with
  `owner == realm && type == 1` — garrisoned armies included, besieging armies included,
  mercenaries included in the headcount.
* `Wages_PayAll` calls `Army_Starve` (§3.3b) *before* it charges anyone, so an army can desert
  from hunger and be billed the reduced wage in the same season.

And the player's observation resolves `kingdom.md`'s puzzlement about troop type not
mattering: **a unit is men, and the weapon is a separate stock item already paid for in iron
and wood.** A knight costs the same wage as a peasant because the wage is for the man; the
mail was bought once, at the blacksmith.

---

## 7. How a battle result returns

`docs/mechanics.md` lists this as ❓. It is `Battle_ReturnToCampaign` (`0x004AB383`), with
`g_battleLoser` (`0x0057C924`) naming the defeated side. For whichever of `g_battleArmyA` /
`g_battleArmyB` lost:

* the winner is charged movement — **+8**, or set to `moveAllowance − 1` (one point left) if
  it is human-owned;
* if the loser was a garrison, county `+0x1BC` is cleared and the county changes hands
  (`0x004A72FE`);
* garrison and besieger links (`+0x199`, `+0x19A`) are cleared, and if it was not a siege,
  `Siege_RecomputeBuildTime` runs again for the winner;
* the battle scratch fields `+0x17A … +0x180` are zeroed on both sides (`0x004AA89F`);
* the loser is destroyed — **except** under autocalc, where an army left with **≥ 50 men**
  merely has its siege lifted, while one below 50 gets message `0x120` (group 288) and dies;
* diplomacy takes a **−20** hit against the winner.

Casualties themselves come back through the battle layer writing `+0x16C + t*2` and `+0x168`;
that path was not traced here.

`Army_StrengthScore` (`0x004AB2AA`), used by the AI and the autocalc, is
`Σ troops[t] × g_troopStrengthWeight[t]` plus the mercenary band, `+ 20` if non-zero.

---

## 8. What could not be established

* **Whether an army can destroy its own realm's fields.** §2.6 shows one path where it cannot.
  A negative result from a single `if`; battles fought on farmland were not looked at.
* **`Move_FloodFill` (`0x0046F700`, 2,115 bytes) was not read.** The cost map it consumes and
  the path extraction that follows are both traced; that the fill itself is a cost-weighted
  breadth-first search is **[I]**.
* **The "lift siege" handler.** The button and the confirmation string exist; what they do to
  `+0x199`, `+0x19A` and the engine records was not traced.
* **The AI personality field at `0x004D8AF8`** that chooses siege engines.
* **Realm `+0x81` as an alliance flag** — read that way by the troop recount and nowhere else
  here.
* **County `+0x2F4`**, the levy surcharge: written as 15, never seen to decay.
* **Unit `+0x152`, `+0x19B`, `+0x1A`,** and the untraced offsets in §1.5.
* **`L2.eng` 31/26** *"Foraging in your county."* has no reachable caller in the panel.
* **The fourth write in each `Unit_TrampleTile` branch**, `industryRecord + 0x18`, which is the
  *next* record's first field. That is what the code does and it is not explained.
* **Nothing here was observed in a running game.** Every claim comes from the binary, its
  static tables and `L2.eng`. The falsifiable predictions worth checking in play: *moves left*
  starts at 15 and a road step costs 1 while open ground costs 3; the army sprite changes at
  301 and 601 men; two armies totalling 1501 refuse to merge; a Spanish band is 50 knights for
  2700 crowns and a Saxon band is 150 macemen for 1900; marching an army over an enemy county's
  mine shuts it down for three seasons.
