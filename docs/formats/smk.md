# Smacker video (`.smk`)

Status: **45 / 45 files container-validated byte-exactly, and 45 / 45 fully decoded —
7,652 frames — by a candidate Rust decoder.** No decoder is in `crates/` yet. This
document is the input to that decision, not a record of one already taken.

Smacker is RAD Game Tools' 1994 video middleware. It is not our format and it is not
worth reverse-engineering: it is publicly documented and there are three working open
implementations. The whole question here is *which one to integrate, and at what cost*.

Analysis scripts:

```bash
node tools/media/smkinfo.js "F:/games/Lords of the Realm II"          # summary + validation
node tools/media/smkinfo.js "F:/games/Lords of the Realm II" --csv    # per-file fields
node tools/media/smkapi.js  "F:/games/Lords of the Realm II/Lords2.exe" \
                            "F:/games/Lords of the Realm II/Smackw32.dll" smack
```

## The corpus (verified)

45 files, **80,878,680 bytes**, 7,652 frames, 10.6 minutes. Every one is **`SMK2`** —
none is `SMK4`.

| Property | Value |
|---|---|
| Signature | `SMK2`, all 45 |
| Frame rate | 12.000 fps in 43 files; 14.085 fps in `BAT_WIN5.SMK` and `BAT_WIN6.SMK` |
| Video | 8-bit palette-indexed, one 768-byte palette carried in the stream |
| Audio | exactly one track (track 0) in all 45 files, 11,025 Hz, 8-bit, Huffman-packed |
| Channels | mono in 37 files, stereo in 8 |
| Ring frame | never set |
| Keyframes | **none** — all 7,652 frame-size entries have the low two bits clear |

Geometry, and what each group is (sizes verified; the "what" is inferred from filenames):

| Size | Files | Apparently |
|---|---|---|
| 400×192 | 25 | battle / siege / castle win-lose stingers (`Bat_*`, `Cas_*`, `Sge_*`, `Cap_cty*`) |
| 296×184 | 11 | character vignettes (`Axemen`, `Cart_*`, `Pill_*`, `Hang`, `Jail`, `Win_game`) |
| 320×200 | 5 | castle construction (`Castle1`–`Castle5`) |
| 640×240 | 1 | `Credits.smk`, Y-doubled → 640×480, exactly the game's screen |
| 560×144 | 1 | `Intro.smk`, Y-doubled → 560×288, 1,578 frames, 131 s |
| 500×144 | 1 | `LOM.SMK`, Y-doubled → 500×288, 1,449 frames, the largest file at 22.7 MB |
| 500×292 | 1 | `Imptitle.smk`, the Impressions logo |

The three Y-doubled files are the only ones with a non-zero header flags word (`0x02`).

### Two files are one file, and it matters

`Axemen.smk` and `AXMEN.SMK` are **two separate NTFS directory entries** (distinct file
IDs) with **identical content** (MD5 `E75F099B…`). NTFS stores names case-sensitively
even though Win32 resolves them case-insensitively, so a GOG install can and does carry
both. Deduplicated, the corpus is 44 files and 79,867,952 bytes.

The reason is visible in the exe: `Lords2.exe` asks for **`axmen.smk`**, which matches
`AXMEN.SMK` and not `Axemen.smk`. Cross-referencing every `*.smk` string in the exe
against the directory:

* The exe holds 51 distinct `*.smk` names. 44 of them exist on disk, and those are 44 of
  the 45 files: **`axemen.smk` is the only shipped file never referenced**.
* Seven referenced names have no file: `cap_cnty.smk`, `cart_cts.smk`, `cart_hmn.smk`,
  `pill_bsp.smk` and `pill_hmn.smk`, which look like cut content; the placeholder
  `null.smk`; and `292822k.smk`, a literal with no file and no obvious meaning.

So our loader must be **case-insensitive and tolerant of missing videos** — the original
plainly is. It must not assume the name in the exe matches the name on disk in case.

### The DOS install has no videos at all

`F:\games\LORDS2` contains zero `.smk` files. The exe's string table holds the
subdirectory names `PL8`, `256`, `WAV`, `SMK`, and the DOS install has `PL8\` and `WAV\`
but no `SMK\` — the videos lived on the CD. Verified.

## Container layout

Everything below is the publicly documented Smacker container. Our reader implements it
independently in `tools/media/smkinfo.js`; nothing is copied from any implementation.

Header, 104 bytes, all little-endian:

| Offset | Type | Meaning |
|--------|------|---------|
| 0x00 | char[4] | `SMK2` or `SMK4` |
| 0x04 | u32 | Width (stored) |
| 0x08 | u32 | Height (stored) |
| 0x0C | u32 | Frame count |
| 0x10 | i32 | Frame interval — see below |
| 0x14 | u32 | Flags: bit 0 ring frame, bits 1 and 2 vertical scaling |
| 0x18 | u32[7] | Largest unpacked audio buffer, per track |
| 0x34 | u32 | Size of the Huffman tree block |
| 0x38 | u32 | MMap tree size |
| 0x3C | u32 | MClr tree size |
| 0x40 | u32 | Full tree size |
| 0x44 | u32 | Type tree size |
| 0x48 | u32[7] | Audio track descriptor, per track |
| 0x64 | u32 | Unused |

Then `frames` × u32 frame sizes, `frames` × u8 frame flags, the tree block, and the
frame payloads back to back. With a ring frame the arrays hold `frames + 1` entries; no
file here has one.

**Frame interval.** Positive values are milliseconds per frame; negative values are units
of 10 µs. Every file here is negative: `-8333` → 12.0005 fps, `-7100` → 14.0845 fps.

**Audio track descriptor** (per track, 0 means "no track"): bits 0–23 are the sample rate;
bit 31 = Huffman-packed, bit 29 = 16-bit, bit 28 = stereo, bit 27 = Bink audio, bit 26 =
DCT. All 45 files read `0x8000_2B11` (mono) or `0x9000_2B11` (stereo) on track 0 and zero
on tracks 1–6.

**Frame sizes** carry flags in the low two bits (bit 0 = keyframe). In this corpus all
7,652 entries are exact multiples of four, so the flags are unused and the sizes are
unambiguous.

**Frame flags byte**: bit 0 = this frame carries a palette update, bits 1–7 = this frame
carries data for audio track 0–6. 58 frames across the corpus update the palette; 7,108
of 7,652 frames carry audio.

### The self-verifying invariant

```
104 + frames*4 + frames + treesSize + Σ frameSizes  ==  filesize
```

All 45 files satisfy this **exactly**, on the first run, with zero slack. As with PL8,
that is the property that makes automated validation meaningful: a wrong reading of any
field drifts and the sum misses. `smkinfo.js` reports the slack per file.

### Palette

Palette updates are delta-encoded against the previous palette with three opcodes —
skip, copy-from-old-at-offset, and set-three-6-bit-values. The stored values are **6-bit
(0–63)** and are expanded to 8-bit by **bit replication**, `(v << 2) | (v >> 4)`.

That is *not* the same expansion as our `.256` palettes, which `docs/formats/pl8.md`
documents as `v * 255 / 63`. The two agree at the ends and differ by one in the middle
(6-bit 16 → 0x41 by replication, 0x40 by the ratio). Worth remembering when our video
output is compared against the original pixel-for-pixel.

## What the original engine actually does

`Lords2.exe` imports `smackw32.DLL` **entirely by ordinal** — 10 symbols, no names. The
DLL does export names, so `tools/media/smkapi.js` joins the two and counts call sites.
This is the whole API surface our engine has to reproduce (verified):

| Ord | Export | Sites | Call sites |
|-----|--------|-------|------------|
| 14 | `_SmackOpen@12` | 1 | `0x0042DB13` |
| 17 | `_SmackSoundOnOff@8` | 2 | `0x0042E433` `0x0042E453` |
| 18 | `_SmackClose@4` | 4 | `0x0042D9EA` `0x0042DEBD` `0x0042DF94` `0x0042E046` |
| 19 | `_SmackDoFrame@4` | 2 | `0x0042DD0D` `0x0042DE2A` |
| 21 | `_SmackNextFrame@4` | 1 | `0x0042DF16` |
| 23 | `_SmackToBuffer@28` | 3 | `0x0042DBA3` `0x0042DD01` `0x0042DE6F` |
| 27 | `_SmackGoto@8` | 1 | `0x0042E445` |
| 28 | `_SmackToBufferRect@8` | 1 | `0x0042DD34` |
| 32 | `_SmackWait@4` | 1 | `0x0042DBF7` |
| 37 | `_SetDirectSoundHWND@4` | 1 | `0x0042E343` |

The other 29 exports are never imported. In particular **`SmackToScreen` is not used** —
the game always decodes into its own buffer and does its own presentation, which is
exactly the shape our `pixels` framebuffer wants.

### The playback path (verified by decompilation)

| Address | Name | What it does |
|---------|------|--------------|
| `0x0042DA18` | `Smk_Open(name, x, y, mode)` | Logs `OK :SMK starting smack`, copies the filename into a **16-byte** buffer at `0x005169D0`, runs the CD-directory dance, `SmackOpen`, then decodes frame 0 immediately. |
| `0x0042DBC7` | `Smk_ServiceFrame()` | One pump of the playback loop. Returns 0 when finished. |
| `0x0042DFE2` | `Smk_Stop()` | `SmackClose`, restore working directory. |
| `0x0042E320` | `Smk_InitSound()` | `SetDirectSoundHWND(hwnd)`, once per process. |
| `0x0042E3BA` | `Smk_OnPaint(hwnd)` | `WM_PAINT`: `SmackSoundOnOff(0)`, `SmackGoto(savedFrame)`, `SmackSoundOnOff(1)`. |
| `0x0042E298` | `Smk_CopyPalette()` | Copies 256 × 3 bytes from the Smack handle to the game palette. |

`Smk_ServiceFrame` per iteration:

1. `SmackWait` — RAD's own frame pacing; the loop stalls until the frame is due.
2. If the handle's field at `+0x68` is non-zero, copy its 768-byte palette (at `+0x6C`)
   into the game palette at `0x004EA1B0` and upload it. Verified: the copy is a plain
   256×3 byte loop with **no scaling**, and the DirectDraw upload at `0x0042F306` is also
   a plain byte copy into `PALETTEENTRY`. Two consequences: `smackw32` hands back **8-bit**
   values, and palette entries **0 and 255 are pinned** to fixed engine colours with
   `peFlags = 0`, while entries 1–254 come from the movie with `PC_NOCOLLAPSE`.
3. If the movie is `intro.smk`, call `0x0041A166` with the current frame number — the
   intro alone drives frame-cued events (subtitles, most likely; inferred).
4. `SmackDoFrame`, then `SmackToBuffer(handle, x, y, 640, 480, backbuffer, 0)` into the
   game's 8-bit 640×480 buffer at `0x004EA1A8`, then mark the region dirty via
   `0x004527A6`. A second path, taken when `0x004DF28C` is set, locks a DirectDraw
   surface instead and uses the surface pitch and `SmackToBufferRect` to blit only the
   changed rectangle.
5. `SmackNextFrame` while `currentFrame < frames - 1`; otherwise `SmackClose` and log
   `OK :SMK natural end of`. `OK :SMK user ends` is the abort path.

Handle fields used, by offset (verified by use, and consistent with the published
`Smack` structure): `+0x04` width, `+0x08` height, `+0x0C` frame count, `+0x68` palette-
changed flag, `+0x6C` 768-byte palette, `+0x374` current frame, `+0x378`/`+0x37C`/`+0x380`
/`+0x384` the last dirty rectangle.

`SmackOpen`'s flags argument is `0`, `0x2000`, or `0x2400` depending on whether a window
handle exists and on the mode argument. The meaning of those bits is **not established** —
they are RAD's, undocumented publicly, and nothing we have decodes them.

## Integration options

Weighed on licence, on Windows build friction (this project has deliberately had no
native build dependencies — see `docs/decisions.md`, D4), and on the hard requirement
that the **original `.smk` files must play directly**, because users bring their own copy
of the game and we may not ship converted video.

| Option | Licence | Native deps on Windows | Verdict |
|---|---|---|---|
| **`smk` 0.1.0** — pure Rust, a declared port of libsmacker 1.2.0 | LGPL-2.1-or-later | **none** (one dep: `log`) | **Recommended** |
| `libsmacker` + `libsmacker-sys` 0.1 | LGPL-2.1 | vendored C, needs `cc` and a C toolchain | Same licence, strictly more friction |
| `ffmpeg-next` 9 / `ffmpeg-sys-next` | crate WTFPL, **FFmpeg itself LGPL-2.1+** | LLVM/libclang, a prebuilt shared FFmpeg, `FFMPEG_DIR`, DLLs on `PATH` | Reject |
| Transcode at install time | n/a | n/a | Reject — see below |
| Write our own SMK2 decoder | MIT, ours | none | The only licence-clean route; not now |

**The task brief's premise needs correcting: libsmacker is not permissively licensed.**
It has been **LGPL v2.1 since January 2020**. Every working Smacker implementation is
copyleft — libsmacker (LGPL-2.1), FFmpeg's `libavcodec/smacker.c` (LGPL-2.1+), ScummVM's
(GPL). The one public-domain implementation, `mewspring/smk` (Go, Unlicense), is
**8 KB of header parsing only** — it parses exactly what `smkinfo.js` already parses and
contains no codec. There is no permissive decoder to adopt.

**Transcoding is not an escape hatch.** We cannot ship converted video, so conversion
would have to run on the user's machine — which needs a Smacker decoder anyway. It only
adds an install step, disk use, and a fidelity question (these are 8-bit palettized
frames driving the same palette the rest of the screen uses). Decoding to a local cache
is a reasonable *optimisation* once a decoder exists; it is not a substitute for one.

### Verification of the recommendation

The recommendation is not on reputation — the `smk` crate has one release, no stars and
no external users worth speaking of. It was measured. A throwaway harness in the
scratchpad decoded **every frame of all 45 files**:

* 44 / 45 decode cleanly. Frame counts match `smkinfo.js` exactly (7,652), geometry
  matches, audio track configuration matches, and 8,798,274 bytes of PCM come out.
* Whole corpus in **1.7 s** — about 4,500 frames/second, against a 12 fps requirement.
* Builds with zero native dependencies, one transitive crate (`log`).
* Reports the three Y-doubled files correctly and hands back stored-size frames, so the
  caller does the line doubling.

**One real bug, found and diagnosed.** `Pill_brn.smk` fails at frame 104 with
`InvalidData("palette copy overlaps destination")`. The crate rejects a palette
copy-block whose source range straddles the write cursor — a guard that is meaningful in
an implementation that copies in place, but this implementation copies out of a snapshot
of the previous palette taken at entry, so overlap is harmless. Verified: with that guard
removed in a local copy, **45 / 45 decode fully, 7,652 frames**, matching the container
totals exactly. FFmpeg reads from a saved palette too and would decode this file. The
file is not corrupt.

That is a one-line upstream fix. Until it lands, this file is a hard failure, so a
decision to adopt the crate carries a decision to patch or vendor it.

### What adopting it costs us

* **The MIT story gets a footnote.** Our source stays MIT; a *binary* that links an
  LGPL-2.1 library is a combined work and must let its recipients relink against a
  modified library. A public source tree plus normal `cargo build` instructions satisfies
  that in practice, and LGPL does not reach our own code. But `docs/decisions.md` D5
  currently reads "this project is MIT" without qualification, and that stops being the
  whole truth the day this lands.
* **We must isolate it.** `crates/l2-formats` is deliberately dependency-free; the video
  decoder must not go there. Put it in its own leaf crate behind our own small trait
  (open, next frame, indexed pixels + palette + PCM), so the LGPL boundary is one
  directory and swapping in our own decoder later is a one-crate change.
* **We inherit an unmaintained dependency.** One release, April 2026, no visible
  community. Vendoring is the realistic fallback, and vendoring LGPL source means keeping
  its licence and marking our modifications.
* **We are trusting a port we have partly checked.** Frame counts and clean decodes prove
  the container walk and the bitstreams are being consumed correctly; they do not prove
  the pixels are right. That check is the same one PL8 will get: render a frame and diff
  it against the original engine's framebuffer.

The escape hatch stays open. The corpus is narrow — SMK2 only, 8-bit, one packed audio
track, no ring frames, no keyframes — so a from-scratch MIT decoder is a bounded job
(the crate is ~1,455 lines including audio) if the licence footnote ever becomes
unacceptable. It is not worth doing before there is a renderer to point it at.

## Verified, inferred, and open

**Verified**: signature, geometry, frame counts, frame rates, audio configuration and
footprint of all 45 files; the size invariant; the absence of ring frames and keyframes;
the duplicate `Axemen`/`AXMEN` pair and the exe's `axmen.smk` reference; the ten imported
ordinals and their call sites; the playback loop, the 640×480 destination buffer, and the
unscaled palette copy with entries 0 and 255 pinned.

**Inferred**: what each geometry group is used for (from filenames); that `0x0041A166`
on `intro.smk` drives subtitles; that `smackw32` returns an 8-bit palette (it follows from
the upload doing no scaling, given `.256` palettes are 6-bit and must be scaled somewhere).

**Open:**

* **Y-scaling is ambiguous, and the two references disagree.** libsmacker treats flag bit
  `0x02` as **Y-double** and `0x04` as **Y-interlace**; FFmpeg's demuxer names them the
  other way round and implements neither, merely halving the aspect ratio. Our three
  affected files set `0x02`. Both readings double the display height — 640×240 → 640×480
  is certainly right for `Credits.smk` — but whether the extra rows are duplicates of
  their neighbours or blank interleave is not settled by anything in the data. Resolve it
  against the running original.
* `SmackOpen`'s `0x2000` / `0x2400` flag bits.
* `0x004527A6(x, y, 0x14, 10, 1)` marks a dirty region in 16-pixel units, which works out
  to 320×160 — smaller than any of the movies. The unit or the argument meaning is not
  fully understood.
* Seeking. No file flags a keyframe, so `SmackGoto` must be re-decoding from frame 0. Our
  implementation will have to do the same, or cache.
* Nothing here has been rendered. Correct-looking totals are not correct-looking pixels.

## Sources

* [libsmacker](https://libsmacker.sourceforge.net/) — format reference and licence
  (LGPL-2.1 since January 2020)
* [`smk` crate](https://crates.io/crates/smk) / [source](https://github.com/roarc0/smk)
* [`libsmacker-sys`](https://crates.io/crates/libsmacker-sys)
* [FFmpeg `libavformat/smacker.c`](https://github.com/FFmpeg/FFmpeg/blob/master/libavformat/smacker.c)
* [`mewspring/smk`](https://github.com/mewspring/smk) — Unlicense, header parsing only
* [rust-ffmpeg build notes](https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building)
