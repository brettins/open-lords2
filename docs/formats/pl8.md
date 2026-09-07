# PL8 sprite format (Lords of the Realm II)

Status: **236 / 291 files verified** (16,638 frames) by round-trip offset checking.
Two storage variants remain unidentified (see Open questions).

Verified against the GOG Windows release, `F:\games\Lords of the Realm II`.

## Verification method

The format is self-verifying, which makes automated validation possible with no
human judgement:

1. Each frame's decoded data must end **exactly** at the next frame's declared
   `dataOffset` (or EOF for the final frame).
2. Every decoded row must consume **exactly** `width` pixels.

`tools/pl8check.js <dir>` runs both checks over a whole directory. Any decoder
change can be re-validated against the full corpus in one command.

## Header (8 bytes)

| Offset | Type | Meaning |
|--------|------|---------|
| 0x00 | u8  | **storage mode**: 0 = raw, 1 = RLE, 2 = unidentified |
| 0x01 | u8  | sub-mode / flags: 0, 1, or 2. Affects layout (see Open questions) |
| 0x02 | u16 | frame count |
| 0x04 | u16 | unknown; varies widely (0..287), 0 in 80 files |
| 0x06 | u8  | unknown, usually 0 |
| 0x07 | u8  | unknown, ranges 0..15 |

Observed (mode:sub) combinations across 291 files:

    1:0 = 138    0:0 = 94    2:0 = 20    0:2 = 23
    2:2 = 10     0:1 = 3     2:1 = 2     1:1 = 1

## Frame table

Immediately follows the header: `frameCount` records of 16 bytes each.

| Offset | Type | Meaning |
|--------|------|---------|
| 0x00 | u16 | width |
| 0x02 | u16 | height |
| 0x04 | u32 | absolute file offset of this frame's pixel data |
| 0x08 | u8[8] | not padding - frequently non-zero. Likely anchor/hotspot + flags. Undecoded. |

Confirmed invariant: the first frame's `dataOffset` always equals
`8 + frameCount * 16`, i.e. pixel data begins right after the frame table.

## Pixel data

Pixels are 8-bit indices into a separate `.256` palette.

### Storage mode 0 - raw

Exactly `width * height` bytes, row-major, no compression, no transparency.

### Storage mode 1 - RLE

Decoded per row; each row consumes exactly `width` pixels:

    n = next byte
    if n == 0:  m = next byte; skip m pixels (transparent)
    if n >  0:  copy the next n bytes as literal palette indices

## Palette files (.256)

768 bytes = 256 entries x 3 bytes (R, G, B). Values are **6-bit VGA (0..63)** and
must be scaled to 8-bit (`v * 255 / 63`).

Palettes are per-context, not global - using the wrong one yields a structurally
correct but wildly miscoloured sprite. `T32_bat1.256` is the battle-sprite
palette; `Lords2.256` is not.

## Open questions

- **Storage mode 2** (32 files, e.g. `Arm_grid`, `Backgrnd`, `Base01`, `Batlfix2`).
  Rows overshoot under RLE rules, so it is a third encoding.
- **Sub-mode 2 with storage 0** (23 files, e.g. `Base2a`). Frame data is *smaller*
  than `width * height` - `Base2a` frame 0 is 24 bytes short - so byte 0x01
  changes the raw layout rather than being a pure flag.
- Header fields at 0x04, 0x06, 0x07 and the frame record's trailing 8 bytes.
- Which palette pairs with which sprite file (currently manual).

## Tools

- `tools/pl8dump.js <file.pl8> <palette.256> <frame> <out.png>` - decode one frame to PNG
- `tools/pl8check.js <dir>` - validate the decoder across a whole directory

---

## Storage mode 2 - investigation notes

32 files, ~2,400 frames. Not yet decoded. What has been established:

**It is not run-length encoded.** `Batlfix2.pl8` frame 0 (26x14) is stored as 196
identical `0x3f` bytes. Any run-length scheme would collapse a solid frame to a
handful of bytes, so the encoding has no repeat primitive.

**Bytes-per-row is usually `ceil(width/2) + 1`.** This holds for 1,847 frames -
the dominant pattern - and initially suggested 4 bits per pixel plus a one-byte
row prefix.

**But that theory is refuted by odd-width frames.** `Fntl2_9.pl8` (a font file)
has frames of `w=7, bpr=7`, `w=5, bpr=5` and `w=9, bpr=9`: bytes-per-row equals
width exactly, i.e. plain 8-bit raw. Mode 2 is therefore heterogeneous, holding
both raw and packed frames.

**Row length is not always fixed.** 336 mode-2 frames have a total size that is
not divisible by their height at all.

**The frame record's trailing bytes are not the discriminator.** Across mode-2
frames, `t[4]=1` and `t[5..7]=0` are constant, while the u16 at `t[2]` runs in an
arithmetic sequence (0, 7, 14, 21, 28, ...). That reads as a position or ordering
key - likely a sprite-sheet coordinate - not an encoding flag.

**High-entropy sample.** `Mtns2a.pl8` frame 3 (10x6) is 36 bytes with 27 distinct
values. Byte values fall in the same range as the confirmed 8-bit palette indices
used by mode 1, which argues against packed nibbles - but 36 bytes cannot hold 60
8-bit pixels, so some pixels are being omitted by a mechanism not yet identified.

### Next step

Statistical inference has plateaued: two readings of the same bytes remain
consistent with all observations. The reliable route is to read the game's own
decoder - locate the PL8 loading routine in `Lords2.exe` (Ghidra) and follow the
branch taken when header byte 0 is 2. This is the oracle principle applied to a
file format rather than to game logic.

---

## Files that still fail under supported storage modes

23 files use storage mode 0 or 1 but do not satisfy the end-offset invariant.
They are pinned in `KNOWN_FAILING` in `crates/l2-formats/tests/corpus.rs`, so a
new failure breaks the build and a fix is reported as "now passing".

The residuals are not random - they cluster:

| Pattern | Files |
|---------|-------|
| Frame data overshoots by exactly **24 bytes** | `Base2a`, `Roads2a`, `Castle2a`, `Town2a`, `Town2b-d` |
| Overshoots by exactly **840** bytes (24 x 35) | `Castle1a-d`, `Town1a-d` |
| Undershoots | `Fntl2_14` (6), `Font_10` (10), `T16_bat1` (61), `T32_bat` (190) |
| RLE row overrun | `Font_c2` (13 pixels consumed, width 7) |

The recurring 24 - and 840 being an exact multiple of it - suggests a fixed
trailing block appended after the pixel data in some files, rather than a
different pixel encoding. Undershoots are more likely a genuinely different
encoding, and the font files may be a format of their own.

### Correction worth recording

An earlier hypothesis held that all 23 files with header byte 1 == 2 fail. That
was wrong: it rested on a coincidence, since the count of `raw:2` files and the
count of failures were both 23. In fact 13 of the `raw:2` files decode correctly
as plain raw, and the true failure set spans several mode combinations. Sub-mode
is *not* currently known to affect decoding.
