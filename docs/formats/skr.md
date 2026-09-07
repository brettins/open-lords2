# `.skr` — battle / skirmish scenario file

The format the user-made **battle maps** live in, and therefore the format the
standalone battle sandbox needs. Produced and consumed by `mapl2.exe`
(Sierra's own *"L2 Battlemap editor"*, 253,440 bytes) and read by `Lords2.exe`.

Status legend, as in [`maps.md`](maps.md): **[V]** = verified against file bytes
and/or the shipped binaries; **[I]** = inferred.

## Corpus — read this before trusting a count

**There is exactly one `.skr` file on either install.**

| File | Size | Notes |
|---|---:|---|
| `F:\games\Lords of the Realm II\USER.SKR` | 133,748 | 29 May 1997; the only one |
| `F:\games\LORDS2\` | — | the DOS install ships no `.skr` at all |

A recursive search of `F:\games`, `E:\` and the user profile found no others.
So every "checked over every file" claim below is a claim over **one** file —
which is why the structure is instead pinned to the two binaries that read and
write it, and why the strongest single check is different: **19 of `USER.SKR`'s
20 maps are byte-identical to the blank map that `mapl2.exe` writes**, so the
terrain block is validated against the editor's own initialiser 19 times over.

`L2MAP.INF` (268 bytes) records the editor's last-used filename as `Eric.skr`,
and `mapl2.exe` defaults new files to `my_maps1.skr`, so user files do exist in
the wild; none are present here.

## Container — three blocks, always 133,748 bytes

**[V] The file has no header, no magic and no length fields.** It is a straight
memory image of three fixed-size arrays. `mapl2.exe` reads the whole file into
one buffer and splits it with three `memcpy`s (`FUN_00419dfa` load,
`FUN_0041bfaf` save), and the save path writes a hard-coded `0x20A74` bytes:

| File offset | Size | Block |
|---|---:|---|
| `0x00000` | 1,760 = `0x6E0` | army table — 40 records of 44 bytes |
| `0x006E0` | 3,660 = `0xE4C` | text table — 20 records of 183 bytes |
| `0x0152C` | 128,328 = `0x1F548` | terrain block — 328 pad + 20 layers of 6,400 |
| | **133,748 = `0x20A74`** | |

```
 1760  +  3660  +  328  +  20 * 80 * 80   =  133748
```

The arithmetic closes exactly, and every one of those four numbers is a literal
in `mapl2.exe`: `0x6E0`, `0xE4C` and `0x1F548` are the three `memcpy` lengths,
`0x20A74` is the write length, and the 328 comes from the per-map offset table
(below). The file is always exactly this size; the editor never writes a short
file and never appends.

## Block 1 — army table (`0x0000`, 1,760 bytes)

**[V] 40 records of 44 bytes = 11 little-endian `u32` counts.** Records are
grouped in pairs by map: `mapl2.exe`'s dialog code indexes them as
`base + mapIndex * 0x58` with the two armies at `+0x00` and `+0x2C`.

| Index | Field | Source |
|---:|---|---|
| 0 | Peasants | **[V]** editor dialog |
| 1 | Crossbowmen | **[V]** editor dialog |
| 2 | Macemen | **[V]** editor dialog |
| 3 | Swordsmen | **[V]** editor dialog |
| 4 | Pikemen | **[V]** editor dialog |
| 5 | Archers | **[V]** editor dialog |
| 6 | Knights | **[V]** editor dialog |
| 7 | Catapults | **[I]** `TROOPS*.ENG` column `Ca` |
| 8 | Siege towers | **[I]** `TROOPS*.ENG` column `To` |
| 9 | Battering rams | **[I]** `TROOPS*.ENG` column `Ra` |
| 10 | Oil | **[I]** `TROOPS*.ENG` column `Oi` |

Fields 0–6 are verified from `mapl2.exe`'s *"Troop composition"* dialog: the
dialog template's static labels are literally `Peasants`, `Crossbowmen`,
`Macemen`, `Swordsmen`, `Pikemen`, `Archers`, `Knights` (also present in German
and French), and `FUN_00419479` fills the seven edit controls of each column
from record offsets `+0x00, +0x04, +0x08, +0x0C, +0x10, +0x14, +0x18` in that
order. **The editor exposes only the first seven fields**; slots 7–10 are never
editable and are `0` in every record of `USER.SKR`.

Fields 7–10 are named from `TROOPS.ENG`, whose column header is
`Pe Xb Ma Sw Pi Ar Kn Ca To Ra Oi` over eleven columns — the same eleven slots,
in the same order, for the same game. `Lords2.exe` clamps exactly columns 7–10
of that table to a maximum of 9, which is the behaviour you would expect of
siege engines and not of troop counts.

### Which record is the attacker

**[V] Record `2m + 0` is the attacker, record `2m + 1` is the defender.**

Read out of the dialog template resource: the *Attacker* group box sits at
x = 70 and the *Defender* group box at x = 125; the edit controls inside the
Attacker box are ids `0x3FA…`, those inside the Defender box are ids
`0x3EC…0x3F1, 0x3F8`. `FUN_00419479` fills the **Defender** ids from
`base + 0x2C` and the **Attacker** ids from `base + 0x00`. This agrees with the
prior art.

`USER.SKR` map 0:

```
attacker  200 peasants,  25 crossbow, 100 mace,  75 sword,  25 pike, 150 archer,  25 knight
defender  100 peasants,  50 crossbow,  50 mace,  50 sword, 100 pike, 100 archer,   0 knight
```

Maps 1–19 all carry the editor's default template for **both** armies,
`{50, 0, 0, 50, 0, 50, 0, 0, 0, 0, 0}` — byte-identical to the eleven-dword
constant at `0x00434040` in `mapl2.exe`.

## Block 2 — text table (`0x06E0`, 3,660 bytes)

**[V] 20 records of 183 bytes**, three NUL-terminated fixed-width fields:

| Offset | Size | Field | Max text |
|---:|---:|---|---:|
| `+0x00` | 13 | short name | 12 chars |
| `+0x0D` | 29 | full name | 28 chars |
| `+0x2A` | 141 | description | 140 chars |

`183 = 13 + 29 + 141`, and the three offsets are literal in `mapl2.exe`
(`FUN_0041bd67` writes the defaults to `base + i*0xB7`, `base + 0x0D + i*0xB7`,
`base + 0x2A + i*0xB7`). The three matching dialog controls are labelled
**"Full name"**, **"Short name"** and **"Brief description"** (German
*Voller Name / Namenskürzel / Kurzbeschreibung*).

**[I] Which of the first two is "short" and which is "full"** follows from the
slot sizes and from the data: the 13-byte field holds `Sample` / `My map`, the
29-byte field holds `Sample Map` / `My battle map`, and `BATTLES.ENG` pairs
`Three Bridges` with `A Bridge Too Far` in the same order.

**Slots are not cleared before writing.** Map 0 of `USER.SKR` has `Sample Map\0`
followed by the stale bytes `st` inside the 29-byte slot. A decoder must stop at
the first NUL and ignore the rest; an encoder that wants byte-identical output
must preserve residue.

The 20 slots line up one-for-one with the 20 `User battle1`…`User battle20`
entries at the end of `BATTLES.ENG` — see [`eng.md`](eng.md).

## Block 3 — terrain (`0x152C`, 128,328 bytes)

**[V] 328 bytes of unused slack, then 20 layers of 6,400 bytes.**

```
map m  ->  file offset  0x152C + 328 + m * 6400   =   0x1674 + m * 0x1900
```

Two independent sources give this exactly:

* `mapl2.exe` holds a 20-entry table of per-map offsets at `0x00433D88`:
  `0x148, 0x1A48, 0x3348, … 0x1DC48` — first entry 328, stride `0x1900` = 6,400,
  last entry + 6,400 = 128,328 = the block size, with no remainder.
* `Lords2.exe` reads a single map with
  `read(file, dst, 0x1900, index * 0x1900 + 0x1674)` (`FUN_0042D806`) — the same
  origin and stride, computed straight against the file.

**[V] Each layer is 80 x 80, one byte per cell, row-major with x fastest**
(`grid[y * 80 + x]`). Both binaries walk it as `for (y < 0x50) for (x < 0x50)`
over a sequential cursor. The editor takes its width and height from
`L2MAP.INF` (both 80), but the 6,400-byte stride is hard-coded, so 80 x 80 is
effectively fixed.

**The 328-byte pad is unexplained.** It is all zero in `USER.SKR`, no code
touches it, and 328 is not a multiple of 80. It is simply where the offset table
starts.

### The blank map — the strongest check we have

`mapl2.exe`'s `FUN_0041A411` initialises a map to all `0x00` except
`(x=40, y=20) = 0x04` and `(x=40, y=60) = 0x0F`. **[V] Maps 1–19 of `USER.SKR`
are byte-identical to that**, 6,400 bytes each, 19 times. A wrong origin or a
wrong stride cannot produce that.

### Terrain byte values

**[V] Twelve byte values are decoded; everything else falls through to open
ground.** The mapping is taken from both binaries, which agree:

| Byte | `mapl2` mask | `Lords2` id | Graphics | Appearance in `USER.SKR` map 0 |
|---:|---:|---:|---|---|
| `0x00` | — | 1 | 16 random ground variants | open field |
| `0x02` | `0x002` | 4 | 11-variant set, flags `0x10\|0x80` | **[V]** an *obstacle*: cell flag `0x10` means impassable. **Not** hills or high ground — nothing on a `.skr` map carries elevation at all, and the high-ground combat modifier is inert here |
| `0x04` | `0x100` | 20 | — | **[V]** deployment marker, side 1 |
| `0x09` | `0x001` | 11 | 49-variant set, flag `0x10` | a continuous river — **[I]** water |
| `0x0A` | `0x020` | 12 | 17-variant set, cell byte 7 = `0x0F` | irregular patches — **[I]** woodland |
| `0x0F` | `0x080` | 30 | — | **[V]** deployment marker, **side 4**. Side **0** deploys at the `0x04` marker; the two sides are numbered 0 and 4, not 0 and 1. Which one is the attacker is still open |
| `0x10` | `0x004` | 7 | multi-cell structure | first row of a bridge |
| `0x12` | `0x005` | 8 | multi-cell structure | middle rows of a bridge |
| `0x14` | `0x004` | 9 | multi-cell structure | last row of a bridge, **written by the game, never by the editor** |
| `0x15` | `0x040` | 13 | 17-variant set (woodland bank, lower index range), flag `0x10` | one-cell-wide straight lines — **unidentified** |
| `0x20` | `0x008` | 3 | gfx `(rand & 7) + 0x20`, flag `0x10` | scattered single cells — **[I]** rocks |
| `0x50` | `0x010` | 6 | gfx `(rand & 7) + 0x7C`, flag `0x10` | **never used** in the only file we have |

The "mask" column is the `u16` the editor keeps per cell (`FUN_0041A054` load,
`FUN_0041A240` save); the "id" column is the byte `Lords2.exe` writes into its
80 x 80 x 8-byte battlefield array (`FUN_0047B8B2`). The alphabet of
`USER.SKR` is `{0x00, 0x02, 0x04, 0x09, 0x0A, 0x0F, 0x10, 0x12, 0x15, 0x20}`
over all 20 layers (128,000 cells) — a strict subset, so no unknown value
appears anywhere in the corpus.

`0x14` is a load-only alias: the editor maps both `0x10` and `0x14` to mask
`0x004` but always writes `0x10` back, so a file round-tripped through the
editor loses every `0x14`. **[V]** `USER.SKR` contains none.

### Bridges are three-part vertical structures

**[V]** When `Lords2.exe` meets `0x12` at cell `(x, y)` it rewrites the cells at
`(x, y+1)` and `(x, y+2)` from `0x10` to `0x14` before assigning graphics. The
result is a column of `7, 8, 8, …, 8, 9`: a distinct near end, a repeated span,
and a distinct far end.

`USER.SKR` map 0 contains two such structures, each 4 cells wide, sitting across
the river with water on both sides — rows 31–35 at columns 20–23 and rows 36–41
at columns 46–49:

```
 31 .......~~~~~~~~~~~~~pppp....=...
 32 .........~~~~~~~~~~~PPPP~~......
 33 .................~~~PPPP~~~.....
 34 ...................~PPPP~~~.....
 35 .................TTTpppp~~~.....       p = 0x10   P = 0x12   ~ = 0x09
```

**[I]** These are bridges. The map's own name is `Sample Map`; the first entry
of `BATTLES.ENG` is `Three Bridges`.

### Deployment markers

**[V]** Bytes `0x04` and `0x0F` are not terrain. For each one `Lords2.exe`
expands the marker into **12 unit start positions** by adding a 12-entry
`(dx, dy)` table at `0x004D9578` to the marker's cell and clamping each result
to `1 … 78`, then overwrites the marker cell with plain ground:

```
(0,0) (-5,0) (5,0) (0,-3) (0,3) (-5,-3) (-5,3) (5,-3) (5,3) (0,6) (0,-6) (-10,-6)
```

`0x04` fills the arrays at `0x00553150` / `0x00544070`; `0x0F` fills
`0x005531B0` / `0x00544050`. A unit picks between them on a byte at
`unitRecord + 0x03` (unit stride `0x34`) in `FUN_0048B9C1` — **zero takes
`0x0F`'s array**, non-zero takes `0x04`'s. That byte is almost certainly the
owning side, but **which side is 0 is not established**, so *which marker is the
attacker's* is an open question. The editor's blank map puts `0x04` at
`(40, 20)` and `0x0F` at `(40, 60)`.

**[V] Every map in `USER.SKR` carries exactly one `0x04` and exactly one `0x0F`.**

## Where the prior art is wrong

The GPL-3 [OpenLotR2](https://github.com/s-ayers/OpenLotR2) project documents
`.skr` in `doc/technical/file-types/.skr.rst`. Its facts were the starting point
here and most of them hold. Corrections, all checked against the file and the
binaries:

| Prior art | Actual |
|---|---|
| "each map is one **64x64** layer" | **80 x 80** — 6,400 bytes, from the offset stride in `mapl2.exe` and the `< 0x50` loops in both binaries |
| text record = 13 + **28** + 141 = **182** | 13 + **29** + 141 = **183**; the field offsets `0`, `0x0D`, `0x2A` and the stride `0xB7` are literals in `mapl2.exe` |
| army record has 11 fields, last 4 unnamed | last four are Catapults / Siege towers / Battering rams / Oil, from the `TROOPS*.ENG` column header |
| terrain layer content not described | twelve decoded byte values, bridges, deployment markers |
| leading 328 bytes of the terrain block | not mentioned; layers do **not** start at `0x152C` |

Its army-record offsets table is also visibly a broken copy-paste (it repeats
"Attacker Army 2 / Defender Army 2" a dozen times and gives two different
offsets for army 20), but the underlying facts — 20 maps, 40 armies of 44 bytes
interleaved attacker-first, then the text table — are right.

Formats are not copyrightable and none of that project's code was read, copied
or translated.

## `L2MAP.INF` — the editor's settings file, not part of `.skr`

268 bytes (`0x10C`), written by `mapl2.exe` `FUN_00419F5D` as a raw dump of its
settings struct. Relevant because it holds the grid dimensions.

| Offset | Type | Meaning |
|---:|---|---|
| `0x00` | u32 | current map index (19 in the shipped file) |
| `0x04` | u32 | ? (32) |
| `0x08` | u32 | ? (14) |
| `0x18` | u32 | **grid width** (80) |
| `0x1C` | u32 | **grid height** (80) |
| `0x28` | u32 x 20 | per-map value, default `0x20`; `FUN_0041A240` copies it from `+0x04` on save |
| `0x78` | u32 x 20 | per-map value, default `0x0E`; copied from `+0x08` on save |
| `0xC8` | char[14] | last-edited filename — `Eric.skr` here, `my_maps1.skr` on a new file |

## Validation and reproduction

```powershell
node E:/dev/lords2/tools/skr/skr.js validate "F:/games/Lords of the Realm II/USER.SKR"
node E:/dev/lords2/tools/skr/skr.js render   "F:/games/Lords of the Realm II/USER.SKR" 0
node E:/dev/lords2/tools/skr/skr.js dump     "F:/games/Lords of the Realm II/USER.SKR"
```

`validate` output:

```
size: 133748  expected 133748 = 20*2*44 + 20*183 + 328 + 20*80*80
  OK   file size is exactly 133748
  OK   328-byte pad at 5420 is all zero (nonzero=0)
  OK   all 60 text fields NUL-terminated inside their slot
  OK   terrain alphabet within the 12 values both binaries decode
  OK   every map carries exactly one 0x04 and one 0x0f marker
  note  maps byte-identical to the editor's blank template (19): 1,2,...,19
```

`render 0` prints the 80 x 80 grid as ASCII; a wrong origin or stride shears it,
and the river, the two bridges and the hill masses are obvious at the right one.

## Open questions

* **Which deployment marker belongs to the attacker.** See above.
* **`0x15`** — one-cell-wide straight lines, drawn from the woodland sprite bank
  at a different index range, flag `0x10`. Fence? Palisade? Road? Not settled.
* **`0x50`** — decoded by both binaries, used by no shipped file.
* **`0x20`** — scattered single impassable cells; "rocks" fits the terrain hints
  in `BATTLES.ENG` but is not proven.
* **The 328-byte pad.** Zero, unreferenced, unexplained.
* The meaning of the per-cell flag bits `0x10` and `0x80` that `Lords2.exe` sets
  from terrain, and of cell byte 7 = `0x0F` on woodland.
* `L2MAP.INF`'s two 20-entry arrays (`0x20`/`0x0E` defaults).
* **Whether a `.skr` can be anything other than 20 maps.** Nothing in either
  binary suggests it can; the count is a literal `0x14` everywhere.

## Ghidra addresses

`mapl2.exe` and `Lords2.exe` are both in the **`mapl2`** Ghidra project
(`E:\dev\ghidra-projects`); scripts in `E:\dev\lords2\ghidra_scripts_skr\`.

```powershell
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
& "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat" `
    "E:\dev\ghidra-projects" mapl2 -process mapl2.exe -noanalysis `
    -scriptPath "E:\dev\lords2\ghidra_scripts_skr" `
    -postScript DecompileFunc.java 0041bd67 0041a054 0041a240 0041a411
```

`FindScalar.java` (new here) lists instructions using a given immediate.

### `mapl2.exe` (253,440 bytes)

| Address | Role |
|---|---|
| `0x00419DFA` | startup: load `l2.sg2`, `l2map.inf`, `l2.eng`, then the last `.skr` |
| `0x0041BF0D` | File ▸ Open — read `.skr`, split into the three blocks |
| `0x0041BFAF` | File ▸ Save — reassemble and write `0x20A74` bytes |
| `0x0041BD67` | File ▸ New — defaults for 20 maps |
| `0x0041A411` | blank one map's terrain layer |
| `0x0041A054` | terrain byte -> editor cell mask (load) |
| `0x0041A240` | editor cell mask -> terrain byte (save) |
| `0x00419479` | troop-composition dialog proc |
| `0x0040ED72` | `l2.eng` lookup by (group, index) |
| `0x0040ED0E` | `l2.eng` group-offset table read (24-bit) |
| `0x00433D88` | 20-entry per-map terrain offset table |
| `0x00434040` | default army record (11 dwords) |
| `0x00465980` | live army array, stride `0x58` per map |
| `0x00466080` | live text array, stride `0xB7` per map |
| `0x00439300` | live terrain block, 128,328 bytes |
| `0x00466FC0` | whole-file staging buffer |
| `0x004CDE20` | `l2map.inf` image, `0x10C` bytes |

### `Lords2.exe` (1,031,680 bytes)

| Address | Role |
|---|---|
| `0x0042D389` | `*.skr` open dialog |
| `0x0042D806` | read map `n`: `offset = n * 0x1900 + 0x1674`, length `0x1900` |
| `0x0047B8B2` | build the battlefield from a loaded layer — the terrain byte -> id map |
| `0x0048B9C1` | place a unit at its side's deployment slot |
| `0x004D9578` | 12 `(dx, dy)` deployment offsets |
| `0x005440E0` | battlefield cell array, 80 x 80 x 8 bytes |
| `0x00553150` | 12 deployment slots for the `0x04` marker |
| `0x005531B0` | 12 deployment slots for the `0x0F` marker |
