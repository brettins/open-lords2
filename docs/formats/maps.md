# `L2_maps.dat` — campaign / skirmish map data

Reverse-engineered from the shipped data files and from the map-loading code in
`Lords2.exe` (Windows release, 1,031,680 bytes).

Files examined (read-only, never modified):

| File | Size | Notes |
|---|---|---|
| `F:\games\Lords of the Realm II\L2_maps.dat` | 2,636,880 | Windows release, 80 slots |
| `F:\games\LORDS2\L2_MAPS.DAT` | 1,318,440 | DOS release, 40 slots; byte-identical prefix of the above |
| `F:\games\Lords of the Realm II\Lords2.exe` | 1,031,680 | contains the loader — the reference implementation |

Status legend used throughout: **[V]** = verified against file bytes and/or
disassembly; **[I]** = inferred, plausible but not proven.

---

## 1. Container

**[V] The file is a flat array of fixed-size map slots. There is no header, no
table of contents, and no delimiters.**

```
record size = 6 * (64 * 64)  +  (65 * 129)
            = 6 * 4096       +  8385
            = 24576          +  8385
            = 32961 bytes            (0x80C1)
```

The arithmetic closes exactly, with no remainder, on both releases:

| Release | File size | / 32961 |
|---|---:|---:|
| DOS `L2_MAPS.DAT` | 1,318,440 | **40.0** slots |
| Windows `L2_maps.dat` | 2,636,880 | **80.0** slots |

The Windows release simply **doubled the slot count from 40 to 80**; its first
1,318,440 bytes are byte-identical to the DOS file, and the 40 appended slots
use the identical record layout.

### 1.1 Slot layout

**[V] The six 64x64 planes come first; the 65x129 layer is last.**

| Offset | Size | Contents |
|---|---:|---|
| `0x0000` | 4096 | plane 0 — tile flags |
| `0x1000` | 4096 | plane 1 — graphic bank selector |
| `0x2000` | 4096 | plane 2 — graphic index within bank |
| `0x3000` | 4096 | plane 3 — multi-tile object part index |
| `0x4000` | 4096 | plane 4 — marker payload |
| `0x5000` | 4096 | plane 5 — county id |
| `0x6000` | 8385 | 65 x 129 isometric screen-lattice background layer |
| | **32961** | |

Each 64x64 plane is **row-major, 64 bytes per row, 64 rows**; the tile at
`(x, y)` is at plane offset `y * 64 + x`.

This ordering is not inferred from the data — it is read directly out of the
game's loader (`FUN_00467770` at `0x00467770`), which loads a slot with:

```c
FUN_004af8d9("l2_maps.dat", buf, 0x80c1, map_index * 0x80c1);   // seek + read
for (y = 0; y < 0x40; y++)
  for (x = 0; x < 0x40; x++) {
      b0 = buf[i + 0x0000];  b1 = buf[i + 0x1000];  b2 = buf[i + 0x2000];
      b3 = buf[i + 0x3000];  b4 = buf[i + 0x4000];  b5 = buf[i + 0x5000];
      ...
      i++;
  }
```

and the companion `FUN_0046797d` at `0x0046797d`, which loads the tail:

```c
FUN_004af8d9("l2_maps.dat", buf, 0x80c1, map_index * 0x80c1);
i = 0;
for (row = 0; row < 0x81; row++)          /* 129 */
  for (col = 0; col < 0x41; col++) {      /*  65 */
      lattice[row][col] = buf[0x6000 + i] + 0x0fff0000;   /* dword, row stride 0x104 */
      i++;
  }
```

`FUN_004af8d9(name, buf, size, offset)` is a thin `open` / `lseek(offset, SEEK_SET)`
/ `read(size)` helper — so **slot `n` lives at byte offset `n * 32961`**, straight
from the code.

### 1.2 Slot usage

**[V]** Classifying a slot as *used* when any of its six planes is non-constant:

| Slot range | Count | State |
|---|---:|---|
| 0 – 23 | 24 | used |
| 24 – 39 | 16 | empty |
| 40 – 59 | 20 | used |
| 60 – 79 | 20 | empty |

**44 maps are populated; 36 slots are unused.** The DOS file therefore has 24
used of 40 slots, and the Windows release adds 20 more.

**[V] An unused slot has all six 64x64 planes filled with `0x00`**, and a tail
containing only the two byte values `0x06` and `0x16`. Slots 24–39 are all
byte-identical to one another, and slots 60–79 are all byte-identical to one
another, but the two groups differ (they carry different tail patterns — i.e.
two different "blank map" templates, one authored for the DOS release and one
for the Windows release).

**[V] All 44 used slots are unique.** Slot 40 is a lightly-revised copy of slot 0
(18.7% of bytes differ; its county plane differs by a single byte and its tail is
identical). The other 19 appended maps differ from every DOS-era map by 27–46% of
bytes, i.e. they are genuinely new maps.

---

## 2. The six 64x64 planes

Byte alphabets observed over all 44 used maps (180,224 tiles) — **[V]**:

| Plane | Distinct | Values |
|---|---:|---|
| 0 | 14 | `0x00,01,02,03,04,08,0a,10,12,20,22,40,80,82` |
| 1 | 4 | `0x00, 0x04, 0x08, 0x0c` |
| 2 | 122 | `0x00 .. 0x79` |
| 3 | 9 | `0x00 .. 0x08` |
| 4 | 7 | `0x00 .. 0x06` |
| 5 | 18 | `0x00 .. 0x10`, plus `0x20` |

### Plane 5 — county id  **[V]**

Values `0`–`16` plus `32`. The loader tracks the highest id below `0x11`:

```c
if ((b5 < 0x11) && (max_county < b5)) max_county = b5;
```

so **county ids run 1..16, and `0` means "not part of any county"**. A map has
between 4 and 16 counties. `32` appears in some maps as a distinct non-county
marker.

Rendering this plane produces unmistakable county partitions (see §5).

### Plane 0 — per-tile flag bits  **[V] structure, [I] some meanings**

Every observed value is a combination of single bits, so this is a bitfield.
Observed combinations: `0x03 = 0x02|0x01`, `0x0a = 0x08|0x02`,
`0x12 = 0x10|0x02`, `0x22 = 0x20|0x02`, `0x82 = 0x80|0x02`.

| Bit | Evidence | Meaning |
|---|---|---|
| `0x04` | **[V]** exact invariant | tile is outside every county |
| `0x40` | **[V]** exact invariant | castle / county seat, always a 2x2 block |
| `0x20` | **[V]** from `FUN_00467a36` | dwelling / housing site |
| `0x80` | **[V]** count, **[I]** meaning | settlement (village) tile |
| `0x08` | **[I]** | tile belongs to a multi-tile object (see plane 3). **Not** a reliable test: 2,604 of 9,877 non-zero plane-3 tiles do not set it, being castle and settlement block members |
| `0x01,0x02,0x10` | unknown | — |

Two exact invariants, checked over all 180,224 tiles of all 44 used maps:

* **`(plane0 & 0x04) != 0`  ⟺  `plane5 == 0`  — 100.0000% agreement.**
  Bit `0x04` is exactly "no county".
* **The `0x40` tiles decompose into 434 complete 2x2 blocks with *zero* leftover
  tiles**, and in *every* map the `0x40` tile count is exactly `4 x (number of
  counties)`. So each county has exactly one 2x2 castle. 434 blocks = the sum of
  county counts over the 44 maps.

Bit `0x20` is confirmed as a dwelling site by `FUN_00467a36`, which sweeps the
64x64 grid and, for each tile with a county in `1..17` and `plane0 & 0x20` set,
increments a per-county counter and rewrites the tile's graphic index to one of
three building sizes (base `0x68`, `0x54` or `0x50`, plus a 0..3 variant taken
from the low bits of the stored graphic index) depending on how many dwellings
that county already has and on a difficulty/settings global. That is the game
drawing a county's housing at different sizes.

### Plane 4 — marker payload  **[V] mechanism, [I] full semantics**

Plane 4 is *not* copied into the runtime tile array. It is only consumed at load
time, dispatched on plane 0's marker bits:

```c
if (b4 != 0) {
    if      (b0 & 0x40)  FUN_00429153(b5, b4);   /* castle tiles  */
    else if (b0 & 0x80)  FUN_0049bbe8(b5, b4);   /* settlement tiles */
}
```

* `FUN_0049bbe8(county, n)` writes `table[n] = (county, n)` and increments a
  counter. **[V]** In 31 of the 44 maps there are exactly **5** settlement tiles
  with a non-zero plane-4 value, carrying the values `1,2,3,4,5` — one each.
  The remaining maps have 2 or 4. This is **[I] the table of player/lord starting
  counties**, and the counter is the number of players the map supports (5 is
  Lords of the Realm II's maximum).
* `FUN_00429153(county, n)` appends `county` to the first free slot of a
  16-entry row `n-1` of a 6-row table. On castle tiles plane 4 takes values
  `0..6`, and the four tiles of one castle typically carry several different
  non-zero values, so each county is appended to several rows. **The meaning of
  these six 16-entry county lists is unresolved.**

### Plane 1 + plane 2 — tile graphics  **[V]**

The map renderer (`FUN_004063c1` at `0x004063c1`) uses them as a pair:

```c
county   = tile[+7];                 /* plane 5 */
flags    = tile[+1];                 /* plane 0 */
gfxIndex = tile[+3];                 /* plane 2 */
bank     = tile[+2] & 0x1c;          /* plane 1 */
switch (bank) {                      /* selects one of five sprite tables  */
  case 0x00: base = tbl0; break;   case 0x04: base = tbl1; break;
  case 0x08: base = tbl2; break;   case 0x0c: base = tbl3; break;
  case 0x10: base = tbl4; break;   default: return;
}
desc = base + gfxIndex * 0x10 + 8;   /* 16-byte sprite descriptors */
```

So **plane 1 selects a graphic bank and plane 2 is the index within it**. The
shipped data only ever uses banks `0x00, 0x04, 0x08, 0x0c`; the fifth bank
(`0x10`) exists in code but is never used by any shipped map. Bits `0x01` and
`0x20` of the runtime field are set at run time, not loaded from the file — which
is why plane 1's on-disk alphabet is only `{0, 4, 8, 12}`.

The tile pixels come from the eight `g_resourceTable` entries `Gfx_LoadCountyMode`
loads — `maps-layers.md` §1.1 and `screens.md` §2.1. `MAPnn.PL8` is a different
thing. `Minimap_Load` (`0x0046A037`) selects it:

```c
name = "map01.pl8" + (slot >> 2) * 0x10;          /* 16-byte name table */
read(name, dst0, 0x4000, dirEntry[(slot & 3) * 5    ]);
read(name, dst1, 0x4000, dirEntry[(slot & 3) * 5 + 1]);
```

**Corrected: neither of those reads is a tile set, and the low two bits are not
the season.** They are the two **128 × 128 minimap rasters** for one map slot — a
county id per pixel, and a shading mask — and each `MAPnn.PL8` carries **four map
slots**, which is what `slot & 3` picks. See `docs/screens.md` §3.1. Three things
settle it: the frames are 128 × 128 and hold county ids 0…14 and shades
{0, 10…13, 63}; `Gfx_LoadCountyMode` takes the season from `g_season`, a separate
global; and 11 shipped `MAPnn.PL8` files × 4 slots = **44**, exactly the used-slot
count, with the missing `map07…map10` covering exactly the empty slots 24…39.

So `g_scenarioIndex` is the map slot 0…59 used **unshifted** — `Map_LoadLattice`
seeks `slot * 0x80C1`, the whole slot stride, and `L2.eng` group 101's sixty
strings name slots 0…59 one for one (`maps-layers.md` §6).

### Plane 3 — multi-tile object part index  **[I], strongly supported**

Values `0..8`, and **[V]** every non-zero plane-3 tile also has `plane0 & 0x08`
set. The value histogram is strikingly regular — per map, values `1,2,3` occur in
**exactly equal counts**, and values `4,5,6,7,8` occur in **exactly equal counts**:

```
slot   0:  0->3917   1,2,3 -> 58,58,58     4..8 ->  1, 1, 1, 1, 1
slot   1:  0->3950   1,2,3 -> 47,47,47     4..8 ->  1, 1, 1, 1, 1
slot   3:  0->3846   1,2,3 -> 70,70,70     4..8 ->  8, 8, 8, 8, 8
slot   4:  0->3656   1,2,3 ->112,111,112   4..8 -> 21,21,21,21,21
```

This is the signature of **multi-tile sprites**: a 3-tile object using part
indices `1,2,3` and a 5-tile object using part indices `4..8`. What the objects
are (bridges, fords, large terrain features) is unresolved.

---

## 3. The 65 x 129 layer — isometric screen-lattice background  **[V]**

This is the layer that looked most mysterious, and the game's renderer resolves
it completely.

**[V] Geometry.** 65 columns x 129 rows, one byte per cell, read row-major (65
consecutive bytes per row). Confirmed by the loader's `for(row<0x81) for(col<0x41)`
nest writing into a dword array of row stride `0x104` = 260 = 65*4. Rendering it
at width 65 gives a coherent landmass; rendering at width 129 shears it.

**[V] Value alphabet.** Across all 44 used maps this layer contains **only two
byte values: `0x06` and `0x16`.** No parity/checkerboard structure (both values
occur at both `(row+col)` parities in equal proportion), so it is a plain dense
rectangular array, *not* a staggered vertex lattice.

**[V] What it is.** The loader stores each byte as `value + 0x0FFF0000` into a
129x65 dword array, and the map renderer (`FUN_0040526e`) walks that array cell
by cell:

```c
cell = lattice[row][col];
if (cell < 0x0fff0000) {
    tileOffset  = cell;            /* cell holds an offset into the 64x64 tile array */
    drawMapTile(...);              /* FUN_004063c1 - uses planes 0,1,2,5 */
} else {
    gfxIndex = cell - 0x0fff0000;  /* the byte straight out of the file */
    drawBackgroundTile(...);       /* FUN_0042a7f1 */
}
screenX += tileWidth;
```

So the array is the **screen lattice of the isometric map view**. At run time the
cells that are covered by a real map tile get overwritten with a pointer into the
tile array; the cells that are *not* covered — the off-map surround — keep their
`0x0FFF0000 + b` value, and `b`, the byte from the file, is the **graphic index
of the background tile to draw there**. `FUN_00429800` does the reverse lookup,
scanning the lattice for a given tile offset to find its screen cell.

**[I]** The two values `0x06` and `0x16` are therefore two background tile
graphics — almost certainly two sea/border variants, drawn through the same
sprite table that plane 2 indexes.

**[V] Why 65 x 129.** The dimensions are `(64+1)` x `(2*64+1)`: an isometric view
places two half-rows of lattice cells per tile row, and needs one extra column and
row for the far edges. A brute-force search over affine maps
`row = a*x + b*y + c`, `col = d*x + e*y + f` (all `|coeff| <= 2`) fitting the
layer's `0x06` cells to the tile-grid land mask over 10 maps ranks
`row = x + y` first — the isometric axis — with `col = 64 - y` or `col = x`.
**[V] But the best fit is only 72.15%, so no exact tile-to-lattice
correspondence was established.** That is expected: the layer describes the
off-map surround, which is authored separately and does not have to be a function
of the tile grid. The exact screen-cell-to-tile mapping is computed at run time
by code not yet traced.

---

## 4. What the map editor turned out to be

`mapl2.exe` (253,440 bytes) shipped in the game directory is **not** an editor for
`L2_maps.dat`. Its own error strings identify it:

```
ERR:Exited L2 Battlemap editor, couldn't start direct draw.
ERR:Exited L2 Battlemap editor, Could not find file 'sg2' file.
*.skr    my_maps1.skr    l2map.inf    l2.sg2
```

**[V]** It is the **battle-map (skirmish) editor**, it works on `.skr` files
(`USER.SKR`, 133,748 bytes — a different format that opens with a run of
little-endian dwords), and it contains **no reference to `l2_maps.dat`** at all.
Its saved settings file `L2MAP.INF` (268 bytes) is a dword blob ending in the
last-edited filename `Eric.skr`.

The reference implementation for `L2_maps.dat` is `Lords2.exe`, which contains
the string `l2_maps.dat` twice, referenced from exactly the two loader functions
documented above.

---

## 5. Validation and reproduction

All scripts are in `E:\dev\lords2\tools\maps\` and read the game files in place.

```powershell
# Container validation: slot count, used/empty census, plane alphabets, invariants
node E:/dev/lords2/tools/maps/validate.js
node E:/dev/lords2/tools/maps/validate.js "F:/games/LORDS2/L2_MAPS.DAT"   # DOS file

# Render one map, and a contact sheet of all 44
node E:/dev/lords2/tools/maps/export_png.js 0
node E:/dev/lords2/tools/maps/export_png.js all
# -> E:\dev\lords2\tools\maps\out\map0.png, all_maps.png
```

`validate.js` output (Windows file) confirms:

```
size: 2636880 ; record size: 32961 = 6*4096 + 65*129 = 32961 ; slots: 80
USED slots (44): 0..23, 40..59
EMPTY slots (36): 24..39, 60..79
tail (65x129) value alphabet over used slots: 0x6,0x16
INVARIANT (plane0 & 0x04) <=> (plane5 == 0): 100.0000% over 180224 tiles
INVARIANT 0x40 tiles form 2x2 blocks: blocks=434 leftover tiles=0
```

and for every used slot individually, `castle(0x40) tiles == 4 x counties`.

### Ghidra

`mapl2.exe` and `Lords2.exe` were imported into a **separate** project named
`mapl2` (the `lords2` project was not touched):

```powershell
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
& "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat" `
    "E:\dev\ghidra-projects" mapl2 -import "F:\games\Lords of the Realm II\Lords2.exe" `
    -analysisTimeoutPerFile 2400

# find the record size / filename references
& "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat" `
    "E:\dev\ghidra-projects" mapl2 -process Lords2.exe -noanalysis `
    -scriptPath "E:\dev\lords2\ghidra_scripts_maps" -postScript FindMapConsts.java

# decompile the loaders and consumers
& "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat" `
    "E:\dev\ghidra-projects" mapl2 -process Lords2.exe -noanalysis `
    -scriptPath "E:\dev\lords2\ghidra_scripts_maps" -postScript DecompileFunc.java `
    00467770 0046797d 004af8d9 00429153 0049bbe8 0046a037 004063c1 00467a36 0040526e 00429800
```

Scripts in `E:\dev\lords2\ghidra_scripts_maps\`: `DecompileFunc.java` (copied
from `ghidra_scripts/`), `FindMapConsts.java`, `XrefData.java`, `ScanRange.java`.

### Key addresses in `Lords2.exe`

| Address | Role |
|---|---|
| `0x00467770` | load slot: 6 planes -> 64x64 tile array |
| `0x0046797d` | load slot: tail -> 129x65 screen lattice |
| `0x004af8d9` | `open` / `lseek` / `read` helper |
| `0x0046a037` | select + load `MAPnn.PL8` tileset (4 variants per file) |
| `0x004063c1` | draw one map tile (planes 0,1,2,5) |
| `0x00467a36` | place county dwellings (plane 0 bit `0x20`) |
| `0x0040526e` | isometric map renderer, walks the 65x129 lattice |
| `0x00429800` | reverse lookup tile -> lattice cell |
| `0x00522f90` | runtime tile array, 4096 entries x 8 bytes |
| `0x0055cea0` | runtime screen lattice, 129 rows x 65 dwords (stride `0x104`) |

---

## 6. Open questions

* Plane 0 bits `0x01`, `0x02`, `0x10` — unidentified. Bit `0x02` co-occurs with
  `0x08/0x10/0x20/0x80`, so it is probably a modifier rather than an object class.
* The six 16-entry county lists built from plane 4 on castle tiles.
* What the plane-3 3-tile and 5-tile objects represent.
* The exact run-time mapping from tile `(x,y)` to screen-lattice cell (best
  affine fit is `row = x + y` at 72%, i.e. the isometric axis, but not exact).
* Map **names** are not in this file — there are no strings in `L2_maps.dat`.
  They are presumably in `L2.eng`.
* Whether county id `32` means "sea county" or something else; it appears in some
  maps and not others (0 to 1213 tiles per map).
