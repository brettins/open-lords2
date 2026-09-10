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
| missiles | `0x0057A100` | `0x4C` | 1 … 100 | arrows, bolts, catapult shot and its debris, burning cells, boiling oil — classes 1, 2, 3, 4, 5 and 7, and **there is no class 6**. This row said *"falling men"* and was wrong; §14.7 corrects it |

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
| `+0x0B` | u8 | fidget tick | [V] | counts up once per frame in `Anim_StandA2`. |
| `+0x0C` | u8 | fidget period | [V] | seeded once in `BattleMan_Create` as `((index*9 + x*16) & 0x3F) + 0xB4`, so 180 … 243. **This is not an animation phase** — §13.8 was right that the handlers step `+0x0E` instead, and this is what `+0x0C` is actually for: when `+0x0B` passes it, `Anim_StandA2` resets `+0x0B` and turns `facingDrawn` one step, left on an even map x and right on an odd one. A standing figure shifts its feet every 180–243 frames and no two neighbours do it together. One writer, two readers (`Anim_StandA2` and `Anim_StandA3`). |
| `+0x12` | u8 | troopType | [V] | 0 … 10, the `TROOPS*.ENG` column order: peasant, crossbowman, maceman, swordsman, pikeman, archer, knight, catapult, siege tower, ram, oil. |
| `+0x13` | u8 | ownerIsHuman | [I] | 1 when the owning realm's byte `+0x05` is set. Same source as unit `+0x01`. **This byte changes the damage this figure takes** — §6.2. |
| `+0x14` | u8 | mercenary | [D] | set for the mercenary contingent of an army. |
| `+0x18` | u8 | **dirc** | [V] | facing, 0 … 7. **0 = N (−y), 1 NE, 2 E (+x), 3 SE, 4 S, 5 SW, 6 W, 7 NW**, and 8 means "same cell". Confirmed four independent ways: `Dir_FromDelta`'s branch structure, the neighbour scan order in `Melee_FindAdjacentEnemy`, and the two neighbour-offset tables `g_cellNeighbourOffsets` and `g_cellIndexNeighbours`. |
| `+0x1C` | i32 | cellOffset | [V] | `(y*80 + x) * 8`, kept in step with x and y. |
| `+0x20` `+0x22` | i16 | **map x**, **map y** | [V] | cell coordinates. |
| `+0x24` `+0x26` | i16 | **tg x**, **tg y** | [V] | where this figure is walking to. |
| `+0x28` `+0x2A` | i16 | **aim x**, **aim y** | [V] | the figure's own aim point. **Corrected:** this row used to say "filled from unit `+0x26`/`+0x28`". It is not. Once `g_battleMen` and `g_battleUnits` were given struct types (`docs/records.json`), the whole corpus could be searched for writers, and there is exactly one non-zero writer — `FUN_004843bc`, the ranged-figure handler, copying the unit's `+0x16`/`+0x18`, which is the aim point §7 of `docs/battle-ai.md` already described. The three other writers only zero it. |
| `+0x2C` | u8 | owner | [V] | player index; **0 means the slot is free**. |
| `+0x2E` | u8 | — | [D] | 0 for side 4, 1 for side 0. |
| `+0x17A` | u8 | side | [V] | 0 or 4, copied from the unit. |
| `+0x178` | i16 | unit | [V] | owning unit index. |

### 2.2 State and animation

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x2F` | u8 | **dly state** | [V] | the state to return to after a delay. |
| `+0x30` | u8 | **delay** | [V] | debug-panel label. |
| `+0x31` | i8 | **state** | [V] | 0 … 17, index into `g_manStateTable` (`0x004D9170`). **All eighteen slots are now named — §14.2.** Initialised to 5 for troop types 0–6 and 11 for 7–10. The bound is the game's own: every one of the eleven per-troop tick handlers tests `state < 0 || state > 0x11` and calls `BattleMan_Destroy` outside it. |
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

The record is typed as `BattleCell` in [`docs/records.json`](records.json) and applied to the
Ghidra database before every corpus rebuild, so the decompiled corpus reads
`g_battlefield[y * 0x50 + x].surface` and `(&g_battlefield[0].figure)[cellOffset]` rather
than a synthetic global per plane. All eight bytes are named, and `RecordProbe` finds all
566 references to it in the binary are one byte wide but two, and those two are
`PUSH 0x5440e0` — the array's own address, not a read of a cell. The 80 × 80 × 8 shape is confirmed from the game's
own side: both callers of the isometric grid renderer pass
`FUN_004bc020(bank, bank2, g_battlefield, 0x50, 0x50, 8, 0, …)`.

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
`Battlefield_BuildCastle` (`0x0047C4BA`), which also write this array.

**Both of those builders are now traced.** They were opaque because every access to this
array was a synthetic global; giving the record a struct type (`BattleCell` in
`docs/records.json`) made them read. Two corrections follow, and they matter for anyone
implementing against the table above.

### 3.0 The `terrain` column above is a translation's output, and it is incomplete  **[V]**

`Battlefield_BuildRandom` reads a byte per cell out of `batfield.pl8`'s raster and
*translates* it into the runtime `terrain` id:

| source byte | → terrain | | source byte | → terrain |
|---:|---:|---|---:|---:|
| `0` | 1 | | `0x0F` | **0x1E** |
| `2` | 4 | | `0x10` | 7 |
| `4` | **0x14** | | `0x12` | 8 |
| `7` | **0x28** | | `0x14` | 9 |
| `8` | **0x29** | | `0x15` | 0x0D |
| `9` | 0x0B | | `0x20…0x3F` | 3, `frame` = source, `flags \|= 0x10` |
| `0x0A` | 0x0C | | `0x50…0x5F` | **6**, `frame` = source + 0x2C, `flags \|= 0x10` |
| | | | anything else | passed through **verbatim** |

Three things the §3 table gets wrong as a result:

* **`6` is not "unused".** It is written for every source byte in `0x50…0x5F` — sixteen
  variants of an impassable object, distinguished by the frame.
* **`0x14`, `0x1E`, `0x28` and `0x29` are live ids that the table does not list at all.**
  The sweep immediately after the translation pairs a `0x14` with the cell one row south
  (`+0x280`) and a `0x28` with a `0x29` at `+8`, `+0x10` or `+0x18` — they are multi-cell
  structures, matched the way the campaign map's 2×2 blocks are.
* The pass-through case means the id space is **open**: a source byte outside every arm
  lands in `terrain` unchanged.

`Battlefield_BuildCastle` uses the byte differently again: its first sweep writes `terrain`
= 11 (water — the moat) where the source byte is `0xEE` and 1 (open) everywhere else, so on
a castle battlefield `terrain` only ever holds those two values and the structure lives in
the other planes.

### 3.0.1 Byte `+4` is escape-encoded during a castle build  **[V]**

`Battlefield_BuildCastle` seeds `elevation` from a 256-entry, 2-byte-per-frame table —
`0x004D7D80`, or `0x004D7B80` when the other tile set is selected — indexed by the cell's
own frame byte. The table's **second** byte is the passability flag: zero sets `flags |= 0x10`.

The first byte is *not* always a height. Values 5…12 are structure codes, and the builder
consumes each one immediately, replacing it with a real elevation and writing `surface` and
`flags` as it goes:

| code | `surface` ← | `elevation` ← | `flags` |
|---:|---:|---:|---|
| 5 | — | 3 | `= 0`, then `\| 0x04` |
| 6 | 6 | 1, or 4 on the other tile set | `= 0`, `\| 0x08`; `flags2 \| 0x80` |
| 7 | 7 (bridge) | 2 | — |
| 8 | 8 | 1 | `= 0`, `\| 0x20`, `\| 0x04` |
| 9 | 0x0B | 0 | `= 0`, `\| 0x40` |
| 10 | 0x0E | 0 | `\| 0x04` |
| 11 | — | 1 | `= 0`, `\| 0x04` |
| 12 | — | 2 | `= 0`, `\| 0x04` |

So `+4` is only an elevation once the build has finished, the `surface` values `8`, `0x0B`
and `0x0E` are written here and appear nowhere in §3's list, and `flags` bits `0x04` and
`0x08` — undocumented above — are set on six of the eight codes. Each code also records a
landmark: code 6 remembers the first such cell in `DAT_00553274` (offset by one row for a
human attacker), code 8 the first in `DAT_00553EE4` and the fifth in `DAT_0053E9D4`.

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
* **The heavy blow lands once per figure, for the whole battle** — not once per exchange, as
  this line read until a player said macemen felt like steady high damage rather than one
  spike. Checking settled it: `blowUsed` (`+0x18C`, `0x0055460C`) has exactly three
  references in the binary — set in `Melee_Tick`, read in `Melee_Tick`, and zeroed in
  `BattleUnit_Create`, which is figure *initialisation* beside a dozen other fields. Nothing
  resets it per exchange. **[V]**, with one caveat: this is an absolute-reference search, so
  code reaching the field through a computed pointer would not appear in it.

  The player's impression is still right, and the design is sharper than "one big hit".
  Macemen and swordsmen have **identical** base attack; they differ only in the heavy blow
  (300 against 100) and armour (12 against 35). At 100 hits per casualty a maceman's opening
  swing kills **three men outright**, from every figure in the unit — which reads as high
  damage output without ever presenting itself as a separate mechanic. It is also exactly
  the manual's *"macemen are good attackers but weak defenders."*

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

#### A shot genuinely traverses, and can be intercepted

**[V]**, and it is the question everything else about missiles turns on. A shot is **not**
resolved at launch and animated afterwards. `Missile_Step`'s hit test is:

```c
g_otherBattleMan = g_battlefield[missile.cellOffset].figure;   /* cell byte +5 */
if (missile.class < 3 && g_otherBattleMan != 0
    && g_battleMen[g_otherBattleMan].state != 2                /* not dead */
    && missile.owner != g_battleMen[g_otherBattleMan].owner)   /* owner, not side */
```

The victim is read out of the cell the missile has **just entered**, fresh, every sub-step.
Nothing anywhere in the 0x4C-byte record remembers who the shot was aimed at — `+0x06` is the
*shooter* — so an arrow cannot check whether it hit the right man, and does not. Five
consequences, all of them visible in play:

* **A body in the flight path takes the arrow.** Anyone who has walked into it is hit
  instead, which is what makes a screening line work.
* **A miss keeps flying.** When the Bresenham line is exhausted the missile switches to
  `FUN_00494265` and coasts on in its launch direction until range, the map edge or somebody
  else stops it. Overshoot can kill a second rank.
* **A target that dies or walks away is not tracked.** The impact point is frozen at launch.
* **Friendly fire is impossible**, and a friendly body does not stop the arrow either: the
  test is on the **owner** byte, not the side. *[I]* — if a battle can ever hold two owners
  on one side, allies would be both targetable and shootable; whether it can was not
  established.
* **One hit per missile is structural, not a rule.** The whole impact block is gated on
  `ttl == 0`, and a hit sets `ttl = 2`, which `Missile_UpdateAll` counts down to nothing.

**Blocking is real too.** Ground more than one level above the launch point, or a cell with
`flags & 0x80` (which `FUN_0049207E` stamps over a siege engine's 3 × 3 footprint), sets a
sticky `blocked` flag. A blocked arrow or bolt is discarded once `blockedTicks` passes `0x20`
— the counter is seeded `(shooterIndex & 0x10) + 4`, so 4 or 20, and a blocked volley gives
up raggedly rather than all at once — or the moment the ground comes back down to the launch
elevation. Catapults are exempt.

#### The damage

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

`g_troopBattleStats` carries three further columns: maximum figures per unit (12 peasants or
archers, 8 crossbowmen, macemen and swordsmen, 10 pikemen, 6 knights, 2 per siege engine, 1
oil), the cell footprint (1 for infantry, 3 for siege engines, 2 for oil), and maximum
figures per formation row. **[V]** All three are now modelled, in
`crates/l2-sim/src/formation.rs`: the first decides where one unit ends and the next begins
when an army is raised, and the other two decide the shape of the rectangle its figures form
up in, both at deployment and at every reform.

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

---

## 13. Drawing a battle

Everything above is state. This section is how it becomes a picture, which is
what `crates/l2-view` implements. Same status legend: **[V]** verified against a
second independent source, **[D]** a straightforward reading of decompiled C,
**[I]** inferred.

### 13.1 The screen

**[V]** `Battle_LoadAssets` (`0x004987B7`) hands the tile renderer its geometry
in one call:

```c
FUN_004bc020(tileset, tileset2, &g_battlefield, 0x50, 0x50, 8, 0, 0x18, 0xf, 0xe, 0x20);
//                                               80    80   8  0    24   15   14    32
```

so a **15 x 14 viewport of 32-pixel tiles at screen `(0, 24)`**, over the 80 x 80
array of 8-byte cells. The consumer (`0x004BCBDC`) steps the destination by
`0x20` in both axes: the battlefield is a **plain square grid seen from above**,
not isometric like the campaign map. 15 x 32 = 480 wide leaves 160 pixels for the
panel; 24 + 14 x 32 = 472 leaves 8 at the bottom.

`0x004BC142` is the whole frame, `(cameraTileX, cameraTileY)` its only
arguments, and it dispatches on the tile size: **16 and 32 are the two zoom
levels**, with a parallel renderer for each. **[V]**

### 13.2 Cell byte `+3` is a PL8 frame index

Section 3 lists byte `+3` as "graphic index" **[D]**. It is now **[V]**, and it
indexes `T32_bat1.pl8` specifically on a field battlefield.

The renderer reads `tileset + cell[+3] * 0x10 + 8` — literally the address of
that frame's record in the PL8 frame table (`pl8.md`: 8-byte header, 16-byte
records). No other interpretation of the byte is possible.

Which tileset is fixed two ways. `Battle_LoadAssets` passes slot 0 of the battle
asset table, which is `t32_bat1.pl8` for a field battle and `t32_stn1.pl8` /
`t32_wod1.pl8` for the two siege variants. And two pieces of arithmetic in
`Battlefield_BuildFromSkr` only fit a 252-frame file:

* woodland's high branch computes `base - 0x17` **as a signed char**, and its
  three highest bases (`0x10`, `0x11`, `0x12`) wrap to **249, 250, 251** — the
  last three frames of `T32_bat1.pl8`, which has exactly 252;
* the `0x15` lines compute `base - 0x1A`, giving **230 … 248**, which is exactly
  the gap between the highest water index (229) and those three.

The index space closes with no overlap and no overflow:

| range | terrain |
|---|---|
| 0 … 15 | open ground, 16 random variants |
| 16 … 31 | woodland |
| 32 … 39 | rocks (`0x20`) |
| 64 … 111, 159 | hills / obstacles (`0x02`) |
| 112 … 123 | ground bordering an obstacle |
| 124 … 131 | terrain id 6, the unused one |
| 140 … 159 | bridge parts |
| 160 … 229 | water |
| 230 … 248 | the `0x15` lines |
| 249 … 251 | woodland, three interior tiles |

**[V]** as arithmetic over the tables; the *appearance* of each range is
**[I]** and has not been checked against a screenshot of the original.

### 13.3 The variants are an auto-tiler, not a random pick

`skr.md` records "49-variant set" and similar without saying what picks the
variant. `Battlefield_BuildFromSkr` does, and it is **[V]**:

1. `0x0047D816` builds an eight-entry mask of "is this neighbour the same
   terrain id", in the order **N, NE, E, SE, S, SW, W, NW** — derived from the
   cell offsets it indexes (`-80, -79, +1, +81, +80, +79, -1, -81` cells).
   Off-map neighbours take a caller-supplied value: `1` for hills, water,
   woodland and lines, `0` when open ground is asking about hills.
2. `0x0046C2DE` matches that mask against a table of 12-byte entries: eight
   pattern bytes where `0` means "must not", `1` means "must" and `2` means
   "don't care", then a base graphic index, an untraced byte, a variant count,
   and a **rotating counter**.
3. The graphic is `base + counter`, where the counter is post-incremented and
   wrapped on every match. So the variants cycle deterministically; they are not
   drawn at random.

Four tables, all in `.data`:

| Address | Entries | Used by |
|---|---:|---|
| `0x004D7550` | 11 | hills / obstacles, id 4 |
| `0x004D75C8` | 6 | open ground bordering an obstacle |
| `0x004D7610` | 49 | water, id 11 |
| `0x004D7860` | 17 | woodland (id 12) and the `0x15` lines (id 13) |

The first table's declared 11 entries physically overlap the second's first
entry, but its tenth is a catch-all, so the eleventh is unreachable. Likewise
the water table's last two entries sit behind its own catch-all. Reproduced as
found rather than tidied.

The genuinely random cases are open ground with no obstacle neighbour
(`rand & 0x0F`), rocks (`(rand & 7) + 0x20`) and id 6 (`(rand & 7) + 0x7C`).
`rand` is `0x00404B2C`, **a 31-bit LFSR with taps at bits 0 and 4, stepped 31
times per call and returning the low seven bits** — and it is stepped **once per
cell** whether the result is used or not, so the sequence depends only on the
seed. **[V]** as a reading of the code. **The seed a battle starts from was not
traced**, so our build takes it as a parameter; the choice only moves which of
sixteen grass tiles a cell gets.

### 13.4 Sprite banks

**[V]** `0x004DA550` holds a table of 120 twenty-byte filenames in two parallel
sets of 60. Set A is `t32_*` tiles, `a2_miss`, `a2_horse`, `engine`,
`catarm1/2`, `t2_*` minimap tiles, `t2_spri`, then **six colours of seven troop
sheets**; set B is identical but with `a3_horse` and the `a3` sheets.

* Colour order is `w, r, y, k, p, b` — white, red, yellow, black, purple, blue —
  indexed by the owning realm's colour byte (campaign unit `+0x02`). `a2g_*`
  files exist on disk and are **not** in the table.
* Troop order within a colour is `psnt, cros, mace, swor, pike, arch, knig`, the
  `TROOPS*.ENG` column order and troop types 0 … 6. **[V]**
* `Battle_Start` loads set A (`a2`); the skirmish and roster screens load set B
  (`a3`). Both sets have their own animation handlers, and the `a3` handlers
  give different poses-per-facing. **Settled, and the answer is that the battle
  state machine never uses `a3` at all: all six `a3` handlers are unreachable
  code.** §14.4. `crates/l2-view` draws `a2` at 32 pixels, which is right.

`0x00480F8B` then writes a sprite-sheet pointer into each figure at `+0x00`,
choosing by troop type and by figure byte `+0x2E` — **`+0x2E` is what selects
army A's bank from army B's**. Knights also get a horse sheet pointer at `+0x04`.
**[V]**

### 13.5 The figure frame layout

**[V]** A sheet is **eight facings of N poses, then eighteen shared frames**:

```
frame = facing * N + pose
  pose 0 … 5            walking, one pose every 4 ticks over a 24-tick loop
  pose 6 …              striking, from a per-troop cycle table
  pose N-1              standing
  pose 10 … 12          drawing a bow          (crossbowmen and archers only)

8 * N + 0 … 5           collapsing, six frames
8 * N + 6 …             dying: 4 half-facings of 3 frames
```

**Corrected: walking and striking were the wrong way round here**, and §13.8's
`+0x18` / `+0x19` row carried the same swap. §14.5 gives the evidence.

| troop | N | standing pose | strike cycle | strike frames | collapse base | dying base |
|---|---:|---:|---|---|---:|---:|
| peasants | 10 | 9 | `0,0,1,1,2,2,1,1,0,0` | 6 … 8 | 80 | 86 |
| crossbowmen | 13 | 9 | `0,0,1,1,2,2,1,1,0,0` | 6 … 8 | 104 | 110 |
| macemen | 12 | 11 | `0,1,2,3,4,4,3,2,1,0` | 6 … 10 | 96 | 102 |
| swordsmen | 12 | 11 | `0,1,2,3,4,4,3,2,1,0` | 6 … 10 | 96 | 102 |
| pikemen | 8 | 7 | `0,0,1,1,1,1,1,0,0,0` | 6 … 7 | 64 | 70 |
| archers | 13 | 9 | `0,0,1,1,2,2,1,1,0,0` | 6 … 8 | 104 | 110 |

from `Anim_WalkA2` (`0x00486D83`, walking), `Anim_StrikeA2` (`0x00486249`,
striking and standing), `Anim_StandA2` (`0x004872AE`, standing with the fidget),
`Anim_CollapseA2` (`0x00487CE4`), `Anim_DyingA2` (`0x00487908`) and
`Anim_DrawBowA2` (`0x0048804A`), which all write the frame index to figure
`+0x10`. Strike cycles are `g_strikeCycleMace` (`0x004D9A00`), `g_strikeCycleBow`
(`0x004D9A28`) and `g_strikeCyclePike` (`0x004D9A50`), stepped every fourth tick
of a forty-tick loop; dying is `base + (facing & 6) / 2 * 3 + phase / 32`.

**The layout closes with nothing spare.** Peasants: 0–5 walk, 6–8 strike (the
cycle's maximum is 2), 9 stand — ten. Macemen: 0–5, 6–10 (maximum 4), 11 —
twelve. Crossbowmen and archers: 0–5, 6–8, 9, and **10–12 the bow draw**, which
is exactly where `Anim_DrawBowA2`'s otherwise unexplained `+ 10` puts it —
thirteen. Pikemen at N = 8 are the one squeeze: the cycle's maximum is 1, so the
strike's second frame is the standing pose. Getting the two blocks the wrong way
round leaves the `+ 10` with nowhere to point.

**Why this is more than a decompiler reading.** Every one of the **36** shipped
`a2` sheets that is not a knight — six colours by six troop types — has exactly
`8 * N + 18` frames, and in all four handler groups the dying base is exactly
`8 * N + 6`. Getting N wrong for any troop breaks both identities at once. The
corpus check is `crates/l2-view/tests/install.rs`.

Knights are the exception: their frame comes from an **8 x 8 `(body facing,
target facing)` table at `0x004D9C30`**, and the engine rotates the body facing
outward until it finds a non-zero entry. The sixteen live entries are spaced
three apart and top out at 53, which with the walk cycle's maximum of 2 reaches
frame 55 — and `A2*_knig.pl8` holds exactly 56 real frames plus four 2 x 2
stubs. `A2_horse.pl8` is 48 frames, eight facings of six, indexed from figure
`+0x11`. **[V]**

### 13.6 Where a figure is drawn

**[V]** `BattleFigure_Draw` (`0x004BDC31`):

```
screen = (mapXY - cameraXY) * tileSize + origin
       + g_walkOffset[facing][walking]
       + (tileSize/2 - spriteWidth/2,  8 - spriteWidth/2)
```

`g_walkOffset` is an 8 x 17 table of `(i32, i32)` at `0x004E4030` for 32-pixel
tiles and `0x004E3BF0` for 16, indexed by facing (`+0x18`) and sub-cell progress
(`+0x32`). Every entry is `(±(32 - 2*step), ±(32 - 2*step))` on the axes the
facing moves along, so all 136 entries collapse to one expression — asserted
against the bytes in `tests/install.rs`.

**This independently confirms the facing numbering.** Facing 0 is north, and its
offset is *positive* y: a figure walking north is drawn trailing to the south of
the cell it is entering. All eight signs are the negation of the facing's own
delta. That is a second source for section 2.1's `dirc` table.

Note the height term uses the sprite **width** for both axes, which is why a
48-pixel man sits 8 pixels left of and 16 above his cell's corner. Reproduced
rather than corrected.

### 13.7 Draw order

**[V]** Per frame: terrain pass, then figures, then a second terrain pass for
cells flagged `0x04` on byte `+1` (tiles that overlap the men), then missiles.

Figures are collected if they lie within one cell of the viewport
(`0x004BD938`), **bubble-sorted by map y ascending** (`0x004BDA92`) and drawn in
that order, so a man lower on the field overlaps one behind him. A stable sort
by y reproduces it.

### 13.8 Figure and cell fields this section adds

New, and not in section 2 or section 3:

| Off | Ev | Meaning |
|---|---|---|
| figure `+0x00` | [V] | pointer to this figure's sprite sheet, set by `0x00480F8B` |
| figure `+0x04` | [V] | pointer to the horse sheet, knights only |
| figure `+0x0E` | [V] | **animation phase**. Counts up and wraps at a bound the state handler chooses: `0x27` walking, `0x17` attacking, `0x5F` dying |
| figure `+0x10` | [V] | **sprite frame index**, what the renderer draws |
| figure `+0x11` | [V] | horse frame index, knights only |
| figure `+0x19` | [V] | a **second** facing byte. `+0x18` drives the sub-cell offset and the **walk** frame; `+0x19` drives the **strike** frame and is the column of the knight table. `+0x0D` is a copy of it, written at the end of every animation handler. **Corrected:** this row used to attach `+0x18` to the attack and `+0x19` to the walk, which is the same swap §14.5 corrects in the frame layout — `Anim_WalkA2` reads `dirc`, `Anim_StrikeA2` reads `dirc2` |
| cell `+2` bit `0x01` | [V] | dirty; the renderer clears it after drawing |
| cell `+2` bit `0x02` | [V] | set on the viewport border |
| cell `+2` bits `0x1C` | [V] | tileset selector: 0 picks `t32_bat1`, 4 picks `t32_bat2`. `Battlefield_BuildFromSkr` clears them, so a field battle only ever uses the first |
| cell `+2` bit `0x80` | [D] | something is drawn on this cell this frame |

Section 2.1's `+0x0C` — "animation phase, seeded as `(index*9 + x*16) & 0x3F +
0xB4`" — is **not** the counter the animation handlers step; they step `+0x0E`.
**It is now established:** `+0x0C` is the *fidget period* and `+0x0B` its
counter, and §2.1 has been rewritten. See also §14.5, which corrects the frame
layout above.

### 13.9 What is not established here

* **The frame rate.** Still open, as section 11 says. Poses advance every four
  ticks and a walk cycle is forty ticks, but nothing converts a tick to a second.
* ~~**Which sprite set (`a2` or `a3`) the battlefield uses at which zoom.**~~
  Settled: the `a3` animation handlers are unreachable — §14.4.
* **Why `Anim_DrawBowA2` reads `g_mapRotation`**, which is the *campaign* map's
  orientation. §14.8. It is the only function in the battle that does.
* **The LFSR seed** a battle starts from, and therefore which grass tile any
  particular cell gets.
* **Missiles, siege engines and the panel.** `A2_miss.pl8`, `Engine.pl8`,
  `Catarm1/2.pl8`, `Misc_bat.pl8` and the 2-pixel minimap tiles are all located
  and none is drawn by us.
* **Nothing has been compared against the original's framebuffer.** Every claim
  here is arithmetic over the binary and the shipped art. The renderer produces
  an indexed 640 x 480 buffer precisely so that comparison stays possible, but
  it has not been made — D8 blocks driving the original's UI, and the proxy-DLL
  route has not been taken this far.

### 13.10 Reproduction

```powershell
# decompile and dump, into tools/view/out/ (gitignored)
powershell -File tools/view/ghraw.ps1 -postScript VBDecomp out.c 0047b8b2 004bdc31
powershell -File tools/view/ghraw.ps1 -postScript VBDump   t.txt bytes 4d7540 1024
```

```bash
# every check in 13.2 - 13.6, headless, no window and no process
LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-view

# watch one
cargo run -p l2-view -- --battle "F:\games\Lords of the Realm II" 1
```

Ghidra scripts live in `ghidra_scripts_view/` (`VBDecomp`, `VBRefs`, `VBDump`,
`VBCallArg`), kept separate from `ghidra_scripts_battle/` so parallel agents do
not edit the same files.

---

## 14. The dispatch tables, read end to end

Sections 1–13 were assembled function by function. This one was assembled by
**constraint propagation**: pick something that cannot lie — a dispatch table
read out of the file — and let each slot's index constrain what the function in
it can be. Where the prediction held, the survivor became an anchor for its
neighbours. Where it failed, the failure is recorded, because that is the only
part of the method that is evidence about the method.

The anchors this section rests on, in order of how much work they did:

1. **`g_troopTickTable` and `g_manStateTable`, read out of `Lords2.exe`.** A
   slot index is a fact, not a guess.
2. **The frame counts of the shipped `.pl8` sheets.** Numbers from outside the
   binary, which is the one thing `decisions.md` C3 could not have had.
3. **The `Cell_TryEnter` return vocabulary**, already **[V]** in §3.
4. **`BattleDebug_Panel`'s own field labels**, §0.

### 14.1 `g_troopTickTable` names itself

`0x004D9140`, eleven function pointers and a null. Slot *t* is the per-frame
handler for troop type *t*, and each one reloads that type's constants into the
figure before tail-calling `g_manStateTable[state]`. The constants are §6.1's
table, and **all eleven handlers agree with all eleven rows**:

| # | function | name | recovery | heavy blow | armour |
|---:|---|---|---:|---:|---:|
| 0 | `0x004825D1` | `TroopTick_Peasant` | 6 | 0 | 0 |
| 1 | `0x004826B0` | `TroopTick_Crossbowman` | 8 | 0 | 12 |
| 2 | `0x00482789` | `TroopTick_Maceman` | 12 | 300 | 12 |
| 3 | `0x00482862` | `TroopTick_Swordsman` | 12 | 100 | 35 |
| 4 | `0x0048293B` | `TroopTick_Pikeman` | 30 | 0 | 35 |
| 5 | `0x00482A14` | `TroopTick_Archer` | 6 | 0 | 0 |
| 6 | `0x00482AED` | `TroopTick_Knight` | 16 | 200 | 25 |
| 7 | `0x00482BC6` | `TroopTick_Catapult` | 20 | 0 | 33 |
| 8 | `0x00482CA8` | `TroopTick_SiegeTower` | 15 | 0 | 35 |
| 9 | `0x00482D8A` | `TroopTick_BatteringRam` | 30 | 0 | 50 |
| 10 | `0x00482E6C` | `TroopTick_Oil` | 8 | 0 | 40 / 25 |

**[V]** — a check that could have failed eleven times over. A wrong table order
scatters the constants across the wrong troops; a wrong reading of §6.1 would
disagree somewhere. Neither happens.

They also settle the `animSet` column §2.2 records as **[D]**: 1, 1, 2, 3, 2, 1,
4, 0, 0, 0, 0. That maps one-to-one onto §6.1's exchange lengths — animSet 1 is
40 ticks, 2 and 3 are 80, 4 is 120, 0 is a siege engine with no exchange.

### 14.2 `g_manStateTable` has eighteen slots and all eighteen are named

`0x004D9170`. The bound is the game's own: every tick handler above tests
`state < 0 || state > 0x11` and calls `BattleMan_Destroy` outside it, and slot
18 is the first entry of `g_battleUnitOrderField` — the two tables are adjacent.

| # | function | name | Ev |
|---:|---|---|---|
| 0 | `0x00482F86` | `BattleMan_StateNone` — `ret` | [V] |
| 1 | `0x00482F91` | `BattleMan_StateDelay` | [V] |
| 2 | `0x004830E9` | `BattleMan_StateDead` | [V] |
| 3 | `0x0048314E` | `BattleMan_StateWalk` | [V] |
| 4 | `0x004831D8` | `BattleMan_StateMelee` | [V] |
| 5 | `0x004832EA` | `BattleMan_StateIdle` | [V] |
| 6 | `0x00483A88` | `BattleMan_StateAttackWall` | [V] |
| 7 | `0x00483CE1` | `BattleMan_LookForMelee` | [V] |
| 8 | `0x00483E55` | `BattleMan_StateChase` | [V] |
| 9 | `0x00483FE1` | `BattleMan_StateFillMoat` | [V] |
| 10 | `0x004842C6` | `BattleMan_StateEngineWalk` | [I] |
| 11 | `0x0048437E` | `BattleMan_StateEngineHalt` | [I] |
| 12 | `0x004843BC` | `BattleMan_StateEngineFire` | [V] |
| 13 | `0x004849FC` | `BattleMan_StateEngineWork` | [I] |
| 14 | `0x00484A0C` | `BattleMan_StateRamGate` | [V] |
| 15 | `0x00484B89` | `BattleMan_StateEngineDead` | [V] |
| 16 | `0x00484BEE` | `BattleMan_StateUnused16` — `ret` | [I] |
| 17 | `0x00484BF9` | `BattleMan_StateCloseToAttack` | [V] |

The **[I]** rows live in `docs/hypotheses.json`, not `symbols.json`.

**§2.2's state list had two entries wrong**, and both are the kind of error a
propagating network catches rather than a reading does:

* **State 6 is not "blocked".** It is a figure hitting the castle wall it just
  walked into. A figure gets there from `BattleMan_Step` when
  `BattleMan_TryStepDir` returns **5**, and §3 already established **[V]** that
  `Cell_TryEnter` returns 5 for cell flag `0x20` against a non-zero side.
* **State 9 is the moat fill**, not "siege-wall movement". `docs/battle-ai.md`
  §5 already said so; §2.2 was never reconciled with it.

Two more states carry the game's own vocabulary. **State 1 is the wait state**:
it counts `delay` (`+0x30`) down and restores `dly state` (`+0x2F`) — the two
labels `BattleDebug_Panel` prints — and `BattleMan_Step` is the writer at the
other end, saving the current state and setting `delay` to `(other & 1) + 1`
when a friendly figure blocks the step. **State 2 is a corpse**: it steps the
collapse animation and counts `+0x173` to 80 before freeing the slot, and state
15 is its siege-engine twin at 120. `+0x173` is a field §2.3 does not list.

### 14.3 How a castle actually comes down

Not written down anywhere before. Two accumulators, and they are not
interchangeable:

| | counter | fed when | threshold | effect |
|---|---|---|---:|---|
| rampart | `g_wallHitsRampart` `0x00554034` | the figure stands on surface **5** | 5,000 | that patch becomes surface 4, the counter **resets**, `g_rampartCellsBreached` + 1 |
| gate | `g_wallHitsGate` `0x00568DA4` | anything else | 20,000 | one-shot: `g_gateBreached` = 1, and `g_siegeApproachScore` and `g_siegeBreachScore` both + 4 |

A man on foot adds **1** per frame; a battering ram in state 14 adds **20**. So
one ram opens a gate in a thousand frames where a lone swordsman needs twenty
thousand. The gate counter never resets, so a siege gets exactly one of those
breaches; the rampart counter does, so a wall can be chewed through repeatedly.
Surface 4 is what `Siege_FindCellSurface4` then hunts for, which is how the
order layer learns the wall is down.

State 14 is reachable **only by a ram**, and that is a fact rather than a
reading: `BattleMan_TryStepDir` sends any siege engine to `Cell_TryEnterEngine`,
whose two leaf tests — `Cell_TryEnterEngineOrtho` and `Cell_TryEnterEngineDiag`,
sweeping the engine's leading edge from `g_engineEdgeOrtho` (three cells for a
orthogonal step) and `g_engineEdgeDiag` (five for a diagonal one, which is what a
3 × 3 footprint exposes, and a second statement of that footprint) — return `6` for a cell flagged `0x20` or `0x40` **only when
`troopType == 9`**. `Cell_TryEnter` itself never returns 6, and `6` is the one
value `BattleMan_Step` turns into state 14.

`Cell_TryEnter` also carries a flag §3 does not list: **`0x08` is impassable for
side 4 and merely occupied for side 0**, and it raises `0x00553F3C` on the way
past. A side-4 figure entering a **surface-7** cell — the bridge — calls
`0x0048551D` first.

### 14.3a `0x00553F3C` is a **third way to win a siege**, and it is not a counter

The flag above is not bookkeeping. `Battle_CheckOutcome` (§7.3) tests it inside
the siege arm and, when it is set, ends the battle with **army A — the besieger
— as the winner**. Its only writer is the `0x08` arm of `Cell_TryEnter`, and it
fires only for a side-4 figure.

So: **an attacker does not have to kill the garrison.** Getting one man to a
cell carrying flag `0x08` wins the siege outright, with the defenders standing.
`[D]` for the mechanism — one write, one read, both unambiguous — and `[I]` for
reading the cell as the keep's door, which is what `Battlefield_BuildCastle`'s
structure codes and `L2.eng` 214/1 (*"Get your soldiers inside the castle to
fight the defenders"*) suggest and neither states.

Nothing before today had this. `armies.md` §7.3 listed the arm as *"the escape
tile flag `DAT_00553F3C`"* without saying who escapes or what it settles; it is
neither an escape nor the defender's.

### 14.3b Flag `0x40` is the drawbridge, and the game's own errata say so

`docs/battle-ai.md` §6.3 describes the defender's routine at `0x00496B9F` as a
one-shot latch that *"scans for any cell carrying flag `0x40`, and if one exists"*
lays down a patch of passable ground — and calls the drawbridge reading `[I]`,
because nothing outside the code said so.

The shipped `Readme.txt` says so. *Drawbridge (pg95)*: **"Note that only the
Stone and Royal castles have drawbridges. Within a siege, drawbridges can not be
closed once they have been opened."** Both halves land: a routine written as a
search that can *fail* is exactly what a feature only two of five castle types
have needs, and *"cannot be closed"* is the latch. **[V]**

The same document settles two more readings in this chapter:

* *Battering Rams*: **"Battering rams are only effective at attacking either
  gatehouses and keeps."** §14.3's two accumulators say the same thing from the
  other side — a ram cannot stand on a rampart, so the only counter it can ever
  feed is the gate's.
* *Siege Towers*: **"Once siege towers reach a wall and 'dock' with it, they can
  not be moved again."** Nothing found in `UnitOrder_SiegeAttTower` or in the
  mover enforces that, and it is recorded here as **unlocated** rather than
  implemented: the handler stops advancing its script on the wall-found path,
  which is not the same rule.

### 14.4 The six `a3` animation handlers are unreachable code

Twelve animation functions sit in `0x00486249 … 0x00488240` in six adjacent
pairs, reached through six sixteen-byte thunks at `0x004861E9 … 0x00486239`.
Every thunk calls the *first* of a pair. The second of every pair has **no
caller and no table entry anywhere in the binary**, and is otherwise the same
code with four constants substituted.

| a2, live | a3, unreachable | what changes |
|---|---|---|
| `Anim_StrikeA2` `0x00486249` | `Anim_StrikeA3` `0x004867CC` | N 12/13/8/10 → 7/8/5/6 |
| `Anim_WalkA2` `0x00486D83` | `Anim_WalkA3` `0x0048702F` | phase wrap `0x17` → `0x0B` |
| `Anim_StandA2` `0x004872AE` | `Anim_StandA3` `0x004875F7` | the same N substitution |
| `Anim_DyingA2` `0x00487908` | `Anim_DyingA3` `0x00487AF6` | base 102/110/70/86 → 57/65/41/49 |
| `Anim_CollapseA2` `0x00487CE4` | `Anim_CollapseA3` `0x00487EB1` | base 96/104/64/80 → 56/64/40/48 |
| `Anim_DrawBowA2` `0x0048804A` | `Anim_DrawBowA3` `0x00488240` | N 13 → 8, base +10 → +6 |

**This is the shape of C3 — six things matching six other things — so it is
anchored outside the binary, in the shipped art:**

* The `a3` sheets hold `8 * N + 13` frames for N = 6, 8, 7, 7, 5, 8: `A3*_psnt`
  61, `A3*_cros` 77, `A3*_mace` 69, `A3*_swor` 69, `A3*_pike` 53, `A3*_arch` 77.
  Those are exactly the six frames-per-facing the `a3` handlers use.
* `Anim_DyingA3`'s bases are `8 * N + 1`, and `13 − 1 = 12` = four half-facings
  of three dying frames, the same twelve as `a2`.
* `Anim_CollapseA3`'s bases are `8 * N + 0` and it plays **one** frame, not the
  six `a2` plays — which is all the remaining space allows.
* `Anim_StrikeA3` writes `horseFrame = dirc` where `a2` writes `dirc * 6`.
  `A3_horse.pl8` has **8** frames; `A2_horse.pl8` has 48.

Five identities over four troop groups, none of them from the code. The **[V]**
claim is that these are the `a3` handlers *and* that nothing calls them. What is
**not** established is why they were compiled in: the skirmish and roster screens
do load the `a3` sheets (§13.4), so something draws them — not this state machine.

### 14.5 Walking and striking were the wrong way round

§13.5 is corrected above. Five agreements, no two of the same kind:

1. `Anim_WalkA2` gives poses 0…5 over a 24-tick loop and is called at the **top**
   of `BattleMan_StateWalk` and `BattleMan_StateChase` — the two states that walk.
2. `Anim_StrikeA2` gives poses from base 6 and is called at the top of
   `BattleMan_StateMelee` and `BattleMan_StateAttackWall` — the two that hit.
3. `BattleMan_StateMelee` calls `Anim_WalkA2` **only** when `BattleMan_Step`
   reports the figure moved; `BattleMan_StateWalk` calls `Anim_StandA2` when it
   did not. Under the old labelling a walking figure would play the attack every
   frame and a duelling figure would attack only while shuffling sideways.
4. `Anim_StrikeA2` plays the cycle only while the figure holds the **attacker**
   role (`+0x185 == 1`) and otherwise shows the standing pose — §6.1's
   attacker/defender mechanic, drawn.
5. The index space closes only one way round. Crossbowmen and archers have
   N = 13: walk 0–5, strike 6–8, stand 9, **bow draw 10–12**. That is where
   `Anim_DrawBowA2`'s `+ 10` points, and there is nowhere else for it to point.

`Anim_WalkA2` reads `dirc` (`+0x18`); `Anim_StrikeA2` reads `dirc2` (`+0x19`).
§13.8's row had those two the other way round as well, for the same reason.

The five cycle tables and four draw curves tile `0x004D9A00 … 0x004D9C08`
exactly — five of `0x28` then four of `0x50` — which is a second reason to
believe the `a3` pairing: the `a3` tables are in the block, sized like the
others, and nothing reads them.

### 14.6 `+0x0C` is the fidget period

§13.8 recorded that `+0x0C` is seeded at creation and is not the counter the
animation steps, and left it there. It has one writer (`BattleMan_Create`) and
two readers, both `Anim_Stand*`. `+0x0B` counts up each frame and, when it
passes `+0x0C`, resets and turns `facingDrawn` one step — one way on an even map
x, the other on an odd one. The seed is `((index * 9 + x * 16) & 0x3F) + 0xB4`,
so **180 to 243 frames**. A rank of men standing still shuffles, and no two
neighbours shuffle together.

### 14.7 The missile flight is a Bresenham line

`Missile_SetupLine` (`0x00493CB9`), called by `Missile_Spawn`, writes `|dx|` to
missile `+0x20`, `|dy|` to `+0x24` and `2 * min − max` to the error term at
`+0x28` — **and 0 when the two are equal**, the perfect diagonal — then snaps
the flight direction `+0x2E` to the nearer octant when one axis is more than
twice the other. The snap moves the *direction* only, never the position; it
matters because `dir` is what the missile coasts along once its line is spent.
`Missile_StepError` (`0x00493B61`) is one error update per sub-step over the
**remaining** counts rather than the original ones (they shrink together, so the
slope it re-derives is the same slope), and it decrements the major axis.
`Missile_StepTowardTargetX` / `…Y` move the 1/32-cell position one unit at a
time. `Missile_OffMap` retires a missile that leaves 0…79 in either axis — the
battlefield bound again. `Missile_LinkToCell` and `Missile_ClearCellLists` are
the two ends of the per-cell missile list §3 records at cell byte `+6`; the link
walk gives up after ten.

**That list is for drawing and nothing else.** Cell `+6` has exactly three
touchers in the whole binary — the two above and the renderer `FUN_004BEED4`,
which walks `head → +0x04 → …` blitting each missile. Hit detection does not use
it: `Missile_Step` reads cell byte `+5`, the figure, directly.

#### The timing is one number wearing two hats

**[V]**, and it is the elegant part. Four sub-steps a tick, one sub-step is
1/32 of a cell, so a tick is exactly **⅛ of a cell** — eight ticks to cross one.
`+0x36` (`ticksFlown`) counts up once per tick and is compared against `+0x38`,
which is loaded with the raw `g_missileStats` range **in eighths of a cell**. So
the same number is both the distance and the tick budget, and `range >> 3` is
the range in cells with no conversion anywhere: bow 120 = 15 cells, crossbow
64 = 8, catapult 160 = 20.

`BattleMan_FireMissile` also runs **eight `Missile_Step`s on the spot** before
the missile is linked to a cell or drawn. Eight ticks is thirty-two sub-steps is
one cell: a missile is born a whole cell out from its shooter, those eight come
out of its range budget, and **a shot can already have hit something before
anybody sees it**.

#### `Missile_Step` does not raise the breach score

**Corrected.** It was reasonable to read it as doing so and it does not. A class-3
(catapult) shot entering a surface-4 cell adds **one** to that cell's own counter
in byte `+0` and becomes class-4 debris; only when the counter passes `0x0F` —
the **sixteenth** hit — does `FUN_0047DFE0` run, and *that* is what turns the
cell to rubble and adds `g_siegeBreachScore` **one per orthogonal neighbour that
is still surface 5**, so 0 to 4 a collapse. The other writers of the score are
the hand and ram gate breaches; none of them is on the missile path.

*Open, and worth someone's time:* `docs/battle.md` and `crates/l2-sim/src/siege.rs`
read surface **4** as *what a breach leaves behind* (`Siege_FindCellSurface4`
hunts for it) and **5** as the rampart. The catapult path shoots **at** surface 4
and scores per adjacent surface 5, which does not fit that reading — it fits
4 = the wall face and 5 = the walkway. Not resolved here; `l2_sim::siege`'s
castle raster is ours, so nothing in this tree turns on it yet.

#### Class 7 is boiling oil, not a falling man

**Corrected.** §0 and `docs/symbols.json` both call missile class 7 a *falling
figure*. The only spawner of class 7 in the binary is `FUN_0047A814`, whose only
callers are the two boiling-oil paths, and what it does is paint a cross of
burning cells around itself every tick — 16 sub-steps, 16 ticks of range, zero
power, invisible to the renderer. There is **no class 6**, and no missile class
anywhere corresponds to a man thrown off a wall. Classes 4 and 5 are catapult
debris and a burning cell.

### 14.8 One thing that stayed coherent and unanchored

`Anim_DrawBowA2` is **the only function in the battle that reads
`g_mapRotation`** — the *campaign* map's orientation, 0/2/4/6 — and it rotates
the figure's facing by it before choosing a frame. Two readings fit equally
well: the battlefield honours a rotation nothing else in §13 implements, or the
routine is a paste from the campaign unit renderer (which does
`facing − g_mapRotation` on the same idiom) and only its `== 0` branch ever
runs. Nothing in the corpus separates them. **Recorded, not narrated** — it is in
`docs/hypotheses.json` under `corrections`, and settling it needs either the
16-pixel renderer read or a live battle.

### 14.9 `g_deterministicBattle` was the wrong name, and the battle layer is where that showed

`0x00553030` is **the multiplayer flag** — a network game is in progress — and it
is now `g_multiplayer` in `docs/symbols.json`, `[V]`. The battle layer is where
the old name became visibly wrong, so the evidence is recorded here:

* It is written to **1 in exactly one place**, `FUN_004B7250`, immediately after
  `FUN_004B7585` succeeds, and to **0 on every failure and teardown path** there
  and in `FUN_004B743B`. Both teardowns then call `vtable+0x24` on the interface
  pointer at `0x004E2A2C`, which `FUN_004B8243` fills from `DirectPlayCreate`.
  `Lords2.exe` imports `DPLAYX.dll`.
* `Net_SendCommand` (`0x0043EDA0`) **returns immediately unless it is set**, so
  every one of the 112 network opcodes is dead in single player.
* `FUN_0043B593` uses it to choose between calling `Battle_Start` locally and
  sending network message `0x3C`, whose serialiser (`0x00444A2F`) packs
  `g_battleApproachLane` and `g_battleRallyGroup` — the two values §8 of
  `docs/battle-ai.md` says are randomised in single player and advanced
  cyclically otherwise.

**The determinism is a consequence, not the subject.** Cycling those two values,
dropping `Battle_UpdateStrengthAdvantage`'s jitter and loading the side-neutral
`TROOPS.ENG` are all things a *networked* game must do so that peers cannot
diverge. The old name asserted the opposite direction of causation and invited
the reading that the original has a determinism switch one could lean on. It
does not; it has a network session, and determinism is what the session costs
it. Nothing in `docs/netcode.md` ever cited the flag — its case for lockstep is
made from first principles about our own engine — so no argument there had to be
withdrawn, but the flag is now positive evidence *for* that case rather than a
name that happened to agree with it: the original ships commands, not state
(`docs/armies.md` §8c), and checksums the result (`Sync_Checksum`).

### 14.10 Reproduction

```bash
# every dispatch table in this section, straight out of the PE
node tools/battle/petable.js "F:/games/Lords of the Realm II/Lords2.exe" 4d9140 12
node tools/battle/petable.js "F:/games/Lords of the Realm II/Lords2.exe" 4d9170 18
node tools/battle/petable.js "F:/games/Lords of the Realm II/Lords2.exe" 4d9a00 50

# the a2/a3 frame identity, from the shipped art rather than the binary
node tools/battle/sheetframes.js "F:/games/Lords of the Realm II"
```

---

## 15. Playing a battle: every input arm of `0x28` … `0x2B`

Sections 1–14 are what a battle *is*. This one is what a **player** can do to
it, and it exists because the input audit (`docs/decisions.md` C61) measured the
battlefield at **0 of 49** and there was no list to build from. Same status
legend: **[V]** verified against a second independent source, **[D]** a reading
of decompiled C, **[I]** inferred.

### 15.1 There are three screens, not four

**[V] Screen `0x28` is unreachable.** Every immediate write of `g_screenId`
(`0x004EAC50`) in the shipped binary was enumerated — 212 `mov byte ptr
[0x004EAC50], imm8` sites, values covering `0x00` … `0x45` — and `0x28` is not
among them, while `0x29`, `0x2A` and `0x2B` all are. No decompiled function
assigns it, and the only indirect writes (`g_screenIdSaved`, `g_menuPrevScreen`,
`DAT_004E65C8`, `DAT_004EAFB0`, `DAT_004F0350`) can restore only a value
`g_screenId` already held. It has a live `Screen_FrameInput` arm and a live
`Screen_Draw` arm, and neither can run — `docs/bugs.md` D37.

Its arm is worth reading anyway, because of what it says the screen *was*:

```c
if (g_screenId == '(') {            /* 0x28 */
    Map_EdgeScroll();
    if (g_mouseRightReleased) { g_screenId = ')'; DAT_0053f238 = 0; DAT_0053e9ac = 1; }
}
```

`DAT_0053F238` is the **pause** word (§15.3), and this is the only place other
than the pause button that clears it. So `0x28` was a look-around-before-it-
starts screen, and leaving it started the fighting. **[I]** on that reading;
**[V]** on the two writes.

| id | what | its arm |
|---|---|---|
| `0x28` | dead | `Screen_FrameInput` `0x0042FF10`, arm `'('` |
| `0x29` | the field | arm `')'` |
| `0x2A` | **the selection drag** | arm `'*'` |
| `0x2B` | the outcome banner | arm `'+'` |

`Screen_HandleInput` (`0x004BA9C8`) has **no arm for any of the four**, so the
battlefield has no widget table: its buttons are a `Hotspot_Test` inside
`Screen_FrameInput`'s own ladder. `Screen_Draw` (`0x0040F1A0`) has arms for
`0x28`, `0x29` and `0x2B` and **none for `0x2A`** — during a drag the screen is
not repainted by the dispatcher at all; `Battle_Frame` paints the field
directly. **[V]**

### 15.2 The screen, in pixels

`Battle_LoadAssets` (`0x004987B7`) fixes the viewport and `BattleMap_Click`,
`FUN_004329A4` and the banner table fix the rest.

```
  x   0 … 479   y  24 … 471   the battlefield, 15 x 14 tiles of 32
  x 480 … 639   y  24 … 183   the overview: the whole 80 x 80 field at 2 px a cell
                y 185 … 404   the banners of the figures you hold
                y 448 … 479   five buttons, 32 x 32 each
```

### 15.3 A battle starts paused, and the pause sound is dead code  **[V]**

`Battle_Start` writes `DAT_0053F238 = 0xFFFFFFFF` before it raises
`g_screenId = 0x29`. Battle button 0 (`FUN_0043B9A1`) toggles that word with a
bitwise NOT, so it flips between `-1` and `0`, and the **first thing a player
does in every battle is press pause, to unpause it**.

While it is set, `FUN_0043C57D` refuses to issue an order, `BattleMap_Click`
refuses to order from the overview, and `Battle_Frame` paints `L2.eng` group 32
index 0 across the bottom of the field through `FUN_00423B4F`.

The same function ends:

```c
DAT_0053f238 = ~DAT_0053f238;
...
if (DAT_0053f238 == 1) { _DAT_005533f0 = 1; Sound_PlayFile("s032_01.wav", 1, 0); }
```

A word that only ever holds `0` or `-1` is never `1`, so **the pause sound never
plays**. `docs/bugs.md` D38.

### 15.4 The five buttons — `DAT_004DC710`, `Hotspot_Test(0x1E0, 0x1C0, …, 5)`

**[V]** The table is five 32 × 32 records at x 0, 32, 64, 96, 128, offset by
(480, 448), so the strip is exactly the panel's width.

| # | handler | what | guard |
|---:|---|---|---|
| 0 | `FUN_0043B9A1` | **pause** | `g_battleChoiceOwner != 0` |
| 1 | `FUN_0043BA29` | **retreat** — `Ui_OpenConfirm(12)` *"Retreat from field?"*, or `Ui_OpenConfirm(11)` *"Surrender castle?"* for a siege garrison | `== 1` |
| 2 | `FUN_0043BBE7` | the **siege gate** — `FUN_00496B9F` | siege, garrison, castle ≥ 3, once |
| 3 | `FUN_0043BD02` | **charge** — `FUN_0047A76D` | `!= 0`, once (`DAT_0055322C`) |
| 4 | `FUN_0043BD67` | **autocalc** — `Ui_OpenConfirm(9)` | `== 1` |

**Retreat and autocalc are the same action.** Both confirms land in
`FUN_0043BE65`, which is `Battle_AutoResolve` and `Battle_ReturnToCampaign(1)`.
And `Battle_AutoResolve` reads the **campaign** records (§4.2) while
`FUN_0043BE65` never calls `Battle_WriteBackCasualties` — so **retreating
discards every casualty the battle has produced** and computes the result from
the armies as they walked on. **[V]**

**Charge is not `Order_ChargeNearest`.** `FUN_0047A76D` walks the local player's
figures of troop type < 7 and sets `unit.halted = 1` and `figure.state = 8`; the
AI's `Order_ChargeNearest` (`0x0048C8AF`) also clears the withdraw flag and the
figures' targets. And it is one-shot: `DAT_0055322C` is set on the first press
and never cleared inside a battle.

### 15.5 Selecting: the drag is a screen  **[V]**

`FUN_0043BF07` (`0x0043BF07`) is four arms in one function, gated on
`DAT_00568964 == 1` (input armed by `Battle_Start`) and `DAT_00553C6C == 0`.

1. **left press, pointer on the field, not already `0x2A`** — record the anchor
   cell (`_DAT_0055CE60/64`) and the anchor pixel (`DAT_0057A0F4/0E8`), set
   `g_screenId = 0x2A`, and set the debug panel's figure to whatever is under
   the press;
2. **button held on `0x2A`, pointer moved** — `FUN_0043C247(player, 0, …)`:
   clear the selection and re-box it, live;
3. **release, or `g_mouseLeftDoubleClick`, on `0x2A`** — `g_screenId = 0x29`,
   then `FUN_00479CF7` classifies the gesture;
4. anything else — decline, and fall through to the next guard.

`FUN_00479CF7` (`0x00479CF7`) returns **1** when the pointer moved 25 pixels or
more on either axis, **2** when it barely moved and the hover says one of your
own men is under it, and **0** otherwise. The three land differently:

* **1** → `FUN_0043C247(player, 1, …)`: clear, box, `FUN_00478987`,
  `FUN_00478F0B`, `Battle_CountMenByType`;
* **2** → the anchor is moved **−8, −8** and the pointer **+8, +8** and *that*
  box is committed, so a click on a man selects a 16-pixel square;
* **0** → **nothing at all**, and in particular the selection is *not* cleared.
  Clearing is the right button's job (§15.7).

The box itself rounds with a quarter-tile tolerance: the near corner rounds up
when it is more than three quarters of the way into a tile
(`FUN_004BC2E5`/`FUN_004BC3AC`) and the far corner rounds down when it is less
than a quarter in (`FUN_004BC346`/`FUN_004BC40D`). And it reads the **occupant
of each cell**, not the sprites the box overlaps, so a man drawn half inside it
and standing outside is not picked. **[V]**

**The commit rearranges the unit array, which is why selection is simulation
state.** `FUN_00478987` (`0x00478987`) asks whether the selection is exactly one
whole unit; if it is not — the player boxed half a unit, or figures from two —
it calls `BattleUnit_Alloc` and moves every selected figure into a **new unit**.
So a box drawn round half a unit *splits* it, and every later order applies to
the new one. Two details reproduced rather than tidied: the new unit's category
is written **inside** the move loop, so the **last** selected figure decides
whether the whole unit is missile (1) or melee (3); and if `BattleUnit_Alloc`
finds no free slot the regroup silently does nothing, at which point
`FUN_00478F0B` (`0x00478F0B`) **truncates the selection to the first figure's
unit** instead. **[V]**

`DAT_00553078`, the count every order arm gates on, is not written by any of
this: `Battle_CountMenByType` (`0x00481B9A`) recounts it from the figure array
every frame. **[V]**

### 15.6 Ordering — `FUN_0043C57D` → `FUN_0043C634` → `BattleUnit_Order`

**[V]** Five guards, all refusals: the pointer must be on the field, it must
**not** be over one of your own men (either hover flag blocks it, which is what
makes a click on a friend a selection and never a destination), the button must
have been *released*, `DAT_00553078` must be non-zero, and `DAT_0053F238` — the
pause — must be clear.

`FUN_0043C634` then plays a troop cry (2 with an enemy under the cursor, 3 on
surface 2, else 1), sets a 30-frame cooldown, and calls

```c
BattleUnit_Order(DAT_0053e984, x, y, g_battleHoverEnemy, DAT_0053e874, 0);
```

on **one unit** — `DAT_0053E984`, which `FUN_00478987` has just made the
selection be.

#### `BattleUnit_Order`'s fifth argument is not `fromPlayer`

`docs/symbols.json` names it that. It is **`DAT_0053E874`, and its only writer is
`Battle_UpdateHover`, which sets it when the hovered cell's surface byte is 15
and clears it otherwise.** All twenty-five AI call sites pass a literal 0. Its
effect inside `BattleUnit_Order` is that a **missile** unit of **side 0** ordered
onto such a cell has `Order_StopShortOfTarget` and `Dest_FindReachableNear`
applied to its destination. **[V]** on the writer and the branch; the name is a
correction, and *why* the game cares about surface 15 specifically is **not
established**.

### 15.7 The right button is a deselect  **[V]**

`FUN_0043C2A9` (`0x0043C2A9`) holds the two panel arms:

* **left press** — walk the local player's selected figures in index order,
  hit-testing rectangles from `DAT_004D31F4`. The base offset is chosen by
  `DAT_00553078`: **0** under 13 figures, **12** under 19, **30** otherwise —
  three layouts of 12, 18 and 50 slots. A hit calls `FUN_0043C4C6`, which
  **drops that figure from the selection**. The loop breaks at
  `0x31 < local_c`, so **only the first fifty banners are clickable** whatever
  the layout;
* **right release** — `FUN_0043C55C` → `FUN_00479A71`, **clear the whole
  selection**. Refused inside `x >= 0x1E1 && 0x18 <= y <= 0xB7`, which is the
  overview panel *minus its leftmost column* — the panel starts at `0x1E0` and
  the guard tests `0x1E1`, so a right click on column 480 deselects.

The three layouts, read out of `DAT_004D31F4` (28-byte records whose first four
`i32`s are `x, y, w, h` for `FUN_004B1DEB`):

| slots | grid | origin | size | pitch |
|---:|---|---|---|---|
| 12 | 3 × 4 | (488, 189) | 45 × 50 | 53 × 55 |
| 18 | 3 × 6 | (488, 186) | 45 × 35 | 53 × 37 |
| 50 | 6 × 9 | (484, 185) | 22 × 18 | 26 × 19 |

### 15.8 The overview panel orders at battlefield scale  **[V]**

`BattleMap_Click` (`0x00432443`) accepts x 480 … 639, y 24 … 183 and maps it to
a cell by `(x − 480) / 2, (y − 24) / 2`. Left button, something selected, no oil
selected and unpaused → `FUN_0043C634`, **a real order from a two-pixel click**.
Anything else, the right button included → the camera goes there
(`cam = cell − 7`).

It is reached from `Screen_FrameInput`'s **epilogue** —
`if ((leftPressed || rightPressed) && g_screenId != 0x12 && FUN_004323FE())` —
which runs after every per-screen arm on every screen but `0x12`. So the
overview is live on `0x29`, `0x2A` and `0x2B` alike.

### 15.9 The keyboard — the window procedure, `0x004B29BE`

**[V]** The battlefield is the only screen in the game with real keyboard verbs,
and they are dispatched from `WndProc` rather than from `Screen_FrameInput`.

| key | arm | gate |
|---|---|---|
| `1` … `9` | `FUN_0043C910` — recall a control group **and put the camera on it** | `g_battlePhase == 2` |
| `Ctrl` + `1` … `9` | `FUN_0043C885` — store one | `g_battlePhase == 2` |
| `H` | `FUN_0043C77A(0)` — form a **line** | `g_battlePhase == 2` |
| `V` | `FUN_0043C77A(1)` — form a **column** | `g_battlePhase == 2` |
| `←` `→` | step `g_debugSelectedMan` | none |
| `F2` | cycle `DAT_00568960`, the debug panel's mode | none, but see below |
| `F3` | toggle `DAT_0055402C`, which is what makes the panel draw | `DAT_0053F684 == 1` |
| `F4` | add one hit to figures 1 … 20 | `DAT_0053F684 == 1` |

`VK_CONTROL` is latched into `DAT_004DF3A8` on key-down and cleared on key-up,
and the digit arm is `if (DAT_004DF3A8 == 0) FUN_0043C910(key); else
FUN_0043C885(key);` — two different functions rather than one with a flag. Nine
groups are reachable (`0x30 < key && key < 0x3A`) and `Battle_Start` clears ten
slots of `DAT_00553400`, stride `0x18`.

**`BattleDebug_Panel` is not on by default**, which §0 leaves open. All three
panel modes open with `if (DAT_0055402C != 0 && g_battlePhase == 2)`, and
`DAT_0055402C` is written only by F3, which is gated on `DAT_0053F684`. So F2
cycles a mode nothing draws unless the debug flag is set.

#### `H` and `V` settle unit `+0x09`  **[V]**

§1 lists `+0x09` as "unnamed and untraced". `FUN_0043C77A` re-issues
`BattleUnit_Order` at the unit's *own* position with the sixth argument set to
1 or 2, and `BattleUnit_Order` stores `facing − 1` there. The **one** reader is
`Formation_ComputeRect` (`0x0048A1C9`), whose whole use of it is

```c
if (g_battleUnits[unit].field_0x9 == '\x01') { g_formationCols = 2; }
```

So it is not a facing: it picks between the troop type's own figures-per-row and
a two-wide column. **Line and column**, which is what the two keys are.

### 15.10 The cursor, and the outcome banner

**[V]** `Battle_Frame`'s ladder for `0x28 ≤ g_screenId ≤ 0x2A` chooses among four
cursor kinds — 0 arrow, 4 move, 5 attack, 6 select — in seven leaves. An enemy
under the pointer beats everything; `0x2A` is always the plain arrow; and with a
selection in hand a pointer **outside** the field still shows the move cursor as
long as it is above y 184, which is the overview panel, where a click really
does order. Note `Battle_UpdateHover` runs *after* the cursor is chosen, so the
original's battle pointer is one frame stale.

`0x2B`'s whole arm is one line: a right release sets `DAT_00568470 = 0x1389`.
`Battle_CheckOutcome` counts that word up once a frame and gives way past 5000,
so the right button **skips** the banner rather than dismissing it.

### 15.11 The count

| screen | arms | note |
|---|---:|---|
| `0x28` | 2 | both dead |
| `0x29` | 23 | 5 buttons, 5 pointer arms, 8 keys, the menu bar, the edge scroll, the two overview outcomes, the cursor |
| `0x2A` | 10 | four of `FUN_0043BF07`, the three drag classes, the two exits, the cursor |
| `0x2B` | 3 | the skip, and the overview's two outcomes |
| **total** | **38** | |

**This is not 49, and the difference is a counting rule rather than a
disagreement about the code.** One (screen, gesture, guard) a player can tell
apart is one arm here; a table of N identical widgets hit-tested in one call is
**one** arm (the menu bar's three titles, the nine control-group keys) and a
ladder with N distinct outcomes is N. C61's 49 was reached with a finer rule
somewhere — most likely the menu bar's titles, the cursor's leaves, or the digit
keys counted individually — and `0x28`'s two are in both totals and should be in
neither.

**`docs/arms.json` holds 33 battlefield records for these 38 arms, and the gap is
the same rule applied from the other end.** This table counts an arm once **per
screen it is live on**, which is how C61 counted (`screens/map.rs`'s table lists
the edge scroll under both `0` and `0x10`); the file counts it once **per
implementation**, because its job is to be compared against the code and the code
has one function. The five that differ are the overview panel, live on all three
screens and one record; the cursor ladder, live on two and one record; and the
drag's three release classes, which are one branch of one function. Both numbers
are in each file so that neither can be quoted without the other.

**One number does agree exactly, and it is the one that would have caught a
disagreement about the code.** C61 records *"Battle: 6 of 6"* right-button arms
missing. There are exactly six, and they are: `0x28`'s dead right release;
`0x29`'s deselect; `0x29`'s right press in the overview, which recentres;
`0x2A`'s cancel; `0x2B`'s skip; and the message scroll's dismissal, which is
every screen's. Two enumerations reaching the same six by different routes is
the cross-check this section could otherwise not have.

`docs/arms.json` is the machine-readable form, and
`crates/l2-game/tests/arms.rs` holds it to the code in both directions.

### 15.12 What is still unknown here

* **`Menu_SaveGame` during a battle.** The menu bar is live on `0x29` and
  `Screen_HandleInput` has no `0x29` arm; whether the original refuses, saves a
  broken file, or saves the campaign underneath was not established.
* **The banner painter's own bound.** The click loop stops at fifty. The painter
  (`FUN_004238B8` / `FUN_004239D5`) is handed a count clamped to `0x50`, and the
  fifty-slot layout has fifty entries; whether it reads past the table for
  banners 50 … 79 was not checked.
* **Why surface 15 specifically** makes a missile unit stop short (§15.6).
* **`DAT_00553C6C`**, which gates the whole selection and order path, and is
  written where nothing here looked.

---

## 16. The ditch, the drawbridge, and what a siege bills the county

The three things a siege *does to the castle*, as opposed to to the men in it. All three
were named in earlier sections and none of them had been followed to the end; doing so
turned up four defects of our own, which are `C81`,
`C82`, `C79` and `C80` in
`docs/decisions.md`.

### 16.1 Two accumulators, and only one of them costs materials  **[V]**

`Siege_RecordCastleDamage` (`0x004784CA`) bills a repair out of two globals, and the whole
of what they mean is which function increments them:

| global | incremented by | what it counts | billed as |
|---|---|---|---|
| `DAT_0057A0D8` | `Moat_Fill` (`0x0047DD86`), **once per cell** | cells of ditch shovelled full | **work only** — 5 man-seasons a cell |
| `DAT_0056D648` | `Wall_Collapse` (`0x0047DFE0`), **once per rampart neighbour** | rampart cells left hanging by a collapse | 15 man-seasons **and** the material |

```c
if (castleLevel < 2) wood  += wallDamage * 10;   /* a palisade is repaired in wood  */
else                 stone += wallDamage * 15;   /* a keep in stone                 */
work += moatFilled * 5 + wallDamage * 15;
```

> **`docs/symbols.md` called `DAT_0057A0D8` `breachDamage`.** It is a misnomer and it was
> load-bearing: read that way, filling a ditch in looks like knocking a wall down, and the
> asymmetry — *shovelling the moat costs the defender labour and not a stick of wood* —
> disappears. The correction is an exhaustive search for writers: three, of which one adds,
> and it is the moat fill. `docs/bugs.md` B69.

**The bill is drawn from one place and it is not `Battle_ReturnToCampaign`.**
`Siege_RecordCastleDamage` has exactly one caller — `FUN_004782C5`, the outcome banner's
frame counter — which runs it at frame 5001, then `Battle_WriteBackCasualties`, then
`Battle_ReturnToCampaign(1)`. So:

* **only a battle watched to its banner bills a repair.** Declining, retreating and the
  Autocalc button all leave for screen `0x13` without reaching that counter, so giving up
  un-does the castle damage exactly as it un-does the casualties;
* the county is billed **before** it can change hands, so a conqueror inherits the wreck and
  the bill;
* `g_multiplayer` skips it entirely, which is not reproduced — see `docs/netcode.md`.

`Siege_RestoreCastleDamage` (`0x004787A4`) is the other half, and it is the **last statement
but one of `Battlefield_BuildCastle`** — so it overwrites the fresh `g_siegeApproachScore =
500` that `Battle_Start` wrote a moment earlier. A besieger thrown off a half-wrecked castle
comes back to its own progress. It restores the *numbers* and not the field.

### 16.2 The moat fill, and why it is four loads  **[V]**

`BattleMan_StateFillMoat` (`0x00483FE1`), slot 9:

```c
if (latched) {
    Anim_Dying();                                    /* the shovelling animation */
    if (cell.surface == 2) {
        if (cell.terrain < g_moatFillSteps)          /* 15 */
            if (++load > (ownerIsHuman ? 100 : 0x50)) { load = 0; cell.terrain++; }
        else { cell.terrain = 0; Moat_Fill(cell); unlatch; }
    } else unlatch;
}
if (!latched) {
    if (Siege_FindCellSurface2(cur) == 0) state = 5;  /* no ditch left within 19 */
    else { Anim_Walk(); BattleMan_Step(0); }          /* go to the next one      */
}
```

**A moat cell takes four loads, not fifteen**, and the reason is that the counter does not
start at zero. `Battlefield_BuildCastle` writes `terrain = 11` — the water id — into every
moat cell and `terrain = 1` into everything else (§3.0), and *this* is what reads the byte
back. So the cell's own terrain id is the fill counter's starting value.

**A fifth `ownerIsHuman` asymmetry**, to add to §6.2's four: the threshold is `100` for a
human's man and `0x50` for an AI's, tested `threshold < counter`, so 101 frames a load
against 81. Four loads is 404 frames against 324 — an AI fills a ditch a quarter faster than
a person does.

`DAT_00553FE4` is the flag that puts a figure in state 9, and **it is the unit's ordered
destination and not the figure's slot**. `Formation_RectIsClear` caches
`(destination.surface == 2)` and then rejects the rectangle for the same reason, so a unit
sent at the ditch has *every* figure enter state 9 while walking to a **dry** slot beside
it. Reading it off the slot instead is what made the state unreachable here for as long as
it existed: no slot is ever chosen on water, because both slot choosers reject an impassable
empty cell.

### 16.3 The drawbridge — `Siege_LowerDrawbridge` (`0x00496B9F`)  **[V]**

Battlefield button 2, the garrison's own. `FUN_0043BBE7` guards it four ways — a siege, the
local player owning **army B**, `g_castleLevel >= 3`, and the `DAT_0052AF9C` latch — and
refuses with `L2.eng` 110 *"Sieges only!"*, 111 *"No drawbridge!"* or 157 *"Drawbridge is
down."*. In multiplayer it sends `Net_SendCommand(0x45, 0)` instead of calling the routine,
which is the original agreeing that this is an **order that enters the simulation**.

The routine scans row-major for the first `flags & 0x40` cell and writes a **7-row by
4-column** patch running south and east from it: `flags = 0`, `surface = 3`,
`frame = DAT_004D9E18[i]`, `flags2 |= 1` then `&= 0xE3`. Then `_DAT_00569588 = 1`,
`DAT_0052AF9C = 1`, four on each siege score, and the two pathfinding planes rebuilt.

`DAT_004D9E18` is **28 `int32`s at file offset `0xD8018`**, read out of `Lords2.exe`:

```
198 198 198 198 | 198 198 197 197 | 198 196 197 197 | 198 195 197 197
193 194 197 197 | 192 197 197 197 | 197 197 197 197
```

Fifteen cells of the filler 197 and thirteen tracing the bridge into the corner nearest the
gate. That shape is the check on the *reading*: 28 `int32`s misread as bytes, or bytes as
`int32`s, would be uniform or noise.

**Those three writes are the same three `BattleMan_StateRamGate` makes on the
twenty-thousandth gate hit.** So the besieger's AI cannot tell a garrison that has opened its
own gate from a gate it broke itself — which is the mechanical reading of the Readme's
*"within a siege, drawbridges can not be closed once they have been opened."*

The scan is missing a `break` on its outer loop; the offset is unaffected and
`g_foundTileX`/`Y` are left at `(0, 0x50)`. `docs/bugs.md` `B85`.

### 16.4 A breach is at ground level  **[V]**

`Wall_Collapse` writes `elevation = 0` over the cell it brings down, and `Wall_SmashAround`
(`0x0049694F`) — the radius-4 square both wall-hitting states run on their threshold —
turns every `0x20` cell in a 9 × 9 into `surface = 5` rubble. A breach is not a step. It
matters because §7's rule allows a difference of one: a hole left at a two-high wall's own
height is a hole nobody can walk through, and the siege runs for ever with its gate open.
