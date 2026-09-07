# PL8 sprite format (Lords of the Realm II)

Status: **236 / 291 files verified** (16,435 frames) by round-trip offset checking,
and byte-identical between two independent decoders across all 21,344 frames in
the corpus. Two storage variants remain unidentified (see Open questions).

(`tools/pl8check.js` reports 16,638; that tally includes frames it walked inside
files that then failed, so it is not the count of *verified* frames.)

Verified against the GOG Windows release, `F:\games\Lords of the Realm II`.

## Verification method

The format is self-verifying, which makes automated validation possible with no
human judgement:

1. Each frame's decoded data must end **exactly** at the next frame's declared
   `dataOffset` (or EOF for the final frame).
2. Every decoded row must consume **exactly** `width` pixels.

`tools/pl8check.js <dir>` runs both checks over a whole directory. Any decoder
change can be re-validated against the full corpus in one command.

### Cross-implementation differential test

Offset checking proves a decoder consumed the right *number* of bytes. It says
nothing about the pixels those bytes became. So two independent decoders exist -
`tools/pl8digest.js` (Node) and `crates/l2-formats/examples/pl8digest.rs` (Rust)
- and `tools/pl8diff.ps1` requires them to produce identical output:

```powershell
.\tools\pl8diff.ps1 -Dir 'F:\games\Lords of the Realm II'
```

Each emits one line per frame, files sorted by name:

    <name> hdr mode=<m> sub=<s> frames=<n> verdict=<ok|err:...>
    <name> <frameIndex> <w>x<h> <fnv1a64 hex | err:...>

The hash covers the palette indices **and** the per-pixel opaque/transparent
mask, since transparency is part of the decode: a decoder that got coverage
right and colour wrong would still diverge. FNV-1a 64 is hand-rolled in both
languages so that `l2-formats` stays dependency-free.

Frames are digested even for files that fail the end-offset invariant, because
most of those decode every frame correctly and disagree only about trailing
bytes. The `verdict` field compares the two implementations' *failure* modes
too, so they must also agree on which files break and exactly how.

Current result: **291 files, 21,344 frames, zero divergent lines.** 18,717
frames decode and hash identically; the remaining 2,627 are refused by both
with the same error token (2,502 storage mode 2, 108 row overrun in `Font_c2`,
17 running past EOF).

**What this establishes, and what it does not.** Agreement rules out an
implementation bug in either decoder: two separately written readings of the
spec are unlikely to fail identically. It says nothing about whether the spec
is right. A misunderstanding shared by both - the sub-mode byte being ignored,
say - would agree just as cleanly. Only the self-verifying offset invariant
above, and eventually comparison against the game's own rendering, speak to
correctness.

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
- **Sub-mode 1 with storage 1** (`Font_c2`, the only such file). Its frames are
  raw, not RLE - see "Font_c2 is stored raw despite declaring RLE" below. So
  byte 0x01 can override byte 0x00, and neither byte alone names the encoding.
- Header fields at 0x04, 0x06, 0x07 and the frame record's trailing 8 bytes.
- Which palette pairs with which sprite file (currently manual).

## Tools

- `tools/pl8dump.js <file.pl8> <palette.256> <frame> <out.png>` - decode one frame to PNG
- `tools/pl8check.js <dir>` - validate the decoder across a whole directory
- `tools/pl8digest.js <dir>` - per-frame digest stream (Node half of the differential test)
- `tools/pl8diff.ps1` - assert the Node and Rust decoders agree on every frame

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

### `Font_c2` is stored raw despite declaring RLE

Turned up while building the differential harness, which reports the exact
pixel counts of every failing row. `Font_c2` is the corpus's only `1:1` file
(storage 1, sub-mode 1), and **all 108 of its frames occupy exactly `width *
height` bytes** - the signature of raw storage. The "runs" the RLE reader sees
are palette indices being misread as opcodes; the recurring `13` in its row
overruns is just the byte `0x0d` appearing first in most glyphs.

So header byte 0 is not the whole storage discriminator: sub-mode can override
it. That is one file, though, and the other 22 failures do not fall out of the
same rule (`Fntl2_14` and `Font_10` are `0:1` and only 61/108 and 101/108 of
their frames fit `width * height`). Not enough to change the decoder on - the
route remains reading the game's own loader. `Font_c2` stays in `KNOWN_FAILING`
until then, and the corpus test will announce it as "now passing" if a future
change fixes it.

### Correction worth recording

An earlier hypothesis held that all 23 files with header byte 1 == 2 fail. That
was wrong: it rested on a coincidence, since the count of `raw:2` files and the
count of failures were both 23. In fact 13 of the `raw:2` files decode correctly
as plain raw, and the true failure set spans several mode combinations. Sub-mode
is *not* currently known to affect decoding.
