# Screens — the campaign map

The presentation layer, starting with the screen the game spends most of its time on.

`docs/symbols.json` had no `ui` section until this document existed, and that gap produced
a real mistake: `crates/l2-view`'s first campaign painter drew all 64 × 64 tiles at once,
which is a view the original does not have. `docs/formats/maps-layers.md` §6 caught that
much; this document is the rest of the screen.

**Status legend.** The three are not interchangeable and the difference is the point.

* **[V] verified** — an exact invariant, or two independent sources agreeing: a number read
  out of the shipped *data* (a PL8 frame table, `L2.eng`) that closes against a number read
  out of the *instruction stream*. Arithmetic that lands on the nose counts here.
* **[D] decompiled** — read out of `tools/oracle/decomp/`. True of the code as written; not
  observed running.
* **[I] inferred** — consistent with everything measured, not proven.

Everything below is reproducible with `rg` over `tools/oracle/decomp/`
(`tools/oracle/decompile-all.ps1` rebuilds it in 22 s) plus reads of the shipped files.

---

## 0. The headline

The campaign screen is **640 × 480**, split into three fixed regions:

```
 0                                            478        640
 +--------------------------------------------+----------+  0
 |  menu bar: File / Options / Help,           banners,   |
 |  year, season, treasury                     24 px tall |
 +--------------------------------------------+----------+  24
 |                                            ||          |
 |  map viewport                              ||  right   |
 |  x [0, 478)   y [24, 474) at near zoom     ||  panel   |
 |               y [24, 408) at far zoom      ||  162 px  |
 |                                            ||  wide    |
 |                                            ||          |
 +--------------------------------------------+----------+  480
```

and it has **two** zoom levels, not three:

| | tile art | tile | pitch | visible | viewport |
|---|---|---|---|---|---|
| near (`0`) | `Base1?.pl8` … | 58 × 30 | 60 × 15 | 8 cols × 30 rows | 480 × 450 |
| far (`2`) | `Base2?.pl8` … | 10 × 6 | 12 × 3 | 40 cols × 128 rows | 480 × 384 |

`FUN_00451FCC` has a third case, zoom `1` (26 × 14 tiles, 17 cols, 64 rows), and it is
**dead code**: `DAT_0057cb18` is written in exactly three places, and never with 1 (§2.2).

**Neither zoom shows the whole map.** The lattice is 65 columns wide; near shows 8 of them
and far shows 40. Far additionally *cannot scroll at all* — the origin is pinned — so a
quarter of the lattice's columns are simply unreachable at that zoom.

---

## 1. The map viewport

### 1.1 `FUN_00451FCC` sets nine globals per zoom  **[D]**

`FUN_00451FCC(zoom)` (`0x00451FCC`) is the only writer of the geometry. Its three cases:

| global | name given | zoom 0 | zoom 1 | zoom 2 |
|---|---|---:|---:|---:|
| `0x0057CB18` | `g_mapZoom` | 0 | 1 | 2 |
| `0x0055CD48` | `g_mapScrollStep` | 1 | 2 | 4 |
| `0x0053E8AC` | `g_mapViewCols` | 8 | 17 | 40 |
| `0x0056D67C` | `g_mapViewRows` | 30 | 64 | 128 |
| `0x0052AFDC` | `g_mapViewX` | 0 | 4 | 0 |
| `0x00553244` | `g_mapViewY` | 9 | 17 | 21 |
| `0x00568220` | `g_mapTilePitch` | 60 | 28 | 12 |
| `0x0057C97C` | `g_mapHalfPitch` | 30 | 14 | 6 |
| `0x00567950` | `g_mapHalfPitchB` | 30 | 14 | 6 |
| `0x0053F65C` | `g_mapRowStep` | 15 | 7 | 3 |

and then two derived bounds:

```c
_DAT_0055cd60 = g_mapTilePitch * g_mapViewCols + g_mapViewX;   /* 480, 480, 480 */
DAT_00553254  = (g_mapViewRows + 1) * g_mapRowStep + g_mapViewY;
```

**`_DAT_0055CD60` is 480 at all three zooms.**  60·8 + 0 = 480, 28·17 + 4 = 480,
12·40 + 0 = 480. That is the width the map is allowed, and 640 − 480 = 160 is what is left
for the right column. **[V]** — three independent constant triples landing on one number.

`DAT_00553254` is 474 / 472 / 408 — the viewport's bottom edge, confirmed twice over in
§1.3.

### 1.2 The pitch is the frame width plus two  **[V] — §4-vs-§6 discrepancy resolved**

`docs/formats/maps-layers.md` §4 gives a tile width of **58** and §6 gives a renderer step
of **60**, and flagged the two as an unresolved disagreement. They are measuring different
things and both are right:

| zoom | PL8 frame (read from the file) | pitch (read from `FUN_00451FCC`) | row step | frame height |
|---|---|---:|---:|---:|
| 0 | `Base1a.pl8` 58 × 30 | 60 | 15 | 30 |
| 1 | (26 × 14; only `Batlfix2.pl8` has this size) | 28 | 7 | 14 |
| 2 | `Base2a.pl8` 10 × 6 | 12 | 3 | 6 |

**pitch = frameWidth + 2 and rowStep = frameHeight / 2, at every zoom.** The two extra
pixels are not padding between tiles: they are the two columns the half-tile blitters drop
at the seam, and they fall outside the clip in both directions (§1.4). §4's 58 is the
*artwork*; §6's 60 is the *column pitch*; they are not interchangeable and §4's derived
3770 × 1935 bounding box is the artwork's, not the renderer's.

Independent confirmation from a third place: `FUN_00406673` adds a fixed byte count to the
frame's data pointer to reach the apex ("overhang") records —

```c
if (zoom == 0) off += 900;  else if (zoom == 1) off += 0xc4;  else off += 0x24;
```

A `w × h` diamond body is `h²/2` bytes: 30²/2 = **450** per half, 900 for both; 14²/2 = 98,
**196 = 0xc4**; 6²/2 = 18, **36 = 0x24**. All three match, which pins the tile sizes from
the instruction stream alone.

### 1.3 `Map_RenderIso` walks a window, and the arithmetic closes  **[V]**

`Map_RenderIso` (`0x0040526E`) draws `g_mapViewRows + 1` lattice rows, alternating between
*aligned* rows (drawn at `x = g_mapViewX + c·pitch`) and *offset* rows (shifted left by
`g_mapHalfPitch`, so they consume one extra lattice column). The first row is drawn with
its top half suppressed and the last with its bottom half suppressed:

```
y  = g_mapViewY;  row = g_startRow
    aligned row, mode 2 (bottom half only)
    y += rowStep; row += 1
    offset row (FUN_00405C2F)
    y += rowStep; row += 1
    repeat (rows-2)/2 times:  aligned row (FUN_00405AE9); offset row (FUN_00405C2F)
    aligned row, mode 1 (top half only)      <- at the y the loop left behind
```

Near zoom: 1 + 1 + 28 + 1 = 31 rows at y = 9, 24, 39 … 459. The first row's bottom half
starts at 9 + 15 = **24**; the last row's top half ends at 459 + 15 = **474**. Far zoom:
129 rows at y = 21 … 405, band **24 … 408**.

Three independent readings agree on those numbers:

* `DAT_00553254 = (rows+1)·rowStep + viewY` — 474 and 408. **[D]**
* `FUN_00429BA4`, the screen→tile hit test, accepts
  `y ∈ [rowStep + viewY, rowStep·rows + rowStep + viewY)` — 24 … 474 and 24 … 408. **[D]**
* `FUN_004081A6`, which draws the county flag, calls
  `Clip_Vertical(0x18, 0x1DA)` — **24 … 474** literally. **[D]**

### 1.4 A clip rectangle reproduces all five blit modes  **[V]**

`Map_DrawTile`'s third argument selects one of five fully-unrolled blitters. Their write
offsets, read out of the decompilation, are:

| mode | blitter (zoom 0 / zoom 2) | what it writes |
|---|---|---|
| 0 | `FUN_00452820` / `FUN_00463B42` | the whole diamond at `(x, y)` |
| 1 | same, early-exit | diamond rows `0 … h/2−1` (bottom clipped) |
| 2 | same, guarded head | diamond rows `h/2 … h−1` (top clipped) |
| 3 | `FUN_0045337B` / `FUN_00463C06` | tile-x `≥ w/2 + 1`, placed as if the tile origin were `x − halfPitch` |
| 4 | `FUN_004538F6` / `FUN_00463C62` | tile-x `≤ w/2 − 2`, tile origin `x` |

The row-1 write of `FUN_00452820` is `*(u16*)(dst + 0x29a) = *(u16*)(src + 2)`;
0x29a = 1·**640** + 26, and 26 is exactly `(58 − 6)/2`, the x-offset of the diamond's
second row. **That is where the 640-byte screen stride is verified**, and every subsequent
row offset in the unrolled listing is `row·640 + (w − rowWidth)/2`.

Modes 3 and 4 between them drop tile-x `w/2 − 1` and `w/2` — two columns, both zooms
(58: drops 28, 29; 10: drops 4, 5). Working out where those land:

* mode 3's tile origin is `viewX − halfPitch`, so its dropped columns land at
  `viewX − 2` and `viewX − 1` — off the left edge.
* mode 4's tile sits at `viewX + halfPitch + (cols−1)·pitch` = 450 (zoom 0) or 474
  (zoom 2), so its dropped columns land at **478 and 479** — under the right panel, whose
  left edge is 478.

So **an ordinary blit clipped to `x ∈ [viewX, 478)`, `y ∈ [24, bottom)` produces the same
pixels as the original's five blitters**, and 478 is not a guess: `FUN_004081A6` calls
`Clip_Horizontal(g_mapViewX, 0x1DE)` and 0x1DE is 478. This is what `l2-view` implements,
and `campaign::tests` asserts the derivation rather than assuming it.

### 1.5 The scroll origin, and what moves it  **[D]**

`0x005651B4` = `g_mapStartCol`, `0x005651B8` = `g_mapStartRow`, indices into the 65 × 129
lattice (`g_screenLattice`, row stride 0x104).

**Clamp** — `FUN_00429B1D` (`0x00429B1D`):

```c
if (col < 0) col = 0;                 if (row < 0) row = 0;
if (col >= 0x41 - viewCols) col = 0x40 - viewCols;
if (row >= 0x81 - viewRows) row = 0x80 - viewRows;
```

Near: col ≤ 56, row ≤ 98. Far: col ≤ 24, **row ≤ 0** — the whole lattice height already
fits, so far zoom has no vertical scroll at all.

**Edge scroll** — `FUN_00432221` → `FUN_00431F59`. The trigger is the *desktop* cursor
sitting on the outermost pixel of the screen (`GetCursorPos` into `0x004E6594`/`0x004E6598`,
compared against `GetSystemMetrics(0)/(1) − 1`), which yields eight directions 0…7 and 8
for "no edge". Each direction moves the origin by `g_mapScrollStep`:

| dir | 0 N | 1 NE | 2 E | 3 SE | 4 S | 5 SW | 6 W | 7 NW |
|---|---|---|---|---|---|---|---|---|
| row | −2s | −2s | 0 | +2s | +2s | +2s | 0 | −2s |
| col | 0 | +s | +s | +s | 0 | −s | −s | −s |

Row moves in twos, which keeps the origin's parity — and the origin is **always even**
(every setter uses an even literal, `row & 0xFFFE`, or ±2). That matters: the traversal
alternates aligned/offset rows starting from the origin, and `maps-layers.md` §4 says
odd lattice rows are the half-shifted ones.

`FUN_00432221` refuses to scroll at all when `g_battlePhase == 0 && g_mapZoom == 2`, so
**the far view is fixed**. `FUN_004BBBE3` throttles the rate from the "Scroll Speed"
option (`_DAT_0053F234`): interval = `((100 − speed)/10)·12 + 2` ms, and a speed of 0
disables scrolling entirely.

**Centre on a county** — `FUN_0043278B(tileOffset)` scans the lattice for the cell holding
that tile and, *only at near zoom*, sets `col = foundCol − 4`, `row = (foundRow & ~1) − 12`.
8 visible columns → −4 is the horizontal centre; 30 visible rows → −12 is not (−15 would
be), so the county lands slightly below centre. Called from the minimap click (§3.2).

**Initial state** — `FUN_00498270` (map-mode init): `row = 0x4A` (74), `col = 0x14` (20),
zoom 0. **Zoom toggle** — `FUN_00434FA1`: near→far saves the origin and sets
`col = 0x0E, row = 0x0C` (which the clamp then pulls to row 0); far→near restores what was
saved. `FUN_004350A1` is the far-zoom click-to-zoom-in: it centres on the clicked tile with
the same `−4 / −12` and switches to near.

### 1.6 Rotation  **[D]**

`FUN_004298C1(rotation)` rebuilds the whole lattice for `rotation ∈ {0, 2, 4, 6}`, and
`FUN_00429F12` / `FUN_0042A01B` step it by ±2 and re-project the scroll origin. Rotation 0
is `maps-layers.md` §4's mapping, recovered exactly from the loop:

```
row = 1 + y + x          col = (64 - y + x) / 2          value = (64*y + x) * 8
```

**Our engine implements rotation 0 only, and says so at the call site.** The other three
are a straightforward transposition of the same loop and are not done.

---

## 2. Zooms

### 2.1 The tile art is loaded per zoom  **[D]**

`FUN_004984DC` (call it `Gfx_LoadCounty`) reloads eight sprite tables when the map mode is
entered *or the zoom changes*:

```c
base = (g_mapZoom == 2) ? 0x20 : 0;
if (0 < g_season && g_season < 5) base += g_season * 8 - 8;
for (i = 0; i < 8; i++)  load resourceTable[base + i]  ->  one of eight pointers
```

The resource table is 20-byte `{char name[16]; u32 size;}` records at **`0x004DA050`**
(`maps-layers.md` §1.1). Entries 0–31 are the 58 × 30 sets in four seasons; 32–63 are the
10 × 6 sets. The eight destinations, recovered from the chain of conditional moves, are:

| i | file (season a) | pointer | used by |
|---|---|---|---|
| 0 | `base1a.pl8` | `0x00568208` | bank `0x00` |
| 1 | `mtns1a.pl8` | `0x0053E914` | bank `0x04` |
| 2 | `roads1a.pl8` | `0x0057D344` | bank `0x08` |
| 3 | `town1a.pl8` | `0x0055409C` | bank `0x0C` |
| 4 | `castle1a.pl8` | `0x0056898C` | bank `0x10` |
| 5 | `sprite1a.pl8` | `0x00553224` | armies (`FUN_00408438`) |
| 6 | `sprite1b.pl8` | `0x0056D8BC` | armies, second sheet |
| 7 | `flags1a.pl8` | `0x0055CE5C` | county flags (`FUN_004081A6`) |

Rows 0–4 are exactly the five banks `Map_DrawTile` selects on `plane1 & 0x1c`, in order —
an independent confirmation of `maps-layers.md` §1.1 from the loader rather than from the
frame counts. `misc_cty.pl8` is loaded alongside into `0x005530C8` (§4).

### 2.2 Zoom 1 is unreachable  **[V]**

`DAT_0057CB18` has exactly three writers in 2,452 functions:

```
00450000.c:911   DAT_0057cb18 = param_1;     /* FUN_00451FCC */
00490000.c:3033  DAT_0057cb18 = 0;
00490000.c:3481  DAT_0057cb18 = 0;           /* FUN_00498270 */
```

and `FUN_00451FCC` has exactly seven callers, passing `2`, `0`, `0`, `0`, `0` and
`DAT_0057cb18` (three times). **Nothing can ever set it to 1.** Corroboration from three
directions: `Map_DrawTile` has no zoom-1 blitter branch at all; `FUN_004984DC` would load
the 58 × 30 art for zoom 1 while `FUN_00406673`'s `+0xc4` expects 26 × 14; and no shipped
PL8 holds 26 × 14 map tiles — the only 26 × 14 file in the install is `Batlfix2.pl8`,
which is battlefield art. **[I]** it is the DOS build's middle zoom, left in.

### 2.3 The selected county follows the viewport  **[D]**

`FUN_004050F6`, the per-frame map draw, is not only a draw:

```c
for (i = 0; i < 0x21; i++) tally[i] = 0;
tally[g_selectedCounty] = 30;              /* hysteresis for the incumbent */
Map_RenderIso();                           /* +1 per tile drawn, +5 if flags & 0x40 */
...
best = argmax(tally[1..31]);
if (best != g_selectedCounty && g_mapZoom == 0) g_selectedCounty = best;
```

So at near zoom **the county the right panel describes is whichever county fills most of
the viewport**, weighted 5× for its castle tiles and with a 30-tile head start for the one
already selected. Scrolling changes the panel. At far zoom the selection is left alone.

This is not reproduced in our engine yet, and the code says so.

---

## 3. The minimap

### 3.1 It is a picture in `MAPnn.PL8`, not a rendering  **[V]**

`Map_LoadTileSets` (`0x0046A037`) — a **misnomer in `symbols.json`; it loads minimaps** —
reads two 128 × 128 rasters out of a `MAPnn.PL8`:

```c
name = "map01.pl8" + (slot >> 2) * 0x10;                 /* map01 … map15 */
File_ReadChunk(name, &off, 4, (slot & 3) * 0x50 + 0xc);
File_ReadChunk(name, &DAT_0052AFF0, 0x4000, off);        /* county id per pixel */
File_ReadChunk(name, &off, 4, ((slot & 3) * 5 + 1) * 0x10 + 0xc);
File_ReadChunk(name, &DAT_00569590, 0x4000, off);        /* the picture */
```

`(slot&3)*0x50 + 0xc` is `8 + record*16 + 4` for record `(slot&3)*5` — the PL8 frame
record's data-offset field. So each `MAPnn.PL8` carries **four map slots**, at frames
`0, 5, 10, 15` (county ids) and `1, 6, 11, 16` (pictures). Reading the files back:
those eight frames are exactly the 128 × 128 ones in every `MAPnn.PL8`. **[V]**

Two things fall out and both check:

* the second parameter is `g_scenarioIndex` and it is **the map slot, 0…59**, used
  unshifted — `Map_LoadLattice(slot)` seeks `slot * 0x80C1` (32,961, the slot stride) and
  `Eng_DrawString(101, g_scenarioIndex, …)` indexes `L2.eng` group 101, whose 60 strings
  `maps-layers.md` §6 lines up one-for-one with slots 0…59. **`maps.md`'s "the low 2 bits
  of the scenario select the season" is wrong** — the season is a separate global,
  `g_season`, and the low 2 bits pick which of four slots inside a `MAPnn.PL8`.
* the install ships `Map01…Map06` and `MAP11…MAP15` — **11 files × 4 slots = 44**, exactly
  the used-slot count, with the missing `map07…map10` covering slots 24…39, exactly the
  empty ones. **[V]**, and a genuinely independent corroboration of the §6 census.

### 3.2 Where it sits, and what it does  **[D]**

`FUN_00410AA9(x, y)` is called as `FUN_00410AA9(0x1E0, 0x19)` = **(480, 25)** and draws:

* the 128 × 128 picture at `(x − 2, y + 3)` = **(478, 28)** — flush with the panel's left
  edge, and 28 + 128 = 156, exactly where the panel's next section begins (§4);
* a county tint over it (`FUN_00410CBD`): a source pixel of 10…13 is land inside a county
  and is replaced from a 8-entry-per-realm ramp at `0x004D2900` indexed
  `realmColour*8 + (v − 10)`; the *selected* county's shade-10 pixels become index 0x20.
  Anything else (sea, coast, 0x24) is left as the picture drew it;
* a 29 × 123 strip (`misc_cty` frame 91 or 92) at `(x + 0x83, y + 7)` = (611, 32);
* one of `misc_cty` frames 93/95/94 at (485, 30) when the overlay mode is 1/2/3.

`DAT_0057A0C4` is the overlay mode: 0 = owner colours, 1/2/3 = three per-county ratings of
the player's own counties (`+0x0B3`, `+0x0B2`, `+0x0B1`) through a 6-entry ramp at
`0x004D28F8`. `FUN_0043AB76` switches it from a hotspot id `DAT_0059154C`: 1–3 pick a mode,
**4 toggles the zoom**.

**Clicking the minimap** — `FUN_0043253A`:

```c
if (x < 0x1E0 || x > 0x25F) return 0;      /* 480 … 607 */
if (y < 0x19  || y > 0x98)  return 0;      /*  25 … 152 */
county = (&DAT_0052AE10)[x + (y - 0x19) * 0x80];
```

`0x0052AE10 + 480 = 0x0052AFF0`, which is the county-id raster `Map_LoadTileSets` filled —
so the index is `(x−480) + (y−25)·128` and the clickable rectangle is exactly the raster,
**128 × 128 at (480, 25)**. A non-zero county selects it and calls `FUN_0043278B` to centre
the map on it.

The picture is blitted at (478, 28) and hit-tested at (480, 25). **That 2-pixel / 3-pixel
disagreement is the original's, not a misreading** — both rectangles are literal constants
in the two functions.

---

## 4. The chrome

### 4.1 `Panels.pl8` is a framed-box kit  **[V]**

`Panels.pl8` (262 frames) is loaded once at startup into **`0x0057D3D0`**, entry 10 of the
13-entry preload table at `0x004D9F48` (`base01.256`, `t32_stn1.256`, `t32_bat1.256`,
`fnt_8`, `fntl2_9`, `font_10`, `fntl2_14`, `fntl2_22`, `mouse`, `system2`, **`panels`**,
`l2.eng`, `vill_gd8`). Its frame table decomposes exactly, and `FUN_00409934` /
`FUN_00409C93` confirm the decomposition from the other side:

| frames | size | role |
|---|---|---|
| 0, 1, 2, 3 | 16 × 16 | corners TL, TR, BR, BL |
| 4 … 15 | 16 × 16 | top edge, 12 variants, cycled `(c−1) % 12` |
| 16 … 27 | 16 × 16 | bottom edge, 12 |
| 28 … 39 | 16 × 16 | left edge, 12 |
| 40 … 51 | 16 × 16 | right edge, 12 |
| 52 … 195 | 16 × 16 | **12 × 12 interior texture**, indexed `52 + c%12 + (r%12)*12` |
| 196 … 203 | 24 × 24 | a separate 8-wide horizontal strip |
| 204 … 255 | 16 × 16 | a second complete border set (`+0xCC`) |
| 256 … 260 | 13 × 16 | five small glyphs |
| 261 | 48 × 48 | one large glyph |

4 corners + 4 × 12 edges = **52 = 0x34**, where the interior starts; 52 + 144 = **196 =
0xC4**, where the 24 × 24 strip starts; 196 + 8 = **204 = 0xCC**, exactly the offset
`FUN_00409934` adds for the second border set; 204 + 52 = **256**, where the glyphs start.
Every boundary lands on the nose, and the frames' own canvas-placement fields (`X`, `Y`)
lay the interior out as a 12 × 12 grid at 16-pixel steps on the artist's sheet.

`FUN_00409397(x, y, wCells, hCells)` is the composite: `FUN_00409934` draws the border in
16-pixel cells and `FUN_00409C93` fills the interior with the texture. Style 2 replaces the
two top corners with edge pieces and omits the top edge — an open-topped box.

### 4.2 The menu bar  **[D]**

`FUN_00419C78` draws the 640 × 24 strip at y = 0:

* background: `FUN_00409E09(0, 0, 25, 1)` then `FUN_00409E09(0x250, 0, 2, 1)` — 24 × 24
  `Panels.pl8` frames 196 + (c mod 8), at 24-pixel steps. 25 · 24 = 600 and the second call
  at x = 592 covers 592…639: **640 px exactly**, with an 8-pixel overlap. **[V]**
* `FUN_00403FDD(0, 0, 0x280, 0x18)` — a bevel outline of the whole bar: top and right in
  palette index 0x1F, bottom and left in 0x10.
* `FUN_0040C5B0(&DAT_004DC428, 3)` — three menus at y = 6 from a 16-byte-per-entry table:
  x = 10, then +32 px between items, `L2.eng` groups **1 = "File"**, **2 = "Options"**,
  **3 = "Help"** (each group's string 0 is the title and the rest are its items — 4, 5 and
  7 items, matching the table's trailing counts exactly). **[V]**
* realm banners: for each realm 1…5 that is alive, `misc_cty` frame `realmColour + 0x55` at
  `x = 270 + i·16, y = 4`. The colour byte is clamped to 1…5 by `FUN_004171EE`, so the
  frames used are **86…90**, and those five are 13 × 16 — which fits the 24-pixel bar,
  while frame 85 (13 × 37) does not. **[V]**
* year at x = 360, y = 6 and the season name after it (`L2.eng` group 29:
  "No Season/Spring/Summer/Autumn/Winter"); the local realm's treasury at x = 500, y = 6.

### 4.3 The right panel is five `Misc_cty.pl8` frames, and they tile exactly  **[V]**

`Misc_cty.pl8` is loaded per map mode into `0x005530C8` (§2.1). Its 162-pixel-wide frames
are the campaign panel, and `FUN_0040F5FD` / `FUN_0040F7D3` place them:

| frame | size | at | covers | when |
|---:|---|---|---|---|
| 54 | 162 × 132 | (478, 24) | y 24 … 155 | always (holds the minimap) |
| 55 | 162 × 94 | (478, 156) | y 156 … 249 | county is the player's |
| 66 | 162 × 52 | (478, 250) | y 250 … 301 | county is the player's |
| 56 | 162 × 128 | (478, 302) | y 302 … 429 | county is the player's |
| 58 | 162 × 274 | (478, 156) | y 156 … 429 | county is **not** the player's |
| 57 | 162 × 30 | (478, 430) | y 430 … 459 | always |
| 59 | 162 × 20 | (478, 460) | y 460 … 479 | always — the **End turn** strip |

24 + 132 = 156; 156 + 94 = 250; 250 + 52 = 302; 302 + 128 = 430; 156 + 274 = 430;
430 + 30 = 460; 460 + 20 = **480**; and 478 + 162 = **640**. The column is covered top to
bottom and edge to edge with no gap and no overlap, under either of the two middle
layouts. The panel's left edge at 478 is the same 478 the map clips to (§1.4).

Frame 59 carries `L2.eng` group 4, **"End turn"**, centred in 162 px at (478, 462) in
`fntl2_9`, colour 0x16 — so the End Turn button *is* that strip.

### 4.4 Palette  **[V]**

`FUN_0040F5FD` ends with `FUN_004B0AB5(0x5691F0)`, and `0x005691F0` is preload entry 0:
**`Base01.256`**. `FUN_004B0AB5` widens 6-bit VGA to 8-bit by multiplying by 4 and then
**forces entry 0 to black**. (Our `Palette` scales by 255/63 instead of 4; the difference is
at most 3/255 per channel and is not corrected here.)

---

## 5. What is drawn on the map

All of these are called from inside the same lattice walk, so they inherit the viewport
cursor `DAT_00591524` / `DAT_00591528`.

| function | draws | from |
|---|---|---|
| `Map_DrawTile` `0x004063C1` | the terrain diamond | one of the five banks |
| `FUN_00406673` | the diamond's apex ("overhang") rows | same |
| `FUN_0042A7F1` | an off-map surround tile | `base` bank, frame = lattice byte − 0x0FFF0000 |
| `FUN_004071A0` | settlements and their state (`flags & 0x80`) | `base`/`town`/`castle` |
| `FUN_004081A6` | the **county flag** on a castle tile (`flags & 0x40`) | `flags1a.pl8`, frame `castleLevel + 0x38`, or 0x4E |
| `FUN_00408438` | **armies** | `sprite1a`/`sprite1b`, offset by an 8-rotation × 16 table per zoom at `0x004D8108` … `0x004D8388` |
| `FUN_00408C50` | a debug number over each tile | gated on `DAT_005BB4A4/A5` |

County **borders are in the tile data**, not an overlay: `maps-layers.md` §2.1 — plane-0
bit `0x02` switches the tile to the `roads` bank's boundary frames. There is no separate
outline pass, and the yellow "selected county" outline our engine draws is ours.

The army sprite direction is `unitFacing − mapRotation` mod 8 (`FUN_00408438`), which is
the one place the rotation feature reaches past the lattice.

---

## 6. Screen regions for input  **[D]**

`FUN_00432640` classifies the cursor into three regions:

```c
if (y < 0x18)                    region = 1;    /* the menu bar */
else if (0x1E0 <= x < 0x1E0+0xA0 && 0x30-0x18 <= y < 0x30+0xA0) region = 2;
```

The four constants are set in `FUN_00498270`: `0x0056D698 = 0x1E0` (480),
`0x0056D69C = 0x30` (48), `0x0056D694 = 0xA0` (160), `0x0056D674 = 0xA0` (160). In campaign
mode the `−0x18` moves the top of region 2 to y = 24, so region 2 is
**x ∈ [480, 640), y ∈ [24, 208)**. (In battle the same rectangle starts at y = 48 and holds
a 160 × 160 battle overview at 2 px per cell — `FUN_00432443` maps a click there through
`(x − 480)/2, (y − 24)/2`, which is an 80 × 80 battlefield and agrees with correction C8.)

Clicks on the map go through `FUN_00429BA4`, which inverts the projection with a
half-pitch modulo and a diagonal tie-break, then reads `g_screenLattice[startRow + dy][startCol + dx]`
and fails if that cell is off-map. Our engine picks off a tag plane instead — exact by
construction for "which tile can the player see here", and *not* the same algorithm.

---

## 7. What our engine does, and what it does not

Implemented in `crates/l2-view/src/campaign.rs` and `crates/l2-view/src/chrome.rs`:

* both real zooms, the real tile art, the real pitches, the scrolling window, the clamp,
  the eight scroll directions and the `−4 / −12` centring;
* the 24-pixel menu bar tiled from `Panels.pl8` 196…203, the framed-box kit, the
  `Misc_cty.pl8` right panel, the `MAPnn.PL8` minimap with its owner tint;
* the clip rectangle derivation of §1.4, asserted rather than assumed.

**Left as ours, and labelled as such in the code:**

* the map **rotation** (only rotation 0);
* the load-time **randomisation of background tile variants** (`maps-layers.md` §4.1) — we
  draw the stored index, so the surround repeats where the original varies it;
* the **auto-selection of the county filling the viewport** (§2.3);
* the county **outline** and the county **markers** — invented, no original equivalent;
* everything the right panel puts *inside* frames 55/66/56/58 (that is the other agent's
  county panel), and the four minimap overlay modes' contents;
* armies, flags and settlement state on the map;
* our own 5 × 7 font, wherever text is drawn.
