# PL8 storage mode 2 — isometric tiles

Status: **solved and validated.** All 2,502 frames in the 32 mode-2 files are
accounted for byte-exactly by the model below (2,364 isometric frames + 134 raw
frames + 4 non-image "grid" tables).

Derived by reading `Lords2.exe` in Ghidra, then validated against the file bytes.
Independently corroborated by prior art (see *Prior art* at the end).

Verified against the GOG Windows release, `F:\games\Lords of the Realm II`.

---

## 1. The headline

**Mode 2 is not a pixel encoding.** It marks a file as belonging to the
isometric map-tile family. The *actual* per-frame encoding is chosen by
**frame-record byte 12**, not by the file header. A mode-2 file can hold plain
raw bitmaps and isometric diamonds side by side, which is why every attempt to
find one uniform "mode 2 codec" failed.

The isometric frames store **only the pixels inside the diamond**, packed
row-major with no control bytes, no run lengths and no row headers. A solid
58x30 tile is exactly 900 bytes because a 58x30 diamond contains exactly 900
pixels. The shape is implicit in `width` and `height`; the game's blitters are
fully unrolled straight-line code with the diamond baked in.

---

## 2. Header (8 bytes) — corrections

| Offset | Type | Meaning |
|--------|------|---------|
| 0x00 | u8 | storage family: 0 = raw, 1 = RLE, **2 = isometric tile set** |
| 0x01 | u8 | **map zoom level** (see below) |
| 0x02 | u16 | frame count |
| 0x04 | u16 | unknown |
| 0x06 | u8 | unknown, always 0 in the corpus |
| 0x07 | u8 | unknown |

### Byte 0x01 is the zoom level (new, verified)

Across the 32 mode-2 files, byte 0x01 predicts the isometric tile size with no
exceptions:

| byte 0x01 | tile size | diamond bytes | files |
|-----------|-----------|---------------|-------|
| 0 | 58 x 30 | 900 | `Base01`, `Base1a-d`, `Mtns1a-d`, `Roads1a-d`, ... |
| 1 | 26 x 14 | 196 | `Batlfix2` |
| 2 | 10 x 6  | 36  | `Base2b-d`, `Mtns2a-d`, `Roads2b-d` |

This matches the binary exactly. `FUN_00406673` computes the start of a frame's
overhang data as `dataOffset + 900` when the global zoom `DAT_0057cb18 == 0`,
`+ 0xC4` (196) when it is 1, and `+ 0x24` (36) when it is 2 — i.e. `+ height^2`
for the three zoom levels.

**A decoder must not depend on this.** Everything needed is derivable per frame
from `width`, `height` and the record bytes. The zoom byte is metadata about
which map zoom the file serves. It is also meaningless for files in the family
that hold no isometric frames (`Fntl2_9` is `2:1` but is entirely raw glyphs).

Note this refutes the reading of bytes 0x00-0x01 as a single little-endian
`uint16` file type: byte 1 takes the values 0, 1 and 2 independently of byte 0,
so a `uint16` would read 0x0002, 0x0102 and 0x0202, not 0/1/2.

---

## 3. Frame record (16 bytes) — the missing fields

| Offset | Type | Meaning |
|--------|------|---------|
| 0x00 | u16 | width (of the bounding box) |
| 0x02 | u16 | height |
| 0x04 | u32 | absolute file offset of the frame's data |
| 0x08 | i16 | X canvas placement |
| 0x0A | i16 | Y canvas placement |
| **0x0C** | **u8** | **shape / encoding type — the real discriminator** |
| **0x0D** | **u8** | **number of extra "overhang" rows** |
| 0x0E | u8 | padding — **never non-zero in any of the 21,344 frames in the corpus** |
| 0x0F | u8 | padding — non-zero in 32 frames, none of them mode 2 |

Bytes 0x0C, 0x0D and 0x0E are demonstrably live: every PL8 draw entry point
loads them into `DAT_00591568`, `DAT_005BB478` and `DAT_0057D3C0` respectively
(e.g. `FUN_0040A21A`, `FUN_0042A7F1`, `FUN_004063C1`, `FUN_00406673`,
`FUN_0040CFD2`). Byte 0x0E is loaded but is zero throughout the corpus.

### Shape byte (0x0C) values

| Value | Meaning | Frame size in bytes | Count in corpus |
|-------|---------|---------------------|-----------------|
| 0 | plain raw rectangle | `width * height` | 138 |
| 1 | isometric diamond only | `height^2` | 1808 |
| 2 | diamond + overhang, full width | `height^2 + rows * width` | 344 |
| 3 | diamond + overhang, **left** half | `height^2 + rows * height` | 104 |
| 4 | diamond + overhang, **right** half | `height^2 + rows * height` | 108 |

`rows` is record byte 0x0D.

Two things worth knowing:

- For types 3 and 4 the per-row length is `height`, which for these tiles equals
  `width/2 + 1` (because `width == 2*height - 2`). Either formula works.
- **For type 1, byte 0x0D is ignored.** 24 `Batlfix2` frames declare type 1 with
  `rows` = 2, 3 or 4 and still occupy exactly `height^2` bytes. The binary
  agrees: `FUN_00406673` only dispatches an overhang blitter when the shape byte
  is 2, 3 or 4. A decoder that adds `rows * something` for type 1 will
  desynchronise on those frames.

---

## 4. Diamond geometry (verified)

Every isometric frame in the corpus satisfies `width == 2*height - 2` with
`height` even (2,364 of 2,364).

Let `hh = height / 2`. Row `r` of the bounding box holds

```
rowWidth(r) = (r < hh) ? 2 + 4*r
                       : 2 + 4*(height - 1 - r)
xStart(r)   = (width - rowWidth(r)) / 2
```

so widths run `2, 6, 10, ... , width, width, ... , 10, 6, 2`. Pixels are stored
strictly in that order, contiguously, top row first. Total = `height^2`.

Palette index 0 is transparent, as everywhere else in PL8; the game's blitters
test each byte against zero before storing.

This is not inferred from the numbers — it is read directly off
`FUN_00452820`, the zoom-0 diamond blitter, which is straight-line unrolled
code. Its destination offsets walk the screen at pitch 640 with row widths
2, 6, 10, 14, 18, 22, ... and its source pointer advances contiguously.

---

## 5. Overhang geometry (verified from the binary)

Types 2, 3 and 4 append `rows` extra records after the `height^2` diamond
block. Each extra record is **not a horizontal row**. It is a **chevron that
traces the diamond's own upper silhouette**, and each successive record is
drawn one screen row higher. That is how the game extrudes mountains, cliffs
and raised roads upward out of a tile without storing a bounding rectangle.

Index the chevron by pair number `m`, each pair being 2 bytes:

```
column(m) = 2*m
row(m)    = |(hh - 1) - m|          // V shape: down the left edge, up the right
```

For a 58x30 tile (`hh` = 15) that is `m = 0..28`: pair 0 at (row 14, col 0),
pair 14 at (row 0, col 28) — the apex — and pair 28 at (row 14, col 56). Those
are exactly the diamond's own left- and right-edge pixels.

| Shape | Pairs stored | `m` range | Bytes per record |
|-------|--------------|-----------|------------------|
| 2 | full chevron | `0 .. width/2 - 1` | `width` |
| 3 | left half | `0 .. hh - 1` | `height` |
| 4 | right half | `hh - 1 .. 2*hh - 2` | `height` |

Extra record `i` (0-based, in file order) is drawn `i + 1` screen rows above the
diamond's own position. So the frame's true bounding box is
`width x (height + rows)`, with the diamond occupying the bottom `height` rows,
and record `i` painting into canvas row `rows + row(m) - (i + 1)`. No write
falls outside that canvas — checked over all 2,364 isometric frames, zero
out-of-bounds.

Blitters read: `FUN_00453E71` (type 2), `FUN_004580A9` (type 3),
`FUN_00459258` (type 4), all at zoom 0. Each advances its source pointer by
exactly the record length per iteration (`ADD 0x3A` = 58 for type 2, `0x1E` = 30
for types 3 and 4) and its destination by one screen row upward.

### One unresolved detail: type 4, the apex pair

For type 4 the natural mapping — the exact mirror of type 3, and the one that
consumes all 30 bytes — is pair `p` (bytes `2p`, `2p+1`) at column `28 + 2p`,
for `p = 0..14`.

The game's own type-4 blitter does something slightly different. `FUN_00459258`
executes `ADD ESI, 0x2` on entry to each record and reads only bytes 2..29; it
paints bytes 2-3 at **both** the apex column and the next column out, and never
reads bytes 0-1.

The two readings agree on columns 30..56 and differ only at the apex column
(28-29). The file data favours the natural mapping: across the 400 type-4
overhang records in the corpus, bytes 0 and 1 are non-zero in 100 and 86
records respectively, so the authoring tool clearly wrote real palette indices
there. Bytes 28-29 (the outermost pair) are non-zero in only 19 and 6 records,
consistent with them being the rarely-reached bottom tip.

**Recommendation: use the natural mapping** (`p -> column 28 + 2p`, all 30 bytes).
It loses no data, and it makes types 3 and 4 exact mirrors. Flagging it because
it is the one place where the shipped binary and the stored data disagree, and
I could not rule out that the game ships an off-by-one here. Rendering both
gives near-identical output; the literal reading simply discards 186 non-zero
bytes across the corpus.

---

## 6. The four `*grid*` files are not images (new, verified)

`Arm_grid.pl8`, `Mercgrid.pl8`, `Villgrid.pl8` and `Vill_gd8.pl8` are the only
mode-2 frames the size model does not fit as pixels. They are **hit-test region
maps at 1/8 resolution — one byte per 8x8 screen block**:

| File | declared | data bytes | `(w/8) * (h/8)` |
|------|----------|-----------|-----------------|
| `Arm_grid`  | 640 x 480 | 4800 | 80 x 60 = **4800** |
| `Mercgrid`  | 640 x 480 | 4800 | 80 x 60 = **4800** |
| `Villgrid`  | 448 x 376 | 2632 | 56 x 47 = **2632** |
| `Vill_gd8`  | 360 x 320 | 1800 | 45 x 40 = **1800** |

Exact in all four cases. Their byte values are also not palette indices: they
span only 9-13 distinct small values (0-14, plus 32 and 60-63), i.e. region IDs.

Corroborated by the binary: `FUN_00417EA7` (the Armoury screen) loads
`arm_grid.pl8` into `&DAT_00542CE0` and then draws the visible screen from a
*different* buffer. The grid buffer's pixel area (`0x00542CF8` = base + 24, past
header and one frame record) is referenced only by `FUN_004357A6`,
`FUN_0043582A` and `FUN_004398F5` — mouse/region code, never a blitter.

So: a decoder should treat these as data, not sprites. They can be detected
without a filename list — `shape == 0` and `dataSize == (w/8) * (h/8)` rather
than `w * h`.

---

## 7. Decoder

```
decodeMode2Frame(buf, rec):
    w, h      = rec.width, rec.height
    shape     = rec[0x0C]
    rows      = (shape >= 2) ? rec[0x0D] : 0      # type 1 ignores it
    hh        = h / 2
    p         = rec.dataOffset

    if shape == 0:
        # plain raw rectangle. (Guard: if size == (w/8)*(h/8) this is a
        # region-ID grid, not pixels — see section 6.)
        return rectangle(w, h, buf[p .. p + w*h])

    # canvas is taller than the declared box: overhang extends upward
    H      = h + rows
    canvas = w x H filled with 0 (transparent)

    # --- diamond, bottom `h` rows of the canvas ---
    for r in 0 .. h-1:
        rowWidth = (r < hh) ? 2 + 4*r : 2 + 4*(h-1-r)
        x0       = (w - rowWidth) / 2
        for x in 0 .. rowWidth-1:
            canvas[rows + r][x0 + x] = buf[p]; p += 1

    # --- overhang chevrons ---
    recLen = (shape == 2) ? w : h
    for i in 0 .. rows-1:
        base = p; p += recLen
        mRange = shape == 2 ? [0 .. w/2 - 1]
               : shape == 3 ? [0 .. hh - 1]
               :              [hh - 1 .. 2*hh - 2]      # shape 4
        k = 0
        for m in mRange:
            row = abs((hh - 1) - m)
            cy  = rows + row - (i + 1)
            for j in 0 .. 1:
                v = buf[base + 2*k + j]
                if v != 0: canvas[cy][2*m + j] = v      # 0 = transparent
            k += 1

    return canvas
```

Invariant: `p` ends exactly at the next frame's `dataOffset` (or EOF).

---

## 8. Validation

Run over the whole game directory:

```
node tools/pl8mode2check.js "F:/games/Lords of the Realm II"
```

Result:

```
mode-2 files: 32   frames ok: 2502   frames bad: 0
  isometric frames (shape 1/2/3/4): 2364, byte-consumption mismatches: 0
  raw frames (shape 0):              134
  1/8-resolution region grids:         4
  out-of-canvas writes:                0
  isometric frames with w == 2h-2 and h even: 2364 / 2364
```

Two independent checks, both passing on all 2,502 frames:

1. **End-offset invariant.** Each frame's decode consumes exactly the bytes from
   its own `dataOffset` up to the next frame's `dataOffset` (or EOF).
2. **Canvas bounds.** Every write from the diamond walk and every chevron pair
   lands inside `width x (height + rows)`; nothing is clipped or dropped.

### Visual confirmation

- `Backgrnd.pl8` frame 0 (`shape 0`, 640x480) with `Backgrnd.256` renders as a
  clean, artefact-free heraldic lions background. Pixel-exact.
- `Title.pl8` frame 0 (`shape 0`, 640x480) renders the title art — castle and
  "Lords of the Realm II" lettering — with correct geometry and no shearing.
  Its **colours are wrong with every shipped `.256`**; the closest by a
  smoothness metric is `Backgrnd.256`, not `Title.256`. That is a
  palette-pairing problem, not a decode problem, and is out of scope here.
- `Base1a.pl8` (`shape 1`) renders crisp isometric diamonds with clean 2:1
  diagonal edges.
- `Mtns1a.pl8` frames 4, 5, 6 (`shape` 2, 3, 4 with 6, 7 and 5 overhang rows)
  render as hills whose peaks rise above the tile — full width, left-leaning and
  right-leaning respectively, matching the shape codes.

---

## 9. Reproducing the Ghidra work

```powershell
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
$GH = "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat"
$A = @("E:\dev\ghidra-projects", "lords2", "-process", "Lords2.exe",
       "-noanalysis", "-scriptPath", "E:\dev\lords2\ghidra_scripts")

# the map-tile draw dispatcher: proves overhang starts at dataOffset + height^2
& $GH @A -postScript DecompSlice.java 00406673 0 99999

# the zoom-0 diamond blitter: row widths 2, 6, 10, ... baked in
& $GH @A -postScript DecompSlice.java 00452820 0 200

# the three overhang blitters (types 2, 3, 4)
& $GH @A -postScript DecompSlice.java 00453e71 0 99999
& $GH @A -postScript DecompSlice.java 004580a9 0 99999
& $GH @A -postScript DecompSlice.java 00459258 0 99999

# raw disassembly, to check the type-4 ADD ESI,0x2 for yourself
& $GH @A -postScript Disasm.java 00459258 40

# where the map tile sets get loaded, and the resource-name table
& $GH @A -postScript DecompSlice.java 004984dc 0 99999
& $GH @A -postScript DumpBytes.java 004d9fc0 400
```

Helper scripts added to `ghidra_scripts/` for this work: `DecompSlice.java`
(decompile a line range — `DecompileFunc.java` truncates at 70 lines),
`RefsTo.java` (functions referencing an address), `FindStrRefs.java` (locate a
literal string and its referrers), `DumpBytes.java`, `Disasm.java`.

### Map of the relevant code

| Address | Role |
|---------|------|
| `FUN_004984DC` | loads the 8 map tile sets from the 20-byte resource table at `0x004DA050` (`name[16]`, `u32 size`) into per-layer globals |
| `FUN_004063C1`, `FUN_00406673`, `FUN_00406BBA` | map tile draw dispatchers; read the frame record, select layer buffer and blitter |
| `FUN_0042A7F1`, `FUN_0042A8D1`, `FUN_0042A9AB` | base-layer draw dispatchers |
| `FUN_00452820` / `FUN_00460501` / `FUN_00463B42` | diamond blitters, zoom 0 / 1 / 2 |
| `FUN_00453E71` / `FUN_00460A1C` / `FUN_00463CBE` | type-2 overhang, zoom 0 / 1 / 2 |
| `FUN_004580A9` / `FUN_00461A54` / `FUN_004640B6` | type-3 overhang, zoom 0 / 1 / 2 |
| `FUN_00459258` / `FUN_00461EFB` / `FUN_00464209` | type-4 overhang, zoom 0 / 1 / 2 |
| `DAT_0057CB18` | global map zoom level (0, 1, 2) |
| `DAT_00591568` / `DAT_005BB478` / `DAT_0057D3C0` | frame record bytes 0x0C / 0x0D / 0x0E |
| `DAT_005AEB6C` | `dataOffset + height^2`, i.e. start of the overhang block |

---

## 10. Prior art

The isometric layout is already public. **https://github.com/s-ayers/pl8image**
(npm `pl8image`, docs at https://pl8image.readthedocs.io/en/latest/.pl8.html),
the parser behind https://github.com/s-ayers/OpenLotR2, documents the same
16-byte record with the type byte at 0x0C and the extra-row count at 0x0D, the
same 0/1/2/3/4 type values, the same `2 + 4*r` row-width progression, and the
same size formulas (`h^2`, `h^2 + rows*w`, `h^2 + rows*(w/2 + 1)`).

The work above was done independently from the binary before that source was
found, and it agrees on every one of those points — which is a useful
cross-check, since the two derivations share no method.

**Licensing: `pl8image` is GPL-3.0 and `l2-formats` is MIT. No code from it has
been read into this repo, copied, or translated.** The overlap above is file
format facts, which are not copyrightable, and everything here was derived from
`Lords2.exe` and from the data files. The decoder in section 7 is written from
the Ghidra reading.

Points this document adds that the prior art does not cover:

- Header byte 0x01 is the map zoom level (0 = 58x30, 1 = 26x14, 2 = 10x6).
- Type 1 ignores the extra-row byte, which 24 `Batlfix2` frames actually
  exercise.
- The exact placement of overhang records: they are chevrons tracing the
  diamond's silhouette, stacked one screen row apart, not horizontal rows.
- The four `*grid*` files are 1/8-resolution region-ID tables, not sprites.
- Record byte 0x0E is loaded by every draw path but is zero throughout the
  corpus.
- The type-4 apex discrepancy in section 5.

Two other sources were checked and contain nothing usable: the XeNTaX thread
(`forum.xentax.com/viewtopic.php?f=18&t=13508`) is dead and not retrievable via
the Wayback Machine, and `retrogamesvault.com/lords2/` hosts only a tool
download with no format notes.
