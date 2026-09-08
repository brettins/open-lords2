# Decisions and corrections

Append-only. The corrections matter as much as the decisions — they stop us re-deriving
conclusions already tried and found wrong.

## Decisions

**D1 — Reimplement the engine; don't build a mod framework on the original.**
A hook-based framework would have taken weeks rather than years, but it is permanently
bounded by the 1996 binary: 256 colours, DirectDraw, fixed resolution. The goal is an
OpenXcom-style moddable engine, so the ceiling matters more than the speed.

**D2 — Incremental replacement, not a big-bang rewrite.**
Each subsystem is reimplemented behind a seam and verified against the original, so
something runs and is verified at every step. This is unusually workable here: no ASLR
plus ~955 KB of game state in fixed `.data` globals means our code can read the original's
live state directly. We don't need to replicate the whole world model before testing one
piece of it.

**D3 — Rust for formats and engine; MSVC C++ only if in-process hooks are needed.**
Binary parsing is Rust at its best. A hook DLL living inside a 32-bit 1996 process is the
one place C++ is less friction.

**D4 — `winit` + `pixels`; not SDL, and definitely not a game engine.**
This is a software-rendered 8-bit palettized 640×480 game. Its entire renderer is a byte
array plus a blitter — roughly 300 lines. We must own a plain `Vec<u8>` framebuffer,
because the endgame is diffing our output against the original's pixel for pixel. An
engine that hides the framebuffer behind a render graph actively fights that requirement.
OpenXcom chose SDL in 2010, when the pure-Rust option didn't exist; that's not evidence
for SDL today.

**D5 — MIT licence; GPL-3 prior art is reference-only.**
Formats are facts and not copyrightable, so their documentation is usable. Their code is
not. See `CLAUDE.md`.

**D5a — OPEN, and not ours to settle: Smacker decoding forces an LGPL choice.**
D5 says "MIT" without qualification. That may not survive contact with the 45 `.smk`
videos.

There is **no permissively licensed Smacker decoder**. `libsmacker` has been LGPL-2.1
since January 2020; the `smk` Rust crate is a declared port of it and is LGPL-2.1 too;
FFmpeg's decoder is likewise unavailable to us on permissive terms. The only public-domain
implementation parses headers and has no codec at all. Transcoding at install time is not
an escape hatch, because transcoding needs a decoder.

**Corrected: transcoding *is* an escape hatch, and probably the right one.**
An earlier revision here said transcoding is no escape because it needs a decoder anyway.
True, but irrelevant — the distinction is **linking versus invoking**. Copyleft obligations
attach to a decoder linked into our binary. They do not attach when the *user* runs a
separate program on their own machine. So a one-time install step that shells out to an
external FFmpeg leaves our binary permissive, and the transcoded files stay in the user's
own directory, which also keeps us clear of the game's copyright.

That yields four options, not three:

1. **Transcode at install via external FFmpeg.** Our binary stays MIT. Costs an install
   step and a dependency on a tool the user supplies.
2. Link an LGPL decoder — workable, but adds notice obligations and, for the one candidate
   crate, an unmaintained dependency with a bug to patch around.
3. Write a Smacker decoder from the format description. A project in itself.
4. Ship without in-game video.

Option 1 has an especially clean variant: the videos are 8-bit palettized at 12 fps and
small resolutions, and our engine already works in palette indices. Transcoding to a
trivial palettized format of our own means **no third-party decoder at runtime at all** —
only our code. The install-time tool does the hard part once, outside our licence boundary.

**Still the user's call**, because it trades a licence constraint for an install step.

If we do adopt one, put it behind our own trait in a leaf crate so swapping it later is a
one-crate change. Note also that the `smk` crate is unmaintained (single release) and has a
real bug — one shipped video dies on a spurious overlap guard — so adopting it means
carrying a patch.

**D6 — Node prototypes are disposable.**
Node answered "can we read this data at all?" quickly. Rust is the implementation. The
Node decoders survive only while they're useful as a porting check.

**D7 — The Node decoders and the differential harness are retired.**
They were built to validate the Node → Rust port, and they did that: 21,344 frames
byte-identical, plus a mutation test proving the harness could actually fail. Once
Rust gained isometric support the two implementations diverged on 2,534 lines — not
a bug, just Rust outgrowing the prototype. Keeping a prototype that silently
disagrees with the real implementation is a liability, and re-porting isometric
decoding into throwaway JavaScript would have added no confidence: two
implementations by the same author on the same day are correlated, so agreement was
always weaker evidence than it sounded. The end-offset invariant, which is a
property of the data rather than of our code, is the stronger check and it lives in
the Rust corpus test.

**D8 — The oracle is driven by code injection, not by synthetic input.**
Phase 5 originally assumed we could drive the original game through its UI and compare
what it did. That is blocked, and the block is empirical rather than theoretical:

- Synthetic input needs window focus, and `SetForegroundWindow` fails silently from a
  background process. Posting `WM_KEYDOWN` directly does not help — verified by minimising
  a window so it could not be focused, then posting keys: no events arrived.
- A fullscreen DirectDraw game generally captures as pure black, so the screen cannot be
  read either.
- Every process spawned to *attempt* input risks stealing focus from the game, which both
  blocks input and, during startup, crashes it outright. An agent tasked with visual
  comparison got the game to its start screen and then stalled; the game's own
  `status.txt` ended `Minimizing. / Paused. / Not active.`

What does work, and needs no focus at all: **reading the game's memory from another
process** (`tools/probe.ps1`, verified against a live `Lords2.exe`), and **running our own
code inside the process** via a `ddraw.dll` proxy. The game imports exactly one function
from DirectDraw, so the proxy is small.

This makes the proxy loader more important than it looked, not less: it sidesteps input
entirely. Differential testing should call the original's functions and read its state
directly rather than pantomiming a player.

**Corollary on the read-only rule:** a proxy DLL has to sit beside the executable, but
`CLAUDE.md` rule 2 says the installs are read-only. Resolve it with a sibling directory of
**hard links** to the original files plus our own DLL — no copying, no space, and the
install stays untouched.

## Corrections

**C1 — "The PL8 format is fully decoded."** Claimed after one sprite rendered correctly.
The corpus check immediately found 153 of 291 files failing. *A single working example
proves nothing about a format.*

**C2 — "All 23 `raw:2` files fail, so sub-mode 2 changes the raw layout."** Wrong. The
count of `raw:2` files and the count of failures were both 23, and that coincidence was
over-read. 13 of those files decode correctly.

**C3 — "The three blitters are the three storage modes, so mode 2 is raw with a row
stride."** Wrong, and retracted. `Clip_Horizontal` sets that selector from clipping
geometry; the blitters are unclipped / clipped-left / clipped-right variants of the same
copy. Three functions happened to match three modes and a plausible story assembled
itself. *Decompiler output invites exactly this error.*

**C4 — Frame count reported as 16,638.** That was the Node checker counting frames inside
files that later failed validation. The figure quoted as the correction, 16,435,
does not reproduce either; an audit could recover neither number. The count that
matters now is 21,344 frames across all 291 files, which does reproduce.

**C5 — Assumed no prior art existed.** The single largest waste of effort so far. The PL8
format — including the per-frame tile-type byte that governs the "unknown" storage mode 2 —
is documented in the GPL-3 `pl8image` package, and `L2_maps.dat`'s slot structure in
OpenLotR2's docs. Both were found by a five-minute search *after* days of equivalent work
had been commissioned. **Search for prior art before reverse-engineering anything.**

**C6 — "Header byte 1 is a sub-mode."** It is the **map zoom level**: 0 → 58×30
tiles, 1 → 26×14, 2 → 10×6, correlating perfectly across all 32 isometric files.
Related: reading bytes 0x00–0x01 as one `u16` is wrong, since byte 1 varies
independently of byte 0.

**C7 — Treating the storage family byte as the encoding.** It is not. The per-frame
`shape` byte at record offset 0x0C decides, which is why no single "mode 2 codec"
ever fit: one isometric file holds raw rectangles and diamonds side by side.

**C8 — "Search for prior art first" needs a second half: verify it.**
C5 was right that not searching cost real effort. But prior art is a *lead*, not an
authority. OpenLotR2's `.skr` documentation is wrong in three places, each caught only by
checking bytes: the battlefield layer is 80×80 rather than 64×64, a text record is 183
bytes rather than 182, and the layers do not begin where it says — an unexplained 328-byte
pad precedes them. A decoder built on the document alone would have desynchronised
immediately.

The rule is therefore: **find prior art, then validate every claim against the data before
building on it.** Its real value is telling you *what to look for*, which is worth a great
deal even when the specifics are wrong.

**C9 — `git add -A` swept agents' files twice before the rule stuck.**
Commit `38278d0` took an agent's in-progress `ghidra_scripts_skr/`, `tools/maps/dump.ps1`
and a whole findings document under a commit message about something else. The explicit-
path rule in `docs/agents.md` was written immediately after and did hold for the next
commit — but the lesson is that a rule written *after* the damage still leaves misleading
history behind.

**C10 — The printed manual is not an oracle either.**
Seven of its rankings have been asserted as tests against tables read out of the
decompiler, and they were the strongest validation available for numbers of that kind. But
the kingdom work found the manual **wrong twice**: it says up to 5 sacks per field where
the constant is 10, and it advises keeping a third of your fields fallow where the rule is
one fallow per *two* grain fields. In both cases long-standing player measurements were
right and the manual wrong.

So manual agreement remains good evidence and stays in the tests, but it is corroboration,
not proof. Where the manual and the binary disagree, **the binary is what the game does**.

**C11 — All kingdom rules live in the executable, not in data files.**
`TROOPS*.ENG` set an expectation that rules would be reachable as data. They are not: every
economic constant — tax, happiness, rations, births, deaths, yields, wages — is in
`Lords2.exe`, clustered in two regions around `0x004D6300` and `0x004D8900`. The only
non-obvious data file, `castles.dat`, is working state created zeroed at new-game, not
rules.

Two consequences. Modding the *original* means patching the binary, which is why an open
engine is worth building at all. And our own engine has to carry these constants as its
own ruleset data — which is exactly what `crates/l2-mods` is for, so they become editable
for the first time.

**C12 — "Terrain cost is charged by deferral, not by weighting."** Wrong, and it reached
shipped code. `docs/battle.md` §8.3 said it, `crates/l2-sim/src/pathfind.rs` repeated it in
a module doc comment, and the implementation weighted nothing — it recorded plain hop count.
The decompiled `Path_Search` does **both**: `cost[nb] = stepCost[nb] + (cost[cur] + 1)`, and
separately re-queues an expensive cell until it has been popped `stepCost` extra times.

Two things made this survive longer than it should have. The claim was load-bearing enough
to be written into a module doc as the file's headline fact, which made it feel settled. And
the test guarding it, `expensive_ground_is_deferred_rather_than_weighted`, asserted only that
the path still ran through the expensive gap — which is true under either reading, because
the gap was the only way through. **A test that passes before and after the change is not
testing the thing its name claims.** It has been replaced by a differential one that measures
the recorded cost with and without the surcharge, and reads 0 under the old code.

Also corrected at the same time, from the same function: 998 and 999 are different blocked
values (a friendly figure versus terrain) and the destination treats them differently, and
the cost field is never relaxed, so the first cost written to a cell stands.

**C13 — An agent's summary is a lead, not a finding.** C12 arrived as a line in an agent's
report. The right response was neither to take it (it contradicted tested code) nor to
dismiss it (it was specific and gave addresses), but to open the raw decompiler output the
agent had left in `tools/battleai/out/path.c` and read the loop. It said something slightly
*stronger* than the summary did — the summary omitted that the cost field is never relaxed.
Extending C8 to our own agents: **the artefact an agent leaves behind is evidence; its prose
is a claim.**

**C14 — The oracle splits into two tiers, and only one of them is cheap.**
Reading the binary is now a routine check rather than an aspiration — `tools/oracle/`
verifies `l2-sim`'s battle tables and `docs/kingdom.md`'s economy tables straight out of
`Lords2.exe`. But the attempt to extend it to the constants `Rules_InitConstants` writes
found a hard line through the middle of the idea.

**Tier one: stored data.** Anything initialised in the image can be read from the file with
no process, no window and no focus. `-Source Both` proved the file is authoritative here:
`g_troopBattleStats`, `g_missileStats` and `g_meleeAttackTable` are byte-identical on disk
and in a live process. This tier is free, repeatable, and needs nothing from the user.

**Tier two: anything written at runtime.** These addresses are uninitialised `.data` — the
file has no bytes for them at all — so the file read is not merely unhelpful but
*confidently wrong*, returning zero for every one. And they cannot be sampled from a
scripted launch, because of D8 seen from a new angle: started from a script, `Lords2`
survives about **2.6 seconds** and its own `status.txt` ends `Window moved. / Not active.`
The launching process holds the foreground, the game sees itself deactivated and exits —
before `Rules_InitConstants` has run. Every constant reads 0, which is evidence about focus
and not about the constants.

So `tools/oracle/runtime.ps1` refuses to launch by default and tells the user to start the
game themselves; it attaches to a running instance. Ten constants remain **unverified**,
including `g_grainMaxSacksPerField`, which is C10's evidence that the printed manual is
wrong. They are not in doubt, but they are not confirmed either, and the difference should
stay visible.

The lesson generalises past this project: *"read it from the binary" is two different
techniques with two different costs*, and conflating them makes the expensive one look done.

**C15 — D8's focus claim is overturned, and the ten runtime constants are verified.**
D8 said synthetic input is blocked because "`SetForegroundWindow` fails silently from a
background process". That is true *on its own*, and it is not the whole story: Windows only
lets the process that already owns the foreground give it away, and the documented way round
that is `AttachThreadInput`. Joining our input queue to the current foreground thread's makes
us count as that owner for as long as we stay attached, at which point the foreground can be
handed over and the attachment dropped. `tools/oracle/runtime.ps1` does exactly this and the
game now survives being launched from a script.

That immediately paid for itself. All ten constants `Rules_InitConstants` writes are now
**verified against a live process** — every one reads zero from the file, so nothing but a
running game could have confirmed them:

| constant | value | why it matters |
|---|---:|---|
| `g_aiAggressionThreshold` | 5 | every field handler attacks above this |
| `g_aiSortieThreshold` | 260 | every siege defender sorties above this |
| `g_moatFillSteps` | 15 | |
| `g_grainMaxSacksPerField` | **10** | **the printed manual says 5** |
| `g_grainYieldPerSack` | 12 | |
| `g_foodPerHead` / `g_foodPerSack` | 10 / 6 | |
| `g_dairyPerHead` | 5 | |
| `g_grainLabourDivisor` / `…Adv` | 2 / 5 | |

`g_grainMaxSacksPerField = 10` is **C10 confirmed at the source**. That correction rested on
long-standing player measurement contradicting the manual; it now rests on the binary.

What this does *not* establish is that driving the original's UI is viable. Focus was one of
D8's three blockers; a fullscreen DirectDraw game still captures as black, and a game that
quits on deactivation is still hostile to automation. But the reason to prefer injection was
partly that input was impossible, and it is now merely difficult — so the tier-two oracle in
C14 is no longer closed, and reaching a running battle is worth another attempt.

The general lesson: **"it fails" and "it fails the way I called it" are different claims.**
D8 recorded a real observation and generalised it one step too far, and that extra step shut
a door for weeks.

## Open questions

- `WEATHER_JITTER_BOUND` in `crates/l2-kingdom` is **invented**. `docs/kingdom.md` §7.3
  gives the weather jitter as `random/8` with no stated range, which is not implementable —
  the constant is the one number in that crate with no evidence behind it, and it is marked
  as such at its definition. It needs tracing before any weather behaviour is trusted.
- Whether `Path_Search`'s visit counters are fully cleared between searches. The clear is
  `FUN_004b3e51(&g_pathVisitCount, 0x1000)` against a 6,400-cell grid; if that count is
  bytes rather than dwords, the last 2,304 cells keep stale counts from the previous search
  and deferral behaves differently on the bottom third of the map. Not resolvable from the
  decompilation alone — it needs the callee's signature.
- The four map planes whose meaning is inferred rather than proven (graphics bank,
  descriptor index, multi-tile object part), and the exact tile → lattice mapping,
  whose best affine fit reaches only 72%.
- 23 files that use supported encodings but fail the end-offset invariant, pinned in
  `KNOWN_FAILING`.
- `Font_c2.pl8` declares RLE but its frames occupy exactly `width × height`.
- `Title.pl8` decodes with correct geometry but no shipped palette colours it.
- The type-4 apex pair, where the stored data and the shipped blitter disagree.
- PL8 header fields at 0x04, 0x06, 0x07.
