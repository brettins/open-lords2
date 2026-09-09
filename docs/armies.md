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
| minimum army | **50** men to raise | [V] `L2.eng` 148 *"impractical to create an army of less than 50 men"* |
| ~~below 30 an army is destroyed~~ | **wrong — that rule is the peasant mob's** | [V] §2.1a: the only `men < 30` test in the binary is in `PeasantMob_Tick`, and `Army_Tick` has none |
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
| `+0x152` | u8 | **mergeTarget** | [V] | the unit to merge into on arrival — §8.5. Not an "order mode". |
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

  > **Corrected, and the phase now reads end to end.** `Siege_StartPhase` does *not*
  > "re-prepare every besieging army" as its own `symbols.json` comment said — it never
  > calls `Siege_Prepare`. It runs `Siege_ValidateLink` (`0x004A8426`) over every besieging
  > army, which **clears `+0x199` when the besieged county has lost its garrison, or when
  > that garrison is no longer garrisoned in that county**, and counts the survivors into
  > `g_siegeCount`. Then it seeds `g_siegeCursor` with 1.
  >
  > `Siege_TickPhase` (`0x004A84BA`) is the pump: it walks that cursor 1 → 150 calling
  > `Siege_BuildTick`, and **returns as soon as one army's engines are ready, leaving the
  > cursor where it is** so the turn machine can run the assault and resume. The assault is
  > `Siege_LaunchAssault` (`0x004A8AAB`). Phase 2 is therefore a three-function state
  > machine — validate, build, assault — and all three are now named. [V]
* **Phase 7, end of season**, calls three things, and **the order is
  `Mercenary_AdvanceAll(); Units_ResetMoves(); Move_BuildCostMap();`** — three consecutive
  statements at `00490000.c:4644`. `Units_ResetMoves` (`0x004651B9`) writes `+0x14C = 0` and
  `+0x153 = 0` for all 150 slots, unconditionally: it does not skip garrisons, besiegers or
  free slots, and it does not touch the allowance. [V]

  > **Corrected.** An earlier revision of this line gave the order as *"`Units_ResetMoves`
  > … then `Move_BuildCostMap` and `Mercenary_AdvanceAll`"*, which is backwards. The
  > mercenaries walk **before** the armies get their moves back. It matters because the
  > order the season's passes run in is the specification (`docs/kingdom.md` §3.4) and two
  > lockstep peers have to agree on it; `crates/l2-kingdom`'s `SEASON_PIPELINE` now carries
  > both, flagged as phase-7 work rather than `Season_Advance`'s.
  >
  > `Move_BuildCostMap` is **not** once a season either. It has four callers, and one of
  > them is `Unit_OrderMove` — so the cost map is rebuilt on *every move order*, which is
  > why a tile trampled two steps ago is already impassable to the next one. The "once a
  > season" note on it in `symbols.json` should go. [D]

Player-ordered movement happens during phase 4 (the players' turn); `Units_Tick`
(`0x004650B0`) is driven from the frame loop and dispatches each unit through
`g_unitTickTable[type]` (`0x004D6A50`). **Slot 5 of that table is NULL** while the dispatcher
accepts types up to 5, so a type-5 unit would call address 0. Nothing spawns one. [D]

### 2.1a One array, four things — which handler is whose

The document's headline is that armies, peasant mobs, merchants and transports share
`g_units`. This is where they stop sharing. All four tick handlers are now named, and the
table is short enough to be the answer:

| type | handler | allowance | sprite | walk table | on crossing a border |
|---:|---|---:|---|---|---|
| 0 | `Unit_TickNone` `0x00465214` | — | — | — | dead: `Units_Tick` clears kind 0 before it indexes |
| **1** army | `Army_Tick` `0x0046521F` | **15** | `0x48`/`0x60`/`0x78` by men, `+3×facing` | `g_unitWalkFrames` (3) | `Unit_EnterCounty` **and** a troop recount |
| **2** mob | `PeasantMob_Tick` `0x00465486` | 10 | `0x90 + 3×facing` | `g_unitWalkFrames` (3) | a troop recount, then `FUN_004ABD0F` |
| **3** merchant | `Merchant_Tick` `0x00465622` | 10 | `6×facing` | `g_merchantWalkFrames` (6) | **nothing** |
| **4** transport | `Transport_Tick` `0x00465761` | 10 | `6×facing` | `g_merchantWalkFrames` (6) | **nothing** |
| 5 | NULL | | | | |

Four things fall out, and three of them change something:

* **Types 3 and 4 are the same function twice.** 319 bytes each, statement for statement
  identical. A transport ticks exactly like a merchant, and a reimplementation that shares
  one routine between them is faithful.
* **`Army_Tick` is the only handler that fires a border event.** Merchants and transports
  assign the county byte and move on — no greeting, no invasion message, no recount. The mob
  recounts but does not greet.
* **The "below 30 men an army is destroyed" rule is not an army rule.** `PeasantMob_Tick`
  opens with `if (men < 30) destroy`, and `men < 0x1E` appears **exactly once** in the whole
  2,452-function corpus. `Army_Tick` has no such test, and `L2.eng` 148 — which §0 cited for
  both halves of that row — only ever says *"impractical to create an army of less than 50
  men"*. §0 is corrected.
* **The merchant sprite is a six-frame cycle**, `g_merchantWalkFrames` (`0x004D6AB8`) =
  `{0,1,2,3,4,5}`, against the army's three-frame ping-pong. `((facing+1) & 7) × 6 + phase`
  covers `0 … 47` with nothing left over, and the three merchants in the battle fixtures sit
  at frames **42, 42 and 18** for facings 6, 6 and 2 — which is that formula, exactly, three
  for three. **[V]** on data the game wrote.

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
| **county town** (`0x40`) | 5 → `Transport_Deliver` **and `Army_AttackCounty`**, move ends | — (the attack charges +8) | 100 |
| settlement (`0x80`) | 6 → `Unit_TrampleTile` adds 7, move ends | **+7, conditionally** | 100 |
| dwelling plot (`0x10`) | 7 → `Unit_BurnDwelling`, move ends | **+7, conditionally** | 100 |
| occupied | `Unit_EnterOccupiedTile` — §2.7 | varies | — |

> **Three corrections to this table.**
>
> 1. **The `0x40` row is the important one, and it was half the story.** The mover's code-5
>    branch calls `Transport_Deliver` *and* `FUN_004A6C68` — see §9 — so a `0x40` tile is
>    not expensive terrain the pathfinder routes around, it is **the objective**. Stepping
>    onto a county's **town** is how a county is taken.
>
>    *(Renamed after `decisions.md` C25: bit `0x40` is the county town, bit `0x80` is the
>    castle. This row said "castle" throughout and the words have been changed, not the
>    behaviour. The game says it plainly — `L2.eng` group 30 index `0x1B`, "Your troops may
>    capture a castleless county by attacking its county town.")*
> 2. **The dwelling-plot row is not free.** `Unit_BurnDwelling` (`0x00468AE2`) charges
>    `+0x153 += 7`, exactly like trampling. It fires only when the unit is not a merchant or
>    transport, the county's owner differs from the unit's, **and** the terrain byte is
>    `0x10`.
> 3. **The settlement `+7` is conditional twice over.** It is charged *inside* each of
>    `Unit_TrampleTile`'s four ruin branches, so an already-ruined site costs nothing and a
>    site in your own county costs nothing; and `Unit_Step` only calls the trampler at all
>    when the terrain byte is below `0x10`. Terrain `0x15`…`0x19` routes to `FUN_004686A0`
>    instead, which is garrison-if-yours / begin-siege-if-not, and costs nothing. [D]
>
> Also worth stating because a reader will assume otherwise: `Unit_StepOnce` **clears the
> road flag at the top of every step** and sets it again only if the tile just classified as
> a road. It is per-step, not sticky, so an army that walks off a road pays 3 on the very
> next tile. And codes 5, 6, 7 and 10 return *before* both the charge and the position
> update, so the unit never enters those tiles — anything it is charged comes from the
> handler. [D]

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
if (Move_ExtractPath(0, destX, destY)) {   /* 0x004701AC */
    Path_CopyToUnit(0, unit);              /* 0x004707BE */
    unit.stepTargetX = unit.destX = destX;
    unit.stepTargetY = unit.destY = destY;
    unit.moveState = 2;
    unit.orderMode  = mode;
    unit.orderFlags = 1, 1, 1;             /* +0x156, +0x157, +0x158 */
}
```

> **Corrected: every write is inside the `if`.** An earlier revision of this block put the
> destination and `moveState = 2` after it as unconditional statements. A failed extraction
> leaves the unit exactly as it was — no destination, not moving, its previous path intact.
>
> That is *not* the same as "an unreachable destination does nothing": `Move_ExtractPath`
> returns **success with a zero-length path** when the destination was never reached, so
> that order is accepted, `moveState` becomes 2, and the army stands still. Only a dead end
> in the descent returns 0. [D]

There are **two distance fields**: `g_moveDistLocal` (`0x00500C30`) for the local player and
`g_moveDistOther` (`0x004F0380`) for everyone else, each 64×64 `i16`. The first argument is
not an index — it is a realm id compared against `g_localPlayer`, and every call site but one
passes the literal 0, so when the human is realm 0 the AI's searches land in the local field
too. `Move_ExtractPath` is a greedy descent down whichever it is given, appending `(x, y)`
pairs into `g_pathBuf` (`0x00553550`, stride 300) with the length in `g_pathLen`
(`0x0053F070`). On a road tile (cost 1) it steps the direction index by 2 — orthogonals only
— and falls back to all eight only when that finds nothing: **confirmed**, verbatim. [D]

Three further facts about the extractor, none of them previously recorded:

* its direction order is **clockwise from north** — N, NE, E, SE, S, SW, W, NW — which is
  *not* the fill's order, and it is the extractor's that decides ties;
* the running best is seeded with `dist[cur]` itself and the test is a strict `<`, so only a
  strictly cheaper neighbour is a candidate and **the lowest direction index wins a tie**;
* the buffer is stored **destination-first** and the source is never appended, while
  `Unit_Step` decrements the length before reading — storage reversed, consumption reversed,
  net forward.

**`Move_ExtractPath` has no length cap and `Unit_OrderMove` does not check one.** The AI
tests `g_pathLen < 0x96` before copying; the player's path does not, so a path over 150 steps
writes past `g_pathBuf`'s 300-byte slot and stores a length the unit's array cannot hold.
That is a buffer overrun rather than a rule and `crates/l2-kingdom` clamps it. [D]

#### `Move_FloodFill` — read, and it is not a breadth-first search

§8 listed this function as *"not read"* and its behaviour as **[I]**. It has been read
(`00460000.c`, `0x0046F700`), and the inference was wrong in the detail that matters.

**It is SPFA — a FIFO-queue Bellman–Ford with full relaxation.**

```text
dist[] = 0;  dist[start] = 1;  queue = [start]
while queue not empty:
    cur  = pop
    dirs = N, E, S, W   and, only if cost[cur] != 1,  NE, SE, SW, NW
    for nbr in dirs:
        c = cost[nbr];  if (mode != 0 && c > 1) c = 100
        if (c == 0) continue                            /* impassable */
        if (dist[nbr] == 0 || dist[cur] + c < dist[nbr]):
            dist[nbr] = dist[cur] + c;  push nbr
```

Six things follow, and every one of them is a place a reasonable reimplementation goes
wrong. All **[D]**.

* **The start cell is seeded with 1, not 0**, because 0 is the unvisited sentinel. Every
  stored distance is therefore `true cost + 1` — which is what `Map_DrawPathMarker`'s
  `distance - 1` is undoing. It is not an off-by-one correction.
* **The cost charged is the cost of the tile being *entered*.** The start tile's own terrain
  is never paid for.
* **A cell is re-relaxed and re-queued when a cheaper route arrives later.** This is not an
  optimisation: a FIFO queue over weights of 1, 3, 6 and 100 does not produce distances in
  sorted order, so without the relaxation branch the field would simply be wrong and the
  greedy descent in §2.3 would be unsound. It is also the sharpest contrast with the
  *battlefield* pathfinder, where the first cost written to a cell stands forever
  (`docs/decisions.md` C12).
* **Cost 0 is impassable and there is no separate blocked mask.** An impassable cell keeps
  `dist == 0`, which is indistinguishable from unreached — deliberately, since neither can
  be walked to.
* **A road tile expands orthogonally only.** `if (cost[cur] != 1)` gates the four diagonals,
  reading the *raw* cost so it holds in both modes. §2.3 records the extractor's half of
  this rule; this is the fill's half, and it is why a tile diagonally off the end of a road
  costs 13 rather than 11. Off a road a diagonal costs **exactly** what an orthogonal step
  costs — no √2, no scaling — so armies prefer diagonals everywhere they are allowed.
* **There is no goal test and no budget.** The fill always covers the whole reachable
  component, however near the destination is.

The queue is a **circular buffer of 1,024 `int` entries that wraps**, so a fill that
outgrows it silently overwrites its own queue and stops with a half-filled field. Its head
and tail cursors are *globals shared by both distance fields* even though the buffers are
per-field. `crates/l2-kingdom` reproduces the capacity and the wrap for the same reason
`l2-sim` reproduces the battle queue's: a search that would have overrun must produce the
original's result.

**The fourth argument is a road preference, and only the AI uses it.** `mode != 0` rewrites
every cost above 1 to 100, so roads become a hundred times cheaper than anything else. The
AI tries `mode = 1` first and falls back to `mode = 0` when the road route comes back at 150
steps or more; `Unit_OrderMove` — every human-ordered move — always passes 0. **AI armies
road-hug and the player's do not**, which is a visible behavioural difference nothing had
recorded.

**There is no bounds checking anywhere in the fill or the extractor.** It is flat index
arithmetic on 4,096 cells, so stepping east from `x = 63` wraps into the next row, and
expanding a tile in row 0 writes *before* the array — into, among other things, the fill's
own queue head cursor. It survives only because every shipped map has an impassable sea
border. `crates/l2-kingdom` rejects out-of-grid neighbours instead and says so: reproducing
the wrap would let a path teleport across the map edge, and reproducing the underflow is not
reproducible behaviour at all. **[I]** that the border is always impassable in practice.

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
troop counts and the three siege-engine records, takes the **higher** of the two `movesUsed`,
and destroys the absorbed unit.

> **Corrected, and it reversed a rule.** An earlier revision of this line said *"the lower of
> the two `movesUsed`"*, which reads as a refund. The body is
>
> ```c
> if ((char)movesUsed[from] < (char)movesUsed[into]) v = movesUsed[into];
> else                                               v = movesUsed[from];
> movesUsed[into] = v;
> ```
>
> — **both arms select the maximum.** Merging a fresh army into a spent one leaves the
> result spent, so a reinforcement can never buy back movement. That is the opposite
> tactical rule from the one the document gave, and it was found by reading the branch
> rather than the summary of it. `docs/decisions.md` C13, again. [D]

---

## 3. Foraging and supply

### 3.1 What `g_optArmiesEat` gates

`g_optArmiesEat` (`0x0053F260`) is `L2.eng` group 50 index 2, ***"Army foraging"***, on the
advanced-options screen — so the game's own word for the feature is foraging. Off in the
England turn-one save. It gates exactly four things:

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

### 4a. The assault, and the rule that makes engines worth building

`Siege_LaunchAssault` (`0x004A8AAB`) is what `Siege_TickPhase` yields to when an army's
engines come in. It works out **which castle is actually being fought**, which is not simply
`castleType`:

```c
if (county.castleDegraded == 1 && county.castleBuilding != 0) level = castleBuilding - 1;
else if (county.castleDegraded == 2)                          level = county[+0x1F9];
else                                                          level = castleType - 1;
```

then sums the three engine counts at `unit + 0x182 + e*6` — the same records §4 describes,
and the offset closes: `0x52F232 − 0x52F0B0 = 0x182`, stride 6, three of them — and applies
**one gate**:

> **`level < 3 || engines > 0`.** Otherwise message `0x119` and the siege is lifted.

`L2.eng` **281** is *"Cannot siege castle" / "Your captains advise that you must build some
siege engines to besiege this castle."* — which is the gate in the game's own words, and is
why this is **[V]** rather than a plausible reading of a comparison. So the first two castle
levels can be stormed bare-handed and everything above a Norman keep cannot; and a besieger
that orders no engines at all against a big castle does not stall, it **gives up** — the
refusal calls `Siege_Break`.

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

**How many bands a map gets** is `g_mercBandCount[g_countyCount]`:
`0, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 12, …`, **twenty entries** — the length is held
by address arithmetic, `0x004DE820 + 20 × 4 = 0x004DE870`, where the string table begins.
England has 14 counties, so **all twelve bands exist there**. [V]

> **Corrected: "clamped to 1 … 12" is wrong at both ends.**
>
> ```c
> n = g_mercBandCount[g_countyCount];
> if (0xc < n) n = 1;      /* above twelve collapses to ONE, not to twelve */
> if (n < 0)   n = 1;      /* and zero is not < 0, so zero stays zero      */
> ```
>
> With the shipped table neither branch can fire, so nothing in the original behaves
> differently — but a modded county-count table hits both, and "clamped" is exactly the
> plausible-sounding description that hides what the code does. `symbols.json` says the same
> thing and should be corrected with it. [D]

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
g_levyMen       = Pct(county.population, pct);         /* Pct(x,p) = (p*x)/100, truncating */
g_levyHappiness = g_armyHappinessCost[pct];
if (pct != 0) g_levyHappiness += county[+0x2F4];       /* a per-county surcharge */
if (100 < g_levyHappiness) g_levyHappiness = 100;      /* the COST is clamped, after the  */
if (g_levyHappiness < 0)   g_levyHappiness = 0;        /* surcharge - not the percentage  */
if ((char)county.happiness < 1) { g_levyMen = 0; g_levyHappiness = 0; }
else while ((char)county.happiness - g_levyHappiness < 1) { pct--; recompute; }
```

> **Three corrections, all [V].**
>
> 1. An earlier revision wrote the clamp as a bare `clamp 0 .. 100;`, which reads as if the
>    *percentage* were clamped. **It is the happiness cost, and it is clamped after the
>    surcharge is added.** The percentage is clamped 0…100 by the slider handler
>    `FUN_00435CEF`, never here.
> 2. It **omitted the `happiness < 1` early-out**, which is the walk-back's only termination
>    guard. Without it a county at zero happiness walks `pct` negative and indexes
>    `g_armyHappinessCost[-1]` forever.
> 3. `pct` is a **by-value parameter**: the walk-back does not write back to `g_levyPercent`.
>    The slider stays where the player put it while the two globals hold the reduced figures,
>    so the screen can read 80% while the county is giving up 59%.
>
> It writes exactly two globals and nothing else: `g_levyMen` (`0x00543FD8`) and
> `g_levyHappinessCost` (`0x00565400`). The slider is `g_levyPercent` (`0x0056D65C`), an
> `int` clamped 0…100, drawn at pixel `x = pct + 196` on a 101-pixel track — which closes
> exactly, and is [V].

**This is the link `kingdom.md` was missing: `g_armyHappinessCost` (`0x004D8778`) is indexed by
the percentage of the county's population being levied.** 102 entries for 0 … 101; the slider
can only reach 100. The curve is gentle to 19 % (cost 9), steepens through the 20s, 30s and
early 40s, hits 100 at 60 % and **saturates at 101 from 61 % upward**. In a county at
happiness 100 with no surcharge the largest levy is **59 %**, and it costs **99**. **[V]** —
the table was already verified over all 102 entries; what is new is what indexes it.

> **Two small corrections.** An earlier revision said *"gentle to about 20 % (cost 9)"* —
> index 18 and 19 are 9 and index 20 is 10 — and gave the largest levy's cost as **98**; the
> table's index 58 is 98 and index 59 is **99**, and the loop needs `happiness − cost ≥ 1`,
> so 99 is exactly affordable at happiness 100 and 100 is not.
>
> Its *reasoning* about the saturation was also wrong even though the conclusion survives.
> It said 101 *"exceeds any possible happiness, so the walk-back loop always fires"*. 101 is
> clamped to 100 first; what fires the loop is `100 − 100 = 0 < 1`.

County `+0x2F4` is set to **15** by `Army_Create` and added to every subsequent levy in that
county, so raising two armies from one county in quick succession costs far more than one.

> **Where it decays is now traced.** `Happiness_UpdateAll` (`0x0044BAEA`) runs
> `if (county[+0x2F4] != 0) county[+0x2F4] -= 5;` over counties 1…`g_countyCount` at the top
> of its own pass, so the 15 is **gone after three seasons**. The same function is what
> zeroes `+0x15` (`shownArmy`) each season, which is why §6.3's write to it is a
> single-season display value. [D]

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
armoury screen move one man at a time between slot 0 and slot `t`, and **never touch slot 7**,
which is why the levy total is what `Army_Create` writes into `+0x168`. The same buffer is
reused, with different field meanings, by the army-split screen (`L2.eng` group 17 *"Army
Division. / Split the army?"*), where row 7 is the mercenary band.

> **`Levy_Init` is the AI's function, and the player's screen uses a different one.** Its
> only callers are the three AI army-raising helpers (`FUN_004A5003`, `FUN_004A50AE`,
> `FUN_004A5389`). The **player** path is `FUN_004AA90A` (`0x004AA90A`), called from
> `Sidebar_Button` as `FUN_004aa90a(g_selectedCounty, g_levyMen)` — byte-for-byte the same
> seeding plus three UI resets. The seeding is identical, so nothing in the *rules* changes;
> what changes is that a mod hook attached to one of them would apply to only half the
> armies in the game. Both read the realm index out of **`county[+0x05]`, the county's
> owner**, rather than taking a realm argument — which is how a neutral county's militia
> ends up seeded from realm 0. [V]
>
> There is a **fourth field at `+0x0C`** that neither seeding function clears: a "chosen
> count when this slot was last selected" latch that `FUN_004AABD8` tests to fire the
> troop-portrait animation. Presentation, not a rule. [D]

**The auto-equip, corrected.** §6.2 said it fills *"ten at a time round-robin until either
the men or a weapon type runs out"*. The second half is wrong:

```c
men = slot[7].chosen;  pass = 0;  changed = true;
while (pass < 50 && changed) {
    changed = false;
    for (t = 1; t < 7; t++) {
        if (men < 10) goto done;                 /* the floor is 10, not 0 */
        if (slot[t].remaining >= 10) {
            slot[t].chosen += 10;  slot[t].remaining -= 10;
            slot[0].chosen -= 10;  men -= 10;  changed = true;
        }
    }
    pass++;
}
```

A weapon type that runs out is **skipped on every later pass** and the round-robin continues
with the others, so an army is armed from whatever the armoury still has rather than stopping
at the first empty rack. There is a **50-pass ceiling** — six types × ten men × fifty passes
is 3,000, twice `ARMY_MAX_MEN`, so it never bites in play — and the floor is `men < 10`, so
**up to nine men are always left as peasants**. [V]

A second, unrelated helper exists: `FUN_004A545F(realm, t)` fills one type to the maximum in
a single move, called by `FUN_004A5389` in the fixed order **5, 1, 3, 2, 6** — archers,
crossbows, swords, maces, armour, with pikes skipped. [D]

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

> **Four corrections to the pseudocode above, all [V].**
>
> 1. **`morale` is the county's happiness *before* the levy cost is deducted.** It is the
>    fifth write after the spawn; the debit is nearly last. A county at 80 that pays 30 for
>    its army still gives it morale 80. The listing above had them the other way round.
> 2. **The happiness debit is clamped**, and the panel is debited what was actually taken:
>
>    ```c
>    if ((char)county.happiness < happinessCost) { county[+0x15] -= county.happiness;
>                                                  county.happiness = 0; }
>    else                                        { county.happiness -= happinessCost;
>                                                  county[+0x15]   -= happinessCost; }
>    ```
>
>    This is reachable, because the cost is a **parameter**, not a global: the AI paths pass
>    an unclamped, surcharge-free figure straight out of the table.
> 3. **The two refusals are not in `Army_Create`.** They are in the confirm handler
>    `FUN_00435B4D`, and **there is a third**: message `0xDD` = group 221, raised when
>    `Army_Create` returns 0 because the county has no free road tile *and* no free open
>    tile to stand the army on. Only the first two are bypassed by a mercenary hire.
> 4. **`Army_Create` sets neither `moveAllowance` nor `movesUsed`.** `Unit_Spawn` memsets
>    the whole `0x1A4`-byte record, so a fresh army carries an allowance of **0** until
>    `Army_Tick` writes 15 on the next frame.
>
> Two omissions rather than errors: the `realm == 0` branch writes `u.owner = 6` and
> `u.shield = 0` — which is where §1.1's *"6 marks an ownerless unit"* comes from, and it is
> how a **neutral county's militia** is created; and `Levy_DebitPopulation` (`0x004A9F18`),
> which is where the `+0x24` / `+0x38` debit actually lives, indexes the basket by
> `county[+0x05]` while `Army_Create` itself uses its `realm` argument. They agree in play.

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

`docs/mechanics.md` listed this as ❓. **The whole path is now implemented** — the campaign
half in `crates/l2-kingdom/src/battle.rs`, the hand-off to the simulation in
`crates/l2-game/src/engagement.rs` — and it is checked end to end against the battle fixture
triple in `crates/l2-game/tests/seam.rs`. This section is rewritten from the four functions
that make it up rather than from `Battle_ReturnToCampaign` alone, because
`Battle_ReturnToCampaign` is only the third of them.

```
Battle_ChooseSettlement  0x004A6A30   §7.1  autocalc, a report, or ask the player
Battle_AutoResolve       0x004AAD07   §7.2  a battle nobody watches
Battle_CheckOutcome      0x00477DFC   §7.3  when a fought battle is over
Battle_WriteBackCasualties 0x0047F474 §7.3  the figures become troop counts again
Battle_ReturnToCampaign  0x004AB383   §7.4  the county, the moves, the loser
Defence_Disband          0x004ABA5A   §7.5  the levy walks home
```

> ### ⚠ `g_battleLoser` (`0x0057C924`) holds the **winner**. The name is inverted.
>
> This is the single most dangerous error in this document, because everything below was
> written on the name and the name is wrong. **Four** independent sites agree against it,
> two of them found since this warning was written:
>
> * **`Battle_AutoResolve`.** When `Army_StrengthScore(A) < Army_StrengthScore(B)`
>   — A is the weaker side, so A loses — it sets **`g_battleLoser = g_battleArmyB`** and
>   applies the survival percentage to **B**'s seven troop counts, zeroing A's.
> * **`Battle_ReturnToCampaign`.** The `g_battleLoser == g_battleArmyA` branch
>   hands the county to **A** via `County_ChangeOwner(A.owner, county)`, charges **A** the
>   winner's movement, and calls `Army_Destroy(g_battleArmyB)`. It also runs
>   `Diplo_Offend(B.owner, A.owner, 20)` — the *loser's* owner resenting the *winner*.
> * **`Battle_CheckOutcome`** (`0x00477DFC`). `if (menA < 1) g_battleLoser = g_battleArmyB;`
>   — the side still standing is the one assigned.
> * **`Battle_SelectOutcomeBanner`** (`0x00478419`). `g_battleWinnerOwner` is
>   `g_units[g_battleLoser].owner`, and this maps `g_localPlayer == g_battleWinnerOwner`
>   onto `L2.eng` group 82's ***"won"*** pair.
>
> All four only make sense if the variable names the side that **won**. Implementing §7 on
> the name as written destroys the winner and hands the county to the corpse. `[V]`
> `crates/l2-kingdom`'s `battle::Verdict` has no field called `loser` beside a field called
> `winner` and no way to fill them in the wrong order, which is the shape this correction
> asks for.

### 7.1 Which of three ways it is settled — `Battle_ChooseSettlement` (`0x004A6A30`)

Called by all three battle entries — `Army_AttackCounty`, the army-versus-army path
`FUN_004A7158`, and `Siege_LaunchAssault` — and every one of them branches on the same two
expressions:

```c
if (A.ownerIsHuman == 0 && B.ownerIsHuman == 0)   return 0;   /* autocalc, silently  */
...
if (g_optFightHumansOnly == 0 && !bothHuman)      /* autocalc, then report screen 0x13 */
else                                              /* prompt screen 0x12, ask          */
```

**A battle between two AI realms never reaches a screen and never reaches the battle
simulation at all**, which is why the autocalc and everything downstream of it lives in
`l2-kingdom` rather than beside `l2-sim`: an AI war has to be fightable with no battle
layer present. `g_optFightHumansOnly` is the advanced option *"Fight humans only?"* and is
**stored inverted** — the byte is 0 when the option displays *Yes*.

There is no distance-from-the-view test, no army-size cut-off and no separate "quick
battle" toggle on this path. The mid-battle *"Autocalc battle?"* button (`0x0043BD67`,
`L2.eng` group 10 index 9) is a fourth entry rather than a fourth outcome: it re-runs
`Battle_AutoResolve` on the counts as they stand, which the battle layer has not yet
written back. `[V]`

**And declining the prompt is exactly the autocalc.** `Battle_Decline` (`0x0043B622`) runs
`Battle_AutoResolve`, `Battle_ReturnToCampaign(0)` and raises the report screen. Saying no
to *"Will you take the field?"* is a way out of *watching* the battle, not out of fighting
it. In single player the prompt waits for ever; in multiplayer a 20- or 30-second timeout
auto-declines. `[V]`

### 7.2 The autocalc — `Battle_AutoResolve` (`0x004AAD07`)

Not in `symbols.json` at all until now, despite being one of only two ways a battle can
end.

```c
sA = Army_StrengthScore(A);  sB = Army_StrengthScore(B);
if (siege) sB = Pct(sB, [160,200,250,320,400][castleLevel]);   /* the DEFENDER only */
ratio = (sA < sB) ? PctOf(sB, sA) : PctOf(sA, sB);             /* larger*100/smaller */
p     = Table_Lookup(ratio, g_autocalcSurvivalLadder, 10, 100);
winner = (sA < sB) ? B : A;                                    /* a tie goes to A */
for t in 0..7: winner.troops[t] = Pct(winner.troops[t], p);
winner.merc.men = Pct(winner.merc.men, p);
winner.men = Σ winner.troops + winner.merc.men;                /* summed, not scaled */
loser.troops[..] = 0;  release the loser's band;  loser.men = 0;
```

**`g_autocalcSurvivalLadder` (`0x004DE710`)** is ten `(threshold, value)` int32 pairs, read
straight out of `Lords2.exe`:

| ratio under | 110 | 130 | 160 | 180 | 220 | 270 | 360 | 500 | 700 | 900 | else |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **winner keeps** | 10 % | 20 % | 30 % | 40 % | 50 % | 65 % | 80 % | 90 % | 95 % | 98 % | 100 % |

`[V]` — the thresholds ascend strictly and the eleventh pair in memory is `(100, 200)`, a
different table starting, which is what fixes the count at ten.

**The shape of that ladder is a rule about the game, not a detail.** An evenly matched
fight leaves the winner **a tenth of its army**: mutual annihilation is the *default*
outcome of auto-resolving a fair battle, and the curve only turns generous once one side is
roughly triple the other. A player who declines a close fight has traded both armies.

> **The autocalc reproduces the battle fixture exactly, and this is the strongest check
> anything in this document has.** `battle-before.sav`'s attacker is 128 peasants, 25
> swordsmen and 25 archers — `128×2 + 25×13 + 25×13 + 20 = 926`. County 3's militia is 122
> peasants and 60 archers — `122×2 + 60×13 + 20 = 1044`. So the defender wins;
> `1044 × 100 / 926 = 112`; 112 lands on the ladder's second rung, 20 %; and 20 % of 122 and
> of 60 is **24 + 12 = 36 men**. `battle-during.sav` leaves county 3 with 546 people and
> `battle-after.sav` has 582. `546 + 36 = 582`. Four independent numbers — the strength
> weights, the `+20`, the ratio and the ladder — all have to be right for that to come out.
> `[V]`

### 7.3 When a fought battle ends — `Battle_CheckOutcome` (`0x00477DFC`)

**This function was in neither `symbols.json` nor `hypotheses.json`, and it is the half of
the battle model that decides anything.** A field battle ends exactly two ways:

* one side's living-men counter reaching zero — `g_battleMenA` (`0x0053F028`) and
  `g_battleMenB` (`0x00553C58`), recomputed every frame over **troop types 0…6 only**, so
  siege engines are worth no men and a side reduced to them has already lost;
* `g_battleWithdrawal` (`0x0056D5C8`) marking a side as having left the field, which is
  tested **before** either counter and so outranks annihilation.

There is **no morale break, no rout threshold and no clock.** And the two counters are the
two numbers `Ui_DrawNumberRight` puts on the battle HUD, so the numbers that decide a
battle are the numbers the player is looking at. `[V]`

Three further arms are sieges and are out of scope here: the escape-tile flag
`DAT_00553F3C`; *assault repulsed, repeat*, where a castle under level 3 with no breach and
no engines left has its breach and approach scores reset to 4 and the battle carries on;
and the same position at level 3 or above, where the besieger loses.

The outcome then goes to screen `0x2B` and one of **seven** `L2.eng` group 82 heading/body
pairs, chosen by `Battle_SelectOutcomeBanner` (`0x00478419`) from two questions — was it a
siege, and is the local player the winner:

| | local player won | local player lost |
|---|---|---|
| field | 0 *won* | 1 *lost* |
| siege, A (the besieger) won | 2 *siege won* | 5 *castle lost* |
| siege, B (the garrison) won | 4 *siege lifted* | 3 *siege lost* |

The seventh pair, 12/13 *"The conflict is over."*, is somebody else's war. Each of the four
siege arms reads as a different sentence, and that is the check on the mapping: A is always
the besieger and B always the garrison, so no two of them are interchangeable.

Once the banner has been up for 5,000 ticks, `Battle_WriteBackCasualties` (`0x0047F474`)
runs and *then* `Battle_ReturnToCampaign(1)`. The write-back is the answer to what this
section used to call *"that path was not traced here"*: both records' `+0x168`, `+0x196` and
all eleven `+0x16C` counts are zeroed, and every living figure of the 80 adds its men back
into its owner's record — into `+0x16C + t*2` for a levied figure and `+0x196` for a
mercenary one. **Only types 0…6 are rebuilt**, so siege engines never survive a battle.

### 7.4 `Battle_ReturnToCampaign` (`0x004AB383`)

The two branches are **not** mirror images, and the differences are the content of the
function:

| | **attacker (A) wins** | **defender (B) wins** |
|---|---|---|
| county changes hands | yes, if the loser was a garrison **or** carried `+0x167` | **never** |
| county `+0x1BC` cleared | if the *defender* was a garrison | if the *attacker* was a garrison |
| winner's moves | `+8`, then an **AI** is set to `allowance − 1` | an **AI** pays `+7`; a human pays nothing |
| loser | destroyed | destroyed |
| diplomacy | −20 from the loser's realm | −20 from the loser's realm |

```c
/* A wins */                             /* B wins */
movesUsed[A] += 8;                       if (B.ownerIsHuman == 0)
if (A.ownerIsHuman == 0)                     movesUsed[B] += 7;
    movesUsed[A] = moveAllowance[A] - 1;
```

`+0x01` is a copy of realm `+0x05` = `isHuman`, so `== 0` is **the AI**. A winning AI is
left with one move; a winning human keeps everything but the 8. And when B wins, an AI pays
7 and **a human pays nothing at all**. `[V]`

**There is no `County_ChangeOwner` anywhere in the B-wins branch.** That is not an omission
in the reading — `g_battleArmyB` is the *defender* at all three call sites, so a defender
that wins keeps a county it already had, or leaves a neutral county neutral. It is the half
of this section that was inverted, and `crates/l2-game/tests/seam.rs` asserts it against
`battle-after.sav`, where the player lost and county 3 is still owner 0.

Also, and easy to miss: `Army_AttackCounty` has *already* charged the attacker 8 moves
before the battle, so **a winning attacker pays 16** and is finished for the season either
way.

The rest:

* garrison and besieger links (`+0x199`, `+0x19A`) are cleared, and if it was not a siege,
  `Siege_RecomputeBuildTime` runs again for the winner;
* the battle scratch fields `+0x17A … +0x180` are zeroed on both sides (`0x004AA89F`);
* the loser is destroyed;
* diplomacy takes a **−20** hit against the winner — but the guard is `loser.owner != 0`,
  which an **ownerless militia's 6 passes**, so the original then indexes a five-realm table
  with 6. `crates/l2-kingdom` refuses instead of reproducing that write.

> ### ⚠ The ≥ 50-men rule is **not** an autocalc rule, and this document said it was.
>
> §7 used to read *"the loser is destroyed — **except** under autocalc, where an army left
> with ≥ 50 men merely has its siege lifted"*. `docs/symbols.md` carried the same sentence.
> Both are wrong. The gate is `g_battleWithdrawal` (`0x0056D5C8`), and
> **`Battle_AutoResolve`'s first statement clears it**. The flag is raised in exactly one
> place — `UnitOrder_SiegeAttKnight`, when an all-knight AI besieger faces an unbreached
> wall and gives up — so it is a *siege-withdrawal* rule, reachable only after a real
> interactive siege, and **under autocalc the loser is always destroyed**. The message
> `0x120` (group 288) for a loser under 50 men belongs to the same branch. `[V]` — the one
> write and the three clears are the only four sites the flag has. See `decisions.md` C30.

### 7.5 The levy walks home — `Defence_Disband` (`0x004ABA5A`)

Runs **after** `Battle_ReturnToCampaign`: immediately after it on the silent path, and as
the last line of the report screen otherwise.

```c
if (unit.defenceMark == 0) return;
if (unit.defenceMark < 2) {
    county[unit.homeCounty].population += unit.menTotal;
    county[unit.homeCounty].popArmy    += unit.menTotal;
    Army_Destroy(unit);
} else {
    unit.defenceMark = 0;
}
```

**Every survivor, not a fraction**, and into the population *and* the panel's *"Army"* line,
exactly as the levy debited both. Mark 2 — an army that already existed and was pressed
into defending — is not disbanded at all; it only loses the mark.

Running it after the return is what makes it correct in both directions: a defence that
*lost* has already been destroyed and this finds nothing, and a defence that *won* is still
standing with its survivors in `+0x168`. §7.2's 36 men are the ones this walks home.

`Armies_ReturnDefences` (`0x004AD39A`) is the turn-end backstop, sweeping every army slot
before the wage pass.

`Army_StrengthScore` (`0x004AB2AA`), used by the AI and the autocalc, is
`Σ troops[t] × g_troopStrengthWeight[t]` plus the mercenary band, floored at 1 and `+ 20`
above that.

---

## 8. Taking a county — the hole in the middle of this document

Everything above traces the record, the levy, movement, supply, sieges and how a battle
*result* comes back, and never names the thing they exist for: **what makes an army take a
county.** Three functions, none of them mentioned anywhere else here.

### 8.1 `Army_AttackCounty` (`FUN_004A6C68`, `0x004A6C68`)

Called from the mover's **code-5 branch** — see §2.2 — with `g_movingUnit` and the tile's
county.

> ### ⚠ It is the county **town**, not the castle tile. C25, one more time.
>
> Code 5 comes from `Unit_TryEnterTile`'s plane-0 **`0x40`** branch, and `decisions.md` C25
> established that `0x40` is the county town and `0x80` the castle — the labels were the
> wrong way round. This sentence was written before that, kept the old label, and has been
> saying "castle tile" ever since. **No code is wrong**: `crates/l2-kingdom` reads the same
> bit under the name `flags::CASTLE`, and `movement::Step::reached_castle` really does fire
> on the town. Only the words are.
>
> The game says it in English. `L2.eng` group 30 description `0x1B`, the tile info panel's
> own text for a `0x40` tile: ***"Your troops may capture a castleless county by attacking
> its county town."***
>
> And the battle fixture agrees on the bytes. In `battle-before.sav` the player's army is
> standing **on** county 3's `+0x74`/`+0x75` — its castle tile, (33, 17) — with no battle
> and no moves spent. In `battle-during.sav` it has walked to (35, 16), beside the town
> whose anchor is (37, 16), and the battle has started. Standing on the castle site did
> nothing; approaching the town is what attacked the county. `[V]`

```c
if (unit.type == 1 && unit.owner != 0 && county.owner != unit.owner
    && (county[+0x1C0] == 0                       /* no castle          */
        || county[+0x1BC] == 0                    /* no garrison        */
        || garrison.owner == unit.owner)) {       /* or it is yours     */
    unit.movesUsed += 8;
    unit.needsDestination = 1;
    g_battleCounty = county;  g_battleIsSiege = 0;

    if (county.owner == 0) {                                  /* neutral  */
        if (county.happiness < 11) defender = 0;               /* surrender */
        else defender = County_RaiseDefence(county, 25|40|50|60 by difficulty, 40, mode 2);
    } else if (!realm[county.owner].isHuman) {                /* AI       */
        defender = FindDefender(county) or County_RaiseDefence(county, 40, 40, mode 1);
    } else {                                                  /* human    */
        defender = FindDefender(county) or County_RaiseDefence(county, 40, 40, mode 0);
    }
    if (defender) defender[+0x167] = (raised ? 1 : 2);
    if (defender == 0) County_ChangeOwner(unit.owner, county);
    else               ...set up the battle...
}
```

**That single `if` is the siege gate.** A county with *both* a castle and a garrison that is
not yours cannot be walked into at all; everything else is a battle or an outright capture.
It is one condition, not a subsystem.

**`+0x167` is the county-defence marker**, which §1.5 lists among the offsets *"not traced"*.
It is written here — 1 for a defence raised on the spot, 2 for an existing army pressed into
the role — and read by `Battle_ReturnToCampaign`, whose A-wins branch captures the county when
`B[+0x167] != 0`, and by `Defence_Disband` (§7.5), which sends a **1** home and merely clears a
**2**. `battle-during.sav` slot 6 carries a 1, so this is `[V]` on the data side as well.

**The `if (defender) defender[+0x167] = (raised ? 1 : 2)` above is a simplification, and the
real thing is asymmetric.** The `2` is written on the **AI** branch only. A human's county
defended by an army that was already standing at its town is **never marked at all**, so
`Defence_Disband` never touches it — which reaches the same end as the 2 would by writing
nothing, and is the sort of accident that looks like a rule until both branches are read
side by side. `[V]`

*(Six merchant records in `lastturn.sav` carry their own county in `+0x167`. That is a
different meaning for a different unit type, like `+0x14F` and `+0x164`, not a contradiction —
but it is **[I]** which of the two is the field's "real" purpose.)*

### 8.2 `County_RaiseDefence` (`FUN_004A50AE`)

Levies a percentage of the county's population and equips it by **who owns the county**:

| mode | county | equipped |
|---:|---|---|
| 0 | a **human**'s | **nothing at all** — the defence is entirely peasants |
| 1 | an **AI**'s | the auto-equip round-robin, out of that realm's real stockpiles |
| 2 | **neutral** | 500 each of pikes, bows and maces are granted to realm 0, then a size ladder |

The neutral ladder, from its nested `if`: **≥ 480 men → 150 archers, 100 pikemen, 50 macemen;
≥ 360 → 100, 70; ≥ 240 → 80, 40; ≥ 120 → 60; below 120 → all peasants.** The happiness cost is
forced to zero — nobody owns the county to be angry at — and `Army_Create(0, …)` gives the
result owner byte 6.

**A county with fewer than 40 people raises nothing**, and is then captured outright. [D]

### 8.2a `County_FindDefendingArmy` (`0x0046D42C`) — read, and both halves of the guess were wrong

§9 listed this as *"modelled as the lowest-numbered army of the county's owner standing in the
county. The function itself was not read: **[I]**."* It has been read.

```c
uint County_FindDefendingArmy(int county) {
    ax = county.anchorX;  ay = county.anchorY;          /* +0x6C / +0x6D */
    if (ax - 2 < 0 || ax + 2 > 64 || ay - 2 < 0 || ay + 2 > 64) return 0;
    best = 0;  bestMen = 0;
    for (y = ay - 2; y < ay + 2; y++)
      for (x = ax - 2; x < ax + 2; x++) {
        u = g_tiles[y*64 + x].unit;                     /* the +5 occupancy plane */
        if (u && units[u].owner == county.owner && units[u].kind == 1
              && units[u].men > bestMen) { best = u; bestMen = units[u].men; }
      }
    return best;
}
```

**Two things were wrong, and they are different kinds of wrong.**

* **The scope.** It is not "in the county". It is a **4×4 tile block** around the county's
  anchor, read out of the tile occupancy plane. An army three tiles from the town does not
  defend it, however deep inside the county it stands.
* **The tie-break.** It is not slot order. It is the **largest** army, by `+0x168`.

**[V] on the arithmetic**, which closes exactly: the scan advances `+8` per column and
`+0x1E0` to the next row, and `512 − 4×8 = 480 = 0x1E0`, so the block is four wide and four
tall and nothing else fits.

**And the asymmetric window is the tell that the reading is right.** `−2 … +1` looks like an
off-by-one until you know the county town is a **2×2 block whose bottom-right corner is the
anchor** (§10.3). With that, the window is exactly *the town, plus the one-tile ring around
it* — a rule you can state in a sentence: **an army defends its county town by standing on it
or beside it.**

The two readings **disagree on shipped data**, which is what makes this a correction rather
than a preference. In `battle-before.sav` county 2's town anchor is (31, 50) and its owner's
only army stands at (30, 46) — the county's own **castle** tile (§8b.3), four rows north of
the town. The old reading returns that army; the original returns 0 and the county levies a
fresh defence.

`crates/l2-kingdom`'s `conquest::find_defender` **has been corrected to this function**, and
both readings are run against those bytes in `crates/l2-kingdom/tests/defence.rs`, which
asserts that they return different answers. The geometry — the `−2 … +1` window, the
size tie-break and the edge guard — is in the unit tests beside the function.

### 8.3 `County_ChangeOwner` (`FUN_004A72FE`)

```c
realm[new].countyCount++;
...one of nine messages, 0x72..0x7E, by how many counties the taker now holds...
county.owner = new;
penalty = realm[new].isHuman ? (difficulty * 20 + 10) : 30;
if (county.happiness < penalty) { county[+0x17] -= happiness; happiness = 0; }
else                            { happiness -= penalty; county[+0x17] -= penalty; }
county[+0x07] = realm[new].shield;
realm[new].peakCounties = max(peakCounties, countyCount);
```

Two things worth naming. **The conquest penalty is drawn on the *"From events"* line**
(`+0x17`, `L2.eng` group 85 index 9) rather than on the army row, so a newly taken county
shows its resentment where a plague would. And **an AI conqueror always costs 30 while a
human's cost tracks the difficulty** — 10 at Easy, 30 at Normal, 50 at Hard — so the setting
decides whether conquest is cheaper for the player than for the AI, and at Normal they are
equal. [D]

### 8.5 Ordering a move is six decisions, and `L2.eng` group 10 names all of them

Everything above is what happens when an army *arrives*. This is what happens when the player
*asks*, and it was the largest unread branch in the subsystem. It came apart in one step
because of a single observation:

> **`L2.eng` group 10 is a directory of the game's confirmable actions**, and
> `Ui_OpenConfirm(prompt, x, y, onYes)` (`0x0040E6F2`) takes that group's index as its first
> argument at every call site.

```
10/0 Exit the game?      10/1 Start a new game?   10/2 Overwrite File?
10/3 Create this army?   10/4 Slaughter villagers?  10/5 Combine armies?
10/6 Disband army?       10/7 Garrison castle?      10/8 Besiege castle?
10/9 Autocalc battle?    10/10 Destroy field?       10/11 Surrender castle?
10/12 Retreat from field? 10/13 Lift the siege?     10/14 Quit? (no destination).
```

So a callback passed to index 7 is the garrison callback — **by the text the player reads**,
not by inference. Thirteen literal call sites cover indices 0, 1 and 4 … 14.

**`Map_HoverUnitTarget` collects six targets; `Map_ConfirmMoveOrder` (`0x004A9252`) turns at
most one of them into a question.** The two functions are a matched pair — the hover writes
exactly the six globals the confirmer reads, and zeroes exactly those six when the path costs
more than the unit has left:

| tile under the cursor | condition | global | dialog | callback |
|---|---|---|---|---|
| county town (`0x40`) | county ≠ unit's | `g_hoverCountyTownCounty` | — | (guard only) |
| dwelling plot (`0x10`) | county ≠ unit's | `g_hoverVillageCounty` | 4 *Slaughter villagers?* | `MoveOrder_Confirm` |
| sown field (`0x20` **and** `County_TileIsField`) | county ≠ unit's | `g_hoverFieldCounty` | 10 *Destroy field?* | `MoveOrder_Confirm` |
| another of your units, not a transport | — | `g_hoverMergeUnit` | 5 *Combine armies?* | `MoveOrder_ConfirmCombine` |
| castle (`0x80`, terrain > `0x14`) | county **=** unit's | `g_hoverGarrisonCounty` | 7 *Garrison castle?* | `MoveOrder_ConfirmGarrison` |
| castle (`0x80`, terrain > `0x14`) | county ≠ unit's | `g_hoverSiegeCounty` | 8 *Besiege castle?* | `MoveOrder_ConfirmSiege` |

The last two rows are **the two arms of one owner test**, and it is the same test
`Unit_ReachCastleBuilding` (`0x004686A0`) makes when an army actually gets there — yours →
`Army_Garrison`, theirs → `Army_BeginSiege`. Two unrelated functions, one for the intention
and one for the act, splitting on `0x80 && terrain > 0x14` identically. That is what promotes
§2.2's third correction from **[D]** to **[V]**.

**Each callback then re-checks, and eleven `L2.eng` strings pin the branches one for one.**
This is the densest external anchoring in the document, so it is worth listing in full:

| callback | branch | msg | `L2.eng` |
|---|---|---:|---|
| garrison | castle under construction | `0xA4` | 164 *"This castle is undergoing construction work…"* |
| garrison | county `+0x1C2` set | `0xA5` | 165 *"You cannot station men in a **ruined** castle."* |
| garrison | cap − garrison < 1 | `0x11B` | 283 *"Castle fully barracked."* |
| garrison | cap − garrison < your men | `0xA6` | 166 *"…does not have the capacity… **Do you want to split your army?**"* |
| garrison | both carry mercenaries | `0xA7` | 167 *"The mercenaries in these armies will not fight together."* |
| garrison | the garrison is besieged | `0x121` | 289 *"…As it is currently under siege !!"* |
| siege | no garrison, castle intact | `0x11D` | 285 *"This castle is deserted my liege. Your enemies await you in the county town."* |
| siege | no garrison, castle degraded | `0x11C` | 284 *"This castle is under construction my liege."* |
| siege | already besieged | `0x113` | 275 *"…other troops already lay siege to this castle. You must **join with or dispose of** the existing siegers."* |
| combine | combined men ≥ 1501 | `0x112` | 274 *"…over their recommended limit of **1500** troops."* |
| plain | destination town's castle is garrisoned | `0x11E` | 286 *"This shire contains a garrisoned castle my lord. We must lay siege to that…"* |

Five of those are worth more than a row in a table:

* **165 names county `+0x1C2`**: it is the *castle ruined* flag. `Army_BeginSiege` refuses on
  the same field, silently.
* **274 is the third independent sighting of 1500**, after `Army_Combine`'s `0x5DD` and the
  cap in `symbols.json`. And it fires *before the order is issued* — the game will not even
  walk you over.
* **275 is `Army_Combine`'s siege-link migration, described in English.** Merging into a
  besieging army makes the survivor the besieger and repoints the garrison's `+0x19A`; the
  string calls that "join with … the existing siegers".
* **286 is `Army_AttackCounty`'s siege gate seen from the player's side.** §8.1 derived that
  gate from one `if`; here is the game explaining it.
* **166 describes a mechanic nothing in this knowledge base has recorded**: group 166 also
  carries *"Your army contains"*, *"more troops can be stationed here."* and *"Do you want to
  split your army?"*, so an army too big for the castle is offered a **split**. Recorded as
  **[I]** in `hypotheses.json` (H5) — the strings are certain, the handler was not read.

**`MoveOrder_ConfirmCombine` is what settles unit `+0x152`.** It is the only caller that
passes a non-zero fifth argument to `Unit_OrderMove`, and what it passes is
`g_hoverMergeUnit`. So `+0x152` is **the unit to merge into on arrival**, not the abstract
"order mode" §1.2 called it.

**Every one of these actions exists twice.** Under `g_multiplayer` the callback sends
a network command instead of acting: `0x29` a move order, `0x2C` a move-and-combine, `0x2E` a
disband, `0x34` a garrison, `0x35` a begin-siege, `0x3A` an industry toggle from the map. See
§8c — the payload of `0x29` is the whole input a lockstep peer needs.

### 8.6 Lifting a siege, and disbanding

Both were open in §9. Both are one function each.

**`Siege_Break` (`0x0043B917`)** clears the besieger's `+0x199` and the garrison's `+0x19A`.
Three callers, and together they are the whole rule:

1. **`Unit_OrderMove`, on every successful type-1 order.** Giving a besieging army anywhere
   to go lifts its siege. `Panel_MoveButton` asks `10/13 "Lift the siege?"` first — but the
   confirmation is only a warning; `Map_BeginMoveSelection`, its yes-callback, does not touch
   `+0x199`. The break happens later, when the order actually takes. **If the path extraction
   fails, the siege survives.**
2. **`Siege_LaunchAssault`**, when the castle is too strong to assault without engines (§4a).
3. **The siege screen's own "Lift siege" button** (`L2.eng` 83/6), directly.

From the map you never see the prompt: `Map_Click` runs `Siege_ValidateLink` first and opens
the **siege preparation screen** rather than a move order if the link is still good.

**`Army_Disband` (`0x00438681`)** releases any mercenary band, returns `troops[1…6]` to the
realm's `weapons[0…5]` — troop type `t` → weapon slot `t−1`, the same off-by-one `Levy_Init`
uses in the other direction — and returns the men to the county's population, its
`labour[8].workers` and its `popArmy`. **Which county** is the interesting part, and the game
states the rule itself: `L2.eng` **145**, *"Your army must disband to its county of origin. If
you no longer rule the county, the army must then disband inside a county that you do rule."*
`Panel_DisbandButton` implements exactly that, clause for clause, and refuses with 145 when
neither holds. [V]

### 8.7 What the shipped position means for all of this

In `lastturn.sav` **every county has `garrisonUnit = 0`**, so at turn one nothing on the map
is behind the siege gate; and **every neutral county sits at happiness 77**, well above the
surrender threshold of 11, so **every neutral capture is a battle and none is a walk-in**.
The surrender path only opens once a county has been taxed or starved into misery — which is
the *"the people are wretched, my liege"* county of §2.5's greeting table, and the same
threshold band.

---

## 8c. Every campaign action already has a wire format

*Properly `netcode.md`'s subject; recorded here because it was found by reading the
campaign-map callbacks, and because it is what a lockstep move order actually is.*

Every one of §8.5's callbacks ends in the same shape:

```c
if (g_multiplayer == 0) Unit_OrderMove(...);
else                            Net_SendCommand(0x29, 0);
```

**`Net_SendCommand` (`0x0043EDA0`)** is the sender, and it is driven by two tables that can be
read straight out of the file:

* **`g_netCmdWriters` (`0x004D57F0`)** — 112 function pointers, one payload serialiser per
  opcode.
* **`g_netCmdLength` (`0x004D5B90`)** — `u8[112]`, the payload length, `0xFF` for "not a
  command". Valid opcodes run **`0x01 … 0x61`**; `0x00` and `0x62 … 0x6F` are all `0xFF`.

**Two invariants close, and both could have failed.** All 112 pointers are **real function
starts** in the decompiled corpus — zero misses. And opcode `0x29`'s writer emits
`1 + 4 + 1 + 2 + 2` bytes through `Net_WriteField`, against a table that says **10**.

The writers also come in **matched pairs**: the reader for opcode *N* is the next function in
address order, using `Net_ReadField` where its twin used `Net_WriteField`, and ending where
`g_netCmdWriters[N+1]` begins. So a command's payload is readable without running anything.

**A campaign move order on the wire is:**

```
op 0x29, 10 bytes:  player (1)  dest (4)  unit (1)  x (2)  y (2)
```

and the campaign-map opcodes are:

| op | len | action |
|---:|---:|---|
| `0x29` | 10 | move order |
| `0x2C` | 24 | move order that merges into a unit on arrival |
| `0x2E` | ? | disband army |
| `0x34` | 12 | garrison castle |
| `0x35` | 12 | begin siege |
| `0x3A` | 2 | toggle an industry from the map (`Map_Click`) |

Sends are once for eleven listed opcodes and **three times**, with flag bits `0x40` then
`0x80`, for everything else — a redundancy scheme, not a retry. The sequence counter wraps
`1 … 39` over forty `0x104`-byte slots.

**Why this matters to `crates/l2-kingdom` rather than to a network layer:** the opcode set is
the original's own answer to *"what is an action?"*, and the payload is its answer to *"what
does an action need to be replayed?"* — 97 of them, enumerated, with lengths. Neither
question had a source before.

## 8b. There is an oracle for an army now

§9's largest disclaimer said *"there is no oracle for an army, and there cannot be one from
the shipped save"* — `lastturn.sav` holds six units and all six are merchants. That was true
of `lastturn.sav` and is no longer true of the fixture set. **The battle triple
(`battle-before/during/after.sav`) contains real armies**, and every army-only offset it
touches can now be checked against bytes the game itself wrote.

Read them with a `g_units` dump through the save-block table — `g_units` is `0x0052F0B0`,
stride `0x1A4`, and it *is* in the table, as is `g_tiles`.

### 8b.1 What is in them

| slot | before | during | after |
|---|---|---|---|
| 1, 2, 3 | merchants, unchanged | | |
| **4** | realm 2, **garrisoned in county 2**, 95 archers | 146 men, mixed | 146 men |
| **5** | realm 1, the human's army, 178 at (33, 17) | at (35, 16), `movesUsed` 10 | **gone** |
| **6** | — | **the raised defence**: owner 6, 182 men, `+0x167 = 1`, `moveAllowance` 0 | **gone** |

### 8b.2 It also settles the type byte, which is now worth saying out loud

`anchor.js`'s `litNum` mis-parsed C character escapes — `'\b'` came back as `0x62`, the letter
— and the decompiler writes small enumerated bytes exactly that way, so
`g_units[i].kind == '\x03'` is the shape most at risk. **The type byte is on disk in all three
fixtures** and needs no tool at all:

```
slots 1, 2, 3 : kind 3, owner 6, moveAllowance 10   (the three merchants)
slots 4, 5    : kind 1, owner 2 / 1, allowance 15   (two armies)
slot  6       : kind 1, owner 6, allowance 0        (the raised defence, +0x167 = 1)
```

So **1 = army and 3 = merchant** are data, not a decompiler reading; the ten-versus-fifteen
move allowance of §1.2 splits along the same line; and any future claim about a `kind`
comparison can be checked here in seconds. (Nothing in this revision came through `anchor.js`
— the literals here were read from the corpus text and from `Lords2.exe` directly — but the
check is cheap and the fixture is the right place to end the argument.)

### 8b.2a Five more things it settles, each of which could have failed

1. **`County_RaiseDefence`'s percentage is exact.** County 3's population is **728**; slot 6
   has **182** men; `728 × 25 % = 182` to the man, and 25 is the `g_optDifficulty == 0` rung
   of §8.1's ladder. **[V]**
2. **The neutral equipment ladder is exact.** §8.2 read the ladder from nested `if`s as
   *"≥ 120 → 60 archers, the rest peasants"*. Slot 6 is **122 peasants + 60 archers = 182**.
   The 60 lands in troop slot 5, which is the archer, and the peasant remainder is what the
   basket has left after `−0x3C`. **[V]**
3. **A levy debits population one for one.** County 2's population falls **639 → 588** across
   the same pair while its garrison grows **95 → 146**. Both deltas are **51**. **[V]**, and
   it is `Levy_DebitPopulation` doing it.
4. **`+0x167` is the county-defence marker, and `Defence_Disband` returns the survivors.**
   §8.1 called both **[D]**. Slot 6 carries `+0x167 = 1`, the *raised on the spot* value; and
   county 3's population goes **728 → 582**, which is `728 − 182 + 36`. So thirty-six of the
   defenders lived and went home. `Defence_Disband` (`0x004ABA5A`) is the function that puts
   them there: mark 1 → men back into `population` and `popArmy`, unit destroyed; mark 2 → the
   mark is simply cleared and the army stays. **[V]**
5. **`+0x1A` is the garrison feedback code.** §1.5 guessed from the code that it takes 2 and 5
   on the garrison path. Slot 4, the garrison, carries **5** — the success value — and stands
   at exactly county 2's `+0x74`/`+0x75`. **[V]**

### 8b.3 A county has three tiles, not one

The fixture makes a distinction the document had blurred:

| field | what it is | county 2 | tile there |
|---|---|---|---|
| `+0x70` | i32 tile offset of the **top-left** of the 2×2 county town | (30, 49) | |
| `+0x6C`/`+0x6D` | the **anchor** — that block's **bottom-right** corner | (31, 50) | flags `0x40` |
| `+0x74`/`+0x75` | the **castle** tile | (30, 46) | flags `0x80`, terrain `0x15` |

The top-left/bottom-right relation holds on **all four occupied counties** of the fixture —
(14,37)/(15,38), (30,49)/(31,50), (36,15)/(37,16), (48,41)/(49,42). That is what makes
`County_FindDefendingArmy`'s `−2 … +1` window read as *the town plus its ring* rather than as
an off-by-one (§8.2a).

And county 3, whose `castleType` is **0**, has terrain **`0x14`** on its `+0x74`/`+0x75` tile —
the bare plot `County_FindCastleTile` stamps at load — while county 2, `castleType` **1**, has
**`0x15`**. Five terrain values `0x15 … 0x19` for five castle types is the obvious reading and
only two of the five are witnessed; it is **[I]** in `hypotheses.json` (H6).

### 8b.4 One thing it refutes, and one it explains

**Slot 6 has `moveAllowance` 0 and `spriteFrame` 0**, while the two ordinary armies beside it
have 15 and a correct sprite. §1.2 says the allowance is *"**15** for an army"*. Both are
written unconditionally at the top of `Army_Tick`, so the honest statement is that **15 is a
tick-maintained invariant, not an initial value**, and an army created mid-turn is briefly
outside it. Anything reproducing `Army_Create` must not assume the field starts at 15.

**The sprite formula checks out twice and misses by one on the third.** Slots 4 and 5 in
`battle-before` both have facing 1 and `spriteFrame` **78**, and `0x48 + 3 × ((1+1) & 7) + 0`
is 78. Slot 5 in `battle-during` has facing 2, `walkPhase` 0 and `spriteFrame` **82**, where
the formula gives 81. The difference is exactly one, and `1` is a value `g_unitWalkFrames`
(`{0, 1, 2, 1}`, `i32`, not bytes) actually contains — `Army_Tick` writes `+0x07` and *then*
calls `Unit_Step`, which advances `+0x1B`, so the saved sprite is one phase behind the saved
phase. That is a fit, not a finding, and it is **[I]** in `hypotheses.json` (H3).

---

## 9. What could not be established

* **Whether an army can destroy its own realm's fields.** §2.6 shows one path where it cannot.
  A negative result from a single `if`; battles fought on farmland were not looked at.
* ~~**`Move_FloodFill` was not read.**~~ **Closed** — it is read, in §2.3, and the guess it
  was a cost-weighted breadth-first search was wrong: it is SPFA, with full relaxation.
* ~~**The "lift siege" handler.**~~ **Closed** — `Siege_Break` (`0x0043B917`), three callers,
  §8.6. It clears `+0x199` and `+0x19A` and leaves the engine records alone.
* **The AI personality field at `0x004D8AF8`** that chooses siege engines.
* **Realm `+0x81` as an alliance flag** — read that way by the troop recount and nowhere else
  here.
* ~~**County `+0x2F4`**, the levy surcharge: never seen to decay.~~ **Closed** — it decays by
  5 a season in `Happiness_UpdateAll`; see §6.1.
* ~~**Unit `+0x167`**, in §1.5's untraced list.~~ **Closed** — it is the county-defence
  marker; see §8.1.
* ~~**Unit `+0x152`**~~ **Closed** — it is the merge-target unit index, §8.5.
  ~~**`+0x1A`**~~ **Closed** — the garrison feedback code, 5 on success, witnessed in the
  fixture (§8b.2). **`+0x19B`** and the rest of §1.5's untraced offsets stand.
* ~~**`FUN_0046D42C`**, the existing-defender search §8.1 calls.~~ **Closed, and the guess
  was wrong in both halves** — it is `County_FindDefendingArmy`, a 4×4 scan around the county
  town returning the *largest* army, not a slot-order scan of the whole county. §8.2a.
* **`+0x12`/`+0x13` are truncated `u8`, not `i8`** as §1.2 types them —
  `((x << 4) & 0xFF, (y << 4) & 0xFF)` on all six merchant records in the shipped save. Not a
  rule anything reads, but the typing is wrong and a reader would infer a sign that is not
  there.
* **`L2.eng` 31/26** *"Foraging in your county."* has no reachable caller in the panel.
* **The fourth write in each `Unit_TrampleTile` branch**, `industryRecord + 0x18`, which is the
  *next* record's first field. That is what the code does and it is not explained.
* ~~**There is no oracle for an army, and there cannot be one from the shipped save.**~~
  **Closed by the fixture set, not by the shipped save** — see §8b, which uses the battle
  triple to turn five of this document's `[D]` readings into `[V]`. The paragraph below is
  kept because its *argument* is still correct and still worth reading: with no second
  source, a plausible reading has nothing to fail against.

  `lastturn.sav`'s `g_units` block holds **six occupied slots and all six are merchants**
  (type 3, owner 6). Not one army. So every army-only offset — `+0x16C` the troop counts,
  `+0x15C` the wage, `+0x166` the morale, `+0x154` the allowance, `+0x182…` the siege
  records, `+0x195…` the mercenary triple — has **no data-side verification available at
  all**, and this whole document is code-only in exactly the region a conquest slice depends
  on. The merchants do verify the shared half: `+0x0C = (y*64 + x) * 8` reproduces on all
  six, and so does the `+0x12`/`+0x13` pixel pair.

  That is why the corrections above matter more than they would elsewhere. With no second
  source, a plausible reading of decompiler output has nothing to fail against, which is
  `docs/decisions.md` C3's exact shape — and three of the corrections in this revision
  (`Army_Combine`'s maximum, `g_battleLoser`'s inversion, the post-battle human/AI test)
  were errors of that kind that had survived precisely because nothing could contradict them.
* **Nothing here was observed in a running game.** Every claim comes from the binary, its
  static tables and `L2.eng`. The falsifiable predictions worth checking in play: *moves left*
  starts at 15 and a road step costs 1 while open ground costs 3; the army sprite changes at
  301 and 601 men; two armies totalling 1501 refuse to merge; a Spanish band is 50 knights for
  2700 crowns and a Saxon band is 150 macemen for 1900; marching an army over an enemy county's
  mine shuts it down for three seasons.
