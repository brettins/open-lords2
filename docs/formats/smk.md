# Smacker video (`.smk`)

Status: **45 / 45 files decoded by our own MIT decoder, `crates/l2-smk` — 7,652 frames,
8,798,274 bytes of PCM — and every one played by the game where `Lords2.exe` plays it.**
Every frame's pixels, every frame's palette and every sample of 44 films hash identically
to an independent decoder's output; the 45th, `Pill_brn.smk`, matches to as far as that
decoder can go. The integration question this document was first written to settle is
closed: **no third-party decoder is linked**, so `docs/decisions.md` D5a's licence choice
does not arise.

Smacker is RAD Game Tools' 1994 video middleware. It is not our format and it was not
reverse-engineered here: its container, trees, block codes, palette opcodes and audio
coding are publicly described, and those facts are what `crates/l2-smk` is written from.

Analysis scripts:

```bash
node tools/media/smkinfo.js "F:/games/Lords of the Realm II"          # summary + validation
node tools/media/smkinfo.js "F:/games/Lords of the Realm II" --csv    # per-file fields
node tools/media/smkapi.js  "F:/games/Lords of the Realm II/Lords2.exe" \
                            "F:/games/Lords of the Realm II/Smackw32.dll" smack
LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-smk --test corpus
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

Geometry, and what each group is — the sizes verified, and the "what" now verified too,
from the call site that plays each (§ *Every film the game plays*):

| Size | Files | What plays it |
|---|---|---|
| 400×192 | 25 | `Battle_CheckOutcome` (`Bat_*`, `Cas_*`, `Sge_*`) and the animated capture message (`Cap_cty*`) |
| 296×184 | 11 | the animated ending message (`Axmen`, `Cart_*`, `Pill_*`, `Hang`, `Jail`, `Win_game`) |
| 320×200 | 5 | `CastleBuild_Confirm` (`Castle1`–`Castle5`), in the chooser's 320×200 preview well |
| 640×240 | 1 | `Credits.smk`, Y-doubled → 640×480, exactly the game's screen |
| 560×144 | 1 | `Intro.smk`, Y-doubled → 560×288, 1,578 frames, 131 s |
| 500×144 | 1 | `LOM.SMK`, Y-doubled → 500×288, 1,449 frames — **Sierra's *Lords of Magic* trailer** |
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
  `null.smk`; and `292822k.smk`, a literal with no file and no obvious meaning. **All seven
  are in the debug viewer's table only** (below), and the game's own ending table gives the
  Countess her pillory twice and the Bishop his cart twice rather than name them.

So our loader is **case-insensitive and tolerant of missing videos**, as the original is.

### The DOS install has no videos at all

`F:\games\LORDS2` contains zero `.smk` files. The exe's string table holds the
subdirectory names `PL8`, `256`, `WAV`, `SMK`, and the DOS install has `PL8\` and `WAV\`
but no `SMK\` — the videos lived on the CD. Verified. On such an install every `Smk_Open`
fails and every caller takes its fail arm, which ours reproduces.

## Container layout

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

**Inside a frame**, in order: the palette chunk (one length byte in units of four, counting
itself), then each flagged audio chunk (a u32 length counting itself), then the video
bitstream to the end of the frame.

### The self-verifying invariant

```
104 + frames*4 + frames + treesSize + Σ frameSizes  ==  filesize
```

All 45 files satisfy this **exactly**, on the first run, with zero slack. `smkinfo.js`
reports the slack per file and `l2_smk::Smk::slack` returns it.

## The codec, as `crates/l2-smk` implements it

Every bitstream is read **least significant bit first**, within bytes in order.

**Trees.** The tree block holds four 16-bit trees — MMap, MClr, Full, Type. Each is a
presence bit; a low-byte and a high-byte 8-bit tree; three 16-bit *escape* values; the tree
itself, depth-first (`1` a branch, `0` a leaf spelled as a low-tree code then a high-tree
code); and a closing `0`. An 8-bit tree is a presence bit, the tree depth-first with 8-bit
leaves, and a closing `0`. A leaf equal to escape *n* stands for the *n*-th most recent value
that tree produced; after each decode a value that is not already the most recent is pushed
to the front of the three, which reset to zero at the start of every frame.

**Blocks.** The picture is 4 × 4 blocks in reading order. A Type code gives a block type in
its low two bits, a run length index in the next six (1…59, then 128, 256, 512, 1024, 2048)
and a colour in its high byte. *Mono*: a MClr code (two colours) and a MMap code (sixteen
bits, top row first, least significant first; set picks the high colour). *Full* (`SMK2`):
per row two Full codes, the first filling columns 2 and 3, the second columns 0 and 1, low
byte first. *Skip*: last frame's pixels stand. *Solid*: the run's colour.

**Palette.** Against the palette as it stood before the frame: `1nnnnnnn` keeps `n + 1`
entries; `01nnnnnn s` copies `n + 1` entries from the **old** palette at `s`; `00rrrrrr gg bb`
sets one entry. Components are 6-bit, widened by replicating their top bits,
`(v << 2) | (v >> 4)` — **not** the `.256` files' `v * 255 / 63`, which differs by one in the
middle of the range. Because copies read the snapshot, a copy whose source overlaps what the
frame has already written is harmless; `Pill_brn.smk` frame 104 is the one film that does it.

**Audio.** A u32 unpacked length; then a *data present* bit, a stereo bit and a 16-bit bit;
one 8-bit delta tree per channel; the first sample of each channel as 8 raw bits, **right
before left**; then one signed delta per sample, channels interleaved from the left.

**Refused rather than guessed:** `SMK4`'s extra full-block modes and 16-bit audio. Neither
occurs here.

### How it was checked

1. **The container closes** for all 45.
2. **Every bitstream is consumed to its padding.** Across 7,652 video frames and 7,108 audio
   chunks the decoder leaves 0–31 bits unread — chunks are padded to four bytes — and every
   audio chunk decodes to exactly the length its header promises.
3. **An independent decoder agrees.** A throwaway program in the scratchpad ran the LGPL
   `smk` crate **as a black box** — its public API only, read off its generated rustdoc; its
   source never opened; nothing of it committed — over all 45 films and printed FNV-1a hashes
   of every frame's pixels, every frame's palette, every track-0 sample and frame 10 alone.
   **All four agree for 44 films.** `Pill_brn.smk`'s frame-10 and audio hashes agree; that
   crate stops at the file's frame 104 on its overlap guard, so its whole-film hashes are ours
   alone. The hashes are pinned in `crates/l2-smk/tests/corpus.rs` as literals.

Ablated: swapping the two Full codes of a row turns the independent-decoder test red on the
first film at frame 10 while the padding test stays green; dropping the recency cache update
overruns a chunk on the first film.

**What none of that is**: `smackw32.dll` drawing a frame. Two decoders agreeing is strong
evidence about the bitstream and no evidence about presentation — see *Open*.

## What the original engine does with them

`Lords2.exe` imports `smackw32.DLL` **entirely by ordinal** — 10 symbols, no names. The
DLL does export names, so `tools/media/smkapi.js` joins the two and counts call sites
(verified):

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

**`SmackToScreen` is not used** — the game always decodes into its own 640 × 480 buffer.

### The playback path (verified by decompilation)

| Address | Name | What it does |
|---------|------|--------------|
| `0x0042D91B` | `Smk_Play(path, x, y, mode, returnScreen)` | `Smk_Open`; on success `g_smkReturnScreen = returnScreen; g_screenId = 0x22`, on failure `g_screenId = returnScreen`. |
| `0x0042DA18` | `Smk_Open(path, x, y, mode)` | Copies the name into a 16-byte buffer, the CD/hard-disk path dance, `SmackOpen(path, flags, -1)`, then `SmackToBuffer` and one `Smk_PlayLoop` — **the first frame is up before `Smk_Play` returns**. |
| `0x0042DBC7` | `Smk_PlayLoop()` | Called once a frame from `Battle_Frame`. **`if (SmackWait(g_smack) == 0)` guards the whole body** — a poll that does nothing until the frame is due; then palette if changed; `FUN_0041A166(frame)` for `intro.smk` only; `SmackDoFrame`; `SmackToBuffer` **only while `frame < frames - 1`**, so the last frame is decoded and never drawn; `SmackNextFrame`, or close and `Smk_OnFinished`. |
| `0x0042DF30` | `Smk_Skip()` | *"OK :SMK user ends"* — close and `Smk_OnFinished`. |
| `0x0042DFE2` | `Smk_CloseQuiet()` | Close without `Smk_OnFinished`; `WM_DESTROY` only. |
| `0x0042E060` | `Smk_OnFinished()` | The start-up chain (below) while `g_appPhase == 1`; otherwise restore the screen, `Music_Play("setup.wav")` back on setup page 1, and `Music_StartCampaign()` if `g_battlePhase == 0`; over the battle banner, `DAT_00568470 = 5001`. |
| `0x0042E320` | `Smk_BindDirectSound()` | `SetDirectSoundHWND(g_directSound)`, once. |
| `0x0042E362` | `Smk_ApplyPalette()` | The film's 768 bytes become the whole screen's palette. |
| `0x0042E3BA` | `Smk_OnPaint(hwnd)` | `WM_PAINT`: black the film's rectangle and re-seek to the current frame. |
| `0x0042D96E` | `Smk_PlayThenClose` | **No caller.** |

`SmackOpen`'s flags are `0`, `0x2000` when DirectSound is up, and `0x2400` for `mode == 1`
(no caller passes 1). `[I]` from RAD's published SDK constants: `0x2000` is `SMACKTRACK1`,
play audio track 0, and `0x400` is `SMACKNOSKIP`.

### What paces a film (verified)

**Not the game.** `App_WinMain`'s pump (`0x0040E9AB`) has no throttle of any kind —
`PeekMessageA`, else `App_IdleFrame` (`0x0040E8BB`) → `App_Draw` → `Battle_Frame`, round
again; its only `Sleep` is the 200 ms taken when the window is inactive. The original has no
tick and no frame rate, and `Smk_PlayLoop`'s body is entirely under
`if (SmackWait(g_smack) == 0)`. So the film's clock is `smackw32`'s, polled.

**And `SmackWait` reads `timeGetTime`.** `_SmackWait@4` is 320 bytes at RVA `0x3170` of
`Smackw32.dll`, and the only imported function it calls is `WINMM.dll!timeGetTime`, at
`+0xB0` — `[V]`, by matching every `FF 15 <imm32>` in `BEGTEXT` against the import table and
attributing each site to the export it falls inside. The deadline is therefore real
milliseconds. Whether the sound driver slews it is `[O]`: the DirectSound path installs a
`timeSetEvent` callback, `_TimerFunc@20`, which also reads `timeGetTime`, and the call sites
do not say what it writes.

**It does not matter, and this is the measurement that settles it.** Every one of the 45
films carries a track exactly `frames × period` long — **within 1 ms**, on films up to 131 s
(`crates/l2-smk/tests/corpus.rs`, `every_track_is_as_long_as_its_picture`). "The audio
buffer" and "the header's frame rate" are one clock to a part in 10⁵, so a player pacing the
header's rate against a *real* clock gets the audio's answer. Ours does: `l2_game::clock`,
and `docs/decisions.md` C193 for the two percent it used to lose instead.

## Every film the game plays

`Smk_Play` has **seven callers**, and they are the whole of the game's video. `[V]` on every
address, coordinate and file name — the names dumped out of `.rdata` at the address each call
site indexes. `crates/l2-game/src/movie.rs` carries the same table beside the code.

| caller | trigger | film | at | after |
|---|---|---|---|---|
| `FUN_004B3571(0)`, from `App_WinMain` | start-up | `intro.smk` | (40, 80) | the chain |
| `Smk_OnFinished`, `g_appPhase == 1` | `intro.smk` ended or was skipped | `imptitle.smk` | (80, 80) | the chain |
| 〃 | `imptitle.smk` ended or was skipped | `credits.smk` | (0, 0) | the title page |
| `FUN_00432B05`, hotspot 4 | *"Lords of Magic?"* — the third record of the title page's table, **kind 3, on the release** | `lom.smk` | (70, 80) | setup page 1, `setup.wav` from the top |
| `CastleBuild_Confirm` | a castle ordered, animations on | `castle1`…`5.smk` by level | (158, 20) | the map; bed restarts |
| `Msg_DrawWindow`, category `0x0D` | a capture letter opens, animations on | `cap_cty1`…`3.smk`, rotating, **first shown is `cap_cty2`** | (40, 105) | the map; bed restarts; narrator reads over the film |
| `Msg_DrawWindow`, category `0x0E` | an ending letter opens, animations on | `FUN_00475B41`: `win_game.smk` for 0xE1; else by lord and years since 1267 — cart <6, pillory <12, jail <18, gallows <24, axe; a human jail <12, gallows <32, axe | (89, 105) slow media, (25, 81) fast | the map or, if `Msg_Dismiss` ended the game, screen 0x1C; narrator reads over the film |
| `Battle_CheckOutcome` | the banner raised, animations on and the local player a side | `bat_win1.smk + (outcome * 4 + DAT_0053F084) * 0x10` — six rows of four; a second table under `DAT_0057A0F0` | (39, 73) | the banner, which leaves with it |
| `Smk_ReplayIntro` | screen `0x44`'s replay thumb | any of 40 names at `0x004D4D60` | (39, 73) | **unreachable**: no `mov byte ptr [g_screenId], 0x44` anywhere in the image |

**Which films no reachable path plays.** `axemen.smk` (never named), and the six films of
the third battle mode's table — `bat_win5`, `bat_win6`, `bat_los5`, `bat_los6`, `cas_win3`,
`cas_los3`, the six dated 1997 — which play only under `DAT_0057A0F0`, a mode this engine
does not have. `crates/l2-game/tests/movies.rs` pins that list against the install.

**Ending a film early** is `Smk_Skip`, and it has three callers: `Screen_FrameInput`'s `0x22`
arm (the multiplayer sync latch; a **right release**; a **left release**; `DAT_004EABB4`, set
on **any `WM_KEYDOWN`**), `FUN_0043AD25` from `Turn_Tick`'s end-of-season phase, and
`Net_LeaveGame`. A skip is `Smk_OnFinished`, so a skip during start-up moves one film on.
**Escape** is also a key, and during the front end the window procedure additionally sets
`g_quitRequest = 1` — so Escape during the original's intro quits the program. Ours skips.

**The intro's frame cues**, `FUN_0041A166`, draw `L2.eng` group 301's eleven lines — *"1268
AD"* onward — centred at y 400 in colour `0xF5`, **only when group 300's first seven
characters are not `"English"`**. On this install they are, so the intro carries no text.

## What ours does, and does not

Built (`crates/l2-game`): screen `0x22` as `ScreenId::Movie(Film)`, every trigger above but
the debug viewer, the four skips that are input, the start-up chain, the whole screen under
the film's palette, the film's sound track through the mixer ungated by the three sound
switches, the music bed stopping for the film and **restarting from its first sample** after
it or after a film that would not open, the narrator over the capture and ending films, the
dimmed and taller windows the animated message and banner branches draw once, and
`FUN_0041A166`'s cues for a translated `L2.eng`. `crates/l2-game/tests/movies.rs` drives all
of it through `Machine::handle` / `update` and `Director::listen`.

Not built, each recorded in `docs/arms.json` or `docs/audio.json`:

* **The capture film cannot be reached.** Nothing in this engine posts a category-`0x0D`
  letter: `County_ChangeOwner` (`0x004A72FE`) raises groups `0x75`…`0x7E` with it and
  `l2_kingdom::conquest::change_owner` leaves the nine letters to a caller that does not
  exist. Film, window and voice are built and tested with a posted record.
* **The fast-media ending layout** and the CD's `smk_high` films.
* **The sync latch, the end-of-season skip and `Net_LeaveGame`'s skip.** The second cannot
  arise here: only the top screen is stepped, so a film pauses our turn — where the
  original's turn runs on under a capture film.
* **Escape quitting the front end** (`App_WndProc`).
* **The third battle mode's table.**

## Verified, inferred, and open

**Verified**: signature, geometry, frame counts, frame rates, audio configuration and
footprint of all 45 files; the size invariant; the absence of ring frames and keyframes;
every film's decoded pixels, palettes and samples against an independent decoder; the
duplicate `Axemen`/`AXMEN` pair; the ten imported ordinals and their call sites; the playback
loop; all seven `Smk_Play` callers, their files, positions and returns; `Smk_Skip`'s three
callers; the start-up chain; **that nothing in the original throttles the frame loop, that
`Smk_PlayLoop`'s body is under `SmackWait`, that `_SmackWait@4`'s only import call is
`timeGetTime`, and that every film's track is `frames × period` long to within a
millisecond**.

**Inferred**: the meaning of `SmackOpen`'s `0x2000` / `0x400` bits (RAD's SDK constants);
that a hard-disk install takes the slow-media ending layout (no `sierra.ini`, so
`g_fastMedia` stays 0); `FUN_004B1867` clearing the back buffer before the front-end films;
`FUN_004B1310`'s dimming reading 6-bit palette values.

**Open:**

* **Whether `smackw32` slews `SmackWait`'s deadline to the sound buffer.** It reads
  `timeGetTime`; `_TimerFunc@20` (a `timeSetEvent` callback the DirectSound path installs)
  reads it too. Undecidable from the call sites, and moot here — see *What paces a film*.
* **Y-scaling.** libsmacker calls flag `0x02` Y-double and `0x04` interlace; FFmpeg names
  them the other way round. Both make the picture twice as tall, which is certain from
  `Credits.smk` (640 × 240 on a 640 × 480 screen). Whether the second row of each pair is a
  copy or black is not in the file; ours copies. **One captured frame of the running intro
  settles it.**
* `0x004527A6(x, y, 0x14, 10, 1)` marks a dirty region in 16-pixel units — 320 × 160, smaller
  than any film. The unit or the argument meaning is not understood.
* Seeking (`Smk_OnPaint`'s `SmackGoto`). No file flags a keyframe, so a re-seek re-decodes
  from frame 0; ours never seeks, because nothing here repaints a window from outside.
* **Pixels against `smackw32` itself.** Correct bitstreams are not a correct presentation.

## Sources

* [libsmacker](https://libsmacker.sourceforge.net/) — format reference and licence
  (LGPL-2.1 since January 2020). Consulted for the format description only.
* [`smk` crate](https://crates.io/crates/smk) — run as a black-box oracle; not read, not linked.
* [FFmpeg `libavformat/smacker.c`](https://github.com/FFmpeg/FFmpeg/blob/master/libavformat/smacker.c)
* [`mewspring/smk`](https://github.com/mewspring/smk) — Unlicense, header parsing only
