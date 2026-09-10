# PL8 sprite format (Lords of the Realm II)

Status: **291 / 291 files validated, 21,344 / 21,344 frames.** Nothing is pinned or
excluded. The same model also validates the older DOS install at 222/222 files and
14,648/14,648 frames.

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

## The family byte is vestigial

**The `shape` byte at record offset 0x0C decides, for every family.**

`Pl8_DrawFrame` (`0x0040A21A`) indexes straight to `buf + frame*0x10 + 8` and never
reads bytes 0 or 1 — the engine cannot see the family byte at all. The decisive
evidence: **`Base2a.pl8` and `Base2b.pl8` differ in exactly one byte** of their
2,248-byte header and frame table — byte 0, reading `0` against `2` — with identical
frame records and only a seasonal recolour between their pixels.

So 18 files are ordinary isometric tile sets whose family byte simply reads `0`.
Dispatching on the family byte charged `w*h` for a diamond; the recurring "24 byte"
and "840 byte" overshoots were just `10*6 − 6²` and `58*30 − 30²`. There was no
trailing block. An isometric file also routinely holds plain rectangles —
`Backgrnd.pl8` is family 2 with a single 640×480 raw frame.

| Shape | Encoding | Bytes |
|-------|----------|-------|
| 0 | Raw rectangle | `width × height` |
| 1 | Isometric diamond only | `height²` |
| 2 | Diamond + full-width chevron overhang | `height² + rows × width` |
| 3 | Diamond + left-half overhang | `height² + rows × height` |
| 4 | Diamond + right-half overhang | `height² + rows × height` |

**The map renderer dispatches on this byte, and only on 2, 3 and 4.** That is a second
derivation of the table above, from the code rather than from the file, and it was done
without reference to it. `Map_DrawTileApex` (`0x00406673`) loads the shape byte into
`g_frameShape` (`0x00591568`) and picks an overhang blitter with a three-way test — `== 2`,
`== 3`, `== 4` — and no default arm; the row pass at `0x00406BBA` does the same. So a
shape-0 or shape-1 frame gets **no overhang pass at all**, which is exactly what "raw
rectangle" and "isometric diamond only" mean. The blitters are named `ApexBlit_S2_*`,
`ApexBlit_S3_*`, `ApexBlit_S4_*` and their `RowBlit_` twins, three zoom variants each.

Shapes 3 and 4 share their blitter with a fifth argument of 2 or 0 where shape 2 has a
separate routine per draw mode — the left-half and right-half cases differ by a parameter
and the chevron case does not, which is the same asymmetry the encoding column shows
(`rows × height` for both halves, `rows × width` for the chevron).

**Shape 1 ignores the overhang count**, even when it is non-zero — 32 frames in the
corpus declare rows and still hold exactly `height²`. Honouring the count there
desynchronises the whole file.

**Shape 0 does not.** A rectangle may store `rows` extra RLE-encoded rows immediately
after it, drawn *above* it — real artwork continuous with the rectangle, such as the
accent on a glyph or the sloped top edge of a hill tile. `Glyph_Draw` (`0x00402A14`)
shifts the destination down by that row count before clipping, reserving exactly those
rows. 70 frames across four files store them, and all 70 land byte-exact.

**Declaring the rows and storing them are two different questions, and only the
second one is structural.** A shape-0 frame *always* reserves the rows it declares:
its canvas is `width × (height + rows)` with the rectangle at row `rows`, whether
or not any bytes for those rows exist. Whether they exist is decided by the byte
span — if the bare rectangle lands exactly on the next frame's offset, nothing is
stored and the reserved rows stay transparent.

Both cases are common and the two body fonts are one of each:

| File | shape-0 frames declaring rows | stored? |
|---|---|---|
| `Fntl2_14.pl8` | 47 | yes — 45 wholly transparent, 2 (`v`, `w`) inked |
| `Fntl2_9.pl8` | 47 | no |
| `Font_c2.pl8` | 47 | no |
| `Font_10.pl8` | 7 | yes, all transparent |
| `T16_bat1.pl8` | 8 | yes, inked |
| `T32_bat.pl8` | 8 | yes, inked |

A stored-but-transparent row is six bytes for a nine-pixel-wide glyph, not
twenty-seven: three RLE rows reading `00 09 00 09 00 09`, one skip run each. The
flat surplus is what it looks like — the rows really are there and really are
empty, and the exporter wrote them rather than eliding them.

Deciding the canvas height on the byte span instead cost a visible bug: it made
`Fntl2_14`'s glyphs `h + rows` tall and `Fntl2_9`'s only `h`, so no caller could
apply `Glyph_Draw`'s `y += rows` correctly for both, and every `rows = 3` letter
of the body font drew three pixels low. A player caught it by opening the original
next to our demo. `crates/l2-formats/tests/corpus.rs` now pins the contract.

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

## Files that used to fail, and why

23 files once missed the end-offset invariant. All are now explained; the corpus is
complete. `KNOWN_FAILING` in `crates/l2-formats/tests/corpus.rs` is empty and kept
as a mechanism — a new failure breaks the build by name.

**18 files: dispatching on the family byte instead of the shape byte.**
`Base2a`, `Roads2a`, `Castle1a-d`, `Castle2a-d`, `Town1a-d`, `Town2a-d` are ordinary
isometric tile sets whose family byte reads `0`. The "24 byte" and "840 byte"
overshoots were `10*6 − 6²` and `58*30 − 30²` — the difference between a rectangle
and a diamond. There was no trailing block; that hypothesis was wrong.

**4 files: stored overhang above a rectangle.**
`Fntl2_14`, `Font_10`, `T16_bat1`, `T32_bat` — 70 frames carrying RLE rows after the
rectangle, drawn above it. The apparent "undershoots" of 6, 10, 61 and 190 bytes were
just the **first** such block per file — the largest are 18, 18, 74 and 242.
`Castle2b/c/d` also first fail at −36, a residual an earlier revision omitted.

**1 file: a wrong header byte.**
`Font_c2` declares RLE but stores raw rectangles. It is the same font as `Fntl2_9`,
exported twice, sharing 103 of 108 frame records. Detected file-wide — every frame
spanning exactly `w*h` — rather than per frame, so one coincidental span cannot
reinterpret a frame of an otherwise valid RLE file.

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
