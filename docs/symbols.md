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
| `0x004676E0` | `Map_InitScenario()` | verified | Whole map bring-up for scenario g_scenarioIndex: Map_LoadPlanes, Map_LoadLattice, Minimap_Load, Map_PlaceDwellings, then Merchant_PickStartCounties. |
| `0x00467770` | `Map_LoadPlanes(slot)` | verified | Reads the six 64x64 byte planes of a map slot, at slot offsets +0x0000 ... +0x5000, in a 64x64 nest (y outer, x inner). Tracks the highest county id below 0x11 into g_countyCount, and dispatches plane 4 to Merchant_RouteAppend (castle tiles) or PlayerStart_Record (settlement tiles). See docs/formats/maps.md. |
| `0x0046797D` | `Map_LoadLattice(slot)` | verified | Reads the trailing 65x129 layer - for(row < 0x81) for(col < 0x41) - storing byte + 0x0FFF0000 into a dword array of row stride 0x104. |
| `0x0046A037` | `Minimap_Load(slot)` | verified | Loads the two 128x128 minimap rasters for map slot 0..59 out of MAPnn.PL8: name = "map01.pl8" + (slot>>2)*0x10, frame (slot&3)*5 -> g_minimapCounty (county id per pixel) and frame (slot&3)*5+1 -> g_minimapPixels (the picture). Each MAPnn.PL8 therefore carries four slots, and the 11 shipped files cover exactly the 44 used slots. It was named Map_LoadTileSets and described as picking a season variant; it does neither - the tile sets are loaded by Gfx_LoadCountyMode and the season is a separate global. |
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


## Battle AI (unit order handlers)

The half of the battle that decides what an *unordered* unit does. `Battle_UpdateAllUnits`
dispatches one of 25 order-handler slots by unit category and battle kind; those slots hold
**17 distinct functions plus one empty stub**. They read a single global mood
(`g_aiStrengthAdvantage`), a per-unit script counter (the debug panel's `orders`) and a
memory of who last hit the unit, and they write only the unit's destination and its
figures' states.

Written up in full in [`battle-ai.md`](battle-ai.md).

<!-- BEGIN symbols.json: battleai -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x0048A9C7` | `UnitOrder_None()` | verified | The empty order handler: an immediate return. Seven of the 25 dispatch-table slots point at it - field category 0, siege-attacker 0 and 8, siege-defender 0, 5, 6 and 7 - so a unit of one of those categories never acts on its own. See docs/battle-ai.md. |
| `0x0047FC01` | `Battle_UpdateStrengthAdvantage()` | verified | Recomputed every 101 frames from Battle_UpdateAllUnits. Sums surviving men over all live figures, weighted 1/2/3/3/2/2/4 by troop type 0..6 and 1 for siege engines, split by figure byte +0x13 (owner is human). Leaves (aiMen * 100 / humanMen) - 100 in g_aiStrengthAdvantage, plus a -10..+21 jitter when g_deterministicBattle is zero. Every order handler branches on that one number. |
| `0x00488DFE` | `BattleUnits_RebuildFromFigures()` | verified | Runs while g_battlePhase == 2. Rebuilds every unit's owner, figure count and first/last figure index from the live figure array; ages the unit timers +0x0C (recently hit), +0x0E (order lock), +0x0F (firing) and +0x2D; sets +0x0D when any figure is in melee and +0x11 when any is on the wall; and, when a figure raises its was-hit flag +0x15, records the attacking figure's unit in +0x0A and sets +0x0C to 50. It runs once per frame from Battle_Frame, so that grudge lasts 50 frames against a 200-frame think interval. This is where the AI's memory of who is hitting it comes from. |
| `0x0048F079` | `BattleUnit_JoinMelee(unit)` | verified | Called from BattleMan_Step on both units the moment a figure walks into an enemy. Puts every other live, non-melee figure of the unit into state 8 (free pursuit). Skipped for human-controlled units, for missile units (category 1) and while the unit's order lock +0x0E is non-zero - so an AI melee unit piles in on contact and a player's unit does not. |
| `0x00481B9A` | `Battle_CountMenByType()` | inferred | Census of surviving men. Among figures whose owner is not human it totals all men into g_aiMenTotal, crossbowmen plus archers into g_aiMenMissile and knights into g_aiMenKnight; it also totals each army's men separately. The field and siege handlers read those three to decide when their side has run out of a troop class. |
| `0x0048A9D2` | `UnitOrder_FieldMissile()` | verified | Field battle, unit category 1 (archers, crossbowmen). Thinks once per 200 frames and not at all while any of its figures is in melee. Above the aggression threshold it shoots the nearest enemy unit if it was hit in the last 50 frames, otherwise advances halfway, holds, and advances again on an `orders` schedule. Below it, it shoots its attacker, answers a rally request, or sits on rally waypoint 0 every fourth think - and once it has been hit more than ten times it backs two cells away from its attacker. |
| `0x0048ACD2` | `UnitOrder_FieldFoot()` | verified | Field battle, unit category 2 (peasants, pikemen). Above the aggression threshold it marches to the enemy end of the field for ten thinks then charges. Below it, being attacked either raises g_aiCommitCounter (by 3 when the side is over its engagement budget, by 20 when missile troops fall under an eighth of the army) or makes the unit back away and call the missile units onto its attacker. It then charges any enemy unit of three or more figures within eight cells, and otherwise returns to rally waypoint 2 every fifth think. |
| `0x0048B02B` | `UnitOrder_FieldMelee()` | verified | Field battle, unit categories 3 (macemen, swordsmen) and 4 (knights): both dispatch slots hold this one function. Identical in shape to UnitOrder_FieldFoot, with thirteen marching thinks rather than ten, a nine-cell charge radius rather than eight, a rally every eighth think, and a limit of two withdrawals per unit. |
| `0x0048D16E` | `UnitOrder_SiegeAttMissile()` | verified | Siege attacker, unit category 1. Field corner on think 0, a castle approach point every fifteenth think, and from think 11 an alternation of two staging routines on an eleven-of-sixteen rotation. Once there is a breach it shoots into it, or moves onto the castle objective when it cannot. |
| `0x0048D412` | `UnitOrder_SiegeAttFoot()` | verified | Siege attacker, unit category 2. The longest of the order scripts: a ladder of `orders` thresholds (8, 18, 25, 31, 40, 51, 60, 71) alternating two staging positions, with a jump that sets `orders` to 100 when the castle layout flag is set. Charges outright once g_aiStrengthAdvantage reaches 151. |
| `0x0048D6FC` | `UnitOrder_SiegeAttMelee()` | verified | Siege attacker, unit category 3. The same staging ladder as UnitOrder_SiegeAttFoot with different thresholds (15, 25, 30, 41, 50, 61, 70, 81), plus a six-phase rotation deciding when to move onto the castle objective and when to hold. |
| `0x0048D9CE` | `UnitOrder_SiegeAttKnight()` | verified | Siege attacker, unit category 4. Moves to a field corner for ten thinks then cycles castle approach points until think 31. Also the only handler that raises the flag at 0x0056D5C8 - when the AI side is all knights and nothing is breached yet. |
| `0x0048DB84` | `UnitOrder_SiegeAttCatapult()` | verified | Siege attacker, unit category 5 (catapults). Nothing for four thinks, a field corner to think 13, a castle approach to think 21, then from think 31 the nearest surface-4 cell within twenty cells of one of its figures - twenty cells being exactly the catapult firing range in g_missileStats. Has no in-melee gate, so it keeps thinking while engaged. |
| `0x0048DDC7` | `UnitOrder_SiegeAttTower()` | verified | Siege attacker, unit category 6 (siege towers). Moves to a field corner for eleven thinks, then - once g_siegeApproachScore passes 6 after think 60, or passes 10 at any time - widens a search from radius 10 to 30 for a surface-4 cell and moves onto it. This is the code that walks a tower up to the wall. |
| `0x0048DFBB` | `UnitOrder_SiegeAttRam()` | verified | Siege attacker, unit category 7 (battering rams). Ignores `orders` entirely: it picks between its stored secondary destination and a castle approach point from three flags - the castle layout flag, g_siegeApproachScore, and whether the drawbridge patch has been laid. |
| `0x0048E097` | `UnitOrder_SiegeDefMissile()` | verified | Siege defender, unit category 1. Rotates through nine wall slots every other think while fewer than three attackers stand on the wall, and falls back on the castle objective when more do. Its sortie branch above g_aiSortieThreshold discards the Enemy_NearestUnit result and then calls Order_HalfwayToUnit on its own unit index, so it does nothing at all - see docs/battle-ai.md 6.3. |
| `0x0048E39C` | `UnitOrder_SiegeDefFoot()` | verified | Siege defender, unit category 2. Thinks every 100 frames rather than 200 and does not check the in-melee flag, so it keeps re-deciding while fighting. Its Enemy_NearestUnit(unit, 10, 0) call discards the result. Then either charges (sortie) or holds the castle objective. |
| `0x0048E4CC` | `UnitOrder_SiegeDefMelee()` | verified | Siege defender, unit category 3. Thinks every 100 frames. Reserves a defence post with Siege_ClaimDefencePost, moves onto an attacking unit that has reached the wall, and otherwise rotates four inner wall slots every fifteenth think - abandoning all of it for the castle objective once four or more attackers are on the wall. |
| `0x0048E774` | `UnitOrder_SiegeDefKnight()` | verified | Siege defender, unit category 4. Sits on wall-slot group 2 every twentieth think until a single attacker reaches the wall, then falls back on the castle objective. The most passive of the defender handlers. |
| `0x0048E8B8` | `UnitOrder_SiegeDefOil()` | verified | Siege defender, unit category 8 (boiling oil). Calls Oil_FindPourTarget: if the unit is high enough on the wall it moves to the densest cluster of enemy figures within six cells (four if it is on the rampart proper), needing three enemies (two). Otherwise it retreats toward the keep. |
| `0x0048E234` | `UnitOrder_SiegeDefWallMissileA()` | verified | Siege defender, unit category 9 - which BattleUnit_Create gives to the first missile unit raised for a side-0 siege defender. The handler does nothing but count its `orders` field up, so this unit holds the wall slot it was deployed on for the whole battle. |
| `0x0048E2C8` | `UnitOrder_SiegeDefWallMissileB()` | verified | Siege defender, unit category 10 - the second missile unit of a side-0 siege defender. Moves to a wall cell beside the enemy figure its own men would rather not shoot at, and falls back on the wall below the keep once attackers reach the rampart. |
| `0x0048EF45` | `Enemy_NearestUnit(unit, maxDist, minFigures)` | verified | Unit-level target selection: the nearest enemy unit by Chebyshev distance between the two units' centres, within maxDist cells and holding at least minFigures figures; 0 if there is none. Missile units call it with maxDist 80 - the whole field - and melee units with 8 or 9. |
| `0x004954DD` | `Melee_ChooseChaseTarget()` | verified | Figure-level target selection for state 8, and the most visible AI decision in the game. Score is the Chebyshev distance to the enemy figure, halved if that figure carries a missile weapon, plus that figure's `targeted` counter; lowest wins. Siege engines and dead figures are skipped, there is no range limit, and the winner's `targeted` is raised by 2 so the next chaser picks somebody else. |
| `0x00495956` | `Missile_FindTargetPreferShooters(range, man, unit)` | verified | Missile_FindTarget with two extra weights: an enemy in state 9 (filling the moat) scores a quarter of its distance, and an enemy carrying a missile weapon scores half. Used by Order_ToWallNearPreferredTarget to post defenders opposite the enemy shooters. |
| `0x00495C41` | `Missile_FindTargetAvoidShooters(range, man, unit)` | verified | The mirror of Missile_FindTargetPreferShooters: state 9 still scores a quarter, but an enemy with a missile weapon scores double, so it is picked last. Used by Order_ToWallNearAvoidedTarget. |
| `0x00495F3D` | `Enemy_FindCluster(x, y, owner, maxSurface, radius, minCount)` | verified | Counts live enemy figures on cells of surface <= maxSurface in a square of the given radius and returns the first such cell's byte offset, or 0 if fewer than minCount were found. The boiling-oil unit's aiming routine. |
| `0x00496566` | `Siege_FindCellSurface4(x, y, radius, maxElevation)` | verified | Nearest cell of surface 4 within a square radius whose elevation is at most maxElevation, left in g_foundWallX / g_foundWallY. Distance is Manhattan when maxElevation is 4 and min(\|dx\|,\|dy\|) otherwise. How catapults, siege towers and the wall-bound defenders find the wall. |
| `0x00496D76` | `Siege_FindCellSurface5(x, y, radius)` | verified | Nearest cell of surface 5 within a square radius by Manhattan distance, left in g_foundTileX / g_foundTileY. Used to put a defending unit on the rampart beside a given cell. |
| `0x00489654` | `BattleUnit_NeedsReform(unit)` | verified | Gate on the every-500-frame reform in Battle_UpdateAllUnits: false for an empty unit, for one whose byte +0x2A (halted) is set, and for a human-controlled unit of fewer than four figures that is not in melee. Its result also picks which slot-assignment pass BattleUnit_Reform runs. |
| `0x0048970E` | `BattleUnit_Reform(unit)` | verified | Re-issues every figure of a unit a destination. If the formation rectangle around the unit's target is clear it assigns rectangle slots directly; if not it searches outward for free ground per figure; a siege tower past a siege-progress threshold sends every figure to the unit target instead. Clears the in-melee flag +0x13 afterwards. |
| `0x00489F9D` | `Formation_RectIsClear(unit)` | verified | Tests every slot of the unit's formation rectangle for being on the map, at the destination's elevation, free of another unit's figure and not impassable. Also caches the destination cell's occupant, its is-water flag and its elevation in the three globals the slot assignment then reads. |
| `0x00489798` | `Formation_AssignSearchedSlots(unit)` | verified | Slot assignment for a blocked destination: for each figure, search outward from the unit target for a usable cell, then send the nearest unassigned figure to it. Claimed cells go in a 100-entry list so two figures never get the same one. |
| `0x00489896` | `Formation_AssignRectSlots(unit)` | verified | Slot assignment for a clear destination: walk the formation rectangle in order and send the nearest unassigned figure to each slot. Greedy nearest-first, so the unit does not fold through itself. |
| `0x00489981` | `Formation_AssignAllToTarget(unit)` | verified | Slot assignment that sends every figure to the unit's target cell rather than to a rectangle. Used for a siege tower once g_siegeApproachScore passes 6. |
| `0x00489A62` | `Formation_NearestFreeFigure(x, y, unit)` | verified | The live figure of `unit` nearest (x,y) by Manhattan distance that has not yet been given a slot in this pass, marked by figure byte +0x17C. |
| `0x00489B8D` | `Formation_SendFigure(x, y, man, unit)` | verified | Gives one figure its destination and entry state: 3 walk, 9 fill in the moat when the destination is water, 10 for a siege engine, 17 closing to attack when the figure has a bow or crossbow and something stands on the destination, 12 when a catapult is aimed at low ground. Clears the route and the barred counter if the destination changed. |
| `0x0048A1C9` | `Formation_ComputeRect(unit)` | verified | Works out the unit's formation rectangle - columns, footprint, width, depth - and its top-left origin, centred on the unit target and clamped to the map. Forces the column count to 2 for units whose byte +0x09 is 1. |
| `0x0048A38E` | `Formation_FindNearbySlot(unit)` | verified | Expanding-ring search of radius 0..19 around the unit target for a cell Formation_SlotIsUsable accepts and that no figure of this pass has claimed. Short-circuits to the target itself on water, and on surface 4 for a catapult unit. |
| `0x0048A672` | `Formation_SlotIsUsable(cellOffset, surface, elevation, unit)` | verified | Whether one cell may serve as a formation slot: rejects an impassable cell that is also empty, accepts one holding an enemy figure, rejects one holding another friendly unit's figure, and in a siege refuses to put a side-0 unit on an empty low-surface cell. |
| `0x004819CC` | `Formation_PickGeometry(unit)` | verified | Chooses which of the unit's troop types dictates the formation by taking the highest g_formationTypePriority, then loads that type's cell footprint and figures-per-row out of g_troopBattleStats. |
| `0x0048B37B` | `Order_DoNothing()` | verified | An immediate return, called from the opening `orders` phases of the field handlers: the units stand still while the battle starts. |
| `0x0048B386` | `Order_HoldPosition()` | verified | Sets the unit's destination to its own current position and re-issues the order, stopping the unit where it stands and reforming it there. |
| `0x0048B546` | `Order_ToSecondTarget()` | verified | Copies the unit's second destination pair (+0x26/+0x28) into the live destination and re-issues the order. The battering ram's default move. |
| `0x0048B5FC` | `Order_StepAwayFromUnit(unit)` | verified | Shifts the current unit's destination two cells directly away from `unit`, along the longer axis and along both when an axis separation exceeds five. Sets the withdrawing flag +0x10 and counts the withdrawal in +0x2B. This is the whole of the AI's retreat behaviour. |
| `0x0048B7FC` | `Order_StepTowardRallyPoint()` | verified | The mirror of Order_StepAwayFromUnit: shifts the destination two cells toward g_aiRallyX / g_aiRallyY and clears g_aiRallyRequest. How a missile unit answers a melee unit that has just been attacked. |
| `0x0048BAC4` | `Order_ToRallyWaypoint(index)` | verified | Moves the unit to one of three rally waypoints for its side, from g_rallyWaypoints indexed by g_battleRallyGroup. Both field battlefield builders fill that table; a .skr map puts all three on the deployment marker. |
| `0x0048BBE2` | `Order_ToWallSlot(group, slot)` | verified | Moves a siege defender onto one of sixteen wall positions in one of the groups at 0x00554180, skipping empty entries and wrapping, offset one cell west and two north of the recorded position. |
| `0x0048BCFB` | `Order_ToCastleApproach(index)` | verified | Moves a siege attacker to one of four approach points per index from the castle layout tables at 0x0055CD90, selected by g_battleApproachLane. Rams always use the primary table; everyone else switches on the castle orientation flag. |
| `0x0048BECD` | `Order_ToFieldCorner(index)` | verified | Sends a siege attacker to a fixed corner of the field - (6,74) or (74,74) - by castle layout, or to Order_ToCastleApproach when the layout flag is neither 1 nor 2. The opening move of most siege-attacker scripts. |
| `0x0048BFF9` | `Order_ToCastleObjective(mode)` | verified | Moves the unit onto one of the two castle cells Battlefield_BuildCastle recorded: always the primary for mode 1, otherwise the secondary unless the moat flag or the castle index says otherwise. |
| `0x0048C06F` | `Order_ToWallBelowKeep()` | verified | Takes the primary castle cell, drops four cells south, and moves onto the nearest surface-4 cell within five. The siege defender's fall-back position. |
| `0x0048C1B9` | `Order_ToKeep()` | verified | Moves the unit onto the secondary castle cell, offset two west and one north, when there is one; onto the primary when there is not. |
| `0x0048C363` | `Order_ToCell(cellOffset)` | verified | Converts a battlefield cell byte offset back into x,y and issues the move. The common tail of most siege order actions. |
| `0x0048C3FE` | `Order_ToSurface5Near(cellOffset)` | verified | Moves the unit onto the nearest surface-5 cell within five cells of the given cell, or does nothing if there is none. |
| `0x0048C4BD` | `Order_OntoUnit(unit)` | verified | Sets the destination to another unit's position and issues the move: walk straight at the enemy. The melee handlers' aggressive answer to being hit. |
| `0x0048C56F` | `Order_HalfwayToUnit(unit)` | verified | Advances half the separation toward another unit. It does nothing at all unless one axis is separated by eight cells or more, and then shifts only those axes separated by six or more. This is why AI missile units advance in stages and then stop. |
| `0x0048C8AF` | `Order_ChargeNearest(unit)` | verified | Sets the halted flag +0x2A and puts every figure of the unit into state 8 - free pursuit of the nearest enemy figure via Melee_ChooseChaseTarget - clearing their routes. The AI's all-out charge, and the only thing a siege defender does once it is overwhelmingly stronger. |
| `0x0048C974` | `Order_ShootAtUnit(unit)` | verified | For each figure of the current unit, finds a target within forty cells restricted to `unit` and puts the figure into state 17 aimed at it. Returns 0 if any figure found nothing, which the caller reads as "that unit is out of reach". |
| `0x0048CBA9` | `Order_ToWallNearPreferredTarget()` | verified | Picks a target with Missile_FindTargetPreferShooters, then moves the unit onto the nearest surface-4 cell within twenty of that target. |
| `0x0048CCE3` | `Order_ToWallNearAvoidedTarget()` | verified | As Order_ToWallNearPreferredTarget but choosing with Missile_FindTargetAvoidShooters, so the unit posts itself opposite enemy foot rather than enemy shooters. |
| `0x0048CE1D` | `Order_ToNearestWallCell()` | verified | Moves onto the nearest surface-4 cell within forty cells of the unit, or does nothing if there is none. |
| `0x0048CE81` | `Order_ToBreachOrStaging()` | verified | While g_siegeApproachScore is under 16 it searches radii 12..19 for a way in and moves there; between 16 and 400 it falls back on the secondary staging table; above 400 it does nothing at all. |
| `0x0048D013` | `Order_ToSiegeStaging()` | verified | The siege attacker's staging move: the primary approach table when g_siegeApproachScore is below 16 or above 399, the secondary table five cells further in between, or a fixed offset from the castle when the orientation flag is set. |
| `0x0048EA95` | `Oil_FindPourTarget()` | verified | The cell a boiling-oil unit should move to: nothing when its own cell is below elevation 2; otherwise the densest enemy cluster within six cells needing three enemies (own surface below 6), or within four needing two (surface 6 and above). |
| `0x0048EBF1` | `Oil_IsOnHighWall()` | verified | True when the unit stands at elevation 2 or more on a cell of surface 6 or above. Decides whether an oil unit holds its post or falls back. |
| `0x0048ECA9` | `BattleUnit_FewOnRampart()` | verified | True when fewer than four of the unit's live figures stand on cells of surface 4 or above, and only once the drawbridge patch has been laid. |
| `0x0048ED95` | `Siege_ClaimDefencePost(unit)` | verified | A twenty-entry (cell, unit) reservation table. Returns the cell already reserved for this unit, or reserves the first free non-empty entry for it. Stops two defending units posting to the same place. |
| `0x0048314E` | `BattleMan_StateWalk()` | verified | Figure state 3: walk toward tg x / tg y and drop to state 5 (idle) when the mover reports an interruption. The state Formation_SendFigure uses for an ordinary move. |
| `0x00483E55` | `BattleMan_StateChase()` | verified | Figure state 8: free pursuit. Re-acquires a target with Melee_ChooseChaseTarget whenever it has none or its target is dead, copies the target's position into its own tg x / tg y every tick, and walks. Siege engines are bounced to state 11 instead. This state ignores the unit's destination entirely. |
| `0x00471C1F` | `Path_BuildTerrainTemplate()` | verified | Rebuilds g_pathBlockedTemplate, the cost field Path_Search copies at the start of every search: 999 for a cell whose flags carry 0x10 or 0x40 and for the one-cell border, 0 everywhere else. It ignores flags 0x80 and 0x20, both of which Cell_TryEnter treats as blocking, so the pathfinder's map is more permissive than the mover's. |
| `0x00471D30` | `Path_BuildElevation()` | verified | Copies cell byte +4 of all 6,400 battlefield cells into g_pathElevation. |
| `0x00471DA6` | `Path_BuildStepCost()` | verified | Rebuilds g_pathStepCost, and is the pathfinder's entire cost function. Zero everywhere except: a cell whose flags carry 0x20 or 0x40 costs 100 and lays a gradient of 12, 8, 4, 4 on the cells one to four rows further south (three cells wide in the last two bands); and, when the flag at 0x0057C910 is set, a cell diagonally adjacent to surface 5 costs 4. Battlefield_BuildFromSkr sets neither 0x20 nor 0x40, so on a .skr battlefield every step costs the same. |
| `0x0047265D` | `Path_NearestReachedNear(x, y, tx, ty)` | verified | The salvage step after Path_Extract fails: an expanding-ring search of radius 0..29 around the unreachable destination for the cheapest cell the flood fill actually reached at the destination's elevation, left in g_foundTileX / g_foundTileY. Gives up if the ring reaches the figure's own position. |
| `0x00472227` | `Path_DetourTooLong(man)` | verified | Cancels a move whose route is absurd: true when the path cost to the destination exceeds 20 in a field battle or 150 in a siege and is also more than five times the straight-line Chebyshev distance. It can only ever return true for a figure whose owner is human, so a player's men give up on long detours and the AI's never do. |
| `0x00404A46` | `Rand_Advance()` | verified | The pseudo-random generator the battle AI uses. Two 31-bit Fibonacci LFSRs (taps at bits 0 and 4) are each stepped 31 times, then masked into six output globals - &0x7FFF, &0x7F and &7 per generator. Distinct from FUN_00404B2C / DAT_005C9A84, the counter the battlefield builders use. |
| `0x00405000` | `Dist_MinAxis(x, y, tx, ty)` | verified | Returns min(\|dx\|, \|dy\|) and leaves the two absolute deltas in g_absDx and g_absDy. The third member of the Dist_Manhattan / Dist_Chebyshev family; used by Siege_FindCellSurface4. |
| `0x004B99C0` | `Battle_Frame()` | inferred | The battle frame loop. Calls Battle_CountMenByType, Battle_UpdateAllMen, Missile_UpdateAll, BattleUnits_RebuildFromFigures, Battle_UpdateAllUnits and BattleDebug_Panel, which fixes all of them at once per frame - and so fixes the unit timers those passes age. |

**Globals**

| Address | Name | Confidence | Meaning |
|---|---|---|---|
| `0x004EEA98` | `g_aiStrengthAdvantage` | verified | The AI's view of the battle, recomputed every 101 frames by Battle_UpdateStrengthAdvantage: weighted AI men as a percentage of weighted human men, minus 100, plus a -10..+21 jitter outside deterministic play. Every field and siege handler branches on it. It is one global for the whole battle, not per side and not per unit. |
| `0x0057C8B4` | `g_aiAggressionThreshold` | verified | Set to 5 by Rules_InitConstants and read only by the three field order handlers: above this advantage they attack, at or below it they hold and skirmish. |
| `0x00552FF0` | `g_aiSortieThreshold` | verified | Set to 260 by Rules_InitConstants and read only by the four siege-defender handlers: above this advantage the garrison lays the drawbridge patch and charges out. |
| `0x004D9C70` | `g_aiAdvantageTimer` | verified | Frame counter in Battle_UpdateAllUnits; at 101 it triggers Battle_UpdateStrengthAdvantage and resets. |
| `0x00553070` | `g_aiCommitCounter` | verified | Zeroed by Battle_Start. A field melee handler adds 3 when the side's engagement count passes its budget byte and 20 when the AI's missile troops fall below an eighth of its army; every unengaged melee unit counts it down by 1 per think. While it is non-zero every AI melee unit goes into free pursuit and every missile unit closes half the distance, so it is a commitment counter rather than a hold. |
| `0x0056D89C` | `g_aiRallyRequest` | verified | Raised by a field melee handler that is under attack, together with g_aiRallyX / g_aiRallyY; the field missile handler consumes it and shifts two cells toward that point. The only unit-to-unit signal in the battle AI. |
| `0x00522F70` | `g_aiRallyX` | verified | X of the enemy unit that attacked the melee unit which raised g_aiRallyRequest. |
| `0x00522F74` | `g_aiRallyY` | verified | Y of the enemy unit that attacked the melee unit which raised g_aiRallyRequest. |
| `0x00553F28` | `g_aiEngagementCount` | verified | Counts, since Battle_Start, how many times a field melee handler has thought while its unit had a live attacker. Compared against the per-battlefield budget byte at 0x00553080 to decide whether the side should commit. |
| `0x00553F74` | `g_aiMenTotal` | verified | Surviving men over all figures whose owner is not human, from Battle_CountMenByType. |
| `0x0056D6AC` | `g_aiMenMissile` | verified | Surviving crossbowmen plus archers on the non-human side. When this falls below an eighth of g_aiMenTotal the field melee handlers add 20 to g_aiCommitCounter. |
| `0x0057C980` | `g_aiMenKnight` | verified | Surviving knights on the non-human side. Two siege-attacker handlers compare it against g_aiMenTotal. |
| `0x0057C8FC` | `g_battlePhase` | verified | Set to 2 by Battle_Start. BattleUnits_RebuildFromFigures does nothing unless it holds 2. |
| `0x0057C8DC` | `g_battleUnitCount` | verified | Number of occupied unit slots, recounted at the end of every BattleUnits_RebuildFromFigures pass. |
| `0x0056D6B0` | `g_battleApproachLane` | verified | 0..3, chosen at random by Battle_Start in normal play and advanced cyclically in deterministic play. Selects which of four columns of the castle approach tables the siege attackers use, and which end-of-field slot the field handlers march to. |
| `0x0053F654` | `g_battleRallyGroup` | verified | 0 or 1, chosen the same way as g_battleApproachLane. Selects which group of three rally waypoints the field handlers use. |
| `0x00522D30` | `g_rallyWaypoints` | verified | The field-battle rally waypoints Order_ToRallyWaypoint reads: two sides x two groups x three (x,y) int pairs, side 0 at 0x00522D30 and side 4 at 0x00522D60, group stride 0x18. Filled by Battlefield_BuildFromSkr and Battlefield_BuildRandom; on a .skr map all three of a group hold the same deployment marker cell. |
| `0x00553FB0` | `g_siegeApproachScore` | inferred | Siege progress as the AI measures it, and the most-read global in the siege handlers. Raised by the routine that turns a filled-in moat cell into open ground - once per adjacent cell of surface 3, 4 or 5 - and by 4 when the drawbridge patch is laid. Thresholds used against it are 3, 4, 8, 10, 16 and 400. The name is a reading of what raises it, not a label from the binary. |
| `0x0053E990` | `g_siegeBreachScore` | inferred | Raised by the routine Missile_Step calls when a shot destroys a cell - once per orthogonal neighbour still of surface 5 - and by 4 when the drawbridge patch is laid. The siege-attacker handlers read non-zero as "there is a way in". |
| `0x00553E64` | `g_attackersOnWall` | inferred | Recounted every frame by Battle_UpdateAllMen: live figures of side 4 standing on a cell of surface 5. Every siege-defender handler branches on it, and at 1, 2, 3, 4 and 6 attackers the defenders progressively abandon the wall for the keep. |
| `0x00553FF0` | `g_siegeEngineCount` | verified | Recounted every frame by Battle_UpdateAllMen: live figures of troop type 7, 8 or 9 - catapult, siege tower, battering ram - on either side. Three siege-attacker handlers refuse to move onto the castle objective while it is zero. |
| `0x004D98C8` | `g_formationTypePriority` | verified | Eleven ints indexed by troop type - 0, 4, 1, 2, 5, 3, 6, 10, 8, 9, 7 - the priority with which a troop type dictates its unit's formation geometry. Siege engines outrank everyone, then knights, pikemen, crossbowmen, archers, swordsmen, macemen, peasants. |
| `0x00507250` | `g_pathBlockedTemplate` | verified | The 80x80 u16 template Path_Search copies over g_pathCost before every search: 999 blocked, 0 free. Rebuilt by Path_BuildTerrainTemplate whenever the battlefield changes. |
| `0x00507248` | `g_pathFailCount` | verified | Zeroed by Battle_Start, incremented once per Path_Extract that returns no path. A ready-made diagnostic for how often the pathfinder is failing in a live battle. |
| `0x004F037C` | `g_pathSearchCount` | verified | Zeroed by Battle_Start, incremented once per flood fill actually run - that is, excluding the adjacent and clear-line early outs. |
| `0x004EE900` | `g_formationClaimedCells` | verified | A 100-entry list of cell byte offsets already claimed during one Formation_AssignSearchedSlots pass, so two figures of a unit are never sent to the same cell. |
| `0x0056D5D0` | `g_scanBattleMan` | verified | Loop cursor of the figure-scanning target selectors - Missile_FindTarget and its two weighted variants, and Melee_ChooseChaseTarget - left holding the chosen figure on success. |
| `0x0057D328` | `g_foundWallX` | verified | X of the cell Siege_FindCellSurface4 last returned. |
| `0x0057D324` | `g_foundWallY` | verified | Y of the cell Siege_FindCellSurface4 last returned. |
| `0x00568224` | `g_formationFootprint` | verified | Cell footprint of the troop type Formation_PickGeometry chose for the current unit. |
| `0x0056D8A8` | `g_formationCols` | verified | Figures per row of the current unit's formation rectangle, capped at the figure count and forced to 2 for units whose byte +0x09 is 1. |
| `0x0057C970` | `g_formationOriginX` | verified | X of the top-left corner of the formation rectangle, and the output of Formation_FindNearbySlot. |
| `0x00567968` | `g_formationOriginY` | verified | Y of the top-left corner of the formation rectangle, and the output of Formation_FindNearbySlot. |
| `0x0058FE28` | `g_randStateA` | verified | First 31-bit LFSR state of Rand_Advance. |
| `0x0058FE08` | `g_randStateB` | verified | Second 31-bit LFSR state of Rand_Advance. |
| `0x0058FD7C` | `g_rand7A` | verified | g_randStateA & 0x7F after Rand_Advance. Battle_UpdateStrengthAdvantage takes (value & 0x1F) - 10 from it, so the AI's read of its own strength wanders by -10..+21. |
| `0x00553030` | `g_deterministicBattle` | inferred | Read by well over a hundred functions across the binary. Where it matters here: when it is zero Battle_Start picks g_battleApproachLane and g_battleRallyGroup at random and Battle_UpdateStrengthAdvantage adds its jitter; when it is non-zero both become deterministic cycles and the jitter is dropped. Sync_Checksum and Turn_AllRealmsDone also read it, and Troops_Load uses the side-neutral TROOPS.ENG when it is set. Everything is consistent with "this is a networked game, so nothing may diverge" - which is a reading, not a proof. |
| `0x00554090` | `g_moatFillSteps` | verified | Set to 15 by Rules_InitConstants. A figure filling in the moat (state 9) raises the target cell terrain byte by 1 every 81 frames - 101 if its owner is human - and the cell becomes open ground when the byte reaches this value. |

<!-- END symbols.json: battleai -->


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
| `0x004D8778` | `g_armyHappinessCost` | verified | int[102] happiness cost of raising an army, indexed by the PERCENTAGE of the county's population taken rather than by the number of men: 0, 1, 1, 1, 2 ... 90 at half the county, then a flat 101 from index 61 up. FUN_004A5003 reads it unbounded, so a county under 50 people indexes past the end into g_goodsPrice and gets a free army. 0x004D8778 + 102 * 4 is exactly 0x004D8910, which is what fixes the length. |
| `0x004D8910` | `g_goodsPrice` | verified | Merchant base price per good id, 15 entries in L2.eng group 6 order: -, grain 2, cattle 12, sheep 0, ale 1, wool 0, iron 1, stone 2, timber 1, pikes 13, bows 16, maces 10, crossbows 24, swords 23, mail 44. |
| `0x004D8950` | `g_goodsStock` | inferred | A second 15-entry table on the same good ids: 1000 grain, 100 cattle, 200 sheep, 100 ale, 500 wool, 100 iron, 100 stone, 200 timber, 500 each weapon. Reads like the quantity a merchant carries. |
| `0x004D8990` | `g_weaponCost` | verified | Six {wood, iron} pairs: crossbow 6/10, mace 4/4, sword 3/10, pike 6/3, bow 13/0, armour 4/18. Debited by Industry_Produce. |
| `0x004D89C0` | `g_castleMaterial` | verified | Five {wood, stone} pairs: palisade 400/40, motte and bailey 800/80, Norman keep 200/1000, stone castle 400/2000, royal castle 800/3000. |
| `0x004D89E8` | `g_castleWorkforce` | verified | Man-seasons per castle type, each value stored twice: 200, 400, 800, 1500, 2500. |
| `0x004D8A10` | `g_castleGarrisonCap` | verified | Troops a castle can hold: 150, 200, 200, 400, 600. |
| `0x004D8A28` | `g_castleTaxBonus` | verified | Percent tax bonus by castle type: 50, 75, 100, 125, 150 - the same ratios as Tax_CollectAll's 480/560/640/720/800 over the castle-less 320. |
| `0x004D8A40` | `g_castleFreeArchers` | verified | Archers a newly finished castle is given: 50, 150, 150, 200, 300. |
| `0x004D8A5C` | `g_aiPersonality` | inferred | Per-AI-lord behaviour parameters, three 0x50-byte rows per lord and only the first row used, so the stride is 0xF0. THIS ADDRESS IS THE RECORD'S +0x04 FIELD, not its start: AI_SetTaxRates reads *(int *)(&g_aiPersonality + (lord * 3 - 3) * 0x50) as the tax-ladder selector, and the record itself begins four bytes earlier at 0x004D8A58 with the farming style AI_ManageFields dispatches on. Four records, lords 1..4: farming style 1/1/0/9, tax ladder 2/2/2/1. A fifth would begin at 0x004D8E18 and what is there fits no pattern. |
| `0x004DC1E0` | `g_aiGoldGrant` | verified | int[5][4] free gold per turn by AI lord and difficulty for a realm holding three or more counties: 0/0/0/0, 0/400/700/1200, 100/500/800/1400, 0/400/700/1200, 250/600/1100/1800. |
| `0x004DC230` | `g_aiGoldGrantSmall` | verified | The same shape, used when the realm holds fewer than three counties. |

<!-- END symbols.json: kingdom -->

## Interface and screen layout

The presentation layer: the campaign screen's viewport, its two zooms, its chrome and its
minimap. Written up in full in [`screens.md`](screens.md).

This section did not exist until this work, and its absence caused a real error:
`crates/l2-view` drew the whole 64 × 64 map at once, which is a view the original does not
have. `Map_SetZoom` is the anchor — ten globals per zoom, and its derived width comes out
at 480 in every case, which is what fixes the 160-pixel right column.

<!-- BEGIN symbols.json: ui -->

| Address | Name | Confidence | What it does |
|---|---|---|---|
| `0x00451FCC` | `Map_SetZoom(zoom)` | verified | The only writer of the campaign map geometry. Sets ten globals per zoom: g_mapZoom, g_mapScrollStep 1/2/4, g_mapViewCols 8/17/40, g_mapViewRows 30/64/128, g_mapViewX 0/4/0, g_mapViewY 9/17/21, g_mapTilePitch 60/28/12, g_mapHalfPitch and g_mapTileHalfStep 30/14/6, g_mapRowStep 15/7/3; then derives g_mapViewRight = pitch*cols + viewX (480 at all three) and g_mapViewBottom = (rows+1)*rowStep + viewY (474/472/408). Case 1 is unreachable: nothing in the binary ever passes 1, and g_mapZoom has no other writer than 0. |
| `0x00429B1D` | `Map_ClampScroll()` | verified | Clamps g_mapStartCol to 0..0x40-g_mapViewCols and g_mapStartRow to 0..0x80-g_mapViewRows. At the far zoom that is col <= 24 and row <= 0, i.e. no vertical scroll at all. |
| `0x00429BA4` | `Map_PickTile(mode)` | verified | Screen -> lattice hit test for the campaign map. Rejects x outside [g_mapViewX, g_mapViewX + pitch*cols) and y outside [viewY + rowStep, viewY + rowStep*(rows+1)), then divides by g_mapTileHalfStep and g_mapRowStep and resolves the diamond with a parity and remainder test. Returns 0 when the cell is off-map. |
| `0x004298C1` | `Map_BuildLattice(rotation)` | verified | Clears g_screenLattice, reloads the tail with Map_LoadLattice, then projects all 64x64 tiles into it for rotation 0/2/4/6. Rotation 0 is row = 1+y+x, col = (64-y+x)/2, value = (64*y+x)*8 - exactly the mapping docs/formats/maps-layers.md section 4 verifies against a live process. |
| `0x00429F12` | `Map_RotateCW()` | verified | g_mapRotation += 2 (mod 8), rebuilds the lattice and re-projects the scroll origin into the new orientation. |
| `0x0042A01B` | `Map_RotateCCW()` | verified | g_mapRotation -= 2 (mod 8), the inverse of Map_RotateCW. |
| `0x0043278B` | `Map_CentreOnTile(tileOffset)` | verified | Scans the 129x65 lattice for the cell holding tileOffset and, only at zoom 0, sets g_mapStartCol = col-4 and g_mapStartRow = (row & ~1) - 12. The row is forced even because the render walk alternates aligned and half-offset lattice rows from the origin. |
| `0x00434FA1` | `Map_ToggleZoom()` | verified | The campaign zoom button: 0 -> Map_ZoomOut, 2 -> Map_ZoomIn. There are only two campaign zooms. |
| `0x00434FD5` | `Map_ZoomOut()` | verified | Saves the scroll origin, sets col 0x0E row 0x0C, Map_SetZoom(2), clamps, reloads the 10x6 tile sets through Gfx_LoadCountyMode and plays S033_02.wav. |
| `0x0043505A` | `Map_ZoomIn()` | verified | Restores the saved scroll origin, Map_SetZoom(0), clamps and reloads the 58x30 tile sets. |
| `0x004350A1` | `Map_ZoomInAtTile()` | verified | Zooms in centred on the last picked tile, using the same col-4 / (row & ~1)-12 centring as Map_CentreOnTile. |
| `0x00431F59` | `Map_ScrollStep()` | verified | Applies g_mapScrollDir 0..7 (N, NE, E, SE, S, SW, W, NW) to the scroll origin: row moves by +/-2*g_mapScrollStep and col by +/-g_mapScrollStep, which is one map tile per step. Reverts the move when Map_ScrollThrottle says it is too soon. In battle it moves the battle view origin instead. |
| `0x00432221` | `Map_EdgeScroll()` | verified | Turns the desktop cursor position into g_mapScrollDir: an edge is the outermost pixel of the desktop (x == 0, x == GetSystemMetrics(0)-1, likewise for y), and 8 means no edge. Returns 0 without scrolling when the campaign map is at the far zoom, so the far view never moves. |
| `0x004BBBE3` | `Map_ScrollThrottle()` | verified | Rate limit for Map_ScrollStep from the Scroll Speed option: one step every ((100 - g_optScrollSpeed)/10)*12 + 2 ms, and never when that quotient reaches 10 - so a speed of 0 disables scrolling. |
| `0x0043253A` | `Minimap_Click()` | verified | Campaign minimap click. Accepts x in 480..607 and y in 25..152, reads g_minimapCounty[(x-480) + (y-25)*128] and, for a non-zero county, selects it and calls Map_CentreOnTile. That 128x128 rectangle is exactly the raster Minimap_Load filled. |
| `0x00432443` | `BattleMap_Click()` | verified | Battle overview click. Accepts x in 480..639 and y in 24..183 and maps it to a battlefield cell by (x-480)/2, (y-24)/2 - a 160x160 panel at 2 pixels per cell over an 80x80 battlefield. |
| `0x00432640` | `Screen_HitRegion()` | verified | Classifies the cursor: region 1 is y < 24 (the menu bar), region 2 is the overview panel rectangle g_overviewX/W by g_overviewY/H, whose top is raised by 24 in campaign mode - so campaign region 2 is x 480..639, y 24..207. |
| `0x0040F5FD` | `Screen_DrawCampaign(full)` | verified | Draws the whole campaign screen: Misc_cty frames 54, 57 and 59 at x 478 (y 24, 430, 460), the map through Map_DrawFrame, the menu bar, the county panel, the minimap at (480, 25), and finally Palette_Set(g_paletteBase01). |
| `0x00419C78` | `Screen_DrawMenuBar()` | verified | The 640x24 bar at y 0: background tiled from Panels.pl8 frames 196+(c mod 8) at 24px steps (25 cells from x 0 plus 2 from x 592 = 640 exactly), a bevel outline, the File/Options/Help titles from L2.eng groups 1/2/3, one 13x16 Misc_cty banner per live realm at x 270+16i y 4, the year and season at x 360 and the local realm treasury at x 500. |
| `0x00410AA9` | `Minimap_Draw(x, y)` | verified | Called as Minimap_Draw(0x1E0, 0x19). Blits the 128x128 g_minimapPixels raster at (x-2, y+3) = (478, 28), tints it through Minimap_DrawOverlay, draws a 29x123 Misc_cty strip at (x+0x83, y+7) and a mode badge at (x+5, y+5). The picture is drawn at (478,28) while Minimap_Click hit-tests (480,25): that 2/3 pixel disagreement is the original. |
| `0x00410CBD` | `Minimap_DrawOverlay(selected, x, y, mode)` | verified | Recolours the minimap raster per county. Source pixels 10..13 are land inside a county and are replaced from an 8-byte-per-realm ramp at g_minimapRealmRamp indexed realmColour*8 + (v-10); shade 10 of the selected county becomes index 0x20. Modes 1..3 instead colour the local players own counties from a 6-entry ramp at g_minimapRatingRamp by county fields +0xB3, +0xB2 and +0xB1. Everything else is left as the picture drew it. |
| `0x0043AB76` | `Minimap_ModeButton()` | verified | Acts on g_uiHotspotId: 1..3 set g_minimapMode, 4 calls Map_ToggleZoom. Pressing the active one again returns to mode 0. |
| `0x0041A734` | `Screen_DrawEndTurn(force)` | verified | Draws Misc_cty frame 59 (162x20) at (478, 460) and centres L2.eng group 4, "End turn", in it at (478, 462) in the 9-point font. That strip is the End Turn button. |
| `0x00409934` | `Ui_DrawBoxBorder(style, x, y, cols, rows)` | verified | Draws a 16-pixel-cell border from Panels.pl8: frames 0/1/2/3 are the TL/TR/BR/BL corners, 4..15 the top edge, 16..27 the bottom, 28..39 the left and 40..51 the right, each cycled (n-1) mod 12. A style above 0 adds 0xCC to reach the second complete border set at 204..255; style 2 also replaces the two top corners with edge pieces and omits the top edge. |
| `0x00409C93` | `Ui_DrawBoxInterior(x, y, cols, rows)` | verified | Fills a 16-pixel-cell area with Panels.pl8 frames 52 + (c mod 12) + (r mod 12)*12 - a 144-frame texture that the file lays out as a 12x12 grid on the artists sheet. |
| `0x00409397` | `Ui_DrawBox(x, y, cols, rows)` | verified | Ui_DrawBoxBorder(0, ...) plus Ui_DrawBoxInterior inset by one cell. Sizes are in 16-pixel cells: the strip under the far-zoom map is Ui_DrawBox(0, 412, 30, 4) = 480x64. |
| `0x00409E09` | `Ui_DrawTileStrip(x, y, cols, rows)` | verified | Tiles Panels.pl8 frames 196 + (c mod 8) - the eight 24x24 frames - at 24-pixel steps. The menu bar background. |
| `0x0040A567` | `Pl8_DrawFrameHere(sheet, frame, x, y)` | verified | Pl8_DrawFrame without the inline range diagnostics: reads the 16-byte frame record, sets g_drawX/g_drawY and g_clipDstAdvance = 0x280 - width. That 0x280 is where the 640-byte screen stride is visible. |
| `0x0040A01D` | `Blit_Raster(src, x, y, w, h)` | verified | Blits a bare w*h raster with no PL8 record through the standard clipper. Used for the 128x128 minimap picture. |
| `0x00403FDD` | `Ui_DrawBevelRect(x, y, w, h)` | verified | Four clipped lines: top and right in palette index 0x1F, bottom and left in 0x10. |
| `0x004B0AB5` | `Palette_Set(pal)` | verified | Copies 256 six-bit RGB triples into the display palette, widening by multiplying by 4, then forces entry 0 to black. |
| `0x004984DC` | `Gfx_LoadCountyMode()` | verified | Loads the campaign map artwork. base = (g_mapZoom == 2) ? 0x20 : 0, plus (g_season-1)*8; then eight consecutive g_resourceTable entries go to g_bankBase, g_bankMtns, g_bankRoads, g_bankTown, g_bankCastle, g_spriteSheetA, g_spriteSheetB and g_flagsSheet in that order - independent confirmation of the bank order in maps-layers.md section 1.1. Misc_cty.pl8 follows into g_miscCtySheet. |
| `0x004050F6` | `Map_DrawFrame()` | verified | One campaign map frame. Zeroes g_countyTileTally, gives the current selection a 30-tile head start, runs Map_RenderIso (which tallies +1 per tile drawn and +5 for a castle tile), then the flag and army passes, and finally - only at zoom 0 - moves g_selectedCounty to whichever county filled most of the viewport. Scrolling therefore changes which county the right panel describes. |
| `0x00405C2F` | `Map_RenderOffsetRow()` | verified | One half-offset lattice row: the first tile with clip mode 3 (right half, origin x - halfPitch), then cols-1 tiles a half pitch to the right, then one with mode 4 (left half). Consumes cols+1 lattice columns. |
| `0x00405AE9` | `Map_RenderAlignedRow()` | verified | One aligned lattice row: cols tiles at x = g_mapViewX + c*pitch, clip mode 0, tallying each into g_countyTileTally. |
| `0x0042A7F1` | `Map_DrawSurroundTile(x, y, mode)` | verified | Draws an off-map surround cell: base bank, frame = the lattice cell value minus 0x0FFF0000, which is the background byte the map file stores in its 65x129 tail. |
| `0x004081A6` | `Map_DrawCountyFlag(mode)` | verified | Draws the county flag over a castle tile from g_flagsSheet, offset (+0x14, +6) from the tile, clipped by Clip_Horizontal(g_mapViewX, 0x1DE) and Clip_Vertical(0x18, 0x1DA) - i.e. the map viewport is x < 478 and y in 24..473. |
| `0x00408438` | `Map_DrawArmies(mode)` | verified | Walks the unit list hanging off a tile and draws each from g_spriteSheetA or g_spriteSheetB, offset by an 8-orientation by 16-frame table per zoom at 0x004D8108/0x004D8188 (zoom 0), 0x004D8208/0x004D8288 (zoom 1) and 0x004D8308/0x004D8388 (zoom 2). The sprite direction is the unit facing minus g_mapRotation, mod 8. |
| `0x00406673` | `Map_DrawTileApex(a, b, mode)` | verified | Draws the chevron ("overhang") records that sit above a diamond tile. Reaches them by adding a fixed body size to the frame data pointer: 900 at zoom 0, 0xC4 at zoom 1, 0x24 at zoom 2, which are h*h/2 for h = 30, 14 and 6 and so pin the three tile sizes from the instruction stream alone. |
| `0x0040C5B0` | `Ui_DrawMenuTitles(items, count)` | verified | Lays the menu-bar titles out left to right from a 16-byte-per-item table, writing each measured x back into the table so the drop-downs know where to open. The campaign table is g_menuBarItems, three items, L2.eng groups 1, 2 and 3. |
| `0x00498270` | `Map_InitMode()` | verified | Campaign map bring-up: scroll origin row 0x4A col 0x14, zoom 0, battle phase 0, overview panel rect (480, 48, 160, 160), map 64x64 with an 8-byte runtime tile record, then Map_BuildLattice(0) and Map_SetZoom(0). |
| `0x0040F1A0` | `Screen_Draw(firstFrame)` | verified | The interface's master switch: 39 cases on g_screenId, each calling one screen's painter. With Screen_DrawWidgets and Screen_HandleInput these are the only three places the screen id is dispatched. docs/screens-county.md section 1. |
| `0x004BA26E` | `Screen_DrawWidgets` | verified | Per-screen overlay pass: draws each screen's widget table through Widget_Draw and steps its animations. Same case order as Screen_Draw. |
| `0x004BA9C8` | `Screen_HandleInput` | verified | Per-screen input pass: hit-tests each screen's widget or hotspot table. Returns non-zero when a widget consumed the click. |
| `0x0040CFD2` | `Widget_Draw(xOffset, yOffset, table, count)` | verified | Draws a table of 24-byte widget records. Frame is record +0x04, plus 1 while the press timer at +0x0D runs. The size at +0x06 selects the sheet: below 24 from g_miscCtySheet, otherwise from g_systemSheet. |
| `0x0040DA1E` | `Widget_Test(xOffset, yOffset, table, count)` | verified | Hit-tests the same table Widget_Draw draws, and runs the press timer and the auto-repeat counter. The hit box is a square of side record +0x06. Publishes record +0x10 and +0x14 into g_uiHotspotId and g_uiHotspotArg, then calls record +0x08. |
| `0x0040E3EE` | `Hotspot_Test(xOffset, yOffset, table, count)` | verified | The invisible variant: the same 24-byte record read as {x0, y0, x1, y1, callback, ...} with nothing drawn. The map sidebar's buttons are these. |
| `0x0040D1BC` | `Ui_OkButton(x, y, mode)` | verified | The tick that closes a panel: g_systemSheet frame 0x33 for mode 0, 0x10 for mode 1. |
| `0x00403DEB` | `Ui_DrawInsetRect(x, y, w, h)` | verified | A recessed rectangle: colour 0x10 along the top and right edges, 0x1F along the bottom and left. |
| `0x00402637` | `Ui_DrawText(str, x, y, font, colour)` | verified | Draws a plain string, each glyph three times - at y-1 and y+1 in two shadow colours, then at y in the real one. Advances g_penAdvance, plus four pixels of trailing space. |
| `0x00402C5E` | `Ui_DrawCentred(group, index, x, y, width, font, colour)` | verified | One L2.eng string, centred inside width. |
| `0x00402F64` | `Ui_DrawNumber(value, lead, suffix, x, y, font, colour)` | verified | A number with a leading character and a suffix. lead overwrites buffer index 0, which Ui_NumberToBuffer leaves free for a sign; '@' is the blank glyph that keeps a zero aligned with its neighbours. |
| `0x004030C6` | `Ui_DrawNumberRight(value, lead, suffix, x, y, width, font, colour)` | verified | Ui_DrawNumber, right-aligned inside width. |
| `0x00402E0C` | `Ui_DrawDelta(value, mode, prefix, suffix, x, y, font, colourPos, colourNeg)` | verified | A signed number with a prefix and a suffix - and nothing at all when the value is zero and mode is 0, which is why the happiness and population panels have blank rows in a quiet season. |
| `0x004022BD` | `Ui_NumberToBuffer(value, start, forceSign)` | verified | Formats value into g_numberBuffer starting at index start, writing '-' or '+' at start and shifting when it does. Callers pass start = 1 so index 0 stays free for a sign character. |
| `0x0041AB67` | `Ui_DrawCount(value, unitIndex, x, y, font, colour)` | verified | A number followed by a noun from L2.eng group 8: index unitIndex when the value is 1 or -1, unitIndex + 1 otherwise. Group 8 is singular at even indices and plural at odd. |
| `0x0041AC95` | `Ui_DrawHappinessDelta(value, x, y, font, colourPos, colourNeg)` | verified | Draws a bracketed signed number with the happiness face after it: a bracket, Ui_DrawDelta, g_miscCtySheet frame 0x17, a closing bracket. |
| `0x0041AC3E` | `Ui_DrawUnitNoun(value, unitIndex, x, y, font, colour)` | verified | Just the group 8 noun, singular when the value is 1. |
| `0x0041A900` | `Ui_DrawYear(year, x, y, style)` | verified | A year with BC or AD from L2.eng group 26. style 3 prints the number alone, which is what the graph axes use. |
| `0x004156A7` | `Ui_HistoryGraph(x, y, mode)` | verified | The 402x155 history graph on the population and happiness panels. mode 0 reads g_countyHistory +0x00 (population, u32), mode 1 reads +0x04 (happiness, u8), walking g_historyLength turns from g_historyHead and wrapping at 400. Returns the peak, which the caller prints. Background from graphs.pl8. |
| `0x00499859` | `Res_LoadStatic` | verified | Loads the thirteen records of g_preloadTable into their fixed .data buffers - three palettes, five fonts, mouse.pl8, System2.pl8, Panels.pl8, l2.eng and vill_gd8.pl8. That table is the whole interface asset list. |
| `0x00499A1C` | `Res_LoadButtons(skin)` | verified | Loads system2.pl8 (skin 0) or system.pl8 (skin 1) into g_systemSheet. Same size and same 84-frame layout, but 69 of System2.pl8s frames are entirely index 0 and none of System.pl8s are, and the kingdom screens ask for skin 1 - so the county panels button art is System.pl8. docs/screens-county.md section 4.2. |
| `0x0040F7D3` | `CountyStrip_Draw` | verified | The selected county's strip in the map sidebar: name, population, happiness, tax rate, ration achieved and the health thermometer for an owned county; frame 0x3A and the owner's name for one you do not hold. docs/screens-county.md section 2. |
| `0x00438CEB` | `CountyStrip_Click` | verified | The 2x2 hotspot over the county strip, and the only way to any of the four county panels: x 488..547 / y 182..211 population, x 568..629 / y 182..211 happiness, and the same two columns below y 212 for tax and rations. The dead band 548..567 is where the health thermometer is drawn. |
| `0x00438E3B` | `CountyStrip_JobClick` | verified | The sidebar's lower plate: farm jobs left of x 560, industry right of it. Sets g_jobPanelJob and opens screen 0x0F. |
| `0x00411B72` | `Panel_Ration` | verified | The ration panel (screen 0x19). Draws L2.eng group 87 against county +0x15E (wanted), +0x15D (achieved, red when they differ), +0x09 (health band), +0x11 and +0x10 as happiness deltas, and the Fed / Eaten rows from +0x16C, +0x170, +0x174, +0x178 and +0x17C. |
| `0x00411FDE` | `Panel_RationSlider` | verified | The grain-to-livestock slider: caps from g_systemSheet frames 0x4A and 0x4B at x 200 and 324, a 100-pixel track from x 224 to 323, and the knob (frame 0x4C, 10 wide) at 220 + county +0x15F. |
| `0x00412B33` | `Panel_JobDetail` | verified | The job popup (screen 0x0F) for g_jobPanelJob, 1..9 into L2.eng group 74. Draws the worker count with the group 8 noun for job*2 + 30, red when labour +0x00 is below +0x04 and a second colour when it is above +0x08 - which is what shows the labour record to be three integers a job, not one. |
| `0x00412E6B` | `Panel_JobIndustry` | verified | The mining, quarrying, wood and blacksmith bodies of the job popup. Maps jobs 5, 6, 7 and 8 to industry records 1, 3, 0 and 2, the same order the village artwork implies. |
| `0x00413590` | `Panel_JobGrain` | verified | The grain body of the job popup: store, fertility band, the event and weather effects on the store, and either what will be sown in spring or what is growing and when it is harvested. Nothing on it is clickable - the player never chooses how much to sow. |
| `0x00413B30` | `Panel_JobCattle` | inferred | The cattle body of the job popup: L2.eng group 77 calf births, cow deaths and the herd-crowding bands. |
| `0x004140F3` | `Panel_JobReclamation` | verified | The field-reclamation body of the job popup: how many fields are being reclaimed and how many seasons to the next one. |
| `0x00413155` | `Panel_JobBlacksmith` | verified | The blacksmith body of the job popup, with smithy.pl8 and hearth.pl8 and the weapon's cost from g_weaponCost. |
| `0x00412143` | `Village_Draw(reload)` | verified | The village screen (screen 0x02): villani1/villani2/vill.pl8, villtops.pl8, and the weather and fertility lines when Advanced Farming is on. The scene starts at y = g_villageTopY. |
| `0x00412666` | `Village_DrawPeasants` | verified | Draws the eight peasant clusters at g_jobClusterOrigins + (0x40, g_villageTopY). |
| `0x004126C9` | `Village_DrawCluster(cluster, x, y)` | verified | One cluster: up to 25 icons from g_peasantIcons at the offsets in g_peasantIconOffsets, each frame bumped by one while the cluster is the drag source. |
| `0x00412421` | `Village_Animate` | inferred | Steps the village's animation counters and draws the animated overlays. |
| `0x0043958A` | `Village_BoxSelect` | verified | Marks every peasant icon inside the rubber-band box, and abandons the whole selection if the box reaches into a second cluster. Leaves the cluster in g_villageDragCluster and the count in g_villageDragCount. |
| `0x0043982C` | `Village_ClusterAt` | verified | Which of the eight clusters the pointer is over, 1-based, or 0. |
| `0x00439B52` | `Labour_Move(county, fromCluster, toCluster, workers)` | verified | Moves workers between two labour slots of one county and re-runs the county's food and industry passes twice. The caller's count is selected icons times county +0xB8 (popBand), clamped to what the source slot holds - so one icon is one twenty-fifth of the population, rounded up. |
| `0x004517CA` | `Job_SlotForCluster(county, cluster)` | verified | Cluster to labour slot through g_jobClusterToSlot, with cluster 0 overridden from stone (5) to iron (4) when the county has industry 1's resource and not industry 3's. That single case pins industry 1 = iron and industry 3 = stone. |
| `0x0045161E` | `Village_RebuildIcons(county)` | verified | Rebuilds g_peasantIcons for one county: workers divided by popBand icons per cluster, with the surplus or shortfall against the slot's +0x04 and +0x08 drawn in a different frame. |
| `0x0043AA32` | `Tax_Increase` | verified | The tax panel's up arrow. Guards taxRate < 0x32 - the player's tax ceiling is 50, and this is where it lives. |
| `0x0043AA83` | `Tax_IncreaseCounty(county)` | verified | Raises one county's tax rate by one, capped at 50, then Tax_RecomputePreview and a redraw. |
| `0x0043AAD5` | `Tax_Decrease` | verified | The tax panel's down arrow. Guards taxRate != 0. |
| `0x0043AB25` | `Tax_DecreaseCounty(county)` | verified | Lowers one county's tax rate by one, floored at 0. |
| `0x0044B80B` | `Tax_RecomputePreview(county)` | verified | Recomputes one county's tax display: +0xC0 taxShown from population, the castle multiplier and the rate; +0x0F = 5 - rate; and +0x16 from g_taxHappinessOther[rate]. It is the only writer of +0x16 anywhere in the binary, so the empire tax happiness term is a table lookup rather than 5 - rate. |
| `0x00448545` | `Panels_RefreshAll` | verified | End of the season pipeline: Ration_Apply, the next-season preview and Tax_RecomputePreview for every county. This is the second Ration_Apply call docs/decisions.md C20 identified from the save. |
| `0x0043A1E9` | `Ration_Increase` | verified | The ration panel's up arrow. Guards rationWanted < 5. |
| `0x0043A23F` | `Ration_IncreaseCounty(county)` | verified | Raises one county's wanted ration by one, capped at 5, then re-applies rations and redraws. |
| `0x0043A2B2` | `Ration_Decrease` | verified | The ration panel's down arrow. Guards rationWanted > 0. |
| `0x0043A307` | `Ration_DecreaseCounty(county)` | verified | Lowers one county's wanted ration by one, floored at 0. |
| `0x0043A379` | `Ration_SliderClick` | verified | The grain-to-livestock slider's hit test: (200,220,24,24) steps down, (325,220,24,24) steps up, and (224,220,102,24) jumps to mouseX - 224. Clamped 0..100, and refused outright unless the county's owner is g_localPlayer. |
| `0x0043A5A9` | `Ration_SetSplit(county, split, sweep)` | verified | Sets county +0x15F and re-runs the food pass; if the new split changes nothing it walks back towards the old value looking for one that does. |
| `0x0043A846` | `Panel_OpenRation` | verified | Opens the ration panel (screen 0x19). |
| `0x0043A8F2` | `Panel_OpenPopulation` | verified | Opens the population panel (screen 0x14). |
| `0x0043A922` | `Panel_OpenHappiness` | verified | Opens the happiness panel (screen 0x16). |
| `0x0043A939` | `Panel_OpenTax` | verified | Opens the tax panel (screen 0x15). |
| `0x0043AC23` | `Turn_End` | verified | The sidebar's full-width bottom button: ends the local player's turn by writing 999 into the realm record's +0x00. |
| `0x0043AE30` | `Sidebar_Button` | verified | The five buttons in the 162x30 strip at y 430, on g_uiHotspotId 1..5: the county's army (screen 0x17), the court (screen 0x09), send supplies (screen 0x18), and two more. |
| `0x00416925` | `Court_Draw` | verified | The court screen (0x09): L2.eng group 70 against the realm record - gold +0x118, iron +0x120, stone +0x128, wood +0x130, six weapon stocks from +0x140, army wages +0xFC and expected tax +0x15C. |
| `0x00438BEC` | `Field_SetType(county, tile, brush)` | verified | Repaints one map tile's field type for a county and re-runs its food passes. Called from a map click with the brush in g_uiHotspotId; this, not any county panel, is how fields are assigned. |

**Globals**

| Address | Name | Confidence | Meaning |
|---|---|---|---|
| `0x0057CB18` | `g_mapZoom` | verified | Campaign map zoom: 0 near (58x30 tiles) or 2 far (10x6). Written only by Map_SetZoom and Map_InitMode, and no caller ever passes 1, so the middle zoom is unreachable. |
| `0x0055CD48` | `g_mapScrollStep` | verified | Lattice columns per scroll step: 1 near, 4 far. Rows move by twice this, which keeps the origins parity. |
| `0x0053E8AC` | `g_mapViewCols` | verified | Lattice columns visible: 8 near, 40 far. The lattice is 65 wide, so neither zoom shows the whole map. |
| `0x0056D67C` | `g_mapViewRows` | verified | Lattice rows visible: 30 near, 128 far. Map_RenderIso draws this many plus one, the first and last half-height. |
| `0x0052AFDC` | `g_mapViewX` | verified | Left edge of the map viewport in screen pixels: 0 at both live zooms (4 at the dead middle one). |
| `0x00553244` | `g_mapViewY` | verified | Y of the first drawn lattice row: 9 near, 21 far. That row is top-clipped, so the visible band starts at viewY + rowStep = 24 at both zooms. |
| `0x00568220` | `g_mapTilePitch` | verified | Screen pixels between lattice columns: 60 near, 12 far - the tile frame width plus two. |
| `0x0057C97C` | `g_mapHalfPitch` | verified | Half the tile pitch: 30 near, 6 far. |
| `0x00567950` | `g_mapTileHalfStep` | verified | The same half pitch again, used as the x step for the half-offset lattice rows and as the divisor in Map_PickTile. |
| `0x0053F65C` | `g_mapRowStep` | verified | Screen pixels between lattice rows: 15 near, 3 far - half the tile frame height. |
| `0x0055CD60` | `g_mapViewRight` | verified | g_mapTilePitch * g_mapViewCols + g_mapViewX. 480 at all three zooms, which is what leaves 160 pixels for the right column. |
| `0x00553254` | `g_mapViewBottom` | verified | (g_mapViewRows + 1) * g_mapRowStep + g_mapViewY: 474 near, 408 far. Agrees with Map_PickTile s own bound and with Clip_Vertical(0x18, 0x1DA) in Map_DrawCountyFlag. |
| `0x005651B4` | `g_mapStartCol` | verified | First visible lattice column, 0..64. Clamped by Map_ClampScroll. |
| `0x005651B8` | `g_mapStartRow` | verified | First visible lattice row, 0..128, and always even - every setter uses an even literal, row & ~1, or steps of two. |
| `0x00522F7C` | `g_mapRotation` | verified | Map orientation, 0/2/4/6. Selects which of four projections Map_BuildLattice writes into g_screenLattice, and is subtracted from a unit facing to pick its sprite. |
| `0x005533A4` | `g_mapScrollDir` | verified | Scroll direction 0..7 clockwise from north, 8 for none. |
| `0x0053F234` | `g_optScrollSpeed` | verified | The Scroll Speed option, 0..100. Map_ScrollThrottle turns it into a minimum interval; 0 disables scrolling. |
| `0x00591524` | `g_drawX` | verified | Destination x for the next blit. The map render walk advances it by g_mapTilePitch per tile. |
| `0x00591528` | `g_drawY` | verified | Destination y for the next blit. The map render walk advances it by g_mapRowStep per lattice row. |
| `0x00567954` | `g_tileCursor` | verified | Byte offset into g_tiles of the tile the render walk is on, i.e. the lattice cell value. Values below 0x0FFF0000 are on-map. |
| `0x0056D594` | `g_latticeRow` | verified | Lattice row cursor inside the render walk. |
| `0x0056D598` | `g_latticeCol` | verified | Lattice column cursor inside the render walk. |
| `0x00569590` | `g_minimapPixels` | verified | The 128x128 minimap picture for the loaded slot, read straight out of MAPnn.PL8 by Minimap_Load. 0x4000 bytes, and 0x569590 + 0x4000 = 0x56D590, just below the render walks cursors. |
| `0x0052AFF0` | `g_minimapCounty` | verified | The 128x128 county id per minimap pixel, the companion raster in MAPnn.PL8. Minimap_Click indexes it as g_minimapCounty[(x-480) + (y-25)*128]. |
| `0x0057A0C4` | `g_minimapMode` | verified | Minimap overlay: 0 owner colours, 1..3 three per-county ratings of the local players own counties. |
| `0x005530C8` | `g_miscCtySheet` | verified | Misc_cty.pl8, loaded by Gfx_LoadCountyMode. Frames 54/55/56/57/58/59/66 are the 162-pixel-wide campaign right panel, 86..90 the realm banners, 91..95 the minimap furniture. |
| `0x0057D3D0` | `g_panelsSheet` | verified | Panels.pl8, entry 10 of the startup preload table. 262 frames: two 52-frame 16x16 border sets at 0 and 204, a 144-frame 12x12 interior texture at 52, and eight 24x24 strip frames at 196. |
| `0x005BB540` | `g_systemSheet` | verified | System2.pl8 or System.pl8, entry 9 of the preload table; swapped by name from the pair of strings at 0x004DBBD0. |
| `0x00568208` | `g_bankBase` | verified | Tile bank 0x00: Base1?.pl8 near, Base2?.pl8 far. Also the sheet the off-map surround is drawn from. |
| `0x0053E914` | `g_bankMtns` | verified | Tile bank 0x04: Mtns1?.pl8 / Mtns2?.pl8. |
| `0x0057D344` | `g_bankRoads` | verified | Tile bank 0x08: Roads1?.pl8 / Roads2?.pl8 - roads, woodland, fields and the county boundary frames. |
| `0x0055409C` | `g_bankTown` | verified | Tile bank 0x0C: Town1?.pl8 / Town2?.pl8. |
| `0x0056898C` | `g_bankCastle` | verified | Tile bank 0x10: Castle1?.pl8 / Castle2?.pl8. Never referenced by the stored map data, only by tiles the loader rewrites. |
| `0x00553224` | `g_spriteSheetA` | verified | Sprite1a.pl8 / Sprite2a.pl8 - armies on the campaign map. |
| `0x0056D8BC` | `g_spriteSheetB` | verified | Sprite1b.pl8 / Sprite2b.pl8 - the second army sheet, selected by a unit type byte of 4. |
| `0x0055CE5C` | `g_flagsSheet` | verified | Flags1a.pl8 / Flags2a.pl8 - the county flags drawn over castle tiles. |
| `0x004D9F48` | `g_preloadTable` | verified | 13 twenty-byte {char name[16]; u32 size;} records loaded once at startup: base01.256, t32_stn1.256, t32_bat1.256, fnt_8, fntl2_9, font_10, fntl2_14, fntl2_22, mouse, system2, panels, l2.eng, vill_gd8. It runs straight into g_resourceTable at 0x004DA050. |
| `0x005691F0` | `g_paletteBase01` | verified | Base01.256, preload entry 0. Screen_DrawCampaign installs it, so this is the campaign screens palette. |
| `0x004D2900` | `g_minimapRealmRamp` | inferred | Eight bytes per realm colour; Minimap_DrawOverlay indexes realmColour*8 + (sourcePixel - 10) to shade county land on the minimap. The stride comes out of the index expression; the tables full extent is not otherwise pinned. |
| `0x004D28F8` | `g_minimapRatingRamp` | inferred | Six bytes indexed by a 0..5 county rating for minimap overlay modes 1..3. Six because the code rejects values above 5. |
| `0x004DC428` | `g_menuBarItems` | verified | Three 16-byte menu-bar records {x, measuredX, y, engGroup, ...}: (10, 6, group 1 File), (group 2 Options), (group 3 Help), each with its drop-down item count. |
| `0x0052F020` | `g_countyTileTally` | verified | 32 dwords, one per county: how much of the viewport that county filled this frame. Map_DrawFrame seeds the current selection with 30 and then takes the maximum. |
| `0x0056D698` | `g_overviewX` | verified | Left edge of the right-hand overview panel: 480. |
| `0x0056D69C` | `g_overviewY` | verified | Top of the overview panel: 48. Screen_HitRegion raises it to 24 in campaign mode. |
| `0x0056D694` | `g_overviewW` | verified | Width of the overview panel: 160. |
| `0x0056D674` | `g_overviewH` | verified | Height of the overview panel: 160. |
| `0x0059154C` | `g_uiHotspotId` | verified | Id of the widget the pointer last acted on. For the minimap buttons 1..3 pick an overlay mode and 4 toggles the map zoom. |
| `0x004EAC4C` | `g_screenHeight` | verified | 480. The vertical companion to g_screenStride, used as the bottom clip by every general blit. |
| `0x004EAC50` | `g_screenId` | verified | Which screen the interface is on. Screen_Draw, Screen_DrawWidgets and Screen_HandleInput all switch on it and nothing else does. The ids are listed in docs/screens-county.md section 1. |
| `0x005530F0` | `g_setupPage` | inferred | Sub-page within the game-setup screen (g_screenId 0x1F), 1..13. |
| `0x00553F54` | `g_jobPanelJob` | verified | Which job the job popup is showing, 1..9 - the same index as L2.eng group 74, and one more than the labour slot. |
| `0x005CD404` | `g_penAdvance` | verified | Width of the text drawn since the last reset, in pixels. Every panel zeroes it, draws a label, and adds it to the x of the value - which is how a label and its value are laid out without either knowing the other's width. |
| `0x00591550` | `g_uiHotspotArg` | verified | Widget record +0x14, published for the callback alongside g_uiHotspotId. |
| `0x0053E9A0` | `g_villageDragCluster` | verified | The peasant cluster the rubber-band selection came from, 1-based; 0 for none. |
| `0x0057C988` | `g_villageDragCount` | verified | How many peasant icons the rubber-band box selected. Multiplied by county +0xB8 to get workers. |
| `0x0053E9EC` | `g_villageDropCluster` | verified | The cluster the drag was dropped on, 1-based. |
| `0x005540A0` | `g_peasantIcons` | verified | Eight clusters of 25 bytes: the sprite frame plus one for each peasant icon on the village screen, or 0 for an empty slot. Rebuilt by Village_RebuildIcons. |
| `0x00553040` | `g_peasantIconSelected` | verified | 25 bytes, one per icon slot of the dragged cluster: 0 unselected, 1 or 2 for the two selected states. |
| `0x004D85A8` | `g_jobClusterOrigins` | verified | Eight {i32 x, i32 y} pairs - where each peasant cluster sits on the village, before the (0x40, g_villageTopY) offset: (22,50) (147,30) (271,26) (268,188) (117,200) (15,245) (146,120) (19,122). |
| `0x004D85E8` | `g_peasantIconOffsets` | verified | 25 {u8 x, u8 y} pairs - a 5x5 grid 16 pixels apart in x and 12 in y, with the rows staggered 0, 4, 8, 0, 4. Twenty-five slots because one icon is one twenty-fifth of the county. |
| `0x004D6780` | `g_jobClusterToSlot` | verified | Eight i32: which labour slot each village cluster is. [5, 0, 1, 3, 6, 7, 8, 2], with cluster 0 overridden to 4 by Job_SlotForCluster. |
| `0x004D63D8` | `g_taxHappinessOther` | verified | i32 by tax rate: the "Other counties" happiness term (county +0x16) that Tax_RecomputePreview writes. 0 up to rate 19, then -1 at 20 falling to -15 at 50. Exactly 51 entries, which is a second reading of the 0..50 tax ceiling. |
| `0x0056D8C0` | `g_countyHistory` | verified | 400 turns x 16 counties x 8 bytes: population as u32 at +0x00 and happiness as u8 at +0x04, indexed by county - 1. What both graph panels draw. g_saveBlocks block 10 is exactly {0x0056D8C0, 51200}, and the shipped lastturn.sav reads back all fourteen counties' stored population and happiness at turn 0. |
| `0x00568DA8` | `g_historyHead` | verified | Write cursor into g_countyHistory, wrapping at 400. |
| `0x00553F30` | `g_historyLength` | verified | How many turns of g_countyHistory are filled, and how many bars Ui_HistoryGraph draws. |
| `0x004DD790` | `g_taxWidgets` | verified | Two 24-byte widget records: the tax panel's up arrow at (192,162) frame 0x15 and its down arrow at (224,162) frame 0x17, both 24x24, kind 4. |
| `0x004DD7C0` | `g_rationWidgets` | verified | Two 24-byte widget records: the ration panel's arrows at (344,130) and (368,130), frames 0x15 and 0x17. |
| `0x004DC680` | `g_sidebarButtons` | verified | Six hotspot records for the map sidebar, offset by (478, 430): five 32-pixel buttons across the 162x30 strip, then the full-width end-turn button below it. |
| `0x004DC620` | `g_minimapModeButtons` | verified | Four hotspot records at (610, 32), a vertical column inside the sidebar's top plate. |
| `0x005B2FA0` | `g_fontHeading` | verified | Fntl2_22.pl8 - every panel heading. |
| `0x005AF8F0` | `g_fontBody` | verified | Fntl2_14.pl8 - every panel body line. |
| `0x005C9A90` | `g_fontSmall` | verified | Fntl2_9.pl8 - the county strip, and nothing else. |
| `0x005AEBA0` | `g_font10` | verified | Font_10.pl8. |
| `0x005CBFB0` | `g_font8` | verified | Fnt_8.pl8 - the battle overlay. |
| `0x0058FEC0` | `g_mouseSheet` | verified | mouse.pl8 - the pointer. |
| `0x00591580` | `g_engText` | verified | l2.eng, whole. Eng_GroupBase indexes the group table from here. |
| `0x0058FDA0` | `g_numberBuffer` | verified | 100-byte scratch that Ui_NumberToBuffer formats into and Ui_DrawNumber draws from. |
| `0x00553D54` | `g_playerNames` | verified | Six 0x2C-byte player names. g_saveBlocks entry 2 is {0x00553D50, 264}, which is 6 x 0x2C. |
| `0x0057C908` | `g_villageTopY` | verified | Where the village scene starts: 64 normally, 132 with Advanced Farming, which is the strip the fertility and weather lines occupy. |
| `0x004D71F0` | `g_glyphWidths` | inferred | One byte per printable character; zero means a four-pixel blank, which is what makes '@' an invisible sign column. |

<!-- END symbols.json: ui -->

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
