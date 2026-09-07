# PL8 — the 23 unexplained files

Status: **all 23 explained.** Under the model below every PL8 file in both shipped
installs satisfies the end-offset invariant:

| Corpus | Files | Frames | Failing |
|--------|-------|--------|---------|
| `F:\games\Lords of the Realm II` (GOG, Windows) | **291 / 291** | **21,344 / 21,344** | 0 |
| `F:\games\LORDS2\PL8` (older DOS install) | **222 / 222** | **14,648 / 14,648** | 0 |

Previously: 268 / 291 files, 19,606 / 21,344 frames. Nothing that passed before
changes classification — the model is strictly additive.

Reproduce:

```bash
node tools/pl8fail/final.js "F:/games/Lords of the Realm II"
node tools/pl8fail/final.js "F:/games/LORDS2/PL8"
```

Scripts live in `tools/pl8fail/`. They are throwaway analysis tools, not a decoder.
`crates/l2-formats/` is untouched by this work.

---

## 1. Summary

There is no new pixel encoding and no trailing metadata block. Two things were wrong:

1. **The decoder dispatched raw-vs-isometric on header byte 0x00. It must dispatch on
   the frame record's shape byte (0x0C) only.** Fifteen files are isometric tile sets
   whose header byte 0x00 says `0`. `pl8.md` already states "the family byte does not
   decide the encoding — the shape byte does", but the implementation only applied that
   inside family 2. Applying it everywhere accounts for **15 of the 23 files** and
   1,667 additional isometric frames.

2. **Frame-record byte 0x0D is live on shape-0 frames too.** It is the number of rows
   the artwork extends *above* the `width × height` rectangle. In four files those rows
   are actually stored, RLE-encoded, immediately after the rectangle. That accounts for
   **4 more files** (70 frames).

The remaining file, `Font_c2.pl8`, is a one-off: header byte 0x00 says `1` (RLE) but
every frame is stored raw.

The "overshoot by exactly 24 / 840" pattern in the old notes was the modal residual,
not the whole story: 24 = `10*6 − 6²` and 840 = `58*30 − 30²`, i.e. the difference
between a raw rectangle and a diamond at the two tile sizes. Frames with a non-zero
overhang count had other residuals in the same files.

---

## 2. Verified decode model

```
decodeFrame(buf, header, rec):
    w, h  = rec.width, rec.height
    shape = rec[0x0C]
    rows  = rec[0x0D]

    if shape != 0:                                  # isometric — ANY header byte 0
        n = h*h
        if shape == 2:            n += rows * w
        if shape == 3 or 4:       n += rows * h
        # shape 1 ignores `rows`, as documented in pl8-mode2.md
        return n

    if header[0x00] == 1 and header[0x01] == 0:      # RLE sprite stream
        return h RLE rows of width w

    # raw rectangle (or, for 4 files, a 1/8-resolution region-id table)
    n = w * h
    if header[0x00] == 0 and rows > 0:
        n += rows RLE rows of width w                # overhang, painted above the rect
    return n
```

Rule usage across the GOG corpus (21,344 frames):

| Rule | Frames |
|------|--------|
| `rle` (family 1, zoom 0) | 9,603 |
| `raw` | 7,636 |
| `iso1` / `iso2` / `iso3` / `iso4` | 2,425 / 812 / 432 / 362 |
| `raw` + overhang rows | 70 |
| region-id grid (structural) | 4 |

Isometric canvas-bounds violations (every diamond pixel and every chevron pair inside
`width × (height + rows)`): **0**. Every isometric frame in the newly reclassified
files also satisfies `width == 2*height − 2` with even height — the same precondition
the mode-2 work established, now over 4,031 frames instead of 2,364.

---

## 3. The 15 isometric files with header byte 0x00 = 0

`Base2a`, `Roads2a`, `Castle1a-d`, `Castle2a-d`, `Town1a-d`, `Town2a-d`.

Every one holds only shape 1/2/3/4 frames with 58×30 (`zoom` byte 0) or 10×6
(`zoom` byte 2) geometry — the same tile sets as their `Base1a-d` / `Roads1b-d` /
`Base2b-d` siblings, which carry header byte `2`.

**Verified, and decisive:** `Base2a.pl8` and `Base2b.pl8` are 7,288 bytes each and
differ in **exactly one byte of the 2,248-byte header + frame table — byte 0** (`0`
vs `2`). Their 140 frame records are identical byte for byte; only 805 of the 5,040
pixel bytes differ, which is the seasonal recolour. The same header byte therefore
carries two different values for two files that are structurally the same tile set.

Decoding them by shape produces clean diamonds:

```
Castle1a f3 (58x30, shape 1)          Base2a f0 (10x6, shape 1)
                            ##            ##
                          ######        ######
                        ##########    ##########
        ...  (widths 2, 6, 10, ... 58, 58, ... 10, 6, 2)  ...
```

**Verified from the binary:** `Pl8_DrawFrame` (`0x0040A21A`) indexes straight to
`buf + frame*0x10 + 8`. It never reads `buf[0]` or `buf[1]`. Nothing in the draw path
can see the header family byte, so an inconsistent value there is harmless to the game
and only ever misled us.

*Inferred:* byte 0x00 most likely records which subsystem exported the file rather than
a codec. It is still a reliable predictor of RLE (see §5), but it does not distinguish
raw from isometric.

---

## 4. Byte 0x0D on shape-0 frames: overhang rows, RLE-encoded

`Fntl2_14` (47 frames), `Font_10` (7), `T16_bat1` (8), `T32_bat` (8).

**Verified from the bytes.** In each of these 70 frames the span is
`w*h + k`, and the `k` bytes starting at `dataOffset + w*h` parse as exactly
`rec[0x0D]` RLE rows of exactly `width` pixels each, ending exactly on the next
frame's `dataOffset`. No frame is off by a byte; no other reading was tried and
discarded.

The old "undershoot by 6 / 10 / 61 / 190" figures were just the largest such block in
each file. `k` is `2 × rows` when every overhang row is empty (a blank row costs the
two bytes `00 <width>`), which is why the font residuals looked like small multiples.

The rows are real artwork, and they are contiguous with the rectangle:

```
Fntl2_14 f21  (8x10, rows=3)          T16_bat1 f40  (16x16, rows=9)
  |........|   <- overhang row 0        |...............#|
  |...#....|   <- overhang row 1        |..............##|
  |.##.....|   <- overhang row 2        |.............###|
  :##......:   <- rectangle row 0       |.............###|
  :##....#.:                            |.............###|
  :.##..###:                            |............####|
  :.##...##:                            |...........#####|
  :.##...##:                            |..........######|
  :.##...##:                            |.........#######|
  :.##...##:                            :################:  <- rectangle
  :####..##:                            :################:
  :..####..:                            :################:
  :....#...:                            :       ...      :
```

The font case is a grave accent whose stroke runs continuously into the glyph body.
The battle-tile case is the sloped upper edge of a hill tile, extending the 16×16
square upward — the square-tile analogue of the isometric chevron overhang in
`pl8-mode2.md` §5. `T32_bat` carries the same eight slots at 32×32 with roughly
double the row counts (9→18, 8→16, 6→12), as you would expect of the same terrain at
two scales.

**Verified from the binary — direction and meaning.** `FUN_00402A14`, the glyph
blitter (it indexes `DAT_004D71F0`, the 224-entry `char − 0x20 → glyph slot` table,
and is called nine times per character by the string drawer `FUN_00402637` for the
drop shadow), loads the record exactly like `Pl8_DrawFrame` and then does:

```
DAT_00591528 = DAT_00591528 + DAT_005BB478;   // destY += record[0x0D]
```

before clipping. The rectangle is drawn `rows` screen rows *lower*, which reserves
exactly `rows` rows above it. That fixes the direction: the stored overhang rows are
the top of the frame, first-stored row topmost. The frame's true canvas is
`width × (height + rows)`, the same shape as the isometric case.

Two corrections to `docs/symbols.md` fall out of this (the file is owned by someone
else, so they are recorded here rather than applied):

- `DAT_005BB478` is **not** "frame record byte 0x0D". It is the blitter's *row
  counter*. `FUN_0040477B` (`Clip_Vertical`) unconditionally overwrites it with
  `visibleRows = height − rowsClippedAtTop`. The load of byte 0x0D into it in
  `Pl8_DrawFrame` is **dead** — `Pl8_DrawFrame` calls `Clip_Vertical` before any
  blitter runs, and so do the sibling entry points at `0x0040A3D0`, `0x0040A682`,
  `0x0040A80E` and `0x0040A9B0`. `FUN_00402A14` is the only path I found that consumes
  the loaded value, and it does so as a Y offset before the clipper fires.
- `Blit_Unclipped` (`0x004B43B1`) loops `DAT_005BB478` rows of `DAT_005C9258` bytes —
  clipped row count × width, not height × width.

**Not established:** the shipped English text renderer never paints those overhang
rows. `FUN_00402A14` applies the Y shift and then calls a single rectangle blitter
(`0x004B41B7` / `0x004B425C` / `0x004B42C5`); nothing re-enters at
`dataOffset + w*h`. So the accents stored in `Fntl2_14` and `Font_10` appear to be
present but unpainted. I could not find a second font path. Plausible readings: they
are for a localised build, or the shipped exe has a bug. Either way, a decoder should
still consume the bytes — the invariant demands it.

### Where byte 0x0D is honoured

Only six files in the corpus have a shape-0 frame with `rec[0x0D] != 0`:

| File | header 0x00 | shape-0 frames with rows > 0 | overhang stored? |
|------|----|-----|-----|
| `Fntl2_14` | 0 | 47 | yes (47/47) |
| `Font_10`  | 0 | 7  | yes (7/7) |
| `T16_bat1` | 0 | 8  | yes (8/8) |
| `T32_bat`  | 0 | 8  | yes (8/8) |
| `Fntl2_9`  | 2 | 47 | **no** (0/47) |
| `Font_c2`  | 1 | 47 | **no** (0/47) |

70 stored, 94 not, no in-between. *Inferred:* header byte 0x00 == 0 is the
discriminator. The sample is thin — the two "no" files are the same font asset
exported twice (identical frame tables, see §5) — so a decoder that would rather not
lean on it can detect the block structurally: try consuming `rows` RLE rows and accept
only if it lands on the next `dataOffset`. That detection is unambiguous over both
corpora (0 misfits, 0 false positives across 21,344 + 14,648 frames).

Note `Fnt_8.pl8` (header 0, zoom 1) has `rows == 0` on all 150 frames, so it is not a
counterexample either way.

---

## 5. `Font_c2.pl8` — header says RLE, content is raw

**Verified.** All 108 frames occupy exactly `width × height`, and read as raw they are
**pixel-identical in shape** to the corresponding frames of `Fntl2_9.pl8`:

```
Font_c2 f40 10x11          Fntl2_9 f40 10x11        Font_c2 f41 8x11
|...####...|               |...####...|             |#######.|
|.##....##.|               |.##....##.|             |.##...##|
|##......##|               |##......##|             |.##...##|
|   ...    |               |   ...    |             |.######.|
```

**103 of the 108 frame records are identical** in width, height and `rows` byte; the
five that differ (frames 80, 81, 105–107) are trailing special glyphs where `Fntl2_9`
holds 2×2 stubs and `Font_c2` holds real artwork. The headers differ only at 0x00
(`1` vs `2`) and 0x04. They are the same font, exported twice. Neither stores its
overhang rows.

So the header byte is simply wrong on this file, and — per §3 — nothing in the engine
reads it, so it never mattered.

`Font_c2.pl8` is also the only family-1 file in the corpus whose header byte 0x01 is
not 0. The other 138 are all zoom 0 and all genuinely RLE (9,603 frames). The model in
§2 uses `header[0x00] == 1 && header[0x01] == 0` as the RLE test, which fits both
corpora exactly, but **that rests on a single file** and should be read as "family 1
means RLE, with one known bad file" rather than as a discovered second discriminator.

Supporting but weak: `font_c2.pl8` does not appear as a string in `Lords2.exe` or
`mapl2.exe`. Nor do `font3c2.pl8`, `t16_bat1.pl8` or `t32_bat.pl8`. The five fonts the
engine does name are `fnt_8`, `fntl2_9`, `fntl2_14`, `fntl2_22`, `font_10`; the battle
tile sets it names are `t32_bat1` / `t32_bat2` (and `t32_bat2.pl8` is not even
shipped). That is consistent with `Font_c2`, `T16_bat1` and `T32_bat` being stale
exports left in the install — but it is not proof, because plenty of live files are
opened under names built at runtime (`base2a.pl8` is a string, `base2b/c/d.pl8` are
not, yet all four ship and are used).

---

## 6. What this changes for `crates/l2-formats`

Owned by someone else; recorded, not applied.

1. Dispatch on `rec[0x0C]` for every file, not only header family 2. The existing
   isometric code needs no change — only the condition that reaches it.
2. On shape-0 frames, consume `rec[0x0D]` extra RLE rows of `width` after the
   rectangle, and make the frame canvas `width × (height + rows)` with the rectangle
   at the bottom.
3. `Font_c2.pl8` needs either the zoom-byte condition or a raw fallback.
4. `KNOWN_FAILING` can be emptied. `VALIDATED_BASELINE` becomes 291.
5. `docs/symbols.md`: `DAT_005BB478` is the blitter row counter, not record byte 0x0D
   (see §4).

## 7. Still open (unchanged by this work)

- Header fields 0x04, 0x06, 0x07. 0x06 is non-zero in exactly one file of 291
  (`T16_bat1`, value 1) — one of the unreferenced ones.
- Whether anything paints the stored font overhang rows (§4).
- Where family-1 RLE is actually decoded. The nine `Pl8_DrawFrame`-shaped entry points
  I examined between `0x0040A127` and `0x0040AE12` all call rectangle blitters; I did
  not locate an RLE blitter and did not look hard.
- `Title.pl8` palette pairing, and the type-4 apex ambiguity — both unrelated, see
  `pl8-mode2.md`.

## 8. Method note

Everything above was derived from the shipped data files and from `Lords2.exe` in
Ghidra. No prior-art source was consulted for this investigation; the two facts that
overlap with the published `pl8image` documentation (the shape byte at 0x0C and the
extra-row count at 0x0D) were already recorded in `pl8.md` before this work started.
The parts that are new here — that byte 0x0D is live on shape-0 frames, that the extra
rows are RLE-encoded, and that header byte 0x00 is unreliable — are not in that
documentation.
