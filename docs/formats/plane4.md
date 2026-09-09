# Plane 4 on castle tiles — the six merchant trade routes

The last open question in [`maps-layers.md`](maps-layers.md) §5.2. That document
established the *shape* of the data — `FUN_00429153` turns plane 4 on castle
tiles into six 16-entry lists of county ids — and could not establish the
meaning, because the sizes correlate with nothing visible in the map data.

**They are trade routes. Each list is the itinerary of one wandering merchant.**

Status legend as elsewhere: **[V]** verified — read directly out of
`Lords2.exe`, or an exact invariant over all 44 shipped maps. **[I]** inferred.

Everything below reproduces with

```bash
node E:/dev/lords2/tools/maps/merchants.js                 # census + invariants, all 44 maps
node E:/dev/lords2/tools/maps/merchants.js "F:/games/Lords of the Realm II" 0   # one slot, routes listed
```

---

## 0. The headline  **[V]**

Plane 4 on a castle tile (`plane0 & 0x40`) is a **merchant route number, 1–6**.
A county's castle occupies four tiles; its three southern quadrants
(`plane3` = 1, 2, 3) each carry a route number or zero, so **a county belongs to
one, two or three of the six routes**.

At load, `Map_LoadPlanes` (`0x00467770`) appends the county id to route
`plane4 − 1` of a 6 × 16 byte table at **`0x00567970`**. After the map is loaded,
`Merchant_PickStartCounties` (`0x004291b3`) chooses a distinct start county for
each route, and at the start of the game `Merchant_SpawnAll` (`0x00427ed0`)
places one **merchant** unit in each of those counties. Every turn,
`Merchant_AdvanceAll` (`0x004280e9`) walks each merchant's route cyclically and
sends it to the next county on its list.

The identification of the unit as a merchant is not a guess — see §3.

---

## 1. How it is stored, and how the table is built  **[V]**

`Map_LoadPlanes` scans the 64×64 grid **y-major, x-inner**, and dispatches on
plane 0's marker bits (this part was already in `maps.md`):

```c
if (plane4 != 0) {
    if      (plane0 & 0x40)  Merchant_RouteAppend(plane5, plane4);   /* castle tile     */
    else if (plane0 & 0x80)  PlayerStart_Record(plane5, plane4);     /* settlement tile */
}
```

`Merchant_RouteAppend(county, route)` at `0x00429153` is four lines of C:

```c
for (i = 0; i <= 15; i++)
    if (g_merchantRoutes[(route - 1) * 16 + i] == 0) {      /* 0x00567970 */
        g_merchantRoutes[(route - 1) * 16 + i] = county;
        return;
    }
```

— append to the first free cell of row `route − 1`. `Merchant_ResetRoutes`
(`0x004290d0`) zeroes the 96-byte table and the 6-byte start-county array before
the scan.

Three consequences, all of which the shipped data respects exactly:

* **Row order is not authored.** It is the order the castles are met in the
  loader's scan — north to south, west to east. England's route 1 comes out as
  `14 → 4 → 7 → 8 → 2`, which is a scan order, not a sensible circuit. The map
  author chooses only *set membership*; the visiting order falls out of castle
  geography. **[V]**
* **A row can hold at most 16 counties**, which is exactly the maximum county
  count, so it cannot overflow. **[V]** 0 overflows over 44 maps.
* **A county can only be appended twice to one row if two quadrants of its
  castle carry the same route number.** No shipped castle does that:
  **0 / 434 castles repeat a route number across their non-zero quadrants**, and
  **0 routes contain a duplicate county** over all 44 maps. The three non-zero
  quadrants are therefore three *distinct* route memberships. **[V]**

### 1.1 Exact invariants over all 44 maps  **[V]**

Output of `tools/maps/merchants.js`:

| Claim | Result |
|---|---|
| counties missing from every route | **0** (all 434) |
| routes containing a duplicate county | **0** |
| route entries that are 0 or exceed the map's county count | **0** |
| routes that overflowed the 16-entry row | **0** |
| castles whose non-zero quadrants repeat a route number | **0 / 434** |
| total route entries | **1033** |

1033 is exactly the plane-4 castle census from `maps-layers.md` §5.2
(`198+196+169+174+171+125`), so the table reproduction is byte-complete.

Counties by how many routes they sit on: **1 route → 42, 2 routes → 185,
3 routes → 207** (434 total). This is the independent cross-check of the quadrant
statistics already in `maps-layers.md` §5.2: the 207 counties on three routes are
exactly the 207 whose *left* quadrant (`plane3 = 2`) is non-zero, and the 42 on
one route are exactly the 42 whose *bottom* quadrant (`plane3 = 3`) is zero. Two
different countings of the same data agree.

---

## 2. What the game does with the table

### 2.1 Choosing the six start counties  **[V]**

`Merchant_PickStartCounties` (`0x004291b3`) runs at the end of
`Map_InitScenario` (`0x004676e0`), immediately after the map, the tile sets and
the dwellings are loaded. For each row it wants a county that no earlier row has
already claimed:

```c
for (row = 0; row < 6; row++) {
    tries = 0; k = 0;
    cand = g_merchantRoutes[row][0];
    while (Merchant_StartCountyTaken(cand) && ++tries < 6) {
        cand = g_merchantRoutes[row][2 + k];      /* note: index 1 is never tried */
        k += 2;
        if (cand == 0) k = 1;                     /* end of row -> restart on odd indices */
    }
    g_merchantStartCounty[row] = cand;            /* 0x00569518 */
}
```

That walk is genuinely odd — it tries entry 0, then the even entries 2, 4, 6, …,
and when it runs off the end of the row it switches to the odd entries 3, 5, 7, …,
never trying entry 1, and gives up after five retries. It is also unguarded
against the value `0`: the start-county array starts zeroed, so
`Merchant_StartCountyTaken(0)` is always true and a row that runs out of
candidates ends up storing `0`.

Two observable consequences, both reproduced by the script:

* **Slot 15 "Rorschach" gives two merchants the same start county** (`1,3,4,8,5,8`).
  1 of 44 maps; the game's own dedup walk fails there. **[V]**
* **A zero start county stops the spawn loop dead** (§2.2), so a map can carry
  non-empty routes that nothing ever walks. **12 such routes** across the 44
  maps — e.g. slot 23 "YinYang" has six non-empty routes but only two merchants.
  **[V]**

### 2.2 Spawning the merchants  **[V]**

`Merchant_SpawnAll` (`0x00427ed0`), reached from the new-game path
`FUN_00497ced` → `Map_InitScenario`:

```c
g_merchantCount = 0;                                    /* 0x005530b4 */
for (i = 0; i <= 5; i++) {
    county = g_merchantStartCounty[i];
    if (county == 0) break;                             /* <- stops, does not skip */
    if (County_FindFreeRoadTile(county) || County_FindFreeOpenTile(county)) {
        Unit_Spawn(3, g_foundTileX, g_foundTileY, 6);   /* type 3, owner 6 */
        unit.hasNoDestination = 1;                      /* +0x150 */
        unit.county           = county;                 /* +0x010 */
        unit.field216         = 100;
        unit.field217         = county;
        unit.routeCursor      = 1;                      /* +0x164 */
        unit.routeIndex       = i;                      /* +0x14f */
        g_merchantCount++;
    }
}
```

`County_FindFreeRoadTile` (`0x00428007`) searches the (2r+1)² box around the
county's anchor tile for **r = 1, 2, 3**, looking for a tile with no unit on it
and `plane0 & 0x01` set — a **road** tile. `County_FindFreeOpenTile`
(`0x00428078`) is the fallback and accepts `plane0 & 0xFD == 0`, i.e. plain grass
with or without the boundary bit.

`Unit_Spawn` (`0x0046e1b0`) allocates the first free slot of the 150-entry unit
array at `0x0052f0b0` (stride `0x1A4`) and — this is the useful bit — writes the
unit's index into **byte +5 of the runtime tile record**.

> This closes an open item in `maps-layers.md` §5.3. That live dump of slot 0
> (England) found *"six tiles whose plane 0 was exactly `0x01` [road] received
> the values 1…6 in byte +5 — six numbered road tiles in six different counties,
> purpose unknown."* They are the six merchants: unit indices 1–6, standing on
> road tiles, in six different counties. The script's simulation says England's
> six start counties are **14, 5, 13, 11, 12, 4** — six different counties, six
> merchants, matching the observation exactly. **[V] code, [V] the six-road-tile
> observation, [I] the specific county list, which was not recorded in that run.**
>
> ### The county list is **[V]** now — the save holds all of it
>
> `g_merchantRoutes` (`0x00567970`), `g_merchantStartCounty` (`0x00569518`) and
> `g_merchantCount` (`0x005530B4`) all fall inside saved blocks, so
> `england-turn1.sav` can be read directly instead of simulated. It says:
>
> ```text
> merchantCount 6      startCounty 14, 5, 13, 11, 12, 4
> route 0  14  4  7  8  2
> route 1   5 12  6  3  8  2  1
> route 2  14 11 13  5 10  9  7  3  1
> route 3  11 12  6 10  4  9  3  1
> route 4  14 11  5 13 12 10  2
> route 5  13  6  4  9  7  8
> ```
>
> **The prediction was 14, 5, 13, 11, 12, 4 and the file says 14, 5, 13, 11, 12, 4.**
> Six for six, from a simulation of `Merchant_PickStartCounties`' odd dedup walk
> written before anyone looked. That upgrades the list to **[V]** and, more
> usefully, it is an independent check on the *walk* — a six-way match is not
> something a wrong reading of that loop produces.
>
> The same save's unit array carries the other half: exactly six units, in slots
> **1 … 6**, all type 3, all owner 6, each standing in the county its row begins
> with, `hasNoDestination = 1` and `routeCursor = 1`. So §2.3's *"the route row
> is `unitIndex − 1`"* — the coupling that only works because merchants get the
> first slots — is checkable against shipped bytes, and it holds.
>
> One thing that falls out and matters elsewhere: **`moveAllowance` (`+0x154`) is
> 0 in all six records.** It is written by the tick handler every frame and
> never persisted, which is the direct evidence for
> [`armies.md`](../armies.md) §2.1a's "tick-maintained rather than initial"
> reading — and a warning for any loader, because a unit restored from a save
> and never ticked cannot move at all.
>
> **All of it is now asserted rather than quoted.** `l2-formats` reads
> `g_merchantRoutes`, `g_merchantStartCounty` and the whole `g_units` array out
> of a save, and `crates/l2-formats/tests/save_england_turn1.rs`'s
> `england_ships_six_merchants_on_the_six_routes_plane4_predicted` and
> `the_six_units_are_merchants_waiting_in_their_start_counties` check every
> number in the block above against the file. `tests/save.rs` then checks the
> shape of the table, the merchant count and the slot-1…n coupling over **every**
> save the machine can reach, which is what makes the coupling an invariant
> rather than an observation about England.

Merchant count per map, from the simulation (an upper bound — a spawn also
requires a free tile, which is never a problem on the shipped maps but is not
proven):

| merchants | maps |
|---:|---:|
| 1 | 1 |
| 2 | 5 |
| 3 | 2 |
| 4 | 6 |
| 5 | 12 |
| 6 | 18 |

This is the answer to the sizes question in `maps-layers.md` §5.2. The sizes do
not correlate with player count or county count because they are not about
players at all — "Equalizer" (`4,0,0,0,0,0`) is a 4-county map with **one**
merchant doing a fixed circuit of all four; "China" (`8,8,8,8,8,8`) is a
16-county map with **six** merchants on six eight-county circuits.

### 2.3 Walking the route  **[V]**

`Merchant_AdvanceAll` (`0x004280e9`) is step 1 of phase 6 of the turn state
machine (`FUN_0049a010`). For every unit of type 3 that is waiting for a
destination:

```c
for (u = 1; u <= 150; u++) {
    if (unit[u].type != 3) continue;
    if (!unit[u].hasNoDestination) { ...move on... }
    dest = 0;
    for (n = 0; dest == 0 && n < 16; n++) {
        dest = g_merchantRoutes[(u - 1) * 16 + unit[u].routeCursor];
        if (dest > g_countyCount) dest = 0;              /* 0x0056d5dc */
        if (++unit[u].routeCursor > 15) unit[u].routeCursor = 0;
    }
    if (dest && County_FindFreeRoadTile(dest)) {
        unit[u].destX = g_foundTileX; unit[u].destY = g_foundTileY;
        unit[u].destCounty = dest;
        unit[u].hasNoDestination = 0;
    }
    ... pathfind and step ...
}
```

Two details worth recording:

* **The route row is `unitIndex − 1`, not the stored route number.** The unit's
  own `routeIndex` field (used for its *name*, §3) is ignored here. This only
  works because the merchants are spawned before any other unit exists and
  therefore occupy unit slots 1…6. It is a real coupling in the original engine,
  not an artefact of decompilation. **[V]**
* The cursor starts at **1**, so a merchant's first destination is the *second*
  county on its list — which is usually not the one it was spawned in, because
  the start county came from `Merchant_PickStartCounties`' dedup walk rather than
  from entry 0.

---

## 3. Why "merchant" is not a guess  **[V]**

Unit type 3 is identified from the game's own UI text, three independent ways.

**(a) The unit info panel.** `UnitPanel_Draw` (`0x0041b19d`) picks a title string
id and a description string id from the unit type, then draws them with
`Eng_DrawString(group, index, …)` (`0x00402d37`) — which is exactly the `L2.eng`
group/string lookup documented in [`eng.md`](eng.md): `Eng_GroupBase(group)` at
`0x0040187a` reads the 24-bit group offset, then `index` NUL-terminated strings
are skipped.

| unit type | title `(31, n)` | description `(31, n)` | icon frame |
|---:|---|---|---:|
| 1 | *(the owner's name, from group `0x5d + owner`)* | 15 `"This is an enemy army."` / 16 `"This is one of your armies."` | 38 / 39 |
| 2 | 5 `"Revolting peasants."` | 13 `"These starving revolutionaries…"` | 37 |
| **3** | **0 `"Merchant."`** | **12 `"Merchants allow a county to buy needed supplies and raise revenue by selling goods."`** | 35 |
| 4 | 2 `"Supplies for"` + county | 14 `"This transport is moving goods from one county to another."` | 36 |

**(b) Merchants have names, indexed by route number.** For a type-3 unit the
panel additionally draws `Eng_DrawString(5, unit.routeIndex, …)`, and `L2.eng`
group 5 is:

| route | merchant |
|---:|---|
| 1 | Jock McTooth |
| 2 | Bob the Shop |
| 3 | Fat Barry |
| 4 | Olde George |
| 5 | Honest Jim |
| 6 | Bernard Slap |
| — | *Weasel Willy* (unused) |
| — | *Little Frank* (unused) |

Eight names ship; only six can ever be used, because the table has six rows.
**Route `n` is always the same named merchant on every map.** **[V]**

**(c) Merchants are non-combatants.** `FUN_004658c1`, which resolves what happens
when a moving unit enters an occupied tile, returns immediately for types 3 and 4
— they cannot be attacked. `FUN_0046873f` (crop trampling) and `FUN_00468ae2`
(dwelling burning) both begin `if (type != 3 && type != 4)`. A merchant walks
through a hostile county without a fight and without doing damage, which is what
the game's merchants do.

Owner is `6`, the same owner byte the engine uses for player index 0 (unowned) —
merchants belong to nobody. **[V]**

The published game documentation agrees at the level it goes to: there are "a
preset number of merchants per map", they "travel the land", and "trade may be
conducted only when a merchant is present in a county"
([manual](https://sierrahelp.com/Documents/Manuals/Lords_of_the_Realm_II_-_Manual.pdf),
[OpenLotR2 docs](https://openlotr2.readthedocs.io/en/latest/game/part-3/Part-3.html)).
No published source documents `L2_maps.dat` at all, let alone plane 4 — this is
new. It was searched for first (CLAUDE.md rule 3 / decisions C5); the OpenLotR2
project documents `.skr` but not the campaign map.

---

## 4. Summary of the encoding

For a map author, the encoding is:

```
county castle 2x2 block, plane 4 per quadrant:

        plane3=0 (top)      always 0
        plane3=1 (right)    route number 1..6, never 0   -> every county is on >= 1 route
        plane3=2 (left)     route number or 0
        plane3=3 (bottom)   route number or 0
        (the non-zero ones are always distinct)
```

and on settlement tiles (`plane0 & 0x80`) plane 4 is unrelated — it is the
player-start table, already settled in `maps-layers.md` §5.1.

The number of merchants a map gets is *not* stored. It is
`min(6, number of routes that survive Merchant_PickStartCounties' dedup walk
before the first zero)` — an emergent property of the route table and of a
slightly buggy selection loop.

---

## 5. What is still open

* **The trade transaction itself.** What a merchant offers, at what price, and
  how the county trade screen decides a merchant is present, was not traced.
  Only the route/movement half of the merchant system is covered here.
* **`unit.morale = 100`** set at spawn (`+0x166`). Still nothing reads it back —
  the panel never draws a merchant's morale — so `100` remains a value with no
  consumer. **[I]** that it means anything at all.
* ~~**`+0x167` is a destination county for transports and unread for
  merchants.**~~ **It is the county the unit was spawned in**, and the save says
  so: `Merchant_SpawnAll` writes `g_merchantStartCounty[i]` there once, nothing
  updates it, and in `siege-sieging.sav` the three merchants carry **3, 4, 2** —
  the start-county array exactly — while standing in counties 4, 3 and 3. It is a
  birthplace, not a position, and it is the *same byte* an army uses for its
  county-defence mark (`docs/armies.md` §1.5, §8.1). **[V]**, over every save the
  machine can reach.
* ~~**No runtime confirmation.**~~ **Confirmed from the game's own saves**, which
  is the oracle this section was waiting for and did not expect to get without
  driving the game. `england-turn1.sav` holds `g_merchantRoutes`,
  `g_merchantStartCounty` and `g_merchantCount` verbatim: England's six start
  counties are **14, 5, 13, 11, 12, 4**, the six route rows are the six derived
  here, and the six units in slots 1…6 are type 3, owner 6, one in each of those
  counties with `nameIndex` 0…5 and a route cursor of 1. The **names** half of
  the prediction — Jock McTooth through Bernard Slap — is still unobserved,
  because a save stores the index and not the string.
* **Slot 15 "Rorschach"** should show two merchants starting in the same county
  (county 8). Still falsifiable, still unobserved: no fixture is that map.
