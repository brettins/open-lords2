# The battle simulation

The real-time tactical battle in `Lords2.exe`: the structures, where they live, and how
damage, movement and orders are computed.

This is the first document about game *rules* rather than a file format, and rules are
much harder to validate — there is no end-offset invariant to close. Read the status
legend as a real claim about evidence:

* **[V] verified** — read directly out of the binary and cross-checked against a second,
  independent source: the game's own debug labels, an exact invariant over the shipped
  data, a `L2.eng` string, or a statement in the printed manual.
* **[D] decompiler-only** — a straightforward reading of decompiled C with no second
  source. Almost certainly right about *what the code does*; the *name* may be wrong.
* **[I] inferred** — consistent with everything measured, not proven.

`docs/decisions.md` C3 is the failure mode to avoid: three functions matched three
storage modes, the numbers lined up, and the story was wrong. Nothing below is asserted
because it "makes sense".

Addresses are from the GOG Windows build (1,031,680 bytes, `ImageBase 0x400000`, no
ASLR), and every name used here is in [`symbols.json`](symbols.json) and applied to the
Ghidra database.

---

## 0. The headline

**A battle is two arrays of 80 records at fixed addresses.**

| | Address | Stride | Count | What it is |
|---|---|---|---|---|
| **units** | `0x00566520` | `0x34` | 1 … 80 | what the player selects, orders and sees a banner for |
| **figures** | `0x00554480` | `0x1B0` | 1 … 80 | the drawn men; each stands for `g_menPerFigure` real soldiers |
| battlefield | `0x005440E0` | `8` | 80 × 80 | one cell |
| missiles | `0x0057A100` | `0x4C` | 1 … 100 | arrows, bolts, shot, fire, falling men |

**[V]** All four counts are loop bounds in the binary: `BattleMen_ClearAll` and
`BattleUnits_ClearAll` both run `for (i = 1; i < 0x51; i++)`, `Missile_UpdateAll` runs
`i < 0x65`, and the battlefield is the `for (y < 0x50) for (x < 0x50)` nest that
`Battlefield_BuildFromSkr` fills. Index 0 is never used in any of them; it is the "none"
value, and a zero owner byte marks a free slot.

There is no allocation and no dynamic sizing anywhere in the battle. **An army that needs
more than the free slots is silently truncated** — `BattleMan_Create` returns 0 and the
caller breaks out of its loop. §5.4 shows this actually happens in shipped data.

### The single best piece of evidence in this whole subsystem

`BattleDebug_Panel` (`0x00424992`) is the **developers' own debug overlay**, still in the
retail binary. It prints figure and unit fields next to their original labels:

```
FIGURE  map x  map y  tg x  tg y  routed  hold it  state  on route  hits  walking
        barred  target  dirc  delay  dly state  targeted  selected  selctd seen
        polar dirc  mov straff
GROUP   re targ  map x  map y  targ x  targ y  orders  firing
```

Every field name in §1 and §2 marked **[V]** comes from that panel. It is as close to an
original symbol table as this binary offers, and it is why `+0x19A` is called *hits* and
not "damage accumulator", and why `+0x18` is *dirc*.

It also killed one plausible-but-wrong story. `routed` (`+0x166`) sits next to `on route`
(`+0x164`), and "routed" in a battle game reads as *morale broken*. It is not: both fields
are only ever touched by the mover, and `+0x166` is incremented every time the figure has
to ask the pathfinder for a new **route**. See §8.3.

---

## 1. The battle unit — `g_battleUnits`, `0x00566520`, stride `0x34`

A unit is the thing the player clicks: one banner, one order, one formation. It owns a
contiguous *range* of figure indices.

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x00` | u8 | owner | [V] | player index. **0 means the slot is free** — this is the allocation test in `BattleUnit_Alloc`. |
| `+0x01` | u8 | humanControlled | [V] | copied from realm record `+0x05`. When set, `Battle_UpdateAllUnits` skips the unit's order handler entirely, so this is "a person is driving this unit". |
| `+0x02` | u8 | figureCount | [V] | number of figures actually created, counted up in `BattleUnit_Create`. |
| `+0x03` | u8 | side | [V] | **0 or 4**, not 0/1. See §4.3. |
| `+0x04` | i16 | firstFigure | [V] | lowest figure index belonging to this unit. |
| `+0x06` | i16 | lastFigure | [V] | highest. Every sweep over a unit's men is `for (i = first; i <= last; i++)` plus a check that the figure's `+0x178` points back here. |
| `+0x08` | u8 | category | [V] | 1 missile, 2 peasants/pikemen, 3 macemen/swordsmen, 4 knights, 5 catapult, 6 tower, 7 ram, 8 oil, 9/10 siege-defender specials. Selects the order handler. Recomputed by `BattleUnit_Classify`. |
| `+0x0F` | u8 | firing | [V] | set to 120 whenever a figure of this unit shoots or strikes; counts down. |
| `+0x14` | i16 | reTarg | [V] | re-target countdown, decremented once per frame; at zero it is reset to 500 and the unit looks for a new target. Initialised to 20, set to 50 when engaged. |
| `+0x1A` | i16 | orders | [V] | order state within the category handler. |
| `+0x1E` `+0x20` | i16 | map x, map y | [V] | the unit's own position: the centre of the bounding box of its live figures (`BattleUnit_Recentre`). |
| `+0x22` `+0x24` | i16 | targ x, targ y | [V] | ordered destination. |
| `+0x26` `+0x28` | i16 | — | [D] | a second destination pair, initialised to the unit's position. |
| `+0x2C` | u8 | orderedTarget | [D] | index of an enemy **figure** the player told this unit to attack. Cleared when that figure dies. |
| `+0x30` | i32 | targetCell | [D] | destination as a battlefield cell byte offset, `(y*80 + x)*8`. |

Unnamed and untraced: `+0x09`, `+0x0D`, `+0x0E`, `+0x10`, `+0x13`, `+0x2D`.

---

## 2. The battle figure — `g_battleMen`, `0x00554480`, stride `0x1B0`

A figure is one animated man on screen. It represents `g_menPerFigure` real soldiers
(§5.1), and it carries a `men` counter that ticks down as they die. When that counter
reaches zero the figure enters the dead state and is removed.

### 2.1 Identity and position

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x09` | u8 | selected | [V] | debug-panel label. |
| `+0x0A` | u8 | selctd seen | [V] | debug-panel label. |
| `+0x0C` | u8 | — | [D] | animation phase, seeded at creation as `(index*9 + x*16) & 0x3F + 0xB4` so identical figures do not march in lockstep. |
| `+0x12` | u8 | troopType | [V] | 0 … 10, the `TROOPS*.ENG` column order: peasant, crossbowman, maceman, swordsman, pikeman, archer, knight, catapult, siege tower, ram, oil. |
| `+0x13` | u8 | ownerIsHuman | [I] | 1 when the owning realm's byte `+0x05` is set. Same source as unit `+0x01`. **This byte changes the damage this figure takes** — §6.2. |
| `+0x14` | u8 | mercenary | [D] | set for the mercenary contingent of an army. |
| `+0x18` | u8 | **dirc** | [V] | facing, 0 … 7. **0 = N (−y), 1 NE, 2 E (+x), 3 SE, 4 S, 5 SW, 6 W, 7 NW**, and 8 means "same cell". Confirmed four independent ways: `Dir_FromDelta`'s branch structure, the neighbour scan order in `Melee_FindAdjacentEnemy`, and the two neighbour-offset tables `g_cellNeighbourOffsets` and `g_cellIndexNeighbours`. |
| `+0x1C` | i32 | cellOffset | [V] | `(y*80 + x) * 8`, kept in step with x and y. |
| `+0x20` `+0x22` | i16 | **map x**, **map y** | [V] | cell coordinates. |
| `+0x24` `+0x26` | i16 | **tg x**, **tg y** | [V] | where this figure is walking to. |
| `+0x28` `+0x2A` | i16 | — | [D] | a second target pair, filled from unit `+0x26`/`+0x28`. |
| `+0x2C` | u8 | owner | [V] | player index; **0 means the slot is free**. |
| `+0x2E` | u8 | — | [D] | 0 for side 4, 1 for side 0. |
| `+0x17A` | u8 | side | [V] | 0 or 4, copied from the unit. |
| `+0x178` | i16 | unit | [V] | owning unit index. |

### 2.2 State and animation

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x2F` | u8 | **dly state** | [V] | the state to return to after a delay. |
| `+0x30` | u8 | **delay** | [V] | debug-panel label. |
| `+0x31` | i8 | **state** | [V] | 0 … 17, index into `g_manStateTable` (`0x004D9170`). Known: **2 dead**, **4 melee**, **5 idle/firing**, **6 blocked**, **7 look for a melee**, 9 siege-wall movement, **12 siege engine attacking**, **17 closing to attack**. Initialised to 5 for troop types 0–6 and 11 for 7–10. |
| `+0x32` | i8 | **walking** | [V] | sub-cell progress; +2 per step, a cell is crossed at 17. |
| `+0x33` | i8 | — | [D] | tick counter against the move delay. |
| `+0x34` | u8 | — | [D] | bit 0 "ready to leave this cell", bit 1 "was interrupted". |
| `+0x35` | u8 | — | [D] | per-frame scratch, cleared at the top of every sweep. |
| `+0x18A` | u8 | animSet | [D] | 0 … 4 by troop type; also picks the melee exchange length (§6.1). |
| `+0x18B` | u8 | recovery | [V] | ticks between blows this figure can absorb. **This is the real melee defence.** §6.1. |
| `+0x189` | u8 | moveDelay | [V] | ticks per movement sub-step. §7. |
| `+0x1A8` | i32 | — | [D] | age in frames. |

### 2.3 Combat

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x172` | u8 | armour | [V] | subtracted from **missile** damage only. Rewritten every tick from the troop type, so it is a constant per type, not per figure. §6.2. |
| `+0x174` | u8 | **target** | [V] | figure index this one is shooting at. |
| `+0x175` | u8 | **targeted** | [V] | countdown while somebody is shooting at this figure. |
| `+0x17B` | i8 | — | [D] | melee recovery counter; the attacker decrements it, and a blow lands when it reaches 0. |
| `+0x17E` | u8 | opponent | [D] | the other figure in a melee duel. |
| `+0x180` | i16 | — | [D] | reload / swing timer. |
| `+0x182` | u8 | — | [D] | set to 120 on firing, counts down; the figure-level twin of unit `+0x0F`. |
| `+0x183` | u8 | engaged | [D] | "in contact, stop walking". |
| `+0x184` | i8 | exchange | [D] | ticks left holding the attacker role in a duel. |
| `+0x185` | u8 | role | [D] | 1 = attacker this exchange, 2 = defender. |
| `+0x18C` | u8 | blowUsed | [D] | set once the heavy-blow bonus has been spent. |
| `+0x194` | u8 | isSiegeEngine | [D] | 1 for troop types 7, 8, 9. |
| `+0x197` | u8 | band | [V] | strength band 0 … 3 from the men left (§5.3). Scales melee attack and missile damage. |
| `+0x198` | i16 | heavyBlow | [V] | extra hits landed **once per melee exchange**: maceman 300, knight 200, swordsman 100, everyone else 0. |
| `+0x19A` | i16 | **hits** | [V] | accumulated damage. **100 hits kills one man** (160 for a siege engine). |
| `+0x19C` | i16 | meleeAttack | [V] | hits per blow, from `g_meleeAttackTable[type][band]`. |
| `+0x19E` | i16 | missileDamage | [V] | hits per hit, from `g_missileStats[class][3]`. |
| `+0x1A0` | i16 | men | [V] | real soldiers still alive in this figure. |
| `+0x1A2` | i16 | menLastFrame | [D] | previous value, for the UI. |
| `+0x16A` | u8 | weaponClass | [V] | 0 melee, 1 bow, 2 crossbow, 3 catapult. |
| `+0x16B` | i8 | reloadTicks | [V] | `g_missileStats[class][1]`. |
| `+0x16C` | u8 | missileSubSteps | [V] | `g_missileStats[class][2]`, always 4. |
| `+0x16E` | i16 | missileSprite | [D] | `g_missileStats[class][4]`; added to the flight direction to pick the missile frame. |
| `+0x170` | i16 | range | [V] | `g_missileStats[class][0]`, in **eighths of a cell** — every consumer computes `range >> 3`. |

### 2.4 Movement and routing

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x164` | u8 | **on route** | [V] | 1 while the figure is following a stored path rather than walking straight at its target. |
| `+0x165` | u8 | **hold it** | [V] | ticks to wait before asking the pathfinder again; set to 64 after each attempt. |
| `+0x166` | i16 | **routed** | [V] | how many times this figure has been **re-routed**. Not morale — §8.3. |
| `+0x168` | u8 | **polar dirc** | [V] | facing chosen when knocked back or thrown. |
| `+0x176` | u8 | **barred** | [V] | consecutive pathfinding failures; at 4 the figure stops trying. |
| `+0x36` | i16 | pathLength | [D] | waypoints left. |
| `+0x38…` | u8×2 | path | [D] | up to 150 `(x, y)` waypoint pairs, consumed from the end. |
| `+0x1A4` | i16 | **mov straff** | [V] | set when the figure attacks without closing. |

---

## 3. The battlefield cell — `g_battlefield`, `0x005440E0`, 8 bytes, index `(y*80 + x)*8`

| Off | Ev | Meaning |
|---|---|---|
| `+0` | [V] | terrain id (`skr.md`'s "Lords2 id" column: 1 open, 3 rocks, 4 hills, 6 unused, 7/8/9 bridge parts, 11 water, 12 woodland, 13 the `0x15` lines). |
| `+1` | [V] | flags. **`0x10` and `0x80` make the cell impassable** — `Cell_TryEnter` rejects `flags & 0x90`. `0x40` blocks everyone too; `0x20` blocks side ≠ 0 only and makes the figure wait rather than reroute. |
| `+2` | [D] | further flags; bits `0x1C` are cleared and rewritten per cell during the build. |
| `+3` | [D] | graphic index. |
| `+4` | [V] | **elevation**. Governs missile damage (§6.2) and blocks movement between cells more than 1 apart (§7). |
| `+5` | [V] | index of the figure standing here, 0 for none. |
| `+6` | [V] | head of the linked list of missiles in this cell. |
| `+7` | [V] | surface type: 7 bridge, **10 and 17 burning**, 15 woodland, 4/5/6 read by `BattleUnit_Order` for siege structures. |

**[V] `Battlefield_BuildFromSkr` never writes byte `+4`.** On a `.skr` battlefield every
cell is at elevation 0, so the elevation rules in §6.2 and §7 are inert there. Elevation
must come from the two other builders, `Battlefield_BuildRandom` (`0x0047AAA3`) and
`Battlefield_BuildCastle` (`0x0047C4BA`), which also write this array — neither was
traced.

### 3.1 This settles two open questions in `skr.md`

**`0x10` on byte `+1` means impassable.** `skr.md` records the bit as set from terrain but
does not say what it does. `Cell_TryEnter` is the consumer, and the terrain values that
get it are `0x02` (hills), `0x09` (water), `0x15`, `0x20` (rocks) and `0x50`. **[V]**

So **`0x02` is an obstacle, not high ground.** `skr.md` reads it as "**[I]** hills / high
ground"; the elevation byte is never written from `.skr` terrain, and the flag it *does*
set is the impassability flag. And **`0x15`, listed as unidentified, is impassable** —
consistent with a fence or palisade, drawn from the woodland sprite bank at a lower index
range and, unlike woodland, not marked as woodland on byte `+7`. **[V]** for impassable,
**[I]** for the interpretation.

---

## 4. Bringing a battle up

```
Battle_Start                 0x004778A0
├── Battlefield_BuildRandom / Battlefield_BuildFromSkr / Battlefield_BuildCastle
├── Battle_InitArmies        0x0047EFEE
│   ├── size class from the two armies                      §5.1
│   ├── BattleMen_ClearAll / BattleUnits_ClearAll
│   └── Battle_RaiseSide  (×2)                              §5.2
└── the first update passes
```

### 4.1 The two armies

**[V]** `g_battleArmyA` (`0x00568210`) and `g_battleArmyB` (`0x00568218`) are indices into
the **campaign unit array** `g_units` (`0x0052F0B0`, stride `0x1A4`). Two fields of that
record drive the battle:

| Off | Meaning |
|---|---|
| `+0x168` | i32, total men over troop types 0 … 6 |
| `+0x16C + t*2` | u16 × 11, the troop counts, in `TROOPS*.ENG` column order |
| `+0x195`, `+0x196`, `+0x197` | a mercenary contingent: troop type, count, present flag |

**[V]** This is the same `+0x16C` offset the prior art reports for the campaign strategic
resolver, and the same eleven-column order as `TROOPS*.ENG` and the `.skr` army record —
three independent sources agreeing on the column order.

### 4.2 Autocalc strength is not the battle

`Battle_InitArmies` also computes `sum(count[t] * g_troopStrengthWeight[t])` for each side.
`g_troopStrengthWeight` (`0x004D4B98`) is seven ints:

```
peasant 2   crossbowman 16   maceman 8   swordsman 13
pikeman 9   archer 13        knight 22
```

**[V]** — and independently published by a third-party decompile of the same binary, which
gives the identical seven values at the identical address. But **it is used only for the
"is one side outnumbered by more than a third" flag and the strategic auto-resolve.** No
part of the real-time simulation reads it. Anyone tuning battle balance from this table
would be tuning the wrong thing.

### 4.3 Sides are 0 and 4, and side 0 is the `0x04` marker

**[V]** `Battle_InitArmies` raises army A with `side = 4` and army B with `side = 0`.
`Deploy_SlotForUnit` then reads the deployment slots as
`0x00553150 + unitOrdinal*8 + (side != 0) * 0x60`, and `skr.md` establishes that
`0x00553150` is filled from the `0x04` marker and `0x005531B0` from the `0x0F` marker.

> **Side 0 deploys at the `0x04` marker (the editor's blank map puts it at y = 20); side 4
> deploys at the `0x0F` marker (y = 60).**

`BattleUnit_OrderToEnemyEnd` looks like a contradiction — it sends side 0 to the `0x0F`
marker's centre — until you notice it is the order that sends a unit to the *enemy's* end
of the field. `0x0048B43C` confirms the geometry independently: it moves side 0 to
`homeY + 10` and side 4 to `homeY − 10`, so side 0 starts at low y and advances down the
field. **[V]**

Which side is the *attacker* is a separate question and is **not settled here**. The one
piece of evidence: in a siege, army B (side 0) is the side raised by
`Battle_RaiseSideSiege`, the castle-aware path — so army B is the defender. **[I]**

### 4.4 Where a skirmish army comes from

**[V]** `Skirmish_Setup` sets army A = campaign unit 1 and army B = unit 2, loads
`BATTLES.ENG` and the troops table, and `Skirmish_FillArmies` copies eleven counts out of
`g_troopsTable` into each. It does **not** read the `.skr` army table on this path;
`Skr_ReadMap` reads only the 6,400-byte terrain layer.

---

## 5. Raising an army: size class, units and figures

### 5.1 How many men one drawn figure represents

```
scale     = Table_Lookup((totalA + totalB) / 76, g_siegeScaleLadder, 1024)
sizeClass = Table_Lookup(totalA + totalB + scale * siegeEngines, g_sizeClassLadder, 8)
menPerFigure = g_menPerFigureTable[sizeClass]
```

**[V]** `g_menPerFigureTable` (`0x004D9658`) is `4, 8, 16, 32, 64, 128, 256, 512, 1024`
and `g_sizeClassLadder` (`0x004D95D8`) breaks at total men of
`305, 609, 1217, 2433, 4865, 9729, 19457, 38913`.

This is the mechanism the manual describes without numbers: *"in one battle, a unit may be
four men, in another it may be 16."* The smallest figure is exactly four men. **[V]**

**[V]** Each side then gets a finer scale if it would otherwise be nearly invisible: if
`sideTotal / menPerFigure < 9` and `menPerFigure > 7`, it is reduced to the next power of
two down (4, 8, 16, 32 or 64).

### 5.2 Splitting into units and figures

`Battle_RaiseSide` walks the eleven troop types in `g_raiseOrder` (`0x004D9870`) —
**ram, oil, knight, sword, mace, pike, crossbow, archer, peasant, tower, catapult** — and
for each cuts the men into units of at most `maxFigures[t] * menPerFigure`:

```
for t in g_raiseOrder:
    men = count[t]
    if t > 6: men *= menPerFigure          # one engine = one figure
    while men > 0:
        make a unit of min(men, maxFigures[t] * menPerFigure)
        men -= that
```

`BattleUnit_Create` then makes `ceil(unitMen / menPerFigure)` figures, laid out in a
rectangle `ceil(figures / maxPerRow)` deep, and gives the last figure the remainder rather
than a full complement. Formation offsets are `Formation_OffsetX` / `Formation_OffsetY`:
column `i / rows`, row `i % rows`, times the troop's cell footprint, with the row offset
negated for side 0 so the two armies face each other. **[V]**

The siege defender uses `Battle_RaiseSideSiege` instead: eight troop types from
`g_raiseOrderSiege` with no catapults, towers or rams, and archers and crossbowmen capped
at three figures per unit. **[I]** on the "defender" reading, **[V]** on the mechanics.

### 5.3 Strength bands

`BattleMan_RecomputeStrength` classifies a figure by how many of its men are left, against
three thresholds loaded per size class from `g_strengthBandTable` (`0x004D9800`):

| band | men remaining | melee attack | missile damage |
|---|---|---|---|
| 0 | ≥ 75 % | full | full |
| 1 | ≥ 50 % | reduced | × 4/5 |
| 2 | ≥ ~19–25 % | reduced | × 3/4 |
| 3 | below | lowest | × 1/2 |

**[V]** The thresholds really are 75 %, 50 % and 18.75–25 % of a full figure at every size
class (`3,2,1` of 4; `12,8,3` of 16; `768,512,192` of 1024). The melee column is
`g_meleeAttackTable`, §6.1.

### 5.4 The 80-figure ceiling is real, and the shipped data sits right against it

`tools/battle/armysim.js` reimplements §5.1 and §5.2 from the tables above and counts
figures.

```
$ node tools/battle/armysim.js skr "F:/games/Lords of the Realm II/USER.SKR"
map  0    600 vs   450 men  class 2    16 men/figure  units  8+ 6  figures 41+33 =  74
map  1    150 vs   150 men  class 0     4 men/figure  units  6+ 6  figures 39+39 =  78
...
  OK   allocations exceeding the 80-figure array: 0
```

**[V] All 20 `.skr` maps land between 74 and 78 figures and none exceeds 80.** That is the
strongest check available here: the model has four independent moving parts (the size
ladder, the men-per-figure table, the per-type `maxFigures`, and the per-unit rounding up),
and getting any of them wrong scatters the result.

> **Weaker than it first reads.** `USER.SKR` holds only **two distinct army
> pairings**: map 0 at 600 v 450 (class 2, 16 men/figure, 74 figures) and maps
> 1-19, which are all the identical blank template at 150 v 150 (class 0,
> 4 men/figure, 78 figures). So the ladder is exercised at **two** points, not
> twenty, and "20 for 20" counts eighteen copies of one case. Real evidence that
> the chain is right where it is tested, but two samples, not twenty.

Landing those two inside a six-wide
band just under the array bound does not happen by accident.

Running the same model over the three `TROOPS*.ENG` files — with the four derived
difficulty groups reconstructed the way `Troops_Load` does, Normal ± 8 % and ± 16 % — gives:

```
checked 525 (battle, difficulty) pairs, difficulty groups derived as the game does
  note field-battle allocations exceeding the 80-figure array: 6
       TROOPS.ENG  row 13 difficulty 1 -> 82        TROOPS2.ENG row 13 difficulty 2 -> 81
       TROOPS.ENG  row 15 difficulty 2 -> 81        TROOPS2.ENG row 24 difficulty 2 -> 81
       TROOPS.ENG  row 34 difficulty 2 -> 82        TROOPS3.ENG row 18 difficulty 2 -> 83
```

519 of 525 fit, and the six that do not overflow by 1 to 3 figures. **[I]** the reading:
those armies really are truncated in the shipped game, losing their last figure or two —
the tail of `g_raiseOrder` is peasants, siege towers and catapults, so a catapult is the
first thing to vanish. Six rows also use the *field* raise order where a siege would use
the shorter one, so some of the six may not arise in play. Not observed in a running game.

Related and untested: `Deploy_SlotForUnit` indexes a **12-entry** slot array by unit
ordinal with no bound check, so the 13th unit of a side reads into the other side's slots.
The shipped `.skr` armies produce at most 8 units per side. **[D]**

---

## 6. Combat resolution

Damage is measured in **hits**. `100 hits = one dead man`, or 160 for a siege engine.
That single constant governs melee, missiles and fire.

### 6.1 Melee

**[V]** Melee is a *duel between two figures*, not an area effect.

`BattleMan_LookForMelee` (state 7) calls `Melee_FindAdjacentEnemy`, which tests the eight
neighbouring cells in the order N, NW, NE, W, E, SW, SE, S and returns the first live enemy
figure — **skipping siege engines entirely, so troop types 7–10 are never melee targets**.
Both figures then enter state 4, record each other in `+0x17E`, turn to face, and one is
flagged attacker (`+0x185 = 1`), the other defender.

`Melee_Tick` (`0x00494908`), once per frame for each figure in state 4:

```c
if (self.recoveryCounter <= 0) {                  /* +0x17B */
    self.hits += opponent.meleeAttack;            /* +0x19A += +0x19C */
    self.recoveryCounter += self.recovery;        /* +0x18B */
}
if (self.hits > 99) { self.hits -= 100; self.men -= 1; }
if (self.men < 1) { self.state = 2; }             /* dead */
else if (self.role == 1) {                        /* this figure is attacking */
    if (!self.blowUsed) { opponent.hits += self.heavyBlow; self.blowUsed = 1; }
    opponent.recoveryCounter -= 1;
    if (--self.exchange < 1) { swap the attacker role }
}
```

Three things fall out of that, and all three match the manual's prose:

* **Damage per blow is the attacker's `meleeAttack`**, from `g_meleeAttackTable`
  (`0x004D98F8`), which is per troop type and strength band.
* **The interval between blows is the *defender's* `recovery`.** A figure that recovers
  slowly is hit rarely. This is the only melee defence in the game: `armour` (`+0x172`) is
  never read by `Melee_Tick`.
* **The heavy blow lands once per exchange**, and is large.

| troop | melee attack (band 0/1/2/3) | recovery | heavy blow | armour | exchange |
|---|---|---|---|---|---|
| Peasants | 5 / 4 / 3 / 2 | 6 | 0 | 0 | 40 |
| Crossbowmen | 5 / 4 / 3 / 2 | 8 | 0 | 12 | 40 |
| Macemen | 15 / 12 / 10 / 6 | 12 | **300** | 12 | 80 |
| Swordsmen | 15 / 12 / 10 / 6 | 12 | 100 | **35** | 80 |
| Pikemen | 10 / 8 / 6 / 5 | **30** | 0 | 35 | 80 |
| Archers | 5 / 4 / 3 / 2 | 6 | 0 | 0 | 40 |
| Knights | **20 / 15 / 11 / 6** | 16 | 200 | 25 | **120** |
| Catapults | 0 | 20 | 0 | 33 | 0 |
| Siege towers | 0 | 15 | 0 | 35 | 0 |
| Battering rams | 0 | 30 | 0 | 50 | 0 |
| Oil | 0 | 8 | 0 | 40 (25 if the owner is human) | 0 |

**[V]** Attack values and bands are `g_meleeAttackTable`; the other four columns are set
every tick by the per-troop-type handlers in `g_troopTickTable` (`0x004D9140`), one
function each.

Cross-check against the printed manual, which gives no numbers at all but does rank things:

| manual | table |
|---|---|
| pikemen's hand-to-hand attack is less than macemen, swordsmen and knights | 10 vs 15, 15, 20 ✓ |
| pikemen's defence value is relatively high | recovery 30, the longest by far ✓ |
| macemen are good attackers but weak defenders | heavy blow 300 (highest), armour 12 ✓ |
| knights have the highest attack | 20, and the longest exchange ✓ |
| archers are nearly useless against swordsmen and knights | attack 5, no bonus, no armour ✓ |

`+0x18C` (`blowUsed`) is set to 1 and, in the code paths examined, never cleared. If that
is genuinely the case a figure lands its heavy blow **once in the whole battle**, which
would be a bug worth reproducing. **Not established** — the writers of `+0x18C` outside
`Melee_Tick` were not traced.

### 6.2 Missiles

`BattleMan_FireMissile` (state 5, only for figures with a weapon class) counts `+0x180` up
each tick. Ten ticks before the reload interval expires it acquires a target with
`Missile_FindTarget`; when the interval expires it spawns a missile whose power is the
figure's `missileDamage` scaled by its strength band, and resets.

| weapon class | used by | range | reload | damage |
|---|---|---|---|---|
| 1 bow | archers | 120/8 = **15 cells** | **50 ticks** | **50** |
| 2 crossbow | crossbowmen | 64/8 = **8 cells** | **100 ticks** | **200** |
| 3 catapult | catapults | 160/8 = **20 cells** | 100 ticks | 200 |

**[V]** `g_missileStats` (`0x004D97B0`), and the manual again: *"archers have greater range
and a faster rate of fire than crossbowmen but do less damage per shot"* — 15 > 8 cells,
50 < 100 ticks, 50 < 200 damage. Three independent agreements from one sentence.

`Missile_Step` (`0x00492C8B`) advances the missile 4 sub-steps per tick and, on entering a
cell holding a live enemy figure, resolves the hit:

```c
power = missile.power;                              /* already band-scaled */
dh    = cellElevation(impact) - missile.launchElevation;
if (!target.ownerIsHuman) { if (dh > 3) power /= 5;  else if (dh > 1) power /= 3; }
else                      { if (dh > 3) power /= 4;  else if (dh > 1) power /= 2; }
power -= target.armour;                             /* +0x172 */
if (target.troopType >= 7) {                        /* shooting at a siege engine */
    threshold = 160;
    if (missile.class == 2) power /= target.ownerIsHuman ? 2 : 3;
    else if (missile.class == 1 && power < 1) power = target.ownerIsHuman ? 6 : 4;
    cap = 2 * (sizeClass * 5 + (target.ownerIsHuman ? 10 : 5));
    if (power > cap) power = cap;
} else threshold = 100;
if (power < 2) power = 2;
target.hits += power;
if (target.hits >= threshold) {                     /* 1..4 men at once */
    target.men -= (hits >= 4*threshold) ? 4 : (hits >= 3*threshold) ? 3
                : (hits >= 2*threshold) ? 2 : 1;
    target.hits = 0;                                /* remainder discarded */
}
if (target.men < 1) target.state = 2;               /* dead */
```

**[V]** as a reading of the code. Consequences worth stating plainly:

* **High ground is a defensive bonus against missiles, and only against missiles.** A
  target 4 or more levels above the shooter takes a fifth of the damage. Nothing in melee
  reads elevation. It is inert on `.skr` maps (§3).
* **Armour is a flat subtraction from missile damage.** A crossbow bolt (200) minus a
  swordsman's armour (35) still kills a man and a half; an arrow (50) minus 35 leaves 15,
  so an archer needs about seven hits to kill one swordsman and two to kill one peasant.
  This is the mechanism behind the folklore that crossbows are the anti-armour weapon.
* **A hit always does at least 2.** No target is immune.
* **The remainder is thrown away** on a missile kill but carried on a melee kill.

**The `ownerIsHuman` asymmetry is real and I cannot explain it.** The byte comes from
realm record `+0x05`, and the only evidence for what that byte means is that
`Battle_UpdateAllUnits` skips the AI order handler for units whose owner has it set — so
"a person is driving this side". With it set, a figure gets *less* protection from high
ground, takes more crossbow damage as a siege engine, and burns faster (§6.3). Whether
this is a deliberate handicap, an inverted test, or a different meaning for realm `+0x05`
entirely is **not established**. It is flagged here precisely because it is the kind of
finding that is easy to over-narrate.

### 6.3 Fire

**[V]** A figure standing on a cell whose byte `+7` is 10 or 17 takes `BattleMan_BurnTick`
damage every frame: 3, 6, 9 or 12 hits by battlefield size class (1, 3, 5, 7 for a siege
engine), plus 1 (or 2) if the owner is human. Same 100/160 threshold, one man per
crossing. Those surface values are written by `0x00485675` and `0x00485861`, the boiling-oil
and burning-cell effects.

### 6.4 What is *not* in the model

Traced and absent, as far as this investigation went:

* **No random numbers in the damage path.** None of `Melee_Tick`, `Missile_Step`,
  `BattleMan_BurnTick`, the eleven per-troop tick handlers, the melee and missile state
  handlers, `BattleMan_Step`, `Cell_TryEnter`, `Path_Search` or `Path_Extract` touches
  `FUN_00404B2C` / `DAT_005C9A84`, the counter the battlefield builder uses for its
  terrain variants. Damage is deterministic given positions and timings. **[D]** — over
  the functions traced, which is most but not all of the simulation; the untraced unit
  order handlers (§11) were not checked.
* **No morale.** `L2.eng` group 47 — the battle "Group Information" panel — does contain
  the string `Morale`, alongside `Heavy Infantry / Light Infantry / Slingers / Mixed Troops
  / Auxiliaries / Men`. **The unit field it displays was not found**: no literal `47` is
  pushed to `Eng_DrawString` or `Eng_GroupBase` anywhere in the binary. Whether the panel
  ships enabled, and what backs the number, is open. Nothing in the combat path reads a
  morale-like value, and the printed manual contains the words "morale", "fatigue" and
  "flank" zero times.
* **No fatigue.** The four-band degradation in §5.3 is attrition — men lost — not tiredness;
  a figure that loses no men never leaves band 0.
* **No flanking or facing bonus.** Facing is set from the direction of travel or of the
  opponent and is read only by the renderer and the melee pairing.

---

## 7. Movement

`BattleMan_Step` (`0x0048F1DD`), once per frame per figure:

```
+0x33 ++ ;  if (+0x33 <= moveDelay) return          # moveDelay+1 ticks per sub-step
+0x33 = 0 ;  +0x32 += 2 ;  if (+0x32 < 17) return   # 9 sub-steps to cross a cell
commit to the next cell
```

so one cell costs `9 * (moveDelay + 1)` ticks:

| troop | moveDelay | ticks per cell |
|---|---|---|
| Knights | **0** | **9** |
| Macemen | 1 | 18 |
| Peasants, Crossbowmen, Archers | 2 | 27 |
| Swordsmen | 3 | 36 |
| Pikemen | 4 | 45 |
| Catapults, towers, rams, oil | 5 | 54 |

**[V]** — `g_troopBattleStats[t][4]`, and the manual: *"macemen are second only to knights
in speed"*, *"pikemen move very slowly"*. Knight to pikeman is exactly 1 : 5.

Direction each cell:

1. `Melee_AdjacentEnemyDir` — if an enemy figure stands in one of the eight neighbours **at
   the same elevation**, go for it and stop moving.
2. otherwise `BattleMan_NextPathDir` if the figure is *on route*, else `Dir_FromDelta`
   straight at `tg x, tg y`.
3. `BattleMan_TryStepDir` → `Cell_TryEnter`, which returns 1 free, 0 blocked by a friendly
   figure or flag `0x40`, 2 impassable, 5 side-gated (`0x20`), 999 enemy present.

**[V] A step is only allowed when the two cells' elevations differ by at most 1**, unless
the destination's elevation is exactly 5. That is the whole terrain-height movement rule.

Blocked by a friendly figure of the same troop type walking to the same place?
`BattleMen_SwapPlaces` exchanges the two figures' positions, cells, hits and men. **[V]**
It is a genuine swap of state between records, not a move.

---

## 8. Orders and pathfinding

### 8.1 The two update sweeps

`Battle_UpdateAllMen` (`0x004822ED`) sweeps figures 1…80 and dispatches
`g_troopTickTable[troopType]()` — which reloads that troop's per-tick constants (armour,
recovery, heavy blow, and the band-scaled melee attack) and tail-calls
`g_manStateTable[state]()`.

`Battle_UpdateAllUnits` (`0x00489401`) sweeps units 1…80, recentres each on its figures,
and — **unless unit `+0x01` is set** — dispatches its order handler from one of three
tables by unit category:

| table | address | entries | when |
|---|---|---|---|
| field battle | `0x004D91B8` | 5 | `g_battleIsSiege == 0` |
| siege attacker | `0x004D91D0` | 9 | siege, unit side ≠ 0 |
| siege defender | `0x004D91F8` | 11 | siege, unit side == 0 |

It also counts unit `+0x14` down from 500 and re-targets at zero. **[V]** The individual
order handlers (`0x0048A9C7`, `0x0048ACD2`, `0x0048B02B` and the twenty siege ones) were
**not** decompiled.

### 8.2 Giving a unit a destination

`BattleUnit_Order` (`0x00479E90`) is the single entry point. Two behaviours stand out:

* **Missile units stop short.** `Order_StopShortOfTarget` pulls the destination back so the
  unit halts `(range/8 − 3)` cells from its target — 12 cells for archers, 5 for
  crossbowmen. This is why ranged units do not close. **[V]**
* **Mixed units split.** If a unit contains both missile and melee figures and is given an
  attack order, it allocates a *new* unit with `BattleUnit_Alloc` and moves the missile
  figures into it. **[D]**

`Dest_FindReachableNear` then does an expanding-ring search of radius 0…19 around the
requested cell for one with the same surface and elevation, so an order onto impassable
ground lands beside it rather than failing. **[V]**

### 8.2a The tables, checked against the oracle

Every number in `crates/l2-sim` came out of a decompiler *listing* — a reading of the
binary, not the binary. `tools/oracle/tables.ps1` closes that gap by reading the tables
straight out of `Lords2.exe`, and the result is the first genuine oracle verification in the
project. **[V]**

| table | address | verdict |
|---|---|---|
| `g_meleeAttackTable` | `0x004D98F8` | **11/11 rows identical** to `TroopTable::DEFAULT`'s `melee_attack` |
| `g_troopBattleStats` moveDelay | `0x004D96D0` | **11/11 identical** to `movement::move_delay` |
| `g_missileStats` | `0x004D97B0` | identical to the values recorded in `docs/symbols.json` |

Three things came out of it beyond the confirmation.

**A reading-comprehension failure worth recording, because it was mine and not the
document's.** The first version of the tool read `g_meleeAttackTable` as 32-bit and produced
`262149`, `131075`, `387389207` — numbers that look like data rather than obvious garbage,
which is C3's failure mode exactly. The giveaway is that `262149` is `0x00040005`: two small
numbers in a trenchcoat. The table is 11 rows × 4 **`u16`** = 88 bytes, and the 176 bytes a
32-bit reading consumes run past its end into an unrelated array. `docs/symbols.json`
already said "11 rows of 4 shorts", and had the tool been written from the symbol entry
rather than around it, the error would never have happened. C8 says to verify prior art
against the data; the converse also holds — **verify your reading against the notes you
already wrote.**

**The binary's troop order is confirmed, not assumed**, and it is exactly the order of the
`Troop` enum in `crates/l2-sim`. The `weaponClass` column of `g_troopBattleStats` is 2 at row
1, 1 at row 5 and 3 at row 7, which pins crossbow, bow and catapult to those rows and admits
only one ordering: Peasants, Crossbowmen, Macemen, Swordsmen, Pikemen, Archers, Knights,
Catapults, SiegeTowers, BatteringRams, Oil.

**The file is authoritative for these three tables, so the game need not be launched again
for them.** Running `-Source Both` launches the game, confirms the image base is still
`0x00400000` in a live process, and compares disk against memory: all three tables are
byte-identical. `Rules_InitConstants` does not touch them. That matters because reading the
file needs no process, no window and no focus — the cheap check is now known to be the
sufficient one.

`g_troopBattleStats` also carries three columns the simulation does not yet model: maximum
figures per unit (12 peasants or archers, 8 crossbowmen, macemen and swordsmen, 10 pikemen,
6 knights, 2 per siege engine, 1 oil), the cell footprint (1 for infantry, 3 for siege
engines, 2 for oil), and maximum figures per formation row. **[V]**

### 8.3 The pathfinder

This is what "routed" means, and it is the part of the system the project most wants to
change, so it is worth being precise.

Figures normally walk **straight at their target** — `Dir_FromDelta`, no search at all. The
pathfinder only runs when a figure is blocked, and then only if it has not already failed
four times (`barred`) and its `hold it` timer has expired.

`Path_Search` (`0x0047095E`) is a **weighted breadth-first flood fill** over the whole
80 × 80 grid. It is not a Dijkstra, and not uniform-cost either — see the correction below.

| structure | address | what |
|---|---|---|
| `g_pathCost` | `0x00504030` | `u16` per cell: 0 unvisited, 1 the start, **998 a friendly figure, 999 impassable** |
| `g_pathQueue` | `0x004FA820` | circular frontier queue of `0x1900` cell indices |
| `g_pathStepCost` | `0x004F2770` | `u8` per cell, the terrain cost |
| `g_pathVisitCount` | `0x004F6470` | `u8` per cell, times reached so far |
| `g_pathElevation` | `0x004F7D80` | `u8` per cell |

**[V]** Mechanics, in order:

* an early out: if the target is adjacent (`Dist_Chebyshev < 2`) or `Path_LineIsClear`
  succeeds, no search happens at all;
* the cost field is seeded from a blocked-cell template and the visit counters zeroed;
* the start cell is set to cost 1 and pushed; the loop runs until the destination has a
  cost or the queue empties;
* **terrain cost is charged twice over, by weighting *and* by deferral.** An earlier
  revision of this section said "by deferral, not by a priority queue", and that was wrong
  on the first half. Both mechanisms are present in the same loop:

  ```c
  /* deferral: an expensive cell is re-queued rather than expanded */
  if (g_pathStepCost[cur] == 0 || g_pathVisitCount[cur]++ >= g_pathStepCost[cur]) {
      sVar3 = g_pathCost[cur] + 1;
      ...
      /* weighting: the recorded cost carries the neighbour's surcharge */
      g_pathCost[nb] = g_pathStepCost[nb] + sVar3;
  ```

  so a cell of cost *k* expands on its (*k*+1)-th pop, and what is written down is
  accumulated cost rather than hop count. See correction **C12** in `docs/decisions.md`;
* **the cost field is never relaxed.** A neighbour is considered only while
  `g_pathCost[nb] == 0`, so the first cost written to a cell stands even when a cheaper
  route reaches it later. The field is therefore not a metric, and this is not Dijkstra;
* **[D] on a `.skr` battlefield every step costs zero.** `Path_BuildStepCost`
  (`0x00471DA6`) only charges cells flagged `0x20` or `0x40`, and `Battlefield_BuildFromSkr`
  sets neither. Field pathfinding is a plain breadth-first search; the weighting exists for
  castles;
* all eight neighbours are expanded, each only if its elevation is within 1 of the current
  cell's;
* the queue index wraps at `0x1900`, so a search that outgrows 6,400 frontier entries
  silently overwrites its own queue.

**998 and 999 are not interchangeable.** `Path_BuildBlockedMap` (`0x00471F45`) writes 998
where a *friendly* figure stands; `Path_BuildTerrainTemplate` (`0x00471C1F`) writes 999 for
terrain and the map border. They part company at the destination: 998 is cleared to 0 and
the search proceeds — the figure paths onto the occupied cell and the mover swaps or waits —
while ≥ 999 abandons the search before the first pop. Enemy figures are never marked at all,
so the pathfinder routes straight through them and leaves contact to the mover.

`Path_Extract` (`0x00472392`) then walks the cost field **downhill from the destination
back to the start**, writing up to 150 `(x, y)` byte pairs into `g_pathWaypoints`
(`0x004F9680`), breaking ties towards the straight-line direction and enforcing the same
elevation rule. `BattleMan_SetPath` copies that list into the figure at `+0x38`, sets the
count at `+0x36`, and sets **`on route`**; `BattleMan_NextPathDir` consumes it from the
end.

So `routed` (`+0x166`) is incremented once per call to this machinery, and `barred`
(`+0x176`) counts consecutive failures. **[V]** The names are the game's own, from the
debug panel, and the fields are only touched by the mover.

---

## 9. Reading a live battle

`Lords2.exe` has no ASLR, so all of the above can be read out of a running game from
another process.

```powershell
powershell -File tools/battle/battlestate.ps1                     # globals, units, figures
powershell -File tools/battle/battlestate.ps1 -Action units
powershell -File tools/battle/battlestate.ps1 -Action cell -X 40 -Y 20
powershell -File tools/battle/battlestate.ps1 -Action raw -Addr 0x554480 -Length 432
```

It opens the process read-only and never writes. **It has been tested against a live
process for its memory-read path, but not against a running battle** — nothing in this
investigation launched the game, per `docs/decisions.md` D8 and the "clean up processes you
start" rule.

The obvious first differential test, once the game is driven: start a `.skr` battle,
compare `armysim.js`'s predicted unit and figure counts against the live arrays. Map 0 of
`USER.SKR` should give size class 2, 16 men per figure, 8 units and 41 figures for the
attacker and 6 units and 33 figures for the defender.

---

## 10. Corrections to other documents

These are for the owners of those files; nothing outside `battle.md`, `symbols.json`,
`symbols.md` and `tools/battle/` was touched.

**`docs/formats/eng.md` §3.1 — the in-memory `TROOPS*.ENG` layout.** The document says the
table is `short[35][5][2][11]` (difficulty outer, side inner). It is the other way round:
**`short[35][2][5][11]`**, row stride `0xDC`, **side stride `0x6E`, difficulty stride
`0x16`**. **[V]** twice: the zeroing loop at the top of `Troops_Load` writes offsets
`0x00, 0x16, 0x2C, 0x42, 0x58` and `0x6E, 0x84, 0x9A, 0xB0, 0xC6` — five of one thing then
five of another — and `Skirmish_FillArmies` reads
`base + row*0xDC + difficulty*0x16 + side*0x6E`. The *file* parse order in §3.1 is not
affected; only the description of the array in memory.

**`docs/formats/eng.md` §3.3 — which of the three troops files is used.** Listed as open.
`Troops_Load` picks `troops.eng` when `DAT_00553030` is non-zero; otherwise `troops2.eng`
when `DAT_0053EF5C == g_localPlayer` and `troops3.eng` when not. So the choice between
`troops2` and `troops3` is **attacker versus defender**, not field versus siege. **[V]** on
the branch, **[I]** on `DAT_0053EF5C` being the attacking player.

**`docs/formats/skr.md` — cell flag `0x10`.** It means **impassable**; see §3.1.

**`docs/formats/skr.md` — terrain `0x02`.** Recorded as "**[I]** hills / high ground". It
is an impassable obstacle, and `Battlefield_BuildFromSkr` never writes the elevation byte,
so nothing on a `.skr` map is high ground. §3.

**`docs/formats/skr.md` — terrain `0x15`, unidentified.** Impassable. §3.1.

**`docs/formats/skr.md` — which deployment marker belongs to which side.** Side 0 deploys
at the `0x04` marker, side 4 at the `0x0F` marker, and side is 0 or 4 rather than 0 or 1.
Which side is the *attacker* remains open. §4.3.

---

## 11. What is still unknown

* **The unit order handlers.** Twenty-five functions across three dispatch tables
  (`0x0048A9C7` … `0x0048ECA9`) decide what an unordered unit does — advance, hold, flank,
  man a wall. None was decompiled. This is where any battle "AI" lives.
* **Elevation.** Where cell byte `+4` comes from on a field or castle battlefield.
  `Battlefield_BuildRandom` and `Battlefield_BuildCastle` were not read.
* **`ownerIsHuman`.** §6.2. The asymmetry is in the code; its intent is not established.
* **The "Morale" display.** `L2.eng` group 47 index 9. No caller found; no backing field
  found.
* **`+0x18C`** — whether the melee heavy blow is really once per battle rather than once
  per exchange.
* **Ticks.** Everything above is in frames. The battle frame rate, and whether it is fixed
  or wall-clock, was not established, so no timing here can be converted to seconds.
* **Siege specifics** — walls, gates, drawbridges, the `0x20` and `0x40` cell flags, the
  oil and boiling-water effects, and `Battle_RaiseSideSiege`'s slot placement.
* **The `.skr` army table.** `Lords2.exe` reads only the terrain layer of a `.skr` on the
  path examined; where (or whether) it reads the 44-byte army records was not found.
* **No runtime confirmation of anything.** Every claim is static: the binary, the shipped
  data files, and the manual. No battle was observed running.

---

## 12. Reproduction

```bash
# figure allocation model vs the shipped data
node tools/battle/armysim.js troops "F:/games/Lords of the Realm II"
node tools/battle/armysim.js skr    "F:/games/Lords of the Realm II/USER.SKR"
node tools/battle/armysim.js one 200 25 100 75 25 150 25 -- 100 50 50 50 100 100 0

# locate a UI string's (group, index) in L2.eng
node tools/battle/engfind.js "F:/games/Lords of the Realm II/L2.eng" Morale "Heavy Infantry"
```

```powershell
# decompile, cross-reference and dump, into tools/battle/out/
powershell -File tools/battle/ghraw.ps1 -postScript BDecomp out.c 00494908 00492c8b
powershell -File tools/battle/ghraw.ps1 -postScript BRefs   r.txt range 566500 566600
powershell -File tools/battle/ghraw.ps1 -postScript BDump   b.txt bytes 4d98f8 88
powershell -File tools/battle/ghraw.ps1 -postScript BCallArg c.txt 00402d37

# read a live battle
powershell -File tools/battle/battlestate.ps1
```

Ghidra scripts live in `ghidra_scripts_battle/` (`BDecomp`, `BRefs`, `BDump`, `BCallArg`),
kept separate from `ghidra_scripts/` so parallel agents do not edit the same files. The
names are pushed into the database with `ghidra_scripts/ApplySymbols.java` as described in
[`symbols.md`](symbols.md).
