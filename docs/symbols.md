# Named addresses in Lords2.exe

GOG Windows build, 21 Jul 2009, 1,031,680 bytes. `ImageBase = 0x400000`, **no ASLR**, so
every address below is stable across runs and safe to hardcode.

Ghidra project: `E:\dev\ghidra-projects\lords2`. The binary has no symbols; these names
are ours. Where a name is a guess, the confidence column says so.

## The names live in the database too

**[`symbols.json`](symbols.json) is the single source of truth.** The tables in this file
are generated from it, and the same file drives a Ghidra script that pushes the names,
signatures and plate comments into the project, so anyone opening `Lords2.exe` in Ghidra
sees `Merchant_SpawnAll`, not `FUN_00427ed0`.

```powershell
# apply everything in docs/symbols.json to the Ghidra database (idempotent; --dry-run to preview)
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
& "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat" `
    "E:\dev\ghidra-projects" lords2 -process Lords2.exe -noanalysis `
    -scriptPath "E:\dev\lords2\ghidra_scripts" -postScript ApplySymbols.java
```

```bash
node tools/symbols/symbols_md.js           # regenerate the tables below from symbols.json
node tools/symbols/symbols_md.js --check   # fail if this file is out of date
```

**Adding a name is a one-line edit in `symbols.json`**, then those two commands. Do not
hand-edit the tables between the `BEGIN`/`END` markers — `--check` will catch it. Prose
outside the markers is yours to write.

---

## Sprite rendering

<!-- BEGIN symbols.json: sprite -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x0040A21A` | `Pl8_DrawFrame(buf, frame, x, y)` | verified | Reads the frame record at buf + frame*0x10 + 8, range-checks the data offset, calls the clipper, dispatches to a blitter. Independently confirms the 8-byte header / 16-byte record layout inferred from offset arithmetic. Also reads record bytes +12, +13 and +14. |
| `0x0040464C` | `Clip_Horizontal(left, right)` | verified | Computes horizontal clipping and sets g_clipState, g_clipVisibleWidth, g_clipSrcSkip, g_clipDstAdvance. |
| `0x0040477B` | `Clip_Vertical(top, bottom)` | verified | Sets g_blitRowCount to height - rowsClippedAtTop, unconditionally. That is why the byte-0x0D load in Pl8_DrawFrame is dead. |
| `0x004B43B1` | `Blit_Unclipped(buf)` | verified | Copies g_spriteWidth bytes per row, skipping zero bytes; 4x unrolled. Loops g_blitRowCount rows - the clipped row count, not the frame height. Palette index 0 is transparent: this is an engine property, not something in the file format. |
| `0x004B446D` | `Blit_ClippedLeft(buf)` | inferred | Selected when g_clipState is 1. |
| `0x004B44D2` | `Blit_ClippedRight(buf)` | verified | As Blit_Unclipped, but also advances the source pointer by the clipped-off remainder each row. |
| `0x00402A14` | `Glyph_Draw(font, ch)` | verified | Text blitter. Shifts the destination down by frame-record byte 0x0D before clipping, reserving rows above the rectangle. The only consumer of that byte. |

**Globals**

| Address | Name | Confidence | Meaning |
|---|---|---|---|
| `0x005CDD20` | `g_clipState` | verified | Clip state: 0 unclipped, 1 clipped left, 2 clipped right, 5 fully offscreen. |
| `0x00591514` | `g_clipVisibleWidth` | verified | Visible (clipped) sprite width. |
| `0x0059150C` | `g_clipSrcSkip` | verified | Source bytes skipped per row = width - visible width. |
| `0x005CD400` | `g_clipDstAdvance` | verified | Destination advance per row = screen stride - visible width. |
| `0x005C9258` | `g_spriteWidth` | verified | Current sprite width. |
| `0x005AEB70` | `g_spriteHeight` | verified | Current sprite height. |
| `0x0058FE04` | `g_spriteDataOffset` | verified | Current source data offset within the PL8 buffer. |
| `0x004EB274` | `g_screenStride` | inferred | Screen stride, 640. |
| `0x005BB478` | `g_blitRowCount` | verified | Blitter row counter. Not frame-record byte 0x0D - Clip_Vertical overwrites it before every blit, so the byte-0x0D load in Pl8_DrawFrame is dead. |
| `0x004EABD0` | `g_ddrawInterface` | verified | A DirectDraw interface pointer. It was NULL at the crash at 0x004522B5, which happened when the window was deactivated during startup under DxWnd. |

<!-- END symbols.json: sprite -->

**Palette index 0 is transparent.** Every blitter copies a byte only when it is non-zero.
This is a property of the engine, not something recorded in the file format — and it is
the bug our own decoder had until the binary revealed it.

A further family of blitters sits around `0x004BC000`–`0x004BF000`. They also read the
clip state and are presumably variants for other pixel paths. Not yet examined.

## Text (`L2.eng`)

<!-- BEGIN symbols.json: text -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x0040187A` | `Eng_GroupBase(group)` | verified | Returns the byte offset of an L2.eng group, read as the low 24 bits of the u32 at file offset 8 + group*4. See docs/formats/eng.md. |
| `0x00402D37` | `Eng_DrawString(group, index, x, y, font, colour)` | verified | Draws string `index` of L2.eng group `group`: Eng_GroupBase(group), then skip `index` NUL-terminated strings, then skip any bytes below 0x20. The (group, index) pairs in the callers are the best evidence available for what a subsystem is. |

<!-- END symbols.json: text -->

`Eng_DrawString`'s `(group, index)` arguments are the single most useful naming tool in the
binary: find the call, look the string up in `L2.eng`, and the surrounding code names
itself. That is how unit type 3 was identified as the merchant — see
[`formats/plane4.md`](formats/plane4.md) §3.

## File I/O

<!-- BEGIN symbols.json: fileio -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x004AF8D9` | `File_ReadChunk(name, buf, len, offset)` | verified | open / lseek / read / close. Retries twice after chdir on failure - almost certainly the original CD-path lookup. |
| `0x004AF502` | `Restore_WorkingDir()` | verified | chdir back, reporting via getcwd on failure. |
| `0x004184C6` | `Armoury_LoadScreen()` | verified | A worked example of the load-then-draw path: selects a weapon sprite set from a global, reads it into a 250 KB buffer, then calls Pl8_DrawFrame. |

<!-- END symbols.json: fileio -->

## Map loading and rendering

<!-- BEGIN symbols.json: map -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x004676E0` | `Map_InitScenario()` | verified | Whole map bring-up for scenario g_scenarioIndex: Map_LoadPlanes, Map_LoadLattice, Map_LoadTileSets, Map_PlaceDwellings, then Merchant_PickStartCounties. |
| `0x00467770` | `Map_LoadPlanes(slot)` | verified | Reads the six 64x64 byte planes of a map slot, at slot offsets +0x0000 ... +0x5000, in a 64x64 nest (y outer, x inner). Tracks the highest county id below 0x11 into g_countyCount, and dispatches plane 4 to Merchant_RouteAppend (castle tiles) or PlayerStart_Record (settlement tiles). See docs/formats/maps.md. |
| `0x0046797D` | `Map_LoadLattice(slot)` | verified | Reads the trailing 65x129 layer - for(row < 0x81) for(col < 0x41) - storing byte + 0x0FFF0000 into a dword array of row stride 0x104. |
| `0x0046A037` | `Map_LoadTileSets(scenario)` | verified | Selects MAPnn.PL8 from a 16-byte name table indexed by scenario >> 2, and one of four variants per file (the seasons) by scenario & 3. |
| `0x00467A36` | `Map_PlaceDwellings()` | verified | Sweeps the 64x64 grid and, for each tile with a county in 1..17 and plane0 bit 0x20 set, increments a per-county counter and rewrites the tile's graphic index to Roads frame 80, 84 or 104 - the three crop states. maps-layers.md 2.4 reads bit 0x20 as farmland on this evidence. |
| `0x0040526E` | `Map_RenderIso()` | verified | Walks the 65x129 lattice. Cells below 0x0FFF0000 hold a runtime offset into the tile array; cells still holding 0x0FFF0000 + b are off-map surround, where b is the background tile graphic index from the file. |
| `0x004063C1` | `Map_DrawTile(x, y, tileOffset)` | verified | Draws one map tile. bank = tile[+2] & 0x1c selects one of five sprite tables; tile[+3] is the frame index within it; tile[+1] is the flags and tile[+7] the county. |
| `0x00429800` | `Map_LatticeCellForTile(tileOffset)` | verified | Reverse lookup: scans the screen lattice for a given tile-array offset to find its screen cell. |

**Globals**

| Address | Name | Confidence | Meaning |
|---|---|---|---|
| `0x004DA050` | `g_resourceTable` | verified | The game's resource directory: {char name[16]; u32 size;} records. Entries 0-31 are the map tile sets, eight layers x four seasons; 32-63 the zoom-2 sets; 64+ the battle-map sets. See docs/formats/maps-layers.md 1.1. |
| `0x0053F034` | `g_scenarioIndex` | verified | Current scenario. Its low 2 bits select the season variant of the tile set; the rest selects the map slot and the MAPnn.PL8 file. |
| `0x00522F90` | `g_tiles` | verified | Runtime tile array, 4096 records x 8 bytes, index y*64 + x. +1 plane0 flags, +2 bank, +3 frame, +4 plane3, +5 unit index on this tile, +6 saved terrain frame, +7 county. See docs/formats/maps-layers.md 5.3. |
| `0x0055CEA0` | `g_screenLattice` | verified | Runtime 129x65 dword screen lattice, row stride 0x104. Covered cells hold tileIndex*8; uncovered cells keep 0x0FFF0000 + backgroundFrame. |
| `0x0056D5DC` | `g_countyCount` | verified | Highest county id on the loaded map, i.e. the county count. Set by Map_LoadPlanes as max(plane5) over ids below 0x11. |

<!-- END symbols.json: map -->

Map slots are addressed by `File_ReadChunk` with `lseek(index * 0x80C1)` — 0x80C1 is
32,961, the slot stride. See [`formats/maps.md`](formats/maps.md) and
[`formats/maps-layers.md`](formats/maps-layers.md).

**`mapl2.exe` is a dead end.** Despite shipping in the game folder, its strings identify
it as the "L2 Battlemap editor" for `.skr` battle scenarios, and it contains no reference
to `l2_maps.dat` at all. The reference implementation for the campaign map format is
`Lords2.exe` itself. Recorded here so nobody re-investigates it.

## Units, merchants and counties

<!-- BEGIN symbols.json: units -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x0046E1B0` | `Unit_Spawn(type, x, y, owner)` | verified | Allocates the first free slot of the 150-entry unit array g_units (stride 0x1A4) and places it at tile (x, y). Requires the tile to hold no unit and to have plane-0 bits 0xFC clear. Writes the unit index into byte +5 of the runtime tile record and sets bit 0x01 of byte +2. Types seen: 1 = army, 2 = revolting peasants, 3 = merchant, 4 = transport. |
| `0x0046CFBD` | `Map_FindFreeRoadTileNear(x, y, r)` | verified | Scans the (2r+1)^2 box around (x, y), clipped to the map, for a tile with no unit on it and plane-0 bit 0x01 (road) set. Returns 1 and leaves the tile in g_foundTileX / g_foundTileY. |
| `0x0046D130` | `Map_FindFreeOpenTileNear(x, y, r)` | verified | As Map_FindFreeRoadTileNear but accepts plane0 & 0xFD == 0 - plain grass, with or without the county-boundary bit. The fallback when no free road tile exists. |
| `0x00428007` | `County_FindFreeRoadTile(county)` | verified | Map_FindFreeRoadTileNear around the county's anchor tile for r = 1, 2, 3. |
| `0x00428078` | `County_FindFreeOpenTile(county)` | verified | Map_FindFreeOpenTileNear around the county's anchor tile for r = 1, 2, 3. |
| `0x00429153` | `Merchant_RouteAppend(county, route)` | verified | Appends `county` to the first free cell of row `route - 1` of g_merchantRoutes. Called from Map_LoadPlanes for every castle tile with a non-zero plane 4. See docs/formats/plane4.md. |
| `0x004290D0` | `Merchant_ResetRoutes()` | verified | Zeroes the 6x16 g_merchantRoutes table and the 6-byte g_merchantStartCounty array, at the top of Map_LoadPlanes. |
| `0x004291B3` | `Merchant_PickStartCounties()` | verified | For each of the six routes, picks a start county no earlier route has claimed: entry 0, then entries 2, 4, 6..., then 3, 5, 7... after running off the end; entry 1 is never tried and it gives up after five retries. Because g_merchantStartCounty starts zeroed, a row with no free candidate stores 0 - which stops Merchant_SpawnAll dead. On slot 15 (Rorschach) the dedup fails and two merchants share county 8. |
| `0x0042925E` | `Merchant_StartCountyTaken(county)` | verified | 1 if `county` already appears in g_merchantStartCounty. Note it also returns 1 for county 0, because unset entries are 0. |
| `0x00427ED0` | `Merchant_SpawnAll()` | verified | Spawns one type-3 merchant, owner 6 (nobody), on a free road tile near each start county, stopping at the first zero start county. Sets the unit's route index (its name, L2.eng group 5) and route cursor, and counts them into g_merchantCount. 18 of the 44 shipped maps get all six merchants; one gets a single merchant. |
| `0x004280E9` | `Merchant_AdvanceAll()` | verified | Turn phase 6, step 1. For every type-3 unit awaiting a destination, walks row (unitIndex - 1) of g_merchantRoutes cyclically from the unit's route cursor, skipping entries above g_countyCount, and sends the merchant to the next county on its route. It indexes the table by unit index, not by the unit's stored route number - which only works because the merchants occupy unit slots 1..6. |
| `0x0049BBE8` | `PlayerStart_Record(county, slot)` | verified | Writes {county, slot} into g_playerStartTable[slot] and counts non-zero slots into g_playerStartCount. Called from Map_LoadPlanes for settlement tiles with a non-zero plane 4: the player start table. See docs/formats/maps-layers.md 5.1. |
| `0x0041B19D` | `UnitPanel_Draw()` | verified | Draws the info panel for the selected unit. Its Eng_DrawString (group, index) pairs name every unit type: L2.eng group 31 strings 0 'Merchant.', 5 'Revolting peasants.', 2 'Supplies for', 15/16 army; descriptions 12/13/14/15/16. For a merchant it also draws group 5 indexed by the route number - the merchant's name. |
| `0x004296B5` | `Transport_Deliver(unit, county)` | verified | If the type-4 unit's destination county matches, adds its cargo to that county's stores. |
| `0x004292AF` | `Transport_Spawn(owner, county, dest)` | inferred | Creates a type-4 transport near `county` and deducts the cargo from that county's stores. |
| `0x004658C1` | `Unit_EnterOccupiedTile()` | inferred | Resolves what happens when the moving unit enters a tile already holding one. Returns immediately for types 3 and 4, so merchants and transports are never attacked; an enemy transport is captured instead. |
| `0x0046873F` | `Unit_TrampleTile(unit, tileOffset)` | inferred | Damages a crop tile an army passes through in a county it does not own. Begins `if (type != 3 && type != 4)` - merchants and transports do no damage. |
| `0x00468AE2` | `Unit_BurnDwelling(unit, tileOffset)` | inferred | Burns a dwelling an army passes through in a county it does not own, and cuts that county's population. Same type-3/type-4 exemption. |

**Globals**

| Address | Name | Confidence | Meaning |
|---|---|---|---|
| `0x0052F0B0` | `g_units` | verified | Unit array, 150 records x 0x1A4 bytes, index 1..149 (0 unused). +0x00 owner, +0x08 type, +0x0A/+0x0B tile x/y, +0x0C tile offset, +0x10 county, +0x14..+0x17 destination x/y, +0x14F merchant route index, +0x150 'needs a destination', +0x151 destination county, +0x164 merchant route cursor, +0x16C.. cargo. |
| `0x0052AFE0` | `g_lastUnitIndex` | verified | Index of the unit Unit_Spawn just allocated. Callers read it straight after the call to fill in the rest of the record. |
| `0x00569570` | `g_foundTileX` | verified | X of the tile the last Map_FindFree*TileNear search settled on. |
| `0x00569578` | `g_foundTileY` | verified | Y of the tile the last Map_FindFree*TileNear search settled on. |
| `0x00567970` | `g_merchantRoutes` | verified | The six merchant trade routes: 6 rows x 16 county ids, built from plane 4 on castle tiles. Row n is the itinerary of the merchant named by L2.eng group 5 string n. See docs/formats/plane4.md. |
| `0x00569518` | `g_merchantStartCounty` | verified | Six bytes: the county each merchant starts in, chosen by Merchant_PickStartCounties. A zero entry stops Merchant_SpawnAll. |
| `0x005530B4` | `g_merchantCount` | verified | Number of merchants Merchant_SpawnAll actually placed, 1..6. |
| `0x00568DB0` | `g_playerStartTable` | verified | Player start table, {county, slot} byte pairs indexed by the plane-4 value on a settlement tile. See docs/formats/maps-layers.md 5.1. |
| `0x0053E8A4` | `g_playerStartCount` | verified | Number of non-zero player start slots on the loaded map: 5, 4 or 2 across the shipped maps. |
| `0x0053FA1C` | `g_county0_anchorX` | inferred | X of county 0's anchor tile in the county record array (stride 0x300); County_FindFreeRoadTile searches around county*0x300 + this. |
| `0x0053FA1D` | `g_county0_anchorY` | inferred | Y of county 0's anchor tile, as above. |
| `0x00569584` | `g_turnPhase` | inferred | Turn state machine phase, switched on by FUN_0049a010. Phase 6 step 1 is Merchant_AdvanceAll. |
| `0x0053F658` | `g_turnPhaseStep` | inferred | Step counter within the current turn phase; incremented on every FUN_0049a010 call. |

<!-- END symbols.json: units -->

Unit types, from the `L2.eng` group 31 strings `UnitPanel_Draw` selects on them:
**1 = army, 2 = revolting peasants, 3 = merchant, 4 = transport**. Owner byte `6` means
nobody. The merchant half of this subsystem is written up in
[`formats/plane4.md`](formats/plane4.md).

## Battle simulation

The real-time battle. Two arrays at fixed addresses hold the whole thing: **80 units**
(`g_battleUnits`, stride `0x34`) - what the player selects and orders - and **80 figures**
(`g_battleMen`, stride `0x1B0`) - the drawn men, each standing for `g_menPerFigure` real
soldiers. Both are indexed 1 to 80; index 0 is the "none" value.

`BattleDebug_Panel` (`0x00424992`) is the developers own field-name overlay and is the
primary evidence for the field names below - it prints figure and unit fields beside
labels like `hits`, `state`, `dirc`, `re targ` and `orders`.

Written up in full in [`battle.md`](battle.md).

<!-- BEGIN symbols.json: battle -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x004778A0` | `Battle_Start()` | verified | Brings a battle up: builds the battlefield (Battlefield_BuildRandom, Battlefield_BuildFromSkr or Battlefield_BuildCastle), then Battle_InitArmies, then the first unit/figure update passes. See docs/battle.md. |
| `0x0047EFEE` | `Battle_InitArmies()` | verified | Chooses the battlefield size class from the two armies, clears the figure and unit arrays, then raises both sides. Also computes each side's autocalc strength as sum(count[t] * g_troopStrengthWeight[t]) over troop types 0..6. |
| `0x0047FEA7` | `Battle_RaiseSide(armyUnit, outnumbered, side)` | verified | Turns one army record into battle units. Walks the 11 troop types in g_raiseOrder, and for each splits the men into units of at most maxFigures*g_menPerFigure. Siege counts are multiplied by g_menPerFigure so that one engine is one figure. `side` is 4 or 0. |
| `0x004801F8` | `Battle_RaiseSideSiege(armyUnit, outnumbered, side)` | inferred | The castle-defender variant of Battle_RaiseSide: uses g_raiseOrderSiege (8 troop types, no catapult/tower/ram), forces archers and crossbowmen to at most 3 figures per unit, and places units by wall slot rather than by deployment marker. |
| `0x00480599` | `BattleUnit_LoadTroopStats(troopType)` | verified | Copies one row of g_troopBattleStats (0x004D96D0, stride 0x14) into scratch globals, then the matching row of g_missileStats (0x004D97B0) selected by the weapon class in field +0x0C. |
| `0x00480662` | `BattleUnit_Create(siegePlacement, troopType, outnumbered, side, men, owner, mercenary)` | verified | Allocates a battle unit and fills it with ceil(men / g_menPerFigure) figures laid out in a rectangle. Sets the unit category from the troop type, and copies the troop and missile stats into every figure. |
| `0x0046E4C8` | `BattleMan_Create(troopType, x0, dx, y0, dy, owner, unit)` | verified | Allocates the first free slot of the 80-entry figure array g_battleMen (stride 0x1B0), places it on the battlefield, writes its index into cell byte +5 and links it to its unit. Returns 0 when all 80 slots are in use, which silently truncates the army. |
| `0x0046E97A` | `BattleUnit_Alloc(owner, humanControlled)` | verified | Allocates the first free slot of the 80-entry unit array g_battleUnits (stride 0x34); a slot is free when byte +0 (owner) is zero. Leaves the index in g_lastBattleUnit. |
| `0x0046E767` | `Missile_Spawn(owner, x, y, tx, ty)` | verified | Allocates one of the 100 slots of g_missiles (stride 0x4C) and sets its start and target positions in 1/32-cell units. Used for arrows, bolts, catapult shot, thrown oil, fire and falling figures. |
| `0x0046EBAE` | `BattleUnit_Clear(rec)` | verified | Zeroes one 0x34-byte battle unit record. |
| `0x0046EB3F` | `BattleMan_Clear(rec)` | verified | Zeroes one 0x1B0-byte battle figure record. |
| `0x0046F0F0` | `BattleMen_ClearAll()` | verified | Zeroes g_battleMen[1..80]. The loop bound 0x51 is what fixes the figure count at 80. |
| `0x0046F18D` | `BattleUnits_ClearAll()` | verified | Zeroes g_battleUnits[1..80]. |
| `0x0046EBE4` | `BattleMan_Destroy(man)` | verified | Clears the figure off its battlefield cell and zeroes its record. |
| `0x004822ED` | `Battle_UpdateAllMen()` | verified | Per-frame sweep over g_battleMen[1..80]. Applies burning damage on surface 10/17, ages the figure, then dispatches g_troopTickTable[troopType] - which reloads the per-troop stats and tail-calls g_manStateTable[state]. |
| `0x00489401` | `Battle_UpdateAllUnits()` | verified | Per-frame sweep over g_battleUnits[1..80]. Recentres each unit on its figures, and runs its order handler unless byte +1 (human controlled) is set. Three dispatch tables: field, siege attacker, siege defender. Counts field +0x14 down from 500 and re-targets at zero. |
| `0x0047EBE8` | `Battle_UpdateMissiles()` | inferred | Rebuilds the cell -> figure occupancy map for all 80 figures, then steps every missile. |
| `0x00485BB1` | `Missile_UpdateAll()` | verified | Steps g_missiles[1..100]. |
| `0x00492C8B` | `Missile_Step()` | verified | Advances the current missile by its steps-per-tick and, when it enters a cell holding an enemy figure, applies the missile damage formula: elevation scaling, minus the target armour, minus the crossbow-vs-engine reduction, floor 2, accumulated into the target hits pool; every 100 hits kills one man. See docs/battle.md. |
| `0x00494908` | `Melee_Tick()` | verified | One tick of a melee duel. The attacker decrements the defender recovery counter; when it expires the defender takes the attacker melee attack value into its hits pool, and once per exchange the attacker heavy-blow bonus as well. 100 hits kills one man; the attacker role swaps when the exchange counter runs out. |
| `0x0049459A` | `BattleMan_BurnTick(man)` | inferred | Adds 3/6/9/12 hits per tick (1/3/5/7 for siege engines) by battlefield size class to a figure standing on a burning cell, +1/+2 more if the owner is human-controlled. Threshold 100 (160 for engines) kills one man. |
| `0x00481ABB` | `BattleMan_RecomputeStrength(man)` | verified | Sets the figure strength band 0..3 by comparing its remaining men against g_strengthBand1/2/3 (75%, 50%, ~19-25% of a full figure), then loads its melee attack value from g_meleeAttackTable[troopType][band]. |
| `0x0047F42D` | `Battle_LoadStrengthBands()` | verified | Loads the three strength-band thresholds for the current battlefield size class from g_strengthBandTable. |
| `0x00483337` | `BattleMan_FireMissile()` | verified | Missile reload and fire. Acquires a target 10 ticks before the reload interval expires, then spawns a missile whose power is the figure missile damage scaled by its strength band (full, 4/5, 3/4, 1/2). |
| `0x004832EA` | `BattleMan_StateIdle()` | verified | Figure state 5. Advances the animation, and fires if the figure has a missile weapon. |
| `0x00483CE1` | `BattleMan_LookForMelee()` | verified | Figure state 7. If an adjacent enemy exists, locks both figures into state 4 facing each other, one as attacker and one as defender; otherwise falls back to idle or missile search. |
| `0x004831D8` | `BattleMan_StateMelee()` | verified | Figure state 4. Drops out if the opponent is gone or dead, otherwise runs Melee_Tick. |
| `0x00494E3F` | `Melee_FindAdjacentEnemy()` | verified | Tests the eight neighbouring cells in the order N, NW, NE, W, E, SW, SE, S and returns the direction of the first live enemy figure, or 8. Siege engines (troop types 7..10) are never returned. |
| `0x004956CC` | `Missile_FindTarget(range, man, unit)` | verified | Nearest live enemy figure within `range` by Manhattan distance, capped at 160. The unit ordered target is forced to distance 0; siege-engine targets get a +35 distance penalty. |
| `0x00494D08` | `Melee_ResetExchange(man)` | inferred | Sets how long this figure holds the attacking role in a duel, from its animation class: 0, 40, 80, 80 or 120 ticks. |
| `0x00494DF9` | `Melee_ResetRecovery(man)` | inferred | Adds the figure recovery interval (+0x18B) to its recovery counter. That interval is the rate at which the figure can be hit in melee, so it is the real melee defence: peasants and archers 6 ticks, crossbowmen 8, macemen and swordsmen 12, knights 16, pikemen 30. |
| `0x0048F1DD` | `BattleMan_Step(noInterrupt)` | verified | Movement. Waits g_troopBattleStats[type].moveDelay ticks per sub-step, accumulates 2 per step until 17, then takes the next path direction and calls BattleMan_TryStepDir. Knights have delay 0 and pikemen 4, which is why knights are fastest and pikemen slowest. |
| `0x00490616` | `BattleMan_TryStepDir(dir)` | verified | Turns a direction 0..7 into a neighbouring cell offset, clamps at the map edge and calls Cell_TryEnter. |
| `0x00490A44` | `Cell_TryEnter(cellOffset)` | verified | Decides whether a figure may enter a battlefield cell: 1 free, 0 blocked by a friendly figure or flag 0x40, 2 impassable (flags 0x10/0x80, or an elevation change of more than 1), 5 flag 0x20 for a non-zero side, 999 enemy figure present. |
| `0x004972F9` | `Melee_AdjacentEnemyDir(x, y, cellOffset)` | verified | Scans the eight neighbours via g_cellNeighbourOffsets and returns the direction of an enemy figure standing at the same elevation, or 8. |
| `0x0046F434` | `Dir_FromDelta(x, y, tx, ty)` | verified | Eight-way direction from (x,y) to (tx,ty): 0 N, 1 NE, 2 E, 3 SE, 4 S, 5 SW, 6 W, 7 NW, 8 same cell. This is the encoding of the figure facing byte. |
| `0x00404EAD` | `Dist_Manhattan(x, y, tx, ty)` | verified | Returns \|dx\| + \|dy\| and leaves the two absolute deltas in g_absDx and g_absDy. |
| `0x00404F4C` | `Dist_Chebyshev(x, y, tx, ty)` | verified | Returns max(\|dx\|, \|dy\|) and leaves the two absolute deltas in g_absDx and g_absDy. |
| `0x00404E4B` | `Table_Lookup(x, pairs, count, dflt)` | verified | Generic (threshold, value) ladder: returns the value of the first pair whose threshold exceeds x, else the default. Used for the battlefield size class and the men-per-figure scale. |
| `0x0047B8B2` | `Battlefield_BuildFromSkr()` | verified | Loads one 80x80 .skr terrain layer and expands it into the 8-byte-per-cell battlefield, including bridge parts and the two 12-slot deployment marker arrays. See docs/formats/skr.md. |
| `0x0047AAA3` | `Battlefield_BuildRandom()` | inferred | The open-field battlefield generator: writes the same cell array and the same deployment slot arrays as Battlefield_BuildFromSkr, but from the campaign map rather than a .skr layer. |
| `0x0047C4BA` | `Battlefield_BuildCastle(castle)` | inferred | The siege battlefield builder; the third writer of the cell array. |
| `0x0042D806` | `Skr_ReadMap(index, dst)` | verified | Reads one .skr terrain layer: 0x1900 bytes at file offset index * 0x1900 + 0x1674. See docs/formats/skr.md. |
| `0x0048169E` | `Deploy_SlotForUnit(unitOrdinal, side)` | verified | Picks a deployment cell for the n-th unit of a side from the 12-slot marker array: side 0 uses the 0x04 marker slots at 0x00553150, side 4 the 0x0F marker slots at 0x005531B0. There are only 12 slots, so a side with more than 12 units reads past the end. |
| `0x004816F9` | `Deploy_SlotForUnitSiege(unitOrdinal, troopType)` | inferred | The castle-defender placement: maps the troop type to one of the marker slots rather than using the unit ordinal, with a rotating fallback list. |
| `0x00481878` | `Formation_OffsetX(footprint, rows, i)` | verified | Column offset of figure i in a units rectangle: (i / rows) * footprint. |
| `0x00481906` | `Formation_OffsetY(footprint, rows, i, side)` | verified | Row offset of figure i: (i % rows) * footprint, negated for side 0 so both sides face each other. |
| `0x004891AD` | `BattleUnit_Recentre(unit)` | verified | Sets the unit position to the centre of the bounding box of its live figures. |
| `0x00479E90` | `BattleUnit_Order(unit, x, y, attackTarget, fromPlayer, facing)` | verified | Gives a battle unit a destination. Missile units stop short of the target by (range/8 - 3) cells; a unit that mixes missile and melee figures is split into two units. |
| `0x0048143B` | `BattleUnit_Classify(unit)` | verified | Recomputes the unit category from the troop types it still contains: catapult 5, tower 6, ram 7, oil 8, any missile figure 1, otherwise 3. The category selects the order handler. |
| `0x00424992` | `BattleDebug_Panel()` | verified | The developers' own battle debug overlay. It prints figure and unit fields beside their original labels - FIGURE / map x / map y / tg x / tg y / routed / hold it / state / on route / hits / walking / barred / target / dirc / delay / dly state / targeted / selected / selctd seen / polar dirc / mov straff, and GROUP / re targ / map x / map y / targ x / targ y / orders / firing. This is the primary evidence for the field names in docs/battle.md. |
| `0x0042AC0C` | `Troops_Load()` | verified | Parses TROOPS.ENG / TROOPS2.ENG / TROOPS3.ENG into g_troopsTable. The in-memory layout is short[35][2][5][11] - side outer (stride 0x6E), difficulty inner (stride 0x16) - and difficulty groups 0,1,3,4 are then overwritten with Normal at +16%, +8%, -8% and -16% on troop types 0..6 only. |
| `0x0042BF46` | `Skirmish_FillArmies()` | verified | Copies one row of g_troopsTable into the two army records used by the battle, choosing the side by whether the player is the attacker and the difficulty column per player, then recomputes each army total men (+0x168) and autocalc strength. |
| `0x0042B7F7` | `Skirmish_Setup()` | inferred | Sets up a stand-alone skirmish: army records 1 and 2, loads BATTLES.ENG and the TROOPS table, fills both armies. |
| `0x0049005F` | `BattleMen_SwapPlaces()` | verified | When a figure walks into a friendly figure of the same troop type it swaps places with it, carrying position, cell, hits and men across. Returns 2 on a swap. |
| `0x00497437` | `Order_StopShortOfTarget(x, y, tx, ty, range)` | verified | Pulls a move destination back so the unit halts (range - 3) cells from its target, leaving the result in g_foundTileX / g_foundTileY. This is how missile units stop at firing range instead of closing. |
| `0x0047095E` | `Path_Search(mode, x, y, tx, ty)` | verified | The battlefield pathfinder. A uniform-cost breadth-first flood fill over the 80x80 grid with a circular queue: g_pathCost[cell] holds cost+1 from the start, a cell is expanded only after being reached g_pathStepCost[cell] times (that is how terrain cost is charged), and a step is only allowed between cells whose elevation differs by at most 1. Skipped entirely when the target is adjacent or the straight line is clear. |
| `0x00471718` | `Path_SearchSiege(x, y, tx, ty)` | inferred | The variant of Path_Search used by figures in state 9 - the siege/wall path. |
| `0x00472392` | `Path_Extract(x, y, tx, ty)` | verified | Walks the g_pathCost field downhill from the destination back to the start, writing the route into g_pathWaypoints as (x,y) byte pairs and returning its length. Ties are broken towards the straight-line direction. Returns 0 for no path and 150 for adjacent or over-long. |
| `0x00472875` | `BattleMan_SetPath(man, length)` | verified | Copies `length` waypoints out of g_pathWaypoints into the figure own list at +0x38, sets the count at +0x36 and marks the figure "on route". |
| `0x00491A34` | `BattleMan_NextPathDir(man)` | verified | Returns the direction towards the figure current waypoint and consumes it. The list is walked from the end, because Path_Extract builds it backwards from the destination. |
| `0x0048A7D9` | `Dest_FindReachableNear(fromX, fromY, x, y, unit)` | verified | Expanding-ring search of radius 0..19 around (x,y) for a cell with the same surface type and elevation as the source, leaving it in g_foundTileX / g_foundTileY. Used to fix up a move order that lands on unreachable ground. |
| `0x004710F2` | `Path_LineIsClear(x, y, tx, ty)` | inferred | Straight-line passability test; when it succeeds Path_Search does no work at all and the figure walks directly at its target. |
| `0x00471F45` | `Path_BuildBlockedMap()` | inferred | Refreshes the per-cell blocking map the flood fill reads, from the live figure positions. |
| `0x0048B9C1` | `BattleUnit_OrderToEnemyEnd(slot)` | inferred | Sends the current unit to a slot of the opposing sides deployment marker - side 0 to the 0x0F marker, side 4 to the 0x04 marker. Together with Deploy_SlotForUnit this establishes that side 0 deploys at the 0x04 marker and side 4 at the 0x0F marker. |

**Globals**

| Address | Name | Confidence | Meaning |
|---|---|---|---|
| `0x00554480` | `g_battleMen` | verified | Battle figure array, 80 records of 0x1B0 bytes, index 1..80 (0 unused). A figure is one drawn man representing g_menPerFigure real soldiers. +0x09 selected, +0x12 troop type, +0x13 owner is human, +0x18 facing 0..7, +0x1C cell byte offset, +0x20/+0x22 x,y, +0x24/+0x26 target x,y, +0x2C owner (0 = free slot), +0x31 state, +0x172 armour, +0x174 target figure, +0x178 owning unit, +0x17A side, +0x17E melee opponent, +0x189 move delay, +0x18B melee recovery interval, +0x197 strength band, +0x198 heavy-blow bonus, +0x19A hits, +0x19C melee attack, +0x1A0 men left. Full map in docs/battle.md. |
| `0x00566520` | `g_battleUnits` | verified | Battle unit array, 80 records of 0x34 bytes, index 1..80 (0 unused). A unit is what the player selects and orders. +0x00 owner (0 = free slot), +0x01 human controlled, +0x02 figure count, +0x03 side (0 or 4), +0x04/+0x06 first and last figure index, +0x08 category, +0x0F firing timer, +0x14 re-target countdown, +0x1A orders, +0x1E/+0x20 position, +0x22/+0x24 target, +0x2C ordered target figure, +0x30 target cell offset. |
| `0x0057A100` | `g_missiles` | verified | Missile / effect array, 100 records of 0x4C bytes, index 1..100. +0x06 firing figure, +0x08 owner, +0x09 class (1 bow, 2 crossbow, 3 catapult, 5 fire, 7 falling figure), +0x0A/+0x0C position in 1/32 cell, +0x14/+0x16 cell x,y, +0x1C cell byte offset, +0x2F launch elevation, +0x36 distance flown, +0x38 range, +0x40 power. |
| `0x005440E0` | `g_battlefield` | verified | Battlefield cell array, 80x80 records of 8 bytes, index (y*80 + x)*8. +0 terrain id, +1 flags (0x10 and 0x80 impassable), +2 flags, +3 graphic index, +4 elevation, +5 index of the figure standing here, +6 head of the missile list in this cell, +7 surface type (7 bridge, 10 and 17 burning, 15 woodland). |
| `0x0053E8F8` | `g_curBattleMan` | verified | Index of the figure currently being updated. The whole figure state machine reads its subject from this global rather than from a parameter. |
| `0x0052EFF4` | `g_otherBattleMan` | verified | Index of the other figure in the current interaction: the melee opponent, the missile target, or the figure being swapped with. |
| `0x00568458` | `g_curBattleUnit` | verified | Index of the unit currently being updated by Battle_UpdateAllUnits. |
| `0x0053F014` | `g_lastBattleMan` | verified | Index of the figure BattleMan_Create just allocated. |
| `0x005530EC` | `g_lastBattleUnit` | verified | Index of the unit BattleUnit_Alloc just allocated. |
| `0x00568990` | `g_lastMissile` | verified | Index of the missile Missile_Spawn just allocated. |
| `0x0056D5B4` | `g_curMissile` | verified | Index of the missile Missile_Step is advancing. |
| `0x00522F6C` | `g_battleSizeClass` | verified | Battlefield size class 0..8, from the combined army size. It selects g_menPerFigure, the strength-band thresholds, and the burning-damage rate. |
| `0x004EEA9C` | `g_menPerFigure` | verified | How many real soldiers one drawn figure stands for: 4, 8, 16, 32, 64, 128, 256, 512 or 1024 by size class. Reduced for a small army so that a side always gets at least 9 figures. |
| `0x0053E8FC` | `g_battleIsSiege` | verified | Non-zero for a siege: selects the castle battlefield builder, the siege unit-order tables and Battle_RaiseSideSiege. |
| `0x0057C8CC` | `g_localPlayer` | verified | The player index this machine controls. Compared against a unit owner to decide whose units are on the player side. |
| `0x0053E86C` | `g_strengthBand1` | verified | Men in a figure below which it drops to strength band 1 - 75% of a full figure. |
| `0x00522F54` | `g_strengthBand2` | verified | Band 2 threshold - 50% of a full figure. |
| `0x00522F5C` | `g_strengthBand3` | verified | Band 3 threshold - about 19-25% of a full figure. |
| `0x00591560` | `g_absDx` | verified | Absolute x distance left by Dist_Manhattan / Dist_Chebyshev. |
| `0x0058FEA0` | `g_absDy` | verified | Absolute y distance left by Dist_Manhattan / Dist_Chebyshev. |
| `0x004D4B98` | `g_troopStrengthWeight` | verified | Seven ints, the autocalc strength weight of one soldier by troop type: peasant 2, crossbowman 16, maceman 8, swordsman 13, pikeman 9, archer 13, knight 22. Used only for army strength comparisons, not by the real-time battle. |
| `0x004D96D0` | `g_troopBattleStats` | verified | Per troop type, 11 rows of 5 ints: max figures per unit, cell footprint, max figures per formation row, weapon class (index into g_missileStats), move delay in ticks. |
| `0x004D97B0` | `g_missileStats` | verified | Per weapon class, 4 rows of 5 ints: range in 1/8 cells, reload interval in ticks, sub-steps per tick, damage, sprite-bank offset. Class 0 melee (all zero), 1 bow (120, 50, 4, 50, 0), 2 crossbow (64, 100, 4, 200, 8), 3 catapult (160, 100, 4, 200, 16). |
| `0x004D98F8` | `g_meleeAttackTable` | verified | Melee attack value by troop type and strength band, 11 rows of 4 shorts. Peasant/crossbow/archer 5,4,3,2; mace and sword 15,12,10,6; pike 10,8,6,5; knight 20,15,11,6; siege engines all zero. |
| `0x004D9658` | `g_menPerFigureTable` | verified | Nine ints indexed by battlefield size class: 4, 8, 16, 32, 64, 128, 256, 512, 1024 men per drawn figure. |
| `0x004D95D8` | `g_sizeClassLadder` | verified | Eight (threshold, size class) pairs mapping total men to the battlefield size class: 305, 609, 1217, 2433, 4865, 9729, 19457, 38913, then 8. |
| `0x004D9618` | `g_siegeScaleLadder` | verified | Eight (threshold, value) pairs giving how many men one siege engine counts as when choosing the size class. |
| `0x004D9800` | `g_strengthBandTable` | verified | Three ints per battlefield size class: the men-per-figure thresholds for strength bands 1, 2 and 3 - 75%, 50% and about 19-25% of a full figure. |
| `0x004D9870` | `g_raiseOrder` | verified | The order in which troop types become units in a field battle: ram, oil, knight, sword, mace, pike, crossbow, archer, peasant, tower, catapult. |
| `0x004D98A0` | `g_raiseOrderSiege` | verified | The castle-defender raise order, eight troop types: oil, archer, crossbow, knight, peasant, sword, mace, pike. Catapults, towers and rams are absent. |
| `0x004D9140` | `g_troopTickTable` | verified | Eleven function pointers indexed by troop type. Each reloads that troop per-tick constants - animation set, recovery interval, heavy-blow bonus, armour - and then tail-calls g_manStateTable[state]. |
| `0x004D9170` | `g_manStateTable` | verified | Eighteen figure-state handlers indexed by figure byte +0x31. Known: 2 dead, 4 melee, 5 idle/firing, 6 blocked, 7 look for melee, 12 siege engine attacking, 17 closing to attack. |
| `0x004D91B8` | `g_unitOrderTableField` | verified | Five unit order handlers for a field battle, indexed by unit category 0..4. |
| `0x004D91D0` | `g_unitOrderTableSiegeAtt` | verified | Nine unit order handlers for the siege attacker (unit side non-zero), indexed by unit category 0..8. |
| `0x004D91F8` | `g_unitOrderTableSiegeDef` | verified | Eleven unit order handlers for the siege defender (unit side zero), indexed by unit category 0..10. |
| `0x004D8588` | `g_cellNeighbourOffsets` | verified | Eight ints, the battlefield cell byte offsets of the neighbours in direction order 0..7: -0x280, -0x278, +8, +0x288, +0x280, +0x278, -8, -0x288. |
| `0x00516AC0` | `g_troopsTable` | verified | The parsed TROOPS*.ENG table: short[35][2][5][11], row stride 0xDC, side stride 0x6E, difficulty stride 0x16. See docs/formats/eng.md and the correction in docs/battle.md. |
| `0x0051FAE0` | `g_troopsAdvantage` | verified | The per-battle defensive advantage 0..10, one per TROOPS*.ENG row. |
| `0x004D49B8` | `g_troopsRowBase` | inferred | Four ints, 0/10/20/35: the first g_troopsTable row for each battle category. |
| `0x00568210` | `g_battleArmyA` | verified | Index into g_units of the first army in the current battle. It is raised on side 4. |
| `0x00568218` | `g_battleArmyB` | verified | Index into g_units of the second army. It is raised on side 0, and is the side the castle-defender path is applied to in a siege. |
| `0x0054409C` | `g_battleStrengthA` | verified | Autocalc strength of army A: sum of count[t] * g_troopStrengthWeight[t] over troop types 0..6. |
| `0x00553FC0` | `g_battleStrengthB` | verified | Autocalc strength of army B. |
| `0x00553E68` | `g_debugSelectedMan` | inferred | The figure index BattleDebug_Panel prints. |
| `0x00504030` | `g_pathCost` | verified | Pathfinder cost field, u16 per battlefield cell, index y*80 + x. 0 means unvisited, 1 the start cell, 998 (0x3E6) blocked. |
| `0x004FA820` | `g_pathQueue` | verified | Pathfinder frontier, a circular queue of 0x1900 cell indices with head g_pathQueueHead and tail g_pathQueueTail. |
| `0x00503018` | `g_pathQueueHead` | verified | Read cursor of g_pathQueue. |
| `0x004F981C` | `g_pathQueueTail` | verified | Write cursor of g_pathQueue; wraps at 0x1900. |
| `0x004F2770` | `g_pathStepCost` | inferred | Per-cell step cost the flood fill charges: a cell is only expanded once it has been reached this many times. |
| `0x004F6470` | `g_pathVisitCount` | verified | How many times the flood fill has reached each cell, compared against g_pathStepCost. |
| `0x004F7D80` | `g_pathElevation` | inferred | Per-cell elevation the flood fill and Path_Extract read; a step is only allowed between cells differing by at most 1. |
| `0x004F9680` | `g_pathWaypoints` | verified | Scratch route built by Path_Extract: (x, y) byte pairs, at most 150, ordered destination-first. BattleMan_SetPath copies it into the figure. |
| `0x004D6C98` | `g_cellIndexNeighbours` | verified | Eight ints, the battlefield cell-index (not byte) deltas in direction order 0..7: -0x50, -0x4F, +1, +0x51, +0x50, +0x4F, -1, -0x51. |

<!-- END symbols.json: battle -->

## Networking

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x004B7FCE` | call to `DirectPlayEnumerateA` | verified | The only call site. Populates the connection-type list. |
| `0x004B826E` | call to `DirectPlayCreate` | verified | The only call site. |

These two are call instructions, not function entry points, which is why they are not in
`symbols.json`.

The exe imports exactly two functions from `DPLAYX.dll`, by ordinals 1 and 2 — the whole
network entry surface. It also references `sierranw.dll` and `snwvalid.dll` (Sierra's
online matchmaking), but neither ships with the GOG release and neither appears in the
import table.

**That path is reached, not dead.** An earlier revision here called it dead code. In fact
the multiplayer menu reaches it and fails with a modal: *"SNWValid.dll not found in Windows
system folder."* The DLL exists nowhere - neither system folder, nor the game directory.
So the GOG release cannot do its original multiplayer at all, and any attempt to capture a
live DirectPlay session has to get past that check first. See `docs/netcode.md`.

## The game logs its own startup

`Lords2.exe` writes `status.txt` beside itself, narrating initialisation with lines like
`OK :DD Set resolution.` and `ERR:`-prefixed failures. This is the best available anchor
for identifying subsystems: find a log string, find what writes it, and a subsystem is
named.

It also looks for a `sierra.ini` that the GOG release doesn't ship, and logs the failure.
Not known to be fatal.

## Layout

```
.text   0x401000 – 0x4CFB06   ~846 KB of code
.rdata  0x4D0000
.data   0x4D2000             ~1 MB virtual, only 80 KB on disk
                             → ~955 KB of zero-initialised globals: the game state
.idata  0x5CF000
```

That `.data` region is why incremental replacement works: the live game state sits at
fixed addresses and can be read from another process while the game runs.
