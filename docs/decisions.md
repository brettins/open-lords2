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

**D9 — A conversion between two crates that may not know about each other gets its own
crate.**
`l2-kingdom` must have no loader, no I/O and no knowledge of a byte layout: it takes plain
data, which is the property that lets a lockstep peer, a replay, a mod and a test all reach
the same state by handing it values. `l2-formats` must stay dependency-free and parse
bytes; teaching it about counties would point the dependency arrow backwards. Importing the
shipped scenario needs both vocabularies, so it belongs to neither.

`crates/l2-scenario` is the seam, and it is the only crate in the workspace allowed to know
a file format *and* a simulation. `Scenario` is plain data in between, with two
constructors that are two different products: `kingdom()` is load-a-save, `starting_kingdom()`
is new-game-on-this-map. A save it has misread is refused — a bad owner, weather, neighbour,
county count or local player each has its own error — because the arithmetic in
`Save::open` having closed is exactly why a surprise at that level should stop.

The cost is one more crate and a **dev-dependency cycle**: `l2-kingdom`'s tests dev-depend
on `l2-scenario`, which depends on `l2-kingdom`. Cargo permits this precisely because a
dev-dependency is not part of the library's own graph, and it is what keeps the
reproduction test where it belongs — next to the rules it is testing — without the library
gaining a dependency it must not have.

**D10 — Our own save format writes through `l2-net`'s canonical encoder, not its own.**
`crates/l2-kingdom/src/save.rs` is the format we write, as against the original's memory
dump that `l2-formats` reads. `docs/netcode.md` §5 and §6 already required a byte-exact
encoding of simulation state — for the tick checksum, the late-join snapshot and the desync
dump — so a second encoder here would mean a save whose bytes and a checksum whose bytes
could disagree, which is the failure `canonical.rs` exists to prevent. The save's trailer
*is* the state checksum, and a test asserts it. Two of the four determinism rules come free
with the type: `Canonical` has no method that writes a float and none that writes a
`usize`.

The header carries a magic, a version and a **ruleset fingerprint**. An unknown version is
refused rather than reinterpreted. The ruleset is fingerprinted rather than stored, because
`Tables` is what a mod replaces and a save carrying its own copy would silently override
whatever mod set the player has enabled: the rules come from the mod layer, and the save
only checks that they are the same rules.

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

**C16 — C14's second tier was mostly imaginary. Read the code, not the data.**
C14 divided the oracle into "stored data, free to read from the file" and "runtime-written
data, which needs a live process". The second half was wrong, and the error was a failure of
imagination rather than of fact.

The reasoning went: `Rules_InitConstants` writes into uninitialised `.data`, the file has no
bytes at those addresses, therefore only a running process can supply the values. Every step
is true and the conclusion does not follow. **The values are not in `.data`, but they are in
`.text`** — the function writes them with `MOV dword ptr [addr], imm32`, and the immediate
sits in the instruction stream:

```text
C7 05 <addr:u32> <imm:u32>      MOV dword ptr [addr], imm32
```

`tools/oracle/initconsts.ps1` disassembles those writes. It recovers all ten constants with
**the same values the live read returned**, with no process, no window, no focus and nothing
a screen lock can spoil — and it finds **five more** that nobody had named
(`0x00553004`=25, `0x00553538`=10, `0x00553218`=3, `0x0052AFC4`=500, `0x00568950`=200),
because a memory read only answers about addresses you already knew to ask about, while the
function tells you everything it writes.

So the live-memory path is not the way to get constants, and `tools/oracle/runtime.ps1` is
kept only as the cross-check that proved the static reading correct. The focus work in C15
stands as a fact about Windows, and its conclusion — "reaching a running battle is worth
another attempt" — is now much weaker: static reading reaches further than assumed, and
running the game should be the last resort rather than the second.

**The general form, and the one worth remembering:** when a value is absent from the data,
look at the code that produces it. "The bytes aren't there" is a statement about where you
looked. This is C3's failure inverted — there, a plausible story was fitted to the
decompiler's output; here, a true observation about the data was allowed to settle a
question the code answers better.

**C17 — "Not resolvable from the decompilation alone" was C16 again, and I wrote it.**
An open question here read: *whether `Path_Search`'s visit counters are fully cleared between
searches … not resolvable from the decompilation alone — it needs the callee's signature.*

The callee's **body** was on disk, in `tools/oracle/decomp/004b0000.c`. `FUN_004B3E51` counts
**bytes**: its tail loop stores one `undefined1` and decrements by one. The call site is
`push 0x1000; push 0x004F6470`, so it clears **4,096 bytes of a 6,400-byte array** — one
counter per cell. Three things fix the extent: the sibling call passes `0x3200` for
`g_pathCost`, exactly 6,400 × `u16`; `docs/battle.md` already recorded the counters as one
byte per cell; and `0x004F6470 + 6400` lands precisely on the next global `Path_Search` uses.

**So cells 4,096 and above — rows 51 to 79, the bottom 36% of the battlefield — begin each
search holding the previous search's counts.** `crates/l2-sim` zeroed all 6,400 and therefore
diverged from the original on every castle map, where step costs are non-zero. Now
reproduced: `Scratch` carries the counters between searches and clears only
`CLEARED_COUNTERS`, with a test that fails if the array is fully cleared.

Two things worth keeping. The question said what it *needed* rather than what had been
*tried*, and "needs the callee's signature" was false — it needed the callee's body, which
cost one `rg`. And this is C16 restated: the answer was in the code, and I looked at the
data. **A claim about what is unknowable should name the technique that was tried and
failed.** Ours named a technique nobody had attempted.

**C18 — C9's rule was necessary and not sufficient: `git commit` commits the index.**
C9 said never `git add -A` while agents are running, and stage explicit paths instead. An
agent followed that exactly and still swept another agent's staged file renames into its
commit, because **`git add <paths>` controls what you add and `git commit` commits the whole
index** — including whatever somebody else staged before you got there.

The form that holds names the paths on the commit: `git commit -F msg -- <paths>`, which
bypasses the index for those paths and takes exactly what you name. `docs/agents.md` now
says so.

The general shape is worth more than the git detail: **a rule aimed at the tool you noticed
can leave the actual mechanism untouched.** C9 blamed `-A` because `-A` was what did the
damage that day; the real hazard was a shared index, and `-A` was only the loudest way to
walk into it.

**C19 — Some rules have no address at all, and the oracle now reads control flow.**
Every oracle check so far has read a *table*: an address, a stride, a count. The last five
mod-controllable rules had no table to read, which is exactly why they were the leftovers —
the AI's four tax ladders are `if`/`else if` chains in `AI_SetTaxRates` (`0x0049D638`), and
the ale bounds and efficiency ceiling are immediates inside their own functions.

`tools/oracle/kingdom.ps1` now recovers them from the instruction stream: 20
`CMP EAX, imm8` thresholds interleaved with 24 `MOV byte ptr [...], imm8` rate stores, which
are the four ladders exactly. This is C16 generalised — there, a constant's value lived in
the `MOV` that wrote it rather than in the `.data` it wrote to; here, an entire *rule* lives
in branch structure rather than in data. **"Where is the table?" is the wrong first question
when the answer may be "there isn't one".**

**C20 — C12 had a second instance, in the test named after the save it never opened.**
`crates/l2-kingdom/tests/reproduction.rs` was headed *"The reproduction from the shipped
save"*, declared `const OWNED: usize = 4`, handed counties 1–4 to the human realm, and
asserted numbers quoted out of `docs/kingdom.md` against rules built from the same
document. It could not fail, and its scenario was invented: the file holds **five owned
counties, one for each of realms 1 to 5**, at indices 1, 4, 8, 11 and 13, with nine
unowned and the person holding county 8 alone. The realm records say the same thing from
the other side — `+0x29` is 1 apiece. `docs/kingdom.md` §9 carried the same wrong count,
which is where the test got it.

It now imports the save through `crates/l2-scenario`, rewinds it one season using the
file's own `popLast` and `happinessLast`, runs `Season_Advance`, and compares twenty-six
stored fields across all fourteen counties. **No rule had to change.**

Two things fell out of doing it properly.

**§4.3's open question is answered.** It said the food fields reproduced for the unowned
counties and not the owned ones, and that it *"did not untangle which write survives"*.
County 1 unties it: it stores `dHapRation = −2` and `shownRation = +1`, which is the two
`Ration_Apply` calls disagreeing — the display copy is taken while happiness is computed,
so the *first* call fed it at Normal and the *second*, next season's preview, says Half.
Feeding it at Normal on an all-grain split costs `DivCeil(417 − 74×5, 6) = 8` sacks and
the county holds none, so **the first call debits the store and the preview does not**.
Read that way every county reproduces. §4.3 was also wrong about the number it quoted: the
owned counties store `+0x17C = 0`, not 3 — the 3 is `rationAchieved`, one field along.

**A save can be inverted, and that turns a fudge into a measurement.** The file records no
opening food store, so the rewind starts county 1 with the grain it *finished* on and it
starves where the real one did not. Rather than hand it a number to make the test pass, the
test searches for every opening `(herd, grain)` that both feeds the county at the level
`shownRation` records and leaves the stores the file holds — and asserts there is exactly
**one**. Nine unowned counties opened on 73 head; county 1 on 8 sacks. Uniqueness is what
makes those numbers evidence: under different rules the solution moves or vanishes.

The general form, and the reason this is worth a numbered entry rather than a bug fix:
**C12's failure mode is not rare and it is not obvious from the test's name — the name is
what hides it.** Both instances were caught by asking what the body actually reads, and in
both cases the answer was "nothing that could disagree with it".

**C21 — An entire layer went un-analysed because no phase was named after it.**
The roadmap has eight phases: foundations, formats, the Rust base, the renderer, the battle
sandbox, the oracle, the kingdom, netcode, modding. Every one of them got the treatment this
project is careful about — decompile, verify against the binary, corpus-test, mark [V]
versus [I]. The result is 899 tests and an economy checked table by table.

**None of them is the user interface**, and so nobody ever decompiled a screen.
`docs/symbols.json` reached 394 named symbols across battle, battleai, kingdom, units,
sprite, map, fileio and text with **no `ui` section at all** — 2,452 functions decompiled and
not one screen among them.

The bill arrived in a single sentence. Shown one screenshot of the campaign map, the user
said *"the map looks nothing like the original game… this looks like a minimap maybe?"*
Reading `Map_RenderIso` took minutes and proved them right: the original walks a **window**
into the lattice and **never shows the whole 64-column map at any zoom.** Ours showed all of
it. See `docs/screens.md`, which is the whole screen written up.

*(One thing that first reading got wrong, corrected while fixing it: it said "three zooms —
tile width 60/28/12, visible columns 8/17/40", taken from `Map_SetZoom`'s three cases. The
campaign screen has **two**. `g_mapZoom` has three writers in the whole binary and none of
them can make it 1; no shipped PL8 holds 26 × 14 map tiles; and `Map_DrawTile` has no zoom-1
branch. Reading one function's cases as the set of reachable states is C3's shape again — a
tidy count believed before its callers were checked.)*

Nobody was careless. The agent that built it had verified map *data*, a blank where the
screen layout should be, and a task that said "build a campaign map screen" rather than
"decompile the campaign map screen, then build it". It filled the blank reasonably and
wrote down that it had — the giveaway is sitting in its own comment, *"fits on one 640 × 480
screen with no scrolling viewport to build."*

Three things worth keeping:

**A plan's categories decide what gets rigour.** Work outside every named phase gets none,
and its absence is invisible precisely because nothing tracks it. The roadmap did not have a
gap labelled "unknown"; it had no label at all.

**"Verified" attaches to a layer, not to a screen.** Every factual claim under that map was
sound — the tile→lattice mapping is checked 4,096/4,096 against a live process. The *data*
was verified and the *presentation* was invented, and a screenshot showing both makes them
look equally finished.

**The cheapest oracle in this project turned out to be a person glancing at a picture.** It
cost one sentence and beat 899 tests, because none of those tests could ask "is this what
the game looks like". Show screens early, to someone who knows the game.

**C22 — "What does the painter paint?" is not "what does the player see?", and only the
caller answers the second.**

The village screen was built and documented as a **full screen** — a page that owns the
framebuffer. It is not. It is a picture blitted into a rectangle over the campaign map,
with the menu bar, the county sidebar and a band of map showing around it.

The false inference, in one line: *"`Village_Draw` is its own case in `Screen_Draw`, and it
calls neither the sidebar nor `CountyStrip_Draw`, therefore it is a full screen."* Every
clause of that is true and the conclusion does not follow. **This engine has no screen clear
anywhere.** Not redrawing the sidebar does not mean the sidebar is not there; it means the
sidebar is still there from the previous frame. The evidence was in the same function all
along — `Village_Draw` blits at (64, 64) and never touches a pixel above or beside it.

It was overturned by a player. He opened a game, clicked the town square, and said *"it
opened up a dialog or however you describe still being able to see the map around it and the
rest of the screen."* His first instinct had been "dialogue"; the decompiled reasoning
talked us out of it, and he was right.

Three things worth keeping.

**Read the site that sets `g_screenId`, not only the painter it dispatches to.** `g_screenId
= 2` appears **exactly once in the whole binary**, in `Map_Click` (`0x0043CE1A`), and that
one site settles the question on its own:

```c
if (county.townTile != 0) {
    g_selectedCounty = clickedCounty;
    Map_CentreOnTile(county.townTile);      /* recentre the campaign map */
    FUN_004050C0();                         /* ... and repaint one frame of it */
}
g_screenId = 2;
Village_Draw(1);
```

**The game centres the map on the town immediately before opening the village.** That is
only worth doing if the map remains visible behind it. One grep, one hit, and it needed no
knowledge of the blit at all.

*(`FUN_004050C0` turns out to be the second half of the same proof: it steps an animation
counter and calls `FUN_004CFB08`, which calls **`Map_DrawFrame`**. The village's own painter
repaints the campaign map and then draws over it.)*

**The absence of the window primitive is not evidence of a page.** This engine has *two*
overlay mechanisms and only one had been characterised:

| | how | who uses it |
|---|---|---|
| a framed window | `Ui_DrawBox` (`0x00409397`) and the `Panels.pl8` 16-pixel kit | the four county panels, the job popup, the merchant, the court |
| a raw blit | one sprite straight to the framebuffer at a fixed origin, no frame, no clear | **the village**, at (64, 64) |

So *"`Village_Draw` contains no `Ui_DrawBox` call"* reads as evidence **for** a full screen
and actually means only that the village is the other kind. A reader who knows one mechanism
and not the other will get this wrong every time, which is why both are now written down in
`docs/screens-county.md` §3.1.

**This is C21's shape at one screen's scale.** There too the *data* was verified and the
*presentation* was invented on top of it; there too a player looking at a picture beat the
test suite. The difference is that C21 had a blank where the layout should be, and this had
something worse — a confident paragraph, `[V]`-adjacent, in three documents and a module
header. A wrong answer written down is more expensive than no answer, and it survives longer
because it stops anyone looking.

**C23 — "The shipped save" was three words that hid a rolling autosave, and a test
suite that passes identically whether or not it ran.**

Nine tests in `crates/l2-formats/tests/save.rs` asserted one saved game's numbers against
`lastturn.sav` *inside the game install*. That file is the **rolling autosave**: the game
rewrites it every turn a human plays. Ten minutes of play replaced it and all nine went
red at once, with bare assertion diffs that read like a broken reader.

**A clean GOG install ships no saves at all** — verified by diffing a pristine copy of the
install against a played one, where exactly six files differ and five of them are saves.
The file was produced by somebody in an earlier session of this project starting a
campaign. "Shipped" was simply wrong, and the wrong word is what made a volatile file look
permanent: nobody protects a fixture they believe the publisher supplied.

Three separate defects came out of one breakage, and only the first is the obvious one.

**1. A path is not an identity.** Three directories on this machine now hold a
`lastturn.sav` and they are three different games. A fixture is now a *name* plus a
*fingerprint* — `l2_testkit::england_turn1()` reads
`%LORDS2_FIXTURES%\england-turn1.sav`, checks it is that position, and returns one of
three states: **ready**, **absent** (skip, with a reason) or **wrong game** (fail, naming
the fixture). The last two were the same state before, which is exactly why this was not
noticed earlier. There is deliberately no fall back to any install's autosave.

**2. Three of the nine were asserting per-playthrough noise under an invariant's name.**
This was settled empirically, by comparing two independently created England turn-one
saves rather than by argument. The five starting counties are always 1, 4, 8, 11 and 13
and realms 1 to 5 always take one each — but **which realm takes which is rolled per
game**, and so are `g_weatherCounty`, which lord sits behind which realm, and which county
the person ends up on. County 11 falls to realm 3 in both saves, which is chance, and is a
fair warning about how convincingly one playthrough reads as a rule.

The third of those is the interesting one. `the_food_configuration_has_three_shapes_and_county_one_is_alone_in_the_third`
asserted that **county 1** is the odd one out on food. In the second save the odd county is
8 — and county 1 there and county 8 here are both **realm 5's**. *One lord always begins
short of food, and it is always realm 5.* A real piece of scenario design had been pinned
to a coincidence, in three test files and in `docs/kingdom.md` §4.3, and would have gone on
passing against the file it was written from. Two more constants had the same shape:
`reproduction.rs`'s `const DEAD_END: usize = 1` conflated "the map's dead end" with "the
county that starves", which happen to coincide in exactly one save.

Note what the failure told us and what it did not. `the_neighbour_lists_are_symmetric_and_name_only_real_counties`
also went red — on `assert_eq!(real.len(), 14)`. **The invariant its name promises held
perfectly on all eleven saves this machine can reach.** A scenario value wearing an
invariant's name is indistinguishable from a broken invariant until somebody separates
them.

**3. A silent skip is worse than a red test, and the suite was full of them.**
`cargo test --workspace` printed `989 passed; 0 failed` with the game present and
`989 passed; 0 failed` with it absent. **116 tests** — the reproduction against a real
save, the renderer against real sprites, every screen test — did not exist on CI, and
nothing said so. Two of those files used `env::var` with no fall back, so all 22 of their
tests had never run on any machine that did not export `LORDS2_DIR`; twelve of them failed
the moment they were made to run, because they took the *assets* and the *position* from
the same directory.

`crates/l2-testkit/tests/census.rs` now reads the source, works out which gate each
`#[test]` sits behind, and asserts the count against a written-down inventory. Adding a
gate fails the build until the inventory is updated. It also prints what the current
environment satisfies, so a run that asserted an eighth of what it looks like it asserted
says so.

The general form, and the one worth keeping: **a test's failure mode is not only "wrong
answer" — it is also "did not run" and "ran against something else".** This project had
careful machinery for the first and none at all for the other two. And the naming matters
more than it looks: three of these defects are downstream of calling a file "shipped".

**C24 — The oracle existed, printed to a console, and was wired to nothing.**

`tools/oracle/*.ps1` has read the battle and economy tables straight out of `Lords2.exe`
since C14. `crates/l2-view/tests/install.rs` has had `va_to_offset` — the four lines that
turn a documented virtual address into a file offset — since it was written. Neither had
ever been applied to `l2-kingdom::tables` or `l2-sim`, the two crates carrying the most
hand-transcribed numbers, and the test guarding `l2-sim`'s was named
`the_unit_size_and_footprint_tables_match_the_oracle_reading` while opening nothing: it
compared a constant against a second spelling of itself, both typed on the same afternoon.

C10, C12, C17 and C20 are four instances of one mechanism — **nothing re-checks** — and
the apparatus to fix it was already in the tree, split across a script that printed and a
test that never ran anywhere useful.

`crates/l2-sim/tests/oracle.rs` and `crates/l2-kingdom/tests/oracle.rs` close it: 66
battle values and eleven economy tables, read from the image. Two things fell out
immediately. `MissileStats::range` is in cells and the table stores **eighths of a cell**,
which nothing had ever confirmed. And 25 of `g_healthDeltaTable`'s 30 entries had never
been read by any test at all, because every county in the England turn-one position sits
on Normal rations at health band 3 — the reproduction test is strong evidence about the
rules that *one save exercises*, and silent about everything else.

The general form: **"we have a tool that could check this" is not a check.** The distance
between a script that prints a table and a test that fails is the whole of the value.

**C25 — Plane-0 bits `0x40` and `0x80` were labelled the wrong way round. `0x40` is the
county town; `0x80` is the castle.**

Found while naming functions by their `L2.eng` string ids, and it is the game correcting us
in English. `TileInfo_Draw` (`0x0041C208`) is the tile info panel, and it picks a heading out
of group 30 by the plane-0 flag byte. Bit `0x40` gets string 7, **"County town."**, with
description `0x1B`, *"Your troops may capture a castleless county by attacking its county
town."* Bit `0x80` gets string 8, **"Castle."**, or 14/15 for under construction and repair.

Three independent checks agree, and each could have failed:

- **The construction field.** The `0x80` branch reads county `+0x1C3` to choose between
  "Castle.", "Castle under construction." and "Castle under repair." `Army_BeginSiege`
  refuses a siege when that same field is 1. Only the castle has a build state.
- **The garrison tile.** `County_FindCastleTile` (`0x00468121`) scans for bit `0x80` and
  writes the result to county `+0x74`/`+0x75` — which is exactly the tile `FUN_004A79A3`
  moves a garrisoning army onto. Its sibling `County_FindTownTile` (`0x00467FD1`) scans for
  bit `0x40` and produces the county *anchor*, the tile `County_FindFreeRoadTile` searches
  around.
- **The mercenary offer.** The `0x40` branch prints *"Mercenaries are available for hire in
  the county."* when county `+0x1AD` is set — the field `Mercenary_AdvanceAll` writes. Bands
  are offered in the town, not the keep.

A fourth, weaker but pleasing: `TileInfo_DrawCastle` prints the barracks capacity out of
`g_castleGarrisonCap`, the same table `FUN_004A79A3` caps a garrison against.

**What was wrong, and what was not.** No code behaves differently — the bits were read
consistently everywhere, just described backwards. `Unit_TryEnterTile`'s return codes are
unchanged; only the words beside them move. `Map_LoadPlanes`'s plane-4 dispatch reads *better*
after the swap: `0x40` → `Merchant_RouteAppend` makes a trade route a list of **market
towns**, and `0x80` → `PlayerStart_Record` makes a player start a **castle**. Both are what
those things are, which is a sign the original label was never checked against meaning.

**`docs/formats/maps-layers.md` had the evidence sitting beside the wrong conclusion.** Its
table says bit `0x40` draws from the **town** graphics bank (`Town1a.pl8`, frames 0–3) and
labels the row "castle site"; bit `0x80` draws from the **base** bank and is labelled
"settlement". The bank names come from the game's own `g_resourceTable`. A row that reads
"town → castle site" should have been read twice. That table is marked **[V]**, and it was:
the *counts* were verified, the *names* were never evidence at all. This is C21's second
lesson in a smaller frame — verification attaches to the measurement, not to the sentence it
sits in.

One loose end kept deliberately: the `0x80` rows in that table are split between the base
bank (1,681 tiles, frames 6–21) and the town bank (725, frames 0/20/30), and the base-bank
range is the same one bit `0x10` uses for dwelling plots. That is consistent with `0x80`
being an *empty plot the castle gets built on* — `County_FindCastleTile` stamps terrain
`0x14` over it at load, and `Unit_TryEnterTile` has a special case for exactly that stamp —
but the 725-tile town-bank half is not explained, and the table has not been rewritten on
one function's say-so.

**C26 — Taxation's effect on happiness was one rule. It is two fields with two rules, and
the second is not a formula at all.**

We wrote `min(5 - rate, 0)` and applied it to both. `Tax_RecomputePreview` (`0x0044B80B`)
is the only writer of either field anywhere in the binary, and its last three statements
separate them:

```c
county[0x0F] = 5 - taxRate;                        /* the county's own term  */
county[0x16] = g_taxHappinessOther[taxRate * 4];   /* every OTHER county's   */
Tax_SumEmpireHappiness(county.owner);
```

`5 - rate` was real, and it belongs to `+0x0F`. `+0x16` — what one county's tax rate does
to the rest of its realm — is a **lookup**, `g_taxHappinessOther` (`0x004D63D8`), 51 `i32`
entries now transcribed as `TAX_HAPPINESS_OTHER`. Its shape is nothing like a formula:
**flat zero from rate 0 through 19**, then a shallow ramp reaching only **−15** at rate 50.
Ours bit from rate 6 and reached −45. Taxing at 19% costs your other counties nothing at
all, and we had it costing them thirteen.

**The two agree at six of the fifty-one rates. One of the six is rate 0, and rate 0 is the
tax rate of every county in the only save the suite tested against.** So the rule was wrong
at 45 of its 51 possible inputs and 932 tests passed. Not a near miss that the fixture
happened not to catch — a rule that was wrong almost everywhere, sitting behind a green
suite, because the fixture exercised one value of its input.

**The general form: a rule can be wrong almost everywhere and still be invisible, when the
only fixture exercises one value of its input.** Coverage of the *code* says nothing here.
`empire_contribution` was called constantly and every call passed rate 0.

**It happened twice in the same session, which is why it is a shape and not an anecdote.**
The cattle-tending rule had the identical failure: `Herd_SeasonTick` passes a labour figure
into the births/deaths function and staffing is `PctOf(labour, herd * 3)`, so an unattended
herd dies. Ours took no labour argument at all — a county could keep cattle with nobody
tending them and lose nothing — and every test passed, because the tested save's counties
are not short-staffed. Two rules, both wrong only in a region one scenario never visits,
found the same afternoon.

That is the argument for the two corrections either side of this one. **C23** is why there
is now more than one fixture and why a fixture is a name plus a fingerprint; **C24** is why
the tables are diffed against the image instead of against a second copy of themselves.
`TAX_HAPPINESS_OTHER` has since been checked byte for byte against `Lords2.exe`, **51 of 51
exact** — and that check earns its place twice over, because the hand transcription *did*
go wrong the first time, off by one at the start of the ramp. A table typed by a person is
a place a silent error lives; the oracle is the only thing that reads it back.

Two smaller things fell out of the same reading. **The tax ceiling is 50, not 100** —
`Tax_Increase` (`0x0043AA32`) guards `taxRate < 0x32`, and the table's 51 entries say the
same thing independently. And `MAX_TAX_RATE` had been living in `l2-game`, where a rule has
no business being: 50 is not a widget's range, it is part of the ruleset, and a mod that
replaces the table is entitled to move it. An arithmetic bound standing in for a rule is
its own small version of this entry.

**C27 — A rule with no way in is not a rule the game has. The grain economy was
finished, tested and unreachable for the whole of a game.**

Every county of the England turn-one position stores `fieldsGrain = 0`, all fourteen. The
three field counts had exactly one production writer in this tree — the scenario import —
so sowing, the seed debit, growth, harvest, `g_grainLabourDivisor`,
`g_grainMaxSacksPerField` and the fallow-per-two-grain rule were all correct, all covered
by tests, and **none of them could ever run for a human player**. `AI_ManageFields` does not
close the gap either: its ladder only adds *fallow* fields, and the grain comes from a
lord's farming style, which this tree does not implement. Nobody farmed.

Nothing was broken and no test could have caught it, because every test that exercised the
grain rules set `fieldsGrain` itself. **Coverage of a rule says nothing about whether the
game can reach it.** That is C26's shape moved one level out: there a rule was wrong at 45
of its 51 inputs because the fixture exercised one; here a rule was right and unreachable
because no fixture had to reach it.

Two things fell out of fixing it, and both are the same lesson about *where* state lives.

**The counts are a cache.** `County_RecountFields` (`FUN_00469B8D`) rebuilds them every
time from the terrain byte of the twenty map tiles in `g_countyFieldTiles`. A field's type
is a property of the **map**, and the county record only counts them — which is why nothing
in the county record could be the writer. Applying the ladder to the tiles the save names
reproduces all three stored counts for all fourteen counties, 168 field tiles, with none of
our rules in the loop.

**And the map was empty.** `Kingdom::new` builds a blank `CampaignMap` and `l2-scenario`
never overwrote it, so every imported game had been running its pathfinding, its
field-crossing and its trampling against 4,096 zero tiles since those were written. The
planes are in the save — `g_tiles` is block 0, at file offset 0 — and were simply not read.
A whole plane of simulation input can be missing without a single test noticing, when every
test that needs a map builds its own.

One more correction rides along, and it is C25 confirmed from the other side.
`Map_Click`'s plane-0 dispatch is three bits in one order:

```c
if      (flags & 0x80) { ...the industry / castle toggle ladder... }
else if (flags & 0x40) { g_screenId = 2; Village_Draw(1); }   /* the village */
else if (flags & 0x20) { g_screenId = 4; }                    /* the field brush */
```

**Clicking a `0x40` tile opens the village**, so `0x40` is the county town — which C25
argued from `L2.eng`'s own words and this settles from behaviour. `crates/l2-kingdom`'s
constant is still called `flags::CASTLE`, because `cost_map` and `movement` read it under
that name in half a dozen places and renaming it means re-reading `Unit_TryEnterTile`'s
return codes alongside; the doc comment carries the correction instead. The fixture agrees
on all three bits and closes C25's loose end as well: every county's `0x80` tiles are one
iron site, one stone, one weapons, one wood and a 2 × 2 block at terrain `0x14` or `0x17`,
and the five counties with a `0x17` block are exactly the five that start owned. `0x17` is
a standing castle; `0x14` is the plot it gets built on.

**C28 — Two names were doing work nobody had checked, and both were wrong. A name is a
claim, and a claim that reaches an artefact gets believed.**

Two unrelated corrections landed in one pass and they are the same failure.

**`conquest::find_defender` was a plausible reading, recorded as `[I]`, and then relied
on.** `docs/armies.md` §9 modelled `FUN_0046D42C` as *"the lowest-numbered army of the
county's owner standing in the county"* and said in the same sentence that the function had
not been read. The crate implemented it, four tests exercised it, and it was **wrong in both
halves**: the original scans a 4×4 tile block around the county *town* and returns the
**largest** army, not a slot-order scan of the county. Neither half of the guess survived.

The part worth keeping is *how it was checked*. The two readings disagree on a file that has
been in `LORDS2_FIXTURES` the whole time — `battle-before.sav` has county 2's town at
(31, 50) and its garrison on the castle tile at (30, 46), four rows away, so ours returned an
army and the original returns nothing. That test was written first, watched fail, and then
fixed (`crates/l2-kingdom/tests/defence.rs`, which runs *both* readings on the same bytes and
asserts they differ). **A correction that cannot be made to fail first is not a correction, it
is a preference** — C12 in the shape it takes when the thing under test is a model rather than
a number.

**`g_deterministicBattle` was `g_multiplayer` all along**, and `docs/audit-method.md`'s own
measurement had already flagged it without anyone acting: an *inferred* global name reaches
the decompiled corpus through `ApplySymbols`, where it appeared **155 times** with no marker
saying it was a guess, while the caveat sat in a document. `0x00553030` is written to 1 in
exactly one place — after a DirectPlay session opens — and to 0 on every teardown. The
determinism-shaped effects that produced the name (cyclic battle selectors, no strength
jitter, the side-neutral `TROOPS.ENG`) are *consequences* of being on a wire.

Nothing in `docs/netcode.md` had leaned on it — its case for lockstep is made from first
principles about our own engine and never cites the flag — so no argument had to be
withdrawn. But it was one refactor away from mattering: a name asserting that *the original
has a deterministic-battle mode* is exactly the sort of thing a later pass builds on. The
cost of being wrong here was 155 call sites and a rename across nine documents; the cost of
being wrong one document later would have been an architecture.

**`hypotheses.json` is the file that exists to stop this, and its own `confidence` key had
two shapes** — 36 entries an enum, 29 a paragraph — so nothing could count it. It is one enum
now (`docs/method.md` §7.6), the prose moved verbatim to `caveat`, and both rules are checked
by `tools/symbols/symbols_md.js`, which CI already runs: a non-enum `confidence` fails, and so
does an address that sits in `symbols.json` *and* `hypotheses.json` at once. The tier
discipline was written down for a year and enforced by nothing.

**C29 — `Map_PlaceDwellings` placed no dwelling. It is the starting-field allocator, and it
is scaled by difficulty.**

Found while typing the campaign tile grid, and it is C25's shape in miniature: the evidence
was already written down beside the wrong name. The function's own `symbols.json` comment
said it sweeps for **plane-0 bit `0x20`** and writes "Roads frame 80, 84 or 104 — the three
crop states", and `maps-layers.md` §2.4 had *already* corrected `maps.md`'s reading of bit
`0x20` from "dwelling / housing site" to **farmland**. The name kept the superseded reading;
nobody re-read it against the sentence underneath it.

Renamed to **`Map_PlaceStartingFields`** (`0x00467A36`). Retyped, it says more than the old
comment did: for each of a county's farm tiles, in scan order, it writes `content` and
`frame` by **difficulty**, so the harder settings start you with fewer improved fields —

| difficulty | pasture (`content` `0x14`) | fallow (`1`) | wild (`0`) |
|---|---|---|---|
| 0, easiest | first 8 | the rest | — |
| 1 | first 6 | next 2 | 8th on |
| 2 | first 4 | next 2 | 6th on |
| 3, hardest | first 4 | — | 5th on |

— with `frame` = base + `((storedFrame + 0xB0) & 3)`, a four-variant pick, and the three
bases 104 / 84 / 80 being exactly the crop frames §2.4 identifies farmland by. That the
three `content` values line up with `County_RecountFields`'s pasture / fallow / wild bands is
the check that could have failed.

**The real dwellings were two unnamed functions all along**, and naming them closes
`maps-layers.md` §8's "what gets built on the four `0x10` plots": `County_FindDwellingPlots`
(`0x00468C41`) records the four plots, `County_UpdateDwellings` (`0x004684C6`) raises or
razes a dwelling on each once a season from the county's population.

**What this costs, and what it does not.** No behaviour changes and no other document
depended on the name; `Map_InitScenario`'s call-order comment is the only other place it
appeared. The lesson is the one C25 already paid for once — **a name is not evidence, and a
`verified` mark attaches to the measurement, not to the label on top of it.** Two documents
in this tree had the correct reading of bit `0x20` while a third named a function after the
wrong one, for long enough that the function was cited by name in the scenario bring-up.

## Open questions

- **The difficulty curve 116/108/100/92/84 rests on the decompilation alone.** Making the
  shipped `TROOPS*.ENG` an oracle for it was tried and does not work: the non-Normal rows
  are hand-authored leftovers (402 of 3,080 populated in `TROOPS.ENG`, ratios running 1.20
  to 2.00 with 116 % nowhere among them) which the engine overwrites. `docs/formats/eng.md`
  §3.2 said those rows were "all zeros" and that is corrected there. The percentages are
  immediates in the caller of `FUN_00404D6B` and would need `initconsts.ps1`'s technique to
  recover.
- **The labour allocator never reruns.** `sum(labour) == population` holds on an imported
  kingdom and is frozen at the import's figure ten seasons later while the population moves
  under it; `FUN_0044F6E7` reallocates every season. Found by replacing C12's shape in
  `ten_more_seasons_...`, pinned there, and named at the assertion that will go red when
  somebody writes the rule.
- `WEATHER_JITTER_BOUND` in `crates/l2-kingdom` is **invented**. `docs/kingdom.md` §7.3
  gives the weather jitter as `random/8` with no stated range, which is not implementable —
  the constant is the one number in that crate with no evidence behind it, and it is marked
  as such at its definition. It needs tracing before any weather behaviour is trusted.
- The four map planes whose meaning is inferred rather than proven (graphics bank,
  descriptor index, multi-tile object part), and the exact tile → lattice mapping,
  whose best affine fit reaches only 72%.
- 23 files that use supported encodings but fail the end-offset invariant, pinned in
  `KNOWN_FAILING`.
- `Font_c2.pl8` declares RLE but its frames occupy exactly `width × height`.
- `Title.pl8` decodes with correct geometry but no shipped palette colours it.
- The type-4 apex pair, where the stored data and the shipped blitter disagree.
- PL8 header fields at 0x04, 0x06, 0x07.
