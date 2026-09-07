# PL8 sprite format (Lords of the Realm II)

Status: **268 / 291 files validated** (18,937 frames), **zero files in an undecoded
storage family**. The 23 remaining failures are pinned in `KNOWN_FAILING` — see
[Unexplained files](#unexplained-files).

Implemented in `crates/l2-formats/src/pl8.rs`. For the isometric family in depth —
the Ghidra work, the overhang derivation, and the one unresolved ambiguity — see
[`pl8-mode2.md`](pl8-mode2.md).

## Why the validation counts mean something

The format verifies itself, which makes automated validation possible without a
human looking at pictures:

1. Each frame's decoded data must end **exactly** at the next frame's declared
   `dataOffset` (or EOF for the final frame).
2. Every RLE row must consume **exactly** `width` pixels.

Thousands of frames landing precisely on their declared byte boundaries cannot
happen by accident — a wrong reading drifts within a frame or two. `cargo test -p
l2-formats` runs both checks over a whole install.

## Header (8 bytes)

| Offset | Type | Meaning |
|--------|------|---------|
| 0x00 | u8 | **Storage family**: 0 = raw, 1 = RLE, 2 = isometric map tiles |
| 0x01 | u8 | **Map zoom level**: 0 → 58×30 tiles, 1 → 26×14, 2 → 10×6 |
| 0x02 | u16 | Frame count |
| 0x04 | u16 | Unknown; varies 0..287 |
| 0x06 | u8 | Unknown, usually 0 |
| 0x07 | u8 | Unknown, 0..15 |

Bytes 0 and 1 are **two independent bytes**, not one `u16`. Reading them as a
single value was an early mistake here; byte 1 varies independently of byte 0.

## Frame table

Immediately follows the header: `frameCount` records of 16 bytes.

| Offset | Type | Meaning |
|--------|------|---------|
| 0x00 | u16 | Width |
| 0x02 | u16 | Height |
| 0x04 | u32 | Absolute file offset of pixel data |
| 0x08 | i16 | Canvas placement X |
| 0x0A | i16 | Canvas placement Y |
| 0x0C | u8 | **Shape** — the per-frame encoding |
| 0x0D | u8 | Overhang row count |
| 0x0E | u16 | Padding |

Confirmed invariant: the first frame's `dataOffset` always equals
`8 + frameCount * 16`, so pixel data begins immediately after the frame table.

The engine reads bytes 0x0C, 0x0D and 0x0E at `Pl8_DrawFrame` (`0x0040A21A`),
which independently corroborates this layout.

## The family byte does not decide the encoding

**The `shape` byte at record offset 0x0C does.** An isometric file routinely holds
plain raw rectangles alongside diamonds — `Backgrnd.pl8` is family 2 but its single
640×480 frame is stored raw. That is why "storage mode 2" resisted analysis for so
long: there was never one mode-2 codec to find.

| Shape | Encoding | Bytes |
|-------|----------|-------|
| 0 | Raw rectangle | `width × height` |
| 1 | Isometric diamond only | `height²` |
| 2 | Diamond + full-width chevron overhang | `height² + rows × width` |
| 3 | Diamond + left-half overhang | `height² + rows × height` |
| 4 | Diamond + right-half overhang | `height² + rows × height` |

**Shape 1 ignores the overhang count**, even when it is non-zero — 24 frames in the
corpus declare rows and still hold exactly `height²`. Honouring the count there
desynchronises the whole file.

## Encodings

### Raw

`width × height` bytes of palette indices, row-major.

### RLE

Decoded per row; each row consumes exactly `width` pixels:

```
n = next byte
if n == 0:  m = next byte; skip m pixels (transparent)
if n >  0:  copy the next n bytes as literal palette indices
```

A skip run of `00 00` advances nothing and would loop forever; the decoder rejects
it. No shipped file contains one.

### Isometric diamond

Every isometric frame satisfies `width == 2 × height − 2` with an even height. With
`hh = height / 2`, row `r` of the bounding box holds

```
rowWidth(r) = (r < hh) ? 2 + 4*r : 2 + 4*(height - 1 - r)
xStart(r)   = (width - rowWidth(r)) / 2
```

so widths run `2, 6, 10, …, width, width, …, 6, 2`, totalling exactly `height²`.
Only the pixels inside the diamond are stored — contiguously, top row first, with
no control bytes and no row headers.

### Isometric overhang

Shapes 2, 3 and 4 append `rows` extra records after the diamond. Each is **not a
horizontal row** but a *chevron* tracing the diamond's own upper silhouette, and
each successive record paints one screen row higher. That is how the engine
extrudes mountains, cliffs and raised roads upward without storing a bounding
rectangle. Pair `m` sits at column `2m` on silhouette row `|hh − 1 − m|`.

The frame's true canvas is therefore `width × (height + rows)`, with the diamond
occupying the bottom `height` rows.

### Region maps — not artwork

Four files (`Arm_grid`, `Mercgrid`, `Vill_gd8`, `Villgrid`) declare shape 0 but hold
only `(width/8) × (height/8)` bytes: mouse hit-test maps at 1/8 resolution, one byte
per 8×8 screen block, holding region ids rather than palette indices. The engine
reads them from region code and never blits them.

The decoder detects these **structurally** — by the byte span, not the filename —
and decodes them at 1/8 scale with every pixel marked transparent, since none of
it is ever painted.

## Transparency

**Palette index 0 is transparent.** Every blitter in the original copies a byte only
when it is non-zero (`0x004B43B1`). This is a property of the engine, not something
recorded in the file, and it applies to raw frames too — our decoder marked raw
frames fully opaque until the binary corrected us.

## Palette files (`.256`)

768 bytes: 256 entries of R, G, B. Values are **6-bit VGA (0..63)** and must be
scaled to 8-bit (`v * 255 / 63`, so 63 maps to a true 255 rather than 252).

Palettes are per-context, not global. Using the wrong one gives a structurally
correct but wildly miscoloured sprite. `T32_bat1.256` is the battle-sprite palette;
`Lords2.256` is not.

## Unexplained files

23 files use a supported encoding but miss the end-offset invariant. They are pinned
in `KNOWN_FAILING` in `crates/l2-formats/tests/corpus.rs`, so a *new* failure breaks
the build and fixing one is reported as "now passing".

| Pattern | Files |
|---------|-------|
| Overshoot by exactly 24 bytes | `Base2a`, `Roads2a`, `Castle2a`, `Town2a`, `Town2b-d` |
| Overshoot by exactly 840 (24 × 35) | `Castle1a-d`, `Town1a-d` |
| Undershoot | `Fntl2_14` (6), `Font_10` (10), `T16_bat1` (61), `T32_bat` (190) |
| RLE row overrun | `Font_c2` |

`Font_c2` is the corpus's only family-1 file with zoom byte 1, and **all 108 of its
frames occupy exactly `width × height`** — it declares RLE but is stored raw. One
file is too little to generalise from, so it stays pinned rather than special-cased.

## Also open

- `Title.pl8` decodes with provably correct geometry, but no shipped `.256` colours
  it properly. `Title.256` is not its palette.
- The type-4 apex pair: the stored data and the shipped blitter disagree at columns
  28–29. We use the natural mapping (the exact mirror of type 3), which discards no
  data. See `pl8-mode2.md`.
- Header fields at 0x04, 0x06 and 0x07.

## History worth keeping

This format was reverse-engineered from scratch before anyone checked for prior art.
It was already published: the GPL-3 [`pl8image`](https://github.com/s-ayers/pl8image)
package documents the header, the frame record and the shape byte. Search first —
see `docs/decisions.md`, C5.
