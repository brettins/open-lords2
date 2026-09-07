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
