# Audio (`.wav`)

Status: **771 / 771 files validated as standard PCM RIFF/WAVE.** Nothing non-standard,
nothing compressed, nothing custom. The older DOS install validates too, 770 / 770.

There is no Lords-specific format here — this is the one asset type Impressions did not
invent. The work is therefore not decoding but deciding what to depend on.

Analysis script:

```bash
node tools/media/wavinfo.js "F:/games/Lords of the Realm II"            # summary + validation
node tools/media/wavinfo.js "F:/games/Lords of the Realm II" --csv      # per-file fields
node tools/media/wavinfo.js "F:/games/LORDS2" --recurse                 # the DOS install
```

It walks the chunk list by seeking, so a 168 MB file costs the same as a 700-byte one.

## The corpus (verified)

771 files, **415,471,756 bytes**, 124.6 minutes. Every file is `RIFF`/`WAVE` with
`wFormatTag = 1` (uncompressed PCM).

| Sample rate | Bits | Channels | Files |
|---|---|---|---|
| 11,025 Hz | 8 | mono | **737** |
| 11,025 Hz | 8 | stereo | **31** |
| 44,100 Hz | 16 | stereo | **2** (`PUMKIN.WAV`, `PUMKIN2.WAV`) |
| 44,100 Hz | 8 | mono | **1** (`Bp180_4.wav`) |

The 31 8-bit stereo files are the long music and ambience beds — `Battle1`–`BATTLE5`,
`Scroll1`–`Scroll5` and `Setup`–`SETUP3`, 43 to 172 seconds each — together with `Fire`,
the seven `Ff_*` fanfares, and ten short stereo effects (`Army`, `Fallow`, `Iron`,
`Merchant`, `Moo_2`, `Rioters`, `Stonecut`, `Supply`, `Wheat`, `Woodcut`, all under four
seconds). Stereo does not mean music here. Everything else is mono 11 kHz speech and
effects.

`Bp180_4.wav` is the single oddity in the speech corpus: 44.1 kHz where its three
siblings `Bp180_1`–`Bp180_3` are 11 kHz. It is valid, just resampled differently — a
1995 mastering slip, not a format variant.

### Checks that passed on all 771

* `RIFF` size field == filesize − 8.
* The chunk walk, honouring RIFF's pad-to-even rule, lands **exactly** on EOF.
* `blockAlign == channels × bits/8` and `byteRate == sampleRate × blockAlign`.
* `data` length is a whole number of blocks.

### The layout is completely uniform

Every one of the 771 files puts the `fmt ` chunk header at offset 12 with a **16-byte**
body, the `data` chunk header at offset 36, and **sample data at byte 44**. No
`WAVEFORMATEXTENSIBLE`, no `fact` chunk, no 18-byte `fmt `.

62 files carry chunks *after* the data: 53 `cue ` + `LIST`, 5 `LIST` + `cue ` + `LIST`,
4 `LIST` alone. All of it is authoring residue, not game data — `LIST/INFO` holding
`ISFT "Sound Forge 4.0"`, `ICRD` dates from 1995–1997 and `IENG` engineer credits
("Keith Zizza", "Steven Serafino"), and `cue ` + `LIST/adtl` region labels reading
"Record Take 001". A decoder must skip them; nothing needs to read them.

**8-bit WAV samples are unsigned** (0–255, silence at 128); 16-bit are signed. That is
the standard rule, and 769 of our 771 files are 8-bit, so it is the one place a naive
reader gets loud noise instead of speech.

### Two files are a third of a gigabyte

`PUMKIN.WAV` and `PUMKIN2.WAV` are **byte-identical** (MD5 `2765C8AA…`), 167,943,896
bytes each: 44.1 kHz 16-bit stereo, **15 minutes 52 seconds**, dated 8 November 1996.
Together they are 336 MB — **81% of the entire WAV footprint**. Both are referenced by
`Lords2.exe` (the string table reads `… swor_p2.wav pumkin.wav pumkin2.wav …`), so they
are real assets rather than build leftovers, and neither exists in the DOS install. What
they contain has not been established here — the name and the November date suggest a
Halloween easter egg.

Excluding the pair, the corpus is 769 files, 79,583,964 bytes, 92.9 minutes — which is
the number to plan around for anything that wants audio resident in memory.

### The exe names more sounds than ship

`Lords2.exe` contains **848** distinct `*.wav` names. **95 are referenced but absent**,
mostly unrecorded speech slots (`s100_01` … `s299_01`, `arch_f1`, `swor_f1`), plus the
placeholder `null.wav`. **18 files ship but are never referenced** (`arch_e5`, `bathit1`,
`boilguy1`, `boilguy2`, `boilwood`, `bp100_3`, `dest_fld`, `ff_capt2`, `ff_msg1`,
`ff_win`, `knig_e3`, `s017_02`, `s017_03`, `s071_07`, `s072_01`, `s072_02`, `s115_02`,
`supply`). Same conclusion as for the videos: **our loader must tolerate a missing file
without failing**, because the original does.

### The DOS install is the same corpus

`F:\games\LORDS2\WAV\` plus three files in the root: 770 files, 74,506,144 bytes, 89.1
minutes, 0 problems, the same configurations (739 mono / 30 stereo / 1 at 44.1 kHz) and
no `PUMKIN`. Useful as a second corpus for the same validator.

## What the original engine does

`Lords2.exe` imports `WINMM.dll` and `DSOUND.dll`. Audio file parsing is **not**
hand-rolled: `0x004B95BA` walks the RIFF with `mmioSeek` + `mmioDescend` (`MMCKINFO`,
`ckid = 'data'`, `MMIO_FINDCHUNK`) and logs `ERR:WAV start read data` on failure. So the
engine does a proper chunk descent rather than assuming byte 44 — even though, in this
corpus, byte 44 would always have worked.

Playback is DirectSound. There are two paths, distinguished by the log strings:
`0x00427990` logs `OK :Load and play streamed WAV file %s` — streaming, as the 168 MB
`PUMKIN` files demand — alongside a load-it-all path for short effects.

## Recommendation

### Parsing: do it ourselves, keep `hound` as the oracle

`crates/l2-formats` is deliberately dependency-free ("a format crate should parse bytes
and nothing else"). Nothing in this corpus justifies breaking that: 16-byte `fmt ` at 12,
`data` at 36, PCM, four configurations, and a chunk walk that must skip trailing `cue `
and `LIST`. That is roughly 60 lines including the unsigned-8-bit conversion, and it
comes with the same self-validating checks the script above already runs.

Take **`hound` 3.5.1** as a **dev-dependency** instead, and cross-check our reader against
it over a whole install, exactly as the PL8 corpus test does.

* Licence **Apache-2.0** — permissive, compatible with our MIT, no copyleft. It does add
  a NOTICE/patent-grant obligation that MIT alone does not have.
* **Zero dependencies**, pure Rust, no build script, nothing native.
* Last release September 2023. Dormant, but the format was frozen in 1991; with
  4.8M recent downloads it is the de-facto standard.
* Verified here: it read **all 771 files, 247,483,967 samples, in 1.27 s**, with its
  declared sample count matching the actual reads for every file. Its 8-bit handling was
  checked directly — raw bytes `[128, 128, …, 127]` come back as `[0, 0, …, -1]`, i.e. it
  subtracts 128 and hands you signed samples.

If the in-house reader ever looks like a liability, promoting `hound` to a real dependency
costs nothing but the Apache-2.0 notice — it is the low-risk fallback, not a bad choice.

### Playback: `rodio` on `cpal`, features trimmed

* `rodio` 0.22.2 — **MIT OR Apache-2.0**, so licence-clean for us.
* `cpal` 0.18.2 — **Apache-2.0**; on Windows it uses **WASAPI** through the pure-Rust
  `windows` bindings. **No external SDK, no C toolchain.** The one exception is the
  opt-in `asio` feature, which downloads Steinberg's SDK and needs LLVM — do not enable it.
* Build with `default-features = false`. Rodio's defaults pull `symphonia` (MPL-2.0),
  `claxon` and `lewton` for MP3/FLAC/Vorbis, none of which this game has. We feed it raw
  PCM we have already parsed.

What it buys: a mixer, device enumeration, resampling to the device rate (our sources are
11,025 Hz, which few devices accept natively), and sink/volume control. What it costs: a
real-time callback thread, device-loss handling, and a dependency tree that is
substantially larger than everything else in this workspace put together. That cost is
for *playback*, and it is unavoidable in some form — it is not a cost of the file format.

**Stream, don't slurp.** `PUMKIN.WAV` alone is 168 MB. Whatever we adopt must decode from
a reader rather than requiring the file in memory; `hound`'s `WavReader` does, our own
reader should, and the original engine had a dedicated streaming path for exactly this.

## Verified, inferred, and open

**Verified**: every claim about the 771 files above — format tag, rates, depths, channel
counts, chunk offsets and the four structural checks, from `tools/media/wavinfo.js`; the
DOS install's 770; the byte-identity of the two `PUMKIN` files; the exe's 848 references
and the two mismatch sets; the engine's `mmio` chunk descent at `0x004B95BA` and its
DirectSound streaming path at `0x00427990`; `hound`'s behaviour on the whole corpus.

**Inferred**: that the 31 stereo files are music/ambience (from names and lengths); that
`PUMKIN` is an easter egg (from the name and the date).

**Open**: what `PUMKIN.WAV` actually contains and where the game plays it; whether any
game logic depends on sample-accurate lengths (nothing suggests it does); how the engine
chooses the streaming path over the resident one.

## Sources

* [`hound`](https://crates.io/crates/hound) — Apache-2.0, zero dependencies
* [`rodio`](https://crates.io/crates/rodio) — MIT OR Apache-2.0
* [`cpal`](https://github.com/RustAudio/cpal) — Apache-2.0; WASAPI default on Windows,
  ASIO opt-in and SDK-dependent
