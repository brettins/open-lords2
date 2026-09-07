# Named addresses in Lords2.exe

GOG Windows build, 21 Jul 2009, 1,031,680 bytes. `ImageBase = 0x400000`, **no ASLR**, so
every address below is stable across runs and safe to hardcode.

Ghidra project: `E:\dev\ghidra-projects\lords2`. The binary has no symbols; these names
are ours. Where a name is a guess, the confidence column says so.

## Sprite rendering

| Address | Name | Confidence | What it does |
|---------|------|-----------|--------------|
| `0x0040A21A` | `Pl8_DrawFrame(buf, frame, x, y)` | verified | Reads the frame record at `buf + frame*0x10 + 8`, range-checks the data offset, calls the clipper, dispatches to a blitter. Independently confirms the 8-byte header / 16-byte record layout we had inferred from offset arithmetic. Also reads record bytes +12, +13 and +14. |
| `0x0040464C` | `Clip_Horizontal(left, right)` | verified | Computes clipping and sets the globals below. |
| `0x004B43B1` | `Blit_Unclipped(buf)` | verified | Copies `width` bytes per row, skipping zero bytes; 4x unrolled. Loops `DAT_005BB478` rows — the **clipped** row count, not the frame height. |
| `0x0040477B` | `Clip_Vertical(...)` | verified | Sets `DAT_005BB478` to `height − rowsClippedAtTop`, unconditionally. |
| `0x00402A14` | `Glyph_Draw(...)` | verified | Text blitter. Shifts the destination down by frame-record byte `0x0D` before clipping, reserving rows above the rectangle. The only consumer of that byte. |
| `0x004B446D` | `Blit_ClippedLeft(buf)` | inferred | Selected when clip state is 1. |
| `0x004B44D2` | `Blit_ClippedRight(buf)` | verified | As unclipped, but also advances the *source* pointer by the clipped-off remainder each row. |

**Palette index 0 is transparent.** Every blitter copies a byte only when it is non-zero.
This is a property of the engine, not something recorded in the file format — and it is
the bug our own decoder had until the binary revealed it.

A further family of blitters sits around `0x004BC000`–`0x004BF000`. They also read the
clip state and are presumably variants for other pixel paths. Not yet examined.

## File I/O

| Address | Name | Confidence | What it does |
|---------|------|-----------|--------------|
| `0x004AF8D9` | `File_ReadChunk(name, buf, len, offset)` | verified | open / lseek / read / close. Retries twice after `chdir` on failure — almost certainly the original CD-path lookup. |
| `0x004AF502` | `Restore_WorkingDir()` | verified | `chdir` back, reporting via `getcwd` on failure. |
| `0x004184C6` | `Armoury_LoadScreen()` | verified | A worked example of the load-then-draw path: selects a weapon sprite set from a global, reads it into a 250 KB buffer, then calls `Pl8_DrawFrame`. |

## Networking

| Address | Name | Confidence | What it does |
|---------|------|-----------|--------------|
| `0x004B7FCE` | call to `DirectPlayEnumerateA` | verified | The only call site. Populates the connection-type list. |
| `0x004B826E` | call to `DirectPlayCreate` | verified | The only call site. |

The exe imports exactly two functions from `DPLAYX.dll`, by ordinals 1 and 2 — the whole
network entry surface. It also references `sierranw.dll` and `snwvalid.dll` (Sierra's
online matchmaking), but neither ships with the GOG release and neither appears in the
import table, so that path is dead code.

## Globals

| Address | Meaning | Confidence |
|---------|---------|-----------|
| `0x005CDD20` | Clip state: 0 unclipped, 1 clipped left, 2 clipped right, 5 fully offscreen | verified |
| `0x00591514` | Visible (clipped) width | verified |
| `0x0059150C` | Source bytes skipped per row = width − visible width | verified |
| `0x005CD400` | Destination advance per row = screen stride − visible width | verified |
| `0x005C9258` | Current sprite width | verified |
| `0x005AEB70` | Current sprite height | verified |
| `0x0058FE04` | Current source data offset | verified |
| `0x004EB274` | Screen stride, 640 | inferred |
| `0x005BB478` | Blitter row counter. **Not** frame-record byte `0x0D` — `Clip_Vertical` overwrites it before every blit, so the byte-`0x0D` load in `Pl8_DrawFrame` is dead. | verified |
| `0x004EABD0` | A DirectDraw interface pointer. It was NULL at the crash at `0x004522B5`, which happened when the window was deactivated during startup under DxWnd. | verified |

## The game logs its own startup

`Lords2.exe` writes `status.txt` beside itself, narrating initialisation with lines like
`OK :DD Set resolution.` and `ERR:`-prefixed failures. This is the best available anchor
for identifying subsystems: find a log string, find what writes it, and a subsystem is
named.

It also looks for a `sierra.ini` that the GOG release doesn't ship, and logs the failure.
Not known to be fatal.

## Layout

```
.text   0x401000 – 0x4CFB06   ~846 KB of code
.rdata  0x4D0000
.data   0x4D2000             ~1 MB virtual, only 80 KB on disk
                             → ~955 KB of zero-initialised globals: the game state
.idata  0x5CF000
```

That `.data` region is why incremental replacement works: the live game state sits at
fixed addresses and can be read from another process while the game runs.

## Map loading

| Address | Name | Confidence | What it does |
|---------|------|-----------|--------------|
| `0x00467770` | `Map_LoadPlanes()` | verified | Reads the six 64x64 byte planes of a map slot, at slot offsets `+0x0000` … `+0x5000`, in a 64x64 nest. |
| `0x0046797D` | `Map_LoadLattice()` | verified | Reads the trailing 65x129 layer — `for(row < 0x81) for(col < 0x41)`. |
| `0x0040526E` | `Map_RenderIso()` | verified | Walks the 65x129 lattice. Cells below `0x0FFF0000` hold a runtime pointer into the tile array; cells still holding `0x0FFF0000 + b` are off-map surround, where `b` is the background tile graphic index from the file. |

Map slots are addressed by `File_ReadChunk` with `lseek(index * 0x80C1)` — 0x80C1 is
32,961, the slot stride. See `docs/formats/maps.md`.

**`mapl2.exe` is a dead end.** Despite shipping in the game folder, its strings identify
it as the "L2 Battlemap editor" for `.skr` battle scenarios, and it contains no reference
to `l2_maps.dat` at all. The reference implementation for the campaign map format is
`Lords2.exe` itself. Recorded here so nobody re-investigates it.
