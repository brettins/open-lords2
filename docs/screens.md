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
* `FUN_004081A6`, the path-preview marker (C49; it is not the county flag), calls
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
**the far view is fixed**. `Map_ScrollThrottle` (`0x004BBBE3`) throttles the rate from the
"Scroll Speed" option (`g_optScrollSpeed`, `0x0053F234`): interval =
`((100 − speed)/10)·12 + 2` ms, and a speed of 0 disables scrolling entirely.

**[V] The default is 60, which is 50 ms, which is 20 tiles a second.** Written by the
options-defaults routine at `0x004AE310` — unnamed in `symbols.json`, and also the writer of
`g_optGameSpeed = 90` (`0x0053F230`) and the settings magic `0x7EC` at `0x0053F204` that
gates a re-default. The persisted settings block is `0x0053F1E0`, 0x468 bytes, and the two
speed options sit at `+0x50` and `+0x54`. `Menu_ScrollSpeed` (`0x00434CEE`) opens the slider
as `min 0, max 100, step 10, format 1` — eleven settings shown as 0 … 10 — and it forms the
pointer as `0x53F1E0 + 0x54` rather than pushing the address, which is why a byte scan for
readers of `0x0053F234` misses the menu. There are exactly two other references in `.text`:
the default above, and the `sub` inside the throttle.

**Three things worth having beside the formula.** The clock is **`timeGetTime`** (WINMM), not
`GetTickCount`. There is a `g_screenId == 0x10` special case that adds 2 to the quotient, i.e.
**+24 ms**; `0x10` is a gap in `screens-county.md`'s id table. And `Map_EdgeScroll` runs on
screens `0x00`, `0x04`, `0x10`, `0x28` and `0x29`, not the campaign map alone.

**The gate is on the movement, not on the detection.** `Map_EdgeScroll` is called
unconditionally every frame from `Screen_FrameInput` — deliberately outside the main loop's
`ticksDue` gate, which is why the map scrolls smoothly while the simulation ticks at game
speed. `Map_ScrollStep` applies the move, calls the throttle, and **undoes it** by restoring
four saved globals if the interval has not elapsed. The remainder is discarded rather than
carried (`g_lastScrollTick = now`), so at high speeds the rate degenerates to one tile per
frame. `crates/l2-game`'s map ignored all of this and scrolled one tile per fixed tick — 62.5
a second against 20 — which a player reported; `docs/decisions.md` C59.

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
| 5 | `sprite1a.pl8` | `0x00553224` | armies, mobs **and merchants** (`FUN_00408438`) |
| 6 | `sprite1b.pl8` | `0x0056D8BC` | transports, and only transports |
| 7 | `flags1a.pl8` | `0x0055CE5C` | the two flags (`FUN_004071A0`) **and** the path balls (`FUN_004081A6`) — §5.1 |

Rows 0–4 are exactly the five banks `Map_DrawTile` selects on `plane1 & 0x1c`, in order —
an independent confirmation of `maps-layers.md` §1.1 from the loader rather than from the
frame counts. `misc_cty.pl8` is loaded alongside into `0x005530C8` (§4).

**The season is a whole-bank swap, not a per-frame variation, and it is `g_season`
directly.** The `+ g_season * 8 - 8` above is the entire mechanism: the eight pointers are
repointed at eight *different files* and every frame index in the renderer keeps its
meaning. So the `a`/`b`/`c`/`d` suffix on `Base1?.pl8`, `Mtns1?.pl8`, `Roads1?.pl8`,
`Town1?.pl8` and `Castle1?.pl8` **is** the season, four sets per zoom, entries 0–31 and
32–63. `Flags1a.pl8` is the exception that proves the stride: entries 7, 15, 23 and 31 all
name it, and `Flags1b/c/d.pl8` ship and are never loaded.

Two consequences worth writing down.

* **A player who says "the season graphics change" is describing this call.** It is also the
  best available explanation of `FUN_004B0CB4`, the ~320 ms fade to quarter brightness over
  palette entries 10 … 245 with exactly two call sites, both on the turn boundary: the
  reload runs inside the dark window. That was an unsupported `[I]`; a person reporting the
  swap independently is a second source for it.
* **The overrides plane survives a season, and it has now been measured.** C41's
  `campaign::Overrides` stores `(bank byte, frame)` and the reload changes neither, so the
  town's 47 … 58 mean the same thing in every season *provided* the four seasonal files of a
  bank share a frame table. This paragraph used to end *"that is the one thing here nobody
  has measured"*. **They do share one.** Over all five near-zoom banks and 1,398 frame
  comparisons the frame count, the canvas anchor, the size and the shape are identical in
  every season; the only difference anywhere is the overhang-row byte on nine `Roads1?.pl8`
  crop frames, which is a taller crop needing a taller picture and not an index moving.
  `maps-layers.md` §1.1a has the numbers and
  `l2-view/tests/install.rs::the_four_seasons_of_a_bank_are_the_same_frame_table` asserts
  them. So the season is a lookup table, and towns do not revert to quarries in spring —
  `l2-game/tests/screens.rs::a_towns_overridden_graphic_survives_every_season` checks that
  at the pixel in all four.

**One correction to the paragraph above, and it matters to anyone implementing this.** The
sentence *"the suffix **is** the season, four sets per zoom, entries 0–31 and 32–63"* is
right for entries 0–31 and **wrong for 32–63**. The zoom-2 half of the table names
`base2a`/`mtns2a`/`roads2a`/`town2a`/`castle2a` in **all four** of its season blocks: the far
view is not seasonal, and `Base2b.pl8` and its eleven siblings ship and are never loaded,
exactly as `Flags1b/c/d.pl8` do. Worse for a naive fix, the dead files are not
interchangeable with the live one — `Town2a.pl8` has 61 frames and `Town2b/c/d.pl8` have 94 —
so deriving the far zoom's filenames from the season letter draws a different sheet for three
seasons in four. `maps-layers.md` §1.1b.

**Ours draws the season.** `l2_view::campaign::Zoom::banks` is a 4 × 5 table per zoom,
transcribed from `g_resourceTable` rather than generated from the suffix, and `MapAssets`
interns it by filename — twenty-five names, twenty distinct files. `campaign::draw` takes
`g_season` and `MapScreen`'s repaint key carries it.
`a_real_turn_turns_the_season_and_the_map_is_repainted_from_other_files` ends a real turn and
requires the picture to change, rather than assigning to `season` and reading the lookup back.

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
* a county tint over it (`Minimap_DrawOverlay`, `0x00410CBD`): a source pixel of 10…13 is
  land inside a county and is replaced from a 8-byte-per-realm ramp at `0x004D2900` indexed
  `realmColour*8 + (v − 10)`; the *selected* county's shade-10 pixels become index 0x20.
  Anything else (sea, coast, 0x24) is left as the picture drew it;
* a 29 × 123 strip (`misc_cty` frame 91 or 92) at `(x + 0x83, y + 7)` = (611, 32);
* one of `misc_cty` frames 93/95/94 at (485, 30) when the overlay mode is 1/2/3.

### 3.3 The three statistic overlays  **[V]**

`g_minimapMode` (`0x0057A0C4`) is the overlay: 0 = owner colours, and **1 labour, 2 food,
3 happiness**, each of which colours *only the local player's own counties* — all three
branches of `Minimap_DrawOverlay` test `owner == g_localPlayer` and skip the pixel
otherwise, leaving the raster's own shade.

**Two adjacent tables, not one.** `0x004D28F8` and `0x004D2900` are eight bytes apart and
it is worth writing the bytes out, because the two have different strides:

```text
004d28f8  0f 15 f3 09 f1 05 | 05 05      the rating ramp, then two bytes nothing indexes
004d2900  0a 0b 0c 0d 20 0b 0c 0d        realm colour 0 — the shades unchanged
004d2908  01 0e 0f f9 20 0e 0f f9        realm colour 1
004d2910  03 f2 f3 fb 20 f2 f3 fb        realm colour 2
004d2918  38 35 32 2f 20 35 32 2f        realm colour 3
004d2920  05 f4 f5 fd 20 f4 f5 fd        realm colour 4
004d2928  04 fc f1 f0 20 fc f1 f0        realm colour 5
```

The realm ramp is **six rows of eight of which four are used** — the second half of each
row is the first with `0x20` substituted for the darkest entry, which is exactly the
substitution the function then makes by hand for the selected county, and is the
corroboration that the stride is 8. The rating ramp is **six flat bytes indexed
`ramp[band]`**, guarded by `band < 6`.

**The rating ramp runs bad to good, and the artwork proves it.** `Misc_cty.pl8` frame 91 —
the strip that replaces the four buttons while an overlay is up — carries a six-swatch
colour bar with a tick against the top swatch and a cross against the bottom, and reading
its pixels down column 5 gives `05 f1 09 f3 15 0f`: **this table reversed**. So band 0 is
the red at the crossed end and band 5 the purple at the ticked end.
`l2-view/tests/install.rs` asserts both halves against the user's own files.

**The bands are computed on every draw**, by `FUN_00451BBA` — the first thing
`Minimap_DrawOverlay` calls — into county bytes `+0x03`, `+0x02` and `+0x01`. Those are
three of the five bytes `Sync_CompareState` skips, i.e. interface state, not simulation
state. *(This document previously called them `+0x0B3`, `+0x0B2` and `+0x0B1`. That was the
literal `0x0053F9B3` in the disassembly mistaken for an offset; `g_counties` is at
`0x0053F9B0`.)*

| mode | byte | rule | values it can take |
|---|---|---|---|
| 1 labour | `+0x03` | 0 if `labour[0] < wanted[0]` or `labour[1] < wanted[1]`; else 6 if `labour[8] + (jobs 0…7 with `workers > useful`) == 0`; else 5 | **0, 5, 6** |
| 2 food | `+0x02` | 0 if `rationAchieved < rationWanted`, else 6 | **0, 6** |
| 3 happiness | `+0x01` | `happiness / 20` | 0 … 5 |

**Two of the three have no middle, and 6 is off the end of the ramp — so those counties
are not coloured at all.** In the shipped game the food overlay is therefore a single red
mark on the counties that went short and nothing anywhere else, and the labour overlay
paints only the ramp's two ends. That is not a gap in the reading: `FUN_00451BBA` has a
*second* food branch, spreading `rationAchieved` over bands 1…5, behind `DAT_00553E60` —
a flag zeroed by the bulk global reset at `0x00497500` and toggled only inside the command
dispatcher at `0x004B29BE`, i.e. a debug switch. With it clear the ramp's middle four
colours are unreachable in food mode.

Corroborated from the user's own saved games: over every save on this machine, `+0x02` is
only ever 0 or 6, `+0x03` only ever 0, 5 or 6, and `+0x01` is `happiness / 20` in every
save where the bands have been computed at all — `l2-formats/tests/save.rs`. (A game whose
overlay was never opened has all three zero, which the England turn-one fixture is.)

**The four buttons are not radio buttons.** `Minimap_ModeButton` (`0x0043AB76`) reads a
hotspot id from `DAT_0059154C` and is a two-state machine:

* **in mode 0** — buttons 1…3 select their mode; button 4 toggles the map zoom;
* **in any other mode** — button 4 turns the overlay *off*; buttons 1…3 do nothing.

So there is no switching straight from food to happiness. The artwork agrees again: frame
`0x5C`, drawn in mode 0, has four buttons on it, and frame `0x5B`, drawn in every other
mode, has the colour bar and **one** button.

### 3.4 Clicking the minimap  **[V]**

`FUN_0043253A`:

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
| `FUN_004071A0` | settlements and their state, **and the two waving flags** (bank `& 0x80`) | `base`/`town`/`castle`, `flags1a.pl8` |
| `FUN_004081A6` | the **path-preview balls** (bank `& 0x40`) | `flags1a.pl8`, frame `0x38 + cost`, or `0x4E` |
| `FUN_00408438` | **every unit** — armies, mobs, merchants and transports | `sprite1a` for all but a transport, offset by an 8-rotation × 16 table per zoom at `0x004D8108` … `0x004D8388` |
| `FUN_00408C50` | a debug number over each tile | gated on `DAT_005BB4A4/A5` |

**Rows two and three were both wrong until a player asked where his flags were, and both
are `docs/decisions.md` C49 and C50.** `FUN_004081A6` is not the county flag: it is the gold
ball on an ordered path, its `0x40` is the **bank** byte's transient path mark rather than
plane 0's county town, and `docs/armies.md` §2.3 has had it right under the name
`Map_DrawPathMarker` the whole time. §1.3's third bullet quotes this function for its clip
rectangle — the numbers are right and the attribution is not.

### 5.1 The two flags  **[V]**

`FUN_004071A0` runs between the terrain and the unit sprites (`Map_DrawFrame` calls
`Map_RenderIso`, then `FUN_00405602`, then `FUN_00405487`), gated on the runtime tile
record's **bank bit `0x80`**, which `County_FindTownTile` and `County_FindCastleTile` set on
their anchor quadrants. It then branches on plane 0:

```c
if      (flags & 0x40)  /* the town   */ { part 0: shield = county +0x07;
                                           part 2: mercenaryOffer ? frame 0x81 : return; }
else if (flags & 0x80)  /* the castle */ { if (content == 0x14) return;      /* unbuilt */
                                           if (!county.garrisonUnit) return; /* +0x1BC  */
                                           shield = units[garrisonUnit].shield; }
frame = shield * 8 - 8 + phase;      /* == (shield - 1) * 8 + phase */
```

`Flags1a.pl8`'s frames `0x00 … 0x27` are forty 32 × 24 pictures laid out five rows by eight
columns: **five shields × eight wave phases**, and `shield = 5, phase = 7` lands on frame 39
exactly, with frame 40 beginning an unrelated block. **The colour is in the frame index**;
there is no palette remap. The castle's shield is the **garrison's**, not the county's, so a
captured castle holding somebody else's garrison flies their colours.

**Correction: only the *town* arm guards a zero shield, and this section said both did.**
The town arm returns on `county.field_0x7 == '\0'` before it computes anything. The castle
arm tests `garrisonUnit == 0` and then computes `shield * 8 - 8 + phase` with **no clamp at
all** — there is no `1 … 5` clamp anywhere in `FUN_004071A0`; the clamp this paragraph was
remembering is `FUN_004171EE`'s, on the menu bar's banners. A garrison whose `shield` is 0
therefore asks for frame `−8 + phase`, the frame-record read fails the `dataOffset < 1`
check, and the function writes `"ERR:top_it no data"` and sets `g_quitRequest = 1`. **[D]** —
no shipped save on this machine has a zero-shield garrison, so it has not been observed.
`docs/draws-map.md` §4, **CNEW-garrison-shield**.

`content == 0x14` is the bare castle plot and `0x15 … 0x19` are castle types 1 … 5
(`FUN_0046826C` stamps `0x14 + castleType`), which is the same line `Map_Click`'s ladder
draws at "13 … 20 is nothing, 21 and up is the castle" (§6).

Placement is `tileOrigin + (0x1A, −0x1C)` at the near zoom and `(6, −0x15)` at the far one,
blitted **with no centring at all** — the frame record's `cx`/`cy` are atlas coordinates and
the function never reads them. The mercenary marker sits at `(0x10, −0x12)`.

**The wave is one global counter.** `FUN_004CFB08` advances `DAT_0057D378` once per 16 ms of
`GetTickCount` and then draws a frame; `Map_DrawFrame` wraps it at `0x80` and sets
`DAT_0057D390 = tick >> 4`. Eight frames, 256 ms each, a 2.05-second loop, every flag on the
map in step.

### 5.2 The unit sprites  **[V]**

`Map_DrawArmies` walks the tile's whole occupancy list and draws **every** unit on it, not
only armies. The sheet is chosen by `kind == 4` alone — a transport uses `g_spriteSheetB` and
everything else, **merchants included**, uses `g_spriteSheetA`.

The frame is read straight out of the record's `+0x07`, which the type's tick handler wrote:

```c
Army_Tick / Mob_Tick:            frame = bank + 3 * ((facing + 1) & 7) + g_unitWalkFrames[phase];
Merchant_Tick / Transport_Tick:  frame =        6 * ((facing + 1) & 7) + phase;
```

with `g_unitWalkFrames` (`0x004D6A78`) = `0, 1, 2, 1`, `g_merchantWalkFrames` (`0x004D6AB8`)
= `0 … 5`, and `bank` one of `0x48`/`0x60`/`0x78` by army size or `0x90` for a peasant mob.
**The rotation is `facing + 1`, not `facing`**, in all four handlers.

`Sprite1a.pl8`'s 168 frames decompose exactly: `0 … 47` are the 40 × 32 merchant, 8 facings ×
6 phases; `48 … 71` are a 3 × 4 dead block; `72 … 95`, `96 … 119` and `120 … 143` are the
three 53 × 44 army banks; `144 … 167` is the mob's. `Sprite1b.pl8` is 48 frames and nothing
else — the transport bank, which is why sheet B needs no base.

The anchor is `tileOrigin + (g_mapTileHalfStep, g_mapHalfPitch)`, and **both of those are 30
at the near zoom and 6 at the far one** — `Map_SetZoom` writes them from one literal — so on
a 58 × 30 (or 10 × 6) tile the anchor is the diamond's bottom vertex, one pixel right of
centre. Then a per-kind nudge (`(0, −4)` for an army or a mob, `(−4, −2)` for a merchant or a
transport) and `x -= w/2; y -= h`, so the figure hangs upwards and reads as standing *on* the
tile.

**Every army and every mob carries a banner, and this section did not say so.**
`Map_DrawArmies` does not stop at the figure. Still inside the same loop, after
`x -= w/2; y -= h`:

```c
if (unit.kind == 1 && unit.shield != 0)  frame = (shield-1)*8 + phase, at (+0x12, -0x15)
else if (unit.kind == 2)                 frame = 0x79 + phase,          at (+0x12, -0x12)
```

both out of `g_flagsSheet`, both marked with `Gfx_MarkTile32` and blitted with the same
clip. At the far zoom the offsets are `(+0x18, −0x15)` and `(+4, −4)`. A **merchant** (kind
3) and a **transport** (kind 4) get none — so an army's owner is legible on the map without
clicking it, which is a thing our engine's coloured square is standing in for. And
`Flags1a.pl8` frames **`0x79 … 0x80`** are the peasant mob's eight-phase banner, the block
`maps-layers.md` §5.5a measured as sitting after the 2 × 2 stubs and left unnamed.
`docs/draws-map.md` §5.3, **CNEW-unit-banner**. **[D]**

The 8 × 16 tables at `0x004D8108` … `0x004D8388` are six `i8` arrays indexed
`[direction][+0x149]`: index 0 is zero and index 1 is the full previous-tile delta, ramping
back to zero at 15. They **drag the sprite backwards toward the tile it stepped out of**
while a step plays out, which is the walk animation; the unit's own `x`/`y` are already at
the destination.

County **borders are in the tile data**, not an overlay: `maps-layers.md` §2.1 — plane-0
bit `0x02` switches the tile to the `roads` bank's boundary frames. There is no separate
outline pass, and the yellow "selected county" outline our engine draws is ours.

The **walk table's** direction is `unitFacing − mapRotation` mod 8 (`FUN_00408438`), which
is the one place the rotation feature reaches past the lattice. The *frame's* rotation is a
different quantity and does not involve the map at all — it is `(facing + 1) & 7`, baked
into `+0x07` by the tick handler (§5.2).

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

**Three things about that pick that were not written down, and one of them is a defect.**

* **`Map_PickTile` never looks at a pixel.** It is pure geometry, so a tile whose artwork
  *overhangs* — and the mine, the forest and the town all do — has most of its building
  standing on its neighbours as far as a click is concerned. `Town1a.pl8` frame 30 is
  58 × 47 on a 58 × 30 tile: **1,314 pixels painted, 857 of them on the tile.** Our engine
  deliberately departs here, testing the frame's opacity mask when the diamond misses;
  `docs/decisions.md` C57 has the argument, and it is the only departure on this path.
* **`Map_ResolvePick` snaps to the north-west anchor of a multi-tile object.** With
  `flags & 0x80` and `content >= 0x15`, or with `flags & 0x40`, the object is 2 wide and
  `t -= (part & 0xf) % 2 * 8 + (part & 0xf) / 2 * 0x200` walks back to its first tile — so
  all four quadrants of a town or a standing castle are one hotspot. It then **blanks the
  flags** for two cases that would otherwise be hotspots: `0x80` with `content == 0x14`
  (the empty castle plot) and `0x10` with `content == 0` (an unbuilt dwelling plot).
* **`Map_Click` does nothing at all at the far zoom.** The whole dispatcher is inside
  `if (g_mapZoom != 2)`.

**And the unit is picked from the tile, not from the figure.** `g_pickedTileUnit =
g_tiles[t].unit` — one byte on the tile record — so a click anywhere on a unit's diamond is
that unit and a click on the part of its sprite that overhangs its neighbours is not.
`docs/decisions.md` C58: ours asked the unit's drawn marker instead, which is nine pixels
across, and a 40 × 32 merchant was therefore mostly unclickable.

**There is no county-selection arm.** `Map_Click` writes `g_selectedCounty` only inside the
merchant, town and industry-site branches, always beside a `Map_CentreOnTile`, and a click on
ordinary ground falls off the end of the function having done nothing. Selecting a county by
clicking the map, and our second click opening its panel, are **both ours** — and because
they are, every hit-test shortfall on this screen turns into a visibly wrong screen rather
than into nothing happening. That is the amplifier under both C57 and C58.

**What is then done with the tile is `Map_Click` (`0x0043CE1A`)**, 1,263 bytes, and it is
where most of this interface is actually reached from. `Map_ResolvePick` (`0x0046D5FE`)
hands it the picked county, that county's owner, `g_pickedTileFlags` — the attribute plane
at `0x00522F91` — and `g_pickedTileGraphic`, which is just `g_tiles[tile]`. Then:

| what was clicked | what happens |
|---|---|
| your army (unit type 1) | its orders, or siege preparation (screen `0x1D`) |
| a merchant (unit type 3) **standing in a county you own** | the merchant (screen `0x08`), after centring on that county's town |
| flags bit **0x80** — an industry building | that industry is **toggled on or off** (`Industry_ToggleFromMap`), the industry chosen by a ladder on the tile *graphic*: 0 … 3 iron, 4 … 6 stone, 7 … 9 weapons, 10 … 12 wood, 21+ castle |
| flags bit **0x40** — the county town | the **village** (screen `0x02`) |
| flags bit **0x20** — farmland | the **field brush** (screen `0x04`) |
| a county that is not yours | `Msg_Enqueue(…, 0x70, …)` |

`g_screenId = 2` appears **exactly once in the binary** and it is in that table's fourth
row — and the branch centres the map on the town and repaints one map frame *before*
opening the village, because the village is drawn over the map rather than instead of it.
`docs/screens-county.md` §6.4.4 and `docs/decisions.md` C22.

**Those three bits are tested in that order, and the order settles what two of them
are.** `0x40` opens the village, so `0x40` is the **county town** — `docs/decisions.md`
C25 argued that from `L2.eng`'s wording and this is the same answer from behaviour.
`0x80` covers the industry sites *and* the castle, which is why its ladder ends at
"21 and up: castle building". All three arms are gated on the county being the local
player's; a click on somebody else's county falls to the last row.

The England turn-one fixture agrees with all of it and adds one thing.
Every county has **exactly four** `0x40` tiles, in one 2 × 2 block, all at terrain 0; its
`0x20` tiles are **exactly** the twenty tiles `g_countyFieldTiles` names, 168 of 168 with
nothing in one set and not the other; and its `0x80` tiles are always one iron site, one
stone, one weapons, one wood and a 2 × 2 block at terrain `0x14` or `0x17` — where the five
counties holding a `0x17` block are exactly the five that start owned. So **`0x17` is a
standing castle and `0x14` is the plot it gets built on**, which is the loose end C25 left
open. **[V]**

**Screen `0x04` is the field brush**, and it is five 48 × 48 buttons in two hotspot tables:
`0x004DC4D0` holds three (fallow `1`, grain `2`, pasture `0x13`) for a tile that is already
a field, `0x004DC530` two (begin reclaiming `0x19`, abandon `0`) for waste; all five call
`FUN_00438B02`, which hands the button's id to `Field_SetType` as a raw terrain value. See
`docs/kingdom.md` §7.2.

---

## 7. What our engine does, and what it does not

Implemented, in `crates/l2-view/src/campaign.rs`, `crates/l2-view/src/chrome.rs` and
`crates/l2-game/src/screens/map.rs`:

* both real zooms and their tile art, the real pitches, the scrolling window, `Map_ClampScroll`'s
  bounds, `Map_ScrollStep`'s eight directions and one-tile step, `Map_ToggleZoom`'s
  save-and-restore of the near origin, and `Map_CentreOnTile`'s `−4 / −12`;
* `Map_InitMode`'s opening state — near zoom, row `0x4A`, column `0x14`;
* the 24-pixel menu bar tiled from `Panels.pl8` 196…203 with the realm banners at
  `270 + 16i`, the framed-box kit, the `Misc_cty.pl8` right column, the far zoom's
  `Ui_DrawBox(0, 412, 30, 4)` strip, the End Turn rectangle;

  **— but not the menu bar's bevel, and not the words inside the far-zoom strip.**
  `Screen_DrawMenuBar` ends its background with `Ui_DrawBevelRect(0, 0, 0x280, 0x18)`, which
  `Chrome::draw_menu_bar_background` does not draw. And `Screen_DrawCampaign` puts four
  things in the far-zoom box, all at literal coordinates: `Eng_DrawString(101,
  g_scenarioIndex, 0x40, 0x1A8)` — the map's name; `Eng_DrawString(34, 0, …, 0x1A8)` —
  *"Year"*; `Ui_DrawYear(g_year, …, 0x1A8, 1)`; and `Eng_DrawString(34, 1, 0x50, 0x1C6)` —
  ***"Click on the county you wish to view."*** So the far view reads *England · Year 1268*
  over that instruction — the game saying in its own words what the far zoom is for, which
  agrees with `Map_Click` doing nothing at zoom 2. Ours draws its own status line there.
  `docs/draws-map.md` §5.4, **CNEW-far-zoom-caption**;
* the `MAPnn.PL8` minimap, its click rectangle, and its owner tint through the realm ramp
  read out of `Lords2.exe` at `0x004D2900`;
* **all four minimap modes** (§3.3) — the labour, food and happiness overlays through the
  rating ramp at `0x004D28F8`, `FUN_00451BBA`'s three bands including the two that answer
  "colour nothing", the mode strip and mode badge, and `Minimap_ModeButton`'s two-state
  button behaviour;
* the clip-rectangle derivation of §1.4, asserted rather than assumed —
  `campaign::tests::the_clip_rectangle_swallows_exactly_the_columns_the_half_blitters_drop`
  goes red if the clip is moved to 480;
* **the unit sprites and the two flags**, from `Sprite1a/1b.pl8` and `Flags1a.pl8`, with the
  original's frame arithmetic, its anchor and its 16 ms wave counter (§5.1, §5.2);
* **`Map_Click`'s merchant arm**, guarded on the *county's* owner as the original guards it,
  centring on that county's town and opening screen `0x08` — **which trades now**, carrying
  the clicked unit with it because `DAT_00553C64` is what the price is computed from. See
  `crates/l2-game/src/screens/merchant.rs`; the stall's hit test is `mercgrid.pl8` read as
  an 80 × 60 map of good ids, and the mouseover is the price plaque of
  `Merchant_HoverPlaque` rather than any generic tooltip — **ours has no generic tooltip
  mechanism**, and `0x00553ECC`, the only candidate on file, turned out to be a
  click guard on move-order mode (`docs/decisions.md` C54's neighbours in `symbols.md`).

  **That sentence read as a claim about `Lords2.exe`, and about `Lords2.exe` it is false.**
  `FUN_00476E95` is a generic tooltip layer: it runs every frame from `Battle_Frame`, is
  gated on the `g_optToolTips` option, waits one second of `timeGetTime` with the pointer
  still, resolves a hotspot id through the per-screen table `DAT_004D6FB8[g_screenId]`, and
  draws `L2.eng` group 220 index *id* in a box beside the cursor. `docs/formats/eng.md` §5
  has had it right all along — *"the 35 tool tips, index = hotspot id"*, `[V]`. **Twenty-four
  of the thirty-five are the campaign sidebar**, resolved by `FUN_00477320` (1,082 bytes),
  and they name the five sidebar buttons and every produce row in order — an independent
  confirmation of `map.rs`'s `SIDEBAR_BUTTONS` and of `FUN_0040FEC1`'s two lists.
  `docs/draws-map.md` §5.1, **CNEW-tooltips**;
* the map opening on the player's own town, which is `Game_SetupRealmsAndCounties`'s tail
  call `FUN_00432746(g_playerStartTable[g_localPlayer * 2])` and not `Map_InitMode`
  (`docs/decisions.md` C48);
* **two of `Map_Click`'s three plane-0 arms**: a click on one of your own settlement tiles
  toggles that industry, and a click on one of your own fields opens the brush. Both are
  gated exactly as the original gates them, and both reach the rules
  (`Kingdom::toggle_industry`, `Kingdom::paint_field`) that the original reaches.

Five oracle tests in `crates/l2-view/tests/install.rs` read the shipped files and the user's
own binary back: all 830 tile frames against the pitch, the 48-byte realm ramp byte for
byte, the 44-slot minimap census, the right column's heights, and `Panels.pl8`'s kit
boundaries. `cargo test -p l2-game --test screens shoot -- --ignored` renders the screen
into the gitignored `out/` so it can be looked at.

**Left as ours, and labelled as such in the code:**

* the map **rotation** — only rotation 0 is built, and `Map_BuildLattice`'s other three are
  not;
* the load-time **randomisation of background tile variants** (`maps-layers.md` §4.1) — we
  draw the stored index, so the off-map surround repeats where the original varies it over
  16 grass and 8 water frames;
* the **auto-selection of the county filling the viewport** (§2.3) — our selection changes
  only on a click;
* **edge scrolling**: the original scrolls when the *desktop* cursor is on the outermost
  pixel of the screen, throttled by the Scroll Speed option. We bind the arrow keys instead,
  and `Z` for the zoom the original gives a minimap button;
* one measured pixel difference: **4,600 of 436,176 diamond body pixels** across the ten
  tile banks hold palette index 0, which the original writes as black and we skip, because
  `DecodedFrame::opaque` cannot tell those from the transparent corners;
* the county **outline** and the county **marker squares** — invented, no original
  equivalent. The flags themselves *are* placed now (§5.1): the town's owner-coloured
  banner, the mercenary-offer marker beside it, and the castle's garrison banner;
* the **field markers** and the words on the brush's buttons. The original does not mark
  fields: it repaints the tile artwork itself, `FUN_0046D7F4` choosing a graphics bank and
  frame from the same terrain value it writes. We do not, because that ladder's bank byte
  (`|1`, `&0xE3`, `|8` or `|0`, `&0x7F`, `|0x80` for pasture) is only half understood and a
  painted tile would claim to be what the game looked like. A marker only claims we know
  what the field is. C21;
* **tile picking**: the original inverts the projection and answers for any tile
  (`Map_PickTile`). We hit-test the diamonds of the tiles that can mean something — the
  selected county's fields and settlements — which is right where it answers and silent
  elsewhere;
* **settlement state** on the map — `FUN_004071A0`'s industry-shut-down marker
  (`Flags1a.pl8` frames `0x28 … 0x37`, cycled on its own 16-step counter) and the field
  overlay block at `0x55 … 0x66`;
* the **walk interpolation** — `Map_DrawArmies` looks the sprite's sub-tile offset up in the
  8 × 16 tables at `0x004D8108` … `0x004D8388` by unit `+0x149`, and we keep no step counter,
  so every unit is drawn at rest (§5.2);
* the **besieger's banner** (`Flags1a.pl8` frame `0x82` with the seasons left under it,
  `FUN_00407F82`) and the **selection flood fill** the original paints for an army under
  orders. Ours are a dot and a ring;
* the File / Options / Help menus, their drop-downs, and everything the right column puts
  *inside* frames 55 / 66 / 56 / 58 — our own numbers go on a dark backing over frame 56,
  which is the one plain part of the column, so they read as an overlay;
* our own 5 × 7 font, wherever text is drawn — the original uses `Fntl2_9`, `Fntl2_14` and
  `Fntl2_22`, which are decoded but not wired up.

## 8. What this corrected elsewhere

* **`maps-layers.md` §6's "three zooms" is wrong** — there are two, and the third case of
  `Map_SetZoom` is unreachable (§2.2). That section now points here.
* **Its open 58-versus-60 discrepancy is closed** (§1.2): 58 is the artwork and 60 is the
  pitch, and `pitch = frameWidth + 2` at every zoom.
* **`maps.md`'s "the low 2 bits of the scenario select the season"** is wrong (§2.1, §3.1).
  The season is `g_season`; those bits pick one of four map slots inside a `MAPnn.PL8`.
* **`symbols.json`'s `Map_LoadTileSets` was misnamed** — it loads minimaps, not tile sets.
  Renamed `Minimap_Load`.
* **This document's own `+0x0B1` … `+0x0B3` were wrong** (§3.3): the minimap's three rating
  bytes are county `+0x01`, `+0x02` and `+0x03`. The old numbers are `0x0053F9B1`… read as
  offsets rather than as addresses into `g_counties` at `0x0053F9B0`.
* **`Minimap_ModeButton`'s comment in `symbols.md` said "pressing the active one again
  returns to mode 0".** It is *button 4* that returns to mode 0; pressing the active mode's
  own button does nothing.
* **`g_scenarioIndex` is the map slot unshifted**, and `crates/l2-game` was shifting it
  right by two. Unobservable on the England turn-one fixture, whose index is 0.

---

## 9. The mouse pointer

**A player asked about this and nothing in the tree had an answer.** *"There's also an
alternative mouse icon in the town square, it's like a question mark of some sort"*, and
then, unprompted, *"only when you're not selecting"*. Both halves are exactly right, and the
second half is the more interesting one: it names the axis the game switches on.

### 9.1 The twelve `Cursor*.cur` files are not read by anything  **[V]**

`Cursor1.cur` … `Cursor12.cur` sit in both installs — the GOG Windows one and the older
`F:\games\LORDS2` — twelve files of **326 bytes each**, byte-for-byte identical between the
two. They are installed because `INSTALL.HST`, the installer's own manifest, lists them; they
are read by nothing.

* `Lords2.exe` imports **`LoadCursorA`, `SetCursor` and `GetCursorPos` from `USER32.dll`, and
  nothing else cursor-shaped**. There is no `LoadCursorFromFile`, no `LoadImageA`, no
  `SetClassLongA`.
* The binary contains **no `.cur` filename and no `Cursor%d`-style format string** — the only
  matches for "cursor" anywhere in it are the three import names above.
* `mapl2.exe` names them nowhere either.

So the shipped `.cur` files are **source art left in the install directory**. What the game
actually draws lives in the executable's own resource directory.

### 9.2 What the game loads: seven cursors out of its own `.rsrc`  **[V]**

`Lords2.exe`'s resource directory holds **seven** `RT_GROUP_CURSOR` entries — ids 102, 103,
104, 105, 110, 111 and 113 — each pointing at one `RT_CURSOR` (ids 3 … 9, 308 bytes each).
Every one is **32 × 32, 1 bpp**, black and white with an AND mask, drawn in the top-left
corner of the bitmap.

`App_InitWindow` (`0x004B2258`) registers `WinLords2Class` with **`hCursor = NULL`** — which
is what makes the pointer the application's problem rather than the window class's — and then
loads eight `HCURSOR`s from those seven resources:

| resource | global | picture | hotspot |
|---:|---|---|---:|
| 105 | `g_cursorArrow` (`0x004EA834`) | the plain arrow pointer | (0, 0) |
| 105 | `g_cursorArrowAlt` (`0x004EB258`) | **the same resource, loaded a second time** | (0, 0) |
| 102 | `g_cursorCross` (`0x004E659C`) | a hollow serifed cross | (10, 10) |
| 103 | `g_cursorCrossTarget` (`0x004EA194`) | the same cross with a filled diamond and a plus at its centre | (10, 10) |
| 104 | `g_cursorRing` (`0x004EA518`) | an empty ring | (9, 9) |
| **110** | **`g_cursorQuestion`** (`0x004EABA8`) | **an arrow corner with a question mark beside it** | (0, 0) |
| 111 | `g_cursorPeasant` (`0x004EAC58`) | an arrow corner with a filled human figure — a head over a body | (0, 0) |
| 113 | `g_cursorScythe` (`0x004EABE4`) | an arrow corner with a scythe | (0, 0) |

**Resource 110 and the shipped `Cursor9.cur` are the same picture**, verified by comparing the
304-byte `BITMAPINFOHEADER`-and-bits payload byte for byte, hotspot (0, 0) on both. It is the
only one of the twelve files that survived into the executable unchanged.

### 9.3 What the twelve files are  **[V] for the pictures, [I] for the intent**

Decoded from the headers — all `type = 2`, one image, 32 × 32, 1 bpp, 304-byte payload:

| file | hotspot | picture |
|---|---:|---|
| `Cursor1.cur` | (15, 1) | a chevron pointing **up** |
| `Cursor2.cur` | (29, 1) | a corner pointing **up-right** |
| `Cursor3.cur` | (28, 15) | a chevron pointing **right** |
| `Cursor4.cur` | (29, 29) | a corner pointing **down-right** |
| `Cursor5.cur` | (16, 30) | a chevron pointing **down** |
| `Cursor6.cur` | (2, 29) | a corner pointing **down-left** |
| `Cursor7.cur` | (2, 16) | a chevron pointing **left** |
| `Cursor8.cur` | (1, 1) | a corner pointing **up-left** |
| **`Cursor9.cur`** | (0, 0) | **arrow corner + question mark** — identical to resource 110 |
| `Cursor10.cur` | (0, 0) | arrow corner + a plain rounded pillar |
| `Cursor11.cur` | (0, 0) | arrow corner + a scythe, curved blade |
| `Cursor12.cur` | (0, 0) | arrow corner + a straight-headed tool, hoe or mallet |

The set reads as two groups. **1 … 8 are the eight compass directions**, and their hotspots
prove it: each sits at the edge or corner of the 32 × 32 square its arrow points to. They are
**edge-scroll cursors for the campaign map** — [I], but the eight-way hotspot pattern is not
consistent with anything else, and `Map_ScrollStep`'s eight directions (§1.5) are the
mechanic they would belong to. **The shipped build never uses them**: the map scrolls with the
ordinary arrow pointer.

**9 … 12 are four drafts of the "arrow plus a glyph" pointer**, of which one shipped verbatim
(9 → resource 110) and two were redrawn — the pillar of 10 became the human figure of 111, the
scythes of 11 and 12 became the scythe of 113. So the `.cur` files are an **earlier revision
of the same artwork**, not a superset of it.

### 9.4 What selects which: one table, and one ladder  **[V]**

`Cursor_Set` (`0x004B1CF3`) is the **only** caller of `SetCursor` in the binary. It takes a
*kind* and switches it onto one of the eight `HCURSOR`s:

| kind | cursor |
|---:|---|
| 0 | `g_cursorArrow` |
| 2 | `g_cursorQuestion` |
| 4 | `g_cursorCross` |
| 5 | `g_cursorCrossTarget` |
| 6 | `g_cursorRing` |
| 12 | `g_cursorPeasant` |
| 13 | `g_cursorArrowAlt` — the same picture as kind 0 |
| 14 | `g_cursorScythe` |
| anything else | `g_cursorArrow` |

Its **one** caller is `Battle_Frame` (`0x004B99C0`) — which despite the name is the whole-game
per-frame function, the same one that ends by calling `Screen_FrameInput`
(`docs/screens-county.md` §2.6). The pointer is therefore re-chosen from scratch on every
frame, and the choice is made in exactly two ways:

```c
if (g_screenId >= 0x28 && g_screenId < 0x2B)   /* the battlefield */
    ... the ladder in §9.7 ...
else
    Cursor_Set(g_cursorByScreen[g_screenId]);  /* a table lookup */
```

### 9.5 The table, and the question mark  **[V]**

`g_cursorByScreen` (`0x004E3098`) is **64 dwords, one per `g_screenId`**. Read out of the
image, **five** rows are non-zero and every other screen in the game gets 0, the plain arrow:

| `g_screenId` | kind | cursor | the screen |
|---:|---:|---|---|
| **`0x02`** | **2** | **question mark** | **the village, idle** |
| `0x06` | 12 | the human figure | the village, **carrying a selection** |
| `0x07` | 13 | arrow (alt) | — nothing sets `g_screenId` to 7 |
| `0x0E` | 2 | question mark | — nothing sets `g_screenId` to 0x0E |
| `0x10` | 14 | the scythe | the campaign map in **army-movement mode** |

**This settles the player's report, both halves.** The village occupies three screen ids —
`0x02` idle, `0x05` while the rubber band is being drawn, `0x06` while the selection is
carried (`docs/screens-county.md` §6.4.1). The table gives the question mark to `0x02` **and
to `0x02` only**: `0x05` falls through to 0 and gets the plain arrow, `0x06` gets the human
figure. *"Only when you're not selecting"* is the table, row for row.

`0x10` is the screen `Map_BeginMoveSelection` (`0x0043723A`) opens, and the tip-screen
driver's own label for it is *"army movement"* — so the scythe is the pointer you get while
choosing where an army marches. The human figure on `0x06` is the peasants in your hand.

**Screens `0x07` and `0x0E` are dead rows.** A scan of every `mov byte ptr [g_screenId], imm`
in `.text` finds **50 distinct values** written and neither 7 nor 0x0E among them; 0x10 is
written exactly once, inside `Map_BeginMoveSelection`. **[I]** rather than **[V]**, because
the scan covers only the immediate-store form — but that form accounts for all 50 ids,
including every one `docs/screens-county.md` §1 lists.

### 9.6 The question mark is **not** the help system  **[V]**

Three separate mechanisms could have owned a question-mark pointer, and none of them does:

* the **help screen** is `g_screenId 0x31` (`Screen_HelpOptions`, `L2.eng` group 45). Its row
  in `g_cursorByScreen` is **0** — the plain arrow;
* the **tip screens** (`L2.eng` groups 200–219, one per screen, once per game, gated on
  `g_optTipScreens`) run through `Tip_Update`, a 20-frame timer that switches `g_screenId` to
  `0x27`. `0x27`'s row is **0** as well, and the tip driver never touches a cursor;
* **`Ui_OpenConfirm`, the message scroll and the drop-down menus** likewise leave the row at 0.

The question mark is a **static property of the village screen**, evaluated fresh every frame
from a constant table, with no state of its own. **[I]** as to what it *means*: on `0x02` a
click on the picture opens the job popup for whatever cluster is under it — `vill_gd8.pl8` is
the 45 × 40 lookup, `docs/screens-county.md` §6.4 — so *"point at something here to ask about
it"* is the reading the behaviour supports. Nothing in `L2.eng` labels the cursor itself.

### 9.7 The battle ladder  **[D]**

Inside `0x28 … 0x2A` the pointer is chosen from what is under it, using five globals
`Battle_UpdateHover` (`0x0047ED9B`) recomputes each frame:

```c
if (g_screenId == 0x2A)                  Cursor_Set(0);   /* arrow */
else if (g_battleHoverEnemy)             Cursor_Set(5);   /* cross + target */
else if (DAT_00553078) {                 /* a selection exists */
    if (DAT_0053E8BC) {                  /* an order is being placed */
        if (DAT_00568968 == 6 && g_battleHoverSurface < 6) Cursor_Set(5);
        else if (DAT_00568968 == 4 && g_battleHoverSurface < 4) Cursor_Set(5);
        else                                               Cursor_Set(4);
    }
    else if (g_battleHoverFriendly)       Cursor_Set(6);  /* ring */
    else if (g_battleHoverFriendlyPicked) Cursor_Set(6);
    else if (!g_battleHoverOnField)       Cursor_Set(g_mouseY >= 0xB8 ? 0 : 4);
    else                                  Cursor_Set(4);  /* plain cross */
}
else {                                    /* nothing selected */
    if (!g_battleHoverOnField)            Cursor_Set(0);
    else if (g_battleHoverFriendly)       Cursor_Set(6);
    else                                  Cursor_Set(0);
}
```

So: **ring** = one of yours is under the pointer, **plain cross** = the battlefield with a
selection in hand, **cross-with-target** = an enemy under the pointer, or a legal destination
while an order is being placed.

**The game says this itself.** `L2.eng`'s battle help reads *"To attack, select units and then
move the cursor onto an enemy unit. **When the cursor turns red**, click on the unit and your
soldiers will attack."* That is the `g_battleHoverEnemy` branch, in the game's own words —
which is what lifts the mapping above a story assembled from a decompiler listing. The word
*red* does not describe these resources, which are 1-bit black and white with no colour at
all; either it is a DOS-build recollection or *red* is loose for *changes*. **[I]**, and
flagged rather than resolved.

**`Battle_UpdateHover` runs after the cursor is chosen** — `Battle_Frame` calls it at
`0x004BA1DB`, well past the ladder at `0x004B9F49` — so the battle pointer is **one frame
behind the pointer position**. That is the same construction `docs/screens-county.md` §2.6
records for `Screen_FrameInput`, and for the same reason: the frame function's tail is the
next frame's head.

### 9.8 What our engine does  **[V]**

**Nothing.** `crates/l2-view` draws the OS cursor everywhere, and no code in the workspace
reads a `.cur` file, a cursor resource or `g_cursorByScreen`. That is a gap, and
`docs/mechanics.md` now carries it as one.

The cost of closing it is small, and worth writing down because it is smaller than it looks —
but it is *not* "parse the twelve files". Two of the three glyph cursors the game actually
draws exist only inside `Lords2.exe`. A faithful implementation reads the user's own binary,
the way `crates/l2-view` already reads the realm ramp at `0x004D2900` (§7): walk `.rsrc` to
`RT_GROUP_CURSOR` 102, 103, 104, 105, 110, 111 and 113, decode seven 32 × 32 1-bpp AND/XOR
pairs, and hang them off a copy of `g_cursorByScreen` plus the §9.7 ladder. There is no new
file format to learn: `.cur`, `.ico` and `RT_CURSOR` are the same three structures, and an
`RT_CURSOR` is a `.cur` with the 22-byte directory replaced by a 4-byte hotspot.
