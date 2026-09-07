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


## Kingdom layer (counties, seasons, economy)

The turn-based half: counties, population, food, happiness, taxation, seasons, crops,
livestock, industry, castles and army wages. Written up in full in
[`kingdom.md`](kingdom.md).

Two tables in the binary do for this layer what `BattleDebug_Panel` did for the battle:
`g_saveBlocks` (`0x004DE960`) lists every persistent state block with its address and
length, and `g_syncBlocks` (`0x004D5B10`) gives array counts and strides. Between them
they pin `g_counties` at `0x0053F9B0`, 17 records of `0x300` bytes, three independent
ways - and the block list sums to the exact byte size of a shipped `lastturn.sav`.

<!-- BEGIN symbols.json: kingdom -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x0049A010` | `Turn_Tick` | verified | Per-frame dispatcher on g_turnPhase (1..7). Phase 1 neutral-county upkeep, 2 army movement, 3 supply transports, 4 the players' turn, 5 peasant mobs, 6 merchants, 7 end of season. Increments g_turnPhaseStep every call. |
| `0x0049CE51` | `Turn_AdvancePhase` | verified | g_turnPhase++, wrapping 7 -> 1 (and then calling FUN_0049B6D3); resets g_turnPhaseStep and sets the next-phase preview at 0x0053F678. |
| `0x0049B762` | `Turn_AllRealmsDone` | verified | Returns non-zero once every in-play realm's AI step counter (realm +0x00) has reached 999. The exit condition of phase 4. |
| `0x0049A581` | `AI_RunTurnStep` | verified | One step of one realm's turn, round-robin over realms 1..5 through 0x0057C944. Realm +0x00 is a 1..14 program counter into fourteen sub-handlers; at 15 + 2*realm the realm is marked done (999). Skipped entirely when realm +0x05 (human) is set. |
| `0x00448440` | `Season_Advance` | verified | The end-of-season pipeline. Rolls g_seasonPrev/g_season/g_seasonNext and the year (when the season that ended was 4 = Winter), bumps g_turnCount, then calls 28 subsystem passes in a fixed order. This is the whole kingdom economy for one turn. |
| `0x00497CED` | `Game_NewGame` | verified | New-game setup: year 1267/1268, g_season = 3, g_seasonNext = 4, then Map_InitScenario, Merchant_SpawnAll and one immediate Season_Advance - which is why a new game begins in Winter 1268. |
| `0x004983B7` | `Rules_InitConstants` | inferred | Writes the tunable economy scalars into .data at startup: g_grainYieldPerSack = 12, g_grainMaxSacksPerField = 10, g_foodPerHead = 10, g_foodPerSack = 6, g_dairyPerHead = 5, g_grainLabourDivisorAdv = 5, g_grainLabourDivisor = 2. |
| `0x0044B59B` | `Tax_CollectAll()` | verified | Per county: rate = 320 + castle bonus (480/560/640/720/800 for castle types 1..5, i.e. +50/75/100/125/150%); take = Pct(Pct(population, rate), taxRate); credited to the owner realm's gold. Also writes the tax happiness delta 5 - taxRate + realm[+0x28] into county +0x0E. |
| `0x0044B99A` | `Tax_SumEmpireHappiness(realm)` | verified | realm[+0x28] = sum over the realm's counties of county +0x16 - the "Other counties" term of the tax happiness effect (L2.eng group 86 index 4). Sums signed bytes into a signed byte, so a large empire can overflow. |
| `0x0044BAEA` | `Happiness_UpdateAll()` | verified | happiness(+0x0C) = last(+0x0D) + tax(+0x0E) + health(+0x10) + ration(+0x11), copied to the display fields +0x12..+0x14, clamped 0..100, accumulated into +0x1C and averaged into +0x18. Unowned counties below 75 get a flat +5 recorded as "From events". |
| `0x0044BD9F` | `Health_UpdateAll()` | verified | health meter (+0x0B) += g_healthDeltaTable[rationLevel][healthBand], clamped 0..100; band (+0x09) = Table_Lookup(meter, g_healthBandLadder); happiness contribution (+0x10) = g_healthHappiness[band]. |
| `0x00449EF3` | `Population_UpdateAll()` | verified | births = Pct(pop, Pct(g_birthRateLadder(pop), happinessFactor)); deaths = Pct(pop, g_deathRateByHealth[band] + g_deathRateBySeason[season]); pop += births - deaths - emigrants + immigrants. Reproduces the shipped lastturn.sav exactly. |
| `0x0044A6BA` | `Migration_UpdateAll()` | verified | Emigration to the happiest adjacent county: pct = Pct(dHappiness, (100 - happiness)/3), movers = min(100, Pct(pop, pct)), halved for unowned counties. Fills county +0x3C/+0x40/+0x58 and the 16-byte source list at +0x48. |
| `0x0044AA41` | `Unrest_UpdateAll()` | verified | Counts county +0x20 up while happiness is low and down while it recovers; at 4 it calls FUN_004AC185, which raises the revolting-peasant army. Human-owned counties get warning messages 0x96..0x99 as the counter climbs. |
| `0x00449889` | `Weather_UpdateAll()` | verified | Dryness (+0x21D) += seasonal delta (Spring +8, Summer +24, Autumn +12, Winter -12) minus rand/8, doubled for one random county and half again for its neighbours; banded into +0x21B: <5 Flooding, <20 Storms, <70 Cloudy, <95 Sunny, else Drought, with Drought becoming Frost in Winter and Spring. Forced to Cloudy when Advanced Farming is off. |
| `0x0044BFD5` | `Fertility_Update(county)` | verified | county +0x208 += 6 * fallowFields(+0x1FF) - 3 * grainFields(+0x201), clamped to -100..100, forced to 0 when Advanced Farming is off. One fallow field per two grain fields is exactly break-even; cattle fields do not enter it. |
| `0x0044C093` | `Field_ReclaimTick(county)` | verified | Advances the 20 per-county field-progress words at +0x90 towards 800, at most 200 per season - the manual's "never more than a quarter of a field in a single season". Rewrites the tile graphic at each quarter. |
| `0x0044C278` | `Field_ReclaimEstimate(county)` | inferred | The same walk without committing: fills +0xE4 (work remaining) and +0x214 (seasons to the next completed field) for the county panel. |
| `0x0044C8AE` | `Grain_SeasonTick()` | verified | The grain cycle. Entering Spring it sows (Grain_Sow, store -= seed); entering Summer and Autumn it grows; entering Winter it harvests into the store. Weather scales each stage. Also applies the rats/surplus random-event modifier at +0x1FC. |
| `0x0044CFE1` | `Grain_Sow(county, labour, grainStore)` | verified | Largest sacksPerField in 10..1 such that grainFields*sacks fits both the grain store and the available labour (g_grainYieldPerSack * sown / labourDivisor). Sets county +0x1A7 when it had to fall back to a single field. |
| `0x0044D15A` | `Grain_Grow(county, labour, crop)` | inferred | Mid-season crop step, entering Summer and Autumn. |
| `0x0044D1E5` | `Grain_Harvest(county, labour, crop)` | inferred | Harvest step, entering Winter. |
| `0x0044D60D` | `Herd_SeasonTick()` | verified | Herd (+0x250) growth: births/deaths from FUN_0044DA99, plus Pct(herd, g_herdWeatherPct[weather]) and the disease/wolves random-event modifier at +0x1FD. |
| `0x0044DF5F` | `Ration_Apply(county, season)` | verified | Picks the highest affordable ration level 0..5 (None/Quarter/Half/Normal/Double/Triple) by descending from the wanted level, requirement = DivCeil(pop, g_rationTable[l].div) * g_rationTable[l].mul plus garrisons when Armies Eat is on; spends dairy first, then splits the rest between livestock and grain by county +0x15F; writes the ration happiness delta 3*level - 8 to +0x11. |
| `0x0044E5BB` | `Food_FromDairy(county)` | verified | herd * g_dairyPerHead (5). The standing herd feeds five people per head per season without being slaughtered. |
| `0x0044E603` | `Food_HeadsForPeople(county, people)` | verified | DivCeil(people, g_foodPerHead) - one slaughtered animal feeds ten people. |
| `0x0044E653` | `Food_SacksForPeople(county, people)` | verified | DivCeil(people, g_foodPerSack) - one sack of grain feeds six people. |
| `0x0044E7B4` | `Food_Available(county)` | verified | herd*5 + slaughterable*10 + grain*6, the county's total feeding capacity this season. |
| `0x0044EA92` | `Industry_Produce(county, commodity, job, baseEfficiency, divisor)` | verified | One industry pass. output = min(resourceLimit, Pct(workers / divisor, efficiency)); commodity 0 wood (job 7, divisor 1, base efficiency 20), 1 iron (job 5, divisor 1, 15), 2 weapons (job 8, divisor 4, 15), 3 stone (job 6, divisor 2, 15). Weapons debit g_weaponCost from the realm's wood and iron. |
| `0x004508DE` | `Castle_BuildTick` | inferred | Advances castle construction in every county, promoting the castle type and raising its free garrison when the work reaches 100%. |
| `0x00450E46` | `Castle_BuildEstimate(county)` | inferred | Fills the castle panel: work left (+0xF0) and seasons remaining (+0x1A6) from the outstanding work and the labour assigned. |
| `0x004ACBD4` | `Wages_PayAll` | verified | realm[+0xFC] = Wages_ForRealm; if the treasury cannot cover it the realm goes through a five-stage bankruptcy escalation (messages 0xA0/0x10E, 0x11F, 0x10F, then FUN_004AD316) counted in realm +0x158. |
| `0x004AD495` | `Wages_ForRealm(realm)` | verified | Sum of Wages_ForUnit over the realm's armies (unit type 1). |
| `0x004AD52B` | `Wages_ForUnit(unit)` | verified | men(+0x168) / 4 for a human owner; for an AI owner /3, /5, /10, /10 by difficulty. Matches the player-measured 250 men -> 62 crowns and 254 -> 63. |
| `0x00448819` | `Event_RollAll` | verified | Draws a random-event id from g_eventTable for each human-owned county after year 1268 and dispatches one of 24 handlers (0x87..0x8E, 0x12E..0x13D) that write the modifiers at county +0x1FB..+0x1FD or move happiness/health directly. |
| `0x0049D638` | `AI_SetTaxRates(realm)` | verified | Sets county +0xB9 from happiness on one of four ladders (neutral counties, then three by AI personality), and grants the AI its per-turn difficulty bonus: gold from g_aiGoldGrant, plus free population, herd and grain scaled by difficulty. |
| `0x0049DFC6` | `AI_ManageFields(realm)` | inferred | For each of the realm's counties: start reclaiming when the field count is low for the population, then run one of two field-allocation strategies and Ration_Apply. |
| `0x0049AA0E` | `Score_RankRealms` | verified | Recomputes each realm's score at +0x50 from counties, population, castles, armies and treasury, bubble-sorts realms 1..5 into the ranking table at 0x00565410 and writes the rank back to realm +0x2B. |
| `0x004ADE93` | `Save_Write(path)` | verified | Writes every block listed in g_saveBlocks back to back, then appends the sixteen 12,800-byte castle plans from castles.dat. 267,028 + 16*12,800 = 471,828, the exact size of lastturn.sav. |
| `0x0049A453` | `Save_RotateAndWrite` | verified | Rotates safeturn.sav <- old_turn.sav <- lastturn.sav and then calls Save_Write. Called from Game_NewGame and at each turn boundary. |
| `0x004AE8A9` | `Castles_CreateFile` | verified | Creates castles.dat as sixteen zeroed 0x3200-byte blocks, one castle plan per county slot. |
| `0x0043FAA4` | `Sync_CompareState` | verified | Multiplayer desync detector: walks three of the g_syncBlocks descriptors and reports the first record and byte that differ between the two state snapshots. |
| `0x0043FD2A` | `Sync_Checksum` | verified | Sums the same descriptor-selected bytes into a per-frame checksum for the network layer. |
| `0x004116FB` | `Panel_Happiness` | verified | Draws L2.eng group 85 ("Happiness in / Last season / From taxes / From ration / From health / From army / From ale / This Season / Average happiness / From events") next to county fields +0x0D, +0x12, +0x13, +0x14, +0x15, +0x194, +0x17, +0x0C and +0x18. This is what names those fields. |
| `0x004110B1` | `Panel_Population` | verified | Draws L2.eng group 73 next to county +0x28 (last season), +0x30 (births), +0x34 (deaths), +0x38 (army), +0x3C/+0x58 (emigrants to), +0x40 (immigrants) and +0x24 (this season). |
| `0x0041152F` | `Panel_Tax` | verified | Draws L2.eng group 86: "Tax rate" = county +0xB9, "People pay" = +0xC0, "This county" = realm +0x28 + county +0x0F, "Other counties" = county +0x16. |
| `0x00404D6B` | `Pct(x, p)` | verified | x * p / 100. The whole economy is written in this. |
| `0x00404DC1` | `PctOf(a, b)` | verified | a * 100 / b, 0 when b is 0. |
| `0x00404E03` | `DivCeil(a, b)` | verified | (a + b - 1) / b, 0 when b is 0. |

**Globals**

| Address | Name | Confidence | Meaning |
|---|---|---|---|
| `0x0053F9B0` | `g_counties` | verified | The county array: 17 records of 0x300 bytes, index 1..16 (0 unused). Base, count and stride all appear together in the multiplayer sync descriptor at 0x004D5B10 and in the save-block table at 0x004DE960. |
| `0x0057BF00` | `g_realms` | verified | Realm/player array: 6 records of 0x160 bytes, index 1..5. +0x00 AI step, +0x04 in play, +0x05 human, +0x07 lord, +0x28 empire tax happiness, +0x2B rank, +0x50 score, +0xFC army wages, +0x118 gold. |
| `0x0053EA00` | `g_countyFieldTiles` | verified | 17 x 20 u32 tile indices - the map tiles that are this county's farm fields. Save block 12 is exactly 1360 bytes = 17 * 80. |
| `0x004DE960` | `g_saveBlocks` | verified | 225 {u32 address, u32 length} records terminated by a zero length; Save_Write dumps each in order. Sums to 267,028 bytes. |
| `0x004D5B10` | `g_syncBlocks` | verified | Seven 12-byte {count, stride, firstComparedOffset} descriptors used by Sync_CompareState and Sync_Checksum: counties (17, 0x300, 5), realms (6, 0x160, 6), units (151, 0x1A4, 0), battle men (81, 0x1B0, 18), missiles (101, 0x4C, 4), battle units (81, 0x34, 0). |
| `0x004D5B70` | `g_stateArrayPtrs` | verified | Eight pointers in the same order as g_syncBlocks: counties, counties, realms, units, battle men, missiles, battle units, battlefield. No reader was found; it corroborates every base address. |
| `0x0057C934` | `g_season` | verified | Current season 1..4, used directly as the index into L2.eng group 29 (Spring, Summer, Autumn, Winter). Season_Advance sets it before running the economy, so the economy sees the season being entered. |
| `0x0057C92C` | `g_seasonNext` | verified | The season after g_season; the year rolls when the season that just ended was 4. |
| `0x00554470` | `g_seasonPrev` | verified | The previous value of g_season. |
| `0x00553EDC` | `g_year` | verified | Displayed year. 1268 on turn 1. |
| `0x00553E74` | `g_yearNext` | verified | g_year + 1, staged for the next roll. |
| `0x00553240` | `g_turnCount` | verified | Turns elapsed; divides the happiness accumulator to give the running average. |
| `0x0057C93C` | `g_selectedCounty` | verified | The county the county panels display. |
| `0x00554020` | `g_weatherCounty` | verified | The county picked this season for the doubled local weather swing. |
| `0x0053F23C` | `g_optDifficulty` | verified | New-game difficulty 0..3 (easy/normal/hard/impossible, L2.eng group 103 indices 11..14). Scales AI gold, AI free resources and AI army wages. |
| `0x0053F25C` | `g_optAdvancedFarming` | verified | Advanced Farming (L2.eng group 102 index 0). When 0, weather is forced to Cloudy in every county and fertility is forced to 0. |
| `0x0053F260` | `g_optArmiesEat` | verified | Armies Eat (L2.eng group 102 index 3). When 1, garrison and enemy troop counts are added to the county's food requirement. |
| `0x0053F26C` | `g_optTimeLimit` | verified | Turn time limit in seconds, 0 for none. |
| `0x0057C8E0` | `g_grainYieldPerSack` | verified | 12. "Each sack planted will grow into 12 sacks" - the game's own FAQ text, L2.eng group 292 index 4. |
| `0x00552FFC` | `g_grainMaxSacksPerField` | verified | 10. The top of Grain_Sow's descending search; the manual's "up to 5 sacks" is wrong. |
| `0x005533BC` | `g_grainLabourDivisorAdv` | verified | 5. Labour divisor used when Advanced Farming is on. |
| `0x0057D34C` | `g_grainLabourDivisor` | verified | 2. Labour divisor used when Advanced Farming is off. |
| `0x00553F60` | `g_dairyPerHead` | verified | 5. People fed per head of the standing herd, without slaughter. |
| `0x00567594` | `g_foodPerHead` | verified | 10. People fed by one slaughtered animal. |
| `0x0057CB30` | `g_foodPerSack` | verified | 6. People fed by one sack of grain. |
| `0x004D6308` | `g_birthRateLadder` | verified | 20 {population, percent} pairs from (40, 100) down to (3000, 1): the base birth rate falls as a county fills up. |
| `0x004D63A8` | `g_deathRateByHealth` | verified | Percent per season by health band 0..4: 35, 20, 8, 3, 0. |
| `0x004D63C0` | `g_deathRateBySeason` | verified | Percent per season, indexed 1..4: Spring 4, Summer 0, Autumn 2, Winter 8. |
| `0x004D64A8` | `g_healthDeltaTable` | verified | int[6][5]: change to the health meter by ration level 0..5 and health band 0..4. Row 3 (Normal) is the first that is positive. |
| `0x004D6520` | `g_healthBandLadder` | verified | Five {threshold, band} pairs: 10/0, 35/1, 65/2, 90/3, 100/4. |
| `0x004D6548` | `g_healthHappiness` | verified | Happiness per season by health band: -10, -5, 0, +1, +2 for Diseased, Sick, Average, Good, Perfect (L2.eng group 20). |
| `0x004D6560` | `g_herdWeatherPct` | verified | Percent change to the herd by weather 0..5: Frost -2, Drought -10, Sunny +5, Cloudy 0, Storms -5, Flooding -10. |
| `0x004D6738` | `g_rationTable` | verified | Six {divisor, multiplier} pairs: (1,0) None, (4,1) Quarter, (2,1) Half, (1,1) Normal, (1,2) Double, (1,3) Triple - exactly L2.eng group 21. |
| `0x004D6108` | `g_eventTable` | verified | The random-event id ring Event_RollAll draws from; 0 means no event. |
| `0x004D8910` | `g_goodsPrice` | verified | Merchant base price per good id, 15 entries in L2.eng group 6 order: -, grain 2, cattle 12, sheep 0, ale 1, wool 0, iron 1, stone 2, timber 1, pikes 13, bows 16, maces 10, crossbows 24, swords 23, mail 44. |
| `0x004D8950` | `g_goodsStock` | inferred | A second 15-entry table on the same good ids: 1000 grain, 100 cattle, 200 sheep, 100 ale, 500 wool, 100 iron, 100 stone, 200 timber, 500 each weapon. Reads like the quantity a merchant carries. |
| `0x004D8990` | `g_weaponCost` | verified | Six {wood, iron} pairs: crossbow 6/10, mace 4/4, sword 3/10, pike 6/3, bow 13/0, armour 4/18. Debited by Industry_Produce. |
| `0x004D89C0` | `g_castleMaterial` | verified | Five {wood, stone} pairs: palisade 400/40, motte and bailey 800/80, Norman keep 200/1000, stone castle 400/2000, royal castle 800/3000. |
| `0x004D89E8` | `g_castleWorkforce` | verified | Man-seasons per castle type, each value stored twice: 200, 400, 800, 1500, 2500. |
| `0x004D8A10` | `g_castleGarrisonCap` | verified | Troops a castle can hold: 150, 200, 200, 400, 600. |
| `0x004D8A28` | `g_castleTaxBonus` | verified | Percent tax bonus by castle type: 50, 75, 100, 125, 150 - the same ratios as Tax_CollectAll's 480/560/640/720/800 over the castle-less 320. |
| `0x004D8A40` | `g_castleFreeArchers` | verified | Archers a newly finished castle is given: 50, 150, 150, 200, 300. |
| `0x004D8A5C` | `g_aiPersonality` | inferred | Per-AI-lord behaviour parameters, three 0x50-byte rows per lord; the first int of each row selects the tax ladder in AI_SetTaxRates. |
| `0x004DC1E0` | `g_aiGoldGrant` | verified | int[5][4] free gold per turn by AI lord and difficulty for a realm holding three or more counties: 0/0/0/0, 0/400/700/1200, 100/500/800/1400, 0/400/700/1200, 250/600/1100/1800. |
| `0x004DC230` | `g_aiGoldGrantSmall` | verified | The same shape, used when the realm holds fewer than three counties. |

<!-- END symbols.json: kingdom -->

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
