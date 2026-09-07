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
import table, so that path is dead code.

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
